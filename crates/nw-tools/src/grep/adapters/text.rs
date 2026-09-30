//! Line-oriented pak text: `.txt`, `.cfg`, `.ini`. Leftover XML is extract.

use std::path::Path;

use super::super::adapter::{
    ContentHit, EntryView, FactDraft, FactKind, QuerySet, SearchAdapter, Tier,
};
use super::{line_facts, score_drafts};

/// Matches config and text files. XML leftovers are first-match exclusive.
pub struct TextAdapter;

impl SearchAdapter for TextAdapter {
    fn name(&self) -> &'static str {
        "text"
    }

    fn tier(&self) -> Tier {
        Tier::Content
    }

    fn matches_entry(&self, entry_name: &str) -> bool {
        Path::new(entry_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "txt" | "cfg" | "ini"))
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        Ok(score_drafts(&self.facts(entry)?, queries))
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        let bytes = entry
            .bytes
            .as_ref()
            .ok_or_else(|| "missing bytes".to_string())?;
        Ok(line_facts(bytes, FactKind::TextLine))
    }
}
