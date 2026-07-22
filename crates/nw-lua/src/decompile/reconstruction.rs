//! Function-wide ownership and materialization decisions for AST reconstruction.

use std::collections::{BTreeMap, HashSet};

use crate::{
    bytecode::OpcodeTable,
    chunk::Proto,
    decompile::{
        analysis::{DecompileAnalysis, NodeId, ValueId},
        ast::Name,
        boolean::BooleanAnalysis,
        control_flow::RegionTree,
        expr_build::{ExprBuilder, is_inlineable_def},
        multi::{
            plan::{EmissionSchedule, NodeEmission},
            table_constructor::TableConstructorPlan,
        },
        naming::NameResolver,
    },
    ir::{SsaFunction, SsaNode, SsaOp, SsaRef},
};

mod regions;

use regions::{
    collect_control_blocks, collect_emittable_nodes, collect_forced_loop_values, emission_sequences,
};

/// The one reconstruction role assigned to an SSA definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueDisposition {
    Inline,
    Materialize,
    ConstructorMember,
    ControlOnly,
    Dead,
}

/// The source-level declaration role owned by a materialized SSA value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationDisposition {
    /// This value introduces its binding with a `local` declaration.
    Declare,
    /// The binding is already in scope and this value assigns it.
    Assign,
    /// The value is not materialized as a named binding.
    None,
}

impl ValueDisposition {
    #[must_use]
    pub const fn is_inline(self) -> bool {
        matches!(
            self,
            Self::Inline | Self::ConstructorMember | Self::ControlOnly
        )
    }
}

pub use crate::decompile::ast::BindingId;

/// Immutable decision for one versioned SSA register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedValue {
    pub disposition: ValueDisposition,
    pub declaration: DeclarationDisposition,
    pub binding: Option<BindingId>,
    pub name: Option<Name>,
    pub materialization_pc: i32,
}

/// Immutable ownership decisions discovered before AST construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructionPlan {
    values: Vec<Vec<Option<PlannedValue>>>,
    entry_declarations: Vec<Name>,
    inline_use_pcs: Vec<Vec<Vec<i32>>>,
    constructor_by_value: Vec<Vec<Option<SsaRef>>>,
    constructor_by_node: Vec<Vec<Option<SsaRef>>>,
    constructor_start_by_node: Vec<Vec<Option<NodeId>>>,
    constructor_plan_by_node: Vec<Vec<Option<TableConstructorPlan>>>,
    emissions: EmissionSchedule,
}

impl ReconstructionPlan {
    /// Build the per-function plan from SSA facts and the structured region tree.
    #[must_use]
    pub fn build(
        proto: &Proto,
        function: &SsaFunction,
        table: &OpcodeTable,
        analysis: &DecompileAnalysis,
        names: &NameResolver<'_>,
        booleans: &BooleanAnalysis,
        regions: Option<&RegionTree>,
    ) -> Self {
        let mut plan = Self::empty(function);
        plan.entry_declarations = names.implicit_entry_declarations();
        plan.discover_constructors(function, analysis);

        let mut forced = captured_binding_values(function, analysis, names);
        let mut control_blocks = booleans.expression_blocks().into_iter().collect();
        let mut emittable_nodes = HashSet::new();
        if let Some(regions) = regions {
            collect_forced_loop_values(&regions.root, function, &mut forced);
            collect_control_blocks(&regions.root, booleans, &mut control_blocks);
            collect_emittable_nodes(&regions.root, &mut emittable_nodes);
        } else {
            emittable_nodes.extend(function.blocks.iter().flat_map(|block| {
                (0..block.nodes.len()).map(|node| NodeId {
                    block: block.index,
                    node,
                })
            }));
        }

        let (decisions, inline_uses) = {
            let exprs = ExprBuilder::new(proto, function, table, analysis, names, booleans, &plan);
            let mut decisions = Vec::new();
            let mut inline_uses = Vec::new();
            for block in &function.blocks {
                for (node, item) in block.nodes.iter().enumerate() {
                    let id = NodeId {
                        block: block.index,
                        node,
                    };
                    for reference in analysis.defs_at(id).iter().copied() {
                        let disposition = classify_value(
                            function,
                            analysis,
                            names,
                            booleans,
                            &plan,
                            &exprs,
                            &forced,
                            &control_blocks,
                            &emittable_nodes,
                            id,
                            item,
                            reference,
                        );
                        let pc = materialization_pc(function, analysis, names, id, item, reference);
                        decisions.push((reference, disposition, pc));
                        inline_uses.push((
                            reference,
                            analysis
                                .real_uses(reference)
                                .iter()
                                .filter_map(|use_id| analysis.node(function, *use_id))
                                .filter(|use_node| exprs.can_inline_ref(reference, use_node.pc))
                                .map(|use_node| use_node.pc)
                                .collect::<Vec<_>>(),
                        ));
                    }
                }
            }
            retain_declaration_anchors(names, analysis, &mut decisions);
            (decisions, inline_uses)
        };

        for (reference, disposition, pc) in decisions {
            let binding = binding_id(names, reference, pc, disposition);
            plan.record_value(
                reference,
                PlannedValue {
                    disposition,
                    declaration: DeclarationDisposition::None,
                    binding,
                    name: planned_name(names, reference, pc, disposition),
                    materialization_pc: pc,
                },
            );
        }
        for (reference, use_pcs) in inline_uses {
            plan.record_inline_uses(reference, use_pcs);
        }
        let sequences = emission_sequences(regions, function);
        plan.emissions = EmissionSchedule::build(function, analysis, names, &plan, &sequences);
        plan.apply_emission_ownership(function, analysis, names);
        plan.replan_declarations(names);
        plan
    }

