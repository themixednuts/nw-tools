use super::*;
use super::{
    helpers::{common_successor, loadbool_value, same_reg},
    value::is_value_chain_continuation,
};

pub(super) fn guarded_condition_value(
    function: &SsaFunction,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let guard = branch_info(function, start, pc_map)?;
    let guard_node = branch_at(function, guard.node)?;
    let SsaOp::Branch {
        rel: RelOp::Test,
        a,
        invert,
        ..
    } = guard_node.op
    else {
        return None;
    };

    let value_block = conditionals::follow_jmp_only(function, guard.true_block, None);
    let pass_block = guard.false_block;
    let value = branch_info(function, value_block, pc_map)?;
    if branch_rel(function, value.node)? == RelOp::TestSet {
        return None;
    }

    let true_block = conditionals::follow_jmp_only(function, value.true_block, None);
    let false_block = conditionals::follow_jmp_only(function, value.false_block, None);
    let (true_ref, true_value) = loadbool_value(function, true_block)?;
    let (false_ref, false_value) = loadbool_value(function, false_block)?;
    if true_value == false_value {
        return None;
    }

    let merge = common_successor(function, pass_block, true_block)
        .filter(|merge| {
            function
                .blocks
                .get(false_block)
                .is_some_and(|block| block.succs.contains(merge))
        })
        .or_else(|| conditionals::find_merge(function, start, pass_block, value_block))?;
    if !pure_select_range(function, guard.node, start, merge) {
        return None;
    }

    let phi = phi_sources(function, merge).find(|phi| {
        phi.operand_from(pass_block)
            .is_some_and(|operand| same_reg(operand, a))
            && phi.operand_from(true_block) == Some(true_ref)
            && phi.operand_from(false_block) == Some(false_ref)
    })?;
    let op = if invert {
        BoolConnector::Or
    } else {
        BoolConnector::And
    };

    Some(ValuePlan {
        start,
        merge,
        dest: phi.dest,
        pc: phi.pc,
        kind: ValuePlanKind::Binary {
            left: a.into(),
            op,
            right: ValueTerm::Condition {
                branch: value.node,
                inverted: !true_value,
            },
        },
    })
}

pub(super) fn comparison_guard_value(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let info = branch_info(function, start, pc_map)?;
    let node = branch_at(function, info.node)?;
    let SsaOp::Branch { rel, .. } = node.op else {
        return None;
    };
    if matches!(rel, RelOp::TestSet) {
        return None;
    }

    let true_block = conditionals::follow_jmp_only(function, info.true_block, None);
    let false_block = conditionals::follow_jmp_only(function, info.false_block, None);
    let true_bool = loadbool_value(function, true_block);
    let false_bool = loadbool_value(function, false_block);
    let (value_block, bool_block, bool_ref, bool_value, condition_inverted) =
        match (true_bool, false_bool) {
            (None, Some((reference, value))) => (true_block, false_block, reference, value, false),
            (Some((reference, value)), None) => (false_block, true_block, reference, value, true),
            _ => return None,
        };

    let merge = common_successor(function, value_block, bool_block)
        .or_else(|| conditionals::find_merge(function, start, value_block, bool_block))?;
    if !pure_select_range(function, info.node, start, merge) {
        return None;
    }
    let phi = phi_sources(function, merge).find(|phi| {
        phi.operand_from(bool_block) == Some(bool_ref) && phi.operand_from(value_block).is_some()
    })?;
    let right = selected_operand(
        function,
        expr_analysis,
        phi.dest,
        value_block,
        phi.operand_from(value_block)?,
    )?;
    let op = if bool_value {
        BoolConnector::Or
    } else {
        BoolConnector::And
    };
    let left_inverted = if bool_value {
        !condition_inverted
    } else {
        condition_inverted
    };

    Some(ValuePlan {
        start,
        merge,
        dest: phi.dest,
        pc: phi.pc,
        kind: ValuePlanKind::Binary {
            left: ValueTerm::Condition {
                branch: info.node,
                inverted: left_inverted,
            },
            op,
            right,
        },
    })
}

