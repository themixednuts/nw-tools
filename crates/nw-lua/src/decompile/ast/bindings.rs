//! Binding-aware AST queries and rewrites.

use std::collections::HashMap;

use super::{BindingId, Block, Expr, FuncBody, Name, Stmt, TableField};

/// Source contexts in which one binding is read.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BindingUsage {
    receiver_reads: usize,
    value_reads: usize,
}

impl BindingUsage {
    #[must_use]
    pub(crate) const fn is_receiver_only(self) -> bool {
        self.receiver_reads > 0 && self.value_reads == 0
    }

    #[must_use]
    pub(crate) const fn receiver_reads(self) -> usize {
        self.receiver_reads
    }

    #[must_use]
    pub(crate) const fn value_reads(self) -> usize {
        self.value_reads
    }

    #[must_use]
    const fn is_used(self) -> bool {
        self.receiver_reads > 0 || self.value_reads > 0
    }
}

/// Return how `binding` is read in `block`, distinguishing `binding.member`
/// receiver positions from ordinary value positions.
#[must_use]
pub(crate) fn binding_usage_in_block(block: &Block, binding: &BindingId) -> BindingUsage {
    binding_usages_in_block(block)
        .get(binding)
        .copied()
        .unwrap_or_default()
}

/// Return read roles for every binding referenced from `block` in one walk.
#[must_use]
pub(crate) fn binding_usages_in_block(block: &Block) -> HashMap<BindingId, BindingUsage> {
    let mut usages = HashMap::new();
    collect_block_usages(block, &mut usages);
    usages
}

/// Return whether a function body reads `binding`.
#[must_use]
pub(crate) fn binding_references_in_func_body(body: &FuncBody, binding: &BindingId) -> bool {
    binding_usage_in_block(&body.body, binding).is_used()
}

fn collect_block_usages(block: &Block, usages: &mut HashMap<BindingId, BindingUsage>) {
    for stmt in &block.0 {
        collect_stmt_usages(stmt, usages);
    }
}

fn collect_stmt_usages(stmt: &Stmt, usages: &mut HashMap<BindingId, BindingUsage>) {
    match stmt {
        Stmt::Local { values, .. } => collect_exprs_usages(values, usages),
        Stmt::Assign { targets, values } => {
            collect_exprs_usages(targets, usages);
            collect_exprs_usages(values, usages);
        }
        Stmt::Call(expr) => collect_expr_usages(expr, usages),
        Stmt::Do(body) => collect_block_usages(body, usages),
        Stmt::While { cond, body } => {
            collect_expr_usages(cond, usages);
            collect_block_usages(body, usages);
        }
        Stmt::Repeat { body, cond } => {
            collect_block_usages(body, usages);
            collect_expr_usages(cond, usages);
        }
        Stmt::If { arms, else_ } => {
            for (cond, body) in arms {
                collect_expr_usages(cond, usages);
                collect_block_usages(body, usages);
            }
            if let Some(body) = else_ {
                collect_block_usages(body, usages);
            }
        }
        Stmt::NumericFor {
            start,
            stop,
            step,
            body,
            ..
        } => {
            collect_expr_usages(start, usages);
            collect_expr_usages(stop, usages);
            if let Some(step) = step {
                collect_expr_usages(step, usages);
            }
            collect_block_usages(body, usages);
        }
        Stmt::GenericFor { exprs, body, .. } => {
            collect_exprs_usages(exprs, usages);
            collect_block_usages(body, usages);
        }
        Stmt::Function { body, local, name } => {
            if !local {
                record_value_usage(name, usages);
            }
            collect_block_usages(&body.body, usages);
        }
        Stmt::FunctionDecl { name, body } => {
            if let Some(base) = name.path.first() {
                if name.method.is_some() || name.path.len() > 1 {
                    record_receiver_usage(base, usages);
                } else {
                    record_value_usage(base, usages);
                }
            }
            collect_block_usages(&body.body, usages);
        }
        Stmt::Return(values) => collect_exprs_usages(values, usages),
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => {}
    }
}

fn collect_exprs_usages(exprs: &[Expr], usages: &mut HashMap<BindingId, BindingUsage>) {
    for expr in exprs {
        collect_expr_usages(expr, usages);
    }
}