    /// Return the planned disposition for one SSA value.
    #[must_use]
    pub fn disposition(&self, reference: SsaRef) -> Option<ValueDisposition> {
        Some(self.value(reference)?.disposition)
    }

    /// Return binding identities whose declaration must be consumed by lowering.
    #[must_use]
    pub(crate) fn declaration_bindings(&self) -> HashSet<BindingId> {
        self.values
            .iter()
            .flat_map(|versions| versions.iter().flatten())
            .filter(|value| value.declaration == DeclarationDisposition::Declare)
            .filter_map(|value| value.binding.clone())
            .collect()
    }

    /// Return the emitted binding identity for one materialized value.
    #[must_use]
    pub fn binding(&self, reference: SsaRef) -> Option<BindingId> {
        self.value(reference)?.binding.clone()
    }

    /// Return the PC selected for declaration and debug-binding ownership.
    #[must_use]
    pub fn materialization_pc(&self, reference: SsaRef) -> Option<i32> {
        Some(self.value(reference)?.materialization_pc)
    }

    /// Return the chosen name of a materialized value.
    #[must_use]
    pub fn name(&self, reference: SsaRef) -> Option<Name> {
        self.value(reference)?.name.clone()
    }

    /// Return declarations encoded only by debug lifetimes at function entry.
    #[must_use]
    pub fn entry_declarations(&self) -> &[Name] {
        &self.entry_declarations
    }

    /// Return whether a value may be inlined at this use site.
    #[must_use]
    pub fn can_inline_at(&self, reference: SsaRef, use_pc: i32) -> Option<bool> {
        let disposition = self.disposition(reference)?;
        if disposition.is_inline() {
            return Some(true);
        }
        let value = ValueId::from_ref(reference)?;
        Some(
            self.inline_use_pcs
                .get(usize::from(value.reg))
                .and_then(|versions| versions.get(usize::try_from(value.ver).ok()?))
                .is_some_and(|pcs| pcs.contains(&use_pc)),
        )
    }

    /// Return the table constructor that owns evaluation of this SSA value.
    #[must_use]
    pub fn constructor_for_value(&self, reference: SsaRef) -> Option<SsaRef> {
        let value = ValueId::from_ref(reference)?;
        self.constructor_by_value
            .get(usize::from(value.reg))?
            .get(usize::try_from(value.ver).ok()?)
            .copied()
            .flatten()
    }

    /// Return the table constructor that owns evaluation of this SSA node.
    #[must_use]
    pub fn constructor_for_node(&self, id: NodeId) -> Option<SsaRef> {
        self.constructor_by_node
            .get(id.block)?
            .get(id.node)
            .copied()
            .flatten()
    }

    /// Return the constructor definition that owns this node.
    #[must_use]
    pub fn constructor_start_for_node(&self, id: NodeId) -> Option<NodeId> {
        self.constructor_start_by_node
            .get(id.block)?
            .get(id.node)
            .copied()
            .flatten()
    }

