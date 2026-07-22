//! Operand, definition, effect, and control-flow capabilities for SSA operations.

use super::{SsaNode, SsaOp, SsaRef, UpvalueCapture};

/// Semantic role of an SSA value use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseRole {
    Value,
    Phi,
    MutatingTable,
    UpvalueCapture,
    LoopControl,
}

/// The three internal values consumed by a Lua numeric or generic `for` step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopControl {
    values: [SsaRef; 3],
}

impl LoopControl {
    #[must_use]
    pub const fn from_base(base: u16) -> Self {
        Self {
            values: [
                SsaRef::reg(base),
                SsaRef::reg(base.saturating_add(1)),
                SsaRef::reg(base.saturating_add(2)),
            ],
        }
    }

    /// First register in the control window.
    #[must_use]
    pub const fn base(self) -> u16 {
        match self.values[0] {
            SsaRef::Reg { reg, .. } => reg,
            SsaRef::None | SsaRef::Const(_) => 0,
        }
    }

    #[must_use]
    pub const fn values(self) -> [SsaRef; 3] {
        self.values
    }

    fn values_mut(&mut self) -> &mut [SsaRef; 3] {
        &mut self.values
    }
}

/// Observable operation effects relevant to motion and elimination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpEffects {
    pub may_invoke: bool,
    pub mutates_state: bool,
    pub terminates_flow: bool,
}

impl OpEffects {
    /// Whether evaluating this operation can invoke user code or mutate visible state.
    #[must_use]
    pub const fn is_observable(self) -> bool {
        self.may_invoke || self.mutates_state
    }

    /// Whether moving another evaluation across this operation can change Lua behavior.
    #[must_use]
    pub const fn blocks_reordering(self) -> bool {
        self.may_invoke || self.mutates_state || self.terminates_flow
    }
}

/// Structural control-flow role of an SSA operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFlowRole {
    Linear,
    Jump,
    Branch,
    Return,
    LoopPrep,
    LoopLatch,
}

impl SsaOp {
    /// Visit every explicit, versioned SSA use exactly once.
    pub fn visit_uses(&self, mut visit: impl FnMut(SsaRef, UseRole)) {
        match self {
            Self::Move { src } => visit(*src, UseRole::Value),
            Self::GetTable { table, key } | Self::SelfOp { table, key, .. } => {
                visit(*table, UseRole::Value);
                visit(*key, UseRole::Value);
            }
            Self::SetGlobal { src, .. } | Self::SetUpval { src, .. } => {
                visit(*src, UseRole::Value);
            }
            Self::SetTable { table, key, value } => {
                visit(*table, UseRole::MutatingTable);
                visit(*key, UseRole::Value);
                visit(*value, UseRole::Value);
            }
            Self::BinOp { left, right, .. } => {
                visit(*left, UseRole::Value);
                visit(*right, UseRole::Value);
            }
            Self::UnOp { value, .. } => visit(*value, UseRole::Value),
            Self::Concat { operands } => {
                for operand in operands {
                    visit(*operand, UseRole::Value);
                }
            }
            Self::Branch { a, b, .. } => {
                visit(*a, UseRole::Value);
                visit(*b, UseRole::Value);
            }
            Self::Call { func, args, .. } | Self::TailCall { func, args, .. } => {
                visit(*func, UseRole::Value);
                for arg in args {
                    visit(*arg, UseRole::Value);
                }
            }
            Self::Return { values, .. } => {
                for value in values {
                    visit(*value, UseRole::Value);
                }
            }
            Self::SetList { table, values, .. } => {
                visit(*table, UseRole::MutatingTable);
                for value in values {
                    visit(*value, UseRole::Value);
                }
            }
            Self::Phi { operands, .. } => {
                for operand in operands {
                    visit(*operand, UseRole::Phi);
                }
            }
            Self::Closure { upvalues, .. } => {
                for capture in upvalues {
                    if let UpvalueCapture::ParentLocal(reference) = capture {
                        visit(*reference, UseRole::UpvalueCapture);
                    }
                }
            }
            Self::ForPrep { control, .. }
            | Self::ForLoop { control, .. }
            | Self::TForLoop { control, .. } => {
                for reference in control.values() {
                    visit(reference, UseRole::LoopControl);
                }
            }
            Self::Nop
            | Self::LoadK { .. }
            | Self::LoadLiteral { .. }
            | Self::LoadBool { .. }
            | Self::LoadNil { .. }
            | Self::GetUpval { .. }
            | Self::GetGlobal { .. }
            | Self::NewTable { .. }
            | Self::Jump { .. }
            | Self::Close { .. }
            | Self::VarArg { .. } => {}
        }
    }

