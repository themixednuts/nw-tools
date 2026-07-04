use crate::decompile::ast::{BinOp, Block, Expr, Stmt, UnOp};

use super::engine::{CleanContext, Rewrite, Rule};

pub struct ElseIfChain;

impl Rule for ElseIfChain {
    fn rewrite_stmt(&self, stmt: Stmt, _ctx: &CleanContext) -> Rewrite<Stmt> {
        let Stmt::If {
            mut arms,
            else_: Some(else_block),
        } = stmt
        else {
            return Rewrite::unchanged(stmt);
        };
        let [
            Stmt::If {
                arms: nested_arms,
                else_: nested_else,
            },
        ] = else_block.0.as_slice()
        else {
            return Rewrite::unchanged(Stmt::If {
                arms,
                else_: Some(else_block),
            });
        };
        arms.extend(nested_arms.iter().cloned());
        Rewrite::changed(Stmt::If {
            arms,
            else_: nested_else.clone(),
        })
    }
}

pub struct DropElseAfterExit;

impl Rule for DropElseAfterExit {
    fn rewrite_block(&self, block: Block, _ctx: &CleanContext) -> Rewrite<Block> {
        let mut out = Vec::with_capacity(block.0.len());
        let mut changed = false;
        for stmt in block.0 {
            let Stmt::If { arms, else_ } = stmt else {
                out.push(stmt);
                continue;
            };
            let Some(((cond, body), rest)) = arms.split_first() else {
                out.push(Stmt::If { arms, else_ });
                continue;
            };
            if !block_always_exits(body) || (rest.is_empty() && else_.is_none()) {
                out.push(Stmt::If { arms, else_ });
                continue;
            }

            out.push(Stmt::If {
                arms: vec![(cond.clone(), body.clone())],
                else_: None,
            });
            if rest.is_empty() {
                if let Some(else_block) = else_ {
                    out.extend(else_block.0);
                }
            } else {
                out.push(Stmt::If {
                    arms: rest.to_vec(),
                    else_,
                });
            }
            changed = true;
        }
        if changed {
            Rewrite::changed(Block::new(out))
        } else {
            Rewrite::unchanged(Block::new(out))
        }
    }
}

pub struct EarlyReturnGuard;

impl Rule for EarlyReturnGuard {
    fn rewrite_block(&self, block: Block, ctx: &CleanContext) -> Rewrite<Block> {
        if !ctx.allows_guard_return() {
            return Rewrite::unchanged(block);
        }
        let [Stmt::If { arms, else_: None }] = block.0.as_slice() else {
            return Rewrite::unchanged(block);
        };
        let [(cond, body)] = arms.as_slice() else {
            return Rewrite::unchanged(block);
        };
        if body.0.is_empty() {
            return Rewrite::unchanged(block);
        }

        let mut stmts = Vec::with_capacity(body.0.len() + 1);
        stmts.push(Stmt::If {
            arms: vec![(
                invert_condition(cond.clone()),
                Block::new(vec![Stmt::Return(Vec::new())]),
            )],
            else_: None,
        });
        stmts.extend(body.0.clone());
        Rewrite::changed(Block::new(stmts))
    }
}

pub struct EmptyBranchCleanup;

impl Rule for EmptyBranchCleanup {
    fn rewrite_block(&self, block: Block, _ctx: &CleanContext) -> Rewrite<Block> {
        let mut out = Vec::with_capacity(block.0.len());
        let mut changed = false;
        for stmt in block.0 {
            if let Stmt::If { arms, else_: None } = &stmt
                && arms.len() == 1
                && arms[0].1.0.is_empty()
                && expr_is_side_effect_free(&arms[0].0)
            {
                changed = true;
                continue;
            }
            out.push(stmt);
        }
        if changed {
            Rewrite::changed(Block::new(out))
        } else {
            Rewrite::unchanged(Block::new(out))
        }
    }

    fn rewrite_stmt(&self, stmt: Stmt, _ctx: &CleanContext) -> Rewrite<Stmt> {
        let Stmt::If {
            arms,
            else_: Some(else_block),
        } = stmt
        else {
            return Rewrite::unchanged(stmt);
        };
        let [(cond, then_block)] = arms.as_slice() else {
            return Rewrite::unchanged(Stmt::If {
                arms,
                else_: Some(else_block),
            });
        };
        if else_block.0.is_empty() {
            return Rewrite::changed(Stmt::If { arms, else_: None });
        }
        if then_block.0.is_empty() {
            return Rewrite::changed(Stmt::If {
                arms: vec![(invert_condition(cond.clone()), else_block)],
                else_: None,
            });
        }
        Rewrite::unchanged(Stmt::If {
            arms,
            else_: Some(else_block),
        })
    }
}

