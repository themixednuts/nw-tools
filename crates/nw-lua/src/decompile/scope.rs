//! Declarations for locals that bytecode reads outside the scope they landed in.
//!
//! A stripped chunk has no scope information. One register that several writes
//! share becomes one binding, and lowering declares it at its first write, which
//! can sit inside a branch that later code reads past. Such a binding moves up to
//! the innermost block that holds all of its declarations and reads.

use std::collections::{HashMap, HashSet};

use super::ast::{
    BindingId, Block, Expr, FuncBody, Name, Stmt, TableField, binding_usages_in_block,
};

/// Declare each local read outside its scope in the block enclosing all its uses.
///
/// A binding with no declaration at all stays as it is, because a read of it has
/// lost its write and only the emit validator can report that. So does one that a
/// closure reads out of scope when its declaration sits in a loop: moving that
/// declaration out would merge the fresh local of each iteration into one.
pub(crate) fn hoist_out_of_scope_locals(block: &mut Block) {
    let mut scan = Scan::default();
    scan.scopes.push(HashSet::new());
    scan.block(block);

    let mut hoisted = scan
        .unresolved
        .keys()
        .filter(|binding| scan.can_hoist(binding))
        .cloned()
        .collect::<HashSet<_>>();
    let mut pending = hoisted.iter().cloned().collect::<Vec<_>>();
    while let Some(binding) = pending.pop() {
        for decl in scan.decls.get(&binding).into_iter().flatten() {
            for sibling in &decl.siblings {
                if hoisted.insert(sibling.clone()) {
                    pending.push(sibling.clone());
                }
            }
        }
    }
    if hoisted.is_empty() {
        return;
    }

    let mut inserts = hoisted
        .iter()
        .filter_map(|binding| scan.insert_for(binding))
        .collect::<Vec<_>>();
    inserts.sort_by(|left, right| {
        right
            .path
            .len()
            .cmp(&left.path.len())
            .then(right.stmt.cmp(&left.stmt))
            .then(right.name.as_bytes().cmp(left.name.as_bytes()))
    });

    convert_declarations(block, &hoisted);
    for insert in inserts {
        if let Some(target) = block_at_mut(block, &insert.path) {
            let at = insert.stmt.min(target.0.len());
            target.0.insert(
                at,
                Stmt::Local {
                    names: vec![insert.name],
                    attribs: Vec::new(),
                    values: Vec::new(),
                },
            );
        }
    }
}

/// One position in the function body: the child blocks entered, then a statement.
#[derive(Debug, Clone)]
struct Site {
    path: Vec<(usize, usize)>,
    stmt: usize,
}

#[derive(Debug)]
struct Declaration {
    site: Site,
    name: Name,
    in_loop: bool,
    siblings: Vec<BindingId>,
}

#[derive(Debug)]
struct Insert {
    path: Vec<(usize, usize)>,
    stmt: usize,
    name: Name,
}

#[derive(Debug, Default)]
struct Scan {
    scopes: Vec<HashSet<BindingId>>,
    path: Vec<(usize, usize)>,
    stmt: usize,
    loops: usize,
    decls: HashMap<BindingId, Vec<Declaration>>,
    unresolved: HashMap<BindingId, Vec<Site>>,
    closure_reads: HashSet<BindingId>,
}

impl Scan {
    fn can_hoist(&self, binding: &BindingId) -> bool {
        self.decls.get(binding).is_some_and(|decls| {
            !self.closure_reads.contains(binding) || !decls.iter().any(|decl| decl.in_loop)
        })
    }

    fn insert_for(&self, binding: &BindingId) -> Option<Insert> {
        let decls = self.decls.get(binding)?;
        let name = decls.first()?.name.clone();
        let sites = decls
            .iter()
            .map(|decl| &decl.site)
            .chain(self.unresolved.get(binding).into_iter().flatten())
            .collect::<Vec<_>>();
        let first = sites.first()?;
        let shared = sites.iter().fold(first.path.len(), |shared, site| {
            first
                .path
                .iter()
                .zip(&site.path)
                .take(shared)
                .take_while(|(left, right)| left == right)
                .count()
        });
        let stmt = sites
            .iter()
            .map(|site| site.path.get(shared).map_or(site.stmt, |entered| entered.0))
            .min()?;
        Some(Insert {
            path: first.path[..shared].to_vec(),
            stmt,
            name,
        })
    }

    fn site(&self) -> Site {
        Site {
            path: self.path.clone(),
            stmt: self.stmt,
        }
    }

