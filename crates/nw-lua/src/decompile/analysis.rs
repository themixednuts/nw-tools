//! One-time SSA facts used by Phase 4 reconstruction.

use crate::ir::{SsaFunction, SsaNode, SsaRef, UseRole};

/// Stable identifier for a node inside an [`SsaFunction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId {
    pub block: usize,
    pub node: usize,
}

/// Stable identifier for a versioned register value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId {
    pub reg: u16,
    pub ver: u32,
}

impl ValueId {
    #[must_use]
    pub const fn from_ref(reference: SsaRef) -> Option<Self> {
        match reference {
            SsaRef::Reg { reg, ver } => Some(Self { reg, ver }),
            SsaRef::None | SsaRef::Const(_) => None,
        }
    }
}

/// Facts for one SSA value slot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ValueFacts {
    pub uses: usize,
    pub phi_uses: usize,
    pub mutating_table_uses: usize,
    pub upvalue_captures: usize,
}

/// Use-count and definition-site facts computed once for a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecompileAnalysis {
    facts: Vec<Vec<ValueFacts>>,
    defs: Vec<Vec<Option<NodeId>>>,
    real_uses: Vec<Vec<Vec<NodeId>>>,
    defs_by_node: Vec<Vec<Vec<SsaRef>>>,
    def_pcs_by_reg: Vec<Vec<i32>>,
    side_effect_prefix_by_block: Vec<Vec<usize>>,
}

impl DecompileAnalysis {
    /// Return the defining node for a versioned register.
    #[must_use]
    pub fn def_site(&self, reference: SsaRef) -> Option<NodeId> {
        let value = ValueId::from_ref(reference)?;
        self.defs
            .get(usize::from(value.reg))
            .and_then(|versions| versions.get(usize::try_from(value.ver).ok()?))
            .copied()
            .flatten()
    }

    /// Return facts for a versioned register.
    #[must_use]
    pub fn facts(&self, reference: SsaRef) -> ValueFacts {
        let Some(value) = ValueId::from_ref(reference) else {
            return ValueFacts::default();
        };
        self.facts
            .get(usize::from(value.reg))
            .and_then(|versions| versions.get(usize::try_from(value.ver).ok()?))
            .copied()
            .unwrap_or_default()
    }

    /// Return total SSA use-count, including phi operands.
    #[must_use]
    pub fn use_count(&self, reference: SsaRef) -> usize {
        self.facts(reference).uses
    }

    /// Return non-phi use-count.
    #[must_use]
    pub fn real_use_count(&self, reference: SsaRef) -> usize {
        let facts = self.facts(reference);
        facts.uses.saturating_sub(facts.phi_uses)
    }

    /// Return non-phi use sites for a versioned register.
    #[must_use]
    pub fn real_uses(&self, reference: SsaRef) -> &[NodeId] {
        let Some(value) = ValueId::from_ref(reference) else {
            return &[];
        };
        self.real_uses
            .get(usize::from(value.reg))
            .and_then(|versions| versions.get(usize::try_from(value.ver).ok()?))
            .map_or(&[], Vec::as_slice)
    }

    /// Return the only non-phi use site for a versioned register.
    #[must_use]
    pub fn single_real_use(&self, reference: SsaRef) -> Option<NodeId> {
        let uses = self.real_uses(reference);
        (uses.len() == 1).then_some(uses[0])
    }

    /// Return whether this value is used as the table being mutated.
    #[must_use]
    pub fn has_mutating_table_use(&self, reference: SsaRef) -> bool {
        self.facts(reference).mutating_table_uses > 0
    }

