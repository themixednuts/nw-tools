use crate::{
    LuaError,
    decompile::{
        ast::{Expr, Stmt},
        stmt_build::StatementBuilder,
    },
};

use super::plan::{CallTransferPlan, MultiLocalPlan, SwapPlan};

pub(crate) fn emit_call_transfer(
    builder: &mut StatementBuilder<'_>,
    plan: &CallTransferPlan,
) -> Result<Vec<Stmt>, LuaError> {
    let call = builder
        .node(plan.call)
        .cloned()
        .ok_or_else(|| LuaError::Malformed("planned result call is missing".to_string()))?;
    let crate::ir::SsaOp::Call {
        func,
        args,
        arg_count,
        ..
    } = &call.op
    else {
        return Err(LuaError::Malformed(
            "planned result transfer does not begin with CALL".to_string(),
        ));
    };
    let value = builder.call_expr(*func, args, *arg_count, call.pc)?;
    let declaration_indexes = plan
        .writes
        .iter()
        .enumerate()
        .filter_map(|(index, write)| builder.will_declare(write.dest).then_some(index))
        .collect::<Vec<_>>();
    let mut stmts = Vec::with_capacity(2);
    if declaration_indexes.len() == plan.writes.len() {
        for write in &plan.writes {
            if !builder.claim_declaration(write.dest) {
                return Err(LuaError::Malformed(
                    "planned result transfer lost declaration ownership".to_string(),
                ));
            }
            builder.activate(write.dest);
        }
        stmts.push(Stmt::Local {
            names: plan.writes.iter().map(|write| write.name.clone()).collect(),
            attribs: Vec::new(),
            values: vec![value],
        });
        return Ok(stmts);
    }

    if !declaration_indexes.is_empty() {
        let names = declaration_indexes
            .into_iter()
            .map(|index| {
                let write = &plan.writes[index];
                let claimed = builder.claim_declaration(write.dest);
                debug_assert!(claimed, "declaration query and claim must agree");
                write.name.clone()
            })
            .collect();
        stmts.push(Stmt::Local {
            names,
            attribs: Vec::new(),
            values: Vec::new(),
        });
    }
    for write in &plan.writes {
        builder.activate(write.dest);
    }
    stmts.push(Stmt::Assign {
        targets: plan
            .writes
            .iter()
            .map(|write| Expr::Name(write.name.clone()))
            .collect(),
        values: vec![value],
    });
    Ok(stmts)
}

pub(crate) fn emit_multi_local(
    builder: &mut StatementBuilder<'_>,
    plan: &MultiLocalPlan,
) -> Result<Stmt, LuaError> {
    let name_count = plan
        .entries
        .iter()
        .map(|entry| entry.names.len())
        .sum::<usize>()
        + plan.leading_nils.len();
    let mut names = Vec::with_capacity(name_count);
    names.extend(plan.leading_nils.iter().map(|local| local.name.clone()));
    let mut values = vec![Expr::Nil; plan.leading_nils.len()];
    values.reserve(plan.entries.len());

    for entry in &plan.entries {
        let node = builder.node(entry.node).cloned().ok_or_else(|| {
            LuaError::Malformed("planned multi-local node is missing".to_string())
        })?;
        values.push(builder.expr_for_node(&node)?);
        names.extend(entry.names.iter().cloned());
    }
    while values
        .last()
        .is_some_and(|value| matches!(value, Expr::Nil))
    {
        values.pop();
    }
    for reference in plan
        .leading_nils
        .iter()
        .map(|local| &local.reference)
        .chain(plan.entries.iter().flat_map(|entry| entry.refs.iter()))
    {
        if !builder.claim_declaration(*reference) {
            return Err(LuaError::Malformed(
                "planned multi-local did not own every declaration".to_string(),
            ));
        }
        builder.activate(*reference);
    }

    Ok(Stmt::Local {
        names,
        attribs: Vec::new(),
        values,
    })
}

pub(crate) fn emit_swap(
    builder: &mut StatementBuilder<'_>,
    plan: &SwapPlan,
) -> Result<Stmt, LuaError> {
    let mut targets = Vec::with_capacity(plan.writes.len());
    let mut values = Vec::with_capacity(plan.writes.len());
    for write in plan.writes.iter().rev() {
        targets.push(Expr::Name(write.name.clone()));
        let (src, pc) = plan
            .saves
            .iter()
            .find_map(|save| (save.dest == write.src).then_some((save.src, save.pc)))
            .unwrap_or((write.src, write.pc));
        values.push(builder.expr_for_ref(src, pc)?);
    }
    for write in &plan.writes {
        builder.activate(write.dest);
    }
    Ok(Stmt::Assign { targets, values })
}
