use std::collections::BTreeSet;

use crate::{
    LuaError,
    decompile::{
        analysis::NodeId,
        boolean::{BooleanAnalysis, ConditionChain},
        region::LinearRegion,
    },
    ir::SsaFunction,
};

use super::super::{
    conditionals::{self, BranchInfo},
    loops::{
        GenericForLoop, LoopAnalysis, NaturalLoop, NaturalLoopKind, NumericForLoop,
        has_tail_body_before_branch,
    },
};
use super::types::{
    BlockSet, Condition, GenericForRegion, IfArm, IfRegion, NumericForRegion, Region, RegionTree,
    RepeatRegion, WhileRegion,
};

mod branch_specials;

impl RegionTree {
    pub fn build(
        function: &SsaFunction,
        loops: &LoopAnalysis,
        pc_map: &[Option<usize>],
        booleans: &BooleanAnalysis,
    ) -> Result<Self, LuaError> {
        let mut structurer = Structurer::new(function, loops, pc_map, booleans);
        let root = structurer.build_sequence(0, None, None)?;
        Ok(Self { root })
    }
}

const MAX_REGION_RECURSION: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceKey {
    start: usize,
    stop: Option<usize>,
    loop_exit: Option<usize>,
}

struct Structurer<'a> {
    function: &'a SsaFunction,
    loops: &'a LoopAnalysis,
    pc_map: &'a [Option<usize>],
    booleans: &'a BooleanAnalysis,
    consumed: Vec<bool>,
    loop_headers: BlockSet,
    active_sequences: Vec<SequenceKey>,
    active_natural_headers: Vec<usize>,
}

impl<'a> Structurer<'a> {
    fn new(
        function: &'a SsaFunction,
        loops: &'a LoopAnalysis,
        pc_map: &'a [Option<usize>],
        booleans: &'a BooleanAnalysis,
    ) -> Self {
        Self {
            function,
            loops,
            pc_map,
            booleans,
            consumed: vec![false; function.blocks.len()],
            loop_headers: loops.loop_headers(function.blocks.len()),
            active_sequences: Vec::new(),
            active_natural_headers: Vec::new(),
        }
    }