pub(super) fn guarded_or_value(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let first = branch_info(function, start, pc_map)?;
    let first_rel = branch_rel(function, first.node)?;
    if first_rel == RelOp::TestSet {
        return None;
    }

    let mut blocks = Vec::new();
    let mut current = start;
    for _ in 0..16 {
        let info = branch_info(function, current, pc_map)?;
        let rel = branch_rel(function, info.node)?;
        if rel == RelOp::TestSet {
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
    if blocks.len() < 2 {
        return None;
    }

    let last_block = *blocks.last()?;
    let last = branch_info(function, last_block, pc_map)?;
    let last_true = conditionals::follow_jmp_only(function, last.true_block, None);
    let last_false = conditionals::follow_jmp_only(function, last.false_block, None);

    let (true_block, true_ref) = loadbool_value(function, last_true)
        .filter(|(_, value)| *value)
        .map(|(reference, _)| (last_true, reference))
        .or_else(|| {
            loadbool_value(function, last_false)
                .filter(|(_, value)| *value)
                .map(|(reference, _)| (last_false, reference))
        })?;
    let expr_block = if true_block == last_true {
        last_false
    } else {
        last_true
    };

    let false_block = shared_prefix_false_block(function, &blocks, true_block, expr_block, pc_map)?;
    let (false_ref, false_value) = loadbool_value(function, false_block)?;
    if false_value {
        return None;
    }

    let merge = common_successor(function, false_block, true_block)?;
    if !function
        .blocks
        .get(expr_block)?
        .succs
        .iter()
        .copied()
        .any(|succ| conditionals::follow_jmp_only(function, succ, Some(merge)) == merge)
    {
        return None;
    }
    if !pure_select_range(function, first.node, start, merge) {
        return None;
    }

    let phi = phi_sources(function, merge).find(|phi| {
        phi.operand_from(false_block) == Some(false_ref)
            && phi.operand_from(true_block) == Some(true_ref)
            && phi.operand_from(expr_block).is_some()
    })?;
    let expr_operand = phi.operand_from(expr_block)?;
    let or_value = selected_operand(function, expr_analysis, phi.dest, expr_block, expr_operand)?;
    let prefix = guarded_prefix_segments(function, &blocks, pc_map)?;
    let or_condition = ConditionSegment {
        node: last.node,
        inverted: true_block == last_false,
        connector: None,
    };

    Some(ValuePlan {
        start,
        merge,
        dest: phi.dest,
        pc: phi.pc,
        kind: ValuePlanKind::GuardedOrValue {
            prefix,
            or_condition,
            or_value,
        },
    })
}

pub(super) fn shared_prefix_false_block(
    function: &SsaFunction,
    blocks: &[usize],
    true_block: usize,
    expr_block: usize,
    pc_map: &[Option<usize>],
) -> Option<usize> {
    let mut result = None;
    for (index, block) in blocks.iter().copied().enumerate() {
        let info = branch_info(function, block, pc_map)?;
        let next = blocks.get(index + 1).copied();
        for target in [info.true_block, info.false_block] {
            let target = conditionals::follow_jmp_only(function, target, None);
            if Some(target) == next || target == true_block || target == expr_block {
                continue;
            }
            if result.is_some_and(|current| current != target) {
                return None;
            }
            result = Some(target);
        }
    }
    result
}

pub(super) fn guarded_prefix_segments(
    function: &SsaFunction,
    blocks: &[usize],
    pc_map: &[Option<usize>],
) -> Option<Vec<ConditionSegment>> {
    let mut segments = Vec::with_capacity(blocks.len().saturating_sub(1));
    for (index, block) in blocks
        .iter()
        .copied()
        .take(blocks.len().saturating_sub(1))
        .enumerate()
    {
        let info = branch_info(function, block, pc_map)?;
        let next = blocks.get(index + 1).copied()?;
        let inverted = if info.true_block == next {
            false
        } else if info.false_block == next {
            true
        } else {
            return None;
        };
        segments.push(ConditionSegment {
            node: info.node,
            inverted,
            connector: Some(BoolConnector::And),
        });
    }
    Some(segments)
}

pub(super) fn guarded_chain_value(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let first = branch_info(function, start, pc_map)?;
    let first_node = branch_at(function, first.node)?;
    let (first_term, mut current, default_block) = guard_term(function, first, first_node)?;
    let mut terms = vec![first_term];
    for _ in 0..16 {
        let info = branch_info(function, current, pc_map)?;
        let node = branch_at(function, info.node)?;
        let SsaOp::Branch { rel, a, invert, .. } = node.op else {
            return None;
        };

        if matches!(rel, RelOp::Test | RelOp::TestSet) && invert {
            let selected_dest = if rel == RelOp::TestSet { node.dest } else { a };
            let selected_block = info.false_block;
            if conditionals::follow_jmp_only(function, info.true_block, None) != default_block {
                return None;
            }
            let merge =
                common_successor(function, selected_block, default_block).or_else(|| {
                    conditionals::find_merge(function, start, selected_block, default_block)
                })?;
            if !pure_select_range(function, first.node, start, merge) {
                return None;
            }
            let phi = phi_sources(function, merge).find(|phi| {
                same_reg(phi.dest, selected_dest)
                    && phi
                        .operand_from(selected_block)
                        .is_some_and(|operand| same_reg(operand, selected_dest))
                    && phi.operand_from(default_block).is_some()
            })?;
            terms.push(if rel == RelOp::Test {
                selected_operand(function, expr_analysis, selected_dest, current, a)?
            } else {
                a.into()
            });
            let fallback = selected_operand(
                function,
                expr_analysis,
                selected_dest,
                default_block,
                phi.operand_from(default_block)?,
            )?;
            return Some(ValuePlan {
                start,
                merge,
                dest: phi.dest,
                pc: phi.pc,
                kind: ValuePlanKind::Chain { terms, fallback },
            });
        }

        if let Some((term, next, guard_default)) = guard_term(function, info, node)
            && guard_default == default_block
        {
            terms.push(term);
            current = next;
            continue;
        }
        return None;
    }
    None
}

pub(super) fn guard_term(
    function: &SsaFunction,
    info: conditionals::BranchInfo,
    node: &crate::ir::SsaNode,
) -> Option<(ValueTerm, usize, usize)> {
    let SsaOp::Branch {
        rel: RelOp::Test,
        a,
        invert,
        ..
    } = node.op
    else {
        return None;
    };
    let term = if invert {
        ValueTerm::Condition {
            branch: info.node,
            inverted: false,
        }
    } else {
        a.into()
    };
    Some((
        term,
        info.true_block,
        conditionals::follow_jmp_only(function, info.false_block, None),
    ))
}
