//! Asset catalog path / id / type text.

use std::path::Path;

use super::super::adapter::{
    ContentHit, EntryView, FactDraft, FactKind, QuerySet, SearchAdapter, Tier,
};
use super::score_drafts;

/// Matches `assetcatalog.catalog` / `assetcatalog_optimized.catalog`.
pub struct CatalogAdapter;

impl SearchAdapter for CatalogAdapter {
    fn name(&self) -> &'static str {
        "catalog"
    }

    fn tier(&self) -> Tier {
        Tier::Content
    }

    fn matches_entry(&self, entry_name: &str) -> bool {
        nw_asset::is_asset_catalog_path(Path::new(entry_name))
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        Ok(score_drafts(&self.facts(entry)?, queries))
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        let bytes = entry
            .bytes
            .as_ref()
            .ok_or_else(|| "missing bytes".to_string())?;
        let catalog = nw_asset::Catalog::parse(bytes).map_err(|_| "undecodable".to_string())?;
        let mut facts = Vec::new();
        match catalog {
            nw_asset::Catalog::Rasc(rasc) => {
                for (index, item) in rasc.entries().iter().enumerate() {
                    let loc = index as u32;
                    push(&mut facts, loc, "path", item.path());
                    push(&mut facts, loc, "asset-id", &item.asset_id().to_string());
                    push(&mut facts, loc, "type", &item.asset_type().to_string());
                }
            }
            nw_asset::Catalog::Raoc(raoc) => {
                for (index, item) in raoc.entries().iter().enumerate() {
                    let loc = index as u32;
                    push(&mut facts, loc, "asset-id", &item.asset_id().to_string());
                }
            }
        }
        Ok(facts)
    }
}

fn push(facts: &mut Vec<FactDraft>, loc_a: u32, field: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    facts.push(FactDraft {
        field: field.to_owned(),
        value: value.to_owned(),
        kind: FactKind::Catalog,
        loc_a,
        loc_b: 0,
    });
}
