//! Shared opcode role and control-flow metadata.
//!
//! The Lua 5.1 entries mirror `luaP_opmodes` from PUC-Rio Lua 5.1.5.

use super::{Instruction, InstructionFormat, SemanticOp};

/// PUC-Rio operand role from `OpArgMask`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpArgMode {
    /// Operand is not used.
    N,
    /// Operand is used as an unsigned immediate or index.
    U,
    /// Operand is a register or jump offset.
    R,
    /// Operand is RK-encoded: register or constant.
    K,
}

/// B/C operand selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandSlot {
    /// B operand.
    B,
    /// C operand.
    C,
}

/// Control-flow classification used by CFG construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlFlowClass {
    /// Instruction performs an explicit jump.
    pub is_jump: bool,
    /// Instruction is a conditional test/skip.
    pub is_conditional_test: bool,
    /// Instruction is a call.
    pub is_call: bool,
    /// Instruction returns from the current function.
    pub is_return: bool,
    /// Instruction participates in loop control.
    pub is_loop: bool,
    /// Instruction can continue to the next instruction.
    pub falls_through: bool,
    /// Instruction ends its basic block.
    pub is_terminator: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowKind {
    Linear,
    JumpSbx,
    ConditionalSkipNext,
    LoadBoolSkip,
    Return,
    ForLoop,
    ForPrep,
    TForLoop,
}

/// Full shared opcode descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpInfo {
    /// B operand role.
    pub b_mode: OpArgMode,
    /// C operand role.
    pub c_mode: OpArgMode,
    /// Whether the PUC opmode table marks A as assigned.
    pub sets_a: bool,
    /// Whether the PUC opmode table marks this as a test.
    pub test: bool,
    /// Raw instruction format.
    pub format: InstructionFormat,
    /// Control-flow classification.
    pub control_flow: ControlFlowClass,
    flow_kind: FlowKind,
}

impl OpInfo {
    /// Return the role for B or C.
    #[must_use]
    pub const fn operand_mode(self, slot: OperandSlot) -> OpArgMode {
        match slot {
            OperandSlot::B => self.b_mode,
            OperandSlot::C => self.c_mode,
        }
    }

    /// Return whether this B/C operand is RK-encoded.
    #[must_use]
    pub const fn is_rk_operand(self, slot: OperandSlot) -> bool {
        matches!(self.operand_mode(slot), OpArgMode::K)
    }

    /// Additional block starts introduced by this instruction.
    #[must_use]
    pub fn leader_pcs(self, pc: usize, inst: Instruction, code_len: usize) -> Vec<usize> {
        let mut leaders = Vec::with_capacity(2);
        match self.flow_kind {
            FlowKind::Linear => {}
            FlowKind::JumpSbx => {
                push_target(&mut leaders, pc, inst.sbx, code_len);
                push_pc(&mut leaders, pc + 1, code_len);
            }
            FlowKind::ConditionalSkipNext | FlowKind::TForLoop => {
                push_pc(&mut leaders, pc + 1, code_len);
                push_pc(&mut leaders, pc + 2, code_len);
            }
            FlowKind::LoadBoolSkip => {
                if inst.c != 0 {
                    push_pc(&mut leaders, pc + 1, code_len);
                    push_pc(&mut leaders, pc + 2, code_len);
                }
            }
            FlowKind::Return => {
                push_pc(&mut leaders, pc + 1, code_len);
            }
            FlowKind::ForLoop => {
                push_target(&mut leaders, pc, inst.sbx, code_len);
                push_pc(&mut leaders, pc + 1, code_len);
            }
            FlowKind::ForPrep => {
                push_target(&mut leaders, pc, inst.sbx, code_len);
                push_pc(&mut leaders, pc + 1, code_len);
            }
        }
        leaders
    }

    /// Successor instruction PCs for a block ending with this instruction.
    #[must_use]
    pub fn successor_pcs(self, pc: usize, inst: Instruction, code_len: usize) -> Vec<usize> {
        let mut succs = Vec::with_capacity(2);
        match self.flow_kind {
            FlowKind::Linear => push_pc(&mut succs, pc + 1, code_len),
            FlowKind::JumpSbx => push_target(&mut succs, pc, inst.sbx, code_len),
            FlowKind::ConditionalSkipNext | FlowKind::TForLoop => {
                push_pc(&mut succs, pc + 1, code_len);
                push_pc(&mut succs, pc + 2, code_len);
            }
            FlowKind::LoadBoolSkip => {
                if inst.c != 0 {
                    push_pc(&mut succs, pc + 2, code_len);
                } else {
                    push_pc(&mut succs, pc + 1, code_len);
                }
            }
            FlowKind::Return => {}
            FlowKind::ForLoop => {
                push_target(&mut succs, pc, inst.sbx, code_len);
                push_pc(&mut succs, pc + 1, code_len);
            }
            FlowKind::ForPrep => push_target(&mut succs, pc, inst.sbx, code_len),
        }
        succs
    }
}

