//! Decompiled lua source, one fact per non-empty line.

use std::path::Path;

use super::super::adapter::{
    ContentHit, EntryView, FactDraft, FactKind, QuerySet, SearchAdapter, Tier,
};
use super::{line_facts, score_drafts};

/// Matches `.luac` bytecode.
pub struct LuaAdapter;

impl SearchAdapter for LuaAdapter {
    fn name(&self) -> &'static str {
        "lua"
    }

    fn tier(&self) -> Tier {
        Tier::Content
    }

    fn matches_entry(&self, entry_name: &str) -> bool {
        Path::new(entry_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("luac"))
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        Ok(score_drafts(&self.facts(entry)?, queries))
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        let bytes = entry
            .bytes
            .as_ref()
            .ok_or_else(|| "missing bytes".to_string())?;
        let stem = Path::new(entry.name)
            .file_stem()
            .and_then(|stem| stem.to_str());
        let source = nw_lua::decompile_with_options_and_module_stem(
            bytes,
            nw_lua::DecompOptions::default(),
            stem,
        )
        .map_err(|_| "undecodable".to_string())?;
        Ok(line_facts(source.as_bytes(), FactKind::LuaLine))
    }
}
