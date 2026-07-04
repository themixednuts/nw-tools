use nw_lua::decompile;

const CONST_ARITH: &[u8] = include_bytes!("fixtures/linear/const_arith.luac");
const LOCAL_ADD: &[u8] = include_bytes!("fixtures/linear/local_add.luac");
const TABLE_FIELD: &[u8] = include_bytes!("fixtures/linear/table_field.luac");
const METHOD_STRING: &[u8] = include_bytes!("fixtures/linear/method_string.luac");
const SQUARE_LOCAL: &[u8] = include_bytes!("fixtures/linear/square_local.luac");

#[test]
fn linear_fixtures_decompile_and_reparse() {
    let cases = [
        ("const_arith", CONST_ARITH),
        ("local_add", LOCAL_ADD),
        ("table_field", TABLE_FIELD),
        ("method_string", METHOD_STRING),
        ("square_local", SQUARE_LOCAL),
    ];

    for (name, bytes) in cases {
        let source = decompile(bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        full_moon::parse(&source).unwrap_or_else(|errors| {
            panic!("{name}: emitted source did not parse:\n{source}\n{errors:#?}")
        });
    }
}

#[test]
fn const_arith_decompiles_to_folded_return() {
    let source = decompile(CONST_ARITH).expect("decompile succeeds");
    assert_eq!(source, "return 7\n");
}

#[test]
fn local_add_decompiles_with_debug_local_names() {
    let source = decompile(LOCAL_ADD).expect("decompile succeeds");
    assert_eq!(source, concat!("local a, b = 10, 20\n", "return a + b\n"));
}

#[test]
fn table_field_decompiles_as_table_assignment_and_read() {
    let source = decompile(TABLE_FIELD).expect("decompile succeeds");
    assert_eq!(
        source,
        concat!("local t = {}\n", "t.x = 5\n", "return t.x\n")
    );
}

#[test]
fn string_method_call_decompiles_and_reparses() {
    let source = decompile(METHOD_STRING).expect("decompile succeeds");
    assert!(source.contains(":upper()"), "{source}");
    assert!(source.contains("return"), "{source}");
}

#[test]
fn square_local_decompiles_with_debug_local_names() {
    let source = decompile(SQUARE_LOCAL).expect("decompile succeeds");
    assert_eq!(
        source,
        concat!("local x = 3\n", "local y = x * x\n", "return y\n")
    );
}
