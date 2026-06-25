use std::borrow::Cow;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use humansize::{DECIMAL, format_size};

use crate::jobs::JobArgs;
use crate::support::{collect_matching, ensure_parent};
use crate::ui::{Cell, Report, Table};

use super::common::{csv_cell, finish_scan, lowered, path_label, text_matches};

#[derive(Debug, Args)]
pub struct Catalog {
    #[command(subcommand)]
    command: CatalogCmd,
}

#[derive(Debug, Subcommand)]
pub enum CatalogCmd {
    #[command(about = "Summarize asset catalog files")]
    Summary(CatalogSummary),
    #[command(about = "Find catalog entries by path text")]
    Find(CatalogFind),
    #[command(about = "Show exact catalog entries by path or asset id")]
    Get(CatalogGet),
    #[command(about = "Export catalog entries to CSV")]
    Export(CatalogExport),
}

#[derive(Debug, Args)]
pub struct CatalogSummary {
    /// Catalog file or directory. Omit to read from the located install's Engine.pak.
    #[arg(long)]
    path: Option<PathBuf>,

    #[arg(long, default_value_t = 25)]
    show: usize,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct CatalogFind {
    query: Vec<String>,

    /// Catalog file or directory. Omit to read from the located install's Engine.pak.
    #[arg(long)]
    path: Option<PathBuf>,

    /// Exact substring match instead of the default fuzzy ranking.
    #[arg(long)]
    exact: bool,

    #[arg(long, default_value_t = 25)]
    show: usize,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct CatalogGet {
    query: Vec<String>,

    /// Catalog file or directory. Omit to read from the located install's Engine.pak.
    #[arg(long)]
    path: Option<PathBuf>,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct CatalogExport {
    out: PathBuf,

    /// Catalog file or directory. Omit to read from the located install's Engine.pak.
    #[arg(long)]
    path: Option<PathBuf>,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Clone)]
struct CatalogScan {
    source: String,
    kind: String,
    version: u32,
    entries: usize,
    matched: usize,
    details: String,
    rows: Vec<CatalogRow>,
}

#[derive(Debug, Clone)]
struct CatalogRow {
    source: String,
    kind: String,
    size: String,
    asset_id: String,
    asset_type: String,
    flags: String,
    path: String,
    /// Fuzzy match score (0 outside fuzzy find); higher ranks first.
    score: u16,
}

#[derive(Debug, Clone)]
struct CatalogExportRow {
    source: String,
    kind: String,
    asset_id: String,
    asset_type: String,
    size_bytes: u32,
    flags: String,
    path: String,
}

impl Catalog {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            CatalogCmd::Summary(cmd) => cmd.run(),
            CatalogCmd::Find(cmd) => cmd.run(),
            CatalogCmd::Get(cmd) => cmd.run(),
            CatalogCmd::Export(cmd) => cmd.run(),
        }
    }
}

impl CatalogSummary {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let inputs = CatalogInput::collect(self.path.as_deref())?;
        let batch = ctx.map_results_compact(
            "catalog",
            &inputs,
            CatalogInput::label,
            |input, progress| {
                progress.step(|| scan_catalog(&input.label(), &input.bytes()?, self.show, &[], false))
            },
        );
        print_catalog_scans(batch, inputs.len(), "catalog")
    }
}

impl CatalogFind {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let inputs = CatalogInput::collect(self.path.as_deref())?;
        let find = lowered(self.query);
        let fuzzy = !self.exact;
        let batch = ctx.map_results_compact(
            "catalog find",
            &inputs,
            CatalogInput::label,
            |input, progress| {
                progress.step(|| scan_catalog(&input.label(), &input.bytes()?, self.show, &find, fuzzy))
            },
        );
        print_catalog_scans(batch, inputs.len(), "catalog find")
    }
}

impl CatalogGet {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let inputs = CatalogInput::collect(self.path.as_deref())?;
        let queries = lowered(self.query);
        let batch = ctx.map_results_compact(
            "catalog get",
            &inputs,
            CatalogInput::label,
            |input, progress| {
                progress.step(|| get_catalog(&input.label(), &input.bytes()?, &queries))
            },
        );
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut rows = Vec::new();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(mut scan) => rows.append(&mut scan),
                Err(error) => errors.push(error),
            }
        }
        rows.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then(left.path.cmp(&right.path))
                .then(left.asset_id.cmp(&right.asset_id))
        });

        let mut report = Report::new("catalog get")
            .stat("catalogs", inputs.len())
            .stat("matched", rows.len());
        report.table_or(catalog_rows_table(rows), "no catalog rows to show");
        report.print();

        finish_scan(cancelled, skipped, &errors, "catalog get")
    }
}

