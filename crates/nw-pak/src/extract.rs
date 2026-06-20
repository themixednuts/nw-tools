//! Extract pak entries to a filesystem tree.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nw_filesystem::safe_join;
use nw_jobs::{CancellationToken, JobRunner};
use thiserror::Error;

use crate::{PakError, PakFile, PakMmapReader};

/// Options for extracting pak entries to disk.
#[derive(Debug, Clone, Copy, Default)]
pub struct PakExtractOptions<'a> {
    /// Optional substring filter applied to pak entry names.
    pub filter: Option<&'a str>,
    /// Optional wildcard filters applied to pak entry names.
    pub globs: &'a [&'a str],
    /// Extract entries one by one for stable debugging.
    pub sequential: bool,
    /// Replace existing destination files.
    pub overwrite: bool,
}

/// Summary of a pak extraction run.
#[derive(Debug, Clone)]
pub struct PakExtractReport {
    output_root: PathBuf,
    selected_entries: usize,
    written_files: usize,
    skipped_existing_files: usize,
    failures: Vec<PakExtractEntryFailure>,
}

impl PakExtractReport {
    #[inline]
    #[must_use]
    pub fn selected_entries(&self) -> usize {
        self.selected_entries
    }

    #[inline]
    #[must_use]
    pub fn written_files(&self) -> usize {
        self.written_files
    }

    #[inline]
    #[must_use]
    pub fn skipped_existing_files(&self) -> usize {
        self.skipped_existing_files
    }

    #[inline]
    #[must_use]
    pub fn failures(&self) -> &[PakExtractEntryFailure] {
        &self.failures
    }

    #[inline]
    #[must_use]
    pub fn has_failures(&self) -> bool {
        !self.failures.is_empty()
    }

    /// Return an error if any entry failed.
    pub fn ensure_success(&self) -> Result<(), PakExtractFailures> {
        if self.has_failures() {
            Err(PakExtractFailures {
                failures: self.failures.len(),
            })
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for PakExtractReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} entries to extract -> {}",
            self.selected_entries,
            self.output_root.display()
        )?;
        writeln!(
            f,
            "wrote {} files{}{}",
            self.written_files,
            if self.skipped_existing_files > 0 {
                format!(
                    ", skipped {} existing (pass --overwrite to replace)",
                    self.skipped_existing_files
                )
            } else {
                String::new()
            },
            if self.failures.is_empty() {
                String::new()
            } else {
                format!(", {} errors", self.failures.len())
            }
        )?;

        for failure in self.failures.iter().take(20) {
            writeln!(f, "  {}: {}", failure.entry, failure.error)?;
        }
        if self.failures.len() > 20 {
            writeln!(f, "  ... ({} more errors hidden)", self.failures.len() - 20)?;
        }
        Ok(())
    }
}

/// One entry-level extraction failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PakExtractEntryFailure {
    pub entry: String,
    pub error: String,
}

/// Error returned by [`PakExtractReport::ensure_success`].
#[derive(Debug, Error)]
#[error("{failures} entries failed to extract")]
pub struct PakExtractFailures {
    failures: usize,
}

#[derive(Debug, Error)]
pub enum PakExtractError {
    #[error("create output root {path:?}")]
    CreateOutputRoot {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("canonicalize output root {path:?}")]
    CanonicalizeOutputRoot {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("open pak {path:?}")]
    OpenPak {
        path: PathBuf,
        #[source]
        source: PakError,
    },

    #[error("extract cancelled ({skipped} queued entry(s) skipped)")]
    Cancelled { skipped: usize },
}

impl PakFile {
    /// Extract matching entries from `pak_path` into `output_root`.
    pub fn extract_to_dir(
        pak_path: impl AsRef<Path>,
        output_root: impl AsRef<Path>,
        options: PakExtractOptions<'_>,
    ) -> Result<PakExtractReport, PakExtractError> {
        let runner = if options.sequential {
            JobRunner::inline()
        } else {
            JobRunner::automatic()
        };
        let cancel = CancellationToken::new();
        extract_to_dir(
            pak_path.as_ref(),
            output_root.as_ref(),
            options,
            &runner,
            &cancel,
        )
    }

