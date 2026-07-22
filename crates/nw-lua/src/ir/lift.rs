//! Lift decoded Lua instructions into unversioned SSA nodes.

use crate::{
    bytecode::{Instruction, OpArgMode, OpcodeTable, OperandSlot, SemanticOp, opinfo},
    chunk::Proto,
};

use super::{BasicBlock, BinOp, LoopControl, RelOp, SsaNode, SsaOp, SsaRef, UnOp, UpvalueCapture};

/// Lift all basic blocks in-place.
pub fn lift_all(
    proto: &Proto,
    table: &OpcodeTable,
    instructions: &[Instruction],
    blocks: &mut [BasicBlock],
) {
    for block in blocks {
        lift_block(proto, table, instructions, block);
    }
}

fn lift_block(
    proto: &Proto,
    table: &OpcodeTable,
    instructions: &[Instruction],
    block: &mut BasicBlock,
) {
    let mut nodes = Vec::with_capacity(block.end_pc - block.start_pc + 1);
    let mut pc = block.start_pc;
    while pc <= block.end_pc {
        let inst = instructions[pc];
        let line = line_number(proto, pc);
        let pc_i32 = pc_to_i32(pc);
        match inst.op {
            SemanticOp::Move => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::Move {
                    src: operand_ref(table, inst, OperandSlot::B),
                },
            )),
            SemanticOp::LoadK => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::LoadK {
                    idx: const_index(inst.bx),
                },
            )),
            SemanticOp::LoadBool => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::LoadBool {
                    value: inst.b != 0,
                    skip_next: inst.c != 0,
                },
            )),
            SemanticOp::LoadNil => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::LoadNil {
                    start: reg_index(inst.a),
                    end: reg_index(inst.b),
                },
            )),
            SemanticOp::GetUpval => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::GetUpval {
                    upval: reg_index(inst.b),
                },
            )),
            SemanticOp::GetGlobal => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::GetGlobal {
                    idx: const_index(inst.bx),
                },
            )),
            SemanticOp::GetTable => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::GetTable {
                    table: operand_ref(table, inst, OperandSlot::B),
                    key: operand_ref(table, inst, OperandSlot::C),
                },
            )),
            SemanticOp::SetGlobal => nodes.push(SsaNode::new(
                pc_i32,
                line,
                SsaOp::SetGlobal {
                    src: reg_ref(inst.a),
                    idx: const_index(inst.bx),
                },
            )),
            SemanticOp::SetUpval => nodes.push(SsaNode::new(
                pc_i32,
                line,
                SsaOp::SetUpval {
                    src: reg_ref(inst.a),
                    upval: reg_index(inst.b),
                },
            )),
            SemanticOp::SetTable => nodes.push(SsaNode::new(
                pc_i32,
                line,
                SsaOp::SetTable {
                    table: reg_ref(inst.a),
                    key: operand_ref(table, inst, OperandSlot::B),
                    value: operand_ref(table, inst, OperandSlot::C),
                },
            )),
            SemanticOp::NewTable => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::NewTable {
                    array_hint: super::TableSizeHint::from_encoded(
                        u16::try_from(inst.b).expect("decoded B operand fits in u16"),
                    ),
                    hash_hint: super::TableSizeHint::from_encoded(
                        u16::try_from(inst.c).expect("decoded C operand fits in u16"),
                    ),
                },
            )),
            SemanticOp::SelfOp => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::SelfOp {
                    table: reg_ref(inst.b),
                    key: operand_ref(table, inst, OperandSlot::C),
                    self_reg: reg_index(inst.a + 1),
                },
            )),
            SemanticOp::Add
            | SemanticOp::Sub
            | SemanticOp::Mul
            | SemanticOp::Div
            | SemanticOp::Mod
            | SemanticOp::Pow
            | SemanticOp::Idiv
            | SemanticOp::Band
            | SemanticOp::Bor
            | SemanticOp::Bxor
            | SemanticOp::Shl
            | SemanticOp::Shr => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::BinOp {
                    op: bin_op(inst.op),
                    left: operand_ref(table, inst, OperandSlot::B),
                    right: operand_ref(table, inst, OperandSlot::C),
                },
            )),
            SemanticOp::Unm | SemanticOp::Not | SemanticOp::Len | SemanticOp::Bnot => {
                nodes.push(SsaNode::with_dest(
                    pc_i32,
                    line,
                    reg_ref(inst.a),
                    SsaOp::UnOp {
                        op: un_op(inst.op),
                        value: operand_ref(table, inst, OperandSlot::B),
                    },
                ));
            }
            SemanticOp::Concat => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::Concat {
                    operands: (inst.b..=inst.c).map(reg_ref).collect(),
                },
            )),
            SemanticOp::Jmp => nodes.push(SsaNode::new(
                pc_i32,
                line,
                SsaOp::Jump {
                    target: jump_target(pc, inst.sbx),
                },
            )),
            SemanticOp::Eq | SemanticOp::Lt | SemanticOp::Le => nodes.push(SsaNode::new(
                pc_i32,
                line,
                SsaOp::Branch {
                    rel: rel_op(inst.op),
                    a: operand_ref(table, inst, OperandSlot::B),
                    b: operand_ref(table, inst, OperandSlot::C),
                    invert: inst.a != 0,
                    t_true: pc_to_i32(pc + 2),
                    t_false: pc_to_i32(pc + 1),
                },
            )),
            SemanticOp::Test => nodes.push(SsaNode::new(
                pc_i32,
                line,
                SsaOp::Branch {
                    rel: RelOp::Test,
                    a: reg_ref(inst.a),
                    b: SsaRef::None,
                    invert: inst.c != 0,
                    t_true: pc_to_i32(pc + 2),
                    t_false: pc_to_i32(pc + 1),
                },
            )),
            SemanticOp::TestSet => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::Branch {
                    rel: RelOp::TestSet,
                    a: reg_ref(inst.b),
                    b: SsaRef::None,
                    invert: inst.c != 0,
                    t_true: pc_to_i32(pc + 2),
                    t_false: pc_to_i32(pc + 1),
                },
            )),
            SemanticOp::Call => nodes.push(lift_call(pc, line, inst, &nodes)),
            SemanticOp::TailCall => nodes.push(lift_tailcall(pc, line, inst, &nodes)),
            SemanticOp::Return => nodes.push(lift_return(pc, line, inst, &nodes)),
            SemanticOp::ForLoop => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a + 3),
                SsaOp::ForLoop {
                    control: LoopControl::from_base(reg_index(inst.a)),
                    target: jump_target(pc, inst.sbx),
                },
            )),
            SemanticOp::ForPrep => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::ForPrep {
                    control: LoopControl::from_base(reg_index(inst.a)),
                    target: jump_target(pc, inst.sbx),
                },
            )),
            SemanticOp::TForLoop => nodes.push(SsaNode::new(
                pc_i32,
                line,
                SsaOp::TForLoop {
                    control: LoopControl::from_base(reg_index(inst.a)),
                    count: inst.c,
                },
            )),
            SemanticOp::SetList => {
                let batch = if inst.c == 0 {
                    instructions
                        .get(pc + 1)
                        .and_then(|extra| i32::try_from(extra.raw).ok())
                        .unwrap_or(inst.c)
                } else {
                    inst.c
                };
                nodes.push(SsaNode::new(
                    pc_i32,
                    line,
                    SsaOp::SetList {
                        table: reg_ref(inst.a),
                        values: setlist_values(inst.a, inst.b, &nodes),
                        base: reg_index(inst.a),
                        count: inst.b,
                        batch,
                    },
                ));
                if inst.c == 0 && pc < block.end_pc {
                    pc += 1;
                    nodes.push(SsaNode::new(pc_to_i32(pc), -1, SsaOp::Nop));
                }
            }
            SemanticOp::Close => nodes.push(SsaNode::new(
                pc_i32,
                line,
                SsaOp::Close {
                    base: reg_index(inst.a),
                },
            )),
            SemanticOp::Closure => {
                let upvalues = closure_upvalues(proto, instructions, pc, inst.bx);
                nodes.push(SsaNode::with_dest(
                    pc_i32,
                    line,
                    reg_ref(inst.a),
                    SsaOp::Closure {
                        proto: const_index(inst.bx),
                        upvalues,
                    },
                ));
                if let Some(nested) = usize::try_from(inst.bx)
                    .ok()
                    .and_then(|idx| proto.protos.get(idx))
                {
                    for _ in 0..nested.nups {
                        pc += 1;
                        if pc > block.end_pc {
                            break;
                        }
                        nodes.push(SsaNode::new(pc_to_i32(pc), -1, SsaOp::Nop));
                    }
                }
            }
            SemanticOp::VarArg => nodes.push(SsaNode::with_dest(
                pc_i32,
                line,
                reg_ref(inst.a),
                SsaOp::VarArg {
                    base: reg_index(inst.a),
                    count: inst.b,
                },
            )),
            SemanticOp::Unknown => nodes.push(SsaNode::new(pc_i32, line, SsaOp::Nop)),
            _ => nodes.push(SsaNode::new(pc_i32, line, SsaOp::Nop)),
        }
        pc += 1;
    }
    block.nodes = nodes;
}

