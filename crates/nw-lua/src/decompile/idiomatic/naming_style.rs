use bstr::BString;
use heck::ToUpperCamelCase;

use crate::decompile::{
    ast::{Block, Expr, FunctionName, Name, Stmt, TableField},
    naming::is_valid_identifier,
};

use super::engine::{CleanContext, Rewrite, Rule};

pub struct ModuleTableName;

impl Rule for ModuleTableName {
    fn rewrite_block(&self, block: Block, ctx: &CleanContext) -> Rewrite<Block> {
        if !ctx.in_root_function() {
            return Rewrite::unchanged(block);
        }
        let Some(stem) = ctx.module_stem.as_deref() else {
            return Rewrite::unchanged(block);
        };
        let Some(binding) = recognized_module_binding(&block) else {
            return Rewrite::unchanged(block);
        };
        if !binding.name.is_synthetic() {
            return Rewrite::unchanged(block);
        }

        let candidate = module_pascal_name(stem);
        let candidate_bytes = BString::from(candidate.as_str());
        if !is_valid_identifier(&candidate_bytes)
            || binding.name.as_bytes() == candidate_bytes.as_slice()
            || count_declarations(&block, binding.name.as_bytes()) != 1
            || count_declarations(&block, candidate_bytes.as_slice()) != 0
            || contains_global_name(&block, candidate_bytes.as_slice())
        {
            return Rewrite::unchanged(block);
        }

        let mut block = block;
        let new_name = Name::synthetic(candidate);
        rename_binding(&mut block, binding.name.as_bytes(), &new_name);
        Rewrite::changed(block)
    }
}

#[derive(Debug)]
struct ModuleBinding {
    name: Name,
}

fn recognized_module_binding(block: &Block) -> Option<ModuleBinding> {
    let returned = returned_name(block)?;
    let table_index = block
        .0
        .iter()
        .position(|stmt| local_table_name(stmt) == Some(returned))?;
    let has_members = block
        .0
        .iter()
        .enumerate()
        .any(|(index, stmt)| index > table_index && module_member_stmt(stmt, returned));
    has_members.then(|| ModuleBinding {
        name: returned.clone(),
    })
}

fn returned_name(block: &Block) -> Option<&Name> {
    let Some(Stmt::Return(values)) = block.0.last() else {
        return None;
    };
    let [Expr::Name(name)] = values.as_slice() else {
        return None;
    };
    Some(name)
}

fn local_table_name(stmt: &Stmt) -> Option<&Name> {
    let Stmt::Local {
        names,
        attribs,
        values,
    } = stmt
    else {
        return None;
    };
    let ([name], [Expr::Table(_)]) = (names.as_slice(), values.as_slice()) else {
        return None;
    };
    attribs.is_empty().then_some(name)
}

fn module_member_stmt(stmt: &Stmt, module: &Name) -> bool {
    match stmt {
        Stmt::Assign { targets, .. } => targets
            .iter()
            .any(|target| target_base_name(target).is_some_and(|name| name == module)),
        Stmt::FunctionDecl { name, .. } => path_base(name).is_some_and(|name| name == module),
        _ => false,
    }
}

fn target_base_name(expr: &Expr) -> Option<&Name> {
    match expr {
        Expr::Name(name) => Some(name),
        Expr::Field { obj, .. } | Expr::Index { obj, .. } => target_base_name(obj),
        _ => None,
    }
}

fn path_base(name: &FunctionName) -> Option<&Name> {
    name.path.first()
}

fn module_pascal_name(stem: &str) -> String {
    normalized_module_stem(stem).to_upper_camel_case()
}

fn normalized_module_stem(stem: &str) -> String {
    stem.chars()
        .map(|ch| {
            if ch == '-' || ch == '.' || ch.is_whitespace() {
                '_'
            } else {
                ch
            }
        })
        .collect()
}

fn count_declarations(block: &Block, name: &[u8]) -> usize {
    block
        .0
        .iter()
        .map(|stmt| count_stmt_declarations(stmt, name))
        .sum()
}

