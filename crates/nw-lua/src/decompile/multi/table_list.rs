use crate::{
    LuaError,
    decompile::{
        analysis::NodeId,
        ast::{Expr, Stmt, TableField},
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

    let end_pc = setlists.last().map_or(node.pc, |setlist| setlist.pc);
    let binding = builder
        .binding_for_def(table_reg, end_pc)
        .or_else(|| builder.binding_for_def(table_reg, node.pc));
    let declared = binding
        .as_ref()
        .is_some_and(|binding| builder.is_local_declared(binding.index));
    let name = binding.as_ref().map_or_else(
        || builder.name_for_ref(node.dest, node.pc),
        |binding| binding.name.clone(),
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

pub(crate) fn constructor_fields(
    builder: &mut StatementBuilder<'_>,
    setlists: &[SsaNode],
    keyed: &[SsaNode],
) -> Result<Vec<TableField>, LuaError> {
    let mut fields = Vec::new();
    for setlist in setlists {
        let SsaOp::SetList { values, count, .. } = &setlist.op else {
            continue;
        };
        let fixed_count = *count != 0;
        let last_index = values.len().saturating_sub(1);
        for (index, value) in values.iter().copied().enumerate() {
            let expr = if fixed_count && index == last_index {
                builder.expr_for_fixed_last_ref(value, setlist.pc)?
            } else {
                builder.expr_for_ref(value, setlist.pc)?
            };
            fields.push(TableField::List(expr));
        }
    }
    let list_count = fields.len();
    for settable in keyed {
        if let Some(field) = keyed_field(builder, settable, list_count)? {
            fields.push(field);
        }
    }
    Ok(fields)
}

fn keyed_field(
    builder: &mut StatementBuilder<'_>,
    node: &SsaNode,
    list_count: usize,
) -> Result<Option<TableField>, LuaError> {
    let SsaOp::SetTable { key, value, .. } = &node.op else {
        return Ok(None);
    };
    let key_expr = builder.expr_for_fixed_last_ref(*key, node.pc)?;
    let value = builder.expr_for_fixed_last_ref(*value, node.pc)?;
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