impl CatalogExport {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let inputs = CatalogInput::collect(self.path.as_deref())?;
        let batch = ctx.map_results_compact(
            "catalog export",
            &inputs,
            CatalogInput::label,
            |input, progress| progress.step(|| export_catalog(&input.label(), &input.bytes()?)),
        );
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut rows = Vec::new();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(mut scan) => rows.append(&mut scan),
                Err(error) => errors.push(error),
            }
        }
        rows.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then(left.path.cmp(&right.path))
                .then(left.asset_id.cmp(&right.asset_id))
        });

        write_catalog_csv(&self.out, &rows)
            .with_context(|| format!("write {}", self.out.display()))?;
        Report::new("catalog export")
            .stat("catalogs", inputs.len())
            .stat("exported", rows.len())
            .stat("path", self.out.display())
            .print();

        finish_scan(cancelled, skipped, &errors, "catalog export")
    }
}

fn print_catalog_scans(
    batch: nw_jobs::JobBatch<Result<CatalogScan>>,
    path_count: usize,
    label: &str,
) -> Result<()> {
    let skipped = batch.skipped();
    let cancelled = batch.was_cancelled();
    let mut scans = Vec::new();
    let mut errors = Vec::new();

    for result in batch.into_completed() {
        match result {
            Ok(scan) => scans.push(scan),
            Err(error) => errors.push(error),
        }
    }
    scans.sort_by(|left, right| left.source.cmp(&right.source));

    let total_entries = scans.iter().map(|scan| scan.entries).sum::<usize>();
    let total_matched = scans.iter().map(|scan| scan.matched).sum::<usize>();
    let mut report = Report::new(label)
        .stat("catalogs", path_count)
        .stat("entries", total_entries)
        .stat("matched", total_matched);
    for scan in &scans {
        let details = if scan.details.is_empty() {
            String::new()
        } else {
            format!("  {}", scan.details)
        };
        report.kv(
            scan.source.clone(),
            format!(
                "{} v{}  entries {}  matched {}{}",
                scan.kind, scan.version, scan.entries, scan.matched, details
            ),
        );
    }

    let mut rows = scans
        .into_iter()
        .flat_map(|scan| scan.rows)
        .collect::<Vec<_>>();
    // Stable sort by fuzzy score (descending) — a no-op when nothing was fuzzy
    // matched, since every score is then 0.
    rows.sort_by_key(|row| std::cmp::Reverse(row.score));
    report.table_or(catalog_rows_table(rows), "no catalog rows to show");
    report.print();

    finish_scan(cancelled, skipped, &errors, label)
}

fn catalog_rows_table(rows: Vec<CatalogRow>) -> Table {
    let mut table = Table::new([
        "Catalog", "Kind", "Size", "AssetId", "Type", "Flags", "Path",
    ])
    .right([2]);
    for row in rows {
        table.push([
            Cell::path(row.source),
            Cell::text(row.kind),
            Cell::size(row.size),
            Cell::text(row.asset_id),
            Cell::text(row.asset_type),
            Cell::text(row.flags),
            Cell::path(row.path),
        ]);
    }
    table
}

/// A catalog to scan: either a loose file on disk, or bytes already read out of
/// the install's `Engine.pak`.
enum CatalogInput {
    File(PathBuf),
    Pak { label: String, bytes: Vec<u8> },
}

