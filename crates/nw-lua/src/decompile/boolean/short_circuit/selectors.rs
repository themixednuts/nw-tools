use super::helpers::{common_successor, loadbool_value, same_reg};
use super::*;

pub(super) fn test_value(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let info = branch_info(function, start, pc_map)?;
    let node = branch_at(function, info.node)?;
    let SsaOp::Branch {
        rel: RelOp::Test,
        a,
        invert,
        ..
    } = node.op
    else {
        return None;
    };

    let value_block = info.true_block;
    let pass_block = info.false_block;
    let merge = common_successor(function, value_block, pass_block)
        .or_else(|| conditionals::find_merge(function, start, value_block, pass_block))?;
    if !pure_select_range(function, info.node, start, merge) {
        return None;
    }

    let phi = phi_sources(function, merge).find(|phi| {
        same_reg(phi.dest, a)
            && phi
                .operand_from(pass_block)
                .is_some_and(|operand| same_reg(operand, a))
            && phi.operand_from(value_block).is_some()
    })?;
    let right = selected_operand(
        function,
        expr_analysis,
        phi.dest,
        value_block,
        phi.operand_from(value_block)?,
    )?;
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
            right,
        },
    })
}

pub(super) fn testset_value(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let info = branch_info(function, start, pc_map)?;
    let node = branch_at(function, info.node)?;
    let SsaOp::Branch { rel, a, invert, .. } = node.op else {
        return None;
    };
    if rel != RelOp::TestSet {
        return None;
    }

    let value_block = info.true_block;
    let pass_block = info.false_block;
    let merge = common_successor(function, value_block, pass_block)
        .or_else(|| conditionals::find_merge(function, start, value_block, pass_block))?;
    if !pure_select_range(function, info.node, start, merge) {
        return None;
    }

    let phi = phi_sources(function, merge)
        .find(|phi| same_reg(phi.dest, node.dest) && phi.operand_from(value_block).is_some())?;
    let right = selected_operand(
        function,
        expr_analysis,
        node.dest,
        value_block,
        phi.operand_from(value_block)?,
    )?;
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
            right,
        },
    })
}

pub(super) fn ternary_value(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let outer = branch_info(function, start, pc_map)?;
    let outer_node = branch_at(function, outer.node)?;
    let SsaOp::Branch {
        rel: RelOp::Test,
        a: first,
        invert: false,
        ..
    } = outer_node.op
    else {
        return None;
    };

    let inner = branch_info(function, outer.true_block, pc_map)?;
    let inner_node = branch_at(function, inner.node)?;
    let SsaOp::Branch {
        rel,
        a: second,
        invert: true,
        ..
    } = inner_node.op
    else {
        return None;
    };
    if !matches!(rel, RelOp::Test | RelOp::TestSet) {
        return None;
    }
    let selected_dest = if rel == RelOp::TestSet {
        inner_node.dest
    } else {
        second
    };

    let selected_block = inner.false_block;
    let default_block = conditionals::follow_jmp_only(function, inner.true_block, None);
    let outer_default = conditionals::follow_jmp_only(function, outer.false_block, None);
    if default_block != outer_default {
        return None;
    }

    let merge = common_successor(function, selected_block, default_block)
        .or_else(|| conditionals::find_merge(function, start, selected_block, default_block))?;
    if !pure_select_range(function, outer.node, start, merge) {
        return None;
    }
    let phi = phi_sources(function, merge).find(|phi| same_reg(phi.dest, selected_dest))?;
    let second = if rel == RelOp::Test {
        selected_operand(
            function,
            expr_analysis,
            selected_dest,
            inner.node.block,
            second,
        )?
    } else {
        second.into()
    };
    let fallback = selected_operand(
        function,
        expr_analysis,
        selected_dest,
        default_block,
        phi.operand_from(default_block)?,
    )?;

    Some(ValuePlan {
        start,
        merge,
        dest: phi.dest,
        pc: phi.pc,
        kind: ValuePlanKind::Ternary {
            first: first.into(),
            second,
            fallback,
        },
    })
}

pub(super) fn branch_ternary_value(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let outer = branch_info(function, start, pc_map)?;
    let outer_node = branch_at(function, outer.node)?;
    if matches!(
        outer_node.op,
        SsaOp::Branch {
            rel: RelOp::TestSet,
            ..
        }
    ) {
        return None;
    }

    let inner = branch_info(function, outer.true_block, pc_map)?;
    let inner_node = branch_at(function, inner.node)?;
    let SsaOp::Branch {
        rel,
        a: second,
        invert: true,
        ..
    } = inner_node.op
    else {
        return None;
    };
    if !matches!(rel, RelOp::Test | RelOp::TestSet) {
        return None;
    }

    let selected_dest = if rel == RelOp::TestSet {
        inner_node.dest
    } else {
        second
    };
    let selected_block = inner.false_block;
    let default_block = conditionals::follow_jmp_only(function, inner.true_block, None);
    let outer_default = conditionals::follow_jmp_only(function, outer.false_block, None);
    if default_block != outer_default {
        return None;
    }

    let merge = common_successor(function, selected_block, default_block)
        .or_else(|| conditionals::find_merge(function, start, selected_block, default_block))?;
    if !pure_select_range(function, outer.node, start, merge) {
        return None;
    }
    let phi = phi_sources(function, merge).find(|phi| same_reg(phi.dest, selected_dest))?;
    let second = if rel == RelOp::Test {
        selected_operand(
            function,
            expr_analysis,
            selected_dest,
            inner.node.block,
            second,
        )?
    } else {
        second.into()
    };
    let fallback = selected_operand(
        function,
        expr_analysis,
        selected_dest,
        default_block,
        phi.operand_from(default_block)?,
    )?;

    Some(ValuePlan {
        start,
        merge,
        dest: phi.dest,
        pc: phi.pc,
        kind: ValuePlanKind::Ternary {
            first: ValueTerm::Condition {
                branch: outer.node,
                inverted: false,
            },
            second,
            fallback,
        },
    })
}

pub(super) fn comparison_value(
    function: &SsaFunction,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let info = branch_info(function, start, pc_map)?;
    let node = branch_at(function, info.node)?;
    let SsaOp::Branch { rel, .. } = node.op else {
        return None;
    };
    if !matches!(rel, RelOp::Eq | RelOp::Lt | RelOp::Le) {
        return None;
    }

    let true_block = conditionals::follow_jmp_only(function, info.true_block, None);
    let false_block = conditionals::follow_jmp_only(function, info.false_block, None);
    let (true_ref, true_value) = loadbool_value(function, true_block)?;
    let (false_ref, false_value) = loadbool_value(function, false_block)?;
    if true_value == false_value {
        return None;
    }

    let merge = common_successor(function, true_block, false_block)
        .or_else(|| conditionals::find_merge(function, start, true_block, false_block))?;
    if !pure_select_range(function, info.node, start, merge) {
        return None;
    }
    let phi = phi_sources(function, merge).find(|phi| {
        phi.operand_from(true_block) == Some(true_ref)
            && phi.operand_from(false_block) == Some(false_ref)
    })?;

    Some(ValuePlan {
        start,
        merge,
        dest: phi.dest,
        pc: phi.pc,
        kind: ValuePlanKind::Condition {
            branch: info.node,
            inverted: !true_value,
        },
    })
}
