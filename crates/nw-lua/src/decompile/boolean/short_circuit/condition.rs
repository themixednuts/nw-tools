use super::helpers::condition_segments;
use super::*;

pub fn condition_chain(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
    loop_headers: &BlockSet,
) -> Option<ConditionChain> {
    let first = branch_info(function, start, pc_map)?;
    if branch_rel(function, first.node)? == RelOp::TestSet {
        return None;
    }

    let mut blocks = Vec::new();
    let mut current = start;
    let mut steps = 0;
    loop {
        if steps >= 32 || (current != start && loop_headers.contains(current)) {
            return None;
        }
        steps += 1;
        let info = branch_info(function, current, pc_map)?;
        if branch_rel(function, info.node)? == RelOp::TestSet {
            return None;
        }
        blocks.push(current);

        let true_is_cond = is_condition_block(
            function,
            expr_analysis,
            info.true_block,
            pc_map,
            loop_headers,
        ) && is_chain_continuation(
            function,
            expr_analysis,
            info,
            info.true_block,
            info.false_block,
            &blocks,
            pc_map,
            loop_headers,
        );
        let false_is_cond = is_condition_block(
            function,
            expr_analysis,
            info.false_block,
            pc_map,
            loop_headers,
        ) && is_chain_continuation(
            function,
            expr_analysis,
            info,
            info.false_block,
            info.true_block,
            &blocks,
            pc_map,
            loop_headers,
        );
        match (true_is_cond, false_is_cond) {
            (true, _) => current = info.true_block,
            (false, true) => current = info.false_block,
            (false, false) => break,
        }
    }

    if blocks.len() < 2 {
        return None;
    }

    let last = branch_info(function, *blocks.last()?, pc_map)?;
    let true_target = conditionals::follow_jmp_only(function, last.true_block, None);
    let false_target = conditionals::follow_jmp_only(function, last.false_block, None);
    if true_target == false_target {
        return None;
    }

    let merge = conditionals::find_merge(function, start, true_target, false_target)
        .unwrap_or_else(|| true_target.max(false_target).saturating_add(1));
    let (body, false_target) = condition_body_and_skip(function, true_target, false_target, merge);
    if body <= start {
        return None;
    }

    let merge = if merge <= body { false_target } else { merge };
    if !condition_chain_targets_are_closed(function, &blocks, body, false_target, merge, pc_map) {
        return None;
    }
    let segments = condition_segments(function, &blocks, body, merge, pc_map)?;

    Some(ConditionChain {
        start,
        blocks,
        body,
        false_target,
        merge,
        segments,
    })
}

pub(super) fn condition_chain_targets_are_closed(
    function: &SsaFunction,
    blocks: &[usize],
    body: usize,
    false_target: usize,
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
                target == body
                    || target == false_target
                    || target == merge
                    || blocks.contains(&target)
            })
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn is_chain_continuation(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    info: conditionals::BranchInfo,
    target: usize,
    sibling: usize,
    chain_blocks: &[usize],
    pc_map: &[Option<usize>],
    loop_headers: &BlockSet,
) -> bool {
    let Some(block) = function.blocks.get(target) else {
        return false;
    };
    if target_preds_are_chain_owned(
        function,
        info.node.block,
        block.preds.as_slice(),
        chain_blocks,
    ) {
        let sibling = conditionals::follow_jmp_only(function, sibling, None);
        return shares_sibling_successor(function, target, sibling)
            || (!conditionals::is_terminal_block(function, sibling)
                && condition_only_reaches(
                    function,
                    expr_analysis,
                    target,
                    sibling,
                    pc_map,
                    loop_headers,
                )
                && sibling_preds_are_owned(
                    function,
                    expr_analysis,
                    info.node.block,
                    target,
                    sibling,
                    chain_blocks,
                    pc_map,
                    loop_headers,
                ))
            || reaches_sibling_terminal(
                function,
                expr_analysis,
                info.node.block,
                target,
                sibling,
                chain_blocks,
                pc_map,
                loop_headers,
            );
    }
    let sibling = conditionals::follow_jmp_only(function, sibling, None);
    conditionals::is_terminal_block(function, sibling)
        && is_failed_test_target(function, info, target)
}