fn collect_expr_usages(expr: &Expr, usages: &mut HashMap<BindingId, BindingUsage>) {
    match expr {
        Expr::Name(name) => record_value_usage(name, usages),
        Expr::Index { obj, key } => {
            if let Expr::Name(name) = obj.as_ref() {
                record_receiver_usage(name, usages);
            } else {
                collect_expr_usages(obj, usages);
            }
            collect_expr_usages(key, usages);
        }
        Expr::Field { obj, .. } => {
            if let Expr::Name(name) = obj.as_ref() {
                record_receiver_usage(name, usages);
            } else {
                collect_expr_usages(obj, usages);
            }
        }
        Expr::Call { func, args, .. } => {
            collect_expr_usages(func, usages);
            collect_exprs_usages(args, usages);
        }
        Expr::Function(body) => collect_block_usages(&body.body, usages),
        Expr::Table(fields) => {
            for field in fields {
                match field {
                    TableField::List(value) | TableField::Named { value, .. } => {
                        collect_expr_usages(value, usages);
                    }
                    TableField::ExprKey { key, value } => {
                        collect_expr_usages(key, usages);
                        collect_expr_usages(value, usages);
                    }
                }
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_usages(lhs, usages);
            collect_expr_usages(rhs, usages);
        }
        Expr::Unary { operand, .. } | Expr::Paren(operand) => {
            collect_expr_usages(operand, usages);
        }
        Expr::Nil
        | Expr::True
        | Expr::False
        | Expr::VarArg
        | Expr::Number(_)
        | Expr::Integer(_)
        | Expr::Str(_)
        | Expr::Global(_) => {}
    }
}

fn record_receiver_usage(name: &Name, usages: &mut HashMap<BindingId, BindingUsage>) {
    if let Some(binding) = name.binding() {
        usages.entry(binding.clone()).or_default().receiver_reads += 1;
    }
}

fn record_value_usage(name: &Name, usages: &mut HashMap<BindingId, BindingUsage>) {
    if let Some(binding) = name.binding() {
        usages.entry(binding.clone()).or_default().value_reads += 1;
    }
}

/// Return whether `spelling` can replace `binding` without colliding with a
/// different local-like identity or changing a global lookup in the tree.
#[must_use]
pub(crate) fn binding_spelling_available_in_block(
    block: &Block,
    binding: &BindingId,
    spelling: &[u8],
) -> bool {
    let mut inspection = RenameInspection::new(binding, spelling);
    inspection.inspect_block(block);
    inspection.target_seen && !inspection.collision
}

/// Function-body form of [`binding_spelling_available_in_block`].
#[must_use]
pub(crate) fn binding_spelling_available_in_func_body(
    body: &FuncBody,
    binding: &BindingId,
    spelling: &[u8],
) -> bool {
    let mut inspection = RenameInspection::new(binding, spelling);
    inspection.inspect_func_body(body);
    inspection.target_seen && !inspection.collision
}

struct RenameInspection<'a> {
    target: &'a BindingId,
    spelling: &'a [u8],
    target_seen: bool,
    collision: bool,
}

impl<'a> RenameInspection<'a> {
    fn new(target: &'a BindingId, spelling: &'a [u8]) -> Self {
        Self {
            target,
            spelling,
            target_seen: false,
            collision: false,
        }
    }

    fn inspect_name(&mut self, name: &Name) {
        match name.binding() {
            Some(binding) if binding == self.target => self.target_seen = true,
            Some(_) if name.as_bytes() == self.spelling => self.collision = true,
            Some(_) | None => {}
        }
    }

    fn inspect_global(&mut self, name: &[u8]) {
        if name == self.spelling {
            self.collision = true;
        }
    }

    fn inspect_block(&mut self, block: &Block) {
        for stmt in &block.0 {
            self.inspect_stmt(stmt);
        }
    }

    fn inspect_func_body(&mut self, body: &FuncBody) {
        if let Some(receiver) = &body.implicit_receiver {
            self.inspect_name(receiver);
        }
        for param in &body.params {
            self.inspect_name(param);
        }
        self.inspect_block(&body.body);
    }

