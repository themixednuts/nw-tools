//! Textual Lua bytecode disassembly.

use std::fmt::Write as _;

use crate::{
    bytecode::{
        Instruction, InstructionFormat, OpArgMode, OpcodeTable, OperandSlot, SemanticOp, opinfo,
    },
    chunk::{Chunk, Constant, LocVar, Proto, UpvalDesc},
    version::LuaVersion,
};

/// Disassemble a parsed chunk using its built-in opcode table.
#[must_use]
pub fn disassemble_chunk(chunk: &Chunk) -> String {
    let Ok(table) = OpcodeTable::builtin(chunk.header.version) else {
        return format!(
            "-- unsupported Lua {} disassembly --\n",
            version_label(chunk.header.version)
        );
    };

    let mut out = String::new();
    writeln!(
        out,
        "-- Lua {} Disassembly --",
        version_label(chunk.header.version)
    )
    .expect("writing to String cannot fail");
    writeln!(out).expect("writing to String cannot fail");
    write_proto_recursive(&mut out, &chunk.root, &table, 0);
    out
}

/// Disassemble a prototype and all nested prototypes.
#[must_use]
pub fn disassemble_proto(proto: &Proto, table: &OpcodeTable) -> String {
    let mut out = String::new();
    write_proto_recursive(&mut out, proto, table, 0);
    out
}

fn write_proto_recursive(out: &mut String, proto: &Proto, table: &OpcodeTable, indent: usize) {
    write_one_proto(out, proto, table, indent);
    for nested in &proto.protos {
        write_proto_recursive(out, nested, table, indent + 1);
    }
}

fn write_one_proto(out: &mut String, proto: &Proto, table: &OpcodeTable, indent: usize) {
    let pad = "  ".repeat(indent);
    let source = if proto.source.is_empty() {
        "(?)".to_string()
    } else {
        escape_bytes(proto.source.as_slice())
    };

    writeln!(
        out,
        "{pad}== function {source}:{}..{} ==",
        proto.line_defined, proto.last_line_defined
    )
    .expect("writing to String cannot fail");
    writeln!(
        out,
        "{pad}   params={} is_vararg={} maxstack={} upvals={} locals={} constants={} sub-protos={}",
        proto.num_params,
        proto.is_vararg,
        proto.max_stack,
        proto.nups,
        proto.loc_vars.len(),
        proto.constants.len(),
        proto.protos.len()
    )
    .expect("writing to String cannot fail");

    if !proto.constants.is_empty() {
        writeln!(out, "{pad}   -- constants --").expect("writing to String cannot fail");
        for (index, constant) in proto.constants.iter().enumerate() {
            writeln!(out, "{pad}   K{index} = {}", format_constant(constant))
                .expect("writing to String cannot fail");
        }
    }

    if !proto.loc_vars.is_empty() {
        writeln!(out, "{pad}   -- locals --").expect("writing to String cannot fail");
        for (index, local) in proto.loc_vars.iter().enumerate() {
            writeln!(
                out,
                "{pad}   L{index} \"{}\" [pc {}..{}]",
                local_name(local),
                local.start_pc,
                local.end_pc - 1
            )
            .expect("writing to String cannot fail");
        }
    }

    if !proto.upvalues.is_empty() {
        writeln!(out, "{pad}   -- upvalues --").expect("writing to String cannot fail");
        for (index, upvalue) in proto.upvalues.iter().enumerate() {
            writeln!(out, "{pad}   U{index} \"{}\"", upvalue_name(upvalue, index))
                .expect("writing to String cannot fail");
        }
    }

    writeln!(out, "{pad}   -- code --").expect("writing to String cannot fail");
    for pc in 0..proto.code.len() {
        writeln!(out, "{pad}   {}", format_instruction(proto, table, pc))
            .expect("writing to String cannot fail");
    }
    writeln!(out).expect("writing to String cannot fail");
}

