use std::collections::HashSet;

use crate::{
    LuaError,
    decompile::{
        analysis::NodeId,
        ast::{Expr, Name, Stmt, TableField},
        expr_build::ident_from_string_expr,
        stmt_build::StatementBuilder,
    },
    ir::{SsaNode, SsaOp, SsaRef},
};

use super::MultiEmit;

pub(crate) fn try_emit(
    builder: &mut StatementBuilder<'_>,
    node_ids: &[NodeId],
    index: usize,
    _node_id: NodeId,
    node: &SsaNode,
    skip: &dyn Fn(&SsaNode) -> bool,
) -> Result<Option<MultiEmit>, LuaError> {
    if !matches!(&node.op, SsaOp::NewTable { .. }) {
        return Ok(None);
    }
    let Some(table_reg) = node.dest.reg_index() else {
        return Ok(None);
    };

    let mut setlists = Vec::new();
    let mut keyed = Vec::new();
    let mut last_setlist_index = None;
    let mut cursor = index + 1;
    while let Some(id) = node_ids.get(cursor).copied() {
        let Some(current) = builder.node(id) else {
            break;
        };
        if current.is_meta_only || skip(current) {
            break;
        }
        if is_matching_setlist(current, node.dest, table_reg) {
            setlists.push(current.clone());
            last_setlist_index = Some(cursor);
            cursor += 1;
            continue;
        }
        if is_matching_settable(current, node.dest, table_reg) {
            keyed.push(current.clone());
            cursor += 1;
            continue;
        }
        if is_constructor_setup(current, table_reg) {
            cursor += 1;
            continue;
        }
        break;
    }

    let Some(last_setlist_index) = last_setlist_index else {
        return Ok(None);
    };

    let fields = constructor_fields(builder, &setlists, &keyed)?;
    if fields.is_empty() {
        return Ok(None);
    }
    if fields_reference_consumed_setup(builder, &node_ids[index + 1..=last_setlist_index], &fields)
    {
        return Ok(None);
    }

    let end_pc = setlists.last().map_or(node.pc, |setlist| setlist.pc);
    let binding = builder
        .binding_for_def(table_reg, end_pc)
        .or_else(|| builder.binding_for_def(table_reg, node.pc));
    let declared = binding
        .as_ref()
        .is_some_and(|binding| builder.is_local_declared(binding.index));
    let name = binding.as_ref().map_or_else(
        || builder.name_for_ref(node.dest, node.pc),
        |binding| builder.name_for_binding_def(binding, node.dest),
    );

    builder.mark_materialized(node.dest, name.clone());
    let table = Expr::Table(fields);
    let stmt = if declared {
        Stmt::Assign {
            targets: vec![Expr::Name(name)],
            values: vec![table],
        }
    } else {
        if let Some(binding) = &binding {
            builder.mark_local_declared(binding.index);
        } else {
            builder.mark_synthetic_declared(name.clone());
        }
        Stmt::Local {
            names: vec![name],
            attribs: Vec::new(),
            values: vec![table],
        }
    };

    Ok(Some(MultiEmit {
        stmt,
        consumed: node_ids[index..=last_setlist_index].to_vec(),
    }))
}

