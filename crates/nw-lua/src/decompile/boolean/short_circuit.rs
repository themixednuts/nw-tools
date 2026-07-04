//! Short-circuit branch-chain recognition.

use crate::{
    decompile::{
        analysis::{DecompileAnalysis, NodeId},
        ast,
        control_flow::{conditionals, regions::BlockSet},
    },
    ir::{RelOp, SsaFunction, SsaOp, SsaRef},
};

use super::{branch_at, branch_info, is_condition_block, is_pure_value_node, phi_sources};

/// Boolean connector between adjacent short-circuit segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolConnector {
    And,
    Or,
}

impl BoolConnector {
    pub(crate) const fn ast_op(self) -> ast::BinOp {
        match self {
            Self::And => ast::BinOp::And,
            Self::Or => ast::BinOp::Or,
        }
    }
}

/// One condition segment in a compound branch chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionSegment {
    pub node: NodeId,
    pub inverted: bool,
    pub connector: Option<BoolConnector>,
}

/// A collapsed `and`/`or` condition chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionChain {
    pub start: usize,
    pub blocks: Vec<usize>,
    pub body: usize,
    pub false_target: usize,
    pub merge: usize,
    pub segments: Vec<ConditionSegment>,
}

/// A value-producing short-circuit select that materializes at a PHI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuePlan {
    pub start: usize,
    pub merge: usize,
    pub dest: SsaRef,
    pub pc: i32,
    pub kind: ValuePlanKind,
}

impl ValuePlan {
    #[must_use]
    pub fn consumed_blocks(&self) -> std::ops::Range<usize> {
        self.start..self.merge
    }
}

/// Expression payload for a value select.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValuePlanKind {
    Binary {
        left: ValueTerm,
        op: BoolConnector,
        right: ValueTerm,
    },
    Ternary {
        first: ValueTerm,
        second: ValueTerm,
        fallback: ValueTerm,
    },
    Chain {
        terms: Vec<ValueTerm>,
        fallback: ValueTerm,
    },
    Condition {
        branch: NodeId,
        inverted: bool,
    },
}

/// One expression segment inside a short-circuit value plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueTerm {
    Ref(SsaRef),
    Node(NodeId),
    Condition { branch: NodeId, inverted: bool },
}

