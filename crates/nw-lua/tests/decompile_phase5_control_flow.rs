mod support;

use nw_lua::decompile;

use support::{compile_file_bytes, compile_source_bytes, demojson_fixture, run_equivalence};

const NUMERIC_FOR: &[u8] = include_bytes!("fixtures/control_flow/numeric_for.luac");
const WHILE_LOOP: &[u8] = include_bytes!("fixtures/control_flow/while.luac");
const REPEAT_LOOP: &[u8] = include_bytes!("fixtures/control_flow/repeat.luac");
const IF_ELSE_PHI: &[u8] = include_bytes!("fixtures/control_flow/if_else_phi.luac");
const IF_ELSEIF_ELSE: &[u8] = include_bytes!("fixtures/control_flow/if_elseif_else.luac");
const GENERIC_FOR: &[u8] = include_bytes!("fixtures/control_flow/generic_for.luac");
const NESTED_FOR_IF: &[u8] = include_bytes!("fixtures/control_flow/nested_for_if.luac");
const ENTRY_NIL_LOCAL: &[u8] = include_bytes!("fixtures/control_flow/entry_nil_local.luac");
const LOCAL_BEFORE_NESTED_IF: &[u8] =
    include_bytes!("fixtures/control_flow/local_before_nested_if.luac");
const BREAK_WRITES_LOCAL: &[u8] = include_bytes!("fixtures/control_flow/break_writes_local.luac");
const LOADNIL_RANGE_LOCALS: &[u8] =
    include_bytes!("fixtures/control_flow/loadnil_range_locals.luac");
const CALL_IN_PHI_ARMS: &[u8] = include_bytes!("fixtures/control_flow/call_in_phi_arms.luac");
const ARM_WRITES_TWO_LOCALS: &[u8] =
    include_bytes!("fixtures/control_flow/arm_writes_two_locals.luac");
const OR_CALL_WITH_TABLE_ARG: &[u8] =
    include_bytes!("fixtures/control_flow/or_call_with_table_arg.luac");
const EMPTY_ARMS_GATE_ELSE: &[u8] =
    include_bytes!("fixtures/control_flow/empty_arms_gate_else.luac");
const SELECT_IN_SELECT_ARM: &[u8] =
    include_bytes!("fixtures/control_flow/select_in_select_arm.luac");

