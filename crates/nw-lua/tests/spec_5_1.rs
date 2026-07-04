mod support;

use std::{collections::BTreeSet, fmt::Write};

use bstr::BString;
use nw_lua::{
    chunk::{Constant, Proto},
    decompile::ast::{Block, Expr, Name, Stmt, TableField},
    to_source,
};
use support::{
    compile_source_bytes, run_bytecode_equivalence, run_equivalence, run_equivalence_with_args,
};

const LUA51_OPCODES: &[&str] = &[
    "MOVE",
    "LOADK",
    "LOADBOOL",
    "LOADNIL",
    "GETUPVAL",
    "GETGLOBAL",
    "GETTABLE",
    "SETGLOBAL",
    "SETUPVAL",
    "SETTABLE",
    "NEWTABLE",
    "SELF",
    "ADD",
    "SUB",
    "MUL",
    "DIV",
    "MOD",
    "POW",
    "UNM",
    "NOT",
    "LEN",
    "CONCAT",
    "JMP",
    "EQ",
    "LT",
    "LE",
    "TEST",
    "TESTSET",
    "CALL",
    "TAILCALL",
    "RETURN",
    "FORLOOP",
    "FORPREP",
    "TFORLOOP",
    "SETLIST",
    "CLOSE",
    "CLOSURE",
    "VARARG",
];

#[test]
fn opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes() {
    let cases = [
        OpcodeCase {
            name: "linear_registers_and_globals",
            source: r#"
spec_global = 5
local a, b
a = 1
b = a
local c = true
local d = false
print(spec_global, b, c, d)
"#,
            args: &[],
            opcodes: &[
                "MOVE",
                "LOADK",
                "LOADBOOL",
                "LOADNIL",
                "GETGLOBAL",
                "SETGLOBAL",
                "CALL",
                "RETURN",
            ],
        },
        OpcodeCase {
            name: "upvalues_and_close",
            source: r#"
local f
do
    local x = 1
    f = function()
        x = x + 1
        return x
    end
end
print(f(), f())
"#,
            args: &[],
            opcodes: &["GETUPVAL", "SETUPVAL", "ADD", "CLOSE", "CLOSURE"],
        },
        OpcodeCase {
            name: "table_fields_and_self",
            source: r#"
local t = { x = 2 }
t.y = t.x + 3
function t:m(v)
    return self.y + v
end
print(t["x"], t.y, t:m(4))
"#,
            args: &[],
            opcodes: &["GETTABLE", "SETTABLE", "NEWTABLE", "SELF"],
        },
        OpcodeCase {
            name: "arithmetic_and_unary",
            source: r#"
local a, b = 8, 3
local flag = false
local r1 = a - b
local r2 = a * b
local r3 = a / b
local r4 = a % b
local r5 = a ^ b
local r6 = -a
local r7 = not flag
local r8 = #"abcd"
print(r1, r2, r3, r4, r5, r6, r7, r8)
"#,
            args: &[],
            opcodes: &["SUB", "MUL", "DIV", "MOD", "POW", "UNM", "NOT", "LEN"],
        },
        OpcodeCase {
            name: "concat_and_compare",
            source: r#"
local a = "a" .. "b" .. "c"
local x = 2
if x == 2 then
    print(a)
end
if x < 3 then
    print("lt")
end
if x <= 2 then
    print("le")
end
if x ~= 4 then
    print("ne")
end
"#,
            args: &[],
            opcodes: &["CONCAT", "JMP", "EQ", "LT", "LE"],
        },
        OpcodeCase {
            name: "boolean_tests",
            source: r#"
local a, b = ...
local x = a and b
if a then
    print(x)
end
"#,
            args: &["left", "right"],
            opcodes: &["TEST", "TESTSET"],
        },
        OpcodeCase {
            name: "numeric_and_generic_loops",
            source: r#"
local s = 0
for i = 1, 3 do
    s = s + i
end
for _, v in pairs({ 1, 2 }) do
    s = s + v
end
while true do
    s = s + 1
    break
end
print(s)
"#,
            args: &[],
            opcodes: &["FORLOOP", "FORPREP", "TFORLOOP", "SETLIST"],
        },
        OpcodeCase {
            name: "vararg_and_tailcall",
            source: r#"
local function id(...)
    return ...
end
local function tail(...)
    return id(...)
end
local t = { id(1, 2, 3) }
print(t[1], t[2], tail("x", "y"))
"#,
            args: &[],
            opcodes: &["TAILCALL", "VARARG"],
        },
    ];

    let mut covered = BTreeSet::new();
    for case in cases {
        let Some(bytecode) = compile_source_bytes(case.name, case.source, false) else {
            return;
        };
        let disassembly = nw_lua::disassemble(&bytecode).expect("disassemble case");
        let ssa = nw_lua::ssa_dump(&bytecode).expect("SSA dump case");
        assert!(
            !ssa.trim().is_empty(),
            "SSA dump was empty for {}",
            case.name
        );

        for opcode in case.opcodes {
            assert!(
                disassembly_mentions_opcode(&disassembly, opcode),
                "case {} did not compile to opcode {opcode}\n{disassembly}",
                case.name
            );
            covered.insert(*opcode);
        }

        let _ = run_equivalence_with_args(case.name, case.source, case.args);
    }

    let expected = LUA51_OPCODES.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        covered,
        expected,
        "direct opcode test coverage mismatch; missing: {:?}",
        expected.difference(&covered).collect::<Vec<_>>()
    );
}