pub struct RedundantDo;

impl Rule for RedundantDo {
    fn rewrite_block(&self, block: Block, _ctx: &CleanContext) -> Rewrite<Block> {
        let mut out = Vec::with_capacity(block.0.len());
        let mut changed = false;
        let last = block.0.len().saturating_sub(1);
        for (index, stmt) in block.0.into_iter().enumerate() {
            let Stmt::Do(body) = stmt else {
                out.push(stmt);
                continue;
            };
            if block_has_local_decls(&body) || (block_always_exits(&body) && index != last) {
                out.push(Stmt::Do(body));
                continue;
            }
            out.extend(body.0);
            changed = true;
        }
        if changed {
            Rewrite::changed(Block::new(out))
        } else {
            Rewrite::unchanged(Block::new(out))
        }
    }
}

fn block_always_exits(block: &Block) -> bool {
    let Some(last) = block.0.last() else {
        return false;
    };
    match last {
        Stmt::Return(_) | Stmt::Break => true,
        Stmt::Do(body) => block_always_exits(body),
        Stmt::If { arms, else_ } => {
            !arms.is_empty()
                && arms.iter().all(|(_, body)| block_always_exits(body))
                && else_.as_ref().is_some_and(block_always_exits)
        }
        _ => false,
    }
}

fn invert_condition(expr: Expr) -> Expr {
    match expr {
        Expr::True => Expr::False,
        Expr::False => Expr::True,
        Expr::Unary {
            op: UnOp::Not,
            operand,
        } => *operand,
        Expr::Binary {
            op: BinOp::And,
            lhs,
            rhs,
        } => Expr::Binary {
            op: BinOp::Or,
            lhs: Box::new(invert_condition(*lhs)),
            rhs: Box::new(invert_condition(*rhs)),
        },
        Expr::Binary {
            op: BinOp::Or,
            lhs,
            rhs,
        } => Expr::Binary {
            op: BinOp::And,
            lhs: Box::new(invert_condition(*lhs)),
            rhs: Box::new(invert_condition(*rhs)),
        },
        Expr::Binary { op, lhs, rhs } => {
            if let Some(op) = invert_binop(op) {
                Expr::Binary { op, lhs, rhs }
            } else {
                Expr::Unary {
                    op: UnOp::Not,
                    operand: Box::new(Expr::Binary { op, lhs, rhs }),
                }
            }
        }
        expr => Expr::Unary {
            op: UnOp::Not,
            operand: Box::new(expr),
        },
    }
}

fn invert_binop(op: BinOp) -> Option<BinOp> {
    Some(match op {
        BinOp::Eq => BinOp::Ne,
        BinOp::Ne => BinOp::Eq,
        BinOp::Lt => BinOp::Ge,
        BinOp::Le => BinOp::Gt,
        BinOp::Gt => BinOp::Le,
        BinOp::Ge => BinOp::Lt,
        _ => return None,
    })
}

fn expr_is_side_effect_free(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Nil
            | Expr::True
            | Expr::False
            | Expr::VarArg
            | Expr::Number(_)
            | Expr::Integer(_)
            | Expr::Str(_)
            | Expr::Name(_)
    )
}

fn block_has_local_decls(block: &Block) -> bool {
    block.0.iter().any(stmt_has_local_decls)
}

fn stmt_has_local_decls(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Local { .. } | Stmt::Function { local: true, .. } => true,
        Stmt::Do(body)
        | Stmt::While { body, .. }
        | Stmt::Repeat { body, .. }
        | Stmt::NumericFor { body, .. }
        | Stmt::GenericFor { body, .. } => block_has_local_decls(body),
        Stmt::If { arms, else_ } => {
            arms.iter().any(|(_, body)| block_has_local_decls(body))
                || else_.as_ref().is_some_and(block_has_local_decls)
        }
        Stmt::Function { .. } | Stmt::FunctionDecl { .. } => false,
        _ => false,
    }
}
