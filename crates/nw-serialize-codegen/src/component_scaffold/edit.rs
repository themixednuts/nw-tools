use std::collections::{BTreeMap, BTreeSet};

pub(super) fn find_matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in text[open..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn ensure_module_use(text: &mut String, use_line: &str) {
    if use_item_already_imported(text, use_line) {
        return;
    }
    if text.contains(use_line) {
        return;
    }

    let mut insert_at = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("//!") || trimmed.is_empty() {
            insert_at += line.len();
            continue;
        }
        break;
    }
    text.insert_str(insert_at, &format!("{use_line}\n"));
}

fn use_item_already_imported(text: &str, use_line: &str) -> bool {
    let Some(stripped) = use_line
        .trim()
        .strip_prefix("use ")
        .and_then(|value| value.strip_suffix(';'))
    else {
        return false;
    };

    let Some((module, item)) = stripped.rsplit_once("::") else {
        return false;
    };
    if item == "*" {
        return text
            .lines()
            .map(str::trim)
            .any(|line| line == use_line.trim());
    }

    for line in text.lines().map(str::trim) {
        let Some(import) = line
            .strip_prefix("use ")
            .and_then(|line| line.strip_suffix(';'))
        else {
            continue;
        };
        if import == stripped {
            return true;
        }
        if let Some(rest) = import
            .strip_prefix(module)
            .and_then(|rest| rest.strip_prefix("::{"))
            && let Some(items) = rest.strip_suffix('}')
            && items
                .split(',')
                .map(str::trim)
                .any(|candidate| candidate == item)
        {
            return true;
        }
    }

    false
}

pub(super) fn find_component_struct_start(text: &str, component_name: &str) -> Option<usize> {
    let needle = format!("pub struct {component_name}");
    let mut search_from = 0usize;
    while let Some(relative) = text[search_from..].find(&needle) {
        let start = search_from + relative;
        let after = text[start + needle.len()..].chars().next();
        if after.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_') {
            return Some(start);
        }
        search_from = start + needle.len();
    }
    None
}

