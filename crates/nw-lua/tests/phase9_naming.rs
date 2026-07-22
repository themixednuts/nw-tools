mod support;

use std::collections::BTreeSet;

use support::{run_equivalence, run_stripped_equivalence};

const SHOPCOMMON: &[u8] = include_bytes!("fixtures/shopcommon.luac");

#[test]
fn stripped_runtime_equivalence_phase9_naming_cases() {
    let cases = [
        (
            "accumulator_loop",
            r#"
local t = {5, 6, 7}
local sum = 0
for i = 1, #t do
    local x = t[i]
    sum = sum + x
end
print(sum)
"#,
        ),
        (
            "nested_scope_register_reuse",
            r#"
do
    local a = 1
    print(a)
end
do
    local a = 2
    print(a)
end
"#,
        ),
        (
            "if_else_phi",
            r#"
local x = 5
local r
if x > 0 then
    r = 1
else
    r = 2
end
print(r)
"#,
        ),
        (
            "generic_for",
            r#"
local t = {10, 20}
local s = 0
for _, v in ipairs(t) do
    s = s + v
end
print(s)
"#,
        ),
        (
            "params_and_upvalue",
            r#"
local base = 100
local function f(a, b)
    return base + a + b
end
print(f(2, 3))
"#,
        ),
        (
            "while_carried_value",
            r#"
local i = 1
local acc = 0
while i <= 4 do
    local x = i * 2
    acc = acc + x
    i = i + 1
end
print(acc)
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_stripped_equivalence(name, source);
    }
}

#[test]
fn stripped_accumulator_loop_uses_one_loop_var_name() {
    let decompiled = match run_stripped_equivalence(
        "accumulator_loop_structural",
        r#"
local t = {5, 6, 7}
local sum = 0
for i = 1, #t do
    local x = t[i]
    sum = sum + x
end
print(sum)
"#,
    ) {
        Some(decompiled) => decompiled,
        None => return,
    };

    let loop_var = numeric_for_var(&decompiled).unwrap_or_else(|| {
        panic!("expected a numeric for loop in stripped accumulator:\n{decompiled}")
    });
    assert_eq!(
        decompiled.matches(&format!("for {loop_var} =")).count(),
        1,
        "expected loop variable to be declared once:\n{decompiled}"
    );
    assert!(
        decompiled.contains(&format!("[{loop_var}]")),
        "expected loop body to use declared loop variable {loop_var:?}:\n{decompiled}"
    );
    assert!(
        !decompiled.contains(&format!("[{loop_var}_")),
        "loop body used an SSA-versioned sibling of {loop_var:?}:\n{decompiled}"
    );
}

#[test]
fn call_result_owns_the_debug_local_initializer() {
    let Some(decompiled) = run_equivalence(
        "call_result_local_initializer",
        r#"
local function RequireScript(path)
    return { path = path }
end
local PopupWrapper = RequireScript("LyShineUI.Popup.PopupRequestWrapper")
print(PopupWrapper.path)
"#,
    ) else {
        return;
    };

    assert!(
        decompiled.contains(
            "local PopupWrapper = RequireScript(\"LyShineUI.Popup.PopupRequestWrapper\")"
        ),
        "call result should own the local declaration:\n{decompiled}"
    );
}

#[test]
fn stripped_anonymous_names_reflect_binding_roles() {
    let Some(decompiled) = run_stripped_equivalence(
        "anonymous_binding_roles",
        r#"
local function accumulate(value, increment)
    local total = value + increment
    print(total)
    return total
end
print(accumulate(2, 3))
"#,
    ) else {
        return;
    };

    assert!(decompiled.contains("function(a0, a1)"), "{decompiled}");
    assert!(decompiled.contains("local l2 = a0 + a1"), "{decompiled}");
    assert!(!decompiled.contains("arg1"), "{decompiled}");
    assert!(!decompiled.contains("local v"), "{decompiled}");
}

#[test]
fn shopcommon_reparses_and_has_no_undefined_synthetic_names() {
    let source = nw_lua::decompile(SHOPCOMMON).expect("shopcommon decompiles");
    full_moon::parse(&source).unwrap_or_else(|errors| {
        panic!("shopcommon emitted source did not parse:\n{source}\n{errors:#?}")
    });
    assert_no_undefined_synthetic_names(&source);
}

fn numeric_for_var(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let line = line.trim_start();
        let rest = line.strip_prefix("for ")?;
        let (name, _) = rest.split_once(' ')?;
        is_identifier(name).then(|| name.to_string())
    })
}