fn format_instruction(proto: &Proto, table: &OpcodeTable, pc: usize) -> String {
    let inst = table.decode(proto.code[pc]);
    let op_name = if inst.op == SemanticOp::Unknown {
        format!("OP_{}", table.raw_opcode(inst.raw))
    } else {
        inst.op.name().to_string()
    };
    let (operands, annotation) = format_operands(proto, table, inst, pc);

    let instruction_text = if operands.is_empty() {
        format!("{op_name:<12}")
    } else {
        format!("{op_name:<12}{operands}")
    };

    let mut line = format!("[{:>4}] ", pc + 1);
    if let Some(line_number) = line_number(proto, pc) {
        line.push_str(&format!("{:<12}", format!("(line {line_number})")));
    } else {
        line.push_str(&" ".repeat(12));
    }
    line.push_str(&format!("{instruction_text:<42}"));
    if !annotation.is_empty() {
        line.push_str("; ");
        line.push_str(&annotation);
    }
    line
}

fn format_operands(
    proto: &Proto,
    table: &OpcodeTable,
    inst: Instruction,
    pc: usize,
) -> (String, String) {
    match inst.op {
        SemanticOp::Move => (
            format!(
                "{}  {}",
                reg_name(proto, inst.a, pc),
                reg_name(proto, inst.b, pc)
            ),
            format!("R{} := R{}", inst.a, inst.b),
        ),
        SemanticOp::LoadK => (
            format!("{}  {}", reg_name(proto, inst.a, pc), k_str(proto, inst.bx)),
            get_constant(proto, inst.bx).map_or_else(String::new, format_constant),
        ),
        SemanticOp::LoadBool => {
            let mut annotation = format!("R{} := {}", inst.a, inst.b);
            if inst.c != 0 {
                annotation.push_str("; skip next");
            }
            (
                format!("{}  {}  {}", reg_name(proto, inst.a, pc), inst.b, inst.c),
                annotation,
            )
        }
        SemanticOp::LoadNil => (
            format!("{}  R{}", reg_name(proto, inst.a, pc), inst.b),
            format!("R{}..R{} := nil", inst.a, inst.b),
        ),
        SemanticOp::GetUpval => (
            format!("{}  {}", reg_name(proto, inst.a, pc), inst.b),
            upvalue_ref(proto, inst.b),
        ),
        SemanticOp::GetGlobal => (
            format!("{}  {}", reg_name(proto, inst.a, pc), k_str(proto, inst.bx)),
            format!("R{} := Gbl[{}]", inst.a, k_str(proto, inst.bx)),
        ),
        SemanticOp::GetTable => (
            format!(
                "{}  {}  {}",
                reg_name(proto, inst.a, pc),
                operand_str(proto, table, inst, OperandSlot::B, pc),
                operand_str(proto, table, inst, OperandSlot::C, pc)
            ),
            format!(
                "R{} := R{}[{}]",
                inst.a,
                inst.b,
                operand_str(proto, table, inst, OperandSlot::C, pc)
            ),
        ),
        SemanticOp::SetGlobal => (
            format!("{}  {}", reg_name(proto, inst.a, pc), k_str(proto, inst.bx)),
            format!("Gbl[{}] := R{}", k_str(proto, inst.bx), inst.a),
        ),
        SemanticOp::SetUpval => (
            format!("{}  {}", reg_name(proto, inst.a, pc), inst.b),
            format!("{} := R{}", upvalue_ref(proto, inst.b), inst.a),
        ),
        SemanticOp::SetTable => (
            format!(
                "{}  {}  {}",
                reg_name(proto, inst.a, pc),
                operand_str(proto, table, inst, OperandSlot::B, pc),
                operand_str(proto, table, inst, OperandSlot::C, pc)
            ),
            format!(
                "R{}[{}] := {}",
                inst.a,
                operand_str(proto, table, inst, OperandSlot::B, pc),
                operand_str(proto, table, inst, OperandSlot::C, pc)
            ),
        ),
        SemanticOp::NewTable => (
            format!("{}  {}  {}", reg_name(proto, inst.a, pc), inst.b, inst.c),
            format!("array_size={} hash_size={}", inst.b, inst.c),
        ),
        SemanticOp::SelfOp => (
            format!(
                "{}  {}  {}",
                reg_name(proto, inst.a, pc),
                reg_name(proto, inst.b, pc),
                operand_str(proto, table, inst, OperandSlot::C, pc)
            ),
            format!(
                "R{} := R{}; R{} := R{}[{}]",
                inst.a + 1,
                inst.b,
                inst.a,
                inst.b,
                operand_str(proto, table, inst, OperandSlot::C, pc)
            ),
        ),
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
        | SemanticOp::Shr => {
            let symbol = binary_symbol(inst.op);
            (
                format!(
                    "{}  {}  {}",
                    reg_name(proto, inst.a, pc),
                    operand_str(proto, table, inst, OperandSlot::B, pc),
                    operand_str(proto, table, inst, OperandSlot::C, pc)
                ),
                format!(
                    "R{} := {} {} {}",
                    inst.a,
                    operand_str(proto, table, inst, OperandSlot::B, pc),
                    symbol,
                    operand_str(proto, table, inst, OperandSlot::C, pc)
                ),
            )
        }
        SemanticOp::Unm | SemanticOp::Not | SemanticOp::Len | SemanticOp::Bnot => {
            let symbol = unary_symbol(inst.op);
            (
                format!(
                    "{}  {}",
                    reg_name(proto, inst.a, pc),
                    reg_name(proto, inst.b, pc)
                ),
                format!("R{} := {symbol}R{}", inst.a, inst.b),
            )
        }
        SemanticOp::Concat => (
            format!("{}  R{}  R{}", reg_name(proto, inst.a, pc), inst.b, inst.c),
            format!("R{} := R{}..R{}", inst.a, inst.b, inst.c),
        ),
        SemanticOp::Jmp => {
            let target = jump_target(pc, inst.sbx);
            (
                inst.sbx.to_string(),
                format!("pc += {} => [{target}]", inst.sbx),
            )
        }
        SemanticOp::Eq | SemanticOp::Lt | SemanticOp::Le => {
            let symbol = comparison_symbol(inst.op);
            (
                format!(
                    "{}  {}  {}",
                    inst.a,
                    operand_str(proto, table, inst, OperandSlot::B, pc),
                    operand_str(proto, table, inst, OperandSlot::C, pc)
                ),
                format!(
                    "if ({} {} {}) ~= {} then skip",
                    operand_str(proto, table, inst, OperandSlot::B, pc),
                    symbol,
                    operand_str(proto, table, inst, OperandSlot::C, pc),
                    inst.a
                ),
            )
        }
        SemanticOp::Test => (
            format!("{}  {}", reg_name(proto, inst.a, pc), inst.c),
            format!("if bool(R{}) ~= {} then skip", inst.a, inst.c),
        ),
        SemanticOp::TestSet => (
            format!(
                "{}  {}  {}",
                reg_name(proto, inst.a, pc),
                reg_name(proto, inst.b, pc),
                inst.c
            ),
            format!(
                "if bool(R{}) == {} then R{} := R{} else skip",
                inst.b, inst.c, inst.a, inst.b
            ),
        ),
        SemanticOp::Call => {
            let args = if inst.b == 0 {
                format!("args=top-{}", inst.a)
            } else {
                format!("args={}", inst.b - 1)
            };
            let returns = if inst.c == 0 {
                "ret=variable".to_string()
            } else {
                format!("ret={}", inst.c - 1)
            };
            (
                format!("{}  {}  {}", reg_name(proto, inst.a, pc), inst.b, inst.c),
                format!("{args} {returns}"),
            )
        }
        SemanticOp::TailCall => (
            format!("{}  {}  {}", reg_name(proto, inst.a, pc), inst.b, inst.c),
            format!("tail call R{}", inst.a),
        ),
        SemanticOp::Return => {
            let annotation = if inst.b == 0 {
                format!("return R{}..top", inst.a)
            } else if inst.b == 1 {
                "return (nothing)".to_string()
            } else {
                format!("return R{}..R{}", inst.a, inst.a + inst.b - 2)
            };
            (
                format!("{}  {}", reg_name(proto, inst.a, pc), inst.b),
                annotation,
            )
        }
        SemanticOp::ForLoop => {
            let target = jump_target(pc, inst.sbx);
            (
                format!("{}  {}", reg_name(proto, inst.a, pc), inst.sbx),
                format!(
                    "R{} += R{}; if R{} <= R{} then pc += {} => [{target}]",
                    inst.a,
                    inst.a + 2,
                    inst.a,
                    inst.a + 1,
                    inst.sbx
                ),
            )
        }
        SemanticOp::ForPrep => {
            let target = jump_target(pc, inst.sbx);
            (
                format!("{}  {}", reg_name(proto, inst.a, pc), inst.sbx),
                format!(
                    "R{} -= R{}; jump to FORLOOP at [{target}]",
                    inst.a,
                    inst.a + 2
                ),
            )
        }
        SemanticOp::TForLoop => (
            format!("{}  {}", reg_name(proto, inst.a, pc), inst.c),
            format!(
                "R{}..R{} := R{}(R{},R{})",
                inst.a + 3,
                inst.a + 2 + inst.c,
                inst.a,
                inst.a + 1,
                inst.a + 2
            ),
        ),
        SemanticOp::SetList => (
            format!("{}  {}  {}", reg_name(proto, inst.a, pc), inst.b, inst.c),
            format!("R{}[({}-1)*FPF+1..{}] from stack", inst.a, inst.c, inst.b),
        ),
        SemanticOp::Close => (
            reg_name(proto, inst.a, pc),
            format!("close upvalues from R{} upward", inst.a),
        ),
        SemanticOp::Closure => (
            format!("{}  P{}", reg_name(proto, inst.a, pc), inst.bx),
            format!("closure of sub-proto {}", inst.bx),
        ),
        SemanticOp::VarArg => {
            let annotation = if inst.b == 0 {
                format!("load all vararg into R{}+", inst.a)
            } else {
                format!("load {} vararg into R{}", inst.b - 1, inst.a)
            };
            (
                format!("{}  {}", reg_name(proto, inst.a, pc), inst.b),
                annotation,
            )
        }
        SemanticOp::Unknown => fallback_operands(table, inst),
        _ => fallback_operands(table, inst),
    }
}

