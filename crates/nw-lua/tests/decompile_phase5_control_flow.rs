mod support;

use nw_lua::decompile;
use std::path::Path;

use support::{compile_file_bytes, compile_source_bytes, run_equivalence};

const NUMERIC_FOR: &[u8] = include_bytes!("fixtures/control_flow/numeric_for.luac");
const WHILE_LOOP: &[u8] = include_bytes!("fixtures/control_flow/while.luac");
const REPEAT_LOOP: &[u8] = include_bytes!("fixtures/control_flow/repeat.luac");
const IF_ELSE_PHI: &[u8] = include_bytes!("fixtures/control_flow/if_else_phi.luac");
const IF_ELSEIF_ELSE: &[u8] = include_bytes!("fixtures/control_flow/if_elseif_else.luac");
const GENERIC_FOR: &[u8] = include_bytes!("fixtures/control_flow/generic_for.luac");
const NESTED_FOR_IF: &[u8] = include_bytes!("fixtures/control_flow/nested_for_if.luac");

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
    let path = Path::new(r"E:\Projects\DEMOJSON\lyshineui\_common\abilitiescommon.lua");
    if !path.exists() {
        eprintln!("skipping missing NW fixture {}", path.display());
        return;
    }
    let bytecode = match compile_file_bytes("abilitiescommon_background_path", path, false) {
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
    let path = Path::new(r"E:\Projects\DEMOJSON\lyshineui\_common\timehelperfunctions.lua");
    if !path.exists() {
        eprintln!("skipping missing NW fixture {}", path.display());
        return;
    }
    let bytecode = match compile_file_bytes("timehelper_convert_seconds", path, false) {
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
