//! Function-wide planning for source statements that own several SSA nodes.

use crate::{
    decompile::{
        analysis::{DecompileAnalysis, NodeId},
        ast::{BindingId, Name},
        naming::NameResolver,
        reconstruction::{ReconstructionPlan, ValueDisposition},
    },
    ir::{SsaFunction, SsaNode, SsaOp, SsaRef},
};

use super::table_constructor::TableConstructorPlan;

/// The immutable source-emission role assigned to one SSA node.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum NodeEmission {
    /// The structured region tree owns the node without emitting it directly.
    #[default]
    Omitted,
    /// The node lowers independently through `StatementBuilder::emit_node`.
    Standalone,
    /// This node owns one source statement reconstructed from several nodes.
    Owner(MultiNodePlan),
    /// This node is evaluated by the named owner and emits no statement itself.
    Member { owner: NodeId },
}

/// A complete multi-node source statement selected before AST construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MultiNodePlan {
    TableConstructor(TableConstructorEmission),
    CallTransfer(CallTransferPlan),
    Swap(SwapPlan),
    MultiLocal(MultiLocalPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallTransferPlan {
    pub call: NodeId,
    pub writes: Vec<ResultWrite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResultWrite {
    pub id: NodeId,
    pub dest: SsaRef,
    pub name: Name,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableConstructorEmission {
    pub constructor: TableConstructorPlan,
    pub members: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SwapPlan {
    pub saves: Vec<SavedValue>,
    pub writes: Vec<NamedWrite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SavedValue {
    pub id: NodeId,
    pub dest: SsaRef,
    pub src: SsaRef,
    pub pc: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedWrite {
    pub id: NodeId,
    pub dest: SsaRef,
    pub name: Name,
    pub src: SsaRef,
    pub pc: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MultiLocalPlan {
    pub leading_nils: Vec<LocalDeclarationPlan>,
    pub entries: Vec<LocalValuePlan>,
    pub members: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalDeclarationPlan {
    pub reference: SsaRef,
    pub name: Name,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalValuePlan {
    pub node: NodeId,
    pub refs: Vec<SsaRef>,
    pub names: Vec<Name>,
}

/// Dense per-node schedule consumed monotonically by statement lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EmissionSchedule {
    nodes: Vec<Vec<NodeEmission>>,
}

impl EmissionSchedule {
    #[must_use]
    pub(crate) fn empty(function: &SsaFunction) -> Self {
        Self {
            nodes: function
                .blocks
                .iter()
                .map(|block| vec![NodeEmission::Omitted; block.nodes.len()])
                .collect(),
        }
    }

    #[must_use]
    pub(crate) fn build(
        function: &SsaFunction,
        analysis: &DecompileAnalysis,
        names: &NameResolver<'_>,
        reconstruction: &ReconstructionPlan,
        sequences: &[Vec<NodeId>],
    ) -> Self {
        let mut schedule = Self::empty(function);
        for id in sequences.iter().flat_map(|nodes| nodes.iter()).copied() {
            schedule.set(id, NodeEmission::Standalone);
        }
        for nodes in sequences {
            let mut index = 0;
            while index < nodes.len() {
                let owner = nodes[index];
                if !matches!(schedule.get(owner), NodeEmission::Standalone) {
                    index += 1;
                    continue;
                }
                let planned =
                    plan_table_constructor(function, analysis, reconstruction, nodes, index)
                        .or_else(|| plan_call_transfer(function, analysis, names, nodes, index))
                        .or_else(|| plan_swap(function, names, nodes, index))
                        .or_else(|| plan_multi_local(function, analysis, names, nodes, index));
                let Some((plan, members)) = planned else {
                    index += 1;
                    continue;
                };
                if !members
                    .iter()
                    .all(|id| matches!(schedule.get(*id), NodeEmission::Standalone))
                {
                    index += 1;
                    continue;
                }
                schedule.set(owner, NodeEmission::Owner(plan));
                for member in members.iter().copied().filter(|id| *id != owner) {
                    schedule.set(member, NodeEmission::Member { owner });
                }
                index += members.len().max(1);
            }
        }
        schedule
    }

    #[must_use]
    pub(crate) fn get(&self, id: NodeId) -> &NodeEmission {
        self.nodes
            .get(id.block)
            .and_then(|nodes| nodes.get(id.node))
            .unwrap_or(&NodeEmission::Omitted)
    }

    #[must_use]
    pub(crate) fn owned_materializations(&self) -> Vec<(NodeId, SsaRef)> {
        self.nodes
            .iter()
            .flat_map(|nodes| nodes.iter())
            .flat_map(|emission| match emission {
                NodeEmission::Owner(MultiNodePlan::CallTransfer(plan)) => plan
                    .writes
                    .iter()
                    .map(|write| (write.id, write.dest))
                    .collect::<Vec<_>>(),
                NodeEmission::Owner(MultiNodePlan::MultiLocal(plan)) => plan
                    .leading_nils
                    .iter()
                    .map(|local| (plan.entries[0].node, local.reference))
                    .chain(plan.entries.iter().flat_map(|entry| {
                        entry
                            .refs
                            .iter()
                            .copied()
                            .map(|reference| (entry.node, reference))
                    }))
                    .collect(),
                _ => Vec::new(),
            })
            .collect()
    }

    fn set(&mut self, id: NodeId, emission: NodeEmission) {
        if let Some(slot) = self
            .nodes
            .get_mut(id.block)
            .and_then(|nodes| nodes.get_mut(id.node))
        {
            *slot = emission;
        }
    }
}

fn plan_call_transfer(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    nodes: &[NodeId],
    index: usize,
) -> Option<(MultiNodePlan, Vec<NodeId>)> {
    let call_id = *nodes.get(index)?;
    let call = node(function, call_id)?;
    let SsaOp::Call { return_count, .. } = &call.op else {
        return None;
    };
    let result_count = usize::try_from(return_count.checked_sub(1)?).ok()?;
    if result_count < 2 {
        return None;
    }
    let base = call.dest.reg_index()?;
    let result_refs = (0..result_count)
        .map(|offset| {
            let reg = base.saturating_add(u16::try_from(offset).ok()?);
            analysis.def_at_reg(call_id, reg)
        })
        .collect::<Option<Vec<_>>>()?;

    let mut writes = Vec::with_capacity(result_count);
    for id in nodes.iter().copied().skip(index + 1).take(result_count) {
        let move_node = node(function, id)?;
        let SsaOp::Move { src } = &move_node.op else {
            return None;
        };
        let result_index = result_refs.iter().position(|reference| reference == src)?;
        if writes
            .iter()
            .any(|(existing, _): &(usize, ResultWrite)| *existing == result_index)
        {
            return None;
        }
        let dest_reg = move_node.dest.reg_index()?;
        let name = names.binding_for_def(dest_reg, move_node.pc).map_or_else(
            || names.name_for_ref(move_node.dest, move_node.pc),
            |binding| names.name_for_binding_def(&binding, move_node.dest),
        );
        writes.push((
            result_index,
            ResultWrite {
                id,
                dest: move_node.dest,
                name,
            },
        ));
    }
    if writes.len() != result_count {
        return None;
    }
    writes.sort_by_key(|(result, _)| *result);
    let writes = writes
        .into_iter()
        .map(|(_, write)| write)
        .collect::<Vec<_>>();
    let members = std::iter::once(call_id)
        .chain(writes.iter().map(|write| write.id))
        .collect::<Vec<_>>();
    Some((
        MultiNodePlan::CallTransfer(CallTransferPlan {
            call: call_id,
            writes,
        }),
        members,
    ))
}

fn plan_table_constructor(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    reconstruction: &ReconstructionPlan,
    nodes: &[NodeId],
    index: usize,
) -> Option<(MultiNodePlan, Vec<NodeId>)> {
    let start = *nodes.get(index)?;
    let node = analysis.node(function, start)?;
    if !matches!(&node.op, SsaOp::NewTable { .. })
        || reconstruction.disposition(node.dest) != Some(ValueDisposition::Materialize)
    {
        return None;
    }
    let constructor = reconstruction.constructor_plan(start)?.clone();
    if constructor.mutation_count() == 0 {
        return None;
    }
    let members = node_range(start, constructor.end())?;
    if nodes.get(index..index + members.len())? != members {
        return None;
    }
    if fields_require_materialized_setup(function, analysis, reconstruction, members.as_slice()) {
        return None;
    }
    Some((
        MultiNodePlan::TableConstructor(TableConstructorEmission {
            constructor,
            members: members.clone(),
        }),
        members,
    ))
}

fn fields_require_materialized_setup(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    reconstruction: &ReconstructionPlan,
    members: &[NodeId],
) -> bool {
    members.iter().copied().any(|id| {
        let Some(node) = analysis.node(function, id) else {
            return false;
        };
        if is_constructor_mutation(node) || !matches!(node.dest, SsaRef::Reg { .. }) {
            return false;
        }
        if reconstruction.constructor_for_value(node.dest) == Some(node.dest) {
            return false;
        }
        analysis.real_uses(node.dest).iter().any(|use_id| {
            members.contains(use_id)
                && analysis.node(function, *use_id).is_some_and(|use_node| {
                    !reconstruction
                        .can_inline_at(node.dest, use_node.pc)
                        .unwrap_or(false)
                })
        })
    })
}

fn plan_swap(
    function: &SsaFunction,
    names: &NameResolver<'_>,
    nodes: &[NodeId],
    index: usize,
) -> Option<(MultiNodePlan, Vec<NodeId>)> {
    let mut saves = Vec::new();
    let mut cursor = index;
    while let Some(id) = nodes.get(cursor).copied() {
        let current = node(function, id)?;
        let SsaOp::Move { src } = &current.op else {
            break;
        };
        let dest_reg = current.dest.reg_index()?;
        if names.binding_for_use(dest_reg, current.pc).is_some()
            || names.binding_for_def(dest_reg, current.pc).is_some()
        {
            break;
        }
        saves.push(SavedValue {
            id,
            dest: current.dest,
            src: *src,
            pc: current.pc,
        });
        cursor += 1;
    }
    if saves.is_empty() {
        return None;
    }

    let mut writes = Vec::new();
    while let Some(id) = nodes.get(cursor).copied() {
        let current = node(function, id)?;
        let SsaOp::Move { src } = &current.op else {
            break;
        };
        let dest_reg = current.dest.reg_index()?;
        let binding = names
            .binding_for_use(dest_reg, current.pc)
            .or_else(|| names.binding_for_def(dest_reg, current.pc))?;
        writes.push(NamedWrite {
            id,
            dest: current.dest,
            name: names.name_for_binding_def(&binding, current.dest),
            src: *src,
            pc: current.pc,
        });
        cursor += 1;
    }
    if writes.len() < 2
        || !saves
            .iter()
            .any(|save| writes.iter().any(|write| write.src == save.dest))
    {
        return None;
    }
    let members = saves
        .iter()
        .map(|save| save.id)
        .chain(writes.iter().map(|write| write.id))
        .collect::<Vec<_>>();
    Some((MultiNodePlan::Swap(SwapPlan { saves, writes }), members))
}

fn plan_multi_local(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    nodes: &[NodeId],
    index: usize,
) -> Option<(MultiNodePlan, Vec<NodeId>)> {
    let first_id = *nodes.get(index)?;
    let first = node(function, first_id)?;
    if !is_direct_value_node(first) {
        return None;
    }
    let base = first.dest.reg_index()?;
    let first_binding = names
        .binding_for_def(base, first.pc)
        .or_else(|| names.debug_binding_for_ref(first.dest))?;
    if has_dominating_binding_def(
        function,
        analysis,
        names,
        first_id,
        &names.debug_binding(first_binding.index),
    ) {
        return None;
    }
    let leading_nils = names
        .implicit_nil_prefix(&first_binding)
        .into_iter()
        .filter(|(_, name)| {
            name.binding().is_none_or(|binding| {
                !has_dominating_binding_def(function, analysis, names, first_id, binding)
            })
        })
        .map(|(reference, name)| LocalDeclarationPlan { reference, name })
        .collect::<Vec<_>>();

    let mut entries = Vec::new();
    let mut members = Vec::new();
    let mut expected = base;
    let mut cursor = index;
    let mut pending_setup = Vec::new();
    while let Some(id) = nodes.get(cursor).copied() {
        let current = node(function, id)?;
        if !is_direct_value_node(current) {
            break;
        }
        if is_call_setup(function, nodes, cursor, expected) {
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
        let Some(entry) = local_value_plan(
            analysis,
            names,
            id,
            current,
            expected,
            result_count,
            first_binding.start_pc,
        ) else {
            break;
        };
        members.append(&mut pending_setup);
        members.push(id);
        entries.push(entry);
        expected = expected.saturating_add(result_count);
        cursor += 1;
        if result_count > 1 {
            break;
        }
    }

    let name_count =
        leading_nils.len() + entries.iter().map(|entry| entry.names.len()).sum::<usize>();
    if name_count < 2 {
        return None;
    }
    let last_pc = entries
        .last()
        .and_then(|entry| node(function, entry.node))
        .map_or(first.pc, |node| node.pc);
    if first_binding.start_pc < first.pc || first_binding.start_pc > last_pc + 2 {
        return None;
    }

    Some((
        MultiNodePlan::MultiLocal(MultiLocalPlan {
            leading_nils,
            entries,
            members: members.clone(),
        }),
        members,
    ))
}

fn has_dominating_binding_def(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    current: NodeId,
    binding: &BindingId,
) -> bool {
    function.blocks.iter().any(|block| {
        block.nodes.iter().enumerate().any(|(node_index, _)| {
            let candidate = NodeId {
                block: block.index,
                node: node_index,
            };
            node_precedes(function, candidate, current)
                && analysis
                    .defs_at(candidate)
                    .iter()
                    .copied()
                    .any(|reference| names.binding_id_for_ref(reference).as_ref() == Some(binding))
        })
    })
}

fn node_precedes(function: &SsaFunction, candidate: NodeId, current: NodeId) -> bool {
    if candidate.block == current.block {
        return candidate.node < current.node;
    }
    function.dom.dominates(candidate.block, current.block)
}

fn local_value_plan(
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    id: NodeId,
    node: &SsaNode,
    base: u16,
    result_count: u16,
    start_pc: i32,
) -> Option<LocalValuePlan> {
    let mut refs = Vec::with_capacity(usize::from(result_count));
    let mut local_names = Vec::with_capacity(usize::from(result_count));
    for offset in 0..result_count {
        let reg = base.saturating_add(offset);
        let reference = analysis
            .def_at_reg(id, reg)
            .unwrap_or_else(|| fallback_ref(node, reg));
        let binding = names
            .binding_for_def(reg, node.pc)
            .or_else(|| names.debug_binding_for_ref(reference))?;
        if binding.start_pc != start_pc || names.is_declared_at_entry(binding.index) {
            return None;
        }
        refs.push(reference);
        local_names.push(names.name_for_binding_def(&binding, reference));
    }
    Some(LocalValuePlan {
        node: id,
        refs,
        names: local_names,
    })
}

fn is_call_setup(function: &SsaFunction, nodes: &[NodeId], cursor: usize, expected: u16) -> bool {
    let Some(current) = nodes.get(cursor).and_then(|id| node(function, *id)) else {
        return false;
    };
    let Some(current_reg) = current.dest.reg_index() else {
        return false;
    };
    if matches!(&current.op, SsaOp::Call { .. }) || current_reg < expected {
        return false;
    }

    for id in nodes.iter().copied().skip(cursor + 1) {
        let Some(next) = node(function, id) else {
            return false;
        };
        if let SsaOp::Call {
            return_count,
            arg_count,
            ..
        } = &next.op
            && next.dest.reg_index() == Some(expected)
            && *return_count > 1
        {
            return call_setup_reg_in_frame(current_reg, expected, *arg_count);
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
    match &node.op {
        SsaOp::Call { return_count, .. } if *return_count > 1 => {
            u16::try_from(*return_count - 1).unwrap_or(u16::MAX)
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

fn node_range(start: NodeId, end: NodeId) -> Option<Vec<NodeId>> {
    (start.block == end.block && start.node <= end.node).then(|| {
        (start.node..=end.node)
            .map(|node| NodeId {
                block: start.block,
                node,
            })
            .collect()
    })
}

fn node(function: &SsaFunction, id: NodeId) -> Option<&SsaNode> {
    function
        .blocks
        .get(id.block)
        .and_then(|block| block.nodes.get(id.node))
}

fn is_constructor_mutation(node: &SsaNode) -> bool {
    matches!(&node.op, SsaOp::SetTable { .. } | SsaOp::SetList { .. })
}

fn is_direct_value_node(node: &SsaNode) -> bool {
    match &node.op {
        SsaOp::LoadK { .. }
        | SsaOp::LoadLiteral { .. }
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