pub(super) fn target_preds_are_chain_owned(
    function: &SsaFunction,
    current: usize,
    preds: &[usize],
    chain_blocks: &[usize],
) -> bool {
    preds.iter().copied().all(|pred| {
        pred == current
            || function.blocks.get(pred).is_some_and(|block| {
                conditionals::is_jmp_only(function, pred)
                    && block
                        .preds
                        .iter()
                        .copied()
                        .all(|jump_pred| chain_blocks.contains(&jump_pred))
            })
    })
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

#[allow(clippy::too_many_arguments)]
pub(super) fn reaches_sibling_terminal(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    current: usize,
    target: usize,
    sibling: usize,
    chain_blocks: &[usize],
    pc_map: &[Option<usize>],
    loop_headers: &BlockSet,
) -> bool {
    let sibling = conditionals::follow_jmp_only(function, sibling, None);
    conditionals::is_terminal_block(function, sibling)
        && condition_only_reaches(
            function,
            expr_analysis,
            target,
            sibling,
            pc_map,
            loop_headers,
        )
        && sibling_preds_are_owned(
            function,
            expr_analysis,
            current,
            target,
            sibling,
            chain_blocks,
            pc_map,
            loop_headers,
        )
}

pub(super) fn condition_only_reaches(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    target: usize,
    pc_map: &[Option<usize>],
    loop_headers: &BlockSet,
) -> bool {
    if start >= function.blocks.len() || target >= function.blocks.len() {
        return false;
    }

    let mut stack = vec![start];
    let mut seen = vec![false; function.blocks.len()];
    while let Some(block) = stack.pop() {
        if block == target {
            return true;
        }
        if block >= function.blocks.len() || seen[block] {
            continue;
        }
        seen[block] = true;

        if !is_condition_block(function, expr_analysis, block, pc_map, loop_headers)
            && !conditionals::is_jmp_only(function, block)
        {
            continue;
        }
        stack.extend(function.blocks[block].succs.iter().copied());
    }

    false
}

#[allow(clippy::too_many_arguments)]
pub(super) fn sibling_preds_are_owned(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    current: usize,
    target: usize,
    sibling: usize,
    chain_blocks: &[usize],
    pc_map: &[Option<usize>],
    loop_headers: &BlockSet,
) -> bool {
    let Some(block) = function.blocks.get(sibling) else {
        return false;
    };
    block.preds.iter().copied().all(|pred| {
        condition_only_reaches(function, expr_analysis, target, pred, pc_map, loop_headers)
            || function.blocks.get(pred).is_some_and(|pred_block| {
                conditionals::is_jmp_only(function, pred)
                    && pred_block.preds.iter().copied().all(|jump_pred| {
                        jump_pred == current
                            || chain_blocks.contains(&jump_pred)
                            || condition_only_reaches(
                                function,
                                expr_analysis,
                                target,
                                jump_pred,
                                pc_map,
                                loop_headers,
                            )
                    })
            })
    })
}

pub(super) fn is_failed_test_target(
    function: &SsaFunction,
    info: conditionals::BranchInfo,
    target: usize,
) -> bool {
    let Some(node) = branch_at(function, info.node) else {
        return false;
    };
    let SsaOp::Branch { invert, .. } = node.op else {
        return false;
    };
    if target == info.true_block {
        invert
    } else if target == info.false_block {
        !invert
    } else {
        false
    }
}

pub(super) fn condition_body_and_skip(
    function: &SsaFunction,
    true_target: usize,
    false_target: usize,
    merge: usize,
) -> (usize, usize) {
    if merge == true_target || merge == false_target {
        return (true_target.min(false_target), true_target.max(false_target));
    }
    let true_jumps_to_merge =
        conditionals::follow_jmp_only(function, true_target, Some(merge)) == merge;
    let false_jumps_to_merge =
        conditionals::follow_jmp_only(function, false_target, Some(merge)) == merge;
    match (true_jumps_to_merge, false_jumps_to_merge) {
        (true, false) => (false_target, true_target),
        (false, true) => (true_target, false_target),
        _ => (true_target.min(false_target), true_target.max(false_target)),
    }
}
