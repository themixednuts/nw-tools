use crate::chunk::Constant;

use super::{NodePosition, PassChange, PassContext, PreservedAnalyses, SsaPass, node, node_mut};
use crate::ir::{BinOp, SsaFunction, SsaLiteral, SsaOp, SsaRef, UnOp, UseRole};

pub struct TrivialPhiElimination;

impl SsaPass for TrivialPhiElimination {
    fn name(&self) -> &'static str {
        "trivial-phi-elimination"
    }

    fn run(&mut self, function: &mut SsaFunction, _context: &mut PassContext<'_>) -> PassChange {
        let actions = positions(function)
            .filter_map(|position| {
                let node = node(function, position)?;
                let SsaOp::Phi { operands, .. } = &node.op else {
                    return None;
                };
                trivial_phi_source(node.dest, operands).map(|_| position)
            })
            .collect::<Vec<_>>();

        for position in &actions {
            let Some((dest, source)) = node(function, *position).and_then(|node| {
                let SsaOp::Phi { operands, .. } = &node.op else {
                    return None;
                };
                trivial_phi_source(node.dest, operands).map(|source| (node.dest, source))
            }) else {
                continue;
            };
            rewrite_uses(function, dest, source);
            if let Some(node) = node_mut(function, *position) {
                node.make_nop();
            }
        }
        changed_if(!actions.is_empty())
    }
}

pub struct CopyPropagation;

impl SsaPass for CopyPropagation {
    fn name(&self) -> &'static str {
        "copy-propagation"
    }

    fn run(&mut self, function: &mut SsaFunction, _context: &mut PassContext<'_>) -> PassChange {
        let actions = positions(function)
            .filter_map(|position| {
                let node = node(function, position)?;
                let SsaOp::Move { .. } = node.op else {
                    return None;
                };
                (!used_by_phi(function, node.dest)).then_some(position)
            })
            .collect::<Vec<_>>();

        for position in &actions {
            let Some((dest, source)) = node(function, *position).and_then(|node| {
                let SsaOp::Move { src } = node.op else {
                    return None;
                };
                (!used_by_phi(function, node.dest)).then_some((node.dest, src))
            }) else {
                continue;
            };
            rewrite_uses(function, dest, source);
            if let Some(node) = node_mut(function, *position) {
                node.make_nop();
            }
        }
        changed_if(!actions.is_empty())
    }
}

pub struct ConstantFolding;

impl SsaPass for ConstantFolding {
    fn name(&self) -> &'static str {
        "constant-folding"
    }

    fn run(&mut self, function: &mut SsaFunction, context: &mut PassContext<'_>) -> PassChange {
        let actions = positions(function)
            .filter_map(|position| {
                let value = fold_node(function, context, position)?;
                Some((position, value))
            })
            .collect::<Vec<_>>();

        for (position, value) in &actions {
            if let Some(node) = node_mut(function, *position) {
                node.op = SsaOp::LoadLiteral {
                    value: value.clone(),
                };
            }
        }
        changed_if(!actions.is_empty())
    }
}

pub struct DeadCodeElimination;

impl SsaPass for DeadCodeElimination {
    fn name(&self) -> &'static str {
        "dead-code-elimination"
    }

    fn run(&mut self, function: &mut SsaFunction, context: &mut PassContext<'_>) -> PassChange {
        let actions = positions(function)
            .filter(|position| {
                let Some(node) = node(function, *position) else {
                    return false;
                };
                if node.op.effects().blocks_reordering() {
                    return false;
                }
                let mut has_def = false;
                let mut used = false;
                node.visit_defs(|reference| {
                    has_def = true;
                    used |= context.use_count(function, reference) > 0;
                });
                has_def && !used
            })
            .collect::<Vec<_>>();

        for position in &actions {
            if let Some(node) = node_mut(function, *position) {
                node.make_nop();
            }
        }
        changed_if(!actions.is_empty())
    }
}

fn fold_node(
    function: &SsaFunction,
    context: &mut PassContext<'_>,
    position: NodePosition,
) -> Option<SsaLiteral> {
    let operation = &node(function, position)?.op;
    match operation {
        SsaOp::BinOp {
            op, left, right, ..
        } => {
            let left = resolve_literal(function, context, *left, &mut Vec::new())?.as_number()?;
            let right = resolve_literal(function, context, *right, &mut Vec::new())?.as_number()?;
            fold_binary(*op, left, right)
        }
        SsaOp::UnOp { op, value } => {
            let value = resolve_literal(function, context, *value, &mut Vec::new())?;
            fold_unary(*op, value, function.version)
        }
        _ => None,
    }
}

