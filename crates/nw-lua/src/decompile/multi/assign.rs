use crate::{
    LuaError,
    decompile::{
        analysis::NodeId,
        ast::{Expr, Name, Stmt},
        stmt_build::StatementBuilder,
    },
    ir::{SsaNode, SsaOp, SsaRef},
};

use super::MultiEmit;

pub(crate) fn try_emit_multi_local(
    builder: &mut StatementBuilder<'_>,
    node_ids: &[NodeId],
    index: usize,
    node: &SsaNode,
    skip: &dyn Fn(&SsaNode) -> bool,
) -> Result<Option<MultiEmit>, LuaError> {
    if !is_direct_value_node(node) {
        return Ok(None);
    }
    let Some(base) = node.dest.reg_index() else {
        return Ok(None);
    };
    let Some(first_binding) = builder.binding_for_def(base, node.pc) else {
        return Ok(None);
    };
    if builder.is_local_declared(first_binding.index) {
        return Ok(None);
    }

    let mut entries = Vec::new();
    let mut expected = base;
    let mut cursor = index;
    let mut pending_setup = Vec::new();
    while let Some(id) = node_ids.get(cursor).copied() {
        let Some(current) = builder.node(id) else {
            break;
        };
        if current.is_meta_only || skip(current) || !is_direct_value_node(current) {
            break;
        }
        if is_call_setup(node_ids, cursor, expected, builder, skip) {
            pending_setup.push(id);
            cursor += 1;
            continue;
        }
        if current.dest.reg_index() != Some(expected) {
            break;
        }
        let result_count = local_result_count(current);
        if result_count == 0 {
            break;
        }
        let Some(slot) = local_value_slot(
            builder,
            id,
            current,
            expected,
            result_count,
            first_binding.start_pc,
        ) else {
            break;
        };
        if slot
            .local_indexes
            .iter()
            .any(|index| builder.is_local_declared(*index))
        {
            break;
        }
        entries.push(slot.with_setup(std::mem::take(&mut pending_setup)));
        expected = expected.saturating_add(result_count);
        cursor += 1;
        if result_count > 1 {
            break;
        }
    }

    let name_count = entries.iter().map(|entry| entry.names.len()).sum::<usize>();
    if name_count < 2 {
        return Ok(None);
    }
    let last_pc = entries.last().map_or(node.pc, |entry| entry.node.pc);
    if first_binding.start_pc < node.pc || first_binding.start_pc > last_pc + 2 {
        return Ok(None);
    }

    let mut names = Vec::with_capacity(name_count);
    let mut values = Vec::with_capacity(entries.len());
    let mut consumed = Vec::new();
    let mut refs = Vec::with_capacity(name_count);
    let mut local_indexes = Vec::with_capacity(name_count);

    for entry in entries {
        values.push(builder.expr_for_node(&entry.node)?);
        refs.extend(entry.refs);
        local_indexes.extend(entry.local_indexes);
        names.extend(entry.names);
        consumed.extend(entry.consumed);
    }

    while values
        .last()
        .is_some_and(|value| matches!(value, Expr::Nil))
    {
        values.pop();
    }

    for ((reference, name), index) in refs
        .into_iter()
        .zip(names.iter().cloned())
        .zip(local_indexes)
    {
        builder.mark_materialized(reference, name);
        builder.mark_local_declared(index);
    }

    Ok(Some(MultiEmit {
        stmt: Stmt::Local {
            names,
            attribs: Vec::new(),
            values,
        },
        consumed,
    }))
}

fn local_value_slot(
    builder: &StatementBuilder<'_>,
    id: NodeId,
    node: &SsaNode,
    base: u16,
    result_count: u16,
    start_pc: i32,
) -> Option<LocalValueSlot> {
    let mut refs = Vec::with_capacity(usize::from(result_count));
    let mut names = Vec::with_capacity(usize::from(result_count));
    let mut local_indexes = Vec::with_capacity(usize::from(result_count));
    for offset in 0..result_count {
        let reg = base.saturating_add(offset);
        let binding = builder.binding_for_def(reg, node.pc)?;
        if binding.start_pc != start_pc {
            return None;
        }
        let reference = builder
            .def_at_reg(id, reg)
            .unwrap_or_else(|| fallback_ref(node, reg));
        refs.push(reference);
        names.push(builder.name_for_binding_def(&binding, reference));
        local_indexes.push(binding.index);
    }
    Some(LocalValueSlot {
        node: node.clone(),
        refs,
        names,
        local_indexes,
        consumed: vec![id],
    })
}

fn is_call_setup(
    node_ids: &[NodeId],
    cursor: usize,
    expected: u16,
    builder: &StatementBuilder<'_>,
    skip: &dyn Fn(&SsaNode) -> bool,
) -> bool {
    let Some(current) = node_ids.get(cursor).and_then(|id| builder.node(*id)) else {
        return false;
    };
    let Some(current_reg) = current.dest.reg_index() else {
        return false;
    };
    if matches!(current.op, SsaOp::Call { .. }) || current_reg < expected {
        return false;
    }

    for id in node_ids.iter().copied().skip(cursor + 1) {
        let Some(next) = builder.node(id) else {
            return false;
        };
        if next.is_meta_only || skip(next) {
            continue;
        }
        if let SsaOp::Call {
            return_count,
            arg_count,
            ..
        } = next.op
            && next.dest.reg_index() == Some(expected)
            && return_count > 1
        {
            return call_setup_reg_in_frame(current_reg, expected, arg_count);
        }
        if let Some(next_reg) = next.dest.reg_index()
            && next_reg < expected
        {
            return false;
        }
        if !is_direct_value_node(next) {
            return false;
        }
    }
    false
}

