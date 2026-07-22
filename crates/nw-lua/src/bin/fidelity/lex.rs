use std::collections::{BTreeSet, HashSet};

#[derive(Debug, Clone, Default)]
pub struct LexFindings {
    pub original_and_or: usize,
    pub decompiled_and_or: usize,
    pub bogus_not_number: usize,
    pub number_short_circuit: usize,
    pub undefined_synthetic_reads: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Ident,
    Number,
    Symbol,
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    text: String,
}

pub fn scan_pair(original: &str, decompiled: &str) -> LexFindings {
    let original_tokens = tokenize(original);
    let decompiled_tokens = tokenize(decompiled);
    LexFindings {
        original_and_or: count_and_or(&original_tokens),
        decompiled_and_or: count_and_or(&decompiled_tokens),
        bogus_not_number: count_bogus_not_number(&decompiled_tokens),
        number_short_circuit: count_number_short_circuit(&decompiled_tokens),
        undefined_synthetic_reads: synthetic_reads(&decompiled_tokens),
    }
}

fn count_and_or(tokens: &[Token]) -> usize {
    tokens
        .iter()
        .filter(|token| token.text == "and" || token.text == "or")
        .count()
}

fn count_bogus_not_number(tokens: &[Token]) -> usize {
    tokens
        .windows(2)
        .filter(|window| window[0].text == "not" && window[1].kind == TokenKind::Number)
        .count()
}

fn count_number_short_circuit(tokens: &[Token]) -> usize {
    tokens
        .windows(3)
        .filter(|window| {
            (window[1].text == "and" || window[1].text == "or")
                && (window[0].kind == TokenKind::Number || window[2].kind == TokenKind::Number)
        })
        .count()
}

fn synthetic_reads(tokens: &[Token]) -> BTreeSet<String> {
    let mut scopes = vec![HashSet::<String>::new()];
    let mut reads = BTreeSet::new();
    let mut idx = 0;

    while idx < tokens.len() {
        let text = tokens[idx].text.as_str();
        match text {
            "function" => {
                if idx > 0
                    && tokens[idx - 1].text == "local"
                    && let Some(name) = tokens.get(idx + 1).filter(|token| is_ident(token))
                {
                    define(&mut scopes, &name.text);
                }
                scopes.push(HashSet::new());
                define_params(tokens, idx, &mut scopes);
            }
            "then" | "do" | "repeat" => scopes.push(HashSet::new()),
            "else" | "elseif" => {
                if scopes.len() > 1 {
                    scopes.pop();
                }
                scopes.push(HashSet::new());
            }
            "end" | "until" => {
                if scopes.len() > 1 {
                    scopes.pop();
                }
            }
            "local" => define_local_names(tokens, idx, &mut scopes),
            "for" => define_for_names(tokens, idx, &mut scopes),
            _ if is_ident(&tokens[idx])
                && is_synthetic_name(text)
                && !is_field_access(tokens, idx)
                && !is_bound(&scopes, text) =>
            {
                reads.insert(text.to_owned());
            }
            _ => {}
        }
        idx += 1;
    }

    reads
}

fn define_local_names(tokens: &[Token], start: usize, scopes: &mut [HashSet<String>]) {
    let mut idx = start + 1;
    if tokens
        .get(idx)
        .is_some_and(|token| token.text == "function")
    {
        idx += 1;
    }
    while let Some(token) = tokens.get(idx) {
        if token.text == "=" || token.text == "in" || token.text == "do" {
            break;
        }
        if is_ident(token) && token.text != "," {
            define(scopes, &token.text);
        }
        idx += 1;
    }
}

fn define_for_names(tokens: &[Token], start: usize, scopes: &mut [HashSet<String>]) {
    let mut idx = start + 1;
    while let Some(token) = tokens.get(idx) {
        if token.text == "=" || token.text == "in" || token.text == "do" {
            break;
        }
        if is_ident(token) && token.text != "," {
            define(scopes, &token.text);
        }
        idx += 1;
    }
}