impl From<SsaRef> for ValueTerm {
    fn from(reference: SsaRef) -> Self {
        Self::Ref(reference)
    }
}

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
        );
        let false_is_cond = is_condition_block(
            function,
            expr_analysis,
            info.false_block,
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

    let body = true_target.min(false_target);
    let false_target = true_target.max(false_target);
    if body <= start {
        return None;
    }

    let merge = conditionals::find_merge(function, start, body, false_target)
        .unwrap_or_else(|| false_target.max(body).saturating_add(1));
    let merge = if merge <= body { false_target } else { merge };
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

pub fn value_plan(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    comparison_value(function, start, pc_map)
        .or_else(|| test_value(function, expr_analysis, start, pc_map))
        .or_else(|| guarded_chain_value(function, expr_analysis, start, pc_map))
        .or_else(|| branch_ternary_value(function, expr_analysis, start, pc_map))
        .or_else(|| ternary_value(function, expr_analysis, start, pc_map))
        .or_else(|| testset_value(function, expr_analysis, start, pc_map))
}

fn test_value(
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

fn testset_value(
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

fn ternary_value(
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

fn branch_ternary_value(
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

fn guarded_chain_value(
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

fn guard_term(
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

fn comparison_value(
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

fn condition_segments(
    function: &SsaFunction,
    blocks: &[usize],
    body: usize,
    merge: usize,
    pc_map: &[Option<usize>],
) -> Option<Vec<ConditionSegment>> {
    let mut segments = Vec::with_capacity(blocks.len());
    for (index, block) in blocks.iter().copied().enumerate() {
        let info = branch_info(function, block, pc_map)?;
        let next = blocks.get(index + 1).copied();
        let true_target = conditionals::follow_jmp_only(function, info.true_block, Some(merge));
        let false_target = conditionals::follow_jmp_only(function, info.false_block, Some(merge));

        let (inverted, connector) = if let Some(next) = next {
            if info.true_block == next {
                if false_target == body {
                    (true, BoolConnector::Or)
                } else {
                    (false, BoolConnector::And)
                }
            } else if info.false_block == next {
                if true_target == body {
                    (false, BoolConnector::Or)
                } else {
                    (true, BoolConnector::And)
                }
            } else if true_target == body || true_target == merge {
                (true_target == merge, BoolConnector::And)
            } else if false_target == body || false_target == merge {
                (false_target == body, BoolConnector::Or)
            } else {
                return None;
            }
        } else if true_target == body {
            (false, BoolConnector::And)
        } else if false_target == body {
            (true, BoolConnector::And)
        } else {
            (false, BoolConnector::And)
        };

        segments.push(ConditionSegment {
            node: info.node,
            inverted,
            connector: next.map(|_| connector),
        });
    }
    Some(segments)
}

fn branch_rel(function: &SsaFunction, id: NodeId) -> Option<RelOp> {
    let node = branch_at(function, id)?;
    let SsaOp::Branch { rel, .. } = node.op else {
        return None;
    };
    Some(rel)
}

fn loadbool_value(function: &SsaFunction, block: usize) -> Option<(SsaRef, bool)> {
    function.blocks.get(block)?.nodes.iter().find_map(|node| {
        let SsaOp::LoadBool { value, .. } = node.op else {
            return None;
        };
        Some((node.dest, value))
    })
}

fn pure_select_range(function: &SsaFunction, branch: NodeId, start: usize, merge: usize) -> bool {
    start < merge
        && branch.block == start
        && (start..merge).all(|block| {
            let Some(block_ref) = function.blocks.get(block) else {
                return false;
            };
            let first_node = if block == start { branch.node } else { 0 };
            block_ref
                .nodes
                .iter()
                .skip(first_node)
                .all(is_pure_value_node)
        })
}

fn same_reg(left: SsaRef, right: SsaRef) -> bool {
    left.reg_index().is_some() && left.reg_index() == right.reg_index()
}

fn selected_operand(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    dest: SsaRef,
    block: usize,
    operand: SsaRef,
) -> Option<ValueTerm> {
    if !same_reg(dest, operand) {
        return Some(operand.into());
    }

    let Some(node_id) = expr_analysis.def_site(operand) else {
        return Some(operand.into());
    };
    if node_id.block != block {
        return Some(operand.into());
    }

    let node = function
        .blocks
        .get(node_id.block)
        .and_then(|block| block.nodes.get(node_id.node))?;
    match node.op {
        SsaOp::Move { src } => Some(src.into()),
        SsaOp::LoadK { idx } => Some(SsaRef::Const(idx).into()),
        SsaOp::LoadNil { .. } => Some(SsaRef::None.into()),
        SsaOp::Phi { .. }
        | SsaOp::Nop
        | SsaOp::Jump { .. }
        | SsaOp::Branch { .. }
        | SsaOp::Return { .. }
        | SsaOp::SetGlobal { .. }
        | SsaOp::SetUpval { .. }
        | SsaOp::SetTable { .. }
        | SsaOp::ForPrep { .. }
        | SsaOp::ForLoop { .. }
        | SsaOp::TForLoop { .. }
        | SsaOp::SetList { .. }
        | SsaOp::Close { .. } => None,
        _ if is_pure_value_node(node) => Some(ValueTerm::Node(node_id)),
        _ => None,
    }
}

fn common_successor(function: &SsaFunction, left: usize, right: usize) -> Option<usize> {
    let left_block = function.blocks.get(left)?;
    let right_block = function.blocks.get(right)?;
    left_block
        .succs
        .iter()
        .copied()
        .filter(|succ| right_block.succs.contains(succ))
        .min()
}
