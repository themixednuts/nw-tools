//! Datasheet content matching: cell values (and column names) at full recall.

use std::path::Path;

use super::super::adapter::{
    ContentHit, EntryView, FactDraft, FactKind, QuerySet, SearchAdapter, Tier,
};
use crate::fuzzy;

/// Matches `.datasheet` entry cells.
pub struct DatasheetAdapter;

impl SearchAdapter for DatasheetAdapter {
    fn name(&self) -> &'static str {
        "datasheet"
    }

    fn tier(&self) -> Tier {
        Tier::Content
    }

    fn matches_entry(&self, entry_name: &str) -> bool {
        nw_datasheet::is_datasheet_path(Path::new(entry_name))
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        let Some(bytes) = entry.bytes.as_ref() else {
            return Err("missing bytes".to_string());
        };
        let sheet = nw_datasheet::Datasheet::parse(bytes).map_err(|_| "undecodable".to_string())?;
        let lowered_terms: Vec<String> = queries
            .terms
            .iter()
            .map(|term| term.to_ascii_lowercase())
            .collect();
        let mut fuzzy = (!queries.exact).then(|| fuzzy::MultiSearch::new(&lowered_terms));
        let mut hits = Vec::new();
        for (row_index, row) in sheet.rows().enumerate() {
            for (column, cell) in row.columns().iter().zip(row.cells()) {
                let value = cell
                    .as_str()
                    .map_or_else(|| cell.to_string(), str::to_string);
                let lowered_value = value.to_ascii_lowercase();
                let lowered_column = column.name().to_ascii_lowercase();
                let score = match fuzzy.as_mut() {
                    Some(search) => {
                        search.score_any([lowered_value.as_str(), lowered_column.as_str()])
                    }
                    None => lowered_terms
                        .iter()
                        .any(|query| {
                            lowered_value.contains(query) || lowered_column.contains(query)
                        })
                        .then_some(0),
                };
                if let Some(score) = score {
                    hits.push(ContentHit {
                        location: format!("row {row_index} · column {}", column.name()),
                        snippet: value,
                        score,
                    });
                }
            }
        }
        Ok(hits)
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        let Some(bytes) = entry.bytes.as_ref() else {
            return Err("missing bytes".to_string());
        };
        let sheet = nw_datasheet::Datasheet::parse(bytes).map_err(|_| "undecodable".to_string())?;
        let mut facts = Vec::new();
        for (row_index, row) in sheet.rows().enumerate() {
            for (column_index, (column, cell)) in row.columns().iter().zip(row.cells()).enumerate()
            {
                let value = cell
                    .as_str()
                    .map_or_else(|| cell.to_string(), str::to_string);
                if value.is_empty() {
                    continue;
                }
                facts.push(FactDraft {
                    field: column.name().to_string(),
                    value,
                    kind: FactKind::Cell,
                    loc_a: row_index as u32,
                    loc_b: column_index as u32,
                });
            }
        }
        Ok(facts)
    }
}
