mod support;

use support::run_equivalence;

#[test]
fn runtime_equivalence_phase8_closure_cases() {
    let cases = [
        (
            "local_function_add",
            r#"
local function add(a, b)
    return a + b
end
print(add(2, 3))
"#,
        ),
        (
            "upvalue_capture",
            r#"
local x = 10
local function get()
    return x
end
x = 20
print(get())
"#,
        ),
        (
            "counter_closure",
            r#"
local function counter()
    local n = 0
    return function()
        n = n + 1
        return n
    end
end
local c = counter()
print(c(), c(), c())
"#,
        ),
        (
            "table_function",
            r#"
local t = {}
function t.f(a)
    return a * 2
end
print(t.f(5))
"#,
        ),
        (
            "method_function",
            r#"
local o = { v = 3 }
function o:get()
    return self.v
end
print(o:get())
"#,
        ),
        (
            "recursive_fib",
            r#"
local function fib(n)
    if n < 2 then
        return n
    end
    return fib(n - 1) + fib(n - 2)
end
print(fib(10))
"#,
        ),
        (
            "vararg_sum",
            r#"
local function sum(...)
    local s = 0
    for _, v in ipairs({ ... }) do
        s = s + v
    end
    return s
end
print(sum(1, 2, 3, 4))
"#,
        ),
        (
            "nested_upvalue_chain",
            r#"
local x = 4
local function outer()
    local y = 6
    return function()
        return x + y
    end
end
local f = outer()
print(f())
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }
}

#[test]
fn recursive_case_decompiles_to_local_function() {
    let decompiled = match run_equivalence(
        "recursive_fib_structural",
        r#"
local function fib(n)
    if n < 2 then
        return n
    end
    return fib(n - 1) + fib(n - 2)
end
print(fib(10))
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    assert!(
        decompiled.contains("local function fib(n)"),
        "expected idiomatic local function form:\n{decompiled}"
    );
}

#[test]
fn method_case_keeps_self_receiver() {
    let decompiled = match run_equivalence(
        "method_structural",
        r#"
local o = { v = 3 }
function o:get()
    return self.v
end
print(o:get())
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    let has_method_form = decompiled.contains("function o:get()");
    let has_assignment_form = decompiled.contains("o.get = function(self)");
    assert!(
        has_method_form || has_assignment_form,
        "expected method sugar or assignment form with self:\n{decompiled}"
    );
    assert!(
        decompiled.contains("self.v"),
        "expected body to use self receiver:\n{decompiled}"
    );
}