fn assert_no_undefined_synthetic_names(source: &str) {
    let tokens = lex_tokens(source);
    let introduced = introduced_synthetic_names(&tokens);
    let read = tokens
        .iter()
        .filter(|token| is_synthetic_name(token))
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = read.difference(&introduced).cloned().collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "synthetic names used without a declaration/assignment target: {missing:?}"
    );
}

fn introduced_synthetic_names(tokens: &[String]) -> BTreeSet<String> {
    let mut introduced = BTreeSet::new();
    let mut index = 0;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "local"
                if tokens
                    .get(index + 1)
                    .is_some_and(|token| token == "function") =>
            {
                if let Some(name) = tokens.get(index + 2)
                    && is_synthetic_name(name)
                {
                    introduced.insert(name.clone());
                }
                index += 2;
            }
            "local" => {
                index += 1;
                while let Some(token) = tokens.get(index) {
                    if token == "=" || statement_boundary(token) {
                        break;
                    }
                    if is_synthetic_name(token) {
                        introduced.insert(token.clone());
                    }
                    index += 1;
                }
            }
            "function" => {
                collect_params(tokens, index + 1, &mut introduced);
            }
            "for" => {
                index += 1;
                while let Some(token) = tokens.get(index) {
                    if token == "=" || token == "in" || statement_boundary(token) {
                        break;
                    }
                    if is_synthetic_name(token) {
                        introduced.insert(token.clone());
                    }
                    index += 1;
                }
            }
            "=" => {
                let mut cursor = index;
                while cursor > 0 {
                    cursor -= 1;
                    let token = &tokens[cursor];
                    if statement_boundary(token) {
                        break;
                    }
                    if is_synthetic_name(token) {
                        introduced.insert(token.clone());
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }
    introduced
}

fn collect_params(tokens: &[String], mut index: usize, introduced: &mut BTreeSet<String>) {
    while let Some(token) = tokens.get(index) {
        if token == "(" {
            break;
        }
        if statement_boundary(token) {
            return;
        }
        index += 1;
    }
    while let Some(token) = tokens.get(index) {
        if token == ")" {
            break;
        }
        if is_synthetic_name(token) {
            introduced.insert(token.clone());
        }
        index += 1;
    }
}

fn statement_boundary(token: &str) -> bool {
    matches!(
        token,
        ";" | "then"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "repeat"
            | "until"
            | "while"
            | "for"
            | "if"
            | "return"
            | "local"
    )
}

fn is_synthetic_name(token: &str) -> bool {
    ["arg", "up", "a", "l", "u", "v"]
        .iter()
        .any(|prefix| has_numeric_components(token, prefix))
}

fn has_numeric_components(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix
                .split('_')
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    })
}

fn is_identifier(token: &str) -> bool {
    let mut bytes = token.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn lex_tokens(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
        } else if is_ident_start(byte) {
            let start = index;
            index += 1;
            while index < bytes.len() && is_ident_continue(bytes[index]) {
                index += 1;
            }
            tokens.push(source[start..index].to_string());
        } else if byte == b'\'' || byte == b'"' {
            index = skip_quoted(bytes, index);
        } else if bytes[index..].starts_with(b"--[[") {
            index = skip_until(bytes, index + 4, b"]]");
        } else if bytes[index..].starts_with(b"--") {
            index = skip_line(bytes, index + 2);
        } else if bytes[index..].starts_with(b"==")
            || bytes[index..].starts_with(b"~=")
            || bytes[index..].starts_with(b"<=")
            || bytes[index..].starts_with(b">=")
        {
            tokens.push(source[index..index + 2].to_string());
            index += 2;
        } else {
            tokens.push(source[index..index + 1].to_string());
            index += 1;
        }
    }
    tokens
}

fn skip_quoted(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
        } else if bytes[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn skip_until(bytes: &[u8], mut index: usize, needle: &[u8]) -> usize {
    while index + needle.len() <= bytes.len() {
        if bytes[index..].starts_with(needle) {
            return index + needle.len();
        }
        index += 1;
    }
    bytes.len()
}

fn skip_line(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}
