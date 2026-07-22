use crate::decompile::{
    ast::{
        Block, Expr, FuncBody, FunctionName, Name, Stmt, binding_references_in_func_body,
        binding_spelling_available_in_func_body, binding_usage_in_block,
        rename_binding_in_func_body,
    },
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
        let captures_declared_binding = name
            .binding()
            .is_none_or(|binding| binding_references_in_func_body(body, binding));
        if !attribs.is_empty() || captures_declared_binding {
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
        let Some(binding) = first.binding().cloned() else {
            return Rewrite::unchanged(Stmt::FunctionDecl { name, body });
        };
        let usage = binding_usage_in_block(&body.body, &binding);
        if !first.is_synthetic()
            || !binding_spelling_available_in_func_body(&body, &binding, b"self")
            || !usage.is_receiver_only()
        {
            return Rewrite::unchanged(Stmt::FunctionDecl { name, body });
        }

        rename_binding_in_func_body(&mut body, &binding, b"self");
        Rewrite::changed(methodize(name, body))
    }
}

fn methodize(mut name: FunctionName, mut body: FuncBody) -> Stmt {
    let method = name
        .path
        .pop()
        .expect("method sugar requires at least one field segment");
    name.method = Some(method);
    body.implicit_receiver = Some(body.params.remove(0).renamed("self"));
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
    target.same_binding(name).then(|| Stmt::Function {
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
