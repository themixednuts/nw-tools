use std::collections::BTreeSet;

use heck::{ToSnakeCase, ToUpperCamelCase};

pub fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

pub fn unique_ident(candidate: String, used: &mut BTreeSet<String>) -> String {
    if used.insert(candidate.clone()) {
        return candidate;
    }
    for suffix in 2usize.. {
        let next = format!("{candidate}_{suffix}");
        if used.insert(next.clone()) {
            return next;
        }
    }
    unreachable!("unbounded suffix search must find a unique identifier")
}

pub fn to_module_ident(value: &str, fallback: &str) -> String {
    snake_ident(value, fallback)
}

pub fn to_snake_ident(value: &str, fallback: &str) -> String {
    snake_ident(value, fallback)
}

pub fn to_upper_camel_ident(value: &str, fallback: &str) -> String {
    rust_ident_from_cased(
        &value.to_upper_camel_case(),
        fallback,
        IdentCase::UpperCamel,
    )
}

fn snake_ident(value: &str, fallback: &str) -> String {
    rust_ident_from_cased(&value.to_snake_case(), fallback, IdentCase::Snake)
}

fn rust_ident_from_cased(value: &str, fallback: &str, case: IdentCase) -> String {
    let mut out = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character);
        } else if matches!(case, IdentCase::Snake) && !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    if matches!(case, IdentCase::Snake) {
        while out.ends_with('_') {
            out.pop();
        }
    }
    if out.is_empty() {
        let fallback_ident = match case {
            IdentCase::Snake => fallback.to_snake_case(),
            IdentCase::UpperCamel => fallback.to_upper_camel_case(),
        };
        out.push_str(&fallback_ident);
    }
    if out.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        match case {
            IdentCase::Snake => out.insert(0, '_'),
            IdentCase::UpperCamel => {
                let fallback = fallback.to_upper_camel_case();
                if !fallback.is_empty() {
                    out.insert_str(0, &fallback);
                } else {
                    out.insert(0, '_');
                }
            }
        }
    }
    if is_rust_keyword(&out) {
        out.push('_');
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdentCase {
    Snake,
    UpperCamel,
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
    )
}
