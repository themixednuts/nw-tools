//! Phase 6 boolean and short-circuit reconstruction.

use std::collections::HashMap;

use crate::{
    decompile::{
        analysis::{DecompileAnalysis, NodeId, ValueId},
        control_flow::{
            conditionals::{self, BranchInfo},
            loops::LoopAnalysis,
            regions::BlockSet,
        },
    },
    ir::{SsaFunction, SsaNode, SsaOp, SsaRef},
};

pub mod normalize;
mod short_circuit;
mod value_chain;

pub use short_circuit::{
    BoolConnector, ConditionChain, ConditionSegment, ValuePlan, ValuePlanKind, ValueTerm,
};

/// Boolean reconstruction facts computed once for a function.
#[derive(Debug, Clone, Default)]
pub struct BooleanAnalysis {
    condition_chains: HashMap<usize, ConditionChain>,
    value_plans: Vec<ValuePlan>,
    value_by_start: HashMap<usize, usize>,
    value_by_phi: HashMap<ValueId, usize>,
}

impl BooleanAnalysis {
    /// Empty analysis for branch-free or Phase 4-only callers.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Return a compound condition chain starting at `block`.
    #[must_use]
    pub fn condition_chain(&self, block: usize) -> Option<&ConditionChain> {
        self.condition_chains.get(&block)
    }

    /// Return a value select plan starting at `block`.
    #[must_use]
    pub fn value_select_start(&self, block: usize) -> Option<&ValuePlan> {
        self.value_by_start
            .get(&block)
            .and_then(|idx| self.value_plans.get(*idx))
    }

    /// Return a value select plan whose internal materialization covers `block`.
    #[must_use]
    pub fn value_select_covering(&self, block: usize) -> Option<&ValuePlan> {
        self.value_plans
            .iter()
            .find(|plan| plan.start < block && block < plan.merge)
    }

    /// Return the value select plan that materializes `reference`.
    #[must_use]
    pub fn value_for_phi(&self, reference: SsaRef) -> Option<&ValuePlan> {
        let value = ValueId::from_ref(reference)?;
        self.value_by_phi
            .get(&value)
            .and_then(|idx| self.value_plans.get(*idx))
    }
}

/// Compute all Phase 6 boolean reconstruction facts for `function`.
#[must_use]
pub fn analyze(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    loops: &LoopAnalysis,
    pc_map: &[Option<usize>],
) -> BooleanAnalysis {
    let loop_headers = loops.loop_headers(function.blocks.len());
    let mut consumed_values = BlockSet::new(function.blocks.len());
    let mut analysis = BooleanAnalysis::empty();

    for block in 0..function.blocks.len() {
        if consumed_values.contains(block) {
            continue;
        }

        if let Some(plan) = short_circuit::value_plan(function, expr_analysis, block, pc_map) {
            let idx = analysis.value_plans.len();
            if let Some(value) = ValueId::from_ref(plan.dest) {
                analysis.value_by_phi.entry(value).or_insert(idx);
            }
            analysis.value_by_start.insert(plan.start, idx);
            for consumed in plan.consumed_blocks() {
                consumed_values.insert(consumed);
            }
            analysis.value_plans.push(plan);
            continue;
        }

        if let Some(chain) =
            short_circuit::condition_chain(function, expr_analysis, block, pc_map, &loop_headers)
        {
            analysis.condition_chains.insert(chain.start, chain);
        }
    }

    analysis
}

pub(crate) fn branch_at(function: &SsaFunction, id: NodeId) -> Option<&crate::ir::SsaNode> {
    function
        .blocks
        .get(id.block)
        .and_then(|block| block.nodes.get(id.node))
}

fn branch_info(
    function: &SsaFunction,
    block: usize,
    pc_map: &[Option<usize>],
) -> Option<BranchInfo> {
    conditionals::branch_info(function, block, pc_map)
}

fn is_condition_block(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    block: usize,
    pc_map: &[Option<usize>],
    loop_headers: &BlockSet,
) -> bool {
    !loop_headers.contains(block)
        && conditionals::is_pure_condition_block(function, block)
        && !has_unrelated_side_effect_before_branch(function, block)
        && !has_prefix_def_used_after_branch(function, expr_analysis, block)
        && short_circuit::value_plan(function, expr_analysis, block, pc_map).is_none()
}

fn has_prefix_def_used_after_branch(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    block: usize,
) -> bool {
    let Some(block_ref) = function.blocks.get(block) else {
        return false;
    };
    let Some(branch_index) = block_ref
        .nodes
        .iter()
        .position(|node| matches!(node.op, SsaOp::Branch { .. }))
    else {
        return false;
    };

    for node_index in 0..branch_index {
        let id = NodeId {
            block,
            node: node_index,
        };
        for reference in expr_analysis.defs_at(id) {
            let facts = expr_analysis.facts(*reference);
            if facts.phi_uses > 0 || facts.upvalue_captures > 0 {
                return true;
            }
            if expr_analysis
                .real_uses(*reference)
                .iter()
                .any(|use_site| use_site.block != block || use_site.node > branch_index)
            {
                return true;
            }
        }
    }

    false
}