impl CatalogInput {
    /// The catalogs for a command: matching loose files under `path`, or — when
    /// `path` is omitted — the catalog read straight from the located install.
    fn collect(path: Option<&Path>) -> Result<Vec<Self>> {
        match path {
            Some(path) => Ok(collect_matching(path, |p| nw_asset::is_asset_catalog_path(p))?
                .into_iter()
                .map(Self::File)
                .collect()),
            None => {
                let install = crate::source::locate()?;
                let (rasc, raoc) = crate::source::install_catalog_bytes(&install.assets())?;
                let mut inputs = vec![Self::Pak {
                    label: format!("Engine.pak/{}", nw_asset::ASSET_CATALOG_PATH),
                    bytes: rasc,
                }];
                if let Some(raoc) = raoc {
                    inputs.push(Self::Pak {
                        label: format!("Engine.pak/{}", nw_asset::ASSET_CATALOG_OPTIMIZED_PATH),
                        bytes: raoc,
                    });
                }
                Ok(inputs)
            }
        }
    }

    fn label(&self) -> String {
        match self {
            Self::File(path) => path_label(path),
            Self::Pak { label, .. } => label.clone(),
        }
    }

    /// The catalog bytes, reading the file on demand.
    fn bytes(&self) -> Result<Cow<'_, [u8]>> {
        match self {
            Self::File(path) => Ok(Cow::Owned(
                std::fs::read(path).with_context(|| format!("read {}", path.display()))?,
            )),
            Self::Pak { bytes, .. } => Ok(Cow::Borrowed(bytes)),
        }
    }
}

fn scan_catalog(
    source: &str,
    bytes: &[u8],
    limit: usize,
    find: &[String],
    fuzzy: bool,
) -> Result<CatalogScan> {
    let catalog = nw_asset::Catalog::parse(bytes).with_context(|| format!("parse {source}"))?;
    let source = source.to_string();
    let mut search = (fuzzy && !find.is_empty()).then(|| crate::fuzzy::MultiSearch::new(find));

    match catalog {
        nw_asset::Catalog::Rasc(catalog) => {
            let mut rows = Vec::new();
            let mut matched = 0usize;
            for entry in catalog.entries() {
                let score = match &mut search {
                    Some(search) => match search.score(entry.path()) {
                        Some(score) => score,
                        None => continue,
                    },
                    None => {
                        if !find.is_empty() && !text_matches(entry.path(), find) {
                            continue;
                        }
                        0
                    }
                };
                matched += 1;
                if rows.len() < limit {
                    rows.push(CatalogRow {
                        source: source.clone(),
                        kind: "RASC".to_string(),
                        size: format_size(u64::from(entry.size_bytes()), DECIMAL),
                        asset_id: entry.asset_id().to_string(),
                        asset_type: entry.asset_type().to_string(),
                        flags: String::new(),
                        path: entry.path().to_string(),
                        score,
                    });
                }
            }
            Ok(CatalogScan {
                source,
                kind: "RASC".to_string(),
                version: catalog.version(),
                entries: catalog.len(),
                matched,
                details: String::new(),
                rows,
            })
        }
        nw_asset::Catalog::Raoc(catalog) => {
            let mut rows = Vec::new();
            let mut matched = 0usize;
            for entry in catalog.entries() {
                let asset_id = entry.asset_id().to_string();
                let asset_type = entry.asset_type().to_string();
                let flags = format!("0x{:08x}", entry.flags());
                let score = match &mut search {
                    Some(search) => {
                        match search.score_any([asset_id.as_str(), asset_type.as_str(), &flags]) {
                            Some(score) => score,
                            None => continue,
                        }
                    }
                    None => {
                        if !find.is_empty() && !raoc_text_matches(entry, find) {
                            continue;
                        }
                        0
                    }
                };
                matched += 1;
                if rows.len() < limit {
                    rows.push(CatalogRow {
                        source: source.clone(),
                        kind: "RAOC".to_string(),
                        size: format_size(u64::from(entry.size_bytes()), DECIMAL),
                        asset_id,
                        asset_type,
                        flags,
                        path: String::new(),
                        score,
                    });
                }
            }
            Ok(CatalogScan {
                source,
                kind: "RAOC".to_string(),
                version: catalog.version(),
                entries: catalog.len(),
                matched,
                details: format!(
                    "path hashes {}  dependencies {}  types {}",
                    catalog.path_ids().len(),
                    catalog.dependencies().len(),
                    catalog.types().len()
                ),
                rows,
            })
        }
    }
}