    /// Rewrite every explicit SSA use in place.
    pub fn rewrite_uses(&mut self, mut rewrite: impl FnMut(&mut SsaRef, UseRole)) {
        match self {
            Self::Move { src } => rewrite(src, UseRole::Value),
            Self::GetTable { table, key } | Self::SelfOp { table, key, .. } => {
                rewrite(table, UseRole::Value);
                rewrite(key, UseRole::Value);
            }
            Self::SetGlobal { src, .. } | Self::SetUpval { src, .. } => {
                rewrite(src, UseRole::Value);
            }
            Self::SetTable { table, key, value } => {
                rewrite(table, UseRole::MutatingTable);
                rewrite(key, UseRole::Value);
                rewrite(value, UseRole::Value);
            }
            Self::BinOp { left, right, .. } => {
                rewrite(left, UseRole::Value);
                rewrite(right, UseRole::Value);
            }
            Self::UnOp { value, .. } => rewrite(value, UseRole::Value),
            Self::Concat { operands } => {
                for operand in operands {
                    rewrite(operand, UseRole::Value);
                }
            }
            Self::Branch { a, b, .. } => {
                rewrite(a, UseRole::Value);
                rewrite(b, UseRole::Value);
            }
            Self::Call { func, args, .. } | Self::TailCall { func, args, .. } => {
                rewrite(func, UseRole::Value);
                for arg in args {
                    rewrite(arg, UseRole::Value);
                }
            }
            Self::Return { values, .. } => {
                for value in values {
                    rewrite(value, UseRole::Value);
                }
            }
            Self::SetList { table, values, .. } => {
                rewrite(table, UseRole::MutatingTable);
                for value in values {
                    rewrite(value, UseRole::Value);
                }
            }
            Self::Phi { operands, .. } => {
                for operand in operands {
                    rewrite(operand, UseRole::Phi);
                }
            }
            Self::Closure { upvalues, .. } => {
                for capture in upvalues {
                    if let UpvalueCapture::ParentLocal(reference) = capture {
                        rewrite(reference, UseRole::UpvalueCapture);
                    }
                }
            }
            Self::ForPrep { control, .. }
            | Self::ForLoop { control, .. }
            | Self::TForLoop { control, .. } => {
                for reference in control.values_mut() {
                    rewrite(reference, UseRole::LoopControl);
                }
            }
            Self::Nop
            | Self::LoadK { .. }
            | Self::LoadLiteral { .. }
            | Self::LoadBool { .. }
            | Self::LoadNil { .. }
            | Self::GetUpval { .. }
            | Self::GetGlobal { .. }
            | Self::NewTable { .. }
            | Self::Jump { .. }
            | Self::Close { .. }
            | Self::VarArg { .. } => {}
        }
    }

