//! Global cross-pak find: fuzzy-match entry names and decoded content.
//!
//! The search pipeline (tickets 002–005): a registry of [`adapter::SearchAdapter`]
//! impls matched per entry, merged name-tier-first, skips counted and never
//! aborting.

pub mod adapter;
pub mod adapters;

use std::path::PathBuf;

use nw_pak::{PakMmapReader, shape};

use crate::support::PakSet;

use adapter::{QuerySet, Tier};

pub mod find;

/// What to search for.
#[derive(Debug, Clone)]
pub struct GrepRequest {
    /// Directory of pak archives (or a single pak file).
    pub root: PathBuf,
    /// Query terms, OR-combined.
    pub queries: Vec<String>,
    /// Exact substring match instead of the default fuzzy ranking.
    pub exact: bool,
}

/// One matched occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepHit {
    /// Pak archive holding the entry, relative to the root.
    pub pak: String,
    /// Archive path of the entry.
    pub entry: String,
    /// Format family of the entry.
    pub format: String,
    /// Where inside the entry the hit is (`name` for name hits,
    /// `row 0 · column Name` for datasheet cells, …).
    pub location: String,
    /// Matched text.
    pub snippet: String,
    /// Fuzzy match score (0 in exact mode); higher ranks first.
    pub score: u16,
}

/// An entry left unsearched, with the reason. Skips never abort the scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// Archive path of the entry.
    pub entry: String,
    /// Why it was skipped (`unreadable`, `undecodable`, …).
    pub reason: String,
}

/// The outcome of [`run`].
#[derive(Debug, Clone, Default)]
pub struct GrepReport {
    /// Hits: name tier first, then content tier, best score first within tiers.
    pub hits: Vec<GrepHit>,
    /// Entries left unsearched.
    pub skipped: Vec<Skipped>,
}

/// Search `request.root` for `request.queries`.
pub fn run(request: &GrepRequest) -> anyhow::Result<GrepReport> {
    let queries = QuerySet::new(&request.queries, request.exact);
    let paks = PakSet::collect(request.root.clone(), Vec::new())?;
    let mut report = GrepReport::default();
    let mut staged: Vec<(Tier, u16, GrepHit)> = Vec::new();
    for path in paks.paths() {
        let reader = PakMmapReader::open(path)?;
        let pak = paks.relative(path);
        for entry in reader.entries() {
            scan_entry(
                &reader,
                &pak,
                entry.name(),
                entry.index(),
                &queries,
                &mut staged,
                &mut report.skipped,
            );
        }
    }
    order_hits(&mut staged);
    report.hits = staged.into_iter().map(|(_, _, hit)| hit).collect();
    Ok(report)
}

fn scan_entry(
    reader: &PakMmapReader,
    pak: &str,
    name: &str,
    index: usize,
    queries: &QuerySet,
    staged: &mut Vec<(Tier, u16, GrepHit)>,
    skipped: &mut Vec<Skipped>,
) {
    let extracted = crate::extract::entry(name, || reader.read_by_index(index).ok());
    if let Some(reason) = extracted.skip {
        skipped.push(Skipped {
            entry: name.to_string(),
            reason: match reason {
                crate::extract::SkipReason::Unreadable => "unreadable".to_string(),
                crate::extract::SkipReason::Undecodable => "undecodable".to_string(),
            },
        });
    }
    let format = shape::path_family(name).to_string();
    let hits = adapters::score_drafts(&extracted.facts, queries);
    let best_name = hits
        .iter()
        .filter(|hit| hit.location == "name")
        .max_by_key(|hit| hit.score);
    for hit in hits
        .iter()
        .filter(|hit| hit.location != "name")
        .chain(best_name)
    {
        let tier = if hit.location == "name" {
            Tier::Name
        } else {
            Tier::Content
        };
        staged.push((
            tier,
            hit.score,
            GrepHit {
                pak: pak.to_string(),
                entry: name.to_string(),
                format: format.clone(),
                location: hit.location.clone(),
                snippet: hit.snippet.clone(),
                score: hit.score,
            },
        ));
    }
}

/// Name tier first, then content tier, best score first within tiers (stable).
fn order_hits(staged: &mut [(Tier, u16, GrepHit)]) {
    staged.sort_by(|left, right| {
        (left.0 as u8)
            .cmp(&(right.0 as u8))
            .then_with(|| right.1.cmp(&left.1))
    });
}