    /// Extract matching entries using caller-supplied parallelism and
    /// cancellation.
    pub fn extract_to_dir_with(
        pak_path: impl AsRef<Path>,
        output_root: impl AsRef<Path>,
        options: PakExtractOptions<'_>,
        runner: &JobRunner,
        cancel: &CancellationToken,
    ) -> Result<PakExtractReport, PakExtractError> {
        extract_to_dir(
            pak_path.as_ref(),
            output_root.as_ref(),
            options,
            runner,
            cancel,
        )
    }
}

fn extract_to_dir(
    pak_path: &Path,
    output_root: &Path,
    options: PakExtractOptions<'_>,
    runner: &JobRunner,
    cancel: &CancellationToken,
) -> Result<PakExtractReport, PakExtractError> {
    fs::create_dir_all(output_root).map_err(|source| PakExtractError::CreateOutputRoot {
        path: output_root.to_path_buf(),
        source,
    })?;

    let output_root =
        output_root
            .canonicalize()
            .map_err(|source| PakExtractError::CanonicalizeOutputRoot {
                path: output_root.to_path_buf(),
                source,
            })?;

    let names = selected_entry_names(pak_path, options.filter, options.globs)?;
    let mut report = PakExtractReport {
        output_root: output_root.clone(),
        selected_entries: names.len(),
        written_files: 0,
        skipped_existing_files: 0,
        failures: Vec::new(),
    };

    if options.sequential {
        extract_sequential(
            pak_path,
            &output_root,
            &names,
            options.overwrite,
            &mut report,
        )?;
    } else {
        extract_parallel(
            pak_path,
            &output_root,
            &names,
            options.overwrite,
            &mut report,
            runner,
            cancel,
        )?;
    }

    Ok(report)
}

fn selected_entry_names(
    pak_path: &Path,
    filter: Option<&str>,
    globs: &[&str],
) -> Result<Vec<String>, PakExtractError> {
    let archive = PakFile::open(pak_path).map_err(|source| PakExtractError::OpenPak {
        path: pak_path.to_path_buf(),
        source,
    })?;
    Ok(archive
        .entries()
        .filter(|entry| entry_selected(entry.name(), filter, globs))
        .map(|entry| entry.name().to_string())
        .collect())
}

fn entry_selected(name: &str, filter: Option<&str>, globs: &[&str]) -> bool {
    if filter.is_none() && globs.is_empty() {
        return true;
    }
    filter.is_some_and(|filter| name.contains(filter))
        || globs.iter().any(|glob| wildcard_match(glob, name))
}

fn wildcard_match(pattern: &str, candidate: &str) -> bool {
    let pattern = pattern.replace('\\', "/").to_ascii_lowercase();
    let candidate = candidate.replace('\\', "/").to_ascii_lowercase();
    let pat = pattern.as_bytes();
    let text = candidate.as_bytes();
    let mut p = 0usize;
    let mut t = 0usize;
    let mut star = None;
    let mut star_t = 0usize;

    while t < text.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star = Some(p);
            star_t = t;
            p += 1;
        } else if let Some(star_pos) = star {
            p = star_pos + 1;
            star_t += 1;
            t = star_t;
        } else {
            return false;
        }
    }

    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

fn extract_sequential(
    pak_path: &Path,
    output_root: &Path,
    names: &[String],
    overwrite: bool,
    report: &mut PakExtractReport,
) -> Result<(), PakExtractError> {
    let mut archive = PakFile::open(pak_path).map_err(|source| PakExtractError::OpenPak {
        path: pak_path.to_path_buf(),
        source,
    })?;
    let mut buf = Vec::new();
    for name in names {
        let Some(dest) = resolve_destination(output_root, name, report) else {
            continue;
        };
        if skip_existing(&dest, overwrite, report) {
            continue;
        }
        match archive
            .read_into(name, &mut buf)
            .map_err(|error| error.to_string())
            .and_then(|_| write_entry(&dest, &buf))
        {
            Ok(()) => report.written_files += 1,
            Err(error) => report.failures.push(PakExtractEntryFailure {
                entry: name.clone(),
                error,
            }),
        }
    }
    Ok(())
}