    /// Return a node by id.
    #[must_use]
    pub fn node<'a>(&self, function: &'a SsaFunction, id: NodeId) -> Option<&'a SsaNode> {
        function
            .blocks
            .get(id.block)
            .and_then(|block| block.nodes.get(id.node))
    }

    /// Return all SSA refs whose definition site is this node.
    #[must_use]
    pub fn defs_at(&self, id: NodeId) -> &[SsaRef] {
        self.defs_by_node
            .get(id.block)
            .and_then(|block| block.get(id.node))
            .map_or(&[], Vec::as_slice)
    }

    /// Return the SSA ref for a register defined by this node.
    #[must_use]
    pub fn def_at_reg(&self, id: NodeId, reg: u16) -> Option<SsaRef> {
        self.defs_at(id)
            .iter()
            .copied()
            .find(|reference| reference.reg_index() == Some(reg))
    }

    /// Return whether a register is redefined after one PC and before another.
    #[must_use]
    pub fn has_later_def_before(&self, reg: u16, after_pc: i32, before_pc: i32) -> bool {
        self.def_pcs_by_reg
            .get(usize::from(reg))
            .is_some_and(|pcs| pcs.iter().any(|pc| *pc > after_pc && *pc < before_pc))
    }

    /// Return whether observable side effects exist strictly between two nodes.
    #[must_use]
    pub fn has_side_effect_between(&self, from: NodeId, to: NodeId) -> bool {
        if from.block != to.block || from.node >= to.node {
            return true;
        }
        let Some(prefix) = self.side_effect_prefix_by_block.get(from.block) else {
            return true;
        };
        let start = from.node.saturating_add(1);
        let end = to.node;
        prefix.get(end).copied().unwrap_or(0) > prefix.get(start).copied().unwrap_or(0)
    }

    fn ensure_value(&mut self, reference: SsaRef) {
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let reg = usize::from(value.reg);
        if reg >= self.facts.len() {
            self.facts.resize_with(reg + 1, Vec::new);
            self.defs.resize_with(reg + 1, Vec::new);
            self.real_uses.resize_with(reg + 1, Vec::new);
        }
        let version = usize::try_from(value.ver).unwrap_or(usize::MAX);
        if version == usize::MAX {
            return;
        }
        if version >= self.facts[reg].len() {
            self.facts[reg].resize(version + 1, ValueFacts::default());
            self.defs[reg].resize(version + 1, None);
            self.real_uses[reg].resize_with(version + 1, Vec::new);
        }
    }

    fn set_def(&mut self, reference: SsaRef, node: NodeId) {
        self.ensure_value(reference);
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let Some(slot) = self
            .defs
            .get_mut(usize::from(value.reg))
            .and_then(|versions| versions.get_mut(usize::try_from(value.ver).ok()?))
        else {
            return;
        };
        if slot.is_none() {
            *slot = Some(node);
            if let Some(defs) = self
                .defs_by_node
                .get_mut(node.block)
                .and_then(|block| block.get_mut(node.node))
            {
                defs.push(reference);
            }
        }
    }

    fn add_use(&mut self, reference: SsaRef, is_phi: bool, node: NodeId) {
        self.ensure_value(reference);
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let Some(facts) = self
            .facts
            .get_mut(usize::from(value.reg))
            .and_then(|versions| versions.get_mut(usize::try_from(value.ver).ok()?))
        else {
            return;
        };
        facts.uses += 1;
        if is_phi {
            facts.phi_uses += 1;
        } else if let Some(uses) = self
            .real_uses
            .get_mut(usize::from(value.reg))
            .and_then(|versions| versions.get_mut(usize::try_from(value.ver).ok()?))
        {
            uses.push(node);
        }
    }

    fn add_mutating_table_use(&mut self, reference: SsaRef) {
        self.ensure_value(reference);
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let Some(facts) = self
            .facts
            .get_mut(usize::from(value.reg))
            .and_then(|versions| versions.get_mut(usize::try_from(value.ver).ok()?))
        else {
            return;
        };
        facts.mutating_table_uses += 1;
    }

    fn add_upvalue_capture(&mut self, reference: SsaRef) {
        self.ensure_value(reference);
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let Some(facts) = self
            .facts
            .get_mut(usize::from(value.reg))
            .and_then(|versions| versions.get_mut(usize::try_from(value.ver).ok()?))
        else {
            return;
        };
        facts.upvalue_captures += 1;
    }

    fn add_def_pc(&mut self, reg: u16, pc: i32) {
        let reg = usize::from(reg);
        if reg >= self.def_pcs_by_reg.len() {
            self.def_pcs_by_reg.resize_with(reg + 1, Vec::new);
        }
        self.def_pcs_by_reg[reg].push(pc);
    }
}

/// Compute Phase 4 use-count and def-site facts once.
#[must_use]
pub fn analyze(function: &SsaFunction) -> DecompileAnalysis {
    let mut analysis = DecompileAnalysis {
        facts: vec![Vec::new(); function.num_regs],
        defs: vec![Vec::new(); function.num_regs],
        real_uses: vec![Vec::new(); function.num_regs],
        defs_by_node: function
            .blocks
            .iter()
            .map(|block| vec![Vec::new(); block.nodes.len()])
            .collect(),
        def_pcs_by_reg: vec![Vec::new(); function.num_regs],
        side_effect_prefix_by_block: side_effect_prefix_by_block(function),
    };

    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let id = NodeId {
                block: block.index,
                node: node_index,
            };
            node.visit_defs(|reference| {
                analysis.set_def(reference, id);
                if let Some(reg) = reference.reg_index() {
                    analysis.add_def_pc(reg, node.pc);
                }
            });
        }
    }

    for pcs in &mut analysis.def_pcs_by_reg {
        pcs.sort_unstable();
        pcs.dedup();
    }

    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let id = NodeId {
                block: block.index,
                node: node_index,
            };
            node.op.visit_uses(|reference, role| {
                analysis.add_use(reference, role == UseRole::Phi, id);
                match role {
                    UseRole::MutatingTable => analysis.add_mutating_table_use(reference),
                    UseRole::UpvalueCapture => analysis.add_upvalue_capture(reference),
                    UseRole::Value | UseRole::Phi | UseRole::LoopControl => {}
                }
            });
        }
    }

    analysis
}

fn side_effect_prefix_by_block(function: &SsaFunction) -> Vec<Vec<usize>> {
    function
        .blocks
        .iter()
        .map(|block| {
            let mut prefix = Vec::with_capacity(block.nodes.len() + 1);
            prefix.push(0);
            for node in &block.nodes {
                let next = prefix.last().copied().unwrap_or(0)
                    + usize::from(node.op.effects().blocks_reordering());
                prefix.push(next);
            }
            prefix
        })
        .collect()
}
