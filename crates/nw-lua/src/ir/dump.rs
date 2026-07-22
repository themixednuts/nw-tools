//! Deterministic SSA text dump.

use std::fmt::Write as _;

use crate::{bytecode::OpcodeTable, chunk::Proto};

use super::{BinOp, RelOp, SsaFunction, SsaNode, SsaOp, SsaRef, UnOp, UpvalueCapture, build_ssa};

/// Dump one SSA function.
#[must_use]
pub fn dump_function(function: &SsaFunction) -> String {
    let mut out = String::new();
    write_function(&mut out, function, 0);
    out
}

/// Build and dump a prototype tree.
#[must_use]
pub fn dump_proto_tree(proto: &Proto, table: &OpcodeTable) -> String {
    let mut out = String::from("-- SSA Dump --\n\n");
    write_proto_recursive(&mut out, proto, table, 0);
    out
}

fn write_proto_recursive(out: &mut String, proto: &Proto, table: &OpcodeTable, indent: usize) {
    let function = build_ssa(proto, table);
    write_function(out, &function, indent);
    for nested in &proto.protos {
        write_proto_recursive(out, nested, table, indent + 1);
    }
}

fn write_function(out: &mut String, function: &SsaFunction, indent: usize) {
    let pad = "  ".repeat(indent);
    let source = if function.source.is_empty() {
        "(?)".to_string()
    } else {
        escape_bytes(function.source.as_slice())
    };
    writeln!(
        out,
        "{pad}== ssa function {source}:{}..{} ==",
        function.line_defined, function.last_line_defined
    )
    .expect("writing to String cannot fail");
    writeln!(
        out,
        "{pad}   params={} is_vararg={} maxstack={} blocks={}",
        function.num_params,
        function.is_vararg,
        function.max_stack,
        function.blocks.len()
    )
    .expect("writing to String cannot fail");

    for block in &function.blocks {
        writeln!(
            out,
            "{pad}BB{} [pc {}..{}] preds:[{}] succs:[{}] idom={}",
            block.index,
            block.start_pc,
            block.end_pc,
            join_usize(&block.preds),
            join_usize(&block.succs),
            block
                .idom
                .map_or_else(|| "-1".to_string(), |idom| idom.to_string())
        )
        .expect("writing to String cannot fail");
        for node in &block.nodes {
            write_node(out, node, &pad);
        }
    }
    writeln!(out).expect("writing to String cannot fail");
}