fn call_setup_reg_in_frame(reg: u16, base: u16, arg_count: i32) -> bool {
    if reg < base {
        return false;
    }
    if arg_count == 0 {
        return true;
    }
    let Ok(arg_count) = u16::try_from(arg_count) else {
        return false;
    };
    reg < base.saturating_add(arg_count)
}

fn local_result_count(node: &SsaNode) -> u16 {
    match node.op {
        SsaOp::Call { return_count, .. } if return_count > 1 => {
            u16::try_from(return_count - 1).unwrap_or(u16::MAX)
        }
        _ => 1,
    }
}

fn fallback_ref(node: &SsaNode, reg: u16) -> SsaRef {
    SsaRef::Reg {
        reg,
        ver: node.dest.version().unwrap_or(0),
    }
}

struct LocalValueSlot {
    node: SsaNode,
    refs: Vec<SsaRef>,
    names: Vec<Name>,
    local_indexes: Vec<usize>,
    consumed: Vec<NodeId>,
}

impl LocalValueSlot {
    fn with_setup(mut self, setup: Vec<NodeId>) -> Self {
        self.consumed.splice(0..0, setup);
        self
    }
}

pub(crate) fn try_emit_swap(
    builder: &mut StatementBuilder<'_>,
    node_ids: &[NodeId],
    index: usize,
    node: &SsaNode,
    skip: &dyn Fn(&SsaNode) -> bool,
) -> Result<Option<MultiEmit>, LuaError> {
    if !matches!(&node.op, SsaOp::Move { .. }) {
        return Ok(None);
    }

    let mut saves = Vec::new();
    let mut cursor = index;
    while let Some(id) = node_ids.get(cursor).copied() {
        let Some(current) = builder.node(id) else {
            break;
        };
        if current.is_meta_only || skip(current) {
            break;
        }
        let SsaOp::Move { .. } = &current.op else {
            break;
        };
        let Some(dest_reg) = current.dest.reg_index() else {
            break;
        };
        if builder.binding_for_use(dest_reg, current.pc).is_some()
            || builder.binding_for_def(dest_reg, current.pc).is_some()
        {
            break;
        }
        let SsaOp::Move { src } = &current.op else {
            break;
        };
        saves.push(SaveTemp {
            id,
            dest: current.dest,
            src: *src,
            pc: current.pc,
        });
        cursor += 1;
    }

    if saves.is_empty() {
        return Ok(None);
    }

    let mut writes = Vec::new();
    while let Some(id) = node_ids.get(cursor).copied() {
        let Some(current) = builder.node(id) else {
            break;
        };
        if current.is_meta_only || skip(current) {
            break;
        }
        let SsaOp::Move { src } = &current.op else {
            break;
        };
        let Some(dest_reg) = current.dest.reg_index() else {
            break;
        };
        let Some(binding) = builder
            .binding_for_use(dest_reg, current.pc)
            .or_else(|| builder.binding_for_def(dest_reg, current.pc))
        else {
            break;
        };
        let name = builder.name_for_binding_def(&binding, current.dest);
        writes.push(WriteBack {
            id,
            dest: current.dest,
            name,
            src: *src,
            pc: current.pc,
        });
        cursor += 1;
    }

    if writes.len() < 2 || !uses_saved_temp(&saves, &writes) {
        return Ok(None);
    }

    let mut targets = Vec::with_capacity(writes.len());
    let mut values = Vec::with_capacity(writes.len());
    for write in writes.iter().rev() {
        targets.push(Expr::Name(write.name.clone()));
        let (src, pc) = saves
            .iter()
            .find_map(|save| (save.dest == write.src).then_some((save.src, save.pc)))
            .unwrap_or((write.src, write.pc));
        values.push(builder.expr_for_ref(src, pc)?);
    }

    for write in &writes {
        builder.mark_materialized(write.dest, write.name.clone());
    }

    let consumed = saves
        .into_iter()
        .map(|save| save.id)
        .chain(writes.into_iter().map(|write| write.id))
        .collect();

    Ok(Some(MultiEmit {
        stmt: Stmt::Assign { targets, values },
        consumed,
    }))
}

struct WriteBack {
    id: NodeId,
    dest: SsaRef,
    name: Name,
    src: SsaRef,
    pc: i32,
}

struct SaveTemp {
    id: NodeId,
    dest: SsaRef,
    src: SsaRef,
    pc: i32,
}

fn uses_saved_temp(saves: &[SaveTemp], writes: &[WriteBack]) -> bool {
    saves
        .iter()
        .any(|save| writes.iter().any(|write| write.src == save.dest))
}

fn is_direct_value_node(node: &SsaNode) -> bool {
    match &node.op {
        SsaOp::LoadK { .. }
        | SsaOp::LoadBool { .. }
        | SsaOp::LoadNil { .. }
        | SsaOp::GetGlobal { .. }
        | SsaOp::GetTable { .. }
        | SsaOp::GetUpval { .. }
        | SsaOp::Move { .. }
        | SsaOp::BinOp { .. }
        | SsaOp::UnOp { .. }
        | SsaOp::Concat { .. } => true,
        SsaOp::Call { return_count, .. } => *return_count > 1,
        _ => false,
    }
}
