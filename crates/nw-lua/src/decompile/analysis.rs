//! One-time SSA facts used by Phase 4 reconstruction.

use crate::ir::{SsaFunction, SsaNode, SsaOp, SsaRef, UpvalueCapture};

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
    defs_by_node: Vec<Vec<Vec<SsaRef>>>,
    def_pcs_by_reg: Vec<Vec<i32>>,
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

    fn ensure_value(&mut self, reference: SsaRef) {
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let reg = usize::from(value.reg);
        if reg >= self.facts.len() {
            self.facts.resize_with(reg + 1, Vec::new);
            self.defs.resize_with(reg + 1, Vec::new);
        }
        let version = usize::try_from(value.ver).unwrap_or(usize::MAX);
        if version == usize::MAX {
            return;
        }
        if version >= self.facts[reg].len() {
            self.facts[reg].resize(version + 1, ValueFacts::default());
            self.defs[reg].resize(version + 1, None);
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

    fn add_use(&mut self, reference: SsaRef, is_phi: bool) {
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
        defs_by_node: function
            .blocks
            .iter()
            .map(|block| vec![Vec::new(); block.nodes.len()])
            .collect(),
        def_pcs_by_reg: vec![Vec::new(); function.num_regs],
    };

    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let id = NodeId {
                block: block.index,
                node: node_index,
            };
            analysis.set_def(node.dest, id);
            if let Some(reg) = node.dest.reg_index() {
                analysis.add_def_pc(reg, node.pc);
            }
        }
    }

    for implicit in &function.implicit_defs {
        let id = NodeId {
            block: implicit.block,
            node: implicit.node,
        };
        analysis.set_def(
            SsaRef::Reg {
                reg: implicit.reg,
                ver: implicit.version,
            },
            id,
        );
        if let Some(node) = analysis.node(function, id) {
            analysis.add_def_pc(implicit.reg, node.pc);
        }
    }

    for pcs in &mut analysis.def_pcs_by_reg {
        pcs.sort_unstable();
        pcs.dedup();
    }

    for block in &function.blocks {
        for node in &block.nodes {
            if let SsaOp::Phi { operands, .. } = &node.op {
                for operand in operands {
                    analysis.add_use(*operand, true);
                }
                continue;
            }

            for_each_use(&node.op, |reference| analysis.add_use(reference, false));
            if let SsaOp::SetTable { table, .. } = &node.op {
                analysis.add_mutating_table_use(*table);
            }
            if let SsaOp::SetList { table, .. } = &node.op {
                analysis.add_mutating_table_use(*table);
            }
            if let SsaOp::Closure { upvalues, .. } = &node.op {
                for capture in upvalues {
                    if let UpvalueCapture::ParentLocal(reference) = capture {
                        analysis.add_upvalue_capture(*reference);
                    }
                }
            }
        }
    }

    analysis
}

pub(crate) fn for_each_use(op: &SsaOp, mut f: impl FnMut(SsaRef)) {
    match op {
        SsaOp::Move { src } => f(*src),
        SsaOp::GetTable { table, key } => {
            f(*table);
            f(*key);
        }
        SsaOp::SetGlobal { src, .. } | SsaOp::SetUpval { src, .. } => f(*src),
        SsaOp::SetTable { table, key, value } => {
            f(*table);
            f(*key);
            f(*value);
        }
        SsaOp::SelfOp { table, key, .. } => {
            f(*table);
            f(*key);
        }
        SsaOp::BinOp { left, right, .. } => {
            f(*left);
            f(*right);
        }
        SsaOp::UnOp { value, .. } => f(*value),
        SsaOp::Concat { operands } => {
            for operand in operands {
                f(*operand);
            }
        }
        SsaOp::Branch { a, b, .. } => {
            f(*a);
            f(*b);
        }
        SsaOp::Call { func, args, .. } | SsaOp::TailCall { func, args, .. } => {
            f(*func);
            for arg in args {
                f(*arg);
            }
        }
        SsaOp::Return { values, .. } => {
            for value in values {
                f(*value);
            }
        }
        SsaOp::SetList { table, values, .. } => {
            f(*table);
            for value in values {
                f(*value);
            }
        }
        SsaOp::Phi { .. }
        | SsaOp::Nop
        | SsaOp::LoadK { .. }
        | SsaOp::LoadBool { .. }
        | SsaOp::LoadNil { .. }
        | SsaOp::GetUpval { .. }
        | SsaOp::GetGlobal { .. }
        | SsaOp::NewTable { .. }
        | SsaOp::Jump { .. }
        | SsaOp::ForPrep { .. }
        | SsaOp::ForLoop { .. }
        | SsaOp::TForLoop { .. }
        | SsaOp::Close { .. }
        | SsaOp::VarArg { .. } => {}
        SsaOp::Closure { upvalues, .. } => {
            for capture in upvalues {
                if let UpvalueCapture::ParentLocal(reference) = capture {
                    f(*reference);
                }
            }
        }
    }
}
