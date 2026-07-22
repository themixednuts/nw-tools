//! Loop discovery and classification from stored CFG/dominator facts.

use std::collections::{BTreeSet, VecDeque};

use crate::{
    decompile::analysis::NodeId,
    ir::{SsaFunction, SsaOp, SsaRef},
};

use super::conditionals::{branch_info, follow_jmp_only, is_pure_condition_block, pc_to_block};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopAnalysis {
    pub natural: Vec<NaturalLoop>,
    pub numeric: Vec<NumericForLoop>,
    pub generic: Vec<GenericForLoop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NaturalLoop {
    pub header: usize,
    pub latch: usize,
    pub blocks: BTreeSet<usize>,
    pub exit: Option<usize>,
    pub kind: NaturalLoopKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalLoopKind {
    While,
    Repeat { tail: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericForLoop {
    pub prep: usize,
    pub loop_block: usize,
    pub body_start: usize,
    pub exit: usize,
    pub base: u16,
    pub prep_node: NodeId,
    pub loop_node: NodeId,
    pub start_node: Option<NodeId>,
    pub stop_node: Option<NodeId>,
    pub step_node: Option<NodeId>,
    pub var: SsaRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericForLoop {
    pub entry: usize,
    pub tfor_block: usize,
    pub latch_block: usize,
    pub body_start: usize,
    pub exit: usize,
    pub base: u16,
    pub count: i32,
    pub tfor_node: NodeId,
    pub call_node: Option<NodeId>,
    pub vars: Vec<SsaRef>,
}

#[must_use]
pub fn analyze(function: &SsaFunction, pc_map: &[Option<usize>]) -> LoopAnalysis {
    let numeric = numeric_for_loops(function, pc_map);
    let generic = generic_for_loops(function);
    let natural = natural_loops(function, pc_map);
    LoopAnalysis {
        natural,
        numeric,
        generic,
    }
}

impl LoopAnalysis {
    #[must_use]
    pub fn natural_at(&self, header: usize) -> Option<&NaturalLoop> {
        self.natural
            .iter()
            .filter(|info| info.header == header)
            .max_by_key(|info| {
                (
                    info.blocks
                        .iter()
                        .next_back()
                        .copied()
                        .unwrap_or(info.latch),
                    info.blocks.len(),
                    info.latch,
                )
            })
    }

    #[must_use]
    pub fn numeric_at(&self, prep: usize) -> Option<&NumericForLoop> {
        self.numeric.iter().find(|info| info.prep == prep)
    }

    #[must_use]
    pub fn generic_at_entry(&self, entry: usize) -> Option<&GenericForLoop> {
        self.generic.iter().find(|info| info.entry == entry)
    }

    #[must_use]
    pub fn loop_headers(&self, len: usize) -> super::regions::BlockSet {
        let mut headers = super::regions::BlockSet::new(len);
        for info in &self.natural {
            headers.insert(info.header);
        }
        headers
    }
}

fn numeric_for_loops(function: &SsaFunction, pc_map: &[Option<usize>]) -> Vec<NumericForLoop> {
    let mut loops = Vec::new();

    for block in &function.blocks {
        let Some((prep_index, base, target)) =
            block
                .nodes
                .iter()
                .enumerate()
                .find_map(|(node_index, node)| {
                    let SsaOp::ForPrep { control, target } = node.op else {
                        return None;
                    };
                    Some((node_index, control.base(), target))
                })
        else {
            continue;
        };
        let Some(loop_block) = pc_to_block(pc_map, target) else {
            continue;
        };
        let Some((loop_index, loop_node)) = function.blocks[loop_block].nodes.iter().enumerate().find(
            |(_, node)| matches!(node.op, SsaOp::ForLoop { control, .. } if control.base() == base),
        ) else {
            continue;
        };

        let body_start = function.blocks[loop_block]
            .succs
            .iter()
            .copied()
            .find(|succ| *succ < loop_block)
            .unwrap_or_else(|| block.index.saturating_add(1));
        let Some(exit) = function.blocks[loop_block]
            .succs
            .iter()
            .copied()
            .find(|succ| *succ != body_start)
        else {
            continue;
        };

        loops.push(NumericForLoop {
            prep: block.index,
            loop_block,
            body_start,
            exit,
            base,
            prep_node: NodeId {
                block: block.index,
                node: prep_index,
            },
            loop_node: NodeId {
                block: loop_block,
                node: loop_index,
            },
            start_node: reaching_def(function, block.index, base, Some(prep_index)),
            stop_node: reaching_def(
                function,
                block.index,
                base.saturating_add(1),
                Some(prep_index),
            ),
            step_node: reaching_def(
                function,
                block.index,
                base.saturating_add(2),
                Some(prep_index),
            ),
            var: loop_node.dest,
        });
    }

    loops
}

fn generic_for_loops(function: &SsaFunction) -> Vec<GenericForLoop> {
    let mut loops = Vec::new();

    for block in &function.blocks {
        let Some((node_index, base, count)) =
            block
                .nodes
                .iter()
                .enumerate()
                .find_map(|(node_index, node)| {
                    let SsaOp::TForLoop { control, count } = node.op else {
                        return None;
                    };
                    Some((node_index, control.base(), count))
                })
        else {
            continue;
        };

        let Some(latch_block) = block
            .succs
            .iter()
            .copied()
            .find(|succ| super::conditionals::is_jmp_only(function, *succ))
        else {
            continue;
        };
        let Some(body_start) = function.blocks[latch_block]
            .succs
            .iter()
            .copied()
            .find(|succ| *succ != block.index)
        else {
            continue;
        };
        let Some(exit) = block
            .succs
            .iter()
            .copied()
            .find(|succ| *succ != latch_block)
        else {
            continue;
        };
        let entry = block
            .preds
            .iter()
            .copied()
            .filter(|pred| *pred < block.index && *pred != body_start)
            .min()
            .unwrap_or(block.index);

        let mut vars = Vec::new();
        function.blocks[block.index].nodes[node_index].visit_defs(|reference| {
            if reference.reg_index().is_some_and(|reg| {
                reg >= base.saturating_add(3)
                    && reg
                        < base.saturating_add(3 + u16::try_from(count.max(0)).unwrap_or(u16::MAX))
            }) {
                vars.push(reference);
            }
        });
        vars.sort_by_key(|reference| reference.reg_index());

        loops.push(GenericForLoop {
            entry,
            tfor_block: block.index,
            latch_block,
            body_start,
            exit,
            base,
            count,
            tfor_node: NodeId {
                block: block.index,
                node: node_index,
            },
            call_node: find_iterator_call(function, entry, block.index, base),
            vars,
        });
    }

    loops
}

fn natural_loops(function: &SsaFunction, pc_map: &[Option<usize>]) -> Vec<NaturalLoop> {
    let mut loops = Vec::new();

    for block in &function.blocks {
        if block_has_for_loop(function, block.index)
            || block_is_generic_latch(function, block.index)
        {
            continue;
        }
        for &succ in &block.succs {
            if block_has_for_loop(function, block.index)
                || block_is_generic_latch(function, block.index)
                || !function.dom.dominates(succ, block.index)
            {
                continue;
            }
            let blocks = natural_loop_blocks(function, succ, block.index);
            let exit = loop_exit(function, &blocks);
            let kind = classify_natural(function, pc_map, succ, block.index, &blocks, exit);
            loops.push(NaturalLoop {
                header: succ,
                latch: block.index,
                blocks,
                exit,
                kind,
            });
        }
    }

    loops.sort_by_key(|info| (info.header, info.latch));
    loops.dedup_by_key(|info| (info.header, info.latch));
    loops
}

fn natural_loop_blocks(function: &SsaFunction, header: usize, latch: usize) -> BTreeSet<usize> {
    let mut set = BTreeSet::new();
    let mut queue = VecDeque::new();
    set.insert(header);
    set.insert(latch);
    queue.push_back(latch);

    while let Some(block) = queue.pop_front() {
        for &pred in &function.blocks[block].preds {
            if set.insert(pred) {
                queue.push_back(pred);
            }
        }
    }

    set
}

fn loop_exit(function: &SsaFunction, blocks: &BTreeSet<usize>) -> Option<usize> {
    let mut exits = blocks
        .iter()
        .filter_map(|block| function.blocks.get(*block))
        .flat_map(|block| block.succs.iter().copied())
        .filter(|succ| !blocks.contains(succ))
        .map(|succ| follow_jmp_only(function, succ, None))
        .collect::<BTreeSet<_>>();

    if exits.len() == 1 {
        exits.pop_first()
    } else {
        exits.pop_last()
    }
}

fn classify_natural(
    function: &SsaFunction,
    pc_map: &[Option<usize>],
    header: usize,
    latch: usize,
    blocks: &BTreeSet<usize>,
    exit: Option<usize>,
) -> NaturalLoopKind {
    let header_branch = branch_info(function, header, pc_map);
    let exits_from_header = header_branch.is_some_and(|branch| {
        !blocks.contains(&branch.true_block) || !blocks.contains(&branch.false_block)
    });
    let exits_from_header_chain =
        header_condition_chain_exits_loop(function, pc_map, header, blocks, exit);

    let header_contains_repeat_tail = (header == latch
        && has_tail_body_before_branch(function, header))
        || has_repeat_tail_update_before_branch(function, header);
    if (exits_from_header || exits_from_header_chain) && !header_contains_repeat_tail {
        NaturalLoopKind::While
    } else {
        let tail = blocks
            .iter()
            .copied()
            .rev()
            .find(|block| {
                branch_info(function, *block, pc_map).is_some_and(|branch| {
                    Some(branch.true_block) == exit
                        || Some(branch.false_block) == exit
                        || branch.true_block == header
                        || branch.false_block == header
                        || branch.true_block == latch
                        || branch.false_block == latch
                })
            })
            .unwrap_or(latch);
        NaturalLoopKind::Repeat { tail }
    }
}

fn header_condition_chain_exits_loop(
    function: &SsaFunction,
    pc_map: &[Option<usize>],
    header: usize,
    blocks: &BTreeSet<usize>,
    exit: Option<usize>,
) -> bool {
    let Some(exit) = exit else {
        return false;
    };
    let mut pending = VecDeque::from([header]);
    let mut visited = BTreeSet::new();
    let mut saw_body = false;
    let mut saw_exit = false;

    while let Some(block) = pending.pop_front() {
        if !visited.insert(block) || visited.len() > 32 {
            continue;
        }
        let Some(branch) = branch_info(function, block, pc_map) else {
            continue;
        };

        for target in [branch.true_block, branch.false_block] {
            let target = follow_jmp_only(function, target, Some(exit));
            if target == exit || !blocks.contains(&target) {
                saw_exit = true;
            } else if target != header && is_pure_condition_block(function, target) {
                pending.push_back(target);
            } else {
                saw_body = true;
            }
        }
    }

    visited.len() > 1 && saw_body && saw_exit
}

fn has_repeat_tail_update_before_branch(function: &SsaFunction, block: usize) -> bool {
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
    let branch_regs = branch_operand_regs(branch);
    let phi_regs = block
        .nodes
        .iter()
        .filter(|node| matches!(node.op, SsaOp::Phi { .. }))
        .filter_map(|node| node.dest.reg_index())
        .collect::<BTreeSet<_>>();

    block
        .nodes
        .iter()
        .take(branch_index)
        .filter(|node| !matches!(node.op, SsaOp::Phi { .. }))
        .filter_map(|node| node.dest.reg_index())
        .any(|dest| branch_regs.contains(&dest) && phi_regs.contains(&dest))
}

pub(crate) fn has_tail_body_before_branch(function: &SsaFunction, block: usize) -> bool {
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
    let branch_regs = branch_operand_regs(branch);

    block.nodes.iter().take(branch_index).any(|node| {
        if let Some(dest) = node.dest.reg_index()
            && branch_regs.contains(&dest)
            && node_uses_reg(node, dest)
        {
            return true;
        }
        let feeds_branch = node
            .dest
            .reg_index()
            .is_some_and(|dest| branch_regs.contains(&dest))
            || branch_regs.iter().any(|reg| node_uses_reg(node, *reg));
        if feeds_branch {
            return false;
        }
        if matches!(
            node.op,
            SsaOp::Call { .. }
                | SsaOp::TailCall { .. }
                | SsaOp::SetGlobal { .. }
                | SsaOp::SetUpval { .. }
                | SsaOp::SetTable { .. }
                | SsaOp::SetList { .. }
        ) {
            return true;
        }
        false
    })
}

fn branch_operand_regs(node: &crate::ir::SsaNode) -> Vec<u16> {
    let SsaOp::Branch { a, b, .. } = node.op else {
        return Vec::new();
    };
    [a.reg_index(), b.reg_index()]
        .into_iter()
        .flatten()
        .collect()
}

fn node_uses_reg(node: &crate::ir::SsaNode, reg: u16) -> bool {
    match &node.op {
        SsaOp::Move { src } | SsaOp::UnOp { value: src, .. } => src.reg_index() == Some(reg),
        SsaOp::BinOp { left, right, .. } => {
            left.reg_index() == Some(reg) || right.reg_index() == Some(reg)
        }
        SsaOp::Concat { operands } => operands
            .iter()
            .any(|operand| operand.reg_index() == Some(reg)),
        _ => false,
    }
}

fn block_has_for_loop(function: &SsaFunction, block: usize) -> bool {
    function.blocks.get(block).is_some_and(|block| {
        block
            .nodes
            .iter()
            .any(|node| matches!(node.op, SsaOp::ForLoop { .. } | SsaOp::TForLoop { .. }))
    })
}

fn block_is_generic_latch(function: &SsaFunction, block: usize) -> bool {
    function.blocks.get(block).is_some_and(|block| {
        block.preds.iter().copied().any(|pred| {
            function.blocks.get(pred).is_some_and(|pred| {
                pred.nodes
                    .iter()
                    .any(|node| matches!(node.op, SsaOp::TForLoop { .. }))
            })
        })
    })
}

fn def_in_block(
    function: &SsaFunction,
    block: usize,
    reg: u16,
    before_node: Option<usize>,
) -> Option<NodeId> {
    let block_ref = function.blocks.get(block)?;
    let end = before_node.unwrap_or(block_ref.nodes.len());
    block_ref
        .nodes
        .iter()
        .take(end)
        .enumerate()
        .rev()
        .find(|(_, node)| node.dest.reg_index() == Some(reg))
        .map(|(node, _)| NodeId { block, node })
}

fn reaching_def(
    function: &SsaFunction,
    block: usize,
    reg: u16,
    before_node: Option<usize>,
) -> Option<NodeId> {
    reaching_def_inner(function, block, reg, before_node, &mut BTreeSet::new())
}

fn reaching_def_inner(
    function: &SsaFunction,
    block: usize,
    reg: u16,
    before_node: Option<usize>,
    visiting: &mut BTreeSet<(usize, u16, usize)>,
) -> Option<NodeId> {
    let visit_key = (block, reg, before_node.unwrap_or(usize::MAX));
    if !visiting.insert(visit_key) {
        return None;
    }
    if let Some(def) = def_in_block(function, block, reg, before_node) {
        visiting.remove(&visit_key);
        return Some(def);
    }

    let block_ref = function.blocks.get(block)?;
    let mut agreed = None;
    for pred in block_ref.preds.iter().copied() {
        if pred == block {
            continue;
        }
        let pred_def = reaching_def_inner(function, pred, reg, None, visiting)?;
        if agreed.is_some_and(|current| current != pred_def) {
            visiting.remove(&visit_key);
            return None;
        }
        agreed = Some(pred_def);
    }

    visiting.remove(&visit_key);
    agreed
}

fn find_iterator_call(
    function: &SsaFunction,
    entry: usize,
    tfor_block: usize,
    base: u16,
) -> Option<NodeId> {
    (entry..=tfor_block).rev().find_map(|block| {
        let block_ref = function.blocks.get(block)?;
        block_ref
            .nodes
            .iter()
            .enumerate()
            .rev()
            .find(|(_, node)| {
                matches!(
                    node.op,
                    SsaOp::Call {
                        base: call_base, ..
                    } if call_base == base
                )
            })
            .map(|(node, _)| NodeId { block, node })
    })
}