fn define_params(tokens: &[Token], function_idx: usize, scopes: &mut [HashSet<String>]) {
    let Some(open_idx) = tokens
        .iter()
        .enumerate()
        .skip(function_idx + 1)
        .find_map(|(idx, token)| (token.text == "(").then_some(idx))
    else {
        return;
    };

    let mut idx = open_idx + 1;
    while let Some(token) = tokens.get(idx) {
        if token.text == ")" {
            break;
        }
        if is_ident(token) && token.text != "," {
            define(scopes, &token.text);
        }
        idx += 1;
    }
}

fn define(scopes: &mut [HashSet<String>], name: &str) {
    if let Some(scope) = scopes.last_mut() {
        scope.insert(name.to_owned());
    }
}

fn is_bound(scopes: &[HashSet<String>], name: &str) -> bool {
    scopes.iter().rev().any(|scope| scope.contains(name))
}

fn is_ident(token: &Token) -> bool {
    token.kind == TokenKind::Ident
}

fn is_field_access(tokens: &[Token], idx: usize) -> bool {
    idx > 0 && (tokens[idx - 1].text == "." || tokens[idx - 1].text == ":")
}

fn is_synthetic_name(name: &str) -> bool {
    ["arg", "up", "a", "l", "u", "v"]
        .iter()
        .any(|prefix| has_numeric_suffix(name, prefix))
}

fn has_numeric_suffix(name: &str, prefix: &str) -> bool {
    let Some(rest) = name.strip_prefix(prefix) else {
        return false;
    };
    !rest.is_empty()
        && rest
            .split('_')
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

fn tokenize(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut idx = 0;

    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch.is_ascii_whitespace() {
            idx += 1;
        } else if ch == '-' && bytes.get(idx + 1) == Some(&b'-') {
            idx = skip_comment(bytes, idx + 2);
        } else if ch == '"' || ch == '\'' {
            idx = skip_string(bytes, idx, ch as u8);
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let start = idx;
            idx += 1;
            while idx < bytes.len() {
                let next = bytes[idx] as char;
                if next.is_ascii_alphanumeric() || next == '_' {
                    idx += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                kind: TokenKind::Ident,
                text: source[start..idx].to_owned(),
            });
        } else if ch.is_ascii_digit() {
            let start = idx;
            idx += 1;
            while idx < bytes.len() {
                let next = bytes[idx] as char;
                if next.is_ascii_alphanumeric() || next == '.' || next == '_' {
                    idx += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                kind: TokenKind::Number,
                text: source[start..idx].to_owned(),
            });
        } else {
            let text = match (bytes.get(idx), bytes.get(idx + 1)) {
                (Some(b'='), Some(b'=')) => {
                    idx += 2;
                    "=="
                }
                (Some(b'~'), Some(b'=')) => {
                    idx += 2;
                    "~="
                }
                (Some(b'<'), Some(b'=')) => {
                    idx += 2;
                    "<="
                }
                (Some(b'>'), Some(b'=')) => {
                    idx += 2;
                    ">="
                }
                (Some(b'.'), Some(b'.')) => {
                    idx += 2;
                    ".."
                }
                _ => {
                    idx += 1;
                    &source[idx - 1..idx]
                }
            };
            tokens.push(Token {
                kind: TokenKind::Symbol,
                text: text.to_owned(),
            });
        }
    }

    tokens
}

fn skip_comment(bytes: &[u8], mut idx: usize) -> usize {
    if bytes.get(idx) == Some(&b'[') && bytes.get(idx + 1) == Some(&b'[') {
        idx += 2;
        while idx + 1 < bytes.len() {
            if bytes[idx] == b']' && bytes[idx + 1] == b']' {
                return idx + 2;
            }
            idx += 1;
        }
        bytes.len()
    } else {
        while idx < bytes.len() && bytes[idx] != b'\n' {
            idx += 1;
        }
        idx
    }
}

fn skip_string(bytes: &[u8], mut idx: usize, quote: u8) -> usize {
    idx += 1;
    while idx < bytes.len() {
        if bytes[idx] == b'\\' {
            idx = (idx + 2).min(bytes.len());
        } else if bytes[idx] == quote {
            return idx + 1;
        } else {
            idx += 1;
        }
    }
    idx
}