    /// Return the constructor plan beginning at this node.
    #[must_use]
    pub(crate) fn constructor_plan(&self, id: NodeId) -> Option<&TableConstructorPlan> {
        self.constructor_plan_by_node
            .get(id.block)?
            .get(id.node)?
            .as_ref()
    }

    /// Return the immutable statement-emission role assigned to this node.
    #[must_use]
    pub(crate) fn node_emission(&self, id: NodeId) -> &NodeEmission {
        self.emissions.get(id)
    }

    fn empty(function: &SsaFunction) -> Self {
        Self {
            values: vec![Vec::new(); function.num_regs],
            entry_declarations: Vec::new(),
            inline_use_pcs: vec![Vec::new(); function.num_regs],
            constructor_by_value: vec![Vec::new(); function.num_regs],
            constructor_by_node: function
                .blocks
                .iter()
                .map(|block| vec![None; block.nodes.len()])
                .collect(),
            constructor_start_by_node: function
                .blocks
                .iter()
                .map(|block| vec![None; block.nodes.len()])
                .collect(),
            constructor_plan_by_node: function
                .blocks
                .iter()
                .map(|block| vec![None; block.nodes.len()])
                .collect(),
            emissions: EmissionSchedule::empty(function),
        }
    }

    fn value(&self, reference: SsaRef) -> Option<&PlannedValue> {
        let value = ValueId::from_ref(reference)?;
        self.values
            .get(usize::from(value.reg))?
            .get(usize::try_from(value.ver).ok()?)?
            .as_ref()
    }

    fn discover_constructors(&mut self, function: &SsaFunction, analysis: &DecompileAnalysis) {
        for block in &function.blocks {
            for (node, item) in block.nodes.iter().enumerate() {
                let start = NodeId {
                    block: block.index,
                    node,
                };
                let Some(window) = TableConstructorPlan::recognize(function, analysis, start)
                else {
                    continue;
                };
                if let Some(slot) = self
                    .constructor_plan_by_node
                    .get_mut(start.block)
                    .and_then(|nodes| nodes.get_mut(start.node))
                {
                    *slot = Some(window.clone());
                }
                self.record_constructor_window(function, start, window.end(), item.dest);
            }
        }
    }

    fn record_constructor_window(
        &mut self,
        function: &SsaFunction,
        start: NodeId,
        end: NodeId,
        table: SsaRef,
    ) {
        if start.block != end.block {
            return;
        }
        let Some(block) = function.blocks.get(start.block) else {
            return;
        };
        for node in start.node..=end.node {
            let Some(item) = block.nodes.get(node) else {
                continue;
            };
            if let Some(slot) = self
                .constructor_by_node
                .get_mut(start.block)
                .and_then(|nodes| nodes.get_mut(node))
            {
                slot.get_or_insert(table);
            }
            if let Some(slot) = self
                .constructor_start_by_node
                .get_mut(start.block)
                .and_then(|nodes| nodes.get_mut(node))
            {
                slot.get_or_insert(start);
            }
            item.visit_defs(|reference| self.record_constructor_value(reference, table));
        }
    }

    fn record_constructor_value(&mut self, reference: SsaRef, table: SsaRef) {
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let slot = value_slot(&mut self.constructor_by_value, value);
        slot.get_or_insert(table);
    }

    fn record_value(&mut self, reference: SsaRef, value: PlannedValue) {
        let Some(id) = ValueId::from_ref(reference) else {
            return;
        };
        *value_slot(&mut self.values, id) = Some(value);
    }

    fn record_inline_uses(&mut self, reference: SsaRef, use_pcs: Vec<i32>) {
        let Some(id) = ValueId::from_ref(reference) else {
            return;
        };
        *value_slot(&mut self.inline_use_pcs, id) = use_pcs;
    }

    fn apply_emission_ownership(
        &mut self,
        function: &SsaFunction,
        analysis: &DecompileAnalysis,
        names: &NameResolver<'_>,
    ) {
        let mut owned = self.emissions.owned_materializations();
        for block in &function.blocks {
            for (node, item) in block.nodes.iter().enumerate() {
                let id = NodeId {
                    block: block.index,
                    node,
                };
                if !matches!(self.emissions.get(id), NodeEmission::Standalone)
                    || !is_fixed_result_statement(item)
                {
                    continue;
                }
                owned.extend(
                    analysis
                        .defs_at(id)
                        .iter()
                        .copied()
                        .map(|reference| (id, reference)),
                );
            }
        }
        for (id, reference) in owned {
            let Some(node) = analysis.node(function, id) else {
                continue;
            };
            let pc = node.pc;
            self.record_value(
                reference,
                PlannedValue {
                    disposition: ValueDisposition::Materialize,
                    declaration: DeclarationDisposition::None,
                    binding: binding_id(names, reference, pc, ValueDisposition::Materialize),
                    name: planned_name(names, reference, pc, ValueDisposition::Materialize),
                    materialization_pc: pc,
                },
            );
        }
    }

