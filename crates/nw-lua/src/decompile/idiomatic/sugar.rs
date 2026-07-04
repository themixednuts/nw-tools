use crate::decompile::{
    ast::{Block, Expr, FuncBody, FunctionName, Name, Stmt},
    naming::is_valid_identifier,
};

use super::engine::{CleanContext, Rewrite, Rule};

pub struct AssignmentFunctionSugar;

impl Rule for AssignmentFunctionSugar {
    fn rewrite_stmt(&self, stmt: Stmt, _ctx: &CleanContext) -> Rewrite<Stmt> {
        let Stmt::Assign { targets, values } = stmt else {
            return Rewrite::unchanged(stmt);
        };
        let ([target], [Expr::Function(body)]) = (targets.as_slice(), values.as_slice()) else {
            return Rewrite::unchanged(Stmt::Assign { targets, values });
        };
        let Some(name) = function_name_from_target(target) else {
            return Rewrite::unchanged(Stmt::Assign { targets, values });
        };
        Rewrite::changed(Stmt::FunctionDecl {
            name,
            body: body.clone(),
        })
    }
}

pub struct LocalFunctionSugar;

impl Rule for LocalFunctionSugar {
    fn rewrite_stmt(&self, stmt: Stmt, _ctx: &CleanContext) -> Rewrite<Stmt> {
        let Stmt::Local {
            names,
            attribs,
            values,
        } = stmt
        else {
            return Rewrite::unchanged(stmt);
        };
        let ([name], [Expr::Function(body)]) = (names.as_slice(), values.as_slice()) else {
            return Rewrite::unchanged(Stmt::Local {
                names,
                attribs,
                values,
            });
        };
        if !attribs.is_empty() || body_references_name(body, name) {
            return Rewrite::unchanged(Stmt::Local {
                names,
                attribs,
                values,
            });
        }
        Rewrite::changed(Stmt::Function {
            name: name.clone(),
            body: body.clone(),
            local: true,
        })
    }
}

pub struct RecursiveLocalFunctionSugar;

impl Rule for RecursiveLocalFunctionSugar {
    fn rewrite_block(&self, block: Block, _ctx: &CleanContext) -> Rewrite<Block> {
        let mut out = Vec::with_capacity(block.0.len());
        let mut changed = false;
        let mut iter = block.0.into_iter().peekable();
        while let Some(stmt) = iter.next() {
            let Some(function_stmt) = recursive_local_function(&stmt, iter.peek()) else {
                out.push(stmt);
                continue;
            };
            let _ = iter.next();
            out.push(function_stmt);
            changed = true;
        }
        if changed {
            Rewrite::changed(Block::new(out))
        } else {
            Rewrite::unchanged(Block::new(out))
        }
    }
}

pub struct MethodDeclarationSugar;

impl Rule for MethodDeclarationSugar {
    fn rewrite_stmt(&self, stmt: Stmt, _ctx: &CleanContext) -> Rewrite<Stmt> {
        let Stmt::FunctionDecl { name, mut body } = stmt else {
            return Rewrite::unchanged(stmt);
        };
        if name.method.is_some() || name.path.len() < 2 || body.params.is_empty() {
            return Rewrite::unchanged(Stmt::FunctionDecl { name, body });
        }

        let first = body.params[0].clone();
        if first.as_bytes() == b"self" {
            return Rewrite::changed(methodize(name, body));
        }
        let usage = receiver_usage_in_block(&body.body, first.as_bytes());
        if !first.is_synthetic()
            || block_declares_name(&body.body, b"self")
            || !usage.is_receiver_only()
        {
            return Rewrite::unchanged(Stmt::FunctionDecl { name, body });
        }

        let self_name = Name::synthetic("self");
        rename_name_uses_in_block(&mut body.body, first.as_bytes(), &self_name);
        Rewrite::changed(methodize(name, body))
    }
}

