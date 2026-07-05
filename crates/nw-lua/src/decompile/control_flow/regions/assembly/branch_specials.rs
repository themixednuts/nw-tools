use std::collections::BTreeSet;

use crate::{LuaError, decompile::control_flow::conditionals};

use super::{
    super::types::{Condition, IfArm, IfRegion, Region},
    BranchInfo, Structurer, linear_block,
};

impl<'a> Structurer<'a> {
    pub(super) fn terminal_guard_if_region(
        &mut self,
        block: usize,
        branch: BranchInfo,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Result<Option<(Region, usize)>, LuaError> {
        let true_end = self.terminal_path_end(branch.true_block, stop);
        let false_end = self.terminal_path_end(branch.false_block, stop);
        if let Some(region) =
            self.final_empty_sibling_if_region(block, branch, true_end, false_end, stop, loop_exit)?
        {
            return Ok(Some(region));
        }
        if true_end.is_some() == false_end.is_some() {
            return Ok(None);
        }

        let (terminal_target, terminal_end, cont_target, inverted) = if let Some(end) = true_end {
            (branch.true_block, end, branch.false_block, false)
        } else {
            (
                branch.false_block,
                false_end.expect("one branch must be terminal"),
                branch.true_block,
                true,
            )
        };
        let terminal_start = conditionals::follow_jmp_only(self.function, terminal_target, stop);
        let cont_start = conditionals::follow_jmp_only(self.function, cont_target, stop);
        if cont_start <= block || cont_start >= self.function.blocks.len() {
            return Ok(None);
        }
        if conditionals::can_reach(self.function, cont_start, terminal_end) {
            return Ok(None);
        }
        if let Some(stop) = stop
            && (cont_start >= stop || terminal_start >= stop)
        {
            return Ok(None);
        }
        if conditionals::is_elseif_candidate(
            self.function,
            cont_start,
            usize::MAX,
            self.pc_map,
            &self.loop_headers,
        ) && conditionals::has_unreachable_jump_immediately_before(self.function, cont_start)
        {
            return Ok(None);
        }

        self.consumed[block] = true;
        let body_stop = terminal_end.saturating_add(1);
        let body = self.build_sequence(terminal_start, Some(body_stop), loop_exit)?;
        let blocks = body.blocks();
        let region = Region::If(Box::new(IfRegion {
            prefix: linear_block(self.function, block),
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
            merge: Some(cont_start),
            phis: Vec::new(),
        }));
        Ok(Some((region, cont_start)))
    }

    pub(super) fn loop_continue_if_region(
        &mut self,
        block: usize,
        branch: BranchInfo,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Result<Option<(Region, usize)>, LuaError> {
        let Some(stop) = stop else {
            return Ok(None);
        };
        if loop_exit.is_none() || stop >= self.function.blocks.len() {
            return Ok(None);
        }

        let true_continue = self.leads_to_loop_continue(branch.true_block, stop);
        let false_continue = self.leads_to_loop_continue(branch.false_block, stop);
        if true_continue == false_continue {
            return Ok(None);
        }

        let (continue_target, body_target, inverted) = if true_continue {
            (branch.true_block, branch.false_block, true)
        } else {
            (branch.false_block, branch.true_block, false)
        };
        let body_start = conditionals::follow_jmp_only(self.function, body_target, Some(stop));
        if body_start <= block || body_start >= stop {
            return Ok(None);
        }

        self.consumed[block] = true;
        self.consume_jmp_path_to_stop(continue_target, stop);
        let body = self.build_sequence(body_start, Some(stop), loop_exit)?;
        let blocks = body.blocks();
        let region = Region::If(Box::new(IfRegion {
            prefix: linear_block(self.function, block),
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
            merge: Some(stop),
            phis: Vec::new(),
        }));
        Ok(Some((region, stop)))
    }

    pub(super) fn break_if_region(
        &mut self,
        block: usize,
        branch: BranchInfo,
        loop_exit: Option<usize>,
    ) -> Result<Option<(Region, usize)>, LuaError> {
        let Some(exit) = loop_exit else {
            return Ok(None);
        };
        let true_break = self.break_path_end(branch.true_block, exit);
        let false_break = self.break_path_end(branch.false_block, exit);
        if true_break.is_some() == false_break.is_some() {
            return Ok(None);
        }

        self.consumed[block] = true;
        let (break_target, break_end, cont_target, inverted) = if let Some(end) = true_break {
            (branch.true_block, end, branch.false_block, false)
        } else {
            (
                branch.false_block,
                false_break.expect("one branch must be a break path"),
                branch.true_block,
                true,
            )
        };
        let break_target = conditionals::follow_jmp_only(self.function, break_target, Some(exit));
        let cont_target = conditionals::follow_jmp_only(self.function, cont_target, Some(exit));

        let mut body_parts = Vec::new();
        let break_stop = break_end.unwrap_or(exit);
        if break_target != exit && break_target != break_stop {
            body_parts.push(self.build_sequence(break_target, Some(break_stop), Some(exit))?);
        } else if break_target != exit && break_end.is_none() {
            body_parts.push(self.build_sequence(break_target, Some(exit), Some(exit))?);
        }
        if let Some(block) = break_end
            && let Some(slot) = self.consumed.get_mut(block)
        {
            *slot = true;
        }
        body_parts.push(Region::Break);
        let body = Region::Sequence(body_parts);
        let blocks = body.blocks();
        let region = Region::If(Box::new(IfRegion {
            prefix: linear_block(self.function, block),
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
            merge: Some(cont_target),
            phis: Vec::new(),
        }));
        Ok(Some((region, cont_target)))
    }

    pub(super) fn is_break_block(&self, block: usize, loop_exit: Option<usize>) -> bool {
        let Some(exit) = loop_exit else {
            return false;
        };
        conditionals::is_jmp_only(self.function, block)
            && self.function.blocks[block].succs.first().copied() == Some(exit)
    }

    fn final_empty_sibling_if_region(
        &mut self,
        block: usize,
        branch: BranchInfo,
        true_end: Option<usize>,
        false_end: Option<usize>,
        stop: Option<usize>,
        loop_exit: Option<usize>,
    ) -> Result<Option<(Region, usize)>, LuaError> {
        let true_empty =
            true_end.filter(|end| conditionals::is_final_empty_return_block(self.function, *end));
        let false_empty =
            false_end.filter(|end| conditionals::is_final_empty_return_block(self.function, *end));
        if true_empty.is_some() == false_empty.is_some() {
            return Ok(None);
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
            return Ok(None);
        }
        if let Some(stop) = stop
            && (body_start >= stop || empty_start >= stop)
        {
            return Ok(None);
        }
        if conditionals::can_reach(self.function, body_start, empty_end)
            || !conditionals::all_paths_terminate(self.function, block, body_start)
        {
            return Ok(None);
        }

        self.consumed[block] = true;
        if let Some(slot) = self.consumed.get_mut(empty_start) {
            *slot = true;
        }
        if let Some(slot) = self.consumed.get_mut(empty_end) {
            *slot = true;
        }

        let body = self.build_sequence(body_start, Some(empty_end), loop_exit)?;
        let blocks = body.blocks();
        let region = Region::If(Box::new(IfRegion {
            prefix: linear_block(self.function, block),
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
            merge: Some(empty_end),
            phis: Vec::new(),
        }));
        Ok(Some((region, empty_end.saturating_add(1))))
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