fn count_stmt_declarations(stmt: &Stmt, name: &[u8]) -> usize {
    match stmt {
        Stmt::Local { names, .. } => names
            .iter()
            .filter(|candidate| candidate.as_bytes() == name)
            .count(),
        Stmt::GenericFor { names, body, .. } => {
            names
                .iter()
                .filter(|candidate| candidate.as_bytes() == name)
                .count()
                + count_declarations(body, name)
        }
        Stmt::NumericFor { var, body, .. } => {
            usize::from(var.as_bytes() == name) + count_declarations(body, name)
        }
        Stmt::Function {
            name: function_name,
            body,
            local,
        } => {
            usize::from(*local && function_name.as_bytes() == name)
                + count_func_declarations(body, name)
        }
        Stmt::FunctionDecl { body, .. } => count_func_declarations(body, name),
        Stmt::Do(body) | Stmt::While { body, .. } | Stmt::Repeat { body, .. } => {
            count_declarations(body, name)
        }
        Stmt::If { arms, else_ } => {
            arms.iter()
                .map(|(_, body)| count_declarations(body, name))
                .sum::<usize>()
                + else_
                    .as_ref()
                    .map_or(0, |body| count_declarations(body, name))
        }
        _ => 0,
    }
}

fn count_func_declarations(body: &crate::decompile::ast::FuncBody, name: &[u8]) -> usize {
    body.params
        .iter()
        .filter(|candidate| candidate.as_bytes() == name)
        .count()
        + count_declarations(&body.body, name)
}

fn contains_global_name(block: &Block, name: &[u8]) -> bool {
    block.0.iter().any(|stmt| stmt_contains_global(stmt, name))
}

fn stmt_contains_global(stmt: &Stmt, name: &[u8]) -> bool {
    match stmt {
        Stmt::Local { values, .. } => values.iter().any(|expr| expr_contains_global(expr, name)),
        Stmt::Assign { targets, values } => {
            targets.iter().any(|expr| expr_contains_global(expr, name))
                || values.iter().any(|expr| expr_contains_global(expr, name))
        }
        Stmt::Call(expr) => expr_contains_global(expr, name),
        Stmt::Do(body) => contains_global_name(body, name),
        Stmt::While { cond, body } => {
            expr_contains_global(cond, name) || contains_global_name(body, name)
        }
        Stmt::Repeat { body, cond } => {
            contains_global_name(body, name) || expr_contains_global(cond, name)
        }
        Stmt::NumericFor {
            start,
            stop,
            step,
            body,
            ..
        } => {
            expr_contains_global(start, name)
                || expr_contains_global(stop, name)
                || step
                    .as_ref()
                    .is_some_and(|step| expr_contains_global(step, name))
                || contains_global_name(body, name)
        }
        Stmt::GenericFor { exprs, body, .. } => {
            exprs.iter().any(|expr| expr_contains_global(expr, name))
                || contains_global_name(body, name)
        }
        Stmt::If { arms, else_ } => {
            arms.iter().any(|(cond, body)| {
                expr_contains_global(cond, name) || contains_global_name(body, name)
            }) || else_
                .as_ref()
                .is_some_and(|body| contains_global_name(body, name))
        }
        Stmt::Function { body, .. } | Stmt::FunctionDecl { body, .. } => {
            contains_global_name(&body.body, name)
        }
        Stmt::Return(values) => values.iter().any(|expr| expr_contains_global(expr, name)),
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => false,
    }
}

fn expr_contains_global(expr: &Expr, name: &[u8]) -> bool {
    match expr {
        Expr::Global(candidate) => candidate.as_slice() == name,
        Expr::Index { obj, key } => {
            expr_contains_global(obj, name) || expr_contains_global(key, name)
        }
        Expr::Field { obj, .. } => expr_contains_global(obj, name),
        Expr::Call { func, args, .. } => {
            expr_contains_global(func, name)
                || args.iter().any(|arg| expr_contains_global(arg, name))
        }
        Expr::Function(body) => contains_global_name(&body.body, name),
        Expr::Table(fields) => fields.iter().any(|field| match field {
            TableField::List(value) => expr_contains_global(value, name),
            TableField::Named { value, .. } => expr_contains_global(value, name),
            TableField::ExprKey { key, value } => {
                expr_contains_global(key, name) || expr_contains_global(value, name)
            }
        }),
        Expr::Binary { lhs, rhs, .. } => {
            expr_contains_global(lhs, name) || expr_contains_global(rhs, name)
        }
        Expr::Unary { operand, .. } | Expr::Paren(operand) => expr_contains_global(operand, name),
        _ => false,
    }
}