#[test]
fn number_literals_recompile_to_exact_lua_51_number_bits() {
    let values = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        42.0,
        -123_456.0,
        0.1,
        -12345.6789,
        9_007_199_254_740_992.0,
        9_007_199_254_740_994.0,
        1.234_567_890_123_456_7,
        1e308,
        1e-300,
        1e-308,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];

    for value in values {
        let source = to_source(&Block::new(vec![Stmt::Return(vec![Expr::Number(value)])]))
            .expect("emit number return");
        if [1e308f64.to_bits(), 1e-300f64.to_bits()].contains(&value.to_bits()) {
            let literal = single_return_literal(&source);
            assert!(
                literal.len() <= 30 && literal.contains('e'),
                "expected compact scientific literal for {value:?}, got {literal:?}\n{source}"
            );
        }
        let Some(bytecode) = compile_source_bytes("number_roundtrip", &source, false) else {
            return;
        };
        let numbers = number_constants(&bytecode);
        assert!(
            numbers
                .iter()
                .any(|candidate| candidate.to_bits() == value.to_bits()),
            "number literal did not round-trip exactly\nvalue bits: {:#018x}\nsource:\n{}\nconstants: {:?}",
            value.to_bits(),
            source,
            numbers
                .iter()
                .map(|number| format!("{:#018x}", number.to_bits()))
                .collect::<Vec<_>>()
        );
    }
}

fn single_return_literal(source: &str) -> &str {
    source
        .trim()
        .strip_prefix("return ")
        .expect("single return statement")
}

#[test]
fn nan_number_literals_are_rejected_instead_of_mis_emitted() {
    let error = to_source(&Block::new(vec![Stmt::Return(vec![Expr::Number(
        f64::NAN,
    )])]))
    .expect_err("NaN cannot be emitted as an exact Lua 5.1 literal");
    assert!(
        error.to_string().contains("NaN"),
        "unexpected error for NaN literal: {error}"
    );
}

