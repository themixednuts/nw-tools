//! Conditional branch helpers: target polarity, merge discovery, and phis.

use std::collections::VecDeque;

use crate::{
    bytecode::SemanticOp,
    decompile::analysis::DecompileAnalysis,
    ir::{RelOp, SsaFunction, SsaNode, SsaOp, SsaRef},
};

use super::regions::BlockSet;
use crate::decompile::analysis::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchInfo {
    pub node: NodeId,
    pub true_block: usize,
    pub false_block: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhiSource {
    pub dest: SsaRef,
    pub pc: i32,
    pub sources: Vec<(usize, SsaRef)>,
}

#[must_use]
pub fn pc_to_block_map(function: &SsaFunction) -> Vec<Option<usize>> {
    let code_len = function.instructions.len();
    let mut map = vec![None; code_len];
    for block in &function.blocks {
        for pc in block.start_pc..=block.end_pc {
            if let Some(slot) = map.get_mut(pc) {
                *slot = Some(block.index);
            }
        }
    }
    map
}

#[must_use]
pub fn branch_info(
    function: &SsaFunction,
    block: usize,
    pc_map: &[Option<usize>],
) -> Option<BranchInfo> {
    let block_ref = function.blocks.get(block)?;
    for (node_index, node) in block_ref.nodes.iter().enumerate() {
        let SsaOp::Branch {
            t_true, t_false, ..
        } = &node.op
        else {
            continue;
        };
        let true_block = pc_to_block(pc_map, *t_true)?;
        let false_block = pc_to_block(pc_map, *t_false)?;
        return Some(BranchInfo {
            node: NodeId {
                block,
                node: node_index,
            },
            true_block,
            false_block,
        });
    }
    None
}

#[must_use]
pub fn pc_to_block(pc_map: &[Option<usize>], pc: i32) -> Option<usize> {
    usize::try_from(pc)
        .ok()
        .and_then(|pc| pc_map.get(pc).copied().flatten())
}

#[must_use]
pub fn follow_jmp_only(function: &SsaFunction, block: usize, stop: Option<usize>) -> usize {
    let mut current = block;
    let mut steps = 0;
    while steps < function.blocks.len().min(64) {
        steps += 1;
        if Some(current) == stop || !is_jmp_only(function, current) {
            break;
        }
        let Some(&next) = function.blocks[current].succs.first() else {
            break;
        };
        current = next;
    }
    current
}

#[must_use]
pub fn can_reach(function: &SsaFunction, start: usize, target: usize) -> bool {
    if start >= function.blocks.len() || target >= function.blocks.len() {
        return false;
    }
    let mut visited = vec![false; function.blocks.len()];
    let mut queue = VecDeque::new();
    visited[start] = true;
    queue.push_back(start);

    while let Some(block) = queue.pop_front() {
        if block == target {
            return true;
        }
        for &succ in &function.blocks[block].succs {
            if succ < function.blocks.len() && !visited[succ] {
                visited[succ] = true;
                queue.push_back(succ);
            }
        }
    }

    false
}

#[must_use]
pub fn is_jmp_only(function: &SsaFunction, block: usize) -> bool {
    let Some(block) = function.blocks.get(block) else {
        return false;
    };
    block.succs.len() == 1
        && block.nodes.iter().all(|node| {
            matches!(
                node.op,
                SsaOp::Jump { .. } | SsaOp::Phi { .. } | SsaOp::Nop | SsaOp::Close { .. }
            )
        })
}

#[must_use]
pub fn is_empty_structural(function: &SsaFunction, block: usize) -> bool {
    let Some(block) = function.blocks.get(block) else {
        return true;
    };
    block.nodes.iter().all(|node| {
        matches!(
            node.op,
            SsaOp::Jump { .. } | SsaOp::Phi { .. } | SsaOp::Nop | SsaOp::Close { .. }
        ) || is_final_empty_return(function, node)
    })
}

#[must_use]
pub fn is_final_empty_return_block(function: &SsaFunction, block: usize) -> bool {
    let Some(block) = function.blocks.get(block) else {
        return false;
    };
    is_empty_structural(function, block.index)
        && block
            .nodes
            .iter()
            .any(|node| is_final_empty_return(function, node))
}

#[must_use]
pub fn is_terminal_block(function: &SsaFunction, block: usize) -> bool {
    is_terminal(function, block)
}

#[must_use]
pub fn has_unreachable_jump_immediately_before(function: &SsaFunction, block: usize) -> bool {
    let Some(block_ref) = function.blocks.get(block) else {
        return false;
    };
    let Some(pc) = block_ref.start_pc.checked_sub(1) else {
        return false;
    };
    function
        .instructions
        .get(pc)
        .is_some_and(|instruction| instruction.op == SemanticOp::Jmp)
        && function
            .blocks
            .iter()
            .all(|block| pc < block.start_pc || pc > block.end_pc)
}

#[must_use]
pub fn is_elseif_candidate(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    block: usize,
    merge: usize,
    pc_map: &[Option<usize>],
    loop_headers: &BlockSet,
) -> bool {
    if block == merge || loop_headers.contains(block) {
        return false;
    }

    let Some(branch) = branch_info(function, block, pc_map) else {
        return false;
    };
    let Some(block_ref) = function.blocks.get(block) else {
        return false;
    };
    if block_ref.succs.len() < 2 {
        return false;
    }

    let mut condition_nodes = vec![false; block_ref.nodes.len()];
    let mut pending = Vec::new();
    block_ref.nodes[branch.node.node]
        .op
        .visit_uses(|reference, _| pending.push(reference));
    while let Some(reference) = pending.pop() {
        let Some(definition) = analysis.def_site(reference) else {
            continue;
        };
        if definition.block != block || definition.node >= branch.node.node {
            continue;
        }
        let Some(owned) = condition_nodes.get_mut(definition.node) else {
            continue;
        };
        if *owned {
            continue;
        }
        *owned = true;
        block_ref.nodes[definition.node]
            .op
            .visit_uses(|operand, _| pending.push(operand));
    }

    block_ref.nodes.iter().enumerate().all(|(index, node)| {
        index == branch.node.node
            || condition_nodes[index]
            || matches!(node.op, SsaOp::Phi { .. } | SsaOp::Nop)
    })
}

#[must_use]
pub fn is_pure_condition_block(function: &SsaFunction, block: usize) -> bool {
    let Some(block_ref) = function.blocks.get(block) else {
        return false;
    };
    let mut branches = 0;
    for node in &block_ref.nodes {
        match &node.op {
            SsaOp::Branch { rel, .. } if *rel != RelOp::TestSet => branches += 1,
            SsaOp::Phi { .. }
            | SsaOp::Nop
            | SsaOp::Jump { .. }
            | SsaOp::LoadK { .. }
            | SsaOp::LoadLiteral { .. }
            | SsaOp::LoadBool { .. }
            | SsaOp::LoadNil { .. }
            | SsaOp::GetGlobal { .. }
            | SsaOp::GetTable { .. }
            | SsaOp::GetUpval { .. }
            | SsaOp::Move { .. }
            | SsaOp::BinOp { .. }
            | SsaOp::UnOp { .. }
            | SsaOp::Concat { .. }
            | SsaOp::SelfOp { .. }
            | SsaOp::Call { .. } => {}
            _ => return false,
        }
    }
    branches == 1
}

#[must_use]
pub fn find_merge(
    function: &SsaFunction,
    branch: usize,
    true_block: usize,
    false_block: usize,
) -> Option<usize> {
    let true_end = follow_jmp_only(function, true_block, None);
    let false_end = follow_jmp_only(function, false_block, None);
    if true_end == false_end && true_end > branch {
        return Some(true_end);
    }

    let true_dist = forward_distances(function, branch, true_end);
    let false_dist = forward_distances(function, branch, false_end);
    let mut best = None;
    let mut best_score = usize::MAX;

    for candidate in (branch + 1)..function.blocks.len() {
        let Some(td) = true_dist[candidate] else {
            continue;
        };
        let Some(fd) = false_dist[candidate] else {
            continue;
        };
        let true_only = strictly_dominated_by(function, candidate, true_end)
            && !strictly_dominated_by(function, candidate, false_end);
        let false_only = strictly_dominated_by(function, candidate, false_end)
            && !strictly_dominated_by(function, candidate, true_end);
        if true_only || false_only {
            continue;
        }
        if !post_dominates(function, candidate, branch) {
            continue;
        }
        let score = td.max(fd);
        if score < best_score {
            best_score = score;
            best = Some(candidate);
        }
    }

    if best.is_none() {
        let true_terminal = is_terminal(function, true_end);
        let false_terminal = is_terminal(function, false_end);
        if true_terminal && false_terminal {
            return Some(true_end.max(false_end).saturating_add(1));
        }

        let true_all_terminate = all_paths_terminate(function, branch, true_end);
        let false_all_terminate = all_paths_terminate(function, branch, false_end);
        if true_all_terminate {
            return Some(false_end);
        }
        if false_all_terminate {
            return Some(true_end);
        }
    }

    best.or_else(|| branch.checked_add(1))
        .filter(|candidate| *candidate <= function.blocks.len())
}

#[must_use]
pub fn phi_sources(function: &SsaFunction, merge: usize) -> Vec<PhiSource> {
    let Some(block) = function.blocks.get(merge) else {
        return Vec::new();
    };
    block
        .nodes
        .iter()
        .filter_map(|node| {
            let SsaOp::Phi { operands, blocks } = &node.op else {
                return None;
            };
            let sources = blocks
                .iter()
                .copied()
                .zip(operands.iter().copied())
                .collect::<Vec<_>>();
            Some(PhiSource {
                dest: node.dest,
                pc: node.pc,
                sources,
            })
        })
        .collect()
}

fn forward_distances(function: &SsaFunction, branch: usize, start: usize) -> Vec<Option<usize>> {
    let mut dist = vec![None; function.blocks.len()];
    if start >= function.blocks.len() {
        return dist;
    }
    let mut queue = VecDeque::new();
    dist[start] = Some(0);
    queue.push_back(start);

    while let Some(block) = queue.pop_front() {
        let next_dist = dist[block].unwrap_or(0) + 1;
        for &succ in &function.blocks[block].succs {
            if succ <= branch || succ >= function.blocks.len() || dist[succ].is_some() {
                continue;
            }
            dist[succ] = Some(next_dist);
            queue.push_back(succ);
        }
    }

    dist
}

fn post_dominates(function: &SsaFunction, candidate: usize, block: usize) -> bool {
    if candidate == block || candidate >= function.blocks.len() || block >= function.blocks.len() {
        return false;
    }

    let mut state = vec![VisitState::Unseen; function.blocks.len()];
    all_paths_reach_candidate_or_terminate(function, candidate, block, &mut state)
}

fn all_paths_reach_candidate_or_terminate(
    function: &SsaFunction,
    candidate: usize,
    block: usize,
    state: &mut [VisitState],
) -> bool {
    if block >= function.blocks.len() {
        return false;
    }
    if block == candidate || is_terminal(function, block) {
        return true;
    }
    match state[block] {
        VisitState::Done(result) => return result,
        VisitState::Visiting => return true,
        VisitState::Unseen => {}
    }

    state[block] = VisitState::Visiting;
    let result =
        !function.blocks[block].succs.is_empty()
            && function.blocks[block].succs.iter().copied().all(|succ| {
                all_paths_reach_candidate_or_terminate(function, candidate, succ, state)
            });
    state[block] = VisitState::Done(result);
    result
}

#[must_use]
pub fn all_paths_terminate(function: &SsaFunction, branch: usize, start: usize) -> bool {
    if start >= function.blocks.len() {
        return false;
    }

    let mut state = vec![TerminationVisitState::Unseen; function.blocks.len()];
    let result = all_paths_terminate_inner(function, branch, start, &mut state);
    result.closed && result.reaches_terminal
}

fn all_paths_terminate_inner(
    function: &SsaFunction,
    branch: usize,
    block: usize,
    state: &mut [TerminationVisitState],
) -> TerminationResult {
    if block >= function.blocks.len() || block <= branch {
        return TerminationResult::open();
    }
    if is_terminal(function, block) {
        return TerminationResult::terminal();
    }
    match state[block] {
        TerminationVisitState::Done(result) => return result,
        TerminationVisitState::Visiting => return TerminationResult::cycle(),
        TerminationVisitState::Unseen => {}
    }

    state[block] = TerminationVisitState::Visiting;
    let mut result = if function.blocks[block].succs.is_empty() {
        TerminationResult::open()
    } else {
        TerminationResult {
            closed: true,
            reaches_terminal: false,
        }
    };
    for succ in function.blocks[block].succs.iter().copied() {
        let succ_result = all_paths_terminate_inner(function, branch, succ, state);
        result.closed &= succ_result.closed;
        result.reaches_terminal |= succ_result.reaches_terminal;
    }
    state[block] = TerminationVisitState::Done(result);
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminationResult {
    closed: bool,
    reaches_terminal: bool,
}

impl TerminationResult {
    const fn open() -> Self {
        Self {
            closed: false,
            reaches_terminal: false,
        }
    }

    const fn terminal() -> Self {
        Self {
            closed: true,
            reaches_terminal: true,
        }
    }

    const fn cycle() -> Self {
        Self {
            closed: true,
            reaches_terminal: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminationVisitState {
    Unseen,
    Visiting,
    Done(TerminationResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unseen,
    Visiting,
    Done(bool),
}

fn strictly_dominated_by(function: &SsaFunction, block: usize, dominator: usize) -> bool {
    if block == dominator {
        return false;
    }
    let mut current = Some(block);
    let mut steps = 0;
    while let Some(block) = current {
        if block == dominator {
            return true;
        }
        if steps >= function.blocks.len() {
            return false;
        }
        steps += 1;
        current = function.blocks.get(block).and_then(|block| block.idom);
    }
    false
}

fn is_terminal(function: &SsaFunction, block: usize) -> bool {
    function
        .blocks
        .get(block)
        .is_none_or(|block| block.succs.is_empty() || block.nodes.iter().any(is_return_node))
}

fn is_return_node(node: &SsaNode) -> bool {
    matches!(node.op, SsaOp::Return { .. } | SsaOp::TailCall { .. })
}

fn is_final_empty_return(function: &SsaFunction, node: &SsaNode) -> bool {
    let SsaOp::Return { values, count, .. } = &node.op else {
        return false;
    };
    *count == 1
        && values.is_empty()
        && usize::try_from(node.pc).ok() == function.instructions.len().checked_sub(1)
}