fn fallback_operands(table: &OpcodeTable, inst: Instruction) -> (String, String) {
    let operands = match table.format_for_raw(inst.raw) {
        InstructionFormat::Abc | InstructionFormat::IvAbc => {
            format!("{}  {}  {}", inst.a, inst.b, inst.c)
        }
        InstructionFormat::Abx => format!("{}  {}", inst.a, inst.bx),
        InstructionFormat::AsBx => format!("{}  {}", inst.a, inst.sbx),
        InstructionFormat::Ax => table.decode_ax(inst.raw).to_string(),
        InstructionFormat::IsJ => table.decode_sj(inst.raw).to_string(),
    };
    (operands, String::new())
}

fn line_number(proto: &Proto, pc: usize) -> Option<i32> {
    proto.line_info.get(pc).copied().filter(|line| *line > 0)
}

fn reg_name(proto: &Proto, reg: i32, pc: usize) -> String {
    let Ok(reg_index) = usize::try_from(reg) else {
        return format!("R{reg}");
    };
    let Ok(pc_i32) = i32::try_from(pc) else {
        return format!("R{reg}");
    };
    for (index, local) in proto.loc_vars.iter().enumerate() {
        if index == reg_index && local.start_pc <= pc_i32 && pc_i32 < local.end_pc {
            let name = local_name(local);
            if !name.is_empty() {
                return name;
            }
        }
    }
    format!("R{reg}")
}