    #[must_use]
    pub const fn effects(&self) -> OpEffects {
        match self {
            Self::GetGlobal { .. }
            | Self::GetTable { .. }
            | Self::SelfOp { .. }
            | Self::BinOp { .. }
            | Self::UnOp { .. }
            | Self::Concat { .. }
            | Self::Call { .. }
            | Self::TailCall { .. } => OpEffects {
                may_invoke: true,
                mutates_state: false,
                terminates_flow: matches!(self, Self::TailCall { .. }),
            },
            Self::SetGlobal { .. } | Self::SetTable { .. } => OpEffects {
                may_invoke: true,
                mutates_state: true,
                terminates_flow: false,
            },
            Self::SetUpval { .. } | Self::SetList { .. } | Self::Close { .. } => OpEffects {
                may_invoke: false,
                mutates_state: true,
                terminates_flow: false,
            },
            Self::Return { .. } => OpEffects {
                may_invoke: false,
                mutates_state: false,
                terminates_flow: true,
            },
            _ => OpEffects {
                may_invoke: false,
                mutates_state: false,
                terminates_flow: false,
            },
        }
    }

    #[must_use]
    pub const fn control_flow_role(&self) -> ControlFlowRole {
        match self {
            Self::Jump { .. } => ControlFlowRole::Jump,
            Self::Branch { .. } => ControlFlowRole::Branch,
            Self::Return { .. } | Self::TailCall { .. } => ControlFlowRole::Return,
            Self::ForPrep { .. } => ControlFlowRole::LoopPrep,
            Self::ForLoop { .. } | Self::TForLoop { .. } => ControlFlowRole::LoopLatch,
            _ => ControlFlowRole::Linear,
        }
    }
}

impl SsaNode {
    /// Visit every versioned definition produced by this instruction.
    pub fn visit_defs(&self, mut visit: impl FnMut(SsaRef)) {
        if self.dest != SsaRef::None {
            visit(self.dest);
        }
        for reference in &self.secondary_defs {
            visit(*reference);
        }
    }

    /// Return the definition for one physical register, if this node writes it.
    #[must_use]
    pub fn def_at_reg(&self, reg: u16) -> Option<SsaRef> {
        if self.dest.reg_index() == Some(reg) {
            return Some(self.dest);
        }
        self.secondary_defs
            .iter()
            .copied()
            .find(|reference| reference.reg_index() == Some(reg))
    }

    pub(crate) fn rewrite_secondary_defs(&mut self, mut rewrite: impl FnMut(&mut SsaRef)) {
        for reference in &mut self.secondary_defs {
            rewrite(reference);
        }
    }

    pub(crate) fn make_nop(&mut self) {
        self.dest = SsaRef::None;
        self.secondary_defs.clear();
        self.op = SsaOp::Nop;
    }
}

pub(super) fn secondary_defs(op: &SsaOp, primary: SsaRef) -> Vec<SsaRef> {
    let primary_reg = primary.reg_index();
    let mut defs = Vec::new();
    match op {
        SsaOp::LoadNil { start, end } => extend_registers(&mut defs, *start, *end, primary_reg),
        SsaOp::ForLoop { control, .. } => push_register(&mut defs, control.base(), primary_reg),
        SsaOp::TForLoop { control, count } => {
            let start = control.base().saturating_add(3);
            let len = u16::try_from((*count).max(0)).unwrap_or(u16::MAX);
            extend_len(&mut defs, start, len, primary_reg);
        }
        SsaOp::SelfOp { self_reg, .. } => push_register(&mut defs, *self_reg, primary_reg),
        SsaOp::Call {
            base, return_count, ..
        } if *return_count >= 3 => {
            let len = u16::try_from(*return_count - 2).unwrap_or(u16::MAX);
            extend_len(&mut defs, base.saturating_add(1), len, primary_reg);
        }
        SsaOp::VarArg { base, count } if *count >= 3 => {
            let len = u16::try_from(*count - 2).unwrap_or(u16::MAX);
            extend_len(&mut defs, base.saturating_add(1), len, primary_reg);
        }
        _ => {}
    }
    defs
}

fn extend_len(out: &mut Vec<SsaRef>, start: u16, len: u16, except: Option<u16>) {
    if len == 0 {
        return;
    }
    extend_registers(
        out,
        start,
        start.saturating_add(len.saturating_sub(1)),
        except,
    );
}

