use crate::{
    LuaError,
    decompile::ast::{Block, Expr, FuncBody, Stmt, TableField},
};

pub(super) fn block(block: &Block) -> Result<(), LuaError> {
    for stmt in &block.0 {
        stmt_constants(stmt)?;
    }
    Ok(())
}

fn stmt_constants(stmt: &Stmt) -> Result<(), LuaError> {
    match stmt {
        Stmt::Local { values, .. } => exprs(values),
        Stmt::Assign { targets, values } => {
            exprs(targets)?;
            exprs(values)
        }
        Stmt::Call(expr) => self::expr(expr),
        Stmt::Do(body) => block(body),
        Stmt::While { cond, body } => {
            expr(cond)?;
            block(body)
        }
        Stmt::Repeat { body, cond } => {
            block(body)?;
            expr(cond)
        }
        Stmt::If { arms, else_ } => {
            for (cond, body) in arms {
                expr(cond)?;
                block(body)?;
            }
            if let Some(body) = else_ {
                block(body)?;
            }
            Ok(())
        }
        Stmt::NumericFor {
            start,
            stop,
            step,
            body,
            ..
        } => {
            expr(start)?;
            expr(stop)?;
            if let Some(step) = step {
                expr(step)?;
            }
            block(body)
        }
        Stmt::GenericFor { exprs, body, .. } => {
            self::exprs(exprs)?;
            block(body)
        }
        Stmt::Function { body, .. } | Stmt::FunctionDecl { body, .. } => func_body(body),
        Stmt::Return(values) => exprs(values),
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => Ok(()),
    }
}

fn func_body(body: &FuncBody) -> Result<(), LuaError> {
    block(&body.body)
}

fn exprs(values: &[Expr]) -> Result<(), LuaError> {
    for value in values {
        expr(value)?;
    }
    Ok(())
}

fn expr(expr: &Expr) -> Result<(), LuaError> {
    match expr {
        Expr::Number(value) if value.is_nan() => Err(LuaError::Emit(
            "cannot emit an exact Lua 5.1 literal for NaN".to_string(),
        )),
        Expr::Index { obj, key } => {
            self::expr(obj)?;
            self::expr(key)
        }
        Expr::Field { obj, .. } => self::expr(obj),
        Expr::Call { func, args, .. } => {
            self::expr(func)?;
            exprs(args)
        }
        Expr::Function(body) => func_body(body),
        Expr::Table(fields) => {
            for field in fields {
                match field {
                    TableField::List(value) | TableField::Named { value, .. } => {
                        self::expr(value)?;
                    }
                    TableField::ExprKey { key, value } => {
                        self::expr(key)?;
                        self::expr(value)?;
                    }
                }
            }
            Ok(())
        }
        Expr::Binary { lhs, rhs, .. } => {
            self::expr(lhs)?;
            self::expr(rhs)
        }
        Expr::Unary { operand, .. } | Expr::Paren(operand) => self::expr(operand),
        Expr::Nil
        | Expr::True
        | Expr::False
        | Expr::VarArg
        | Expr::Number(_)
        | Expr::Integer(_)
        | Expr::Str(_)
        | Expr::Name(_)
        | Expr::Global(_) => Ok(()),
    }
}