fn local_name(local: &LocVar) -> String {
    escape_bytes(local.name.as_slice())
}

fn upvalue_name(upvalue: &UpvalDesc, index: usize) -> String {
    if upvalue.name.is_empty() {
        format!("upval[{index}]")
    } else {
        escape_bytes(upvalue.name.as_slice())
    }
}

fn upvalue_ref(proto: &Proto, index: i32) -> String {
    usize::try_from(index)
        .ok()
        .and_then(|index| {
            proto
                .upvalues
                .get(index)
                .map(|upvalue| upvalue_name(upvalue, index))
        })
        .unwrap_or_else(|| format!("upval[{index}]"))
}

fn operand_str(
    proto: &Proto,
    table: &OpcodeTable,
    inst: Instruction,
    slot: OperandSlot,
    pc: usize,
) -> String {
    let field = match slot {
        OperandSlot::B => inst.b,
        OperandSlot::C => inst.c,
    };
    match opinfo::info_for(inst.op).operand_mode(slot) {
        OpArgMode::K => rk_field_str(proto, table, field, pc),
        OpArgMode::R => reg_name(proto, field, pc),
        OpArgMode::U | OpArgMode::N => field.to_string(),
    }
}

fn rk_field_str(proto: &Proto, table: &OpcodeTable, field: i32, pc: usize) -> String {
    if table.is_k(field) {
        k_str(proto, table.rk_index(field))
    } else {
        reg_name(proto, field, pc)
    }
}