fn resolve_literal(
    function: &SsaFunction,
    context: &mut PassContext<'_>,
    reference: SsaRef,
    visiting: &mut Vec<SsaRef>,
) -> Option<SsaLiteral> {
    if let SsaRef::Const(index) = reference {
        return context
            .constants()
            .get(usize::try_from(index).ok()?)
            .map(literal_from_constant);
    }
    if visiting.contains(&reference) {
        return None;
    }
    let position = context.definition_position(function, reference)?;
    let operation = &node(function, position)?.op;
    visiting.push(reference);
    let value = match operation {
        SsaOp::LoadK { idx } => context
            .constants()
            .get(usize::try_from(*idx).ok()?)
            .map(literal_from_constant),
        SsaOp::LoadLiteral { value } => Some(value.clone()),
        SsaOp::Move { src } => resolve_literal(function, context, *src, visiting),
        _ => None,
    };
    visiting.pop();
    value
}

fn literal_from_constant(constant: &Constant) -> SsaLiteral {
    match constant {
        Constant::Nil => SsaLiteral::Nil,
        Constant::Boolean(value) => SsaLiteral::Boolean(*value),
        Constant::Number(value) => SsaLiteral::number(*value),
        Constant::Integer(value) => SsaLiteral::Integer(*value),
        Constant::Str(value) => SsaLiteral::Str(value.clone()),
    }
}

fn fold_binary(op: BinOp, left: f64, right: f64) -> Option<SsaLiteral> {
    let value = match op {
        BinOp::Add => left + right,
        BinOp::Sub => left - right,
        BinOp::Mul => left * right,
        BinOp::Div => left / right,
        BinOp::Mod if right != 0.0 => left - (left / right).floor() * right,
        BinOp::Pow
        | BinOp::IDiv
        | BinOp::BAnd
        | BinOp::BOr
        | BinOp::BXor
        | BinOp::Shl
        | BinOp::Shr => return None,
        BinOp::Mod => return None,
    };
    value.is_finite().then(|| SsaLiteral::number(value))
}

fn fold_unary(
    op: UnOp,
    value: SsaLiteral,
    target: crate::version::LuaTarget,
) -> Option<SsaLiteral> {
    match op {
        UnOp::Neg => Some(SsaLiteral::number(-value.as_number()?)),
        UnOp::Not => Some(SsaLiteral::Boolean(!is_truthy(&value))),
        UnOp::Len => match value {
            SsaLiteral::Str(value) => match target {
                crate::version::LuaTarget::V51 => Some(SsaLiteral::number(value.len() as f64)),
            },
            _ => None,
        },
        UnOp::BNot => None,
    }
}

fn is_truthy(value: &SsaLiteral) -> bool {
    !matches!(value, SsaLiteral::Nil | SsaLiteral::Boolean(false))
}

fn trivial_phi_source(dest: SsaRef, operands: &[SsaRef]) -> Option<SsaRef> {
    let mut sources = operands.iter().copied().filter(|operand| *operand != dest);
    let source = sources.next()?;
    sources.all(|operand| operand == source).then_some(source)
}

fn used_by_phi(function: &SsaFunction, reference: SsaRef) -> bool {
    function.blocks.iter().any(|block| {
        block.nodes.iter().any(|node| {
            let mut used = false;
            node.op.visit_uses(|operand, role| {
                used |= role == UseRole::Phi && operand == reference;
            });
            used
        })
    })
}

fn rewrite_uses(function: &mut SsaFunction, from: SsaRef, to: SsaRef) {
    for block in &mut function.blocks {
        for node in &mut block.nodes {
            node.op.rewrite_uses(|reference, _| {
                if *reference == from {
                    *reference = to;
                }
            });
        }
    }
}

fn positions(function: &SsaFunction) -> impl Iterator<Item = NodePosition> + '_ {
    function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(block, item)| {
            (0..item.nodes.len()).map(move |node| NodePosition { block, node })
        })
}

fn changed_if(changed: bool) -> PassChange {
    if changed {
        PassChange::changed(PreservedAnalyses::ControlFlow)
    } else {
        PassChange::unchanged()
    }
}