/// Return the shared descriptor for a semantic opcode.
#[must_use]
pub const fn info_for(op: SemanticOp) -> OpInfo {
    use InstructionFormat::{Abc, Abx, AsBx};
    use OpArgMode::{K, N, R, U};
    use SemanticOp as Op;

    match op {
        Op::Move => opmode(false, true, R, N, Abc, FlowKind::Linear),
        Op::LoadK => opmode(false, true, K, N, Abx, FlowKind::Linear),
        Op::LoadBool => opmode(false, true, U, U, Abc, FlowKind::LoadBoolSkip),
        Op::LoadNil => opmode(false, true, R, N, Abc, FlowKind::Linear),
        Op::GetUpval => opmode(false, true, U, N, Abc, FlowKind::Linear),
        Op::GetGlobal => opmode(false, true, K, N, Abx, FlowKind::Linear),
        Op::GetTable => opmode(false, true, R, K, Abc, FlowKind::Linear),
        Op::SetGlobal => opmode(false, false, K, N, Abx, FlowKind::Linear),
        Op::SetUpval => opmode(false, false, U, N, Abc, FlowKind::Linear),
        Op::SetTable => opmode(false, false, K, K, Abc, FlowKind::Linear),
        Op::NewTable => opmode(false, true, U, U, Abc, FlowKind::Linear),
        Op::SelfOp => opmode(false, true, R, K, Abc, FlowKind::Linear),
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Pow => {
            opmode(false, true, K, K, Abc, FlowKind::Linear)
        }
        Op::Unm | Op::Not | Op::Len => opmode(false, true, R, N, Abc, FlowKind::Linear),
        Op::Concat => opmode(false, true, R, R, Abc, FlowKind::Linear),
        Op::Jmp => opmode(false, false, R, N, AsBx, FlowKind::JumpSbx),
        Op::Eq | Op::Lt | Op::Le => opmode(true, false, K, K, Abc, FlowKind::ConditionalSkipNext),
        Op::Test | Op::TestSet => opmode(true, true, R, U, Abc, FlowKind::ConditionalSkipNext),
        Op::Call => opmode_call(false, true, U, U, Abc, FlowKind::Linear),
        Op::TailCall => opmode_call(false, true, U, U, Abc, FlowKind::Return),
        Op::Return => opmode(false, false, U, N, Abc, FlowKind::Return),
        Op::ForLoop => opmode(false, true, R, N, AsBx, FlowKind::ForLoop),
        Op::ForPrep => opmode(false, true, R, N, AsBx, FlowKind::ForPrep),
        Op::TForLoop => opmode(true, false, N, U, Abc, FlowKind::TForLoop),
        Op::SetList => opmode(false, false, U, U, Abc, FlowKind::Linear),
        Op::Close => opmode(false, false, N, N, Abc, FlowKind::Linear),
        Op::Closure => opmode(false, true, U, N, Abx, FlowKind::Linear),
        Op::VarArg => opmode(false, true, U, N, Abc, FlowKind::Linear),
        _ => opmode(false, false, U, U, Abc, FlowKind::Linear),
    }
}

const fn opmode(
    test: bool,
    sets_a: bool,
    b_mode: OpArgMode,
    c_mode: OpArgMode,
    format: InstructionFormat,
    flow_kind: FlowKind,
) -> OpInfo {
    opmode_with_call(test, sets_a, b_mode, c_mode, format, flow_kind, false)
}

const fn opmode_call(
    test: bool,
    sets_a: bool,
    b_mode: OpArgMode,
    c_mode: OpArgMode,
    format: InstructionFormat,
    flow_kind: FlowKind,
) -> OpInfo {
    opmode_with_call(test, sets_a, b_mode, c_mode, format, flow_kind, true)
}

const fn opmode_with_call(
    test: bool,
    sets_a: bool,
    b_mode: OpArgMode,
    c_mode: OpArgMode,
    format: InstructionFormat,
    flow_kind: FlowKind,
    is_call: bool,
) -> OpInfo {
    OpInfo {
        b_mode,
        c_mode,
        sets_a,
        test,
        format,
        control_flow: control_flow(flow_kind, is_call),
        flow_kind,
    }
}

const fn control_flow(flow_kind: FlowKind, is_call: bool) -> ControlFlowClass {
    match flow_kind {
        FlowKind::Linear => ControlFlowClass {
            is_jump: false,
            is_conditional_test: false,
            is_call,
            is_return: false,
            is_loop: false,
            falls_through: true,
            is_terminator: false,
        },
        FlowKind::JumpSbx | FlowKind::ForPrep => ControlFlowClass {
            is_jump: true,
            is_conditional_test: false,
            is_call,
            is_return: false,
            is_loop: matches!(flow_kind, FlowKind::ForPrep),
            falls_through: false,
            is_terminator: true,
        },
        FlowKind::ConditionalSkipNext | FlowKind::TForLoop => ControlFlowClass {
            is_jump: false,
            is_conditional_test: true,
            is_call,
            is_return: false,
            is_loop: matches!(flow_kind, FlowKind::TForLoop),
            falls_through: true,
            is_terminator: true,
        },
        FlowKind::LoadBoolSkip => ControlFlowClass {
            is_jump: false,
            is_conditional_test: false,
            is_call,
            is_return: false,
            is_loop: false,
            falls_through: true,
            is_terminator: false,
        },
        FlowKind::Return => ControlFlowClass {
            is_jump: false,
            is_conditional_test: false,
            is_call,
            is_return: true,
            is_loop: false,
            falls_through: false,
            is_terminator: true,
        },
        FlowKind::ForLoop => ControlFlowClass {
            is_jump: true,
            is_conditional_test: true,
            is_call,
            is_return: false,
            is_loop: true,
            falls_through: true,
            is_terminator: true,
        },
    }
}

fn push_target(out: &mut Vec<usize>, pc: usize, sbx: i32, code_len: usize) {
    let Ok(pc_i32) = i32::try_from(pc) else {
        return;
    };
    let target = pc_i32 + 1 + sbx;
    if let Ok(target) = usize::try_from(target) {
        push_pc(out, target, code_len);
    }
}

fn push_pc(out: &mut Vec<usize>, pc: usize, code_len: usize) {
    if pc < code_len && !out.contains(&pc) {
        out.push(pc);
    }
}