#[test]
fn string_literals_recompile_to_exact_lua_51_bytes() {
    let mut all_bytes = Vec::with_capacity(256);
    all_bytes.extend(0u8..=255);
    let long = (0..768)
        .map(|index| u8::try_from(index % 256).expect("byte"))
        .collect::<Vec<_>>();
    let cases = [
        BString::from(Vec::new()),
        BString::from(b"plain ascii".to_vec()),
        BString::from(b"quote\" backslash\\ newline\n carriage\r crlf\r\n tab\t".to_vec()),
        BString::from(vec![0, b'a', 0, b'b', 0x1f, 0x7f, 0x80, 0xff]),
        BString::from(all_bytes),
        BString::from(long),
    ];

    for bytes in cases {
        let source = to_source(&Block::new(vec![Stmt::Return(vec![Expr::Str(
            bytes.clone(),
        )])]))
        .expect("emit string return");
        let Some(bytecode) = compile_source_bytes("string_roundtrip", &source, false) else {
            return;
        };
        let strings = string_constants(&bytecode);
        assert!(
            strings.iter().any(|candidate| candidate == &bytes),
            "string literal did not round-trip exactly\nsource:\n{}\nexpected len: {}\nconstant lens: {:?}",
            source,
            bytes.len(),
            strings
                .iter()
                .map(|string| string.len())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn recovered_reserved_and_invalid_names_are_emitted_safely() {
    let Some(mut keyword_global) = compile_source_bytes(
        "patched_keyword_global",
        r#"
abc = 41
print(abc)
"#,
        false,
    ) else {
        return;
    };
    replace_all_same_len(&mut keyword_global, b"abc", b"end");
    let Some(decompiled) = run_bytecode_equivalence("patched_keyword_global", &keyword_global, &[])
    else {
        return;
    };
    assert!(
        decompiled.contains(r#"_G["end"]"#),
        "keyword global should use bracket form:\n{decompiled}"
    );
    assert!(
        !decompiled.contains("end ="),
        "keyword global must not be emitted as an identifier:\n{decompiled}"
    );

    let Some(mut spaced_field) = compile_source_bytes(
        "patched_spaced_field",
        r#"
local t = {}
t.abc = 12
print(t.abc)
"#,
        false,
    ) else {
        return;
    };
    replace_all_same_len(&mut spaced_field, b"abc", b"a b");
    let Some(decompiled) = run_bytecode_equivalence("patched_spaced_field", &spaced_field, &[])
    else {
        return;
    };
    assert!(
        decompiled.contains(r#"["a b"]"#),
        "invalid field name should use bracket form:\n{decompiled}"
    );
    assert!(
        !decompiled.contains(".a b"),
        "invalid field name must not be emitted with dot syntax:\n{decompiled}"
    );

    let Some(mut keyword_method) = compile_source_bytes(
        "patched_keyword_method",
        r#"
local t = {}
t.abc = function(self, value)
    print(value, self.marker)
end
t.marker = "ok"
t:abc(7)
"#,
        false,
    ) else {
        return;
    };
    replace_all_same_len(&mut keyword_method, b"abc", b"end");
    let Some(decompiled) = run_bytecode_equivalence("patched_keyword_method", &keyword_method, &[])
    else {
        return;
    };
    assert!(
        decompiled.contains(r#"["end"]"#),
        "keyword method should use bracketed call target:\n{decompiled}"
    );
    assert!(
        !decompiled.contains(":end") && !decompiled.contains(".end"),
        "keyword method must not use colon or dot syntax:\n{decompiled}"
    );
}

#[test]
fn emitter_brackets_invalid_ast_field_names_defensively() {
    let block = Block::new(vec![
        Stmt::Local {
            names: vec![Name::from("t")],
            attribs: Vec::new(),
            values: vec![Expr::Table(vec![
                TableField::Named {
                    name: Name::new("end"),
                    value: Expr::Number(1.0),
                },
                TableField::Named {
                    name: Name::new("with space"),
                    value: Expr::Number(2.0),
                },
            ])],
        },
        Stmt::Return(vec![
            Expr::Field {
                obj: Box::new(Expr::Name(Name::from("t"))),
                name: Name::new("end"),
            },
            Expr::Field {
                obj: Box::new(Expr::Name(Name::from("t"))),
                name: Name::new("with space"),
            },
        ]),
    ]);

    let source = to_source(&block).expect("emit invalid field bracket forms");
    assert!(source.contains(r#"["end"]"#), "{source}");
    assert!(source.contains(r#"["with space"]"#), "{source}");
    assert!(!source.contains(".end"), "{source}");
    assert!(!source.contains(".with space"), "{source}");
}

#[test]
fn edge_protos_runtime_equivalence() {
    let cases = [
        (
            "empty_function",
            r#"
local f = function()
end
local r = f()
print(r == nil)
"#,
        ),
        (
            "only_return",
            r#"
local function f()
    return
end
local r = f()
print(r == nil)
"#,
        ),
        (
            "function_vararg",
            r#"
local function f(...)
    local t = { ... }
    return #t, t[1], t[2]
end
print(f("a", "b"))
"#,
        ),
        (
            "deeply_nested_closures",
            r#"
local function a(x)
    return function(y)
        return function(z)
            return x + y + z
        end
    end
end
print(a(1)(2)(3))
"#,
        ),
        (
            "method_self",
            r#"
local object = { value = 8 }
function object:add(delta)
    self.value = self.value + delta
    return self.value
end
print(object:add(5))
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }

    let many_upvalues = many_upvalues_source(55);
    let _ = run_equivalence("maxish_upvalues", &many_upvalues);
}

#[test]
fn chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields() {
    for little_endian in [true, false] {
        for int_size in [1, 2, 4, 8] {
            for size_t_size in [1, 2, 4, 8] {
                let bytes = minimal_return_number_chunk(
                    Layout {
                        little_endian,
                        int_size,
                        size_t_size,
                        number: NumberEncoding::Float64(0.5),
                    },
                    0,
                );
                assert_number_constant(&bytes, 0.5);
            }
        }
    }

    let float32 = minimal_return_number_chunk(
        Layout {
            little_endian: true,
            int_size: 4,
            size_t_size: 4,
            number: NumberEncoding::Float32(3.5),
        },
        0,
    );
    assert_number_constant(&float32, 3.5);

    let little = minimal_return_number_chunk(
        Layout {
            little_endian: true,
            int_size: 4,
            size_t_size: 8,
            number: NumberEncoding::Float64(1.25),
        },
        0,
    );
    assert_number_constant(&little, 1.25);

    let big = minimal_return_number_chunk(
        Layout {
            little_endian: false,
            int_size: 2,
            size_t_size: 4,
            number: NumberEncoding::Float64(-7.5),
        },
        0,
    );
    assert_number_constant(&big, -7.5);

    let integral = minimal_return_number_chunk(
        Layout {
            little_endian: false,
            int_size: 4,
            size_t_size: 2,
            number: NumberEncoding::Integral32(-123),
        },
        0,
    );
    assert_number_constant(&integral, -123.0);

    let exact_integral64 = minimal_return_number_chunk(
        Layout {
            little_endian: true,
            int_size: 4,
            size_t_size: 4,
            number: NumberEncoding::Integral64(1_i64 << 60),
        },
        0,
    );
    assert_number_constant(&exact_integral64, (1_i64 << 60) as f64);

    let inexact_integral64 = minimal_return_number_chunk(
        Layout {
            little_endian: true,
            int_size: 4,
            size_t_size: 4,
            number: NumberEncoding::Integral64(9_007_199_254_740_993),
        },
        0,
    );
    assert!(
        nw_lua::parse_chunk(&inexact_integral64).is_err(),
        "inexact 64-bit integral lua_Number should be rejected"
    );

    let bad_instruction_size = minimal_return_number_chunk(
        Layout {
            little_endian: true,
            int_size: 4,
            size_t_size: 4,
            number: NumberEncoding::Float32(3.5),
        },
        4,
    );
    assert!(
        nw_lua::parse_chunk(&bad_instruction_size).is_err(),
        "unsupported instruction size should be rejected"
    );
}

struct OpcodeCase {
    name: &'static str,
    source: &'static str,
    args: &'static [&'static str],
    opcodes: &'static [&'static str],
}

fn disassembly_mentions_opcode(disassembly: &str, opcode: &str) -> bool {
    disassembly
        .lines()
        .any(|line| line.split_whitespace().any(|token| token == opcode))
}

fn number_constants(bytecode: &[u8]) -> Vec<f64> {
    let chunk = nw_lua::parse_chunk(bytecode).expect("parse bytecode");
    let mut numbers = Vec::new();
    collect_number_constants(&chunk.root, &mut numbers);
    numbers
}

fn collect_number_constants(proto: &Proto, out: &mut Vec<f64>) {
    for constant in &proto.constants {
        if let Constant::Number(value) = constant {
            out.push(*value);
        }
    }
    for child in &proto.protos {
        collect_number_constants(child, out);
    }
}

fn string_constants(bytecode: &[u8]) -> Vec<BString> {
    let chunk = nw_lua::parse_chunk(bytecode).expect("parse bytecode");
    let mut strings = Vec::new();
    collect_string_constants(&chunk.root, &mut strings);
    strings
}

fn collect_string_constants(proto: &Proto, out: &mut Vec<BString>) {
    for constant in &proto.constants {
        if let Constant::Str(value) = constant {
            out.push(value.clone());
        }
    }
    for child in &proto.protos {
        collect_string_constants(child, out);
    }
}

fn replace_all_same_len(bytes: &mut [u8], from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let mut count = 0;
    for index in 0..=bytes.len().saturating_sub(from.len()) {
        if &bytes[index..index + from.len()] == from {
            bytes[index..index + to.len()].copy_from_slice(to);
            count += 1;
        }
    }
    assert!(count > 0, "test bytecode did not contain {:?}", from);
}

fn many_upvalues_source(count: usize) -> String {
    let mut source = String::new();
    for index in 1..=count {
        writeln!(source, "local v{index} = {index}").expect("write local");
    }
    source.push_str("local function sum()\n    return ");
    for index in 1..=count {
        if index > 1 {
            source.push_str(" + ");
        }
        write!(source, "v{index}").expect("write sum term");
    }
    source.push_str("\nend\nprint(sum())\n");
    source
}

#[derive(Clone, Copy)]
struct Layout {
    little_endian: bool,
    int_size: u8,
    size_t_size: u8,
    number: NumberEncoding,
}

#[derive(Clone, Copy)]
enum NumberEncoding {
    Float32(f32),
    Float64(f64),
    Integral32(i32),
    Integral64(i64),
}

impl NumberEncoding {
    fn size(self) -> u8 {
        match self {
            Self::Float32(_) | Self::Integral32(_) => 4,
            Self::Float64(_) | Self::Integral64(_) => 8,
        }
    }

    fn integral(self) -> bool {
        matches!(self, Self::Integral32(_) | Self::Integral64(_))
    }
}

fn minimal_return_number_chunk(layout: Layout, instruction_size_delta: u8) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"\x1bLua");
    out.push(0x51);
    out.push(0);
    out.push(u8::from(layout.little_endian));
    out.push(layout.int_size);
    out.push(layout.size_t_size);
    out.push(4 + instruction_size_delta);
    out.push(layout.number.size());
    out.push(u8::from(layout.number.integral()));

    write_size_t(&mut out, layout, 0);
    write_int(&mut out, layout, 0);
    write_int(&mut out, layout, 0);
    out.push(0);
    out.push(0);
    out.push(2);
    out.push(2);

    write_int(&mut out, layout, 2);
    write_instruction(&mut out, layout, abc(1, 0, 0, 0));
    write_instruction(&mut out, layout, abc(30, 0, 2, 0));

    write_int(&mut out, layout, 1);
    out.push(3);
    write_number(&mut out, layout);

    write_int(&mut out, layout, 0);
    write_int(&mut out, layout, 0);
    write_int(&mut out, layout, 0);
    write_int(&mut out, layout, 0);

    out
}

fn assert_number_constant(bytes: &[u8], expected: f64) {
    let numbers = number_constants(bytes);
    assert_eq!(
        numbers
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        vec![expected.to_bits()]
    );
}

fn write_int(out: &mut Vec<u8>, layout: Layout, value: i32) {
    let value = i64::from(value);
    let bytes = if layout.little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    write_sized(out, &bytes, layout.int_size, layout.little_endian);
}

fn write_size_t(out: &mut Vec<u8>, layout: Layout, value: u64) {
    let bytes = if layout.little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    write_sized(out, &bytes, layout.size_t_size, layout.little_endian);
}

fn write_instruction(out: &mut Vec<u8>, layout: Layout, raw: u32) {
    if layout.little_endian {
        out.extend_from_slice(&raw.to_le_bytes());
    } else {
        out.extend_from_slice(&raw.to_be_bytes());
    }
}

fn write_number(out: &mut Vec<u8>, layout: Layout) {
    match layout.number {
        NumberEncoding::Float32(value) => {
            if layout.little_endian {
                out.extend_from_slice(&value.to_le_bytes());
            } else {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        NumberEncoding::Float64(value) => {
            if layout.little_endian {
                out.extend_from_slice(&value.to_le_bytes());
            } else {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        NumberEncoding::Integral32(value) => {
            if layout.little_endian {
                out.extend_from_slice(&value.to_le_bytes());
            } else {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        NumberEncoding::Integral64(value) => {
            if layout.little_endian {
                out.extend_from_slice(&value.to_le_bytes());
            } else {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
    }
}

fn write_sized(out: &mut Vec<u8>, bytes: &[u8], size: u8, little_endian: bool) {
    let size = usize::from(size);
    if little_endian {
        out.extend_from_slice(&bytes[..size]);
    } else {
        out.extend_from_slice(&bytes[bytes.len() - size..]);
    }
}

fn abc(op: u32, a: u32, b: u32, c: u32) -> u32 {
    op | (a << 6) | (c << 14) | (b << 23)
}