fn closure_upvalues(
    proto: &Proto,
    instructions: &[Instruction],
    closure_pc: usize,
    proto_idx: i32,
) -> Vec<UpvalueCapture> {
    let Some(nested) = usize::try_from(proto_idx)
        .ok()
        .and_then(|idx| proto.protos.get(idx))
    else {
        return Vec::new();
    };

    let mut captures = Vec::with_capacity(usize::from(nested.nups));
    for offset in 0..usize::from(nested.nups) {
        let Some(pseudo) = instructions.get(closure_pc + 1 + offset).copied() else {
            break;
        };
        match pseudo.op {
            SemanticOp::Move => captures.push(UpvalueCapture::ParentLocal(reg_ref(pseudo.b))),
            SemanticOp::GetUpval => {
                captures.push(UpvalueCapture::ParentUpvalue(reg_index(pseudo.b)));
            }
            _ => break,
        }
    }
    captures
}

fn lift_call(pc: usize, line: i32, inst: Instruction, previous: &[SsaNode]) -> SsaNode {
    let args = call_args(inst.a, inst.b, previous);
    SsaNode::with_dest(
        pc_to_i32(pc),
        line,
        reg_ref(inst.a),
        SsaOp::Call {
            func: reg_ref(inst.a),
            args,
            base: reg_index(inst.a),
            arg_count: inst.b,
            return_count: inst.c,
        },
    )
}

