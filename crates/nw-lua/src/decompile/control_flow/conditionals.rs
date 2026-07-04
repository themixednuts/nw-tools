//! Conditional branch helpers: target polarity, merge discovery, and phis.

use std::collections::VecDeque;

use crate::ir::{RelOp, SsaFunction, SsaNode, SsaOp, SsaRef};

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
        node.is_meta_only
            || matches!(
                node.op,
                SsaOp::Jump { .. }
                    | SsaOp::Phi { .. }
                    | SsaOp::Nop
                    | SsaOp::Close { .. }
                    | SsaOp::Return { .. }
            )
    })
}

#[must_use]
pub fn is_elseif_candidate(
    function: &SsaFunction,
    block: usize,
    merge: usize,
    pc_map: &[Option<usize>],
    loop_headers: &BlockSet,
) -> bool {
    if block == merge
        || loop_headers.contains(block)
        || branch_info(function, block, pc_map).is_none()
    {
        return false;
    }

    let Some(block_ref) = function.blocks.get(block) else {
        return false;
    };
    if block_ref.succs.len() < 2 {
        return false;
    }

    let mut branches = 0;
    for node in &block_ref.nodes {
        match &node.op {
            SsaOp::Branch { .. } => branches += 1,
            SsaOp::Phi { .. }
            | SsaOp::Nop
            | SsaOp::Jump { .. }
            | SsaOp::LoadK { .. }
            | SsaOp::LoadBool { .. }
            | SsaOp::LoadNil { .. }
            | SsaOp::GetGlobal { .. }
            | SsaOp::GetTable { .. }
            | SsaOp::GetUpval { .. }
            | SsaOp::Move { .. }
            | SsaOp::BinOp { .. }
            | SsaOp::UnOp { .. }
            | SsaOp::Concat { .. } => {}
            _ => return false,
        }
    }
    branches == 1
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
            | SsaOp::LoadBool { .. }
            | SsaOp::LoadNil { .. }
            | SsaOp::GetGlobal { .. }
            | SsaOp::GetTable { .. }
            | SsaOp::GetUpval { .. }
            | SsaOp::Move { .. }
            | SsaOp::BinOp { .. }
            | SsaOp::UnOp { .. }
            | SsaOp::Concat { .. }
            | SsaOp::SelfOp { .. } => {}
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

    if is_terminal(function, true_end) {
        return Some(false_end);
    }
    if is_terminal(function, false_end) {
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
        let score = td.max(fd);
        if score < best_score {
            best_score = score;
            best = Some(candidate);
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
