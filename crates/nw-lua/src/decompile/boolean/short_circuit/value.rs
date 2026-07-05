use super::helpers::{
    common_successor, condition_segments, is_failed_test_target, loadbool_value, same_reg,
};
use super::*;

pub fn value_plan(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    super::super::value_chain::and_or_value_chain(function, expr_analysis, start, pc_map)
        .or_else(|| super::guards::guarded_condition_value(function, start, pc_map))
        .or_else(|| condition_bool_chain_value(function, expr_analysis, start, pc_map))
        .or_else(|| super::guards::guarded_or_value(function, expr_analysis, start, pc_map))
        .or_else(|| super::guards::comparison_guard_value(function, expr_analysis, start, pc_map))
        .or_else(|| super::selectors::comparison_value(function, start, pc_map))
        .or_else(|| super::selectors::test_value(function, expr_analysis, start, pc_map))
        .or_else(|| super::guards::guarded_chain_value(function, expr_analysis, start, pc_map))
        .or_else(|| super::selectors::branch_ternary_value(function, expr_analysis, start, pc_map))
        .or_else(|| super::selectors::ternary_value(function, expr_analysis, start, pc_map))
        .or_else(|| super::selectors::testset_value(function, expr_analysis, start, pc_map))
}

pub(super) fn condition_bool_chain_value(
    function: &SsaFunction,
    _expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let first = branch_info(function, start, pc_map)?;
    if branch_rel(function, first.node)? == RelOp::TestSet {
        return None;
    }

    let mut blocks = Vec::new();
    let mut current = start;
    for _ in 0..16 {
        let info = branch_info(function, current, pc_map)?;
        if branch_rel(function, info.node)? == RelOp::TestSet {
            return None;
        }
        blocks.push(current);

        let true_is_cond = conditionals::is_pure_condition_block(function, info.true_block)
            && is_value_chain_continuation(function, info, info.true_block, info.false_block);
        let false_is_cond = conditionals::is_pure_condition_block(function, info.false_block)
            && is_value_chain_continuation(function, info, info.false_block, info.true_block);
        match (true_is_cond, false_is_cond) {
            (true, _) => current = info.true_block,
            (false, true) => current = info.false_block,
            (false, false) => break,
        }
    }

    let last = branch_info(function, *blocks.last()?, pc_map)?;
    let left = conditionals::follow_jmp_only(function, last.true_block, None);
    let right = conditionals::follow_jmp_only(function, last.false_block, None);
    if left == right {
        return None;
    }
    let (left_ref, left_value) = loadbool_value(function, left)?;
    let (right_ref, right_value) = loadbool_value(function, right)?;
    if left_value == right_value {
        return None;
    }

    let true_block = if left_value { left } else { right };
    let merge = common_successor(function, left, right)
        .or_else(|| conditionals::find_merge(function, start, left, right))?;
    if !condition_bool_chain_targets_are_closed(function, &blocks, left, right, merge, pc_map) {
        return None;
    }
    if !pure_select_range(function, first.node, start, merge) {
        return None;
    }
    let phi = phi_sources(function, merge).find(|phi| {
        phi.operand_from(left)
            .is_some_and(|operand| same_reg(operand, left_ref))
            && phi
                .operand_from(right)
                .is_some_and(|operand| same_reg(operand, right_ref))
    })?;
    let segments = condition_segments(function, &blocks, true_block, merge, pc_map)?;

    Some(ValuePlan {
        start,
        merge,
        dest: phi.dest,
        pc: phi.pc,
        kind: ValuePlanKind::ConditionChain {
            segments,
            true_block,
            false_block: if left_value { right } else { left },
        },
    })
}

pub(super) fn condition_bool_chain_targets_are_closed(
    function: &SsaFunction,
    blocks: &[usize],
    left: usize,
    right: usize,
    merge: usize,
    pc_map: &[Option<usize>],
) -> bool {
    blocks.iter().copied().all(|block| {
        let Some(info) = branch_info(function, block, pc_map) else {
            return false;
        };
        [info.true_block, info.false_block]
            .into_iter()
            .map(|target| conditionals::follow_jmp_only(function, target, Some(merge)))
            .all(|target| {
                target == left || target == right || target == merge || blocks.contains(&target)
            })
    })
}

pub(super) fn is_value_chain_continuation(
    function: &SsaFunction,
    info: conditionals::BranchInfo,
    target: usize,
    sibling: usize,
) -> bool {
    let Some(block) = function.blocks.get(target) else {
        return false;
    };
    if block.preds.as_slice() == [info.node.block] {
        return shares_sibling_successor(function, target, sibling)
            || reaches_sibling_bool_value(function, target, sibling);
    }
    let sibling = conditionals::follow_jmp_only(function, sibling, None);
    (conditionals::is_terminal_block(function, sibling)
        || loadbool_value(function, sibling).is_some())
        && is_failed_test_target(function, info, target)
}

pub(super) fn shares_sibling_successor(
    function: &SsaFunction,
    target: usize,
    sibling: usize,
) -> bool {
    let sibling = conditionals::follow_jmp_only(function, sibling, None);
    function.blocks.get(target).is_some_and(|block| {
        block
            .succs
            .iter()
            .copied()
            .any(|succ| conditionals::follow_jmp_only(function, succ, None) == sibling)
    })
}

pub(super) fn reaches_sibling_bool_value(
    function: &SsaFunction,
    target: usize,
    sibling: usize,
) -> bool {
    let sibling = conditionals::follow_jmp_only(function, sibling, None);
    loadbool_value(function, sibling).is_some()
        && conditionals::can_reach(function, target, sibling)
}