fn write_node(out: &mut String, node: &SsaNode, pad: &str) {
    write!(out, "{pad}  [{:>4}] {} ", node.pc, kind_name(&node.op))
        .expect("writing to String cannot fail");
    if let Some(dest) = dest_string(node.dest) {
        write!(out, "{dest} := ").expect("writing to String cannot fail");
    }

    match &node.op {
        SsaOp::Phi { operands, blocks } => {
            write!(out, "phi(").expect("writing to String cannot fail");
            for (index, operand) in operands.iter().enumerate() {
                if index > 0 {
                    write!(out, ", ").expect("writing to String cannot fail");
                }
                let block = blocks.get(index).copied().unwrap_or(usize::MAX);
                write!(out, "{} from BB{block}", ref_to_string(*operand))
                    .expect("writing to String cannot fail");
            }
            write!(out, ")").expect("writing to String cannot fail");
        }
        SsaOp::Nop => {}
        SsaOp::Move { src } => write!(out, "{}", ref_to_string(*src)).unwrap(),
        SsaOp::LoadK { idx } => write!(out, "K{idx}").unwrap(),
        SsaOp::LoadLiteral { value } => write!(out, "{value:?}").unwrap(),
        SsaOp::LoadBool { value, skip_next } => write!(out, "{value} skip={skip_next}").unwrap(),
        SsaOp::LoadNil { start, end } => write!(out, "R{start}..R{end}").unwrap(),
        SsaOp::GetUpval { upval } => write!(out, "U{upval}").unwrap(),
        SsaOp::GetGlobal { idx } => write!(out, "G[K{idx}]").unwrap(),
        SsaOp::GetTable { table, key } => {
            write!(out, "{}[{}]", ref_to_string(*table), ref_to_string(*key)).unwrap();
        }
        SsaOp::SetGlobal { src, idx } => write!(out, "G[K{idx}] {}", ref_to_string(*src)).unwrap(),
        SsaOp::SetUpval { src, upval } => write!(out, "U{upval} {}", ref_to_string(*src)).unwrap(),
        SsaOp::SetTable { table, key, value } => write!(
            out,
            "{}[{}] {}",
            ref_to_string(*table),
            ref_to_string(*key),
            ref_to_string(*value)
        )
        .unwrap(),
        SsaOp::NewTable {
            array_hint,
            hash_hint,
        } => write!(
            out,
            "array={} hash={}",
            array_hint.encoded(),
            hash_hint.encoded()
        )
        .unwrap(),
        SsaOp::SelfOp {
            table,
            key,
            self_reg,
        } => write!(
            out,
            "{}[{}] self=R{self_reg}",
            ref_to_string(*table),
            ref_to_string(*key)
        )
        .unwrap(),
        SsaOp::BinOp { op, left, right } => write!(
            out,
            "[{}] {} {}",
            bin_symbol(*op),
            ref_to_string(*left),
            ref_to_string(*right)
        )
        .unwrap(),
        SsaOp::UnOp { op, value } => {
            write!(out, "[{}] {}", un_symbol(*op), ref_to_string(*value)).unwrap();
        }
        SsaOp::Concat { operands } => {
            write_refs(out, operands);
        }
        SsaOp::Jump { target } => write!(out, "target={target}").unwrap(),
        SsaOp::Branch {
            rel,
            a,
            b,
            invert,
            t_true,
            t_false,
        } => write!(
            out,
            "[{}] inv={} {} {} true={} false={}",
            rel_symbol(*rel),
            invert,
            ref_to_string(*a),
            ref_to_string(*b),
            t_true,
            t_false
        )
        .unwrap(),
        SsaOp::Call {
            func,
            args,
            base,
            arg_count,
            return_count,
        } => {
            write!(
                out,
                "base=R{base} argc={arg_count} retc={return_count} func={} args:",
                ref_to_string(*func)
            )
            .unwrap();
            write_refs(out, args);
        }
        SsaOp::TailCall {
            func,
            args,
            base,
            arg_count,
            return_count,
        } => {
            write!(
                out,
                "base=R{base} argc={arg_count} retc={return_count} func={} args:",
                ref_to_string(*func)
            )
            .unwrap();
            write_refs(out, args);
        }
        SsaOp::Return {
            values,
            base,
            count,
        } => {
            write!(out, "base=R{base} count={count} values:").unwrap();
            write_refs(out, values);
        }
        SsaOp::ForPrep { control, target } | SsaOp::ForLoop { control, target } => {
            write!(out, "base=R{} target={target}", control.base()).unwrap();
        }
        SsaOp::TForLoop { control, count } => {
            write!(out, "base=R{} count={count}", control.base()).unwrap();
        }
        SsaOp::SetList {
            base,
            count,
            batch,
            values,
            ..
        } => {
            write!(out, "base=R{base} count={count} batch={batch} values:").unwrap();
            write_refs(out, values);
        }
        SsaOp::Close { base } => write!(out, "base=R{base}").unwrap(),
        SsaOp::Closure { proto, upvalues } => {
            write!(out, "P{proto} upvalues:").unwrap();
            write_upvalues(out, upvalues);
        }
        SsaOp::VarArg { base, count } => write!(out, "base=R{base} count={count}").unwrap(),
    }
    writeln!(out).expect("writing to String cannot fail");
}

fn write_refs(out: &mut String, refs: &[SsaRef]) {
    write!(out, "[").expect("writing to String cannot fail");
    for (index, reference) in refs.iter().enumerate() {
        if index > 0 {
            write!(out, ", ").expect("writing to String cannot fail");
        }
        write!(out, "{}", ref_to_string(*reference)).expect("writing to String cannot fail");
    }
    write!(out, "]").expect("writing to String cannot fail");
}