    fn replan_declarations(&mut self, names: &NameResolver<'_>) {
        let declared_bindings = names
            .initially_declared_locals()
            .into_iter()
            .map(|index| names.debug_binding(index))
            .chain(
                self.entry_declarations
                    .iter()
                    .filter_map(|name| name.binding().cloned()),
            )
            .collect::<HashSet<_>>();
        for value in self
            .values
            .iter_mut()
            .flat_map(|versions| versions.iter_mut().flatten())
        {
            value.declaration = match value.binding.as_ref() {
                Some(binding) if value.disposition == ValueDisposition::Materialize => {
                    if declared_bindings.contains(binding) {
                        DeclarationDisposition::Assign
                    } else {
                        DeclarationDisposition::Declare
                    }
                }
                _ => DeclarationDisposition::None,
            };
        }
    }
}

fn captured_binding_values(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
) -> HashSet<SsaRef> {
    let definitions = function
        .blocks
        .iter()
        .flat_map(|block| {
            (0..block.nodes.len()).flat_map(move |node| {
                analysis
                    .defs_at(NodeId {
                        block: block.index,
                        node,
                    })
                    .iter()
                    .copied()
            })
        })
        .collect::<Vec<_>>();
    let captured = definitions
        .iter()
        .copied()
        .filter(|reference| analysis.facts(*reference).upvalue_captures > 0)
        .filter_map(|reference| names.binding_id_for_ref(reference))
        .collect::<HashSet<_>>();

    definitions
        .iter()
        .copied()
        .filter(|reference| {
            names
                .binding_id_for_ref(*reference)
                .is_some_and(|binding| captured.contains(&binding))
        })
        .collect()
}

fn is_fixed_result_statement(node: &SsaNode) -> bool {
    matches!(
        &node.op,
        SsaOp::Call { return_count, .. } if *return_count > 2
    ) || matches!(&node.op, SsaOp::VarArg { count, .. } if *count >= 3)
}

fn retain_declaration_anchors(
    names: &NameResolver<'_>,
    analysis: &DecompileAnalysis,
    decisions: &mut [(SsaRef, ValueDisposition, i32)],
) {
    let live_bindings = decisions
        .iter()
        .filter(|(reference, _, _)| analysis.use_count(*reference) > 0)
        .filter_map(|(reference, _, _)| names.binding_id_for_ref(*reference))
        .filter(|binding| binding.is_debug_local())
        .collect::<HashSet<_>>();
    let mut first_definition = BTreeMap::new();
    for (index, (reference, _, pc)) in decisions.iter().enumerate() {
        let Some(binding) = names.binding_id_for_ref(*reference) else {
            continue;
        };
        if !live_bindings.contains(&binding) {
            continue;
        }
        first_definition
            .entry(binding)
            .and_modify(|(first_pc, first_index)| {
                if *pc < *first_pc {
                    *first_pc = *pc;
                    *first_index = index;
                }
            })
            .or_insert((*pc, index));
    }
    for (_, index) in first_definition.into_values() {
        if decisions[index].1 == ValueDisposition::Dead {
            decisions[index].1 = ValueDisposition::Materialize;
        }
    }
}

fn value_slot<T: Default>(slots: &mut Vec<Vec<T>>, value: ValueId) -> &mut T {
    let reg = usize::from(value.reg);
    let version = usize::try_from(value.ver).unwrap_or(usize::MAX);
    if reg >= slots.len() {
        slots.resize_with(reg + 1, Vec::new);
    }
    if version >= slots[reg].len() {
        slots[reg].resize_with(version.saturating_add(1), T::default);
    }
    &mut slots[reg][version]
}

