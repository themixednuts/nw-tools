mod support;

use support::{run_equivalence, run_equivalence_with_args};

#[test]
fn runtime_equivalence_phase7_multi_cases() {
    let cases = [
        (
            "swap",
            r#"
local a, b = 1, 2
a, b = b, a
print(a, b)
"#,
        ),
        (
            "multi_local_from_call",
            r#"
local s, e = string.find("hello world", "o")
print(s, e)
"#,
        ),
        (
            "call_results_as_call_args",
            r#"
print(string.find("hello", "l"))
"#,
        ),
        (
            "multiret_in_table",
            r#"
local t = {string.byte("AB", 1, 2)}
print(t[1], t[2])
"#,
        ),
        (
            "fixed_multi_return_unpack",
            r#"
local t = {10, 20, 30}
print(unpack(t))
"#,
        ),
        (
            "setlist_longer_list",
            r#"
local t = {1,2,3,4,5,6,7,8,9,10}
print(#t, t[10])
"#,
        ),
        (
            "multiple_targets_from_call",
            r#"
local a, b, c = string.byte("XYZ", 1, 3)
print(a, b, c)
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }
}

#[test]
fn runtime_equivalence_phase7_vararg_cases() {
    let table_source = r#"
local t = {...}
print(#t)
"#;
    let print_source = r#"
print(...)
"#;

    let _ = run_equivalence_with_args("vararg_table", table_source, &["alpha", "beta", "gamma"]);
    let _ = run_equivalence_with_args("vararg_print", print_source, &["alpha", "beta"]);
}

#[test]
fn swap_decompiles_as_single_multiple_assignment() {
    let decompiled = match run_equivalence(
        "swap_structural",
        r#"
local a, b = 1, 2
a, b = b, a
print(a, b)
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    assert!(
        decompiled.contains("a, b = b, a"),
        "expected grouped swap assignment:\n{decompiled}"
    );
    assert!(
        !decompiled.contains("v2"),
        "swap temp should not leak into source:\n{decompiled}"
    );
}

#[test]
fn call_multireturn_decompiles_as_one_local_assignment() {
    let decompiled = match run_equivalence(
        "find_structural",
        r#"
local s, e = string.find("hello world", "o")
print(s, e)
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    assert!(
        decompiled.contains(r#"local s, e = string.find("hello world", "o")"#),
        "expected one multi-local from call:\n{decompiled}"
    );
}