pub(super) fn find_attr_block_start(text: &str, struct_start: usize) -> usize {
    let struct_line_start = text[..struct_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let mut block_start = struct_line_start;
    let mut pos = struct_line_start;

    while pos > 0 {
        let line_end = if text.as_bytes().get(pos.wrapping_sub(1)) == Some(&b'\n') {
            pos - 1
        } else {
            pos
        };
        let line_start = text[..line_end]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let line = text[line_start..line_end].trim();
        if line.is_empty() {
            break;
        }

        if line.starts_with("///") || line.starts_with('#') || is_attribute_continuation_line(line)
        {
            block_start = line_start;
            pos = line_start;
            if line_start == 0 {
                break;
            }
            continue;
        }
        break;
    }

    block_start
}

fn is_attribute_continuation_line(line: &str) -> bool {
    line == "]"
        || line == ")]"
        || line.ends_with(',')
        || line.contains(" = ")
        || line.starts_with(')')
}

pub(super) fn add_derive_token_to_attr_block(
    text: &mut String,
    attr_block_start: usize,
    struct_start: usize,
    token: &str,
) -> bool {
    let Some(derive_attr) = find_derive_invocations(text, attr_block_start, struct_start)
        .into_iter()
        .next_back()
    else {
        return false;
    };
    let mut tokens = derive_tokens(&text[derive_attr.start..derive_attr.end]);
    if tokens.iter().any(|part| derive_token_matches(part, token)) {
        return true;
    }
    tokens.push(token.to_owned());
    replace_derive_invocation(text, derive_attr, &tokens);
    true
}

pub(super) fn remove_derive_token_from_attr_block(
    text: &mut String,
    attr_block_start: usize,
    struct_start: usize,
    token: &str,
) {
    let mut invocations = find_derive_invocations(text, attr_block_start, struct_start);
    invocations.sort_by_key(|derive_attr| std::cmp::Reverse(derive_attr.start));
    for derive_attr in invocations {
        let tokens = derive_tokens(&text[derive_attr.start..derive_attr.end]);
        if !tokens.iter().any(|part| derive_token_matches(part, token)) {
            continue;
        }
        let tokens = tokens
            .into_iter()
            .filter(|part| !derive_token_matches(part, token))
            .collect::<Vec<_>>();
        replace_derive_invocation(text, derive_attr, &tokens);
        return;
    }
}

#[derive(Debug, Clone, Copy)]
struct DeriveInvocation {
    start: usize,
    end: usize,
}

fn find_derive_invocations(text: &str, start: usize, end: usize) -> Vec<DeriveInvocation> {
    let mut invocations = Vec::new();
    let mut search_from = start;
    while search_from < end {
        let Some(relative_open) = text[search_from..end].find("#[derive(") else {
            break;
        };
        let attr_start = search_from + relative_open;
        let inner_start = attr_start + "#[derive(".len();
        let Some(relative_close) = text[inner_start..end].find(")]") else {
            break;
        };
        let inner_end = inner_start + relative_close;
        let attr_end = inner_end + ")]".len();
        invocations.push(DeriveInvocation {
            start: attr_start,
            end: attr_end,
        });
        search_from = attr_end;
    }
    invocations
}

fn derive_tokens(attr: &str) -> Vec<String> {
    let Some(open) = attr.find("#[derive(") else {
        return Vec::new();
    };
    let inner_start = open + "#[derive(".len();
    let Some(close) = attr[inner_start..]
        .find(")]")
        .map(|offset| inner_start + offset)
    else {
        return Vec::new();
    };
    attr[inner_start..close]
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn derive_token_matches(candidate: &str, token: &str) -> bool {
    candidate == token || candidate.rsplit("::").next() == Some(token)
}

fn replace_derive_invocation(text: &mut String, derive_attr: DeriveInvocation, tokens: &[String]) {
    if tokens.is_empty() {
        text.replace_range(derive_attr.start..derive_attr.end, "");
        return;
    }

    let original = &text[derive_attr.start..derive_attr.end];
    let replacement = if original.contains('\n') {
        let line_start = text[..derive_attr.start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let indent = &text[line_start..derive_attr.start];
        let mut out = String::new();
        let item_indent = format!("{indent}    ");
        out.push_str("#[derive(\n");
        for token in tokens {
            out.push_str(&item_indent);
            out.push_str(token);
            out.push_str(",\n");
        }
        out.push_str(indent);
        out.push_str(")]");
        out
    } else {
        format!("#[derive({})]", tokens.join(", "))
    };
    text.replace_range(derive_attr.start..derive_attr.end, &replacement);
}

pub(super) fn existing_reexports_by_module(text: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut reexports = BTreeMap::<String, BTreeSet<String>>::new();
    let mut cursor = 0usize;
    while let Some(relative_start) = text[cursor..].find("pub use ") {
        let start = cursor + relative_start;
        let Some(relative_end) = text[start..].find(';') else {
            break;
        };
        let end = start + relative_end + 1;
        let statement = &text[start + "pub use ".len()..end - 1].trim();

        if let Some((module, items)) = parse_reexport_statement(statement) {
            reexports.entry(module).or_default().extend(items);
        }
        cursor = end;
    }
    reexports
}

fn parse_reexport_statement(statement: &str) -> Option<(String, Vec<String>)> {
    if let Some(open) = statement.find("::{") {
        let close = statement.rfind('}')?;
        let module = statement[..open].trim().to_owned();
        let items = statement[open + 3..close]
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        return Some((module, items));
    }

    let (module, item) = statement.rsplit_once("::")?;
    Some((module.trim().to_owned(), vec![item.trim().to_owned()]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_module_use_detects_grouped_imports() {
        let mut text = "use az_derive::{AzRtti, AzTypeInfo};\n\npub struct Thing;\n".to_owned();

        ensure_module_use(&mut text, "use az_derive::AzTypeInfo;");

        assert_eq!(
            text,
            "use az_derive::{AzRtti, AzTypeInfo};\n\npub struct Thing;\n"
        );
    }

    #[test]
    fn existing_reexports_collects_grouped_and_single_items() {
        let text = "\
pub use groups::{GroupsComponent, GroupsPlugin};
pub use player::PlayerComponent;
";

        let reexports = existing_reexports_by_module(text);

        assert_eq!(
            reexports
                .get("groups")
                .expect("grouped reexport should be indexed")
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["GroupsComponent".to_owned(), "GroupsPlugin".to_owned()]
        );
        assert_eq!(
            reexports
                .get("player")
                .expect("single reexport should be indexed")
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["PlayerComponent".to_owned()]
        );
    }

    #[test]
    fn derive_tokens_are_added_and_removed_by_unqualified_name() {
        let mut text = "\
#[derive(Debug)]
#[derive(bevy::prelude::Reflect)]
pub struct Thing;
"
        .to_owned();
        let struct_start = find_component_struct_start(&text, "Thing").expect("struct start");
        let attr_start = find_attr_block_start(&text, struct_start);

        assert!(add_derive_token_to_attr_block(
            &mut text,
            attr_start,
            struct_start,
            "Clone"
        ));
        let struct_start = find_component_struct_start(&text, "Thing").expect("struct start");
        remove_derive_token_from_attr_block(&mut text, attr_start, struct_start, "Reflect");

        assert!(text.contains("#[derive(Debug)]"));
        assert!(text.contains("#[derive(Clone)]"));
        assert!(!text.contains("Reflect"));
    }
}
