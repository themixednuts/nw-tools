use std::collections::BTreeSet;

use crate::{LuaError, decompile::control_flow::conditionals};

use super::{
    super::types::{Condition, IfArm, IfRegion, Region},
    BranchInfo, Structurer,
};

/// A branch whose continuation has source-level meaning beyond a regular
/// two-arm `if`. Classification is immutable; lowering owns all mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchRegionPlan {
    LoopBreak {
        body_start: usize,
        break_end: Option<usize>,
        continuation: usize,
        inverted: bool,
        exit: usize,
    },
    LoopContinue {
        body_start: usize,
        continue_target: usize,
        inverted: bool,
        stop: usize,
    },
    TerminalGuard {
        body_start: usize,
        body_stop: usize,
        continuation: usize,
        inverted: bool,
    },
    FinalEmptySibling {
        body_start: usize,
        empty_start: usize,
        empty_end: usize,
        inverted: bool,
    },
}

impl<'a> Structurer<'a> {
    pub(super) fn special_branch_region(
        &mut self,
        block: usize,
        branch: BranchInfo,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Result<Option<(Region, usize)>, LuaError> {
        let Some(plan) = self.classify_special_branch(block, branch, stop, loop_exit) else {
            return Ok(None);
        };
        self.lower_special_branch(block, branch, plan, loop_exit)
            .map(Some)
    }

    fn classify_special_branch(
        &self,
        block: usize,
        branch: BranchInfo,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Option<BranchRegionPlan> {
        self.loop_break_plan(branch, loop_exit)
            .or_else(|| self.loop_continue_plan(block, branch, stop, loop_exit))
            .or_else(|| self.terminal_guard_plan(block, branch, stop))
    }

    fn loop_break_plan(
        &self,
        branch: BranchInfo,
        loop_exit: Option<usize>,
    ) -> Option<BranchRegionPlan> {
        let exit = loop_exit?;
        let true_break = self.break_path_end(branch.true_block, exit);
        let false_break = self.break_path_end(branch.false_block, exit);
        if true_break.is_some() == false_break.is_some() {
            return None;
        }

        let (break_target, break_end, continuation, inverted) = if let Some(end) = true_break {
            (branch.true_block, end, branch.false_block, false)
        } else {
            (
                branch.false_block,
                false_break.expect("one branch must be a break path"),
                branch.true_block,
                true,
            )
        };
        Some(BranchRegionPlan::LoopBreak {
            body_start: conditionals::follow_jmp_only(self.function, break_target, Some(exit)),
            break_end,
            continuation: conditionals::follow_jmp_only(self.function, continuation, Some(exit)),
            inverted,
            exit,
        })
    }

    fn loop_continue_plan(
        &self,
        block: usize,
        branch: BranchInfo,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Option<BranchRegionPlan> {
        let stop = stop?;
        if loop_exit.is_none() || stop >= self.function.blocks.len() {
            return None;
        }

        let true_continue = self.leads_to_loop_continue(branch.true_block, stop);
        let false_continue = self.leads_to_loop_continue(branch.false_block, stop);
        if true_continue == false_continue {
            return None;
        }

        let (continue_target, body_target, inverted) = if true_continue {
            (branch.true_block, branch.false_block, true)
        } else {
            (branch.false_block, branch.true_block, false)
        };
        let body_start = conditionals::follow_jmp_only(self.function, body_target, Some(stop));
        (body_start > block && body_start < stop).then_some(BranchRegionPlan::LoopContinue {
            body_start,
            continue_target,
            inverted,
            stop,
        })
    }

    fn terminal_guard_plan(
        &self,
        block: usize,
        branch: BranchInfo,
        stop: Option<usize>,
    ) -> Option<BranchRegionPlan> {
        let true_end = self.terminal_path_end(branch.true_block, stop);
        let false_end = self.terminal_path_end(branch.false_block, stop);
        if let Some(plan) = self.final_empty_sibling_plan(block, branch, true_end, false_end, stop)
        {
            return Some(plan);
        }
        if true_end.is_some() == false_end.is_some() {
            return None;
        }

        let (terminal_target, terminal_end, continuation, inverted) = if let Some(end) = true_end {
            (branch.true_block, end, branch.false_block, false)
        } else {
            (
                branch.false_block,
                false_end.expect("one branch must be terminal"),
                branch.true_block,
                true,
            )
        };
        let body_start = conditionals::follow_jmp_only(self.function, terminal_target, stop);
        let continuation = conditionals::follow_jmp_only(self.function, continuation, stop);
        if continuation <= block || continuation >= self.function.blocks.len() {
            return None;
        }
        if conditionals::can_reach(self.function, continuation, terminal_end) {
            return None;
        }
        if let Some(stop) = stop
            && (continuation >= stop || body_start >= stop)
        {
            return None;
        }
        if conditionals::is_elseif_candidate(
            self.function,
            self.analysis,
            continuation,
            usize::MAX,
            self.pc_map,
            &self.loop_headers,
        ) && conditionals::has_unreachable_jump_immediately_before(self.function, continuation)
        {
            return None;
        }

        Some(BranchRegionPlan::TerminalGuard {
            body_start,
            body_stop: terminal_end.saturating_add(1),
            continuation,
            inverted,
        })
    }

    fn final_empty_sibling_plan(
        &self,
        block: usize,
        branch: BranchInfo,
        true_end: Option<usize>,
        false_end: Option<usize>,
        stop: Option<usize>,
    ) -> Option<BranchRegionPlan> {
        let true_empty =
            true_end.filter(|end| conditionals::is_final_empty_return_block(self.function, *end));
        let false_empty =
            false_end.filter(|end| conditionals::is_final_empty_return_block(self.function, *end));
        if true_empty.is_some() == false_empty.is_some() {
            return None;
        }

        let (empty_target, empty_end, body_target, inverted) = if let Some(end) = true_empty {
            (branch.true_block, end, branch.false_block, true)
        } else {
            (
                branch.false_block,
                false_empty.expect("one branch must be the final empty return"),
                branch.true_block,
                false,
            )
        };
        let empty_start = conditionals::follow_jmp_only(self.function, empty_target, stop);
        let body_start = conditionals::follow_jmp_only(self.function, body_target, Some(empty_end));
        if body_start <= block || body_start >= self.function.blocks.len() {
            return None;
        }
        if let Some(stop) = stop
            && (body_start >= stop || empty_start >= stop)
        {
            return None;
        }
        if conditionals::can_reach(self.function, body_start, empty_end)
            || !conditionals::all_paths_terminate(self.function, block, body_start)
        {
            return None;
        }

        Some(BranchRegionPlan::FinalEmptySibling {
            body_start,
            empty_start,
            empty_end,
            inverted,
        })
    }

    fn lower_special_branch(
        &mut self,
        block: usize,
        branch: BranchInfo,
        plan: BranchRegionPlan,
        loop_exit: Option<usize>,
    ) -> Result<(Region, usize), LuaError> {
        self.consumed[block] = true;
        let (body, merge, next, inverted) = match plan {
            BranchRegionPlan::LoopBreak {
                body_start,
                break_end,
                continuation,
                inverted,
                exit,
            } => {
                let mut body_parts = Vec::new();
                let break_stop = break_end.unwrap_or(exit);
                if body_start != exit && body_start != break_stop {
                    body_parts.push(self.build_sequence(
                        body_start,
                        Some(break_stop),
                        Some(exit),
                    )?);
                } else if body_start != exit && break_end.is_none() {
                    body_parts.push(self.build_sequence(body_start, Some(exit), Some(exit))?);
                }
                if let Some(block) = break_end
                    && let Some(slot) = self.consumed.get_mut(block)
                {
                    *slot = true;
                }
                body_parts.push(Region::Break);
                (
                    Region::Sequence(body_parts),
                    continuation,
                    continuation,
                    inverted,
                )
            }
            BranchRegionPlan::LoopContinue {
                body_start,
                continue_target,
                inverted,
                stop,
            } => {
                self.consume_jmp_path_to_stop(continue_target, stop);
                (
                    self.build_sequence(body_start, Some(stop), loop_exit)?,
                    stop,
                    stop,
                    inverted,
                )
            }
            BranchRegionPlan::TerminalGuard {
                body_start,
                body_stop,
                continuation,
                inverted,
            } => (
                self.build_sequence(body_start, Some(body_stop), loop_exit)?,
                continuation,
                continuation,
                inverted,
            ),
            BranchRegionPlan::FinalEmptySibling {
                body_start,
                empty_start,
                empty_end,
                inverted,
            } => {
                if let Some(slot) = self.consumed.get_mut(empty_start) {
                    *slot = true;
                }
                if let Some(slot) = self.consumed.get_mut(empty_end) {
                    *slot = true;
                }
                (
                    self.build_sequence(body_start, Some(empty_end), loop_exit)?,
                    empty_end,
                    empty_end.saturating_add(1),
                    inverted,
                )
            }
        };

        Ok((
            self.single_arm_if_region(block, branch, inverted, body, merge),
            next,
        ))
    }

    fn single_arm_if_region(
        &self,
        block: usize,
        branch: BranchInfo,
        inverted: bool,
        body: Region,
        merge: usize,
    ) -> Region {
        let blocks = body.blocks();
        Region::If(Box::new(IfRegion {
            prefix: self.linear_block(block),
            arms: vec![IfArm {
                condition: Condition {
                    branch: branch.node,
                    inverted,
                    compound: None,
                },
                body,
                blocks,
            }],
            else_: None,
            else_blocks: Vec::new(),
            merge: Some(merge),
            phis: Vec::new(),
        }))
    }

    pub(super) fn is_break_block(&self, block: usize, loop_exit: Option<usize>) -> bool {
        let Some(exit) = loop_exit else {
            return false;
        };
        conditionals::is_jmp_only(self.function, block)
            && self.function.blocks[block].succs.first().copied() == Some(exit)
    }

    fn break_path_end(&self, start: usize, exit: usize) -> Option<Option<usize>> {
        let mut current = start;
        let mut first_jump = None;
        let mut seen = BTreeSet::new();
        loop {
            if current == exit {
                return Some(first_jump);
            }
            if current >= self.function.blocks.len() || !seen.insert(current) {
                return None;
            }
            if conditionals::is_jmp_only(self.function, current) {
                first_jump.get_or_insert(current);
                current = self.function.blocks[current].succs.first().copied()?;
                continue;
            }
            if first_jump.is_some() {
                return None;
            }
            let [succ] = self.function.blocks[current].succs.as_slice() else {
                return None;
            };
            current = *succ;
        }
    }

    fn leads_to_stop(&self, start: usize, stop: usize) -> bool {
        conditionals::follow_jmp_only(self.function, start, Some(stop)) == stop
    }

    fn leads_to_loop_continue(&self, start: usize, stop: usize) -> bool {
        self.leads_to_stop(start, stop)
            || self.active_natural_headers.iter().copied().any(|header| {
                conditionals::follow_jmp_only(self.function, start, Some(stop)) == header
            })
    }

    fn consume_jmp_path_to_stop(&mut self, start: usize, stop: usize) {
        let mut current = start;
        let mut seen = BTreeSet::new();
        while current != stop
            && current < self.function.blocks.len()
            && seen.insert(current)
            && conditionals::is_jmp_only(self.function, current)
        {
            self.consumed[current] = true;
            let Some(next) = self.function.blocks[current].succs.first().copied() else {
                break;
            };
            current = next;
        }
    }
}