    fn build_sequence(
        &mut self,
        start: usize,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Result<Region, LuaError> {
        let key = SequenceKey {
            start,
            stop,
            loop_exit,
        };
        if self.active_sequences.len() >= MAX_REGION_RECURSION
            || self.active_sequences.contains(&key)
        {
            return Err(LuaError::Unsupported(format!(
                "cyclic control-flow region while structuring blocks {start}..{stop:?}"
            )));
        }
        self.active_sequences.push(key);
        let result = self.build_sequence_inner(start, stop, loop_exit);
        self.active_sequences.pop();
        result
    }

    fn build_sequence_inner(
        &mut self,
        start: usize,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Result<Region, LuaError> {
        let mut regions = Vec::new();
        let mut current = start;

        while current < self.function.blocks.len() && stop.is_none_or(|stop| current < stop) {
            if self.consumed[current] {
                current += 1;
                continue;
            }

            if self
                .booleans
                .value_select_covering(current)
                .is_some_and(|plan| plan.start != current && Some(plan.merge) != stop)
            {
                self.consumed[current] = true;
                current += 1;
                continue;
            }

            if self.is_break_block(current, loop_exit) {
                self.consumed[current] = true;
                regions.push(Region::Break);
                current += 1;
                continue;
            }

            if let Some(info) = self.loops.numeric_at(current).cloned() {
                let next = info.exit;
                regions.push(self.numeric_for_region(info)?);
                current = next;
                continue;
            }

            if let Some(info) = self.loops.generic_at_entry(current).cloned() {
                let next = info.exit;
                regions.push(self.generic_for_region(info)?);
                current = next;
                continue;
            }

            if !self.active_natural_headers.contains(&current)
                && let Some(info) = self.loops.natural_at(current).cloned()
            {
                let next = info.exit.unwrap_or_else(|| max_block(&info.blocks) + 1);
                match info.kind {
                    NaturalLoopKind::While => regions.push(self.while_region(&info)?),
                    NaturalLoopKind::Repeat { tail } => {
                        regions.push(self.repeat_region(&info, tail)?);
                    }
                }
                current = next;
                continue;
            }

            if let Some(plan) = self.booleans.value_select_start(current)
                && stop.is_none_or(|stop| plan.merge <= stop)
                && Some(plan.merge) != stop
                && plan.merge > current
            {
                for block in plan.consumed_blocks() {
                    if let Some(slot) = self.consumed.get_mut(block) {
                        *slot = true;
                    }
                }
                regions.push(Region::Linear(linear_block_covering(
                    self.function,
                    current,
                    plan.consumed_blocks(),
                )));
                current = plan.merge;
                continue;
            }

            if let Some(chain) = self.booleans.condition_chain(current).cloned()
                && stop.is_none_or(|stop| chain.merge <= stop)
                && chain.merge > current
            {
                let (region, next) = self.compound_if_region(chain, stop, loop_exit)?;
                regions.push(region);
                current = next;
                continue;
            }

            if let Some(branch) = conditionals::branch_info(self.function, current, self.pc_map) {
                let (region, next) = self.if_region(current, branch, stop, loop_exit)?;
                regions.push(region);
                current = next;
                continue;
            }

            let linear_block_index = current;
            self.consumed[linear_block_index] = true;
            regions.push(Region::Linear(linear_block(
                self.function,
                linear_block_index,
            )));
            if conditionals::is_terminal_block(self.function, linear_block_index) {
                break;
            }
            current = self.next_linear_block(current, stop);
        }

        Ok(Region::Sequence(regions))
    }

    fn numeric_for_region(&mut self, info: NumericForLoop) -> Result<Region, LuaError> {
        self.consumed[info.prep] = true;
        self.consumed[info.loop_block] = true;
        let body = self.build_sequence(info.body_start, Some(info.loop_block), Some(info.exit))?;
        Ok(Region::NumericFor(Box::new(NumericForRegion {
            prefix: linear_block(self.function, info.prep),
            info,
            body,
        })))
    }

    fn generic_for_region(&mut self, info: GenericForLoop) -> Result<Region, LuaError> {
        self.consumed[info.entry] = true;
        self.consumed[info.tfor_block] = true;
        self.consumed[info.latch_block] = true;
        let body = self.build_sequence(info.body_start, Some(info.tfor_block), Some(info.exit))?;
        Ok(Region::GenericFor(Box::new(GenericForRegion {
            prefix: linear_block(self.function, info.entry),
            info,
            body,
        })))
    }

    fn while_region(&mut self, info: &NaturalLoop) -> Result<Region, LuaError> {
        self.consumed[info.header] = true;
        if let Some(chain) = self.booleans.condition_chain(info.header).cloned()
            && info.blocks.contains(&chain.body)
            && info
                .exit
                .is_some_and(|exit| self.leads_to_exit(chain.false_target, exit))
            && chain
                .blocks
                .iter()
                .copied()
                .skip(1)
                .all(|block| !has_tail_body_before_branch(self.function, block))
        {
            for block in &chain.blocks {
                if let Some(slot) = self.consumed.get_mut(*block) {
                    *slot = true;
                }
            }
            let stop = Some(max_block(&info.blocks) + 1);
            self.active_natural_headers.push(info.header);
            let body = self.build_sequence(chain.body, stop, info.exit);
            self.active_natural_headers.pop();
            let body = body?;
            let branch = chain.segments.first().map_or(
                NodeId {
                    block: info.header,
                    node: 0,
                },
                |segment| segment.node,
            );
            return Ok(Region::While(Box::new(WhileRegion {
                prefix: linear_block(self.function, info.header),
                condition: Condition {
                    branch,
                    inverted: false,
                    compound: Some(info.header),
                },
                body,
                exit: info.exit,
            })));
        }

        let branch = conditionals::branch_info(self.function, info.header, self.pc_map);
        let Some(branch) = branch else {
            return Ok(Region::While(Box::new(WhileRegion {
                prefix: linear_block(self.function, info.header),
                condition: Condition {
                    branch: NodeId {
                        block: info.header,
                        node: 0,
                    },
                    inverted: false,
                    compound: None,
                },
                body: Region::Sequence(Vec::new()),
                exit: info.exit,
            })));
        };
        let true_block = conditionals::follow_jmp_only(self.function, branch.true_block, info.exit);
        let false_block =
            conditionals::follow_jmp_only(self.function, branch.false_block, info.exit);
        let body_start = if info.blocks.contains(&true_block) {
            true_block
        } else {
            false_block
        };
        let inverted = body_start != true_block;
        let stop = Some(max_block(&info.blocks) + 1);
        self.active_natural_headers.push(info.header);
        let body = self.build_sequence(body_start, stop, info.exit);
        self.active_natural_headers.pop();
        let body = body?;
        Ok(Region::While(Box::new(WhileRegion {
            prefix: linear_block(self.function, info.header),
            condition: Condition {
                branch: branch.node,
                inverted,
                compound: None,
            },
            body,
            exit: info.exit,
        })))
    }

    fn repeat_region(&mut self, info: &NaturalLoop, tail: usize) -> Result<Region, LuaError> {
        let mut parts = Vec::new();
        if info.header < tail {
            self.active_natural_headers.push(info.header);
            let body = self.build_sequence(info.header, Some(tail), info.exit);
            self.active_natural_headers.pop();
            parts.push(body?);
        }
        if !self.consumed.get(tail).copied().unwrap_or(true) {
            self.consumed[tail] = true;
            parts.push(Region::Linear(linear_block(self.function, tail)));
        }
        let branch = conditionals::branch_info(self.function, tail, self.pc_map);
        let condition = branch.map_or(
            Condition {
                branch: NodeId {
                    block: tail,
                    node: 0,
                },
                inverted: false,
                compound: None,
            },
            |branch| {
                let true_target =
                    conditionals::follow_jmp_only(self.function, branch.true_block, info.exit);
                Condition {
                    branch: branch.node,
                    inverted: Some(true_target) != info.exit,
                    compound: None,
                }
            },
        );
        Ok(Region::Repeat(Box::new(RepeatRegion {
            body: Region::Sequence(parts),
            condition,
            exit: info.exit,
        })))
    }

    fn compound_if_region(
        &mut self,
        chain: ConditionChain,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Result<(Region, usize), LuaError> {
        let merge = self.enclosing_stop_merge(chain.body, chain.false_target, chain.merge, stop);
        for block in &chain.blocks {
            if let Some(slot) = self.consumed.get_mut(*block) {
                *slot = true;
            }
        }

        let true_body = self.build_sequence(chain.body, Some(merge), loop_exit)?;
        let true_blocks = true_body.blocks();
        let else_ = if chain.false_target != merge
            && chain.false_target < self.function.blocks.len()
            && !conditionals::is_empty_structural(self.function, chain.false_target)
        {
            Some(self.build_sequence(chain.false_target, Some(merge), loop_exit)?)
        } else {
            None
        };
        let else_blocks = else_.as_ref().map_or_else(Vec::new, Region::blocks);
        let phis = if merge < self.function.blocks.len() {
            conditionals::phi_sources(self.function, merge)
        } else {
            Vec::new()
        };

        Ok((
            Region::If(Box::new(IfRegion {
                prefix: linear_block(self.function, chain.start),
                arms: vec![IfArm {
                    condition: Condition {
                        branch: chain.segments[0].node,
                        inverted: false,
                        compound: Some(chain.start),
                    },
                    body: true_body,
                    blocks: true_blocks,
                }],
                else_,
                else_blocks,
                merge: (merge < self.function.blocks.len()).then_some(merge),
                phis,
            })),
            merge,
        ))
    }

    fn if_region(
        &mut self,
        block: usize,
        branch: BranchInfo,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Result<(Region, usize), LuaError> {
        if let Some((region, next)) = self.break_if_region(block, branch, loop_exit)? {
            return Ok((region, next));
        }
        if let Some((region, next)) =
            self.loop_continue_if_region(block, branch, stop, loop_exit)?
        {
            return Ok((region, next));
        }
        if let Some((region, next)) =
            self.terminal_guard_if_region(block, branch, stop, loop_exit)?
        {
            return Ok((region, next));
        }

        self.consumed[block] = true;
        let mut true_block = conditionals::follow_jmp_only(self.function, branch.true_block, stop);
        let mut false_block =
            conditionals::follow_jmp_only(self.function, branch.false_block, stop);
        let mut merge = conditionals::find_merge(self.function, block, true_block, false_block)
            .unwrap_or_else(|| block.saturating_add(1));
        if let Some(stop) = stop
            && merge > stop
        {
            merge = stop;
        }
        merge = self.enclosing_stop_merge(true_block, false_block, merge, stop);
        true_block = conditionals::follow_jmp_only(self.function, true_block, Some(merge));
        false_block = conditionals::follow_jmp_only(self.function, false_block, Some(merge));

        let true_body = if true_block == merge {
            Region::Sequence(Vec::new())
        } else {
            self.build_sequence(true_block, Some(merge), loop_exit)?
        };
        let true_blocks = true_body.blocks();
        let mut arms = vec![IfArm {
            condition: Condition {
                branch: branch.node,
                inverted: false,
                compound: None,
            },
            body: true_body,
            blocks: true_blocks,
        }];

        let mut else_start = false_block;
        while else_start < self.function.blocks.len()
            && !self.consumed[else_start]
            && self.is_elseif_start(else_start, merge)
        {
            if let Some(chain) = self.booleans.condition_chain(else_start).cloned()
                && let Some(next) =
                    self.push_compound_elseif_arm(&mut arms, chain, &mut merge, stop, loop_exit)?
            {
                else_start = next;
                continue;
            }

            let Some(elseif_branch) =
                conditionals::branch_info(self.function, else_start, self.pc_map)
            else {
                break;
            };
            let next_merge = conditionals::find_merge(
                self.function,
                else_start,
                elseif_branch.true_block,
                elseif_branch.false_block,
            );
            let mut next_merge = next_merge.unwrap_or(merge);
            if let Some(stop) = stop
                && next_merge > stop
            {
                next_merge = stop;
            }
            let extends_terminal_chain = else_start == merge && next_merge > merge;
            let shares_existing_merge = self.branch_exits_or_reaches_merge(
                elseif_branch.true_block,
                elseif_branch.false_block,
                merge,
                stop,
            );
            if next_merge != merge && !extends_terminal_chain && !shares_existing_merge {
                break;
            }
            if extends_terminal_chain {
                merge = next_merge;
            }
            self.consumed[else_start] = true;
            let arm_true =
                conditionals::follow_jmp_only(self.function, elseif_branch.true_block, Some(merge));
            let body = if arm_true == merge {
                Region::Sequence(Vec::new())
            } else {
                self.build_sequence(arm_true, Some(merge), loop_exit)?
            };
            let blocks = body.blocks();
            arms.push(IfArm {
                condition: Condition {
                    branch: elseif_branch.node,
                    inverted: false,
                    compound: None,
                },
                body,
                blocks,
            });
            else_start = conditionals::follow_jmp_only(
                self.function,
                elseif_branch.false_block,
                Some(merge),
            );
        }

        let else_ = if else_start != merge
            && else_start < self.function.blocks.len()
            && !conditionals::is_empty_structural(self.function, else_start)
        {
            if self.consumed[else_start]
                && conditionals::is_terminal_block(self.function, else_start)
            {
                Some(Region::Linear(linear_block(self.function, else_start)))
            } else {
                Some(self.build_sequence(else_start, Some(merge), loop_exit)?)
            }
        } else {
            None
        };
        let else_blocks = else_.as_ref().map_or_else(Vec::new, Region::blocks);
        let phis = if merge < self.function.blocks.len() {
            conditionals::phi_sources(self.function, merge)
        } else {
            Vec::new()
        };

        Ok((
            Region::If(Box::new(IfRegion {
                prefix: linear_block(self.function, block),
                arms,
                else_,
                else_blocks,
                merge: (merge < self.function.blocks.len()).then_some(merge),
                phis,
            })),
            merge,
        ))
    }

    fn push_compound_elseif_arm(
        &mut self,
        arms: &mut Vec<IfArm>,
        chain: ConditionChain,
        merge: &mut usize,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Result<Option<usize>, LuaError> {
        if chain.segments.is_empty()
            || chain
                .blocks
                .iter()
                .any(|block| *block >= self.function.blocks.len() || self.consumed[*block])
        {
            return Ok(None);
        }

        let mut next_merge =
            self.enclosing_stop_merge(chain.body, chain.false_target, chain.merge, stop);
        if let Some(stop) = stop
            && next_merge > stop
        {
            next_merge = stop;
        }
        let extends_terminal_chain = chain.start == *merge && next_merge > *merge;
        let shares_existing_merge =
            self.branch_exits_or_reaches_merge(chain.body, chain.false_target, *merge, stop);
        if next_merge != *merge && !extends_terminal_chain && !shares_existing_merge {
            return Ok(None);
        }
        if extends_terminal_chain {
            *merge = next_merge;
        }

        for block in &chain.blocks {
            self.consumed[*block] = true;
        }
        let body = if chain.body == *merge {
            Region::Sequence(Vec::new())
        } else {
            self.build_sequence(chain.body, Some(*merge), loop_exit)?
        };
        let blocks = body.blocks();
        arms.push(IfArm {
            condition: Condition {
                branch: chain.segments[0].node,
                inverted: false,
                compound: Some(chain.start),
            },
            body,
            blocks,
        });

        let next = conditionals::follow_jmp_only(self.function, chain.false_target, Some(*merge));
        Ok(Some(next))
    }

    fn enclosing_stop_merge(
        &self,
        true_start: usize,
        false_start: usize,
        current_merge: usize,
        stop: Option<usize>,
    ) -> usize {
        let Some(stop) = stop else {
            return current_merge;
        };
        if stop >= self.function.blocks.len() || stop <= current_merge {
            return current_merge;
        }

        let true_terminal = self.terminal_path_end(true_start, Some(stop));
        let false_terminal = self.terminal_path_end(false_start, Some(stop));
        match (true_terminal, false_terminal) {
            (Some(terminal), None) => self
                .stop_merge_for_terminal_sibling(false_start, terminal, current_merge, stop)
                .unwrap_or(current_merge),
            (None, Some(terminal)) => self
                .stop_merge_for_terminal_sibling(true_start, terminal, current_merge, stop)
                .unwrap_or(current_merge),
            _ => current_merge,
        }
    }

    fn stop_merge_for_terminal_sibling(
        &self,
        continuation_start: usize,
        terminal: usize,
        current_merge: usize,
        stop: usize,
    ) -> Option<usize> {
        let continuation =
            conditionals::follow_jmp_only(self.function, continuation_start, Some(stop));
        let merge_is_terminal = terminal == current_merge;
        let merge_is_lua_else_body = current_merge == continuation
            && (terminal > continuation
                || conditionals::has_unreachable_jump_immediately_before(
                    self.function,
                    continuation,
                ));
        if (merge_is_terminal || merge_is_lua_else_body)
            && conditionals::can_reach(self.function, continuation, stop)
        {
            Some(stop)
        } else {
            None
        }
    }

    fn is_elseif_start(&self, block: usize, merge: usize) -> bool {
        if block == merge
            && !conditionals::has_unreachable_jump_immediately_before(self.function, block)
        {
            return false;
        }
        if self.booleans.value_select_start(block).is_some() {
            return false;
        }
        conditionals::is_elseif_candidate(
            self.function,
            block,
            if block == merge { usize::MAX } else { merge },
            self.pc_map,
            &self.loop_headers,
        )
    }

    fn branch_exits_or_reaches_merge(
        &self,
        true_target: usize,
        false_target: usize,
        merge: usize,
        stop: Option<usize>,
    ) -> bool {
        self.arm_exits_or_reaches_merge(true_target, merge, stop)
            && self.arm_exits_or_reaches_merge(false_target, merge, stop)
    }

    fn arm_exits_or_reaches_merge(&self, target: usize, merge: usize, stop: Option<usize>) -> bool {
        let start = conditionals::follow_jmp_only(self.function, target, Some(merge));
        start == merge
            || conditionals::can_reach(self.function, start, merge)
            || self.terminal_path_end(target, stop).is_some()
    }

    fn terminal_path_end(&self, start: usize, stop: Option<usize>) -> Option<usize> {
        let end = conditionals::follow_jmp_only(self.function, start, stop);
        conditionals::is_terminal_block(self.function, end).then_some(end)
    }

    fn leads_to_exit(&self, block: usize, exit: usize) -> bool {
        block == exit || conditionals::follow_jmp_only(self.function, block, Some(exit)) == exit
    }

    fn next_linear_block(&self, block: usize, stop: Option<usize>) -> usize {
        let block_ref = &self.function.blocks[block];
        if let [succ] = block_ref.succs.as_slice()
            && *succ > block
            && stop.is_none_or(|stop| *succ <= stop)
        {
            return *succ;
        }
        block + 1
    }
}

fn linear_block(function: &SsaFunction, block: usize) -> LinearRegion {
    let nodes = function
        .blocks
        .get(block)
        .map(|block_ref| {
            (0..block_ref.nodes.len())
                .map(|node| NodeId { block, node })
                .collect()
        })
        .unwrap_or_default();
    LinearRegion {
        nodes,
        covered_blocks: vec![block],
    }
}

fn linear_block_covering(
    function: &SsaFunction,
    block: usize,
    covered_blocks: impl IntoIterator<Item = usize>,
) -> LinearRegion {
    let mut region = linear_block(function, block);
    region.covered_blocks = covered_blocks.into_iter().collect();
    region
}

fn max_block(blocks: &BTreeSet<usize>) -> usize {
    blocks.iter().next_back().copied().unwrap_or(0)
}