    fn inspect_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Local { names, values, .. } => {
                self.inspect_names(names);
                self.inspect_exprs(values);
            }
            Stmt::Assign { targets, values } => {
                self.inspect_exprs(targets);
                self.inspect_exprs(values);
            }
            Stmt::Call(expr) => self.inspect_expr(expr),
            Stmt::Do(body) => self.inspect_block(body),
            Stmt::While { cond, body } => {
                self.inspect_expr(cond);
                self.inspect_block(body);
            }
            Stmt::Repeat { body, cond } => {
                self.inspect_block(body);
                self.inspect_expr(cond);
            }
            Stmt::If { arms, else_ } => {
                for (cond, body) in arms {
                    self.inspect_expr(cond);
                    self.inspect_block(body);
                }
                if let Some(body) = else_ {
                    self.inspect_block(body);
                }
            }
            Stmt::NumericFor {
                var,
                start,
                stop,
                step,
                body,
            } => {
                self.inspect_name(var);
                self.inspect_expr(start);
                self.inspect_expr(stop);
                if let Some(step) = step {
                    self.inspect_expr(step);
                }
                self.inspect_block(body);
            }
            Stmt::GenericFor { names, exprs, body } => {
                self.inspect_names(names);
                self.inspect_exprs(exprs);
                self.inspect_block(body);
            }
            Stmt::Function { name, body, .. } => {
                self.inspect_name(name);
                self.inspect_func_body(body);
            }
            Stmt::FunctionDecl { name, body } => {
                self.inspect_names(&name.path);
                self.inspect_func_body(body);
            }
            Stmt::Return(values) => self.inspect_exprs(values),
            Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => {}
        }
    }

    fn inspect_names(&mut self, names: &[Name]) {
        for name in names {
            self.inspect_name(name);
        }
    }

    fn inspect_exprs(&mut self, exprs: &[Expr]) {
        for expr in exprs {
            self.inspect_expr(expr);
        }
    }

    fn inspect_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Name(name) => self.inspect_name(name),
            Expr::Global(name) => self.inspect_global(name),
            Expr::Index { obj, key } => {
                self.inspect_expr(obj);
                self.inspect_expr(key);
            }
            Expr::Field { obj, .. } => self.inspect_expr(obj),
            Expr::Call { func, args, .. } => {
                self.inspect_expr(func);
                self.inspect_exprs(args);
            }
            Expr::Function(body) => self.inspect_func_body(body),
            Expr::Table(fields) => {
                for field in fields {
                    match field {
                        TableField::List(value) | TableField::Named { value, .. } => {
                            self.inspect_expr(value);
                        }
                        TableField::ExprKey { key, value } => {
                            self.inspect_expr(key);
                            self.inspect_expr(value);
                        }
                    }
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.inspect_expr(lhs);
                self.inspect_expr(rhs);
            }
            Expr::Unary { operand, .. } | Expr::Paren(operand) => self.inspect_expr(operand),
            Expr::Nil
            | Expr::True
            | Expr::False
            | Expr::VarArg
            | Expr::Number(_)
            | Expr::Integer(_)
            | Expr::Str(_) => {}
        }
    }
}

/// Rename every declaration and reference carrying `binding` in `block`.
pub(crate) fn rename_binding_in_block(block: &mut Block, binding: &BindingId, spelling: &[u8]) {
    for stmt in &mut block.0 {
        rename_binding_in_stmt(stmt, binding, spelling);
    }
}

/// Rename every declaration and reference carrying `binding` in `body`.
pub(crate) fn rename_binding_in_func_body(
    body: &mut FuncBody,
    binding: &BindingId,
    spelling: &[u8],
) {
    if let Some(receiver) = &mut body.implicit_receiver {
        rename_name(receiver, binding, spelling);
    }
    rename_names(&mut body.params, binding, spelling);
    rename_binding_in_block(&mut body.body, binding, spelling);
}

fn rename_binding_in_stmt(stmt: &mut Stmt, binding: &BindingId, spelling: &[u8]) {
    match stmt {
        Stmt::Local { names, values, .. } => {
            rename_names(names, binding, spelling);
            rename_exprs(values, binding, spelling);
        }
        Stmt::Assign { targets, values } => {
            rename_exprs(targets, binding, spelling);
            rename_exprs(values, binding, spelling);
        }
        Stmt::Call(expr) => rename_binding_in_expr(expr, binding, spelling),
        Stmt::Do(body) => rename_binding_in_block(body, binding, spelling),
        Stmt::While { cond, body } => {
            rename_binding_in_expr(cond, binding, spelling);
            rename_binding_in_block(body, binding, spelling);
        }
        Stmt::Repeat { body, cond } => {
            rename_binding_in_block(body, binding, spelling);
            rename_binding_in_expr(cond, binding, spelling);
        }
        Stmt::If { arms, else_ } => {
            for (cond, body) in arms {
                rename_binding_in_expr(cond, binding, spelling);
                rename_binding_in_block(body, binding, spelling);
            }
            if let Some(body) = else_ {
                rename_binding_in_block(body, binding, spelling);
            }
        }
        Stmt::NumericFor {
            var,
            start,
            stop,
            step,
            body,
        } => {
            rename_name(var, binding, spelling);
            rename_binding_in_expr(start, binding, spelling);
            rename_binding_in_expr(stop, binding, spelling);
            if let Some(step) = step {
                rename_binding_in_expr(step, binding, spelling);
            }
            rename_binding_in_block(body, binding, spelling);
        }
        Stmt::GenericFor { names, exprs, body } => {
            rename_names(names, binding, spelling);
            rename_exprs(exprs, binding, spelling);
            rename_binding_in_block(body, binding, spelling);
        }
        Stmt::Function { name, body, .. } => {
            rename_name(name, binding, spelling);
            rename_binding_in_func_body(body, binding, spelling);
        }
        Stmt::FunctionDecl { name, body } => {
            rename_names(&mut name.path, binding, spelling);
            rename_binding_in_func_body(body, binding, spelling);
        }
        Stmt::Return(values) => rename_exprs(values, binding, spelling),
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => {}
    }
}