fn extend_registers(out: &mut Vec<SsaRef>, start: u16, end: u16, except: Option<u16>) {
    if start > end {
        return;
    }
    for reg in start..=end {
        push_register(out, reg, except);
    }
}

fn push_register(out: &mut Vec<SsaRef>, reg: u16, except: Option<u16>) {
    if Some(reg) != except {
        out.push(SsaRef::reg(reg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::RelOp;

    #[test]
    fn use_roles_and_rewrites_share_the_same_operands() {
        let mut op = SsaOp::SetTable {
            table: SsaRef::reg(0),
            key: SsaRef::reg(1),
            value: SsaRef::reg(2),
        };
        let mut before = Vec::new();
        op.visit_uses(|reference, role| before.push((reference, role)));

        op.rewrite_uses(|reference, _| reference.set_version(7));

        let mut after = Vec::new();
        op.visit_uses(|reference, role| after.push((reference, role)));
        assert_eq!(
            before.iter().map(|item| item.1).collect::<Vec<_>>(),
            [UseRole::MutatingTable, UseRole::Value, UseRole::Value,]
        );
        assert!(
            after
                .iter()
                .all(|(reference, _)| reference.version() == Some(7))
        );
    }

    #[test]
    fn loop_controls_are_versioned_operands() {
        let mut op = SsaOp::Branch {
            rel: RelOp::Test,
            a: SsaRef::reg(0),
            b: SsaRef::None,
            invert: false,
            t_true: 2,
            t_false: 1,
        };
        op.rewrite_uses(|reference, _| reference.set_version(3));
        let mut uses = Vec::new();
        op.visit_uses(|reference, role| uses.push((reference, role)));
        assert_eq!(uses[0], (SsaRef::Reg { reg: 0, ver: 3 }, UseRole::Value));

        let mut loop_op = SsaOp::ForLoop {
            control: LoopControl::from_base(4),
            target: 0,
        };
        loop_op.rewrite_uses(|reference, _| reference.set_version(9));
        let mut controls = Vec::new();
        loop_op.visit_uses(|reference, role| controls.push((reference, role)));
        assert_eq!(controls.len(), 3);
        assert!(controls.iter().all(|(reference, role)| {
            reference.version() == Some(9) && *role == UseRole::LoopControl
        }));
    }

    #[test]
    fn multi_register_definitions_belong_to_their_node() {
        let call = SsaNode::with_dest(
            0,
            -1,
            SsaRef::reg(4),
            SsaOp::Call {
                func: SsaRef::reg(4),
                args: Vec::new(),
                base: 4,
                arg_count: 1,
                return_count: 4,
            },
        );
        let mut call_defs = Vec::new();
        call.visit_defs(|reference| call_defs.push(reference));
        assert_eq!(call_defs, [SsaRef::reg(4), SsaRef::reg(5), SsaRef::reg(6)]);

        let load_nil =
            SsaNode::with_dest(1, -1, SsaRef::reg(1), SsaOp::LoadNil { start: 1, end: 3 });
        let mut nil_defs = Vec::new();
        load_nil.visit_defs(|reference| nil_defs.push(reference));
        assert_eq!(nil_defs, [SsaRef::reg(1), SsaRef::reg(2), SsaRef::reg(3)]);
    }

    #[test]
    fn effects_and_control_roles_are_semantic_facts() {
        assert!(
            SsaOp::GetTable {
                table: SsaRef::reg(0),
                key: SsaRef::constant(0),
            }
            .effects()
            .may_invoke
        );
        assert!(
            SsaOp::SetList {
                table: SsaRef::reg(0),
                values: Vec::new(),
                base: 0,
                count: 0,
                batch: 1,
            }
            .effects()
            .mutates_state
        );
        assert_eq!(
            SsaOp::ForPrep {
                control: LoopControl::from_base(0),
                target: 4,
            }
            .control_flow_role(),
            ControlFlowRole::LoopPrep
        );
    }
}