    fn in_scope(&self, binding: &BindingId) -> bool {
        binding.is_external_upvalue() || self.scopes.iter().any(|scope| scope.contains(binding))
    }

    fn block(&mut self, block: &Block) {
        for (index, stmt) in block.0.iter().enumerate() {
            self.stmt = index;
            self.statement(stmt);
        }
    }

    fn child(&mut self, slot: usize, body: &Block, looping: bool, vars: &[Name]) {
        let stmt = self.stmt;
        self.enter(slot, looping, vars);
        self.block(body);
        self.leave(stmt, looping);
    }

    fn enter(&mut self, slot: usize, looping: bool, vars: &[Name]) {
        self.path.push((self.stmt, slot));
        self.scopes.push(HashSet::new());
        self.loops += usize::from(looping);
        for var in vars {
            self.mark_declared(var);
        }
    }

    fn leave(&mut self, stmt: usize, looping: bool) {
        self.loops -= usize::from(looping);
        self.scopes.pop();
        self.path.pop();
        self.stmt = stmt;
    }

    fn statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Local { names, values, .. } => {
                self.exprs(values);
                self.declare(names);
            }
            Stmt::Assign { targets, values } => {
                self.exprs(targets);
                self.exprs(values);
            }
            Stmt::Call(expr) => self.expr(expr),
            Stmt::Do(body) => self.child(0, body, false, &[]),
            Stmt::While { cond, body } => {
                self.expr(cond);
                self.child(0, body, true, &[]);
            }
            Stmt::Repeat { body, cond } => {
                let stmt = self.stmt;
                self.enter(0, true, &[]);
                self.block(body);
                self.stmt = body.0.len();
                self.expr(cond);
                self.leave(stmt, true);
            }
            Stmt::If { arms, else_ } => {
                for (index, (cond, body)) in arms.iter().enumerate() {
                    self.expr(cond);
                    self.child(index, body, false, &[]);
                }
                if let Some(body) = else_ {
                    self.child(arms.len(), body, false, &[]);
                }
            }
            Stmt::NumericFor {
                var,
                start,
                stop,
                step,
                body,
            } => {
                self.expr(start);
                self.expr(stop);
                if let Some(step) = step {
                    self.expr(step);
                }
                self.child(0, body, true, std::slice::from_ref(var));
            }
            Stmt::GenericFor { names, exprs, body } => {
                self.exprs(exprs);
                self.child(0, body, true, names);
            }
            Stmt::Function { name, body, local } => {
                if *local {
                    self.declare(std::slice::from_ref(name));
                } else {
                    self.read(name);
                }
                self.closure(body);
            }
            Stmt::FunctionDecl { name, body } => {
                if let Some(root) = name.path.first() {
                    self.read(root);
                }
                self.closure(body);
            }
            Stmt::Return(values) => self.exprs(values),
            Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => {}
        }
    }

    fn exprs(&mut self, exprs: &[Expr]) {
        for expr in exprs {
            self.expr(expr);
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Name(name) => self.read(name),
            Expr::Index { obj, key } => {
                self.expr(obj);
                self.expr(key);
            }
            Expr::Field { obj, .. } => self.expr(obj),
            Expr::Call { func, args, .. } => {
                self.expr(func);
                self.exprs(args);
            }
            Expr::Function(body) => self.closure(body),
            Expr::Table(fields) => {
                for field in fields {
                    match field {
                        TableField::List(value) | TableField::Named { value, .. } => {
                            self.expr(value);
                        }
                        TableField::ExprKey { key, value } => {
                            self.expr(key);
                            self.expr(value);
                        }
                    }
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Expr::Unary { operand, .. } | Expr::Paren(operand) => self.expr(operand),
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

    /// Count a closure's reads as reads at the statement that creates it. Its own
    /// body was already repaired when that prototype was lowered.
    fn closure(&mut self, body: &FuncBody) {
        for binding in binding_usages_in_block(&body.body).into_keys() {
            if !self.in_scope(&binding) {
                let site = self.site();
                self.unresolved
                    .entry(binding.clone())
                    .or_default()
                    .push(site);
                self.closure_reads.insert(binding);
            }
        }
    }

    fn read(&mut self, name: &Name) {
        if let Some(binding) = name.binding()
            && !self.in_scope(binding)
        {
            let site = self.site();
            self.unresolved
                .entry(binding.clone())
                .or_default()
                .push(site);
        }
    }

    fn declare(&mut self, names: &[Name]) {
        let siblings = names
            .iter()
            .filter_map(Name::binding)
            .cloned()
            .collect::<Vec<_>>();
        for name in names {
            if let Some(binding) = name.binding() {
                let declaration = Declaration {
                    site: self.site(),
                    name: name.clone(),
                    in_loop: self.loops > 0,
                    siblings: siblings.clone(),
                };
                self.decls
                    .entry(binding.clone())
                    .or_default()
                    .push(declaration);
            }
            self.mark_declared(name);
        }
    }

    fn mark_declared(&mut self, name: &Name) {
        if let Some(binding) = name.binding()
            && let Some(scope) = self.scopes.last_mut()
        {
            scope.insert(binding.clone());
        }
    }
}

fn is_hoisted(name: &Name, hoisted: &HashSet<BindingId>) -> bool {
    name.binding()
        .is_some_and(|binding| hoisted.contains(binding))
}

/// Turn each declaration of a hoisted binding into the assignment it performs.
fn convert_declarations(block: &mut Block, hoisted: &HashSet<BindingId>) {
    for stmt in &mut block.0 {
        match stmt {
            Stmt::Local { names, values, .. }
                if names.iter().any(|name| is_hoisted(name, hoisted)) =>
            {
                let targets = names.drain(..).map(Expr::Name).collect();
                let mut values = std::mem::take(values);
                if values.is_empty() {
                    values.push(Expr::Nil);
                }
                *stmt = Stmt::Assign { targets, values };
            }
            Stmt::Function { name, local, .. } if *local && is_hoisted(name, hoisted) => {
                *local = false;
            }
            Stmt::Do(body)
            | Stmt::While { body, .. }
            | Stmt::Repeat { body, .. }
            | Stmt::NumericFor { body, .. }
            | Stmt::GenericFor { body, .. } => convert_declarations(body, hoisted),
            Stmt::If { arms, else_ } => {
                for (_, body) in arms {
                    convert_declarations(body, hoisted);
                }
                if let Some(body) = else_ {
                    convert_declarations(body, hoisted);
                }
            }
            _ => {}
        }
    }
}

fn block_at_mut<'a>(root: &'a mut Block, path: &[(usize, usize)]) -> Option<&'a mut Block> {
    let mut current = root;
    for (stmt, slot) in path {
        current = match current.0.get_mut(*stmt)? {
            Stmt::Do(body)
            | Stmt::While { body, .. }
            | Stmt::Repeat { body, .. }
            | Stmt::NumericFor { body, .. }
            | Stmt::GenericFor { body, .. } => body,
            Stmt::If { arms, else_ } => {
                if *slot < arms.len() {
                    &mut arms.get_mut(*slot)?.1
                } else {
                    else_.as_mut()?
                }
            }
            _ => return None,
        };
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile::ast::{BindingId, FunctionId};

    fn local(index: usize) -> Name {
        Name::synthetic(format!("l{index}"))
            .with_binding(BindingId::synthetic(&FunctionId::default(), index))
    }

    fn read(index: usize) -> Expr {
        Expr::Name(local(index))
    }

    fn if_arm(body: Vec<Stmt>) -> Stmt {
        Stmt::If {
            arms: vec![(Expr::True, Block::new(body))],
            else_: None,
        }
    }

    fn declare(index: usize, value: Expr) -> Stmt {
        Stmt::Local {
            names: vec![local(index)],
            attribs: Vec::new(),
            values: vec![value],
        }
    }

    #[test]
    fn moves_a_declaration_to_the_block_holding_its_reads() {
        let mut block = Block::new(vec![
            if_arm(vec![declare(0, Expr::Integer(1))]),
            Stmt::Return(vec![read(0)]),
        ]);

        hoist_out_of_scope_locals(&mut block);

        assert_eq!(
            block,
            Block::new(vec![
                Stmt::Local {
                    names: vec![local(0)],
                    attribs: Vec::new(),
                    values: Vec::new(),
                },
                if_arm(vec![Stmt::Assign {
                    targets: vec![read(0)],
                    values: vec![Expr::Integer(1)],
                }]),
                Stmt::Return(vec![read(0)]),
            ])
        );
    }

    #[test]
    fn leaves_a_read_with_no_declaration_for_the_validator() {
        let original = Block::new(vec![Stmt::Return(vec![read(0)])]);
        let mut block = original.clone();

        hoist_out_of_scope_locals(&mut block);

        assert_eq!(block, original);
    }

    #[test]
    fn leaves_a_local_in_scope_alone() {
        let original = Block::new(vec![if_arm(vec![
            declare(0, Expr::Integer(1)),
            Stmt::Return(vec![read(0)]),
        ])]);
        let mut block = original.clone();

        hoist_out_of_scope_locals(&mut block);

        assert_eq!(block, original);
    }
}
