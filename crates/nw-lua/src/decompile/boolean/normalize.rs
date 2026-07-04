//! Boolean expression normalization.

use crate::decompile::ast::{self, Expr};

/// Normalize a boolean expression for readable Lua output.
#[must_use]
pub fn normalize(expr: Expr) -> Expr {
    match expr {
        Expr::Binary { op, lhs, rhs } => {
            let lhs = normalize(*lhs);
            let rhs = normalize(*rhs);
            normalize_binary(op, lhs, rhs)
        }
        Expr::Unary {
            op: ast::UnOp::Not,
            operand,
        } => invert(normalize(*operand)),
        Expr::Unary { op, operand } => Expr::Unary {
            op,
            operand: Box::new(normalize(*operand)),
        },
        Expr::Paren(inner) => Expr::Paren(Box::new(normalize(*inner))),
        other => other,
    }
}

/// Invert an expression, applying comparison flips and De Morgan where useful.
#[must_use]
pub fn invert(expr: Expr) -> Expr {
    match normalize(expr) {
        Expr::Binary { op, lhs, rhs } => match op {
            ast::BinOp::Eq => normalize_binary(ast::BinOp::Ne, *lhs, *rhs),
            ast::BinOp::Ne => normalize_binary(ast::BinOp::Eq, *lhs, *rhs),
            ast::BinOp::Lt => normalize_binary(ast::BinOp::Ge, *lhs, *rhs),
            ast::BinOp::Le => normalize_binary(ast::BinOp::Gt, *lhs, *rhs),
            ast::BinOp::Gt => normalize_binary(ast::BinOp::Le, *lhs, *rhs),
            ast::BinOp::Ge => normalize_binary(ast::BinOp::Lt, *lhs, *rhs),
            ast::BinOp::And => normalize_binary(ast::BinOp::Or, invert(*lhs), invert(*rhs)),
            ast::BinOp::Or => normalize_binary(ast::BinOp::And, invert(*lhs), invert(*rhs)),
            other => Expr::Unary {
                op: ast::UnOp::Not,
                operand: Box::new(Expr::Binary {
                    op: other,
                    lhs,
                    rhs,
                }),
            },
        },
        Expr::Unary {
            op: ast::UnOp::Not,
            operand,
        } => *operand,
        other => Expr::Unary {
            op: ast::UnOp::Not,
            operand: Box::new(other),
        },
    }
}

fn normalize_binary(op: ast::BinOp, lhs: Expr, rhs: Expr) -> Expr {
    if is_literal(&lhs)
        && !is_literal(&rhs)
        && let Some(flipped) = flipped_comparison(op)
    {
        return Expr::Binary {
            op: flipped,
            lhs: Box::new(rhs),
            rhs: Box::new(lhs),
        };
    }

    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn flipped_comparison(op: ast::BinOp) -> Option<ast::BinOp> {
    Some(match op {
        ast::BinOp::Lt => ast::BinOp::Gt,
        ast::BinOp::Le => ast::BinOp::Ge,
        ast::BinOp::Gt => ast::BinOp::Lt,
        ast::BinOp::Ge => ast::BinOp::Le,
        _ => return None,
    })
}

fn is_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Nil | Expr::True | Expr::False | Expr::Number(_) | Expr::Integer(_) | Expr::Str(_)
    )
}
