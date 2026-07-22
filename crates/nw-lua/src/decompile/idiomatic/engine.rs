use crate::decompile::ast::{Block, Expr, FuncBody, Stmt, TableField};

const MAX_FIXPOINT_ITERS: usize = 32;

/// Context available to idiomatic rewrite rules.
#[derive(Debug, Clone)]
pub struct CleanContext {
    pub module_stem: Option<String>,
    pub function_depth: usize,
    block_role: BlockRole,
}

impl CleanContext {
    pub fn new(module_stem: Option<String>) -> Self {
        Self {
            module_stem,
            function_depth: 0,
            block_role: BlockRole::FunctionBody,
        }
    }

    pub fn allows_guard_return(&self) -> bool {
        self.block_role == BlockRole::FunctionBody
    }

    pub fn in_root_function(&self) -> bool {
        self.function_depth == 0 && self.block_role == BlockRole::FunctionBody
    }

    fn control_block(&self) -> Self {
        Self {
            block_role: BlockRole::ControlBody,
            ..self.clone()
        }
    }

    fn function_body(&self) -> Self {
        Self {
            function_depth: self.function_depth + 1,
            block_role: BlockRole::FunctionBody,
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockRole {
    FunctionBody,
    ControlBody,
}

/// A rewrite result for one AST node.
#[derive(Debug, Clone)]
pub struct Rewrite<T> {
    pub value: T,
    pub changed: bool,
}

impl<T> Rewrite<T> {
    pub fn unchanged(value: T) -> Self {
        Self {
            value,
            changed: false,
        }
    }

    pub fn changed(value: T) -> Self {
        Self {
            value,
            changed: true,
        }
    }
}

/// One small local AST rewrite rule.
pub trait Rule {
    fn rewrite_block(&self, block: Block, _ctx: &CleanContext) -> Rewrite<Block> {
        Rewrite::unchanged(block)
    }

    fn rewrite_stmt(&self, stmt: Stmt, _ctx: &CleanContext) -> Rewrite<Stmt> {
        Rewrite::unchanged(stmt)
    }

    fn rewrite_expr(&self, expr: Expr, _ctx: &CleanContext) -> Rewrite<Expr> {
        Rewrite::unchanged(expr)
    }
}

/// Generic bottom-up fixpoint rewriter.
pub struct Engine<'a> {
    rules: &'a [&'a dyn Rule],
}

impl<'a> Engine<'a> {
    pub fn new(rules: &'a [&'a dyn Rule]) -> Self {
        Self { rules }
    }

    pub fn run(&self, mut block: Block, ctx: CleanContext) -> Block {
        for _ in 0..MAX_FIXPOINT_ITERS {
            let rewrite = self.fold_block(block, &ctx);
            block = rewrite.value;
            if !rewrite.changed {
                break;
            }
        }
        block
    }

    fn fold_block(&self, block: Block, ctx: &CleanContext) -> Rewrite<Block> {
        let mut changed = false;
        let stmts = block
            .0
            .into_iter()
            .map(|stmt| {
                let rewrite = self.fold_stmt(stmt, ctx);
                changed |= rewrite.changed;
                rewrite.value
            })
            .collect();
        let rewrite = self.apply_block_rules(Block::new(stmts), ctx);
        Rewrite {
            changed: changed || rewrite.changed,
            value: rewrite.value,
        }
    }

    fn fold_stmt(&self, stmt: Stmt, ctx: &CleanContext) -> Rewrite<Stmt> {
        let mut changed = false;
        let stmt = match stmt {
            Stmt::Local {
                names,
                attribs,
                values,
            } => Stmt::Local {
                names,
                attribs,
                values: self.fold_exprs(values, ctx, &mut changed),
            },
            Stmt::Assign { targets, values } => Stmt::Assign {
                targets: self.fold_exprs(targets, ctx, &mut changed),
                values: self.fold_exprs(values, ctx, &mut changed),
            },
            Stmt::Call(expr) => Stmt::Call(self.fold_expr(expr, ctx, &mut changed)),
            Stmt::Do(body) => {
                Stmt::Do(self.fold_child_block(body, &ctx.control_block(), &mut changed))
            }
            Stmt::While { cond, body } => Stmt::While {
                cond: self.fold_expr(cond, ctx, &mut changed),
                body: self.fold_child_block(body, &ctx.control_block(), &mut changed),
            },
            Stmt::Repeat { body, cond } => Stmt::Repeat {
                body: self.fold_child_block(body, &ctx.control_block(), &mut changed),
                cond: self.fold_expr(cond, ctx, &mut changed),
            },
            Stmt::If { arms, else_ } => {
                let child_ctx = ctx.control_block();
                Stmt::If {
                    arms: arms
                        .into_iter()
                        .map(|(cond, body)| {
                            (
                                self.fold_expr(cond, ctx, &mut changed),
                                self.fold_child_block(body, &child_ctx, &mut changed),
                            )
                        })
                        .collect(),
                    else_: else_.map(|body| self.fold_child_block(body, &child_ctx, &mut changed)),
                }
            }
            Stmt::NumericFor {
                var,
                start,
                stop,
                step,
                body,
            } => Stmt::NumericFor {
                var,
                start: self.fold_expr(start, ctx, &mut changed),
                stop: self.fold_expr(stop, ctx, &mut changed),
                step: step.map(|step| Box::new(self.fold_expr(*step, ctx, &mut changed))),
                body: self.fold_child_block(body, &ctx.control_block(), &mut changed),
            },
            Stmt::GenericFor { names, exprs, body } => Stmt::GenericFor {
                names,
                exprs: self.fold_exprs(exprs, ctx, &mut changed),
                body: self.fold_child_block(body, &ctx.control_block(), &mut changed),
            },
            Stmt::Function { name, body, local } => Stmt::Function {
                name,
                body: self.fold_func_body(body, ctx, &mut changed),
                local,
            },
            Stmt::FunctionDecl { name, body } => Stmt::FunctionDecl {
                name,
                body: self.fold_func_body(body, ctx, &mut changed),
            },
            Stmt::Return(values) => Stmt::Return(self.fold_exprs(values, ctx, &mut changed)),
            Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => stmt,
        };
        let rewrite = self.apply_stmt_rules(stmt, ctx);
        Rewrite {
            changed: changed || rewrite.changed,
            value: rewrite.value,
        }
    }

