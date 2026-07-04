mod support;

use support::run_equivalence;

#[test]
fn runtime_equivalence_phase6_boolean_cases() {
    let cases = [
        (
            "and_or_values",
            r#"
local a, b = true, false
local x = a and b
local y = a or b
print(x)
print(y)
"#,
        ),
        (
            "ternary_nil_left",
            r#"
local a, b, c = nil, 2, 3
local r = a and b or c
print(r)
"#,
        ),
        (
            "ternary_truthy_left",
            r#"
local a, b, c = 1, 2, 3
local r = a and b or c
print(r)
"#,
        ),
        (
            "comparison_chain",
            r#"
local x = 5
if x > 0 and x < 10 then
    print("mid")
else
    print("out")
end
"#,
        ),
        (
            "while_guard",
            r#"
local n = 0
while n < 3 do
    n = n + 1
end
print(n)
"#,
        ),
        (
            "default_or",
            r#"
local p = nil
local d = p or "default"
print(d)
"#,
        ),
        (
            "comparison_values",
            r#"
local x = 7
local ge = x >= 5
local le = x <= 5
local gt = x > 5
print(ge)
print(le)
print(gt)
"#,
        ),
        (
            "demorgan",
            r#"
local a, b = true, false
if not (a and b) then
    print("not both")
end
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }
}

#[test]
fn reconstructs_and_or_ternary_assignment_form() {
    let decompiled = match run_equivalence(
        "ternary_form",
        r#"
local a, b, c = 1, 2, 3
local r = a and b or c
print(r)
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    assert!(
        decompiled.contains("local r = a and b or c"),
        "expected ternary idiom in decompiled source:\n{decompiled}"
    );
}

#[test]
fn normalizes_literal_left_comparison_form() {
    let decompiled = match run_equivalence(
        "gt_form",
        r#"
local x = 7
local gt = x > 5
print(gt)
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    assert!(
        decompiled.contains("x > 5"),
        "expected right-literal greater-than comparison:\n{decompiled}"
    );
    assert!(
        !decompiled.contains("5 < x"),
        "literal-left comparison should have been normalized:\n{decompiled}"
    );
}
