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
    ir::{SsaFunction, SsaOp, SsaRef},
};

pub mod normalize;
mod short_circuit;

pub use short_circuit::{
    BoolConnector, ConditionChain, ConditionSegment, ValuePlan, ValuePlanKind,
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

        if let Some(chain) = short_circuit::condition_chain(function, block, pc_map, &loop_headers)
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

fn is_condition_block(function: &SsaFunction, block: usize, loop_headers: &BlockSet) -> bool {
    !loop_headers.contains(block)
        && conditionals::is_pure_condition_block(function, block)
        && !crate::decompile::control_flow::loops::has_tail_body_before_branch(function, block)
}

fn is_pure_value_block(function: &SsaFunction, block: usize) -> bool {
    let Some(block_ref) = function.blocks.get(block) else {
        return false;
    };
    block_ref.nodes.iter().all(|node| {
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
                    | SsaOp::Closure { .. }
            )
    })
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
