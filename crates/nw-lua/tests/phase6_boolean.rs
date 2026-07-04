mod support;

use support::{compile_source_bytes, run_equivalence};

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

#[test]
fn reconstructs_short_circuit_values_for_non_local_consumers() {
    let cases = [
        (
            "short_circuit_settable_consumer",
            r#"
local function show(value)
    print(tostring(value))
end
local a = false
local b = "fallback"
local c = true
local d = "selected"
local t = {}
t.x = a or b
show(t.x)
t.y = c and d
show(t.y)
"#,
        ),
        (
            "short_circuit_method_field_consumer",
            r#"
local function show(value)
    print(tostring(value))
end
local M = { y = true }
function M:f(default)
    self.x = self.x or default
    show(self.x)
    self.y = self.y and default
    show(self.y)
end
M:f("method")
"#,
        ),
        (
            "short_circuit_table_constructor_consumer",
            r#"
local function show(value)
    print(tostring(value))
end
local a = false
local b = "field"
local c = true
local d = "kept"
local t = { x = a or b, y = c and d }
show(t.x)
show(t.y)
"#,
        ),
        (
            "short_circuit_global_consumer",
            r#"
local function show(value)
    print(tostring(value))
end
local a = false
local b = true
G = a or 1
show(G)
H = b and 2
show(H)
"#,
        ),
        (
            "short_circuit_call_and_return_consumers",
            r#"
local function show(value)
    print(tostring(value))
end
local function id(value)
    return value
end
local function pick(a, b)
    return a or b
end
local a = false
local b = "call"
show(id(a or b))
show(pick(false, "return"))
"#,
        ),
        (
            "short_circuit_call_value_with_nil_fallback",
            r#"
local function show(value)
    print(tostring(value))
end
local function id(value)
    return value
end
local function pick(value)
    return value and id(value) or nil
end
show(pick(false))
show(pick("nil-fallback"))
"#,
        ),
        (
            "short_circuit_guard_chain_settable_consumer",
            r#"
local function show(value)
    print(tostring(value))
end
local function pick(p)
    local t = {}
    t.right = p and p.right and p.right or 0
    return t.right
end
show(pick(nil))
show(pick({ right = 5 }))
"#,
        ),
        (
            "short_circuit_function_value_settable_consumer",
            r#"
local function show(value)
    print(tostring(value))
end
local function pick(playOnce)
    local t = {}
    t.onComplete = not playOnce and function()
        return "again"
    end or nil
    return t.onComplete
end
show(pick(true))
local callback = pick(false)
if callback then
    show(callback())
end
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }
}

#[test]
fn minimal_settable_or_reconstructs_expression_form() {
    let bytecode = match compile_source_bytes(
        "minimal_settable_or_form",
        r#"
local function make(name, mode)
    local t = {}
    t.name = name
    t.mode = mode or 99
    return t
end
"#,
        false,
    ) {
        Some(bytecode) => bytecode,
        None => return,
    };
    let decompiled = nw_lua::decompile(&bytecode).expect("minimal repro decompiles");

    assert!(
        decompiled.contains("t.mode = mode or 99"),
        "expected SETTABLE consumer to inline short-circuit value:\n{decompiled}"
    );
    assert!(
        !decompiled.contains("v3"),
        "short-circuit SETTABLE consumer must not read undefined synthetic v3:\n{decompiled}"
    );
}