fn lift_tailcall(pc: usize, line: i32, inst: Instruction, previous: &[SsaNode]) -> SsaNode {
    let args = call_args(inst.a, inst.b, previous);
    SsaNode::new(
        pc_to_i32(pc),
        line,
        SsaOp::TailCall {
            func: reg_ref(inst.a),
            args,
            base: reg_index(inst.a),
            arg_count: inst.b,
            return_count: inst.c,
        },
    )
}

fn lift_return(pc: usize, line: i32, inst: Instruction, previous: &[SsaNode]) -> SsaNode {
    let values = if inst.b > 1 {
        (0..(inst.b - 1))
            .map(|offset| reg_ref(inst.a + offset))
            .collect()
    } else if inst.b == 0 {
        top_set_reg(previous)
            .filter(|top| i32::from(*top) >= inst.a)
            .map_or_else(Vec::new, |top| {
                (inst.a..=i32::from(top)).map(reg_ref).collect()
            })
    } else {
        Vec::new()
    };
    SsaNode::new(
        pc_to_i32(pc),
        line,
        SsaOp::Return {
            values,
            base: reg_index(inst.a),
            count: inst.b,
        },
    )
}

fn call_args(base: i32, arg_count: i32, previous: &[SsaNode]) -> Vec<SsaRef> {
    if arg_count > 1 {
        (0..(arg_count - 1))
            .map(|offset| reg_ref(base + 1 + offset))
            .collect()
    } else if arg_count == 0 {
        top_set_reg(previous)
            .filter(|top| i32::from(*top) > base)
            .map_or_else(Vec::new, |top| {
                ((base + 1)..=i32::from(top)).map(reg_ref).collect()
            })
    } else {
        Vec::new()
    }
}

