use nw_lua::{
    bytecode::{OpcodeTable, SemanticOp},
    chunk::Proto,
    disasm::{disassemble_chunk, disassemble_proto},
    parse_chunk,
    version::LuaTarget,
};

const SHOPCOMMON: &[u8] = include_bytes!("fixtures/shopcommon.luac");
const IDLE_HEROES_TABLE: &str = include_str!("fixtures/idle_heroes.txt");
const RK_BIT: u32 = 1 << 8;
const SBX_BIAS: i32 = (1 << 17) - 1;

#[test]
fn decodes_lua_51_instruction_fields() {
    let table = lua51_table();

    let move_inst = table.decode(abc(0, 1, 2, 3));
    assert_eq!(move_inst.op, SemanticOp::Move);
    assert_eq!(move_inst.a, 1);
    assert_eq!(move_inst.b, 2);
    assert_eq!(move_inst.c, 3);
    assert_eq!(move_inst.bx, 1_027);
    assert_eq!(move_inst.sbx, -130_044);

    let loadk = table.decode(abx(1, 4, 42));
    assert_eq!(loadk.op, SemanticOp::LoadK);
    assert_eq!(loadk.a, 4);
    assert_eq!(loadk.bx, 42);
    assert_eq!(loadk.sbx, 42 - SBX_BIAS);

    let getglobal = table.decode(abx(5, 7, 300));
    assert_eq!(getglobal.op, SemanticOp::GetGlobal);
    assert_eq!(getglobal.a, 7);
    assert_eq!(getglobal.bx, 300);

    let call = table.decode(abc(28, 0, 3, 2));
    assert_eq!(call.op, SemanticOp::Call);
    assert_eq!((call.a, call.b, call.c), (0, 3, 2));

    let jmp_forward = table.decode(asbx(22, 0, 12));
    assert_eq!(jmp_forward.op, SemanticOp::Jmp);
    assert_eq!(jmp_forward.sbx, 12);

    let jmp_backward = table.decode(asbx(22, 0, -4));
    assert_eq!(jmp_backward.op, SemanticOp::Jmp);
    assert_eq!(jmp_backward.sbx, -4);

    let eq = table.decode(abc(23, 1, rk(2), 3));
    assert_eq!(eq.op, SemanticOp::Eq);
    assert_eq!((eq.a, eq.b, eq.c), (1, 258, 3));

    let lt = table.decode(abc(24, 0, 5, rk(255)));
    assert_eq!(lt.op, SemanticOp::Lt);
    assert_eq!((lt.a, lt.b, lt.c), (0, 5, 511));

    let ret = table.decode(abc(30, 2, 1, 0));
    assert_eq!(ret.op, SemanticOp::Return);
    assert_eq!((ret.a, ret.b, ret.c), (2, 1, 0));

    let closure = table.decode(abx(36, 5, 7));
    assert_eq!(closure.op, SemanticOp::Closure);
    assert_eq!((closure.a, closure.bx), (5, 7));

    let setlist = table.decode(abc(34, 1, 0, 2));
    assert_eq!(setlist.op, SemanticOp::SetList);
    assert_eq!((setlist.a, setlist.b, setlist.c), (1, 0, 2));
}

#[test]
fn rk_helpers_handle_boundary() {
    let table = lua51_table();

    assert!(!table.is_k(255));
    assert_eq!(table.rk_index(255), 255);
    assert!(table.is_k(256));
    assert_eq!(table.rk_index(256), 0);
    assert!(table.is_k(511));
    assert_eq!(table.rk_index(511), 255);
}

#[test]
fn standard_lua_51_table_decodes_shopcommon_without_unknown_opcodes() {
    let chunk = parse_chunk(SHOPCOMMON).expect("shopcommon chunk parses");
    let table = lua51_table();

    let mut decoded = 0;
    let unknown = count_unknown_opcodes(&chunk.root, &table, &mut decoded);

    assert!(decoded > 0);
    assert_eq!(unknown, 0);
}

#[test]
fn disassembles_shopcommon_and_nested_proto_stably() {
    let chunk = parse_chunk(SHOPCOMMON).expect("shopcommon chunk parses");
    let table = lua51_table();
    let output = disassemble_chunk(&chunk);

    assert!(!output.trim().is_empty());
    assert!(output.contains("-- Lua 5.1 Disassembly --"));
    assert!(output.contains("== function"));
    assert!(output.contains("GETGLOBAL"));

    let nested = first_nested_proto(&chunk.root).expect("fixture has nested proto");
    let nested_output = disassemble_proto(nested, &table);
    let code_lines = instruction_lines(&nested_output);

    assert_eq!(code_lines.len(), 15);
    assert!(code_lines[0].contains("[   1]"));
    assert!(code_lines[0].contains("GETGLOBAL   R1  K0(\"assert\")"));
    assert!(code_lines[6].contains("LOADK       R2  K4(\"ShopScreen.ShopId\")"));
    assert!(code_lines[14].contains("RETURN      R0  1"));
}

#[test]
fn parses_custom_opcode_table_text() {
    let table = OpcodeTable::from_custom_text(IDLE_HEROES_TABLE).expect("custom table parses");

    assert_eq!(table.version, LuaTarget::V51);
    assert_eq!(table.op_bits, 6);
    assert_eq!(table.a_bits, 8);
    assert_eq!(table.b_bits, 9);
    assert_eq!(table.c_bits, 9);
    assert_eq!(table.map[0], SemanticOp::Sub);
    assert_eq!(table.map[28], SemanticOp::Move);
    assert_eq!(table.map[63], SemanticOp::GetGlobal);

    let shuffled_move = table.decode(abc(28, 1, 2, 3));
    assert_eq!(shuffled_move.op, SemanticOp::Move);
}

#[test]
fn semantic_names_round_trip() {
    assert_eq!(SemanticOp::from_name("MOVE"), Some(SemanticOp::Move));
    assert_eq!(SemanticOp::from_name("semMOVE"), Some(SemanticOp::Move));
    assert_eq!(
        SemanticOp::from_name("tforloop54"),
        Some(SemanticOp::TForLoop54)
    );
    assert_eq!(SemanticOp::TForLoop54.name(), "TFORLOOP54");
}

fn lua51_table() -> OpcodeTable {
    OpcodeTable::builtin(LuaTarget::V51)
}

fn abc(op: u32, a: u32, b: u32, c: u32) -> u32 {
    op | (a << 6) | (c << 14) | (b << 23)
}

fn abx(op: u32, a: u32, bx: u32) -> u32 {
    op | (a << 6) | (bx << 14)
}

fn asbx(op: u32, a: u32, sbx: i32) -> u32 {
    abx(
        op,
        a,
        u32::try_from(sbx + SBX_BIAS).expect("sBx fits in Bx"),
    )
}

fn rk(index: u32) -> u32 {
    index | RK_BIT
}

fn count_unknown_opcodes(proto: &Proto, table: &OpcodeTable, decoded: &mut usize) -> usize {
    let mut unknown = 0;
    for &raw in &proto.code {
        *decoded += 1;
        if table.decode(raw).op == SemanticOp::Unknown {
            unknown += 1;
        }
    }
    for nested in &proto.protos {
        unknown += count_unknown_opcodes(nested, table, decoded);
    }
    unknown
}

fn first_nested_proto(proto: &Proto) -> Option<&Proto> {
    proto.protos.first()
}

fn instruction_lines(disassembly: &str) -> Vec<&str> {
    disassembly
        .lines()
        .filter(|line| line.trim_start().starts_with('['))
        .collect()
}