fn rename_binding(block: &mut Block, from: &[u8], to: &Name) {
    for stmt in &mut block.0 {
        rename_stmt(stmt, from, to);
    }
}

fn rename_stmt(stmt: &mut Stmt, from: &[u8], to: &Name) {
    match stmt {
        Stmt::Local { names, values, .. } => {
            rename_names(names, from, to);
            for value in values {
                rename_expr(value, from, to);
            }
        }
        Stmt::Assign { targets, values } => {
            for target in targets {
                rename_expr(target, from, to);
            }
            for value in values {
                rename_expr(value, from, to);
            }
        }
        Stmt::Call(expr) => rename_expr(expr, from, to),
        Stmt::Do(body) => rename_binding(body, from, to),
        Stmt::While { cond, body } => {
            rename_expr(cond, from, to);
            rename_binding(body, from, to);
        }
        Stmt::Repeat { body, cond } => {
            rename_binding(body, from, to);
            rename_expr(cond, from, to);
        }
        Stmt::NumericFor {
            start,
            stop,
            step,
            body,
            ..
        } => {
            rename_expr(start, from, to);
            rename_expr(stop, from, to);
            if let Some(step) = step {
                rename_expr(step, from, to);
            }
            rename_binding(body, from, to);
        }
        Stmt::GenericFor { exprs, body, .. } => {
            for expr in exprs {
                rename_expr(expr, from, to);
            }
            rename_binding(body, from, to);
        }
        Stmt::If { arms, else_ } => {
            for (cond, body) in arms {
                rename_expr(cond, from, to);
                rename_binding(body, from, to);
            }
            if let Some(body) = else_ {
                rename_binding(body, from, to);
            }
        }
        Stmt::Function { name, body, local } => {
            if *local && name.as_bytes() == from {
                *name = to.clone();
            }
            rename_func_body(body, from, to);
        }
        Stmt::FunctionDecl { name, body } => {
            if let Some(first) = name.path.first_mut()
                && first.as_bytes() == from
            {
                *first = to.clone();
            }
            rename_func_body(body, from, to);
        }
        Stmt::Return(values) => {
            for value in values {
                rename_expr(value, from, to);
            }
        }
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => {}
    }
}

fn rename_func_body(body: &mut crate::decompile::ast::FuncBody, from: &[u8], to: &Name) {
    rename_names(&mut body.params, from, to);
    rename_binding(&mut body.body, from, to);
}

fn rename_names(names: &mut [Name], from: &[u8], to: &Name) {
    for name in names {
        if name.as_bytes() == from {
            *name = to.clone();
        }
    }
}

fn rename_expr(expr: &mut Expr, from: &[u8], to: &Name) {
    match expr {
        Expr::Name(name) if name.as_bytes() == from => *name = to.clone(),
        Expr::Index { obj, key } => {
            rename_expr(obj, from, to);
            rename_expr(key, from, to);
        }
        Expr::Field { obj, .. } => rename_expr(obj, from, to),
        Expr::Call { func, args, .. } => {
            rename_expr(func, from, to);
            for arg in args {
                rename_expr(arg, from, to);
            }
        }
        Expr::Function(body) => rename_func_body(body, from, to),
        Expr::Table(fields) => {
            for field in fields {
                match field {
                    TableField::List(value) => rename_expr(value, from, to),
                    TableField::Named { value, .. } => rename_expr(value, from, to),
                    TableField::ExprKey { key, value } => {
                        rename_expr(key, from, to);
                        rename_expr(value, from, to);
                    }
                }
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            rename_expr(lhs, from, to);
            rename_expr(rhs, from, to);
        }
        Expr::Unary { operand, .. } | Expr::Paren(operand) => rename_expr(operand, from, to),
        _ => {}
    }
}