#[allow(clippy::too_many_arguments)]
fn classify_value(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    booleans: &BooleanAnalysis,
    plan: &ReconstructionPlan,
    exprs: &ExprBuilder<'_>,
    forced: &HashSet<SsaRef>,
    control_blocks: &HashSet<usize>,
    emittable_nodes: &HashSet<NodeId>,
    id: NodeId,
    node: &SsaNode,
    reference: SsaRef,
) -> ValueDisposition {
    if self_op_consumed_by_call(function, analysis, node, reference) {
        return ValueDisposition::ControlOnly;
    }
    if forced.contains(&reference)
        || analysis.facts(reference).upvalue_captures > 0
        || closure_captures_destination(node, reference)
        || (matches!(&node.op, SsaOp::Closure { .. }) && names.has_debug_binding(reference))
    {
        return ValueDisposition::Materialize;
    }
    if reference != node.dest {
        return if reference
            .reg_index()
            .and_then(|reg| names.binding_for_def(reg, node.pc))
            .is_some()
        {
            ValueDisposition::Materialize
        } else if analysis.real_use_count(reference) == 0 {
            ValueDisposition::Dead
        } else {
            match node.op {
                SsaOp::LoadNil { .. } | SsaOp::SelfOp { .. } => ValueDisposition::Inline,
                SsaOp::ForLoop { .. } | SsaOp::TForLoop { .. } => ValueDisposition::ControlOnly,
                _ => ValueDisposition::Materialize,
            }
        };
    }

    if booleans.value_for_phi(reference).is_some() {
        let named = names.has_debug_binding(reference);
        return if named || analysis.real_use_count(reference) > 1 {
            ValueDisposition::Materialize
        } else {
            ValueDisposition::Inline
        };
    }

    if analysis.use_count(reference) == 0 && !node.op.effects().is_observable() {
        return ValueDisposition::Dead;
    }

    if !emittable_nodes.contains(&id) && is_inlineable_def(&node.op) {
        return ValueDisposition::ControlOnly;
    }

    if control_blocks.contains(&id.block)
        && is_inlineable_def(&node.op)
        && !exprs.is_stable_named_def(node)
        && analysis
            .real_uses(reference)
            .iter()
            .all(|use_id| control_blocks.contains(&use_id.block))
    {
        return ValueDisposition::ControlOnly;
    }

    let pc = materialization_pc(function, analysis, names, id, node, reference);
    let can_inline = exprs.can_inline_ref(reference, node.pc);
    if let Some(reg) = reference.reg_index()
        && let Some(binding) = names.binding_for_def(reg, pc)
        && !analysis.has_later_def_before(reg, node.pc, binding.start_pc)
        && !can_inline
    {
        return ValueDisposition::Materialize;
    }
    if value_plan_consumes_constructor_def(function, analysis, booleans, id, node) {
        return ValueDisposition::ConstructorMember;
    }
    if value_plan_consumes_def(function, analysis, booleans, id, node) {
        return ValueDisposition::ControlOnly;
    }
    if plan
        .constructor_for_value(reference)
        .is_some_and(|owner| owner != reference)
    {
        return ValueDisposition::ConstructorMember;
    }

    let uses = analysis.real_use_count(reference);
    if uses == 0 {
        return ValueDisposition::Dead;
    }
    if can_inline {
        ValueDisposition::Inline
    } else {
        ValueDisposition::Materialize
    }
}

fn closure_captures_destination(node: &SsaNode, reference: SsaRef) -> bool {
    let Some(dest_reg) = reference.reg_index() else {
        return false;
    };
    matches!(
        &node.op,
        SsaOp::Closure { upvalues, .. }
            if upvalues.iter().any(|capture| matches!(
                capture,
                crate::ir::UpvalueCapture::ParentLocal(captured)
                    if captured.reg_index() == Some(dest_reg)
            ))
    )
}

fn binding_id(
    names: &NameResolver<'_>,
    reference: SsaRef,
    pc: i32,
    disposition: ValueDisposition,
) -> Option<BindingId> {
    if disposition != ValueDisposition::Materialize {
        return None;
    }
    if let Some(binding) = reference
        .reg_index()
        .and_then(|reg| names.binding_for_def(reg, pc))
    {
        return Some(names.debug_binding(binding.index));
    }
    names.binding_id_for_ref(reference)
}

fn planned_name(
    names: &NameResolver<'_>,
    reference: SsaRef,
    pc: i32,
    disposition: ValueDisposition,
) -> Option<Name> {
    if disposition != ValueDisposition::Materialize {
        return None;
    }
    Some(
        reference
            .reg_index()
            .and_then(|reg| names.binding_for_def(reg, pc))
            .map_or_else(
                || names.name_for_ref(reference, pc),
                |binding| names.name_for_binding_def(&binding, reference),
            ),
    )
}

