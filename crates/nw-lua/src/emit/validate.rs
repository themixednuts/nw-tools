use std::collections::HashSet;

use crate::{
    LuaError,
    decompile::ast::{BindingId, Block, Expr, FuncBody, Name, Stmt, TableField},
};

pub(super) fn block(block: &Block) -> Result<(), LuaError> {
    BindingValidator::default().validate(block)?;
    block_constants(block)
}

fn block_constants(block: &Block) -> Result<(), LuaError> {
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
        Stmt::Do(body) => block_constants(body),
        Stmt::While { cond, body } => {
            expr(cond)?;
            block_constants(body)
        }
        Stmt::Repeat { body, cond } => {
            block_constants(body)?;
            expr(cond)
        }
        Stmt::If { arms, else_ } => {
            for (cond, body) in arms {
                expr(cond)?;
                block_constants(body)?;
            }
            if let Some(body) = else_ {
                block_constants(body)?;
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
            block_constants(body)
        }
        Stmt::GenericFor { exprs, body, .. } => {
            self::exprs(exprs)?;
            block_constants(body)
        }
        Stmt::Function { body, .. } | Stmt::FunctionDecl { body, .. } => func_body(body),
        Stmt::Return(values) => exprs(values),
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => Ok(()),
    }
}

fn func_body(body: &FuncBody) -> Result<(), LuaError> {
    block_constants(&body.body)
}

fn exprs(values: &[Expr]) -> Result<(), LuaError> {
    for value in values {
        expr(value)?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct BindingValidator {
    scopes: Vec<HashSet<BindingId>>,
}

impl BindingValidator {
    fn validate(mut self, block: &Block) -> Result<(), LuaError> {
        self.scopes.push(HashSet::new());
        self.block(block)
    }

    fn block(&mut self, block: &Block) -> Result<(), LuaError> {
        for (index, stmt) in block.0.iter().enumerate() {
            self.stmt(stmt).map_err(|error| match error {
                LuaError::Emit(message) => {
                    LuaError::Emit(format!("statement {index} ({stmt:?}): {message}"))
                }
                other => other,
            })?;
        }
        Ok(())
    }

    fn nested_block(&mut self, block: &Block) -> Result<(), LuaError> {
        self.scopes.push(HashSet::new());
        let result = self.block(block);
        self.scopes.pop();
        result
    }

    fn stmt(&mut self, stmt: &Stmt) -> Result<(), LuaError> {
        match stmt {
            Stmt::Local { names, values, .. } => {
                self.exprs(values)?;
                self.declare_all(names);
            }
            Stmt::Assign { targets, values } => {
                self.exprs(targets)?;
                self.exprs(values)?;
            }
            Stmt::Call(expr) => self.expr(expr)?,
            Stmt::Do(body) => self.nested_block(body)?,
            Stmt::While { cond, body } => {
                self.expr(cond)?;
                self.nested_block(body)?;
            }
            Stmt::Repeat { body, cond } => {
                self.scopes.push(HashSet::new());
                let result = self.block(body).and_then(|()| self.expr(cond));
                self.scopes.pop();
                result?;
            }
            Stmt::If { arms, else_ } => {
                for (cond, body) in arms {
                    self.expr(cond)?;
                    self.nested_block(body)?;
                }
                if let Some(body) = else_ {
                    self.nested_block(body)?;
                }
            }
            Stmt::NumericFor {
                var,
                start,
                stop,
                step,
                body,
            } => {
                self.expr(start)?;
                self.expr(stop)?;
                if let Some(step) = step {
                    self.expr(step)?;
                }
                self.scopes.push(HashSet::new());
                self.declare(var);
                let result = self.block(body);
                self.scopes.pop();
                result?;
            }
            Stmt::GenericFor { names, exprs, body } => {
                self.exprs(exprs)?;
                self.scopes.push(HashSet::new());
                self.declare_all(names);
                let result = self.block(body);
                self.scopes.pop();
                result?;
            }
            Stmt::Function { name, body, local } => {
                if *local {
                    self.declare(name);
                } else {
                    self.read(name)?;
                }
                self.function(body)?;
            }
            Stmt::FunctionDecl { name, body } => {
                if let Some(root) = name.path.first() {
                    self.read(root)?;
                }
                self.function(body)?;
            }
            Stmt::Return(values) => self.exprs(values)?,
            Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => {}
        }
        Ok(())
    }

    fn function(&mut self, body: &FuncBody) -> Result<(), LuaError> {
        self.scopes.push(HashSet::new());
        if let Some(receiver) = &body.implicit_receiver {
            self.declare(receiver);
        }
        self.declare_all(&body.params);
        let result = self.block(&body.body);
        self.scopes.pop();
        result
    }

    fn exprs(&mut self, exprs: &[Expr]) -> Result<(), LuaError> {
        for expr in exprs {
            self.expr(expr)?;
        }
        Ok(())
    }

    fn expr(&mut self, expr: &Expr) -> Result<(), LuaError> {
        match expr {
            Expr::Name(name) => self.read(name),
            Expr::Index { obj, key } => {
                self.expr(obj)?;
                self.expr(key)
            }
            Expr::Field { obj, .. } => self.expr(obj),
            Expr::Call { func, args, .. } => {
                self.expr(func)?;
                self.exprs(args)
            }
            Expr::Function(body) => self.function(body),
            Expr::Table(fields) => {
                for field in fields {
                    match field {
                        TableField::List(value) | TableField::Named { value, .. } => {
                            self.expr(value)?;
                        }
                        TableField::ExprKey { key, value } => {
                            self.expr(key)?;
                            self.expr(value)?;
                        }
                    }
                }
                Ok(())
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.expr(lhs)?;
                self.expr(rhs)
            }
            Expr::Unary { operand, .. } | Expr::Paren(operand) => self.expr(operand),
            Expr::Nil
            | Expr::True
            | Expr::False
            | Expr::VarArg
            | Expr::Number(_)
            | Expr::Integer(_)
            | Expr::Str(_)
            | Expr::Global(_) => Ok(()),
        }
    }

    fn declare_all(&mut self, names: &[Name]) {
        for name in names {
            self.declare(name);
        }
    }

    fn declare(&mut self, name: &Name) {
        if let Some(binding) = name.binding()
            && !binding.is_external_upvalue()
            && let Some(scope) = self.scopes.last_mut()
        {
            scope.insert(binding.clone());
        }
    }

    fn read(&self, name: &Name) -> Result<(), LuaError> {
        let Some(binding) = name.binding() else {
            return Ok(());
        };
        if binding.is_external_upvalue()
            || self
                .scopes
                .iter()
                .rev()
                .any(|scope| scope.contains(binding))
        {
            return Ok(());
        }
        Err(LuaError::Emit(format!(
            "identifier {} references undeclared binding {binding:?}",
            String::from_utf8_lossy(name.as_bytes())
        )))
    }
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