#[test]
fn control_flow_fixtures_decompile_and_reparse() {
    let cases = [
        ("numeric_for", NUMERIC_FOR, &["for", "do", "end"][..]),
        ("while", WHILE_LOOP, &["while", "do", "end"][..]),
        ("repeat", REPEAT_LOOP, &["repeat", "until"][..]),
        ("if_else_phi", IF_ELSE_PHI, &["if", "else", "end"][..]),
        (
            "if_elseif_else",
            IF_ELSEIF_ELSE,
            &["if", "elseif", "else", "end"][..],
        ),
        ("generic_for", GENERIC_FOR, &["for", "in", "do", "end"][..]),
        ("nested_for_if", NESTED_FOR_IF, &["for", "if", "end"][..]),
    ];

    for (name, bytes, keywords) in cases {
        let source = decompile(bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        full_moon::parse(&source).unwrap_or_else(|errors| {
            panic!("{name}: emitted source did not parse:\n{source}\n{errors:#?}")
        });
        for keyword in keywords {
            assert!(
                source.contains(keyword),
                "{name}: expected keyword {keyword:?} in\n{source}"
            );
        }
    }
}

#[test]
fn stripped_local_with_no_load_nil_is_declared_at_function_entry() {
    let source = decompile(ENTRY_NIL_LOCAL).expect("decompile succeeds");
    let declaration = source.find("local l1, l2").expect(&source);
    let loop_start = source.find("for ").expect(&source);
    assert!(declaration < loop_start, "{source}");
    assert_eq!(source.matches("local l1").count(), 1, "{source}");
    assert!(source.contains("return l1, l2"), "{source}");
}

#[test]
fn local_read_by_a_nested_if_stays_in_the_else_block() {
    let source = decompile(LOCAL_BEFORE_NESTED_IF).expect("decompile succeeds");
    let else_start = source.find("else\n").expect(&source);
    let declaration = source.find("local l3 = a1[a0]").expect(&source);
    assert!(else_start < declaration, "{source}");
    assert_eq!(source.matches("elseif").count(), 1, "{source}");
}

#[test]
fn local_written_before_a_break_keeps_both_writes() {
    let source = decompile(BREAK_WRITES_LOCAL).expect("decompile succeeds");
    assert!(source.contains("= false"), "{source}");
    assert!(source.contains("= true"), "{source}");
    assert!(source.contains("break"), "{source}");
}

#[test]
fn loadnil_range_declares_every_register_it_clears() {
    let source = decompile(LOADNIL_RANGE_LOCALS).expect("decompile succeeds");
    let loop_start = source.find("for ").expect(&source);
    assert!(source[..loop_start].contains("local l1, l2"), "{source}");
}

#[test]
fn calls_moved_into_a_phi_stay_in_their_arms() {
    let source = decompile(CALL_IN_PHI_ARMS).expect("decompile succeeds");
    assert!(source.contains("load_first("), "{source}");
    assert!(source.contains("load_second("), "{source}");
    assert!(source.contains("elseif"), "{source}");
    let report = source.find("report(").expect(&source);
    let else_start = source.find("else\n").expect(&source);
    assert!(else_start < report, "{source}");
}

#[test]
fn value_select_leaves_arms_that_write_another_local() {
    let source = decompile(ARM_WRITES_TWO_LOCALS).expect("decompile succeeds");
    assert!(source.contains("l2[l1]"), "{source}");
    assert!(source.contains("l3[l1]"), "{source}");
    assert!(source.contains("= false"), "{source}");
    assert!(source.contains("= true"), "{source}");
}

#[test]
fn or_fallback_call_keeps_its_table_argument() {
    let source = decompile(OR_CALL_WITH_TABLE_ARG).expect("decompile succeeds");
    assert!(source.contains(" and \"\" or format("), "{source}");
    assert!(source.contains("level = "), "{source}");
}

#[test]
fn select_nested_in_a_select_arm_fails_instead_of_dropping_code() {
    let error = decompile(SELECT_IN_SELECT_ARM).expect_err("nested select is not supported");
    assert!(error.to_string().contains("jumps past"), "{error}");
}

#[test]
fn empty_arms_still_gate_the_else_block() {
    let source = decompile(EMPTY_ARMS_GATE_ELSE).expect("decompile succeeds");
    let missing = source.find("\"missing\"").expect(&source);
    assert!(source[..missing].contains("if "), "{source}");
    assert!(source.contains("\"done\""), "{source}");
}

#[test]
fn if_else_phi_declares_once_and_assigns_in_both_arms() {
    let source = decompile(IF_ELSE_PHI).expect("decompile succeeds");
    assert_eq!(source.matches("local r").count(), 1, "{source}");
    assert!(source.contains("r = 1"), "{source}");
    assert!(source.contains("r = 2"), "{source}");
    assert!(source.contains("return r"), "{source}");
}

#[test]
fn runtime_equivalence_preserves_returning_if_elseif_fallthrough_chain() {
    let decompiled = match run_equivalence(
        "returning_if_elseif_fallthrough_chain",
        r#"
local M = {}
function M:sel(a, b, x, y, z)
    if a then return x elseif b then return y end
    return z
end
print(M:sel(true, false, "x", "y", "z"))
print(M:sel(false, true, "x", "y", "z"))
print(M:sel(false, false, "x", "y", "z"))
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    assert!(decompiled.contains("if a then"), "{decompiled}");
    assert!(decompiled.contains("elseif b then"), "{decompiled}");
    assert!(decompiled.contains("return x"), "{decompiled}");
    assert!(decompiled.contains("return y"), "{decompiled}");
    assert!(decompiled.contains("return z"), "{decompiled}");
}

#[test]
fn runtime_equivalence_preserves_loop_if_body_with_empty_continue_arm() {
    let decompiled = match run_equivalence(
        "loop_if_body_empty_continue_arm",
        r#"
local function probe(values)
    local i = 1
    local out = ""
    while values[i] do
        local keepGoing = values[i] == "keep"
        i = i + 1
        if not keepGoing then
            out = out .. "x"
        end
    end
    print(out)
end
probe({ "keep", "stop", "keep" })
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    assert!(
        decompiled.contains("out = out .. \"x\""),
        "loop-local if body must not be dropped:\n{decompiled}"
    );
    assert!(
        !decompiled.contains("break"),
        "empty loop backedge must not become break:\n{decompiled}"
    );
}

#[test]
fn runtime_equivalence_preserves_guarded_early_return_body() {
    let decompiled = match run_equivalence(
        "guarded_early_return_body",
        r#"
local M = {}
function M:guard(n, omit)
    if n <= 0 then return omit and "z" or "zzz" end
    return "pos"
end
print(M:guard(0, true))
print(M:guard(0, false))
print(M:guard(2, false))
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    assert!(
        decompiled.contains("if n <= 0 then") || decompiled.contains("if n > 0 then"),
        "{decompiled}"
    );
    assert!(
        decompiled.contains("return omit and \"z\" or \"zzz\""),
        "{decompiled}"
    );
    assert!(decompiled.contains("return \"pos\""), "{decompiled}");
    assert!(!decompiled.contains("if n <= 0 then\nend"), "{decompiled}");
}

#[test]
fn runtime_equivalence_preserves_nested_returning_branch_inside_else() {
    let decompiled = match run_equivalence(
        "nested_returning_branch_inside_else",
        r#"
local M = {}
function M:nested(flag, a, b)
    if flag then return a
    else
        if a > b then return "hi" end
        return "lo"
    end
end
print(M:nested(true, "yes", "no"))
print(M:nested(false, 3, 1))
print(M:nested(false, 1, 3))
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    assert!(decompiled.contains("if flag then"), "{decompiled}");
    assert!(decompiled.contains("return a"), "{decompiled}");
    assert!(decompiled.contains("return \"hi\""), "{decompiled}");
    assert!(decompiled.contains("return \"lo\""), "{decompiled}");
}

#[test]
fn structural_returning_branch_patterns_decompile_with_all_returns() {
    let bytecode = match compile_source_bytes(
        "structural_returning_branch_patterns",
        r#"
local M = {}
function M:sel(a, b, x, y, z)
    if a then return x elseif b then return y end
    return z
end
function M:guard(n, omit)
    if n <= 0 then return omit and "z" or "zzz" end
    return "pos"
end
function M:nested(flag, a, b)
    if flag then return a
    else
        if a > b then return "hi" end
        return "lo"
    end
end
return M
"#,
        false,
    ) {
        Some(bytecode) => bytecode,
        None => return,
    };
    let decompiled = decompile(&bytecode).expect("minimal branch patterns decompile");

    assert!(decompiled.contains("elseif b then"), "{decompiled}");
    for expected in [
        "return x",
        "return y",
        "return z",
        "return omit and \"z\" or \"zzz\"",
        "return \"pos\"",
        "return a",
        "return \"hi\"",
        "return \"lo\"",
    ] {
        assert!(
            decompiled.contains(expected),
            "expected {expected:?} in\n{decompiled}"
        );
    }
    assert!(!decompiled.contains("then\nend"), "{decompiled}");
}

#[test]
fn nw_get_background_path_preserves_returning_elseif_chain() {
    let Some(path) = demojson_fixture("lyshineui/_common/abilitiescommon.lua") else {
        eprintln!(
            "skipping missing NW fixture abilitiescommon.lua; set {}",
            support::DEMOJSON_ROOT_ENV
        );
        return;
    };
    let bytecode = match compile_file_bytes("abilitiescommon_background_path", &path, false) {
        Some(bytecode) => bytecode,
        None => return,
    };
    let decompiled = decompile(&bytecode).expect("abilitiescommon decompiles");

    assert!(
        decompiled.contains("if useInfoOnlyPaths then"),
        "{decompiled}"
    );
    assert!(
        decompiled.contains("elseif usePassivePaths then"),
        "{decompiled}"
    );
    assert!(
        decompiled.contains("infoOnlyBackgroundPathByCategory")
            && decompiled.contains("passiveBackgroundPathByCategory")
            && decompiled.contains("backgroundPathByCategory"),
        "{decompiled}"
    );
    let function_body = decompiled
        .split("function AbilitiesCommon:GetBackgroundPath")
        .nth(1)
        .unwrap_or(&decompiled);
    assert!(
        function_body.matches("return ").count() >= 3,
        "expected all three branch returns to survive:\n{decompiled}"
    );
}

#[test]
fn nw_time_helper_preserves_guarded_returns_and_nested_body() {
    let Some(path) = demojson_fixture("lyshineui/_common/timehelperfunctions.lua") else {
        eprintln!(
            "skipping missing NW fixture timehelperfunctions.lua; set {}",
            support::DEMOJSON_ROOT_ENV
        );
        return;
    };
    let bytecode = match compile_file_bytes("timehelper_convert_seconds", &path, false) {
        Some(bytecode) => bytecode,
        None => return,
    };
    let decompiled = decompile(&bytecode).expect("timehelperfunctions decompiles");

    assert!(decompiled.contains("if seconds <= 0 then"), "{decompiled}");
    assert!(
        decompiled.contains("return omitZeros and \"00\" or \"00:00:00\""),
        "{decompiled}"
    );
    assert!(decompiled.contains("if not omitZeros then"), "{decompiled}");
    assert!(decompiled.contains("local timeString"), "{decompiled}");
    assert!(
        decompiled.contains("return showDays and days .. \":\" .. timeString or timeString"),
        "{decompiled}"
    );
    assert!(decompiled.contains("return outString"), "{decompiled}");
    assert!(
        !decompiled.contains("if seconds <= 0 then\nend"),
        "{decompiled}"
    );
    assert!(
        !decompiled.contains("if not omitZeros then\nend"),
        "{decompiled}"
    );
}