fn write_upvalues(out: &mut String, upvalues: &[UpvalueCapture]) {
    write!(out, "[").expect("writing to String cannot fail");
    for (index, capture) in upvalues.iter().enumerate() {
        if index > 0 {
            write!(out, ", ").expect("writing to String cannot fail");
        }
        match capture {
            UpvalueCapture::ParentLocal(reference) => {
                write!(out, "local {}", ref_to_string(*reference))
                    .expect("writing to String cannot fail");
            }
            UpvalueCapture::ParentUpvalue(upvalue) => {
                write!(out, "upvalue U{upvalue}").expect("writing to String cannot fail");
            }
        }
    }
    write!(out, "]").expect("writing to String cannot fail");
}

fn dest_string(reference: SsaRef) -> Option<String> {
    match reference {
        SsaRef::Reg { .. } => Some(ref_to_string(reference)),
        SsaRef::None | SsaRef::Const(_) => None,
    }
}

fn ref_to_string(reference: SsaRef) -> String {
    match reference {
        SsaRef::None => "_".to_string(),
        SsaRef::Reg { reg, ver } => format!("R{reg}_{ver}"),
        SsaRef::Const(idx) => format!("K{idx}"),
    }
}

fn kind_name(op: &SsaOp) -> &'static str {
    match op {
        SsaOp::Nop => "NOP",
        SsaOp::Phi { .. } => "PHI",
        SsaOp::Move { .. } => "MOVE",
        SsaOp::LoadK { .. } => "LOADK",
        SsaOp::LoadLiteral { .. } => "LOADLITERAL",
        SsaOp::LoadBool { .. } => "LOADBOOL",
        SsaOp::LoadNil { .. } => "LOADNIL",
        SsaOp::GetUpval { .. } => "GETUPVAL",
        SsaOp::GetGlobal { .. } => "GETGLOBAL",
        SsaOp::GetTable { .. } => "GETTABLE",
        SsaOp::SetGlobal { .. } => "SETGLOBAL",
        SsaOp::SetUpval { .. } => "SETUPVAL",
        SsaOp::SetTable { .. } => "SETTABLE",
        SsaOp::NewTable { .. } => "NEWTABLE",
        SsaOp::SelfOp { .. } => "SELF",
        SsaOp::BinOp { .. } => "BINOP",
        SsaOp::UnOp { .. } => "UNOP",
        SsaOp::Concat { .. } => "CONCAT",
        SsaOp::Jump { .. } => "JUMP",
        SsaOp::Branch { .. } => "BRANCH",
        SsaOp::Call { .. } => "CALL",
        SsaOp::TailCall { .. } => "TAILCALL",
        SsaOp::Return { .. } => "RETURN",
        SsaOp::ForPrep { .. } => "FORPREP",
        SsaOp::ForLoop { .. } => "FORLOOP",
        SsaOp::TForLoop { .. } => "TFORLOOP",
        SsaOp::SetList { .. } => "SETLIST",
        SsaOp::Close { .. } => "CLOSE",
        SsaOp::Closure { .. } => "CLOSURE",
        SsaOp::VarArg { .. } => "VARARG",
    }
}

fn bin_symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Pow => "^",
        BinOp::IDiv => "//",
        BinOp::BAnd => "&",
        BinOp::BOr => "|",
        BinOp::BXor => "~",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
    }
}

fn un_symbol(op: UnOp) -> &'static str {
    match op {
        UnOp::Neg => "-",
        UnOp::Not => "not",
        UnOp::Len => "#",
        UnOp::BNot => "~",
    }
}

fn rel_symbol(op: RelOp) -> &'static str {
    match op {
        RelOp::Eq => "==",
        RelOp::Lt => "<",
        RelOp::Le => "<=",
        RelOp::Test => "test",
        RelOp::TestSet => "testset",
    }
}

fn join_usize(values: &[usize]) -> String {
    values
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
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
