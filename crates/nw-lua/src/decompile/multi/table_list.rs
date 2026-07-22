use crate::{
    LuaError,
    decompile::{
        ast::{Expr, Stmt, TableField},
        expr_build::ident_from_string_expr,
        stmt_build::StatementBuilder,
    },
    ir::{SsaNode, SsaOp, SsaRef},
};

use super::plan::TableConstructorEmission;

pub(crate) fn emit(
    builder: &mut StatementBuilder<'_>,
    plan: &TableConstructorEmission,
) -> Result<Stmt, LuaError> {
    let start =
        plan.members.first().copied().ok_or_else(|| {
            LuaError::Malformed("planned table constructor has no owner".to_string())
        })?;
    let node = builder
        .node(start)
        .cloned()
        .ok_or_else(|| LuaError::Malformed("planned table constructor is missing".to_string()))?;
    let table_reg = node.dest.reg_index().ok_or_else(|| {
        LuaError::Malformed("planned table constructor has no register".to_string())
    })?;
    let setlists = plan
        .constructor
        .setlists()
        .iter()
        .filter_map(|id| builder.node(*id))
        .cloned()
        .collect::<Vec<_>>();
    let keyed = plan
        .constructor
        .keyed()
        .iter()
        .filter_map(|id| builder.node(*id))
        .cloned()
        .collect::<Vec<_>>();

    let fields = constructor_fields(builder, &setlists, &keyed)?;
    let end_pc = builder
        .node(plan.constructor.end())
        .map_or(node.pc, |end| end.pc);
    let binding = builder
        .binding_for_def(table_reg, end_pc)
        .or_else(|| builder.binding_for_def(table_reg, node.pc));
    let declares = builder.claim_declaration(node.dest);
    let name = binding.as_ref().map_or_else(
        || builder.name_for_ref(node.dest, node.pc),
        |binding| builder.name_for_binding_def(binding, node.dest),
    );

    builder.activate(node.dest);
    let table = Expr::Table(fields);
    let stmt = if declares {
        Stmt::Local {
            names: vec![name],
            attribs: Vec::new(),
            values: vec![table],
        }
    } else {
        Stmt::Assign {
            targets: vec![Expr::Name(name)],
            values: vec![table],
        }
    };

    Ok(stmt)
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
