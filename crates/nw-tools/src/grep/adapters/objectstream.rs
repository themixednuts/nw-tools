//! ObjectStream string leaves and field names.

use std::borrow::Cow;
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::OnceLock;

use nw_objectstream::lookup::NameLookup;
use nw_objectstream::value::read_string;

use super::super::adapter::{
    ContentHit, EntryView, FactDraft, FactKind, QuerySet, SearchAdapter, Tier,
};
use super::score_drafts;

const EXTENSIONS: &[&str] = &[
    "dynamicslice",
    "slice",
    "prefab",
    "spawnable",
    "uicanvas",
    "networkspawnable",
    "entityprototype",
];

/// Matches slice/prefab/UI ObjectStream entries.
pub struct ObjectStreamAdapter;

impl SearchAdapter for ObjectStreamAdapter {
    fn name(&self) -> &'static str {
        "objectstream"
    }

    fn tier(&self) -> Tier {
        Tier::Content
    }

    fn matches_entry(&self, entry_name: &str) -> bool {
        Path::new(entry_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| EXTENSIONS.iter().any(|want| ext.eq_ignore_ascii_case(want)))
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        Ok(score_drafts(&self.facts(entry)?, queries))
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        let raw = entry
            .bytes
            .as_deref()
            .ok_or_else(|| "missing bytes".to_string())?;
        let bytes = peel(raw).ok_or_else(|| "undecodable".to_string())?;
        let stream = nw_objectstream::ObjectStream::from_bytes(bytes.as_ref(), names())
            .map_err(|_| "undecodable".to_string())?;
        let mut facts = Vec::new();
        for (index, element) in stream.iter_recursive().enumerate() {
            let loc = index as u32;
            if let Ok(text) = read_string(element) {
                if !text.is_empty() {
                    facts.push(FactDraft {
                        field: element
                            .field()
                            .map(|name| name.to_string())
                            .unwrap_or_default(),
                        value: text.to_string(),
                        kind: FactKind::ObjectValue,
                        loc_a: loc,
                        loc_b: 0,
                    });
                }
            }
            if let Some(field) = element.field() {
                if !field.is_empty() {
                    facts.push(FactDraft {
                        field: String::new(),
                        value: field.to_string(),
                        kind: FactKind::ObjectField,
                        loc_a: loc,
                        loc_b: 0,
                    });
                }
            }
        }
        Ok(facts)
    }
}

fn names() -> Option<&'static NameLookup> {
    static LOOKUP: OnceLock<Option<NameLookup>> = OnceLock::new();
    LOOKUP
        .get_or_init(|| NameLookup::from_serialize_json(nw_resources::SERIALIZE_JSON).ok())
        .as_ref()
}

fn peel(bytes: &[u8]) -> Option<Cow<'_, [u8]>> {
    if nw_objectstream::looks_like_objectstream(bytes) {
        return Some(Cow::Borrowed(bytes));
    }
    if !nw_pak::azcs::is_azcs(bytes) {
        return None;
    }
    let mut decoded = Vec::new();
    nw_pak::azcs::decompress(&mut Cursor::new(bytes))
        .ok()?
        .read_to_end(&mut decoded)
        .ok()?;
    nw_objectstream::looks_like_objectstream(&decoded).then_some(Cow::Owned(decoded))
}