fn rename_exprs(exprs: &mut [Expr], binding: &BindingId, spelling: &[u8]) {
    for expr in exprs {
        rename_binding_in_expr(expr, binding, spelling);
    }
}

fn rename_binding_in_expr(expr: &mut Expr, binding: &BindingId, spelling: &[u8]) {
    match expr {
        Expr::Name(name) => rename_name(name, binding, spelling),
        Expr::Index { obj, key } => {
            rename_binding_in_expr(obj, binding, spelling);
            rename_binding_in_expr(key, binding, spelling);
        }
        Expr::Field { obj, .. } => rename_binding_in_expr(obj, binding, spelling),
        Expr::Call { func, args, .. } => {
            rename_binding_in_expr(func, binding, spelling);
            rename_exprs(args, binding, spelling);
        }
        Expr::Function(body) => rename_binding_in_func_body(body, binding, spelling),
        Expr::Table(fields) => {
            for field in fields {
                match field {
                    TableField::List(value) | TableField::Named { value, .. } => {
                        rename_binding_in_expr(value, binding, spelling);
                    }
                    TableField::ExprKey { key, value } => {
                        rename_binding_in_expr(key, binding, spelling);
                        rename_binding_in_expr(value, binding, spelling);
                    }
                }
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            rename_binding_in_expr(lhs, binding, spelling);
            rename_binding_in_expr(rhs, binding, spelling);
        }
        Expr::Unary { operand, .. } | Expr::Paren(operand) => {
            rename_binding_in_expr(operand, binding, spelling);
        }
        Expr::Nil
        | Expr::True
        | Expr::False
        | Expr::VarArg
        | Expr::Number(_)
        | Expr::Integer(_)
        | Expr::Str(_)
        | Expr::Global(_) => {}
    }
}

fn rename_names(names: &mut [Name], binding: &BindingId, spelling: &[u8]) {
    for name in names {
        rename_name(name, binding, spelling);
    }
}

fn rename_name(name: &mut Name, binding: &BindingId, spelling: &[u8]) {
    if name.binding() == Some(binding) {
        *name = name.renamed(spelling.to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile::ast::FunctionId;

    fn bound(name: &str, binding: &BindingId) -> Name {
        Name::from(name).with_binding(binding.clone())
    }

    #[test]
    fn binding_rename_crosses_closures_without_touching_a_shadow() {
        let function = FunctionId::root();
        let target = BindingId::synthetic(&function, 0);
        let shadow = BindingId::synthetic(&function, 1);
        let mut block = Block::new(vec![
            Stmt::Local {
                names: vec![bound("v0", &target)],
                attribs: Vec::new(),
                values: vec![Expr::Table(Vec::new())],
            },
            Stmt::Do(Block::new(vec![
                Stmt::Local {
                    names: vec![bound("v0", &shadow)],
                    attribs: Vec::new(),
                    values: vec![Expr::Number(1.0)],
                },
                Stmt::Return(vec![Expr::Name(bound("v0", &shadow))]),
            ])),
            Stmt::Return(vec![Expr::Function(FuncBody::new(
                Vec::new(),
                false,
                Block::new(vec![Stmt::Return(vec![Expr::Name(bound("v0", &target))])]),
            ))]),
        ]);

        rename_binding_in_block(&mut block, &target, b"Module");
        let rendered = format!("{block:?}");
        assert!(
            rendered.contains("Name(\"Module\", Synthetic"),
            "{rendered}"
        );
        assert!(rendered.contains("Name(\"v0\", Synthetic"), "{rendered}");
    }

    #[test]
    fn rename_availability_and_receiver_usage_use_binding_identity() {
        let function = FunctionId::root();
        let target = BindingId::synthetic(&function, 0);
        let other = BindingId::synthetic(&function, 1);
        let receiver = Expr::Field {
            obj: Box::new(Expr::Name(bound("v0", &target))),
            name: Name::new("field"),
        };
        let block = Block::new(vec![
            Stmt::Local {
                names: vec![bound("self", &other)],
                attribs: Vec::new(),
                values: vec![Expr::Number(1.0)],
            },
            Stmt::Return(vec![receiver, Expr::Name(bound("v0", &other))]),
        ]);

        assert!(binding_usage_in_block(&block, &target).is_receiver_only());
        assert!(!binding_spelling_available_in_block(
            &block, &target, b"self"
        ));
    }
}
