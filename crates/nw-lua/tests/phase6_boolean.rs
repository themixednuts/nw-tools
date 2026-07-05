mod support;

use std::path::Path;
use support::{compile_source_bytes, run_equivalence};

use support::compile_file_bytes;

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

#[test]
fn runtime_equivalence_r2_short_circuit_repros() {
    let cases = [
        (
            "r2_and_or_ternary",
            r#"
local function pick(a, b, c)
    local x = a and b or c
    print(tostring(x))
end
pick(true, "b", "c")
pick(false, "b", "c")
"#,
        ),
        (
            "r2_self_and_assignment",
            r#"
local M = {}
function M:f(flag, d)
    flag = flag and 0 < d
    print(tostring(flag))
end
M:f(true, 3)
M:f(true, 0)
M:f(false, 3)
M:f(nil, 3)
"#,
        ),
        (
            "r2_return_table_or_default",
            r#"
local M = {}
function M:g(t, k, d)
    return t[k] or d
end
print(M:g({ x = "hit" }, "x", "miss"))
print(M:g({}, "x", "miss"))
"#,
        ),
        (
            "r2_short_circuit_loop_bound",
            r#"
local M = {}
function M:loopbound(flag, t)
    local out = ""
    for i = flag and 1 or 2, #t do
        out = out .. t[i]
    end
    print(out)
end
M:loopbound(true, { "a", "b", "c" })
M:loopbound(false, { "a", "b", "c" })
"#,
        ),
        (
            "r2_loop_filter",
            r#"
local M = {}
function M:filter(t)
    local seen = false
    local out = ""
    for i = 1, #t do
        if seen or t[i] ~= "00" then
            out = out .. t[i]
            seen = true
        end
    end
    print(out)
end
M:filter({ "00", "03", "04" })
M:filter({ "00", "00" })
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }
}

#[test]
fn r2_preserves_value_position_short_circuit_shape() {
    let bytecode = match compile_source_bytes(
        "r2_short_circuit_shapes",
        r#"
local M = {}
function M:f(flag, d) flag = flag and 0 < d return flag end
function M:g(t, k, d) return t[k] or d end
return M
"#,
        false,
    ) {
        Some(bytecode) => bytecode,
        None => return,
    };
    let decompiled = nw_lua::decompile(&bytecode).expect("R2 shape repro decompiles");

    assert!(
        decompiled.contains("flag = flag and d > 0"),
        "self-and assignment must keep the left operand as a value expression:\n{decompiled}"
    );
    assert!(
        decompiled.contains("return t[k] or d"),
        "table fallback return must stay a single short-circuit return expression:\n{decompiled}"
    );
    assert!(
        !decompiled.contains("if flag then"),
        "self-and assignment must not be over-structured into an if:\n{decompiled}"
    );
}

#[test]
fn runtime_equivalence_r3_boolean_chain_residuals() {
    let cases = [
        (
            "r3_and_comparison_or_comparison",
            r#"
local M = { RESPONSE_REASON_CLIENT_THROTTLED = "client" }
function M:f(failureReason)
    return type(failureReason) == "number" and failureReason == 7 or failureReason == self.RESPONSE_REASON_CLIENT_THROTTLED
end
print(M:f(7), M:f(8), M:f("client"), M:f("other"))
"#,
        ),
        (
            "r3_long_type_guarded_boolean_chain",
            r#"
local NaNStringSet = {}
local infinity = 1 / 0
local negativeInfinity = -1 / 0
local function usable(number)
    return type(number) == "number" and number > negativeInfinity and number < infinity and (number ~= 0 or not NaNStringSet[tostring(number)])
end
print(usable(1), usable(nil), usable(0 / 0), usable(0))
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }
}

#[test]
fn r3_preserves_boolean_chain_operands() {
    let bytecode = match compile_source_bytes(
        "r3_boolean_chain_operands",
        r#"
local M = { RESPONSE_REASON_CLIENT_THROTTLED = "client" }
function M:f(failureReason)
    return type(failureReason) == "number" and failureReason == 7 or failureReason == self.RESPONSE_REASON_CLIENT_THROTTLED
end
return M
"#,
        false,
    ) {
        Some(bytecode) => bytecode,
        None => return,
    };
    let decompiled = nw_lua::decompile(&bytecode).expect("R3 boolean residual decompiles");

    assert!(
        decompiled.contains("failureReason == 7"),
        "numeric throttled comparison must not be dropped:\n{decompiled}"
    );
    assert!(
        decompiled.contains("failureReason == self.RESPONSE_REASON_CLIENT_THROTTLED"),
        "client-throttled comparison must not be dropped:\n{decompiled}"
    );
    assert!(
        !decompiled.contains("type(failureReason) == \"number\" or false"),
        "boolean chain must not collapse to the first guard only:\n{decompiled}"
    );
}

#[test]
fn convert_seconds_short_circuit_reconstruction_is_faithful() {
    let source = Path::new(r"E:\Projects\DEMOJSON\lyshineui\_common\timehelperfunctions.lua");
    let bytecode = match compile_file_bytes("r2_timehelper", source, false) {
        Some(bytecode) => bytecode,
        None => return,
    };
    let decompiled = nw_lua::decompile(&bytecode).expect("timehelperfunctions decompiles");

    assert!(
        decompiled.contains("showDays = showDays and days > 0"),
        "showDays guard must preserve the existing showDays operand:\n{decompiled}"
    );
    assert!(
        decompiled.contains("for i = showDays and 1 or 2, #order do"),
        "short-circuit numeric loop lower bound must stay inline:\n{decompiled}"
    );
    assert!(
        decompiled.contains("if hasFoundNonZero or order[i] ~= \"00\" then"),
        "loop filter condition must preserve the original or condition:\n{decompiled}"
    );
    assert!(
        !decompiled.contains("not 1") && !decompiled.contains("v14"),
        "time helper must not contain bogus numeric negation or undefined synthetic loop bound:\n{decompiled}"
    );
}
