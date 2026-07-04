//! Control-flow graph construction.

use crate::bytecode::{Instruction, opinfo};

use super::BasicBlock;

/// Build basic blocks and CFG edges from decoded instructions.
#[must_use]
pub fn build_cfg(instructions: &[Instruction]) -> Vec<BasicBlock> {
    if instructions.is_empty() {
        return Vec::new();
    }

    let leaders = detect_leaders(instructions);
    let pc_map = block_map(&leaders);
    let mut blocks = partition_blocks(&leaders, &pc_map);
    add_edges(instructions, &pc_map, &mut blocks);
    prune_unreachable(blocks)
}

/// Detect block leaders.
#[must_use]
pub fn detect_leaders(instructions: &[Instruction]) -> Vec<bool> {
    let mut leaders = vec![false; instructions.len()];
    leaders[0] = true;
    for (pc, inst) in instructions.iter().copied().enumerate() {
        let info = opinfo::info_for(inst.op);
        for leader in info.leader_pcs(pc, inst, instructions.len()) {
            leaders[leader] = true;
        }
    }
    leaders
}

fn block_map(leaders: &[bool]) -> Vec<Option<usize>> {
    let mut pc_map = vec![None; leaders.len()];
    let mut current = None;
    let mut next_block = 0;
    for (pc, is_leader) in leaders.iter().copied().enumerate() {
        if is_leader {
            current = Some(next_block);
            next_block += 1;
        }
        pc_map[pc] = current;
    }
    pc_map
}

fn partition_blocks(leaders: &[bool], pc_map: &[Option<usize>]) -> Vec<BasicBlock> {
    let count = leaders.iter().filter(|leader| **leader).count();
    let mut ranges = vec![(usize::MAX, 0); count];
    for (pc, block) in pc_map.iter().copied().enumerate() {
        let Some(block) = block else {
            continue;
        };
        let range = &mut ranges[block];
        range.0 = range.0.min(pc);
        range.1 = pc;
    }
    ranges
        .into_iter()
        .enumerate()
        .map(|(index, (start, end))| BasicBlock::new(index, start, end))
        .collect()
}

fn add_edges(instructions: &[Instruction], pc_map: &[Option<usize>], blocks: &mut [BasicBlock]) {
    for block_index in 0..blocks.len() {
        let end_pc = blocks[block_index].end_pc;
        let inst = instructions[end_pc];
        let info = opinfo::info_for(inst.op);
        for succ_pc in info.successor_pcs(end_pc, inst, instructions.len()) {
            if let Some(succ) = pc_to_block(pc_map, succ_pc) {
                add_succ(blocks, block_index, succ);
            }
        }
    }
}

fn pc_to_block(pc_map: &[Option<usize>], pc: usize) -> Option<usize> {
    pc_map.get(pc).copied().flatten()
}

fn add_succ(blocks: &mut [BasicBlock], src: usize, dst: usize) {
    if src == dst {
        if !blocks[src].succs.contains(&dst) {
            blocks[src].succs.push(dst);
        }
        if !blocks[src].preds.contains(&src) {
            blocks[src].preds.push(src);
        }
        return;
    }

    if !blocks[src].succs.contains(&dst) {
        blocks[src].succs.push(dst);
    }
    if !blocks[dst].preds.contains(&src) {
        blocks[dst].preds.push(src);
    }
}

fn prune_unreachable(blocks: Vec<BasicBlock>) -> Vec<BasicBlock> {
    if blocks.is_empty() {
        return blocks;
    }

    let mut reachable = vec![false; blocks.len()];
    let mut stack = vec![0];
    reachable[0] = true;
    while let Some(block) = stack.pop() {
        for &succ in &blocks[block].succs {
            if !reachable[succ] {
                reachable[succ] = true;
                stack.push(succ);
            }
        }
    }

    if reachable.iter().all(|block| *block) {
        return blocks;
    }

    let mut remap = vec![None; blocks.len()];
    let mut next = 0;
    for (old, is_reachable) in reachable.iter().copied().enumerate() {
        if is_reachable {
            remap[old] = Some(next);
            next += 1;
        }
    }

    let mut pruned = blocks
        .into_iter()
        .enumerate()
        .filter_map(|(old, mut block)| {
            let new_index = remap[old]?;
            block.index = new_index;
            block.succs = block
                .succs
                .into_iter()
                .filter_map(|succ| remap[succ])
                .collect();
            block.preds = block
                .preds
                .into_iter()
                .filter_map(|pred| remap[pred])
                .collect();
            Some(block)
        })
        .collect::<Vec<_>>();

    for block in &mut pruned {
        block.succs.sort_unstable();
        block.succs.dedup();
        block.preds.sort_unstable();
        block.preds.dedup();
    }
    pruned
}
