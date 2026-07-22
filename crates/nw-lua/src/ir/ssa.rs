//! Phi placement and SSA renaming.

use super::{BasicBlock, SsaNode, SsaOp, SsaRef};
use crate::ir::dom::DomInfo;

/// Def-site and liveness facts computed once for phi placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefSites {
    pub blocks_by_reg: Vec<Vec<usize>>,
    pub defines: Vec<Vec<bool>>,
    pub use_before_def: Vec<Vec<bool>>,
    pub live_in: Vec<Vec<bool>>,
}

/// Compute def-sites and live-in sets once.
#[must_use]
pub fn collect_def_sites(blocks: &[BasicBlock], num_regs: usize) -> DefSites {
    let mut blocks_by_reg = vec![Vec::new(); num_regs];
    let mut defines = vec![vec![false; num_regs]; blocks.len()];
    let mut use_before_def = vec![vec![false; num_regs]; blocks.len()];

    for block in blocks {
        let mut block_defs = vec![false; num_regs];
        for node in &block.nodes {
            node.op.visit_uses(|reference, _| {
                if let Some(reg) = reg_usize(reference, num_regs)
                    && !block_defs[reg]
                {
                    use_before_def[block.index][reg] = true;
                }
            });

            node.visit_defs(|reference| {
                if let Some(reg) = reg_usize(reference, num_regs) {
                    block_defs[reg] = true;
                    defines[block.index][reg] = true;
                }
            });
        }
    }

    for (block, regs) in defines.iter().enumerate() {
        for (reg, defined) in regs.iter().copied().enumerate() {
            if defined {
                blocks_by_reg[reg].push(block);
            }
        }
    }

    let live_in = compute_live_in(blocks, &defines, &use_before_def, num_regs);
    DefSites {
        blocks_by_reg,
        defines,
        use_before_def,
        live_in,
    }
}

/// Insert Cytron phi functions using stored dominance frontiers.
pub fn insert_phi_functions(
    blocks: &mut [BasicBlock],
    num_regs: usize,
    dom: &DomInfo,
    def_sites: &DefSites,
) {
    let mut has_phi = vec![vec![false; num_regs]; blocks.len()];
    for (reg, def_blocks) in def_sites.blocks_by_reg.iter().enumerate().take(num_regs) {
        let mut worklist = def_blocks.clone();
        let mut in_worklist = vec![false; blocks.len()];
        for &block in &worklist {
            in_worklist[block] = true;
        }

        while let Some(block) = worklist.pop() {
            in_worklist[block] = false;
            for &frontier in &dom.dominance_frontiers[block] {
                if !def_sites.live_in[frontier][reg] || has_phi[frontier][reg] {
                    continue;
                }
                has_phi[frontier][reg] = true;
                insert_phi(blocks, frontier, u16::try_from(reg).unwrap_or(u16::MAX));
                if !def_sites.defines[frontier][reg] && !in_worklist[frontier] {
                    in_worklist[frontier] = true;
                    worklist.push(frontier);
                }
            }
        }
    }
}

/// Rename all register references in dominator-tree order.
pub fn rename(blocks: &mut [BasicBlock], num_regs: usize, num_params: usize, dom: &DomInfo) {
    let mut state = RenameState::new(num_regs, num_params);
    if !blocks.is_empty() {
        rename_block(blocks, 0, dom, &mut state);
    }
}

fn insert_phi(blocks: &mut [BasicBlock], block: usize, reg: u16) {
    let pc = i32::try_from(blocks[block].start_pc).unwrap_or(i32::MAX);
    let phi = SsaNode::phi(pc, reg, &blocks[block].preds);
    let pos = blocks[block]
        .nodes
        .iter()
        .take_while(|node| matches!(node.op, SsaOp::Phi { .. }))
        .count();
    blocks[block].nodes.insert(pos, phi);
}

fn compute_live_in(
    blocks: &[BasicBlock],
    defines: &[Vec<bool>],
    use_before_def: &[Vec<bool>],
    num_regs: usize,
) -> Vec<Vec<bool>> {
    let mut live_in = use_before_def.to_vec();
    let mut live_out = vec![vec![false; num_regs]; blocks.len()];
    let mut next_out = vec![false; num_regs];
    let mut changed = true;
    while changed {
        changed = false;
        for block in blocks.iter().rev() {
            next_out.fill(false);
            for &succ in &block.succs {
                for (reg, live) in live_in[succ].iter().copied().enumerate() {
                    next_out[reg] |= live;
                }
            }

            for reg in 0..num_regs {
                let input = use_before_def[block.index][reg]
                    || (next_out[reg] && !defines[block.index][reg]);
                if live_in[block.index][reg] != input {
                    live_in[block.index][reg] = input;
                    changed = true;
                }
            }

            if live_out[block.index] != next_out {
                live_out[block.index].copy_from_slice(&next_out);
                changed = true;
            }
        }
    }
    live_in
}

