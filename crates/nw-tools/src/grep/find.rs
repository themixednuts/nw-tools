//! Find over facts: live extract or a complete content index.
//!
//! The index is an accelerator. Live extract is the same pipeline as
//! [`super::run`] and always works without a cold build.

use std::path::Path;

use nw_pak::PakMmapReader;

use crate::grep::adapter::{FactDraft, FactKind};
use crate::index::{
    ContentIndex, CrcCase, EntryFormat, IndexedHit, Lookup, LookupTarget, fact_location,
};
use crate::resources::{self, SessionOverlay};
use crate::support::PakSet;
use nw_datasheet::game_system::Crc32;

/// Where find reads facts from.
#[derive(Clone, Copy)]
pub enum Source<'a> {
    /// Persist hits from an already-open index (tests, known-complete cache).
    Indexed(&'a ContentIndex),
    /// Extract every entry under `root` and match in process.
    Live(&'a Path),
    /// Prefer a complete index for `root`; otherwise live-extract.
    Auto {
        index: Option<&'a ContentIndex>,
        root: &'a Path,
    },
}

/// CRC lookup over facts, overlaying session dumps.
///
/// # Errors
///
/// Returns an error if pak discovery, extract, or an index read fails.
pub fn lookup(
    source: Source<'_>,
    overlay: &SessionOverlay,
    lookup: Lookup,
) -> anyhow::Result<Vec<IndexedHit>> {
    Ok(merge(
        persist(source, PersistQuery::Lookup(lookup))?,
        overlay,
        |draft| draft_matches(draft, lookup),
    ))
}

/// Fuzzy or exact search over facts, overlaying session dumps.
///
/// # Errors
///
/// Returns an error if pak discovery, extract, or an index read fails.
pub fn search(
    source: Source<'_>,
    overlay: &SessionOverlay,
    queries: &[String],
    exact: bool,
) -> anyhow::Result<Vec<IndexedHit>> {
    Ok(merge(
        persist(source, PersistQuery::Search { queries, exact })?,
        overlay,
        |draft| {
            text_matches(&draft.value, queries, exact) || text_matches(&draft.field, queries, exact)
        },
    ))
}

enum PersistQuery<'a> {
    Lookup(Lookup),
    Search { queries: &'a [String], exact: bool },
}

fn persist(source: Source<'_>, query: PersistQuery<'_>) -> anyhow::Result<Vec<IndexedHit>> {
    match source {
        Source::Indexed(index) => indexed(index, query),
        Source::Live(root) => live(root, query),
        Source::Auto { index, root } => match index {
            Some(index) if index.is_complete(root) => indexed(index, query),
            _ => live(root, query),
        },
    }
}

fn indexed(index: &ContentIndex, query: PersistQuery<'_>) -> anyhow::Result<Vec<IndexedHit>> {
    let mut hits = match &query {
        PersistQuery::Lookup(lookup) => index.lookup(*lookup)?,
        PersistQuery::Search { queries, exact } => index.search(queries, *exact)?,
    };
    if !index.has_embedded() {
        hits.retain(|hit| hit.pak != resources::PAK_NAME);
        hits.extend(SessionOverlay::embedded().hits(|draft| query_keeps(&query, draft)));
    }
    Ok(hits)
}

fn live(root: &Path, query: PersistQuery<'_>) -> anyhow::Result<Vec<IndexedHit>> {
    let paks = PakSet::collect(root.to_path_buf(), Vec::new())?;
    let mut hits = Vec::new();
    for path in paks.paths() {
        let reader = PakMmapReader::open(path)?;
        let pak = paks.relative(path);
        for entry in reader.entries() {
            let name = entry.name();
            let extracted =
                crate::extract::entry(name, || reader.read_by_index(entry.index()).ok());
            if let Some(reason) = extracted.skip {
                tracing::warn!(pak, entry = name, ?reason, "entry left unsearched");
            }
            for draft in &extracted.facts {
                if !query_keeps(&query, draft) {
                    continue;
                }
                hits.push(hit_from_draft(&pak, name, extracted.format, draft));
            }
        }
    }
    hits.extend(SessionOverlay::embedded().hits(|draft| query_keeps(&query, draft)));
    Ok(hits)
}

fn query_keeps(query: &PersistQuery<'_>, draft: &FactDraft) -> bool {
    match query {
        PersistQuery::Lookup(lookup) => draft_matches(draft, *lookup),
        PersistQuery::Search { queries, exact } => {
            text_matches(&draft.value, queries, *exact)
                || text_matches(&draft.field, queries, *exact)
        }
    }
}

fn hit_from_draft(pak: &str, entry: &str, format: EntryFormat, draft: &FactDraft) -> IndexedHit {
    IndexedHit {
        pak: pak.to_owned(),
        entry: entry.to_owned(),
        format,
        location: fact_location(draft),
        field: draft.field.clone(),
        value: draft.value.clone(),
        kind: draft.kind,
    }
}

fn merge(
    mut persist: Vec<IndexedHit>,
    overlay: &SessionOverlay,
    keep: impl Fn(&FactDraft) -> bool,
) -> Vec<IndexedHit> {
    persist.retain(|hit| !(hit.pak == resources::PAK_NAME && overlay.replaces(&hit.entry)));
    persist.extend(overlay.hits(keep));
    persist.sort_by(|left, right| {
        (
            &left.pak,
            &left.entry,
            &left.location,
            &left.field,
            &left.value,
        )
            .cmp(&(
                &right.pak,
                &right.entry,
                &right.location,
                &right.field,
                &right.value,
            ))
    });
    persist
}

fn draft_matches(draft: &FactDraft, lookup: Lookup) -> bool {
    let text = match lookup.target {
        LookupTarget::Name if draft.kind == FactKind::FileName => draft.value.as_str(),
        LookupTarget::Name => return false,
        LookupTarget::Field => draft.field.as_str(),
        LookupTarget::Value => draft.value.as_str(),
    };
    let crc = match lookup.case {
        CrcCase::Original => Crc32::from_str(text),
        CrcCase::Lower => Crc32::from_str_lower(text),
    };
    crc.value() == lookup.crc.value()
}

fn text_matches(text: &str, queries: &[String], exact: bool) -> bool {
    let hay = text.to_ascii_lowercase();
    if exact {
        return queries
            .iter()
            .any(|query| hay.contains(&query.to_ascii_lowercase()));
    }
    queries.iter().any(|query| {
        let mut search = crate::fuzzy::Search::new(&query.to_ascii_lowercase());
        search.score(&hay).is_some()
    })
}

impl SessionOverlay {
    pub(crate) fn hits(&self, keep: impl Fn(&FactDraft) -> bool) -> Vec<IndexedHit> {
        let mut hits = Vec::new();
        for row in &self.rows {
            for draft in &row.facts {
                if !keep(draft) {
                    continue;
                }
                hits.push(IndexedHit {
                    pak: resources::PAK_NAME.to_owned(),
                    entry: row.entry.clone(),
                    format: row.format,
                    location: fact_location(draft),
                    field: draft.field.clone(),
                    value: draft.value.clone(),
                    kind: draft.kind,
                });
            }
        }
        hits
    }
}
