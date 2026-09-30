//! The adapter registry: every searchable format in one place.
//!
//! Adding a format means adding a file here — shared code never changes.

mod animation;
mod audio;
mod catalog;
mod datasheet;
mod lua;
mod mannequin;
mod name;
mod objectstream;
mod resource;
mod text;

use super::adapter::{ContentHit, FactDraft, FactKind, QuerySet, SearchAdapter};
use crate::fuzzy;

/// Every registered adapter, name tier first. Content adapters are first-match.
pub fn all() -> Vec<Box<dyn SearchAdapter>> {
    vec![
        Box::new(name::NameAdapter),
        Box::new(datasheet::DatasheetAdapter),
        Box::new(mannequin::MannequinAdapter),
        Box::new(objectstream::ObjectStreamAdapter),
        Box::new(catalog::CatalogAdapter),
        Box::new(animation::AnimationAdapter),
        Box::new(audio::AudioAdapter),
        Box::new(lua::LuaAdapter),
        Box::new(resource::ResourceAdapter),
        Box::new(text::TextAdapter),
    ]
}

fn line_facts(bytes: &[u8], kind: FactKind) -> Vec<FactDraft> {
    String::from_utf8_lossy(bytes)
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| FactDraft {
            field: String::new(),
            value: line.to_string(),
            kind,
            loc_a: (index + 1) as u32,
            loc_b: 0,
        })
        .collect()
}

pub(crate) fn score_drafts(drafts: &[FactDraft], queries: &QuerySet) -> Vec<ContentHit> {
    let lowered_terms: Vec<String> = queries
        .terms
        .iter()
        .map(|term| term.to_ascii_lowercase())
        .collect();
    let mut fuzzy = (!queries.exact).then(|| fuzzy::MultiSearch::new(&lowered_terms));
    let mut hits = Vec::new();
    for draft in drafts {
        let lowered = draft.value.to_ascii_lowercase();
        let score = match fuzzy.as_mut() {
            Some(search) => search.score(&lowered),
            None => lowered_terms
                .iter()
                .any(|query| lowered.contains(query.as_str()))
                .then_some(0),
        };
        if let Some(score) = score {
            hits.push(ContentHit {
                location: crate::index::fact_location(draft),
                snippet: draft.value.clone(),
                score,
            });
        }
    }
    hits
}