fn extract_parallel(
    pak_path: &Path,
    output_root: &Path,
    names: &[String],
    overwrite: bool,
    report: &mut PakExtractReport,
    runner: &JobRunner,
    cancel: &CancellationToken,
) -> Result<(), PakExtractError> {
    let mut pending = Vec::new();
    for name in names {
        let Some(dest) = resolve_destination(output_root, name, report) else {
            continue;
        };
        if skip_existing(&dest, overwrite, report) {
            continue;
        }
        pending.push((name.clone(), dest));
    }
    if pending.is_empty() {
        return Ok(());
    }

    let reader =
        Arc::new(
            PakMmapReader::open(pak_path).map_err(|source| PakExtractError::OpenPak {
                path: pak_path.to_path_buf(),
                source,
            })?,
        );
    let batch = runner.map_until_cancelled(&pending, cancel, {
        let reader = Arc::clone(&reader);
        move |(name, dest)| (name.clone(), dest.clone(), reader.read(name))
    });
    let skipped = batch.skipped();
    let cancelled = batch.was_cancelled();

    for (name, dest, result) in batch.into_completed() {
        match result
            .map_err(|error| error.to_string())
            .and_then(|bytes| write_entry(&dest, &bytes))
        {
            Ok(()) => report.written_files += 1,
            Err(error) => report.failures.push(PakExtractEntryFailure {
                entry: name.clone(),
                error,
            }),
        }
    }

    if cancelled {
        return Err(PakExtractError::Cancelled { skipped });
    }

    Ok(())
}

fn resolve_destination(
    output_root: &Path,
    name: &str,
    report: &mut PakExtractReport,
) -> Option<PathBuf> {
    match safe_join(output_root, name) {
        Ok(dest) => Some(dest),
        Err(error) => {
            report.failures.push(PakExtractEntryFailure {
                entry: name.to_string(),
                error: error.to_string(),
            });
            None
        }
    }
}

fn skip_existing(dest: &Path, overwrite: bool, report: &mut PakExtractReport) -> bool {
    if dest.exists() && !overwrite {
        report.skipped_existing_files += 1;
        true
    } else {
        false
    }
}

fn write_entry(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut file =
        fs::File::create(dest).map_err(|error| format!("create {}: {error}", dest.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("write {}: {error}", dest.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_formats_success_deterministically() {
        let report = PakExtractReport {
            output_root: PathBuf::from("out"),
            selected_entries: 3,
            written_files: 2,
            skipped_existing_files: 1,
            failures: Vec::new(),
        };

        assert_eq!(
            report.to_string(),
            "3 entries to extract -> out\nwrote 2 files, skipped 1 existing (pass --overwrite to replace)\n"
        );
    }

    #[test]
    fn report_formats_failures_deterministically() {
        let report = PakExtractReport {
            output_root: PathBuf::from("out"),
            selected_entries: 1,
            written_files: 0,
            skipped_existing_files: 0,
            failures: vec![PakExtractEntryFailure {
                entry: "../bad".to_string(),
                error: "archive path contains a parent-directory component: ../bad".to_string(),
            }],
        };

        assert_eq!(
            report.to_string(),
            "1 entries to extract -> out\nwrote 0 files, 1 errors\n  ../bad: archive path contains a parent-directory component: ../bad\n"
        );
    }

    #[test]
    fn wildcard_selector_matches_paths_case_insensitively() {
        assert!(entry_selected(
            "SharedAssets/Coatlicue/World/Regions/r_+00_+00/region.slicedata",
            None,
            &["sharedassets/coatlicue/world/regions/*/region.slicedata"],
        ));
        assert!(!entry_selected(
            "sharedassets/coatlicue/world/regions/r_+00_+00/region.heightmap",
            None,
            &["sharedassets/coatlicue/world/regions/*/region.slicedata"],
        ));
    }
}