fn fields_reference_consumed_setup(
    builder: &StatementBuilder<'_>,
    consumed_setup: &[NodeId],
    fields: &[TableField],
) -> bool {
    let consumed_names = consumed_setup
        .iter()
        .filter_map(|id| builder.node(*id))
        .filter(|node| !is_constructor_mutation(node))
        .filter_map(|node| {
            if matches!(node.dest, SsaRef::Reg { .. }) {
                Some(builder.name_for_ref(node.dest, node.pc))
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    !consumed_names.is_empty()
        && fields
            .iter()
            .any(|field| table_field_contains_name(field, &consumed_names))
}

fn is_constructor_mutation(node: &SsaNode) -> bool {
    matches!(&node.op, SsaOp::SetTable { .. } | SsaOp::SetList { .. })
}

fn table_field_contains_name(field: &TableField, names: &HashSet<Name>) -> bool {
    match field {
        TableField::List(value) | TableField::Named { value, .. } => {
            expr_contains_name(value, names)
        }
        TableField::ExprKey { key, value } => {
            expr_contains_name(key, names) || expr_contains_name(value, names)
        }
    }
}

fn expr_contains_name(expr: &Expr, names: &HashSet<Name>) -> bool {
    match expr {
        Expr::Name(name) => names.contains(name),
        Expr::Index { obj, key } => {
            expr_contains_name(obj, names) || expr_contains_name(key, names)
        }
        Expr::Field { obj, .. } => expr_contains_name(obj, names),
        Expr::Call { func, args, .. } => {
            expr_contains_name(func, names) || args.iter().any(|arg| expr_contains_name(arg, names))
        }
        Expr::Function(body) => body
            .body
            .0
            .iter()
            .any(|stmt| stmt_contains_name(stmt, names)),
        Expr::Table(fields) => fields
            .iter()
            .any(|field| table_field_contains_name(field, names)),
        Expr::Binary { lhs, rhs, .. } => {
            expr_contains_name(lhs, names) || expr_contains_name(rhs, names)
        }
        Expr::Unary { operand, .. } | Expr::Paren(operand) => expr_contains_name(operand, names),
        Expr::Nil
        | Expr::True
        | Expr::False
        | Expr::VarArg
        | Expr::Number(_)
        | Expr::Integer(_)
        | Expr::Str(_)
        | Expr::Global(_) => false,
    }
}

fn stmt_contains_name(stmt: &Stmt, names: &HashSet<Name>) -> bool {
    match stmt {
        Stmt::Local { values, .. } | Stmt::Return(values) => {
            values.iter().any(|expr| expr_contains_name(expr, names))
        }
        Stmt::Assign { targets, values } => targets
            .iter()
            .chain(values)
            .any(|expr| expr_contains_name(expr, names)),
        Stmt::Call(expr) => expr_contains_name(expr, names),
        Stmt::Do(block) => block.0.iter().any(|stmt| stmt_contains_name(stmt, names)),
        Stmt::If { arms, else_ } => {
            arms.iter().any(|(cond, block)| {
                expr_contains_name(cond, names)
                    || block.0.iter().any(|stmt| stmt_contains_name(stmt, names))
            }) || else_
                .as_ref()
                .is_some_and(|block| block.0.iter().any(|stmt| stmt_contains_name(stmt, names)))
        }
        Stmt::While { cond, body } | Stmt::Repeat { cond, body } => {
            expr_contains_name(cond, names)
                || body.0.iter().any(|stmt| stmt_contains_name(stmt, names))
        }
        Stmt::NumericFor {
            start,
            stop,
            step,
            body,
            ..
        } => {
            expr_contains_name(start, names)
                || expr_contains_name(stop, names)
                || step
                    .as_ref()
                    .is_some_and(|step| expr_contains_name(step, names))
                || body.0.iter().any(|stmt| stmt_contains_name(stmt, names))
        }
        Stmt::GenericFor { exprs, body, .. } => {
            exprs.iter().any(|expr| expr_contains_name(expr, names))
                || body.0.iter().any(|stmt| stmt_contains_name(stmt, names))
        }
        Stmt::Function { body, .. } | Stmt::FunctionDecl { body, .. } => body
            .body
            .0
            .iter()
            .any(|stmt| stmt_contains_name(stmt, names)),
        Stmt::Break | Stmt::Goto(_) | Stmt::Label(_) => false,
    }
}

pub(crate) fn constructor_fields(
    builder: &mut StatementBuilder<'_>,
    setlists: &[SsaNode],
    keyed: &[SsaNode],
) -> Result<Vec<TableField>, LuaError> {
    fields_from_nodes(
        setlists.iter(),
        keyed.iter(),
        &mut |reference, pc, mode| match mode {
            ConstructorValueMode::Normal => builder.expr_for_ref(reference, pc),
            ConstructorValueMode::FixedLast => builder.expr_for_fixed_last_ref(reference, pc),
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstructorValueMode {
    Normal,
    FixedLast,
}

pub(crate) fn fields_from_nodes<'a, S, K, F>(
    setlists: S,
    keyed: K,
    expr_for_ref: &mut F,
) -> Result<Vec<TableField>, LuaError>
where
    S: IntoIterator<Item = &'a SsaNode>,
    K: IntoIterator<Item = &'a SsaNode>,
    F: FnMut(SsaRef, i32, ConstructorValueMode) -> Result<Expr, LuaError>,
{
    let mut fields = Vec::new();
    for setlist in setlists {
        let SsaOp::SetList { values, count, .. } = &setlist.op else {
            continue;
        };
        let fixed_count = *count != 0;
        let last_index = values.len().saturating_sub(1);
        for (index, value) in values.iter().copied().enumerate() {
            let expr = if fixed_count && index == last_index {
                expr_for_ref(value, setlist.pc, ConstructorValueMode::FixedLast)?
            } else {
                expr_for_ref(value, setlist.pc, ConstructorValueMode::Normal)?
            };
            fields.push(TableField::List(expr));
        }
    }
    let list_count = fields.len();
    for settable in keyed {
        if let Some(field) = keyed_field(settable, list_count, expr_for_ref)? {
            fields.push(field);
        }
    }
    Ok(fields)
}

fn keyed_field(
    node: &SsaNode,
    list_count: usize,
    expr_for_ref: &mut impl FnMut(SsaRef, i32, ConstructorValueMode) -> Result<Expr, LuaError>,
) -> Result<Option<TableField>, LuaError> {
    let SsaOp::SetTable { key, value, .. } = &node.op else {
        return Ok(None);
    };
    let key_expr = expr_for_ref(*key, node.pc, ConstructorValueMode::FixedLast)?;
    let value = expr_for_ref(*value, node.pc, ConstructorValueMode::FixedLast)?;
    if let Some(name) = ident_from_string_expr(&key_expr) {
        return Ok(Some(TableField::Named { name, value }));
    }
    if key_may_overlap_list(&key_expr, list_count) {
        return Err(LuaError::Unsupported(
            "table constructor has keyed field that may overlap list entries".to_string(),
        ));
    }
    Ok(Some(TableField::ExprKey {
        key: key_expr,
        value,
    }))
}

fn key_may_overlap_list(key: &Expr, list_count: usize) -> bool {
    if list_count == 0 {
        return false;
    }
    match key {
        Expr::Integer(value) => {
            *value >= 1 && usize::try_from(*value).is_ok_and(|v| v <= list_count)
        }
        Expr::Number(value) if value.fract() == 0.0 && *value >= 1.0 => *value <= list_count as f64,
        Expr::Number(_) | Expr::Str(_) | Expr::True | Expr::False | Expr::Nil => false,
        _ => true,
    }
}

pub(crate) fn is_matching_setlist(node: &SsaNode, table: SsaRef, table_reg: u16) -> bool {
    matches!(
        &node.op,
        SsaOp::SetList {
            table: setlist_table,
            base,
            ..
        } if *setlist_table == table || *base == table_reg
    )
}

pub(crate) fn is_matching_settable(node: &SsaNode, table: SsaRef, table_reg: u16) -> bool {
    matches!(
        &node.op,
        SsaOp::SetTable {
            table: settable_table,
            value,
            ..
        } if (*settable_table == table || settable_table.reg_index() == Some(table_reg))
            && value.reg_index() != Some(table_reg)
    )
}

pub(crate) fn is_constructor_setup(node: &SsaNode, table_reg: u16) -> bool {
    if matches!(
        &node.op,
        SsaOp::SetTable { table, .. } if table.reg_index().is_some_and(|reg| reg > table_reg)
    ) {
        return true;
    }
    if matches!(
        &node.op,
        SsaOp::SetList { base, .. } if *base > table_reg
    ) {
        return true;
    }
    let Some(dest_reg) = node.dest.reg_index() else {
        return matches!(&node.op, SsaOp::Nop);
    };
    if dest_reg <= table_reg {
        return false;
    }
    matches!(
        &node.op,
        SsaOp::Nop
            | SsaOp::LoadK { .. }
            | SsaOp::LoadBool { .. }
            | SsaOp::LoadNil { .. }
            | SsaOp::GetGlobal { .. }
            | SsaOp::GetTable { .. }
            | SsaOp::GetUpval { .. }
            | SsaOp::Move { .. }
            | SsaOp::BinOp { .. }
            | SsaOp::UnOp { .. }
            | SsaOp::Concat { .. }
            | SsaOp::SelfOp { .. }
            | SsaOp::Call { .. }
            | SsaOp::VarArg { .. }
            | SsaOp::Closure { .. }
            | SsaOp::NewTable { .. }
    )
}