fn setlist_values(base: i32, count: i32, previous: &[SsaNode]) -> Vec<SsaRef> {
    if count > 0 {
        (1..=count).map(|offset| reg_ref(base + offset)).collect()
    } else {
        top_set_reg(previous)
            .filter(|top| i32::from(*top) > base)
            .map_or_else(Vec::new, |top| {
                ((base + 1)..=i32::from(top)).map(reg_ref).collect()
            })
    }
}

fn top_set_reg(nodes: &[SsaNode]) -> Option<u16> {
    nodes.iter().rev().find_map(|node| match &node.op {
        SsaOp::Call {
            base,
            return_count: 0,
            ..
        }
        | SsaOp::VarArg { base, count: 0, .. } => Some(*base),
        _ => None,
    })
}

fn operand_ref(table: &OpcodeTable, inst: Instruction, slot: OperandSlot) -> SsaRef {
    let field = match slot {
        OperandSlot::B => inst.b,
        OperandSlot::C => inst.c,
    };
    match opinfo::info_for(inst.op).operand_mode(slot) {
        OpArgMode::K if table.is_k(field) => SsaRef::constant(const_index(table.rk_index(field))),
        OpArgMode::K | OpArgMode::R => reg_ref(field),
        OpArgMode::U | OpArgMode::N => SsaRef::None,
    }
}

fn line_number(proto: &Proto, pc: usize) -> i32 {
    proto.line_info.get(pc).copied().unwrap_or(-1)
}

fn bin_op(op: SemanticOp) -> BinOp {
    match op {
        SemanticOp::Add => BinOp::Add,
        SemanticOp::Sub => BinOp::Sub,
        SemanticOp::Mul => BinOp::Mul,
        SemanticOp::Div => BinOp::Div,
        SemanticOp::Mod => BinOp::Mod,
        SemanticOp::Pow => BinOp::Pow,
        SemanticOp::Idiv => BinOp::IDiv,
        SemanticOp::Band => BinOp::BAnd,
        SemanticOp::Bor => BinOp::BOr,
        SemanticOp::Bxor => BinOp::BXor,
        SemanticOp::Shl => BinOp::Shl,
        SemanticOp::Shr => BinOp::Shr,
        _ => BinOp::Add,
    }
}

fn un_op(op: SemanticOp) -> UnOp {
    match op {
        SemanticOp::Unm => UnOp::Neg,
        SemanticOp::Not => UnOp::Not,
        SemanticOp::Len => UnOp::Len,
        SemanticOp::Bnot => UnOp::BNot,
        _ => UnOp::Neg,
    }
}

fn rel_op(op: SemanticOp) -> RelOp {
    match op {
        SemanticOp::Eq => RelOp::Eq,
        SemanticOp::Lt => RelOp::Lt,
        SemanticOp::Le => RelOp::Le,
        _ => RelOp::Eq,
    }
}

fn jump_target(pc: usize, sbx: i32) -> i32 {
    pc_to_i32(pc) + 1 + sbx
}

fn reg_ref(reg: i32) -> SsaRef {
    u16::try_from(reg).map_or(SsaRef::None, SsaRef::reg)
}

fn reg_index(reg: i32) -> u16 {
    u16::try_from(reg).unwrap_or(0)
}

fn const_index(idx: i32) -> u32 {
    u32::try_from(idx).unwrap_or(0)
}

fn pc_to_i32(pc: usize) -> i32 {
    i32::try_from(pc).unwrap_or(i32::MAX)
}