fn methodize(mut name: FunctionName, mut body: FuncBody) -> Stmt {
    let method = name
        .path
        .pop()
        .expect("method sugar requires at least one field segment");
    name.method = Some(method);
    body.params.remove(0);
    Stmt::FunctionDecl { name, body }
}

fn recursive_local_function(current: &Stmt, next: Option<&Stmt>) -> Option<Stmt> {
    let Stmt::Local {
        names,
        attribs,
        values,
    } = current
    else {
        return None;
    };
    let [name] = names.as_slice() else {
        return None;
    };
    if !attribs.is_empty() || !values.is_empty() {
        return None;
    }
    let Some(Stmt::Assign { targets, values }) = next else {
        return None;
    };
    let ([Expr::Name(target)], [Expr::Function(body)]) = (targets.as_slice(), values.as_slice())
    else {
        return None;
    };
    (target == name).then(|| Stmt::Function {
        name: name.clone(),
        body: body.clone(),
        local: true,
    })
}

fn function_name_from_target(target: &Expr) -> Option<FunctionName> {
    match target {
        Expr::Global(name) => Some(FunctionName::dotted(vec![Name::new(name.clone())])),
        Expr::Field { .. } | Expr::Index { .. } => {
            let path = function_path_from_expr(target)?;
            (path.len() >= 2).then(|| FunctionName::dotted(path))
        }
        _ => None,
    }
}

fn function_path_from_expr(expr: &Expr) -> Option<Vec<Name>> {
    match expr {
        Expr::Name(name) => Some(vec![name.clone()]),
        Expr::Global(name) => Some(vec![Name::new(name.clone())]),
        Expr::Field { obj, name } => {
            let mut path = function_path_from_expr(obj)?;
            path.push(name.clone());
            Some(path)
        }
        Expr::Index { obj, key } => {
            let Expr::Str(field) = key.as_ref() else {
                return None;
            };
            if !is_valid_identifier(field) {
                return None;
            }
            let mut path = function_path_from_expr(obj)?;
            path.push(Name::new(field.clone()));
            Some(path)
        }
        _ => None,
    }
}

fn body_references_name(body: &FuncBody, name: &Name) -> bool {
    block_references_name(&body.body, name.as_bytes())
}

fn block_references_name(block: &Block, name: &[u8]) -> bool {
    block.0.iter().any(|stmt| stmt_references_name(stmt, name))
}

fn stmt_references_name(stmt: &Stmt, name: &[u8]) -> bool {
    match stmt {
        Stmt::Local { values, .. } => values.iter().any(|expr| expr_references_name(expr, name)),
        Stmt::Assign { targets, values } => {
            targets.iter().any(|expr| expr_references_name(expr, name))
                || values.iter().any(|expr| expr_references_name(expr, name))
        }
        Stmt::Call(expr) => expr_references_name(expr, name),
        Stmt::Do(body) => block_references_name(body, name),
        Stmt::While { cond, body } => {
            expr_references_name(cond, name) || block_references_name(body, name)
        }
        Stmt::Repeat { body, cond } => {
            block_references_name(body, name) || expr_references_name(cond, name)
        }
        Stmt::NumericFor {
            start,
            stop,
            step,
            body,
            ..
        } => {
            expr_references_name(start, name)
                || expr_references_name(stop, name)
                || step
                    .as_ref()
                    .is_some_and(|step| expr_references_name(step, name))
                || block_references_name(body, name)
        }
        Stmt::GenericFor { exprs, body, .. } => {
            exprs.iter().any(|expr| expr_references_name(expr, name))
                || block_references_name(body, name)
        }
        Stmt::If { arms, else_ } => {
            arms.iter().any(|(cond, body)| {
                expr_references_name(cond, name) || block_references_name(body, name)
            }) || else_
                .as_ref()
                .is_some_and(|body| block_references_name(body, name))
        }
        Stmt::Function { body, .. } | Stmt::FunctionDecl { body, .. } => {
            body.params.iter().all(|param| param.as_bytes() != name)
                && block_references_name(&body.body, name)
        }
        Stmt::Return(values) => values.iter().any(|expr| expr_references_name(expr, name)),
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => false,
    }
}

