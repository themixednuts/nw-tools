use crate::{
    LuaError,
    decompile::{
        analysis::NodeId,
        ast::{Expr, Name, Stmt},
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
    emit_result_assignment(builder, slots, node.pc, value)
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
        let declares = builder.will_declare(reference);
        let name = binding.as_ref().map_or_else(
            || builder.name_for_ref(reference, node.pc),
            |binding| builder.name_for_binding_def(binding, reference),
        );
        slots.push(ResultSlot {
            reference,
            declares,
            name,
        });
    }
    Ok(slots)
}

fn emit_result_assignment(
    builder: &mut StatementBuilder<'_>,
    slots: Vec<ResultSlot>,
    pc: i32,
    value: Expr,
) -> Result<Stmt, LuaError> {
    let declares = slots.iter().all(|slot| slot.declares);
    if !declares && slots.iter().any(|slot| slot.declares) {
        return Err(LuaError::Unsupported(format!(
            "mixed existing and new multi-result targets at pc {}: {}",
            pc,
            slots
                .iter()
                .map(|slot| format!("{:?}:{}", slot.name, slot.declares))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    let names = slots
        .iter()
        .map(|slot| slot.name.clone())
        .collect::<Vec<_>>();

    for slot in &slots {
        if declares && !builder.claim_declaration(slot.reference) {
            return Err(LuaError::Malformed(
                "planned multi-result declaration was already consumed".to_string(),
            ));
        }
        builder.activate(slot.reference);
    }

    if declares {
        Ok(Stmt::Local {
            names,
            attribs: Vec::new(),
            values: vec![value],
        })
    } else {
        Ok(Stmt::Assign {
            targets: names.into_iter().map(Expr::Name).collect(),
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
    declares: bool,
    name: Name,
}
