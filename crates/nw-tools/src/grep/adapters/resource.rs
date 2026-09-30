//! Name-like string leaves from the bundled (or session) reflection dumps.
//!
//! Scans `"key": "value"` pairs without building a JSON DOM. Non-name keys
//! are skipped without allocating. The decompressed behavior-context JSON is
//! dropped after the first scan.

use std::sync::OnceLock;

use super::super::adapter::{
    ContentHit, EntryView, FactDraft, FactKind, QuerySet, SearchAdapter, Tier,
};
use super::score_drafts;
use crate::resources::ResourceView;

const NAME_KEYS: &[&[u8]] = &[
    b"name",
    b"m_name",
    b"typeName",
    b"m_typeName",
    b"componentName",
    b"className",
    b"fieldName",
    b"prettyName",
    b"displayName",
    b"description",
    b"attributeName",
    b"deprecatedName",
    b"expected",
    b"methodName",
    b"behaviorName",
    b"eventName",
    b"first",
    b"mapKey",
];

/// Matches bundled (or session) serialize / behavior-context / module JSON.
pub struct ResourceAdapter;

impl SearchAdapter for ResourceAdapter {
    fn name(&self) -> &'static str {
        "resource"
    }

    fn tier(&self) -> Tier {
        Tier::Content
    }

    fn matches_entry(&self, entry_name: &str) -> bool {
        let name = entry_name.replace('\\', "/");
        name.eq_ignore_ascii_case("serialize.json")
            || name.eq_ignore_ascii_case("behavior-context.json")
            || (name.starts_with("modules/") && name.ends_with(".json"))
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        Ok(score_drafts(&self.facts(entry)?, queries))
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        let bytes = entry
            .bytes
            .as_deref()
            .ok_or_else(|| "missing bytes".to_string())?;
        if is_embedded_bytes(bytes)
            && let Some(cached) = cached_embedded(entry.name)
        {
            return Ok(cached.to_vec());
        }
        Ok(scan_named_strings(bytes))
    }
}

struct Embedded {
    files: Vec<(String, Vec<FactDraft>)>,
}

static EMBEDDED: OnceLock<Embedded> = OnceLock::new();

fn is_embedded_bytes(bytes: &[u8]) -> bool {
    std::ptr::eq(bytes, nw_resources::SERIALIZE_JSON)
        || std::ptr::eq(bytes, nw_resources::behavior_context_json())
        || nw_resources::MODULE_DESCRIPTORS
            .iter()
            .any(|module| std::ptr::eq(bytes, module.bytes))
}

fn cached_embedded(path: &str) -> Option<&'static [FactDraft]> {
    EMBEDDED
        .get_or_init(load_embedded)
        .files
        .iter()
        .find(|(name, _)| name == path)
        .map(|(_, drafts)| drafts.as_slice())
}

fn load_embedded() -> Embedded {
    Embedded {
        files: ResourceView::embedded()
            .files()
            .map(|(path, bytes)| (path, scan_named_strings(bytes)))
            .collect(),
    }
}

fn scan_named_strings(bytes: &[u8]) -> Vec<FactDraft> {
    let mut drafts = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        let Some((key, after_key)) = next_json_string_bytes(bytes, index) else {
            index += 1;
            continue;
        };
        let mut cursor = skip_ws(bytes, after_key);
        if bytes.get(cursor) != Some(&b':') {
            index = after_key;
            continue;
        }
        cursor = skip_ws(bytes, cursor + 1);
        if bytes.get(cursor) != Some(&b'"') {
            index = after_key;
            continue;
        }
        if !is_name_key(key) {
            index = skip_json_string(bytes, cursor).unwrap_or(cursor + 1);
            continue;
        }
        let Some((value, after_value)) = next_json_string(bytes, cursor) else {
            index = after_key;
            continue;
        };
        if keep_json_string(&value) {
            drafts.push(FactDraft {
                field: String::from_utf8_lossy(key).into_owned(),
                value,
                kind: FactKind::JsonString,
                loc_a: u32::try_from(index).unwrap_or(u32::MAX),
                loc_b: 0,
            });
        }
        index = after_value;
    }
    drafts
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn is_name_key(key: &[u8]) -> bool {
    NAME_KEYS.contains(&key)
}

fn next_json_string_bytes(bytes: &[u8], start: usize) -> Option<(&[u8], usize)> {
    let end = skip_json_string(bytes, start)?;
    if bytes[start + 1..end - 1].contains(&b'\\') {
        return None;
    }
    Some((&bytes[start + 1..end - 1], end))
}

fn next_json_string(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    let end = skip_json_string(bytes, start)?;
    let inner = &bytes[start + 1..end - 1];
    if inner.contains(&b'\\') {
        let raw = std::str::from_utf8(&bytes[start..end]).ok()?;
        return Some((serde_json::from_str(raw).ok()?, end));
    }
    Some((std::str::from_utf8(inner).ok()?.to_owned(), end))
}

fn skip_json_string(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut cursor = start + 1;
    let mut escaped = false;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' if !escaped => escaped = true,
            b'"' if !escaped => return Some(cursor + 1),
            _ => escaped = false,
        }
        cursor += 1;
    }
    None
}

fn keep_json_string(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.len() < 2 {
        return false;
    }
    if trimmed == "No Description" {
        return false;
    }
    if trimmed.starts_with("0x") || trimmed.contains("+0x") {
        return false;
    }
    if is_uuid(trimmed) {
        return false;
    }
    if trimmed
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.' || byte == b'-')
    {
        return false;
    }
    true
}

fn is_uuid(text: &str) -> bool {
    let raw = text.trim_matches(|c| c == '{' || c == '}');
    if raw.len() != 36 {
        return false;
    }
    raw.as_bytes().iter().enumerate().all(|(index, byte)| {
        if index == 8 || index == 13 || index == 18 || index == 23 {
            *byte == b'-'
        } else {
            byte.is_ascii_hexdigit()
        }
    })
}