    fn fold_func_body(
        &self,
        mut body: FuncBody,
        ctx: &CleanContext,
        changed: &mut bool,
    ) -> FuncBody {
        let rewrite = self.fold_block(body.body, &ctx.function_body());
        *changed |= rewrite.changed;
        body.body = rewrite.value;
        body
    }

    fn fold_exprs(&self, exprs: Vec<Expr>, ctx: &CleanContext, changed: &mut bool) -> Vec<Expr> {
        exprs
            .into_iter()
            .map(|expr| self.fold_expr(expr, ctx, changed))
            .collect()
    }

    fn fold_expr(&self, expr: Expr, ctx: &CleanContext, changed: &mut bool) -> Expr {
        let expr = match expr {
            Expr::Index { obj, key } => Expr::Index {
                obj: Box::new(self.fold_expr(*obj, ctx, changed)),
                key: Box::new(self.fold_expr(*key, ctx, changed)),
            },
            Expr::Field { obj, name } => Expr::Field {
                obj: Box::new(self.fold_expr(*obj, ctx, changed)),
                name,
            },
            Expr::Call { func, args, method } => Expr::Call {
                func: Box::new(self.fold_expr(*func, ctx, changed)),
                args: self.fold_exprs(args, ctx, changed),
                method,
            },
            Expr::Function(body) => Expr::Function(self.fold_func_body(body, ctx, changed)),
            Expr::Table(fields) => Expr::Table(
                fields
                    .into_iter()
                    .map(|field| self.fold_table_field(field, ctx, changed))
                    .collect(),
            ),
            Expr::Binary { op, lhs, rhs } => Expr::Binary {
                op,
                lhs: Box::new(self.fold_expr(*lhs, ctx, changed)),
                rhs: Box::new(self.fold_expr(*rhs, ctx, changed)),
            },
            Expr::Unary { op, operand } => Expr::Unary {
                op,
                operand: Box::new(self.fold_expr(*operand, ctx, changed)),
            },
            Expr::Paren(inner) => Expr::Paren(Box::new(self.fold_expr(*inner, ctx, changed))),
            expr => expr,
        };
        let rewrite = self.apply_expr_rules(expr, ctx);
        *changed |= rewrite.changed;
        rewrite.value
    }

    fn fold_table_field(
        &self,
        field: TableField,
        ctx: &CleanContext,
        changed: &mut bool,
    ) -> TableField {
        match field {
            TableField::List(value) => TableField::List(self.fold_expr(value, ctx, changed)),
            TableField::Named { name, value } => TableField::Named {
                name,
                value: self.fold_expr(value, ctx, changed),
            },
            TableField::ExprKey { key, value } => TableField::ExprKey {
                key: self.fold_expr(key, ctx, changed),
                value: self.fold_expr(value, ctx, changed),
            },
        }
    }

    fn fold_child_block(&self, block: Block, ctx: &CleanContext, changed: &mut bool) -> Block {
        let rewrite = self.fold_block(block, ctx);
        *changed |= rewrite.changed;
        rewrite.value
    }

    fn apply_block_rules(&self, mut block: Block, ctx: &CleanContext) -> Rewrite<Block> {
        let mut any_changed = false;
        loop {
            let mut changed = false;
            for rule in self.rules {
                let rewrite = rule.rewrite_block(block, ctx);
                block = rewrite.value;
                if rewrite.changed {
                    changed = true;
                    any_changed = true;
                    break;
                }
            }
            if !changed {
                return Rewrite {
                    value: block,
                    changed: any_changed,
                };
            }
        }
    }

    fn apply_stmt_rules(&self, mut stmt: Stmt, ctx: &CleanContext) -> Rewrite<Stmt> {
        let mut any_changed = false;
        loop {
            let mut changed = false;
            for rule in self.rules {
                let rewrite = rule.rewrite_stmt(stmt, ctx);
                stmt = rewrite.value;
                if rewrite.changed {
                    changed = true;
                    any_changed = true;
                    break;
                }
            }
            if !changed {
                return Rewrite {
                    value: stmt,
                    changed: any_changed,
                };
            }
        }
    }

    fn apply_expr_rules(&self, mut expr: Expr, ctx: &CleanContext) -> Rewrite<Expr> {
        let mut any_changed = false;
        loop {
            let mut changed = false;
            for rule in self.rules {
                let rewrite = rule.rewrite_expr(expr, ctx);
                expr = rewrite.value;
                if rewrite.changed {
                    changed = true;
                    any_changed = true;
                    break;
                }
            }
            if !changed {
                return Rewrite {
                    value: expr,
                    changed: any_changed,
                };
            }
        }
    }
}
