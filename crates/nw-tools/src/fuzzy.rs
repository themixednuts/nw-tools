//! Fuzzy matching shared by the interactive filters and the CLI search commands,
//! backed by `frizbee` — the same SIMD Smith-Waterman matcher that powers the
//! `fff` file finder.
//!
//! Two shapes are offered: [`rank`] for one-shot batch matching over an
//! in-memory list (the TUI filters), and [`Search`]/[`MultiSearch`] for streaming
//! use where a query is built once and then scored against many candidates as
//! they arrive (the CLI scans, which iterate pak entries on worker threads).

use frizbee::{Config, Matcher, match_list};

/// Fuzzy-rank `haystacks` against `query`, returning the index of each matching
/// entry paired with its score, best match first. Non-matches are dropped. An
/// empty query keeps every entry in its original order.
pub fn rank(query: &str, haystacks: &[String]) -> Vec<(usize, u16)> {
    if query.is_empty() {
        return (0..haystacks.len()).map(|index| (index, 0)).collect();
    }
    // Config::default() sorts by score descending and requires every needle
    // character to be present (max_typos = 0), which is the behaviour we want
    // for an incremental filter.
    match_list(query, haystacks, &Config::default())
        .into_iter()
        .map(|matched| (matched.index as usize, matched.score))
        .collect()
}

/// A reusable matcher for one query. Build once per thread, then [`Search::score`]
/// many candidates. `frizbee`'s prefilter rejects non-matches cheaply, so this is
/// efficient even when most candidates miss.
pub struct Search {
    matcher: Matcher,
}

impl Search {
    pub fn new(query: &str) -> Self {
        Self {
            matcher: Matcher::new(query, &Config::default()),
        }
    }

    /// Score `haystack`; `None` means it does not match the query.
    pub fn score(&mut self, haystack: &str) -> Option<u16> {
        self.matcher
            .match_list(std::slice::from_ref(&haystack))
            .first()
            .map(|matched| matched.score)
    }
}

/// A set of OR-combined queries (the multi-term `find` commands). A candidate
/// matches if any term matches; its score is the best term's score.
pub struct MultiSearch {
    searches: Vec<Search>,
}

impl MultiSearch {
    pub fn new(queries: &[String]) -> Self {
        Self {
            searches: queries.iter().map(|query| Search::new(query)).collect(),
        }
    }

    /// Best score of any term against `haystack` (`None` = no term matched).
    pub fn score(&mut self, haystack: &str) -> Option<u16> {
        self.searches
            .iter_mut()
            .filter_map(|search| search.score(haystack))
            .max()
    }

    /// Best score of any term against any of `haystacks`.
    pub fn score_any<'a>(&mut self, haystacks: impl IntoIterator<Item = &'a str>) -> Option<u16> {
        haystacks
            .into_iter()
            .filter_map(|haystack| self.score(haystack))
            .max()
    }
}