fn k_str(proto: &Proto, index: i32) -> String {
    match get_constant(proto, index) {
        Some(constant) => format!("K{index}({})", format_constant(constant)),
        None => format!("K{index}"),
    }
}

fn get_constant(proto: &Proto, index: i32) -> Option<&Constant> {
    usize::try_from(index)
        .ok()
        .and_then(|index| proto.constants.get(index))
}

fn format_constant(constant: &Constant) -> String {
    match constant {
        Constant::Nil => "nil".to_string(),
        Constant::Boolean(value) => value.to_string(),
        Constant::Number(value) => {
            crate::number::lua51_number_literal(*value).unwrap_or_else(|| value.to_string())
        }
        Constant::Integer(value) => value.to_string(),
        Constant::Str(bytes) => format!("\"{}\"", escape_bytes(bytes.as_slice())),
    }
}

fn escape_bytes(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &byte in bytes {
        match byte {
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            32..=126 => out.push(char::from(byte)),
            _ => out.push_str(&format!("\\{byte:03}")),
        }
    }
    out
}

fn binary_symbol(op: SemanticOp) -> &'static str {
    match op {
        SemanticOp::Add => "+",
        SemanticOp::Sub => "-",
        SemanticOp::Mul => "*",
        SemanticOp::Div => "/",
        SemanticOp::Mod => "%",
        SemanticOp::Pow => "^",
        SemanticOp::Idiv => "//",
        SemanticOp::Band => "&",
        SemanticOp::Bor => "|",
        SemanticOp::Bxor => "~",
        SemanticOp::Shl => "<<",
        SemanticOp::Shr => ">>",
        _ => "?",
    }
}

fn unary_symbol(op: SemanticOp) -> &'static str {
    match op {
        SemanticOp::Unm => "-",
        SemanticOp::Not => "not ",
        SemanticOp::Len => "#",
        SemanticOp::Bnot => "~",
        _ => "?",
    }
}

fn comparison_symbol(op: SemanticOp) -> &'static str {
    match op {
        SemanticOp::Eq => "==",
        SemanticOp::Lt => "<",
        SemanticOp::Le => "<=",
        _ => "?",
    }
}

fn jump_target(pc: usize, offset: i32) -> i32 {
    i32::try_from(pc).map_or(offset + 2, |pc| pc + offset + 2)
}

fn version_label(version: LuaVersion) -> &'static str {
    match version {
        LuaVersion::V51 => "5.1",
        LuaVersion::V52 => "5.2",
        LuaVersion::V53 => "5.3",
        LuaVersion::V54 => "5.4",
        LuaVersion::V55 => "5.5",
    }
}
