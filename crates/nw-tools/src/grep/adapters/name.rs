//! Entry-name matching: the always-registered `Tier::Name` adapter.

use super::super::adapter::{ContentHit, EntryView, FactDraft, QuerySet, SearchAdapter, Tier};
use crate::fuzzy;

/// Matches archive paths; needs no bytes.
pub struct NameAdapter;

impl SearchAdapter for NameAdapter {
    fn name(&self) -> &'static str {
        "name"
    }

    fn tier(&self) -> Tier {
        Tier::Name
    }

    fn matches_entry(&self, _entry_name: &str) -> bool {
        true
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        let score = queries
            .terms
            .iter()
            .filter_map(|query| {
                if queries.exact {
                    entry
                        .name
                        .to_ascii_lowercase()
                        .contains(&query.to_ascii_lowercase())
                        .then_some(0)
                } else {
                    let mut search = fuzzy::Search::new(query);
                    search.score(entry.name)
                }
            })
            .max();
        Ok(score
            .map(|score| {
                vec![ContentHit {
                    location: "name".to_string(),
                    snippet: entry.name.to_string(),
                    score,
                }]
            })
            .unwrap_or_default())
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        Ok(crate::extract::path_name_facts(entry.name))
    }
}