fn get_catalog(source: &str, bytes: &[u8], queries: &[String]) -> Result<Vec<CatalogRow>> {
    let catalog = nw_asset::Catalog::parse(bytes).with_context(|| format!("parse {source}"))?;
    let source = source.to_string();
    let mut rows = Vec::new();

    match catalog {
        nw_asset::Catalog::Rasc(catalog) => {
            for entry in catalog.entries() {
                let asset_id = entry.asset_id().to_string();
                let asset_type = entry.asset_type().to_string();
                let path = entry.path().to_string();
                let haystack = [
                    path.to_ascii_lowercase(),
                    asset_id.to_ascii_lowercase(),
                    asset_type.to_ascii_lowercase(),
                ];
                if !queries
                    .iter()
                    .any(|query| haystack.iter().any(|value| value == query))
                {
                    continue;
                }
                rows.push(CatalogRow {
                    source: source.clone(),
                    kind: "RASC".to_string(),
                    size: format_size(u64::from(entry.size_bytes()), DECIMAL),
                    asset_id,
                    asset_type,
                    flags: String::new(),
                    path,
                    score: 0,
                });
            }
        }
        nw_asset::Catalog::Raoc(catalog) => {
            for entry in catalog.entries() {
                let asset_id = entry.asset_id().to_string();
                let asset_type = entry.asset_type().to_string();
                let flags = format!("0x{:08x}", entry.flags());
                let haystack = [
                    asset_id.to_ascii_lowercase(),
                    asset_type.to_ascii_lowercase(),
                    flags.to_ascii_lowercase(),
                ];
                if !queries
                    .iter()
                    .any(|query| haystack.iter().any(|value| value == query))
                {
                    continue;
                }
                rows.push(CatalogRow {
                    source: source.clone(),
                    kind: "RAOC".to_string(),
                    size: format_size(u64::from(entry.size_bytes()), DECIMAL),
                    asset_id,
                    asset_type,
                    flags,
                    path: String::new(),
                    score: 0,
                });
            }
        }
    }

    Ok(rows)
}

fn export_catalog(source: &str, bytes: &[u8]) -> Result<Vec<CatalogExportRow>> {
    let catalog = nw_asset::Catalog::parse(bytes).with_context(|| format!("parse {source}"))?;
    let source = source.to_string();

    Ok(match catalog {
        nw_asset::Catalog::Rasc(catalog) => catalog
            .entries()
            .iter()
            .map(|entry| CatalogExportRow {
                source: source.clone(),
                kind: "RASC".to_string(),
                asset_id: entry.asset_id().to_string(),
                asset_type: entry.asset_type().to_string(),
                size_bytes: entry.size_bytes(),
                flags: String::new(),
                path: entry.path().to_string(),
            })
            .collect(),
        nw_asset::Catalog::Raoc(catalog) => catalog
            .entries()
            .iter()
            .map(|entry| CatalogExportRow {
                source: source.clone(),
                kind: "RAOC".to_string(),
                asset_id: entry.asset_id().to_string(),
                asset_type: entry.asset_type().to_string(),
                size_bytes: entry.size_bytes(),
                flags: format!("0x{:08x}", entry.flags()),
                path: String::new(),
            })
            .collect(),
    })
}

fn write_catalog_csv(path: &Path, rows: &[CatalogExportRow]) -> Result<()> {
    ensure_parent(path)?;
    let mut csv = String::from("catalog,kind,asset_id,asset_type,size_bytes,flags,path\n");
    for row in rows {
        csv.push_str(&csv_cell(&row.source));
        csv.push(',');
        csv.push_str(&csv_cell(&row.kind));
        csv.push(',');
        csv.push_str(&csv_cell(&row.asset_id));
        csv.push(',');
        csv.push_str(&csv_cell(&row.asset_type));
        csv.push(',');
        csv.push_str(&row.size_bytes.to_string());
        csv.push(',');
        csv.push_str(&csv_cell(&row.flags));
        csv.push(',');
        csv.push_str(&csv_cell(&row.path));
        csv.push('\n');
    }
    std::fs::write(path, csv)?;
    Ok(())
}

fn raoc_text_matches(entry: &nw_asset::RaocEntry, needles: &[String]) -> bool {
    let fields = [
        entry.asset_id().to_string().to_ascii_lowercase(),
        entry.asset_type().to_string().to_ascii_lowercase(),
        format!("0x{:08x}", entry.flags()),
    ];
    needles
        .iter()
        .any(|needle| fields.iter().any(|value| value.contains(needle)))
}
