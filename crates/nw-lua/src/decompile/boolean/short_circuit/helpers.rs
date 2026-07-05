use super::*;

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

pub(super) fn condition_segments(
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

pub(in crate::decompile::boolean) fn branch_rel(
    function: &SsaFunction,
    id: NodeId,
) -> Option<RelOp> {
    let node = branch_at(function, id)?;
    let SsaOp::Branch { rel, .. } = node.op else {
        return None;
    };
    Some(rel)
}

pub(super) fn loadbool_value(function: &SsaFunction, block: usize) -> Option<(SsaRef, bool)> {
    function.blocks.get(block)?.nodes.iter().find_map(|node| {
        let SsaOp::LoadBool { value, .. } = node.op else {
            return None;
        };
        Some((node.dest, value))
    })
}

pub(in crate::decompile::boolean) fn pure_select_range(
    function: &SsaFunction,
    branch: NodeId,
    start: usize,
    merge: usize,
) -> bool {
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

pub(super) fn same_reg(left: SsaRef, right: SsaRef) -> bool {
    left.reg_index().is_some() && left.reg_index() == right.reg_index()
}

pub(in crate::decompile::boolean) fn selected_operand(
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

pub(super) fn common_successor(function: &SsaFunction, left: usize, right: usize) -> Option<usize> {
    let left_block = function.blocks.get(left)?;
    let right_block = function.blocks.get(right)?;
    left_block
        .succs
        .iter()
        .copied()
        .filter(|succ| right_block.succs.contains(succ))
        .min()
}