fn rename_block(
    blocks: &mut [BasicBlock],
    block_index: usize,
    dom: &DomInfo,
    state: &mut RenameState,
) {
    let mut defs = Vec::new();
    let node_count = blocks[block_index].nodes.len();

    for node_index in 0..node_count {
        let mut continue_after_phi = false;
        {
            let node = &mut blocks[block_index].nodes[node_index];
            if matches!(node.op, SsaOp::Phi { .. }) {
                if let Some(reg) = rename_def(state, &mut node.dest) {
                    defs.push(reg);
                }
                continue_after_phi = true;
            } else {
                node.op
                    .rewrite_uses(|reference, _| rename_use(state, reference));
                if let Some(reg) = rename_def(state, &mut node.dest) {
                    defs.push(reg);
                }
                node.rewrite_secondary_defs(|reference| {
                    if let Some(reg) = rename_def(state, reference) {
                        defs.push(reg);
                    }
                });
            }
        }
        if continue_after_phi {
            continue;
        }
    }

    fill_successor_phi_uses(blocks, block_index, state);

    for &child in &dom.dom_children[block_index] {
        rename_block(blocks, child, dom, state);
    }

    for reg in defs.into_iter().rev() {
        state.pop(reg);
    }
}

fn fill_successor_phi_uses(blocks: &mut [BasicBlock], block_index: usize, state: &RenameState) {
    let succ_count = blocks[block_index].succs.len();
    for succ_index in 0..succ_count {
        let succ = blocks[block_index].succs[succ_index];
        let Some(pred_index) = blocks[succ]
            .preds
            .iter()
            .position(|pred| *pred == block_index)
        else {
            continue;
        };

        for node in &mut blocks[succ].nodes {
            let SsaOp::Phi { operands, .. } = &mut node.op else {
                break;
            };
            if let Some(reg) = node.dest.reg_index() {
                operands[pred_index] = SsaRef::Reg {
                    reg,
                    ver: state.peek(reg),
                };
            }
        }
    }
}

fn reg_usize(reference: SsaRef, num_regs: usize) -> Option<usize> {
    let reg = usize::from(reference.reg_index()?);
    (reg < num_regs).then_some(reg)
}

fn rename_def(state: &mut RenameState, reference: &mut SsaRef) -> Option<u16> {
    let reg = reference.reg_index()?;
    let reg_index = usize::from(reg);
    if reg_index >= state.counter.len() {
        return None;
    }
    let version = state.counter[reg_index];
    state.counter[reg_index] += 1;
    state.stacks[reg_index].push(version);
    reference.set_version(version);
    Some(reg)
}

fn rename_use(state: &RenameState, reference: &mut SsaRef) {
    let Some(reg) = reference.reg_index() else {
        return;
    };
    reference.set_version(state.peek(reg));
}

#[derive(Debug, Clone)]
struct RenameState {
    counter: Vec<u32>,
    stacks: Vec<VersionStack>,
}

impl RenameState {
    fn new(num_regs: usize, num_params: usize) -> Self {
        let mut state = Self {
            counter: vec![1; num_regs],
            stacks: vec![VersionStack::default(); num_regs],
        };
        for reg in 0..num_params.min(num_regs) {
            state.stacks[reg].push(0);
        }
        state
    }

    fn peek(&self, reg: u16) -> u32 {
        self.stacks
            .get(usize::from(reg))
            .map_or(0, VersionStack::peek)
    }

    fn pop(&mut self, reg: u16) {
        if let Some(stack) = self.stacks.get_mut(usize::from(reg)) {
            stack.pop();
        }
    }
}

#[derive(Debug, Clone, Default)]
struct VersionStack {
    items: Vec<u32>,
}

impl VersionStack {
    fn push(&mut self, version: u32) {
        self.items.push(version);
    }

    fn peek(&self) -> u32 {
        self.items.last().copied().unwrap_or(0)
    }

    fn pop(&mut self) {
        self.items.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BinOp, SsaOp};

    #[test]
    fn phi_placement_adds_phi_only_for_live_join_value() {
        let mut blocks = vec![
            BasicBlock::synthetic(0, vec![1, 2], vec![]),
            BasicBlock::synthetic(1, vec![3], vec![0]),
            BasicBlock::synthetic(2, vec![3], vec![0]),
            BasicBlock::synthetic(3, vec![], vec![1, 2]),
        ];
        blocks[1].nodes.push(SsaNode::with_dest(
            1,
            -1,
            SsaRef::reg(0),
            SsaOp::LoadBool {
                value: true,
                skip_next: false,
            },
        ));
        blocks[2].nodes.push(SsaNode::with_dest(
            2,
            -1,
            SsaRef::reg(0),
            SsaOp::LoadBool {
                value: false,
                skip_next: false,
            },
        ));
        blocks[1].nodes.push(SsaNode::with_dest(
            1,
            -1,
            SsaRef::reg(1),
            SsaOp::LoadK { idx: 0 },
        ));
        blocks[3].nodes.push(SsaNode::with_dest(
            3,
            -1,
            SsaRef::reg(2),
            SsaOp::BinOp {
                op: BinOp::Add,
                left: SsaRef::reg(0),
                right: SsaRef::constant(0),
            },
        ));
        let dom = crate::ir::dom::analyze(&blocks);
        let def_sites = collect_def_sites(&blocks, 4);

        insert_phi_functions(&mut blocks, 4, &dom, &def_sites);

        let phis = blocks[3]
            .nodes
            .iter()
            .filter(|node| matches!(node.op, SsaOp::Phi { .. }))
            .collect::<Vec<_>>();
        assert_eq!(phis.len(), 1);
        assert_eq!(phis[0].dest, SsaRef::reg(0));
    }
}