fn self_op_consumed_by_call(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    node: &SsaNode,
    reference: SsaRef,
) -> bool {
    if reference != node.dest || !matches!(node.op, SsaOp::SelfOp { .. }) {
        return false;
    }
    let [use_id] = analysis.real_uses(reference) else {
        return false;
    };
    analysis.node(function, *use_id).is_some_and(|use_node| {
        matches!(
            use_node.op,
            SsaOp::Call { func, .. } | SsaOp::TailCall { func, .. } if func == reference
        )
    })
}

fn value_plan_consumes_def(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    booleans: &BooleanAnalysis,
    id: NodeId,
    node: &SsaNode,
) -> bool {
    if !matches!(node.dest, SsaRef::Reg { .. }) || !is_inlineable_def(&node.op) {
        return false;
    }
    let Some(plan) = booleans
        .value_select_start(id.block)
        .or_else(|| booleans.value_select_covering(id.block))
    else {
        return false;
    };
    if node.dest == plan.dest || !plan.consumed_blocks().contains(&id.block) {
        return false;
    }
    let facts = analysis.facts(node.dest);
    if facts.uses == 0 || facts.upvalue_captures > 0 || facts.mutating_table_uses > 0 {
        return false;
    }
    let Some(branch) = function.blocks.get(plan.start).and_then(|block| {
        block
            .nodes
            .iter()
            .position(|candidate| matches!(candidate.op, SsaOp::Branch { .. }))
    }) else {
        return false;
    };
    analysis.real_uses(node.dest).iter().all(|use_id| {
        plan.consumed_blocks().contains(&use_id.block)
            && (use_id.block != plan.start || use_id.node >= branch)
    })
}

fn value_plan_consumes_constructor_def(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    booleans: &BooleanAnalysis,
    id: NodeId,
    node: &SsaNode,
) -> bool {
    if !matches!(node.op, SsaOp::NewTable { .. }) {
        return false;
    }
    let Some(plan) = booleans
        .value_select_start(id.block)
        .or_else(|| booleans.value_select_covering(id.block))
    else {
        return false;
    };
    if node.dest == plan.dest || !plan.consumed_blocks().contains(&id.block) {
        return false;
    }
    let Some(table_reg) = node.dest.reg_index() else {
        return false;
    };
    let uses = analysis.real_uses(node.dest);
    !uses.is_empty()
        && uses.iter().all(|use_id| {
            let Some(use_node) = analysis.node(function, *use_id) else {
                return false;
            };
            if plan.consumed_blocks().contains(&use_id.block) {
                return crate::decompile::multi::table_constructor::is_matching_settable(
                    use_node, node.dest, table_reg,
                ) || crate::decompile::multi::table_constructor::is_matching_setlist(
                    use_node, node.dest, table_reg,
                );
            }
            use_id.block == plan.merge
                && matches!(use_node.op, SsaOp::Phi { .. })
                && use_node.dest == plan.dest
        })
}

fn materialization_pc(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    _id: NodeId,
    node: &SsaNode,
    reference: SsaRef,
) -> i32 {
    if let SsaOp::Closure { upvalues, .. } = &node.op {
        return node
            .pc
            .saturating_add(i32::try_from(upvalues.len()).unwrap_or(i32::MAX));
    }
    if reference != node.dest || !matches!(node.op, SsaOp::NewTable { .. }) {
        return node.pc;
    }
    let Some(table_reg) = reference.reg_index() else {
        return node.pc;
    };
    let pc = analysis
        .real_uses(reference)
        .iter()
        .filter_map(|id| analysis.node(function, *id))
        .filter(|use_node| {
            crate::decompile::multi::table_constructor::is_matching_settable(
                use_node, reference, table_reg,
            ) || crate::decompile::multi::table_constructor::is_matching_setlist(
                use_node, reference, table_reg,
            )
        })
        .map(|use_node| use_node.pc)
        .max()
        .unwrap_or(node.pc);
    if let Some(binding) = names.binding_for_def(table_reg, pc)
        && analysis.has_later_def_before(table_reg, node.pc, binding.start_pc)
    {
        node.pc
    } else {
        pc
    }
}