fn has_unrelated_side_effect_before_branch(function: &SsaFunction, block: usize) -> bool {
    let Some(block) = function.blocks.get(block) else {
        return false;
    };
    let Some((branch_index, branch)) = block
        .nodes
        .iter()
        .enumerate()
        .find(|(_, node)| matches!(node.op, SsaOp::Branch { .. }))
    else {
        return false;
    };

    let mut needed = branch_operands(branch);
    for node in block.nodes.iter().take(branch_index).rev() {
        if node.is_meta_only {
            continue;
        }
        let feeds_condition = node.dest != SsaRef::None && needed.contains(&node.dest);
        if node_has_observable_side_effect(node) && !feeds_condition {
            return true;
        }
        if feeds_condition {
            add_used_refs(&node.op, &mut needed);
        }
    }

    false
}

fn branch_operands(node: &SsaNode) -> Vec<SsaRef> {
    let SsaOp::Branch { a, b, .. } = node.op else {
        return Vec::new();
    };
    [a, b]
        .into_iter()
        .filter(|reference| *reference != SsaRef::None)
        .collect()
}

fn add_used_refs(op: &SsaOp, out: &mut Vec<SsaRef>) {
    let mut push = |reference| {
        if reference != SsaRef::None && !out.contains(&reference) {
            out.push(reference);
        }
    };
    match op {
        SsaOp::Move { src }
        | SsaOp::SetGlobal { src, .. }
        | SsaOp::SetUpval { src, .. }
        | SsaOp::UnOp { value: src, .. } => push(*src),
        SsaOp::GetTable { table, key } | SsaOp::SelfOp { table, key, .. } => {
            push(*table);
            push(*key);
        }
        SsaOp::SetTable { table, key, value } => {
            push(*table);
            push(*key);
            push(*value);
        }
        SsaOp::BinOp { left, right, .. } => {
            push(*left);
            push(*right);
        }
        SsaOp::Concat { operands } | SsaOp::Phi { operands, .. } => {
            for operand in operands {
                push(*operand);
            }
        }
        SsaOp::Branch { a, b, .. } => {
            push(*a);
            push(*b);
        }
        SsaOp::Call { func, args, .. } | SsaOp::TailCall { func, args, .. } => {
            push(*func);
            for arg in args {
                push(*arg);
            }
        }
        SsaOp::Return { values, .. } | SsaOp::SetList { values, .. } => {
            for value in values {
                push(*value);
            }
        }
        SsaOp::Closure { upvalues, .. } => {
            for capture in upvalues {
                if let crate::ir::UpvalueCapture::ParentLocal(reference) = capture {
                    push(*reference);
                }
            }
        }
        SsaOp::Nop
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
    }
}

fn node_has_observable_side_effect(node: &SsaNode) -> bool {
    matches!(
        node.op,
        SsaOp::Call { .. }
            | SsaOp::TailCall { .. }
            | SsaOp::SetGlobal { .. }
            | SsaOp::SetUpval { .. }
            | SsaOp::SetTable { .. }
            | SsaOp::SetList { .. }
    )
}

fn is_pure_value_node(node: &SsaNode) -> bool {
    node.is_meta_only
        || matches!(
            node.op,
            SsaOp::Phi { .. }
                | SsaOp::Nop
                | SsaOp::Jump { .. }
                | SsaOp::Branch { .. }
                | SsaOp::Move { .. }
                | SsaOp::LoadK { .. }
                | SsaOp::LoadBool { .. }
                | SsaOp::LoadNil { .. }
                | SsaOp::GetUpval { .. }
                | SsaOp::GetGlobal { .. }
                | SsaOp::GetTable { .. }
                | SsaOp::NewTable { .. }
                | SsaOp::SelfOp { .. }
                | SsaOp::BinOp { .. }
                | SsaOp::UnOp { .. }
                | SsaOp::Concat { .. }
                | SsaOp::Call { .. }
                | SsaOp::Closure { .. }
        )
}

fn phi_sources(function: &SsaFunction, block: usize) -> impl Iterator<Item = PhiData<'_>> {
    function
        .blocks
        .get(block)
        .into_iter()
        .flat_map(|block| block.nodes.iter())
        .filter_map(|node| {
            let SsaOp::Phi { operands, blocks } = &node.op else {
                return None;
            };
            Some(PhiData {
                dest: node.dest,
                pc: node.pc,
                operands,
                blocks,
            })
        })
}

#[derive(Debug, Clone, Copy)]
struct PhiData<'a> {
    dest: SsaRef,
    pc: i32,
    operands: &'a [SsaRef],
    blocks: &'a [usize],
}

impl PhiData<'_> {
    fn operand_from(self, block: usize) -> Option<SsaRef> {
        self.blocks
            .iter()
            .copied()
            .zip(self.operands.iter().copied())
            .find_map(|(source, operand)| (source == block).then_some(operand))
    }
}
