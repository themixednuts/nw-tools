use crate::{
    LuaError,
    decompile::{
        analysis::NodeId,
        ast::{Expr, Name, Stmt},
        naming::LocalBinding,
        stmt_build::StatementBuilder,
    },
    ir::{SsaNode, SsaRef},
};

pub(crate) fn fixed_call_assignment(
    builder: &mut StatementBuilder<'_>,
    node_id: NodeId,
    node: &SsaNode,
    func: SsaRef,
    args: &[SsaRef],
    arg_count: i32,
    return_count: i32,
) -> Result<Stmt, LuaError> {
    let Some(base) = node.dest.reg_index() else {
        return Err(LuaError::Malformed(
            "fixed-result call has no destination register".to_string(),
        ));
    };
    let call = builder.call_expr(func, args, arg_count, node.pc)?;
    fixed_result_assignment(builder, node_id, node, base, return_count - 1, call)
}

pub(crate) fn fixed_result_assignment(
    builder: &mut StatementBuilder<'_>,
    node_id: NodeId,
    node: &SsaNode,
    base: u16,
    result_count: i32,
    value: Expr,
) -> Result<Stmt, LuaError> {
    let slots = result_slots(builder, node_id, node, base, result_count)?;
    emit_result_assignment(builder, slots, value)
}

fn result_slots(
    builder: &StatementBuilder<'_>,
    node_id: NodeId,
    node: &SsaNode,
    base: u16,
    result_count: i32,
) -> Result<Vec<ResultSlot>, LuaError> {
    let count = usize::try_from(result_count)
        .map_err(|_| LuaError::Malformed("negative result count".to_string()))?;
    let mut slots = Vec::with_capacity(count);
    for offset in 0..count {
        let reg = base.saturating_add(u16::try_from(offset).unwrap_or(u16::MAX));
        let reference = builder
            .def_at_reg(node_id, reg)
            .unwrap_or_else(|| fallback_ref(node, reg));
        let binding = builder.binding_for_def(reg, node.pc);
        let declared = binding
            .as_ref()
            .is_some_and(|binding| builder.is_local_declared(binding.index));
        let name = binding.as_ref().map_or_else(
            || builder.name_for_ref(reference, node.pc),
            |binding| builder.name_for_binding_def(binding, reference),
        );
        slots.push(ResultSlot {
            reference,
            binding,
            declared,
            name,
        });
    }
    Ok(slots)
}

fn emit_result_assignment(
    builder: &mut StatementBuilder<'_>,
    slots: Vec<ResultSlot>,
    value: Expr,
) -> Result<Stmt, LuaError> {
    let has_declared_target = slots.iter().any(|slot| slot.declared);
    if has_declared_target
        && slots
            .iter()
            .any(|slot| slot.binding.is_none() || !slot.declared)
    {
        return Err(LuaError::Unsupported(
            "mixed existing and new multi-result targets are deferred".to_string(),
        ));
    }

    let names = slots
        .iter()
        .map(|slot| slot.name.clone())
        .collect::<Vec<_>>();

    for slot in &slots {
        builder.mark_materialized(slot.reference, slot.name.clone());
        if !has_declared_target && let Some(binding) = &slot.binding {
            builder.mark_local_declared(binding.index);
        }
        if !has_declared_target && slot.binding.is_none() {
            builder.mark_synthetic_declared(slot.name.clone());
        }
    }

    if has_declared_target {
        Ok(Stmt::Assign {
            targets: names.into_iter().map(Expr::Name).collect(),
            values: vec![value],
        })
    } else {
        Ok(Stmt::Local {
            names,
            attribs: Vec::new(),
            values: vec![value],
        })
    }
}

fn fallback_ref(node: &SsaNode, reg: u16) -> SsaRef {
    SsaRef::Reg {
        reg,
        ver: node.dest.version().unwrap_or(0),
    }
}

struct ResultSlot {
    reference: SsaRef,
    binding: Option<LocalBinding>,
    declared: bool,
    name: Name,
}