fn expr_references_name(expr: &Expr, name: &[u8]) -> bool {
    match expr {
        Expr::Name(candidate) => candidate.as_bytes() == name,
        Expr::Index { obj, key } => {
            expr_references_name(obj, name) || expr_references_name(key, name)
        }
        Expr::Field { obj, .. } => expr_references_name(obj, name),
        Expr::Call { func, args, .. } => {
            expr_references_name(func, name)
                || args.iter().any(|arg| expr_references_name(arg, name))
        }
        Expr::Function(body) => {
            body.params.iter().all(|param| param.as_bytes() != name)
                && block_references_name(&body.body, name)
        }
        Expr::Table(fields) => fields.iter().any(|field| match field {
            crate::decompile::ast::TableField::List(value) => expr_references_name(value, name),
            crate::decompile::ast::TableField::Named { value, .. } => {
                expr_references_name(value, name)
            }
            crate::decompile::ast::TableField::ExprKey { key, value } => {
                expr_references_name(key, name) || expr_references_name(value, name)
            }
        }),
        Expr::Binary { lhs, rhs, .. } => {
            expr_references_name(lhs, name) || expr_references_name(rhs, name)
        }
        Expr::Unary { operand, .. } | Expr::Paren(operand) => expr_references_name(operand, name),
        _ => false,
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct ReceiverUsage {
    receiver: bool,
    other: bool,
}

impl ReceiverUsage {
    fn is_receiver_only(self) -> bool {
        self.receiver && !self.other
    }

    fn merge(&mut self, other: ReceiverUsage) {
        self.receiver |= other.receiver;
        self.other |= other.other;
    }
}

fn receiver_usage_in_block(block: &Block, name: &[u8]) -> ReceiverUsage {
    let mut usage = ReceiverUsage::default();
    for stmt in &block.0 {
        usage.merge(receiver_usage_in_stmt(stmt, name));
    }
    usage
}

fn receiver_usage_in_stmt(stmt: &Stmt, name: &[u8]) -> ReceiverUsage {
    let mut usage = ReceiverUsage::default();
    match stmt {
        Stmt::Local { values, .. } => {
            for value in values {
                usage.merge(receiver_usage_in_expr(value, name));
            }
        }
        Stmt::Assign { targets, values } => {
            for target in targets {
                usage.merge(receiver_usage_in_expr(target, name));
            }
            for value in values {
                usage.merge(receiver_usage_in_expr(value, name));
            }
        }
        Stmt::Call(expr) => usage.merge(receiver_usage_in_expr(expr, name)),
        Stmt::Do(body) => usage.merge(receiver_usage_in_block(body, name)),
        Stmt::While { cond, body } => {
            usage.merge(receiver_usage_in_expr(cond, name));
            usage.merge(receiver_usage_in_block(body, name));
        }
        Stmt::Repeat { body, cond } => {
            usage.merge(receiver_usage_in_block(body, name));
            usage.merge(receiver_usage_in_expr(cond, name));
        }
        Stmt::NumericFor {
            start,
            stop,
            step,
            body,
            ..
        } => {
            usage.merge(receiver_usage_in_expr(start, name));
            usage.merge(receiver_usage_in_expr(stop, name));
            if let Some(step) = step {
                usage.merge(receiver_usage_in_expr(step, name));
            }
            usage.merge(receiver_usage_in_block(body, name));
        }
        Stmt::GenericFor { exprs, body, .. } => {
            for expr in exprs {
                usage.merge(receiver_usage_in_expr(expr, name));
            }
            usage.merge(receiver_usage_in_block(body, name));
        }
        Stmt::If { arms, else_ } => {
            for (cond, body) in arms {
                usage.merge(receiver_usage_in_expr(cond, name));
                usage.merge(receiver_usage_in_block(body, name));
            }
            if let Some(body) = else_ {
                usage.merge(receiver_usage_in_block(body, name));
            }
        }
        Stmt::Function { body, .. } | Stmt::FunctionDecl { body, .. } => {
            if body.params.iter().all(|param| param.as_bytes() != name) {
                usage.merge(receiver_usage_in_block(&body.body, name));
            }
        }
        Stmt::Return(values) => {
            for value in values {
                usage.merge(receiver_usage_in_expr(value, name));
            }
        }
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => {}
    }
    usage
}

fn receiver_usage_in_expr(expr: &Expr, name: &[u8]) -> ReceiverUsage {
    let mut usage = ReceiverUsage::default();
    match expr {
        Expr::Name(candidate) if candidate.as_bytes() == name => usage.other = true,
        Expr::Index { obj, key } => {
            if expr_is_name(obj, name) {
                usage.receiver = true;
            } else {
                usage.merge(receiver_usage_in_expr(obj, name));
            }
            usage.merge(receiver_usage_in_expr(key, name));
        }
        Expr::Field { obj, .. } => {
            if expr_is_name(obj, name) {
                usage.receiver = true;
            } else {
                usage.merge(receiver_usage_in_expr(obj, name));
            }
        }
        Expr::Call { func, args, .. } => {
            usage.merge(receiver_usage_in_expr(func, name));
            for arg in args {
                usage.merge(receiver_usage_in_expr(arg, name));
            }
        }
        Expr::Function(body) => {
            if body.params.iter().all(|param| param.as_bytes() != name) {
                usage.merge(receiver_usage_in_block(&body.body, name));
            }
        }
        Expr::Table(fields) => {
            for field in fields {
                match field {
                    crate::decompile::ast::TableField::List(value) => {
                        usage.merge(receiver_usage_in_expr(value, name));
                    }
                    crate::decompile::ast::TableField::Named { value, .. } => {
                        usage.merge(receiver_usage_in_expr(value, name));
                    }
                    crate::decompile::ast::TableField::ExprKey { key, value } => {
                        usage.merge(receiver_usage_in_expr(key, name));
                        usage.merge(receiver_usage_in_expr(value, name));
                    }
                }
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            usage.merge(receiver_usage_in_expr(lhs, name));
            usage.merge(receiver_usage_in_expr(rhs, name));
        }
        Expr::Unary { operand, .. } | Expr::Paren(operand) => {
            usage.merge(receiver_usage_in_expr(operand, name));
        }
        _ => {}
    }
    usage
}

fn expr_is_name(expr: &Expr, name: &[u8]) -> bool {
    matches!(expr, Expr::Name(candidate) if candidate.as_bytes() == name)
}

fn block_declares_name(block: &Block, name: &[u8]) -> bool {
    block.0.iter().any(|stmt| stmt_declares_name(stmt, name))
}

fn stmt_declares_name(stmt: &Stmt, name: &[u8]) -> bool {
    match stmt {
        Stmt::Local { names, .. } => names.iter().any(|candidate| candidate.as_bytes() == name),
        Stmt::GenericFor { names, body, .. } => {
            names.iter().any(|candidate| candidate.as_bytes() == name)
                || block_declares_name(body, name)
        }
        Stmt::NumericFor { var, body, .. } => {
            var.as_bytes() == name || block_declares_name(body, name)
        }
        Stmt::Function {
            name: var,
            local: true,
            ..
        } => var.as_bytes() == name,
        Stmt::Do(body) | Stmt::While { body, .. } | Stmt::Repeat { body, .. } => {
            block_declares_name(body, name)
        }
        Stmt::If { arms, else_ } => {
            arms.iter().any(|(_, body)| block_declares_name(body, name))
                || else_
                    .as_ref()
                    .is_some_and(|body| block_declares_name(body, name))
        }
        Stmt::Function { body, .. } | Stmt::FunctionDecl { body, .. } => {
            body.params
                .iter()
                .any(|candidate| candidate.as_bytes() == name)
                || block_declares_name(&body.body, name)
        }
        _ => false,
    }
}

fn rename_name_uses_in_block(block: &mut Block, from: &[u8], to: &Name) {
    for stmt in &mut block.0 {
        rename_name_uses_in_stmt(stmt, from, to);
    }
}

fn rename_name_uses_in_stmt(stmt: &mut Stmt, from: &[u8], to: &Name) {
    match stmt {
        Stmt::Local { values, .. } => {
            for value in values {
                rename_name_uses_in_expr(value, from, to);
            }
        }
        Stmt::Assign { targets, values } => {
            for target in targets {
                rename_name_uses_in_expr(target, from, to);
            }
            for value in values {
                rename_name_uses_in_expr(value, from, to);
            }
        }
        Stmt::Call(expr) => rename_name_uses_in_expr(expr, from, to),
        Stmt::Do(body) => rename_name_uses_in_block(body, from, to),
        Stmt::While { cond, body } => {
            rename_name_uses_in_expr(cond, from, to);
            rename_name_uses_in_block(body, from, to);
        }
        Stmt::Repeat { body, cond } => {
            rename_name_uses_in_block(body, from, to);
            rename_name_uses_in_expr(cond, from, to);
        }
        Stmt::NumericFor {
            start,
            stop,
            step,
            body,
            ..
        } => {
            rename_name_uses_in_expr(start, from, to);
            rename_name_uses_in_expr(stop, from, to);
            if let Some(step) = step {
                rename_name_uses_in_expr(step, from, to);
            }
            rename_name_uses_in_block(body, from, to);
        }
        Stmt::GenericFor { exprs, body, .. } => {
            for expr in exprs {
                rename_name_uses_in_expr(expr, from, to);
            }
            rename_name_uses_in_block(body, from, to);
        }
        Stmt::If { arms, else_ } => {
            for (cond, body) in arms {
                rename_name_uses_in_expr(cond, from, to);
                rename_name_uses_in_block(body, from, to);
            }
            if let Some(body) = else_ {
                rename_name_uses_in_block(body, from, to);
            }
        }
        Stmt::Function { body, .. } | Stmt::FunctionDecl { body, .. } => {
            if body.params.iter().all(|param| param.as_bytes() != from) {
                rename_name_uses_in_block(&mut body.body, from, to);
            }
        }
        Stmt::Return(values) => {
            for value in values {
                rename_name_uses_in_expr(value, from, to);
            }
        }
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => {}
    }
}

fn rename_name_uses_in_expr(expr: &mut Expr, from: &[u8], to: &Name) {
    match expr {
        Expr::Name(name) if name.as_bytes() == from => *name = to.clone(),
        Expr::Index { obj, key } => {
            rename_name_uses_in_expr(obj, from, to);
            rename_name_uses_in_expr(key, from, to);
        }
        Expr::Field { obj, .. } => rename_name_uses_in_expr(obj, from, to),
        Expr::Call { func, args, .. } => {
            rename_name_uses_in_expr(func, from, to);
            for arg in args {
                rename_name_uses_in_expr(arg, from, to);
            }
        }
        Expr::Function(body) => {
            if body.params.iter().all(|param| param.as_bytes() != from) {
                rename_name_uses_in_block(&mut body.body, from, to);
            }
        }
        Expr::Table(fields) => {
            for field in fields {
                match field {
                    crate::decompile::ast::TableField::List(value) => {
                        rename_name_uses_in_expr(value, from, to);
                    }
                    crate::decompile::ast::TableField::Named { value, .. } => {
                        rename_name_uses_in_expr(value, from, to);
                    }
                    crate::decompile::ast::TableField::ExprKey { key, value } => {
                        rename_name_uses_in_expr(key, from, to);
                        rename_name_uses_in_expr(value, from, to);
                    }
                }
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            rename_name_uses_in_expr(lhs, from, to);
            rename_name_uses_in_expr(rhs, from, to);
        }
        Expr::Unary { operand, .. } | Expr::Paren(operand) => {
            rename_name_uses_in_expr(operand, from, to);
        }
        _ => {}
    }
}
