use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use humansize::{DECIMAL, format_size};
use nw_localization::{
    KeyFileIndex, LANGUAGE_MANIFEST_ASSET_PATH, LanguageCode, LanguageManifest, LocalizationCatalog,
    LocalizationCatalogBuilder, LocalizationDocument, LocalizationKey, LocalizationLoader,
    LocalizationTag, localization_asset_path, localization_keys,
};
use nw_objectstream::ObjectStreamEncoding;
use nw_objectstream::lookup::NameLookup;
use nw_pak::PakMmapReader;

use crate::jobs::{JobArgs, RunCtx};
use crate::support::{PakSet, PathSelector, collect_matching, load_lookup, path_ext};
use crate::tui::SheetSource;
use crate::ui::{Cell, Report, Table};

#[derive(Debug, Subcommand)]
pub enum Cmd {
    #[command(about = "Inspect asset catalog files")]
    Catalog(Catalog),
    #[command(about = "Inspect datasheet files")]
    Datasheet(Datasheet),
    #[command(name = "dds", about = "Inspect or convert DDS texture files")]
    Dds(Dds),
    #[command(name = "model", about = "Convert CGF meshes to glTF (.glb/.gltf)")]
    Model(crate::model::Model),
    #[command(name = "objectstream", about = "Inspect ObjectStream files")]
    ObjectStream(ObjectStream),
}

impl Cmd {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Catalog(cmd) => cmd.run(),
            Self::Datasheet(cmd) => cmd.run(),
            Self::Dds(cmd) => cmd.run(),
            Self::Model(cmd) => cmd.run(),
            Self::ObjectStream(cmd) => cmd.run(),
        }
    }
}

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

#[derive(Debug, Args)]
pub struct Datasheet {
    /// Datasheet file or directory. Omit to browse datasheets under the current directory.
    path: Option<PathBuf>,

    #[arg(long, default_value_t = 25)]
    show: usize,

    #[arg(long)]
    columns: bool,

    #[arg(long)]
    rows: Option<usize>,

    #[arg(long)]
    find: Vec<String>,

    /// Exact substring match for --find instead of the default fuzzy ranking.
    #[arg(long)]
    exact: bool,

    #[arg(long)]
    show_empty: bool,

    /// Locale used to resolve localization labels in string cells.
    #[arg(long)]
    locale: Option<LanguageCode>,

    /// Asset root used to load localization. Defaults to the detected game assets path.
    #[arg(long = "loc-root", value_name = "ROOT", requires = "locale")]
    loc_root: Option<PathBuf>,

    /// Localization manifest tag to load; repeat for multiple tags.
    #[arg(long = "loc-tag", requires = "locale")]
    loc_tags: Vec<LocalizationTag>,

    /// String rendering mode when localization is loaded.
    #[arg(long, value_enum, default_value_t = LocalizeArg::Text)]
    localize: LocalizeArg,

    /// Export rows to CSV under this file or directory.
    #[arg(long, value_name = "PATH")]
    csv: Option<PathBuf>,

    /// Replace existing CSV outputs.
    #[arg(long, requires = "csv")]
    overwrite: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Dds {
    /// DDS file or directory. Omit to browse textures under the current directory.
    path: Option<PathBuf>,

    #[arg(long, default_value_t = 40)]
    show: usize,

    /// Convert the texture(s) to this format, written under --out.
    #[arg(long, value_enum, requires = "out")]
    to: Option<DdsFormat>,

    /// Output path/dir for --to.
    #[arg(long, value_name = "PATH", requires = "to")]
    out: Option<PathBuf>,

    /// Decode the texture and show it inline (kitty graphics protocol).
    #[arg(long)]
    view: bool,

    /// Replace existing outputs.
    #[arg(long)]
    overwrite: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

/// Output format for `dds --to`.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum DdsFormat {
    Ktx2,
    Png,
}

#[derive(Debug, Args)]
pub struct ObjectStream {
    /// ObjectStream file or directory. Omit to browse files under the current directory.
    path: Option<PathBuf>,

    #[arg(long, conflicts_with = "to")]
    dom: bool,

    #[arg(long, conflicts_with = "to")]
    query: Option<String>,

    /// Exact substring match instead of the default fuzzy ranking (with --query).
    #[arg(long)]
    exact: bool,

    /// Convert ObjectStream files to this encoding.
    #[arg(long, value_enum)]
    to: Option<EncodingArg>,

    /// Conversion output file or directory. Defaults beside each input.
    #[arg(long, value_name = "PATH", requires = "to")]
    out: Option<PathBuf>,

    /// Replace existing conversion outputs.
    #[arg(long, requires = "to")]
    overwrite: bool,

    /// Case-insensitive path substring prefilter.
    #[arg(long)]
    filter: Option<String>,

    /// Path glob prefilter; repeat for multiple patterns.
    #[arg(long)]
    glob: Vec<String>,

    #[arg(long, default_value_t = 40)]
    show: usize,

    #[arg(long, default_value_t = 20)]
    files: usize,

    #[arg(long)]
    no_names: bool,

    /// Optional extension prefilter before content sniffing.
    #[arg(long = "ext")]
    extensions: Vec<String>,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EncodingArg {
    Binary,
    Xml,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LocalizeArg {
    Key,
    Text,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectMode {
    Stats,
    Dom { limit: usize },
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

#[derive(Debug, Clone)]
struct SheetOptions<'a> {
    columns: bool,
    rows: Option<usize>,
    find: Vec<String>,
    fuzzy: bool,
    show_empty: bool,
    localization: Option<&'a LocalizationCatalog>,
    localize: LocalizeArg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SheetSummary {
    version: u32,
    rows: usize,
    columns: usize,
    cells: usize,
    name: String,
    type_name: String,
    string_columns: usize,
    number_columns: usize,
    boolean_columns: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SheetTotals {
    files: usize,
    rows: usize,
    columns: usize,
    cells: usize,
}

#[derive(Debug, Clone)]
struct SheetScan {
    source: String,
    summary: SheetSummary,
    columns: Vec<ColumnRow>,
    rows: Vec<RowSample>,
    hits: Vec<SheetHit>,
}

#[derive(Debug, Clone)]
struct ColumnRow {
    source: String,
    index: String,
    kind: String,
    crc: String,
    name: String,
}

#[derive(Debug, Clone)]
struct RowSample {
    source: String,
    row: String,
    values: String,
}

#[derive(Debug, Clone)]
struct SheetHit {
    source: String,
    row: String,
    column: String,
    value: String,
    /// Fuzzy match score (0 in exact mode); higher ranks first.
    score: u16,
}

#[derive(Debug, Clone)]
struct SheetExport {
    source: String,
    output: String,
    rows: String,
}

#[derive(Debug, Clone)]
struct DdsScan {
    source: String,
    kind: String,
    dimensions: String,
    mipmaps: String,
    format: String,
    dx10: String,
    cry: String,
    bytes: String,
}

#[derive(Debug, Clone)]
struct DdsGroup {
    header: PathBuf,
    sidecars: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct DdsConvert {
    source: String,
    output: String,
    bytes: String,
}

#[derive(Debug, Clone)]
enum ObjectScan {
    Stats(ObjectStatsScan),
    Dom(ObjectDomScan),
    Search {
        source: String,
        hits: Vec<ObjectHit>,
    },
}

#[derive(Debug, Clone)]
struct ObjectStatsScan {
    source: String,
    stats: nw_objectstream::stats::Stats,
    names_loaded: bool,
}

#[derive(Debug, Clone)]
struct ObjectDomScan {
    source: String,
    version: u32,
    top_level_elements: usize,
    total_elements: usize,
    rows: Vec<ObjectDomRow>,
}

#[derive(Debug, Clone)]
struct ObjectDomRow {
    index: String,
    flags: String,
    id: String,
    type_name: String,
    field: String,
}

#[derive(Debug, Clone)]
struct ObjectHit {
    kind: String,
    count: u64,
    score: u32,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectConvertRow {
    source: String,
    output: String,
    encoding: String,
    bytes: String,
}

impl From<nw_datasheet::DatasheetSummary<'_>> for SheetSummary {
    fn from(summary: nw_datasheet::DatasheetSummary<'_>) -> Self {
        Self {
            version: summary.version,
            rows: summary.rows,
            columns: summary.columns,
            cells: summary.cells,
            name: summary.name.to_owned(),
            type_name: summary.type_name.to_owned(),
            string_columns: summary.string_columns,
            number_columns: summary.number_columns,
            boolean_columns: summary.boolean_columns,
        }
    }
}

impl SheetTotals {
    fn add(&mut self, summary: &SheetSummary) {
        self.files += 1;
        self.rows += summary.rows;
        self.columns += summary.columns;
        self.cells += summary.cells;
    }
}

impl Catalog {
    fn run(self) -> Result<()> {
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

impl Datasheet {
    fn run(self) -> Result<()> {
        // The interactive grid is the front door: with no path it streams from
        // the located install's paks, and it loads localization dynamically from
        // inside the TUI — so it short-circuits before any filesystem scan or
        // eager catalog load.
        let will_grid = crate::tui::interactive()
            && self.csv.is_none()
            && !self.columns
            && self.rows.is_none()
            && self.find.is_empty();
        if will_grid {
            let mode = match self.localize {
                LocalizeArg::Key => 0,
                LocalizeArg::Text => 1,
                LocalizeArg::Both => 2,
            };
            let initial = self.locale.as_ref().map(|code| code.as_str().to_string());
            return browse_datasheets(
                self.path.clone(),
                self.loc_root.clone(),
                initial,
                mode,
                self.jobs.jobs,
            );
        }

        let ctx = self.jobs.ctx()?;
        let root = self.path.clone().unwrap_or_else(|| PathBuf::from("."));
        let paths = collect_matching(&root, nw_datasheet::is_datasheet_path)?;
        let find = lowered(self.find);
        let needs_localization = self.locale.is_some()
            && self.localize != LocalizeArg::Key
            && (self.csv.is_some() || self.rows.is_some() || !find.is_empty());
        let needed_keys = if needs_localization {
            collect_datasheet_localization_keys(
                &paths,
                (self.csv.is_none() && find.is_empty())
                    .then_some(self.rows)
                    .flatten(),
            )?
        } else {
            BTreeSet::new()
        };
        let localization = self
            .locale
            .clone()
            .filter(|_| needs_localization)
            .map(|locale| {
                let root = match self.loc_root.as_ref() {
                    Some(root) => root.clone(),
                    None => nw_locator::Install::locate()?.assets(),
                };
                let assets = nw_asset::AssetStore::open(root)?;
                // Run the parallel load on the configured --jobs pool.
                let catalog = ctx.runner.install(|| {
                    LocalizationLoader::new(&assets, locale)
                        .tags(self.loc_tags.clone())
                        .keys(needed_keys.iter().cloned())
                        .load()
                })?;
                Ok::<LocalizationCatalog, anyhow::Error>(catalog)
            })
            .transpose()?;
        let localization = localization.map(Arc::new);
        if let Some(catalog) = localization.as_ref() {
            let loc = catalog.report();
            Report::new("localization")
                .stat("language", catalog.language())
                .stat("needed", needed_keys.len())
                .stat("files", loc.source_files())
                .stat("entries", loc.entries())
                .stat("duplicates", loc.duplicates().len())
                .print();
        }
        let options = SheetOptions {
            columns: self.columns,
            rows: self.rows,
            find,
            fuzzy: !self.exact,
            show_empty: self.show_empty,
            localization: localization.as_deref(),
            localize: self.localize,
        };
        if let Some(out) = self.csv.as_ref() {
            return export_datasheets(&ctx, &root, &paths, out, &options, self.overwrite);
        }
        let batch = ctx.map_results_compact(
            "datasheet",
            &paths,
            |path| path_label(path),
            |path, progress| progress.step(|| scan_sheet(path, &options)),
        );
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
        let mut report = sheet_summary_report(&scans, self.show);

        if self.columns {
            let mut table = Table::new(["Source", "Index", "Type", "CRC", "Name"]).right([1]);
            for row in scans.iter().flat_map(|scan| scan.columns.clone()) {
                table.push([
                    Cell::path(row.source),
                    Cell::text(row.index),
                    Cell::text(row.kind),
                    Cell::text(row.crc),
                    Cell::text(row.name),
                ]);
            }
            if !table.is_empty() {
                report.table(table);
            }
        }

        if self.rows.is_some() {
            let mut table = Table::new(["Source", "Row", "Values"]).right([1]);
            for row in scans.iter().flat_map(|scan| scan.rows.clone()) {
                table.push([
                    Cell::path(row.source),
                    Cell::text(row.row),
                    Cell::text(row.values),
                ]);
            }
            if !table.is_empty() {
                report.table(table);
            }
        }

        if !options.find.is_empty() {
            let mut hits = scans
                .into_iter()
                .flat_map(|scan| scan.hits)
                .collect::<Vec<_>>();
            // Stable rank by fuzzy score; a no-op in exact mode (all scores 0).
            hits.sort_by_key(|hit| std::cmp::Reverse(hit.score));
            let mut table = Table::new(["Source", "Row", "Column", "Value"]).right([1]);
            for hit in hits {
                table.push([
                    Cell::path(hit.source),
                    Cell::text(hit.row),
                    Cell::text(hit.column),
                    Cell::text(hit.value),
                ]);
            }
            report.table_or(table, "no datasheet matches");
        }
        report.print();

        finish_scan(cancelled, skipped, &errors, "datasheet")
    }
}

impl Dds {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        if let Some(format) = self.to {
            let path = self
                .path
                .as_deref()
                .context("--to needs a DDS file or directory path")?;
            let out = self.out.as_deref().expect("--out is required with --to");
            return match format {
                DdsFormat::Ktx2 => convert_dds_to_ktx2(&ctx, path, out, self.overwrite),
                DdsFormat::Png => write_dds_png(path, out, self.overwrite),
            };
        }
        if self.view {
            let path = self
                .path
                .as_deref()
                .context("--view needs a DDS file path")?;
            return view_dds(path);
        }

        // No path: default to the located install and browse its textures
        // straight out of the pak catalog (the install has no loose DDS files).
        if self.path.is_none() {
            return self.browse_install(&ctx);
        }

        let root = self.path.clone().unwrap_or_else(|| PathBuf::from("."));
        // A directory on a TTY: open the interactive filesystem texture browser.
        if crate::tui::interactive() && root.is_dir() {
            let items = dds_browser_items_fs(&ctx, &root)?;
            if items.is_empty() {
                Report::new("dds")
                    .stat("dir", root.display())
                    .note("no DDS textures found")
                    .print();
                return Ok(());
            }
            let store = Arc::new(crate::tui::TextureStore::Fs);
            let source = root.display().to_string();
            return Ok(crate::tui::dds_browser(items, store, source, ctx.runner.clone())?);
        }

        let paths = collect_matching(&root, |path| nw_dds::is_dds_path(path))?;
        let batch = ctx.map_results_compact(
            "dds",
            &paths,
            |path| path_label(path),
            |path, progress| progress.step(|| scan_dds(path)),
        );
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

        let mut report = Report::new("dds")
            .stat("files", scans.len())
            .stat("shown", scans.len().min(self.show));
        let mut table = Table::new([
            "Source",
            "Kind",
            "Dimensions",
            "Mips",
            "Format",
            "DX10",
            "Cry",
            "Bytes",
        ])
        .right([7]);
        for scan in scans.iter().take(self.show) {
            table.push([
                Cell::path(scan.source.clone()),
                Cell::text(scan.kind.clone()),
                Cell::text(scan.dimensions.clone()),
                Cell::text(scan.mipmaps.clone()),
                Cell::text(scan.format.clone()),
                Cell::text(scan.dx10.clone()),
                Cell::dim(scan.cry.clone()),
                Cell::size(scan.bytes.clone()),
            ]);
        }
        report.table_or(table, "no DDS files to show");
        if scans.len() > self.show {
            report.more(scans.len() - self.show, "file(s)");
        }
        report.print();

        finish_scan(cancelled, skipped, &errors, "dds")
    }

    /// Browse textures straight out of the located install's pak catalog. On a
    /// TTY this opens the interactive browser; piped, it prints a texture listing
    /// so the install default is still useful in scripts.
    fn browse_install(&self, ctx: &RunCtx) -> Result<()> {
        let install = nw_locator::Install::locate().context(
            "no New World install found; pass a path, or run `nw-tools locate` to check detection",
        )?;
        let paks = PakSet::collect(install.assets(), Vec::new())?;
        if paks.paths().is_empty() {
            Report::new("dds")
                .stat("install", install.assets().display())
                .note("no pak archives found in the install")
                .print();
            return Ok(());
        }
        let (items, index) = dds_browser_items_pak(ctx, paks.paths())?;
        if items.is_empty() {
            Report::new("dds")
                .stat("install", install.assets().display())
                .note("no DDS textures found in the install paks")
                .print();
            return Ok(());
        }

        if crate::tui::interactive() {
            let store = Arc::new(crate::tui::TextureStore::Pak(index));
            let source = install.assets().display().to_string();
            return Ok(crate::tui::dds_browser(items, store, source, ctx.runner.clone())?);
        }

        let mut report = Report::new("dds")
            .stat("install", install.assets().display())
            .stat("textures", items.len())
            .stat("shown", items.len().min(self.show));
        let mut table = Table::new(["Texture", "Mips"]).right([1]);
        for item in items.iter().take(self.show) {
            table.push([
                Cell::path(item.label.clone()),
                Cell::text(item.sidecars.len().to_string()),
            ]);
        }
        report.table_or(table, "no DDS textures to show");
        if items.len() > self.show {
            report.more(items.len() - self.show, "texture(s)");
        }
        report.print();
        Ok(())
    }
}

impl ObjectStream {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let lookup = load_lookup(self.no_names)?;
        let selector = PathSelector::new(self.filter, self.glob);
        let root = self.path.clone().unwrap_or_else(|| PathBuf::from("."));
        let paths = objectstream_paths(&root, &self.extensions, &selector)?;
        if let Some(encoding) = self.to {
            return convert_objectstreams(
                &ctx,
                &root,
                &paths,
                self.out.as_deref(),
                encoding.into(),
                self.overwrite,
                lookup.as_ref(),
            );
        }

        if self.dom && self.query.is_none() && crate::tui::interactive() {
            return browse_objectstreams(&paths, lookup.as_ref());
        }

        let mode = if self.dom {
            ObjectMode::Dom { limit: self.show }
        } else {
            ObjectMode::Stats
        };
        let query = self.query.clone();
        let fuzzy = !self.exact;
        let batch = ctx.map_results_compact(
            "objectstream",
            &paths,
            |path| path_label(path),
            |path, progress| {
                progress.step(|| {
                    scan_objectstream(
                        path,
                        mode,
                        query.as_deref(),
                        fuzzy,
                        self.show,
                        lookup.as_ref(),
                    )
                })
            },
        );
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut scans = Vec::new();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(Some(scan)) => scans.push(scan),
                Ok(None) => {}
                Err(error) => errors.push(error),
            }
        }
        scans.sort_by(|left, right| object_source(left).cmp(object_source(right)));

        let shown = scans.len().min(self.files);
        let mut report = Report::new("objectstream")
            .stat("files", scans.len())
            .stat("shown", shown)
            .stat("names", if lookup.is_some() { "loaded" } else { "off" });
        for scan in scans.into_iter().take(self.files) {
            match scan {
                ObjectScan::Stats(scan) => push_object_stats(&mut report, &scan),
                ObjectScan::Dom(scan) => push_object_dom(&mut report, &scan),
                ObjectScan::Search { source, hits } => {
                    push_object_search(&mut report, &source, hits)
                }
            }
        }
        report.print();

        finish_scan(cancelled, skipped, &errors, "objectstream")
    }
}

fn convert_objectstreams(
    ctx: &RunCtx,
    root: &Path,
    paths: &[PathBuf],
    out: Option<&Path>,
    encoding: ObjectStreamEncoding,
    overwrite: bool,
    lookup: Option<&NameLookup>,
) -> Result<()> {
    let batch = ctx.map_results_compact(
        "objectstream conversion",
        paths,
        |path| path_label(path),
        |path, progress| {
            progress.step(|| convert_objectstream(root, path, out, encoding, overwrite, lookup))
        },
    );
    let skipped = batch.skipped();
    let cancelled = batch.was_cancelled();
    let mut converted = Vec::new();
    let mut errors = Vec::new();

    for result in batch.into_completed() {
        match result {
            Ok(Some(row)) => converted.push(row),
            Ok(None) => {}
            Err(error) => errors.push(error),
        }
    }
    converted.sort_by(|left, right| left.source.cmp(&right.source));

    let mut report = Report::new("objectstream conversion")
        .stat("objectstreams", paths.len())
        .stat("converted", converted.len())
        .stat("encoding", encoding);
    let mut table = Table::new(["Source", "Output", "Encoding", "Bytes"]).right([3]);
    for row in converted {
        table.push([
            Cell::path(row.source),
            Cell::path(row.output),
            Cell::text(row.encoding),
            Cell::size(row.bytes),
        ]);
    }
    report.table_or(table, "no ObjectStreams converted");
    report.print();

    finish_scan(cancelled, skipped, &errors, "objectstream conversion")
}

fn convert_objectstream(
    root: &Path,
    path: &Path,
    out: Option<&Path>,
    encoding: ObjectStreamEncoding,
    overwrite: bool,
    lookup: Option<&NameLookup>,
) -> Result<Option<ObjectConvertRow>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let Some(payload) = objectstream_payload(&bytes)
        .with_context(|| format!("decode wrapper for {}", path.display()))?
    else {
        return Ok(None);
    };
    let converted = nw_objectstream::ObjectStream::transcode_bytes(&payload, encoding, lookup)
        .with_context(|| format!("convert {}", path.display()))?;
    let output = objectstream_output_path(root, path, out, encoding);
    if output.exists() && !overwrite {
        bail!(
            "{} exists (pass --overwrite to replace it)",
            output.display()
        );
    }
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(&output, &converted).with_context(|| format!("write {}", output.display()))?;

    Ok(Some(ObjectConvertRow {
        source: path_label(path),
        output: output.display().to_string(),
        encoding: encoding.to_string(),
        bytes: format_size(converted.len(), DECIMAL),
    }))
}

/// Decode a single DDS texture (assembling its split sidecars) to an RGBA image,
/// returning it alongside the header path it came from.
fn decode_dds(path: &Path) -> Result<(PathBuf, image::RgbaImage)> {
    if path.is_dir() {
        bail!("--view and --to expect a single DDS file, not a directory");
    }
    let group = collect_dds_groups(path)?
        .into_iter()
        .next()
        .context("no DDS texture found")?;
    let header_bytes =
        std::fs::read(&group.header).with_context(|| format!("read {}", group.header.display()))?;
    let mut sidecar_bytes = Vec::with_capacity(group.sidecars.len());
    for sidecar in &group.sidecars {
        let part = split_part_for_path(sidecar)?;
        let bytes =
            std::fs::read(sidecar).with_context(|| format!("read {}", sidecar.display()))?;
        sidecar_bytes.push((part, bytes));
    }
    let sidecars = sidecar_bytes
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes.as_slice()))
        .collect::<Vec<_>>();
    let decoded = nw_dds::decode_top_mip(&header_bytes, &sidecars)
        .with_context(|| format!("decode {}", group.header.display()))?;
    let image = image::RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba)
        .context("decoded texture had an unexpected size")?;
    Ok((group.header, image))
}

/// Show a DDS texture inline via the kitty graphics protocol.
fn view_dds(path: &Path) -> Result<()> {
    let (header, image) = decode_dds(path)?;
    if crate::ui::theme::caps().graphics {
        let shown = fit_image(&image, 720);
        crate::ui::image::print_kitty_rgba(shown.as_raw(), shown.width(), shown.height());
    } else {
        Report::new("dds view")
            .stat("source", path_label(&header))
            .stat("size", format!("{}x{}", image.width(), image.height()))
            .note("this terminal has no inline-image support (kitty graphics protocol)")
            .note("write a file with --to png --out <path> instead")
            .print();
    }
    Ok(())
}

/// Decode a DDS texture and write it as a PNG.
fn write_dds_png(path: &Path, out: &Path, overwrite: bool) -> Result<()> {
    let (header, image) = decode_dds(path)?;
    if out.exists() && !overwrite {
        bail!("{} exists (pass --overwrite to replace it)", out.display());
    }
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    image
        .save(out)
        .with_context(|| format!("write {}", out.display()))?;
    Report::new("dds png")
        .stat("source", path_label(&header))
        .stat("size", format!("{}x{}", image.width(), image.height()))
        .stat("png", out.display())
        .print();
    Ok(())
}

/// Downscale an image so its largest side is at most `max`, preserving aspect.
fn fit_image(image: &image::RgbaImage, max: u32) -> image::RgbaImage {
    let (width, height) = (image.width(), image.height());
    if width <= max && height <= max {
        return image.clone();
    }
    let scale = f64::from(max) / f64::from(width.max(height));
    let target_w = ((f64::from(width) * scale) as u32).max(1);
    let target_h = ((f64::from(height) * scale) as u32).max(1);
    image::imageops::resize(image, target_w, target_h, image::imageops::FilterType::Triangle)
}

fn convert_dds_to_ktx2(ctx: &RunCtx, path: &Path, out: &Path, overwrite: bool) -> Result<()> {
    let groups = collect_dds_groups(path)?;
    let batch = ctx.map_results_compact(
        "dds conversion",
        &groups,
        |group| path_label(&group.header),
        |group, progress| progress.step(|| convert_dds_group(group, path, out, overwrite)),
    );
    let skipped = batch.skipped();
    let cancelled = batch.was_cancelled();
    let mut converted = Vec::new();
    let mut errors = Vec::new();

    for result in batch.into_completed() {
        match result {
            Ok(row) => converted.push(row),
            Err(error) => errors.push(error),
        }
    }
    converted.sort_by(|left, right| left.source.cmp(&right.source));

    let mut report = Report::new("dds conversion")
        .stat("textures", groups.len())
        .stat("converted", converted.len());
    let mut table = Table::new(["Source", "Output", "Bytes"]).right([2]);
    for row in converted {
        table.push([
            Cell::path(row.source),
            Cell::path(row.output),
            Cell::size(row.bytes),
        ]);
    }
    report.table_or(table, "no DDS textures converted");
    report.print();

    finish_scan(cancelled, skipped, &errors, "dds conversion")
}

/// Discover logical textures under a filesystem directory for the browser: every
/// DDS header with its ordered split-mip sidecars. Path classification fans out
/// across the job pool; the directory walk and grouping stay on the caller.
fn dds_browser_items_fs(ctx: &RunCtx, root: &Path) -> Result<Vec<crate::tui::DdsItem>> {
    let paths = collect_matching(root, |path| nw_dds::is_dds_path(path))?;
    let classified = ctx
        .runner
        .try_map(&paths, |path| -> Result<(nw_dds::SplitPart, String)> {
            Ok((split_part_for_path(path)?, path.to_string_lossy().into_owned()))
        })?;
    let mut items = group_dds_items(classified, |header| relative_label(root, header))?;
    items.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(items)
}

/// Discover logical textures straight out of the install's pak archives. Each pak
/// is opened and enumerated on a worker thread; DDS entries are classified into
/// logical textures, and a path → (reader, entry) index is built from the readers
/// discovery already opened so the browser's lazy reads are O(1) (no re-parsing,
/// no catalog — the install ships none).
fn dds_browser_items_pak(
    ctx: &RunCtx,
    pak_paths: &[PathBuf],
) -> Result<(Vec<crate::tui::DdsItem>, crate::tui::PakIndex)> {
    let per_pak = ctx.runner.map(pak_paths, |path| {
        let mut found = Vec::new();
        if let Ok(reader) = PakMmapReader::open(path) {
            let reader = Arc::new(reader);
            for entry in reader.entries() {
                let name = entry.name();
                if nw_dds::is_dds_path(name) {
                    found.push((name.to_ascii_lowercase(), entry.index(), reader.clone()));
                }
            }
        }
        found
    });

    let mut index = crate::tui::PakIndex::new();
    let mut classified = Vec::new();
    for list in per_pak {
        for (name, entry, reader) in list {
            if let Some(part) = nw_dds::SplitPart::from_path(&name) {
                classified.push((part, name.clone()));
            }
            index.insert(name, (reader, entry));
        }
    }

    let mut items = group_dds_items(classified, |header| header.to_string())?;
    items.sort_by(|left, right| left.label.cmp(&right.label));
    Ok((items, index))
}

/// Group classified `(part, key)` pairs into one [`crate::tui::DdsItem`] per
/// header, attaching its mip sidecars in load order. `label` derives the display
/// label from the resolved header key.
#[derive(Default)]
struct DdsGroupBuild {
    base_header: Option<String>,
    base_mips: Vec<(nw_dds::SplitPart, String)>,
    alpha_header: Option<String>,
    alpha_mips: Vec<(nw_dds::SplitPart, String)>,
}

fn group_dds_items(
    classified: Vec<(nw_dds::SplitPart, String)>,
    label: impl Fn(&str) -> String,
) -> Result<Vec<crate::tui::DdsItem>> {
    // Group every part — base header/mips and the attached-alpha header/mips —
    // under the one base key (`foo.dds`), so the alpha surface stays with its base.
    let mut groups = BTreeMap::<String, DdsGroupBuild>::new();
    for (part, key) in classified {
        let base = dds_base_key(&key, part)?;
        let group = groups.entry(base).or_default();
        match part {
            nw_dds::SplitPart::Header => group.base_header = Some(key),
            nw_dds::SplitPart::AlphaHeader => group.alpha_header = Some(key),
            nw_dds::SplitPart::Mip { alpha: false, .. } => group.base_mips.push((part, key)),
            nw_dds::SplitPart::Mip { alpha: true, .. } => group.alpha_mips.push((part, key)),
        }
    }

    let by_index =
        |(part, _): &(nw_dds::SplitPart, String)| part.mip_index().unwrap_or(0);
    Ok(groups
        .into_iter()
        .map(|(base, mut group)| {
            group.base_mips.sort_by_key(by_index);
            group.alpha_mips.sort_by_key(by_index);
            let alpha = (!group.alpha_mips.is_empty() || group.alpha_header.is_some()).then(|| {
                crate::tui::AlphaSurface {
                    header: group.alpha_header.unwrap_or_else(|| format!("{base}.a")),
                    sidecars: group.alpha_mips,
                }
            });
            crate::tui::DdsItem {
                label: label(&base),
                header: group.base_header.unwrap_or_else(|| base.clone()),
                sidecars: group.base_mips,
                alpha,
            }
        })
        .collect())
}

/// Resolve the base texture key (`foo.dds`) for any DDS part — base or attached
/// alpha. Works on both filesystem and pak virtual paths (string, separator-safe).
fn dds_base_key(key: &str, part: nw_dds::SplitPart) -> Result<String> {
    match part {
        nw_dds::SplitPart::Header => Ok(key.to_string()),
        nw_dds::SplitPart::AlphaHeader => Ok(key
            .strip_suffix(".a")
            .or_else(|| key.strip_suffix(".A"))
            .unwrap_or(key)
            .to_string()),
        nw_dds::SplitPart::Mip { .. } => {
            let lower = key.to_ascii_lowercase();
            let pos = lower
                .rfind(".dds.")
                .with_context(|| format!("invalid DDS sidecar path: {key}"))?;
            Ok(key[..pos + ".dds".len()].to_string())
        }
    }
}

/// A texture's path relative to the browse root, with forward slashes.
fn relative_label(root: &Path, path: &str) -> String {
    Path::new(path)
        .strip_prefix(root)
        .map(|rel| rel.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
        .replace('\\', "/")
}

fn collect_dds_groups(path: &Path) -> Result<Vec<DdsGroup>> {
    let paths = collect_dds_inputs(path)?;
    let mut groups = BTreeMap::<PathBuf, DdsGroup>::new();
    for dds_path in paths {
        let part = split_part_for_path(&dds_path)?;
        let header = dds_header_path(&dds_path, part)?;
        let group = groups.entry(header.clone()).or_insert_with(|| DdsGroup {
            header,
            sidecars: Vec::new(),
        });
        if matches!(part, nw_dds::SplitPart::Mip { .. }) {
            group.sidecars.push(dds_path);
        }
    }

    let mut groups = groups.into_values().collect::<Vec<_>>();
    for group in &mut groups {
        group.sidecars.sort();
    }
    groups.sort_by(|left, right| left.header.cmp(&right.header));
    Ok(groups)
}

fn collect_dds_inputs(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.is_file() {
        return collect_matching(path, |path| nw_dds::is_dds_path(path));
    }

    let part = split_part_for_path(path)?;
    let header = dds_header_path(path, part)?;
    let parent = header.parent().filter(|path| !path.as_os_str().is_empty());
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(parent.unwrap_or_else(|| Path::new(".")))
        .with_context(|| format!("scan DDS sidecars for {}", header.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_file() || !nw_dds::is_dds_path(entry.path()) {
            continue;
        }
        let candidate = entry.path();
        let candidate_part = split_part_for_path(&candidate)?;
        if dds_header_path(&candidate, candidate_part)? == header {
            paths.push(candidate);
        }
    }
    paths.sort();
    Ok(paths)
}

fn convert_dds_group(
    group: &DdsGroup,
    input: &Path,
    out: &Path,
    overwrite: bool,
) -> Result<DdsConvert> {
    let header_bytes =
        std::fs::read(&group.header).with_context(|| format!("read {}", group.header.display()))?;
    let mut sidecar_bytes = Vec::with_capacity(group.sidecars.len());
    for path in &group.sidecars {
        let part = split_part_for_path(path)?;
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        sidecar_bytes.push((part, bytes));
    }
    sidecar_bytes.sort_by_key(|(part, _)| match part {
        nw_dds::SplitPart::Header | nw_dds::SplitPart::AlphaHeader => (0u8, 0u32),
        nw_dds::SplitPart::Mip { index, alpha } => (u8::from(*alpha), *index),
    });
    let sidecars = sidecar_bytes
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes.as_slice()))
        .collect::<Vec<_>>();
    let ktx = nw_dds::Ktx2::from_dds(&header_bytes, &sidecars)
        .with_context(|| format!("convert {}", group.header.display()))?;
    let output = dds_ktx2_output_path(input, &group.header, out)?;
    if output.exists() && !overwrite {
        bail!(
            "{} exists; pass --overwrite to replace it",
            output.display()
        );
    }
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(&output, ktx.bytes()).with_context(|| format!("write {}", output.display()))?;

    Ok(DdsConvert {
        source: group.header.display().to_string(),
        output: output.display().to_string(),
        bytes: format_size(ktx.bytes().len(), DECIMAL),
    })
}

fn split_part_for_path(path: &Path) -> Result<nw_dds::SplitPart> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("DDS path is not UTF-8: {}", path.display()))?;
    nw_dds::SplitPart::from_path(name)
        .with_context(|| format!("not a DDS texture path: {}", path.display()))
}

fn dds_header_path(path: &Path, part: nw_dds::SplitPart) -> Result<PathBuf> {
    match part {
        nw_dds::SplitPart::Header | nw_dds::SplitPart::AlphaHeader => Ok(path.to_path_buf()),
        nw_dds::SplitPart::Mip { alpha, .. } => {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .with_context(|| format!("DDS path is not UTF-8: {}", path.display()))?;
            let lower = name.to_ascii_lowercase();
            let dds_pos = lower
                .rfind(".dds.")
                .with_context(|| format!("invalid DDS sidecar path: {}", path.display()))?;
            let mut header_name = name[..dds_pos + ".dds".len()].to_string();
            if alpha {
                header_name.push_str(".a");
            }
            Ok(path.with_file_name(header_name))
        }
    }
}

fn dds_ktx2_output_path(input: &Path, header: &Path, out: &Path) -> Result<PathBuf> {
    let exact_file = input.is_file()
        && out
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ktx2"));
    if exact_file {
        return Ok(out.to_path_buf());
    }

    let relative = if input.is_dir() {
        header.strip_prefix(input).unwrap_or(header).to_path_buf()
    } else {
        PathBuf::from(
            header
                .file_name()
                .with_context(|| format!("DDS header has no file name: {}", header.display()))?,
        )
    };
    let mut output = out.join(relative);
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("DDS output path is not UTF-8: {}", output.display()))?;
    output.set_file_name(dds_ktx2_file_name(file_name));
    Ok(output)
}

fn dds_ktx2_file_name(name: &str) -> String {
    if let Some(stem) = strip_suffix_ignore_ascii_case(name, ".dds.a") {
        format!("{stem}.a.ktx2")
    } else if let Some(stem) = strip_suffix_ignore_ascii_case(name, ".dds") {
        format!("{stem}.ktx2")
    } else {
        format!("{name}.ktx2")
    }
}

fn strip_suffix_ignore_ascii_case<'a>(value: &'a str, suffix: &str) -> Option<&'a str> {
    let split = value.len().checked_sub(suffix.len())?;
    value
        .get(split..)
        .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
        .then(|| &value[..split])
}

fn scan_dds(path: &Path) -> Result<DdsScan> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let source = path.display().to_string();
    let asset = nw_dds::Asset::parse(&source, &bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(match asset.kind() {
        nw_dds::AssetKind::Header(dds) => {
            let dx10 = dds.dx10().map_or_else(
                || "-".to_string(),
                |header| header.dxgi_format().to_string(),
            );
            DdsScan {
                source,
                kind: if dds.is_cry_extended() {
                    "DDS/Cry".to_string()
                } else {
                    "DDS".to_string()
                },
                dimensions: format!("{}x{}x{}", dds.width(), dds.height(), dds.depth().max(1)),
                mipmaps: format!(
                    "{} (persistent {})",
                    dds.mipmaps(),
                    dds.header().persistent_mips()
                ),
                format: dds.format_name(),
                dx10,
                cry: format!(
                    "flags=0x{:08x} split={} alpha={}",
                    dds.header().cry_flags().bits(),
                    dds.is_split(),
                    dds.has_attached_alpha()
                ),
                bytes: format_size(bytes.len(), DECIMAL),
            }
        }
        nw_dds::AssetKind::Split(payload) => {
            let part = payload.part();
            let mipmaps = part.mip_index().map_or_else(
                || "-".to_string(),
                |index| format!("{index} alpha={}", part.is_alpha()),
            );
            DdsScan {
                source,
                kind: part.to_string(),
                dimensions: "-".to_string(),
                mipmaps,
                format: "-".to_string(),
                dx10: "-".to_string(),
                cry: "-".to_string(),
                bytes: format_size(payload.bytes().len(), DECIMAL),
            }
        }
    })
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
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
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

fn csv_cell(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn export_datasheets(
    ctx: &RunCtx,
    root: &Path,
    paths: &[PathBuf],
    out: &Path,
    options: &SheetOptions<'_>,
    overwrite: bool,
) -> Result<()> {
    if paths.len() > 1 && out.extension().is_some() {
        bail!("CSV output must be a directory when exporting more than one datasheet");
    }

    let batch = ctx.map_results_compact(
        "datasheet export",
        paths,
        |path| path_label(path),
        |path, progress| progress.step(|| export_datasheet(root, path, out, options, overwrite)),
    );
    let skipped = batch.skipped();
    let cancelled = batch.was_cancelled();
    let mut exported = Vec::new();
    let mut errors = Vec::new();

    for result in batch.into_completed() {
        match result {
            Ok(row) => exported.push(row),
            Err(error) => errors.push(error),
        }
    }
    exported.sort_by(|left, right| left.source.cmp(&right.source));

    let mut report = Report::new("datasheet export")
        .stat("datasheets", paths.len())
        .stat("exported", exported.len());
    let mut table = Table::new(["Source", "Output", "Rows"]).right([2]);
    for row in exported {
        table.push([
            Cell::path(row.source),
            Cell::path(row.output),
            Cell::text(row.rows),
        ]);
    }
    report.table_or(table, "no datasheets exported");
    report.print();

    finish_scan(cancelled, skipped, &errors, "datasheet export")
}

fn export_datasheet(
    root: &Path,
    path: &Path,
    out: &Path,
    options: &SheetOptions<'_>,
    overwrite: bool,
) -> Result<SheetExport> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut sheet = nw_datasheet::Datasheet::parse(&bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    if let Some(localization) = options.localization {
        sheet.set_localization(Some(localization));
    }

    let output = datasheet_csv_output_path(root, path, out);
    if output.exists() && !overwrite {
        bail!(
            "{} exists (pass --overwrite to replace it)",
            output.display()
        );
    }
    write_datasheet_csv(&output, &sheet, options)
        .with_context(|| format!("write {}", output.display()))?;

    Ok(SheetExport {
        source: path_label(path),
        output: output.display().to_string(),
        rows: sheet.len().to_string(),
    })
}

fn write_datasheet_csv(
    path: &Path,
    sheet: &nw_datasheet::Datasheet<'_>,
    options: &SheetOptions<'_>,
) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    let mut csv = String::new();
    for (index, column) in sheet.columns().iter().enumerate() {
        if index > 0 {
            csv.push(',');
        }
        csv.push_str(&csv_cell(column.name()));
    }
    csv.push('\n');

    for row in sheet.rows() {
        for (index, cell) in row.cells().iter().enumerate() {
            if index > 0 {
                csv.push(',');
            }
            csv.push_str(&csv_cell(&cell_text(sheet, cell, options)));
        }
        csv.push('\n');
    }

    std::fs::write(path, csv)?;
    Ok(())
}

fn datasheet_csv_output_path(root: &Path, source: &Path, out: &Path) -> PathBuf {
    if root.is_file() && out.extension().is_some() {
        return out.to_path_buf();
    }

    let relative = if root.is_file() {
        source
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| source.to_path_buf())
    } else {
        source.strip_prefix(root).unwrap_or(source).to_path_buf()
    };
    let mut output = out.join(relative);
    output.set_extension("csv");
    output
}

fn collect_datasheet_localization_keys(
    paths: &[PathBuf],
    row_limit: Option<usize>,
) -> Result<BTreeSet<LocalizationKey>> {
    let mut keys = BTreeSet::new();
    for path in paths {
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let sheet = nw_datasheet::Datasheet::parse(&bytes)
            .with_context(|| format!("parse {}", path.display()))?;
        for row in sheet.rows().take(row_limit.unwrap_or(usize::MAX)) {
            for cell in row.cells() {
                if let Some(value) = cell.as_str() {
                    keys.extend(localization_keys(value));
                }
            }
        }
    }
    Ok(keys)
}

fn scan_sheet(path: &Path, options: &SheetOptions) -> Result<SheetScan> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut sheet = nw_datasheet::Datasheet::parse(&bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    if let Some(localization) = options.localization {
        sheet.set_localization(Some(localization));
    }
    let summary = SheetSummary::from(sheet.summary());
    let source = path.display().to_string();
    let columns = if options.columns {
        sheet
            .columns()
            .iter()
            .enumerate()
            .map(|(index, column)| ColumnRow {
                source: source.clone(),
                index: index.to_string(),
                kind: column.column_type().to_string(),
                crc: format!("0x{:08x}", column.crc()),
                name: column.name().to_string(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let rows = options
        .rows
        .map(|limit| {
            sheet
                .rows()
                .enumerate()
                .take(limit)
                .map(|(index, row)| RowSample {
                    source: source.clone(),
                    row: index.to_string(),
                    values: row_values(&sheet, &row, options),
                })
                .collect()
        })
        .unwrap_or_default();
    let hits = if options.find.is_empty() {
        Vec::new()
    } else {
        find_sheet_cells(&source, &sheet, options)
    };

    Ok(SheetScan {
        source,
        summary,
        columns,
        rows,
        hits,
    })
}

fn scan_objectstream(
    path: &Path,
    mode: ObjectMode,
    query: Option<&str>,
    fuzzy: bool,
    limit: usize,
    lookup: Option<&NameLookup>,
) -> Result<Option<ObjectScan>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let Some(bytes) = objectstream_payload(&bytes)
        .with_context(|| format!("decode wrapper for {}", path.display()))?
    else {
        return Ok(None);
    };
    let source = path.display().to_string();
    if let Some(query) = query {
        let needle = query.to_ascii_lowercase();
        let mut search = fuzzy.then(|| crate::fuzzy::Search::new(query));
        let hits =
            nw_objectstream::query::collect_search_matches(
                &bytes,
                lookup,
                |value| match &mut search {
                    Some(search) => search.score(value).map(u32::from),
                    None => value.to_ascii_lowercase().contains(&needle).then_some(1),
                },
            )
            .with_context(|| format!("search {}", path.display()))?;
        let mut hits = hits
            .into_iter()
            .map(|(hit, stats)| ObjectHit {
                kind: hit.kind.label().to_string(),
                count: stats.count,
                score: stats.score,
                value: trim_cell(hit.value),
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then(right.count.cmp(&left.count))
                .then(left.kind.cmp(&right.kind))
                .then(left.value.cmp(&right.value))
        });
        hits.truncate(limit);
        return Ok(Some(ObjectScan::Search { source, hits }));
    }

    match mode {
        ObjectMode::Stats => {
            let stats = nw_objectstream::stats::Stats::from_bytes(&bytes, lookup)
                .with_context(|| format!("inspect {}", path.display()))?;
            Ok(Some(ObjectScan::Stats(ObjectStatsScan {
                source,
                stats,
                names_loaded: lookup.is_some(),
            })))
        }
        ObjectMode::Dom { limit } => {
            let stream = nw_objectstream::ObjectStream::from_bytes(&bytes, lookup)
                .with_context(|| format!("parse {}", path.display()))?;
            Ok(Some(ObjectScan::Dom(object_dom_scan(
                source, &stream, limit,
            ))))
        }
    }
}

fn browse_objectstreams(paths: &[PathBuf], lookup: Option<&NameLookup>) -> Result<()> {
    if paths.is_empty() {
        Report::new("objectstream").stat("files", 0usize).print();
        return Ok(());
    }
    if paths.len() == 1 {
        return open_objectstream_tree(&paths[0], lookup);
    }

    loop {
        let mut table = Table::new(["ObjectStream", "Bytes"]).right([1]);
        for path in paths {
            let size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
            table.push([
                Cell::path(path_label(path)),
                Cell::size(format_size(size, DECIMAL)),
            ]);
        }
        let stats = vec![("objectstreams".to_string(), paths.len().to_string())];
        match crate::tui::pick("objectstreams", stats, table, 0)? {
            Some(selection) if !selection.is_empty() => {
                open_objectstream_tree(Path::new(&selection), lookup)?;
            }
            _ => return Ok(()),
        }
    }
}

fn open_objectstream_tree(path: &Path, lookup: Option<&NameLookup>) -> Result<()> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let Some(bytes) = objectstream_payload(&bytes)
        .with_context(|| format!("decode wrapper for {}", path.display()))?
    else {
        bail!("{} is not an ObjectStream payload", path.display());
    };
    let stream = nw_objectstream::ObjectStream::from_bytes(&bytes, lookup)
        .with_context(|| format!("parse {}", path.display()))?;
    let mut nodes = Vec::new();
    collect_tree_nodes(stream.elements(), 0, &mut nodes);
    crate::tui::tree(path.display().to_string(), nodes)?;
    Ok(())
}

fn collect_tree_nodes(
    elements: &[nw_objectstream::Element],
    depth: usize,
    nodes: &mut Vec<crate::tui::TreeNode>,
) {
    for element in elements {
        let unresolved_type = element.name().is_empty();
        let type_name = if unresolved_type {
            "<unknown-type>".to_string()
        } else {
            element.name().to_string()
        };
        let (field, unresolved_field) = match element.field() {
            Some(name) => (name.to_string(), false),
            None => match element.name_crc() {
                Some(crc) => (format!("#{crc:08x}"), true),
                None => (String::new(), false),
            },
        };
        let meta = format!("id {} flags {:#04x}", element.id(), element.flags);
        let children = element.children();
        nodes.push(crate::tui::TreeNode {
            depth,
            children: children.len(),
            type_name,
            field,
            meta,
            unresolved_type,
            unresolved_field,
        });
        collect_tree_nodes(children, depth + 1, nodes);
    }
}

fn object_dom_scan(
    source: String,
    stream: &nw_objectstream::ObjectStream,
    limit: usize,
) -> ObjectDomScan {
    let mut rows = Vec::new();
    let mut total_elements = 0usize;
    for (index, element) in stream.iter_recursive().enumerate() {
        total_elements += 1;
        if rows.len() < limit {
            let type_name = if element.name().is_empty() {
                "<unknown-type>".to_string()
            } else {
                element.name().to_string()
            };
            rows.push(ObjectDomRow {
                index: index.to_string(),
                flags: format!("{:#04x}", element.flags),
                id: element.id().to_string(),
                type_name,
                field: element
                    .field()
                    .map_or_else(String::new, ToString::to_string),
            });
        }
    }

    ObjectDomScan {
        source,
        version: stream.version(),
        top_level_elements: stream.elements().len(),
        total_elements,
        rows,
    }
}

fn push_object_stats(report: &mut Report, scan: &ObjectStatsScan) {
    let stats = scan.stats;
    report.section(format!("{} ({})", scan.source, stats.mode_label()));
    report.kv("version", stats.version.to_string());
    report.kv("elements", stats.elements.to_string());
    report.kv("max depth", stats.max_depth.to_string());
    report.kv("bytes", stats.bytes.to_string());
    if scan.names_loaded {
        report.kv(
            "resolved",
            format!(
                "{} elements had a known type, {} fields had a known name",
                stats.resolved_types, stats.resolved_fields
            ),
        );
    } else {
        report.kv("resolved", "(no serialize.json - names unresolved)");
    }
}

fn push_object_dom(report: &mut Report, scan: &ObjectDomScan) {
    report.section(format!("{} (DOM)", scan.source));
    report.kv("version", scan.version.to_string());
    report.kv("top-level elements", scan.top_level_elements.to_string());
    if !scan.rows.is_empty() {
        let mut table = Table::new(["Index", "Flags", "Id", "Type", "Field"]).right([0]);
        for row in &scan.rows {
            table.push([
                Cell::text(row.index.clone()),
                Cell::dim(row.flags.clone()),
                Cell::text(row.id.clone()),
                Cell::text(row.type_name.clone()),
                Cell::text(row.field.clone()),
            ]);
        }
        report.table(table);
    }
    let remaining = scan.total_elements.saturating_sub(scan.rows.len());
    if remaining > 0 {
        report.more(remaining, "element(s)");
    }
}

fn push_object_search(report: &mut Report, source: &str, hits: Vec<ObjectHit>) {
    report.section(format!("{source}: {} hit group(s)", hits.len()));
    let mut table = Table::new(["Kind", "Count", "Score", "Value"]).right([1, 2]);
    for hit in hits {
        table.push([
            Cell::text(hit.kind),
            Cell::text(hit.count.to_string()),
            Cell::text(hit.score.to_string()),
            Cell::text(hit.value),
        ]);
    }
    report.table_or(table, "no ObjectStream matches");
}

/// The active localization language and last error.
struct LocaleSlot {
    code: Option<String>,
    error: Option<String>,
}

/// A language's growing catalog: files are added on demand (per sheet), so it only
/// ever holds the entries the user has actually looked at.
struct LangState {
    builder: LocalizationCatalogBuilder,
    catalog: Arc<LocalizationCatalog>,
    loaded: HashSet<String>,
}

impl LangState {
    fn new(language: LanguageCode) -> Self {
        let builder = LocalizationCatalog::builder(language);
        let catalog = Arc::new(builder.clone().build());
        Self {
            builder,
            catalog,
            loaded: HashSet::new(),
        }
    }
}

/// Where one sheet's bytes come from: a loose file on disk, or an entry inside a
/// memory-mapped pak archive.
#[derive(Clone)]
enum SheetSrc {
    File(PathBuf),
    Pak {
        reader: Arc<PakMmapReader>,
        index: usize,
    },
}

/// The growing set of discovered sheets. Append-only so sheet ids stay stable
/// while the background discovery sweep fills it in.
#[derive(Default)]
struct Discovered {
    sources: Vec<SheetSrc>,
    labels: Vec<String>,
    sizes: Vec<u64>,
}

/// A workspace of datasheets browsed together — either loose files or the
/// datasheet entries streamed straight out of the game's pak archives. Pak
/// discovery and the cross-reference index both run on a background thread pool
/// so the picker is usable from the first frame and fills in live; queries
/// return whatever is ready and grow over time. Localization is loaded lazily so
/// the language can be switched from inside the TUI without blocking startup.
struct DatasheetWorkspace {
    me: Weak<DatasheetWorkspace>,
    sheets: Mutex<Discovered>,
    /// Shared worker pool (honors the `--jobs` setting) for discovery, indexing,
    /// and parallel localization loads.
    runner: nw_jobs::JobRunner,
    /// Pak files to scan for datasheets (empty in file mode).
    pak_paths: Vec<PathBuf>,
    /// Paks scanned so far, and the total to scan.
    scanned: AtomicUsize,
    discover_total: usize,
    discover_done: AtomicBool,
    index: Mutex<HashMap<String, Vec<crate::tui::Loc>>>,
    indexed: AtomicUsize,
    index_done: AtomicBool,
    cancel: AtomicBool,
    // Localization, loaded and swapped on demand from a background thread.
    asset_root: Option<PathBuf>,
    /// Available language codes, enumerated lazily from the install on first use.
    languages: Mutex<Option<Vec<String>>>,
    locale: Mutex<LocaleSlot>,
    /// Bumped whenever resolved localization changes; the viewer watches this.
    loc_gen: AtomicU64,
    /// True while a background localization load is in flight (drives the
    /// "loading" indicator and serializes ensures).
    loc_ensuring: AtomicBool,
    /// Growing catalogs per language (only the touched files' entries).
    langs: Mutex<HashMap<String, LangState>>,
    /// Key → file index (built once, persisted), enabling per-sheet targeted loads.
    key_index: Mutex<Option<Arc<KeyFileIndex>>>,
    /// All localization source file names, cached for the no-index fallback.
    source_names: Mutex<Option<Vec<String>>>,
    /// Localization files found during discovery: virtual path → owning pak
    /// reader. Seeds the asset store so locale loads resolve in O(1) and reuse
    /// the paks discovery already opened (no re-parsing). Only localization paks
    /// are retained here — a handful — so the memory cost is small.
    loc_index: Mutex<HashMap<String, Arc<PakMmapReader>>>,
}

impl DatasheetWorkspace {
    fn new(
        sheets: Discovered,
        pak_paths: Vec<PathBuf>,
        discover_done: bool,
        asset_root: Option<PathBuf>,
        jobs: Option<usize>,
    ) -> Arc<Self> {
        let discover_total = pak_paths.len();
        let runner =
            nw_jobs::JobRunner::from_jobs(jobs).unwrap_or_else(|_| nw_jobs::JobRunner::automatic());
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            sheets: Mutex::new(sheets),
            runner,
            pak_paths,
            scanned: AtomicUsize::new(0),
            discover_total,
            discover_done: AtomicBool::new(discover_done),
            index: Mutex::new(HashMap::new()),
            indexed: AtomicUsize::new(0),
            index_done: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            asset_root,
            languages: Mutex::new(None),
            locale: Mutex::new(LocaleSlot {
                code: None,
                error: None,
            }),
            loc_gen: AtomicU64::new(0),
            loc_ensuring: AtomicBool::new(false),
            langs: Mutex::new(HashMap::new()),
            key_index: Mutex::new(None),
            source_names: Mutex::new(None),
            loc_index: Mutex::new(HashMap::new()),
        })
    }

    /// Build a workspace over loose `.datasheet` files on disk (discovery is
    /// immediate — the list is known up front).
    fn from_files(
        paths: Vec<PathBuf>,
        asset_root: Option<PathBuf>,
        jobs: Option<usize>,
    ) -> Arc<Self> {
        let labels = paths.iter().map(|path| path_label(path)).collect();
        let sizes = paths
            .iter()
            .map(|path| std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0))
            .collect();
        let sources = paths.into_iter().map(SheetSrc::File).collect();
        let sheets = Discovered {
            sources,
            labels,
            sizes,
        };
        Self::new(sheets, Vec::new(), true, asset_root, jobs)
    }

    /// Build a workspace that streams datasheets out of `pak_paths`. Returns
    /// instantly; the paks are opened and scanned on the background pool.
    fn from_paks(
        pak_paths: Vec<PathBuf>,
        asset_root: Option<PathBuf>,
        jobs: Option<usize>,
    ) -> Arc<Self> {
        Self::new(Discovered::default(), pak_paths, false, asset_root, jobs)
    }

    fn sheet_lock(&self) -> std::sync::MutexGuard<'_, Discovered> {
        self.sheets.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn len(&self) -> usize {
        self.sheet_lock().sources.len()
    }

    /// Available language codes known so far (non-blocking). Empty until the
    /// background sweep has read the install's language manifest.
    fn cached_languages(&self) -> Vec<String> {
        self.languages
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .unwrap_or_default()
    }

    fn languages_known(&self) -> bool {
        self.languages
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
    }

    /// Store the enumerated language codes (first writer wins).
    fn set_languages(&self, manifest: &LanguageManifest) {
        let codes = manifest
            .languages()
            .iter()
            .map(|entry| entry.code().as_str().to_string())
            .collect::<Vec<_>>();
        let mut slot = self
            .languages
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if slot.is_none() {
            *slot = Some(codes);
        }
    }

    /// An asset store seeded with the localization paks discovery already opened,
    /// so locale reads resolve in O(1) without re-parsing any pak.
    fn asset_store(&self) -> Option<nw_asset::AssetStore> {
        let root = self.asset_root.as_ref()?;
        let store = nw_asset::AssetStore::open(root).ok()?;
        let index = self
            .loc_index
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        store.seed_paths(index.iter().map(|(path, reader)| (path.clone(), reader.clone())));
        Some(store)
    }

    /// Enumerate languages through the asset store. Used in file mode (no
    /// discovery sweep), off the UI thread.
    fn warm_languages(&self) {
        if self.languages_known() {
            return;
        }
        if let Some(assets) = self.asset_store()
            && let Ok(Some(bytes)) = assets.read_path(LANGUAGE_MANIFEST_ASSET_PATH)
            && let Ok(manifest) = LanguageManifest::parse_bytes(&bytes)
        {
            self.set_languages(&manifest);
        }
    }

    /// Capture languages from an already-open pak if it holds the manifest — free
    /// during discovery, so the language list appears within seconds.
    fn capture_languages(&self, reader: &PakMmapReader) {
        if self.languages_known() {
            return;
        }
        if let Some(entry) = reader.entry(LANGUAGE_MANIFEST_ASSET_PATH)
            && let Ok(bytes) = reader.read_by_index(entry.index())
            && let Ok(manifest) = LanguageManifest::parse_bytes(&bytes)
        {
            self.set_languages(&manifest);
        }
    }

    /// Read one sheet's raw datasheet bytes, from disk or straight out of its pak.
    fn read_bytes(&self, sheet: usize) -> Option<Vec<u8>> {
        let source = self.sheet_lock().sources.get(sheet)?.clone();
        match source {
            SheetSrc::File(path) => std::fs::read(path).ok(),
            SheetSrc::Pak { reader, index } => reader.read_by_index(index).ok(),
        }
    }

    fn lock_locale(&self) -> std::sync::MutexGuard<'_, LocaleSlot> {
        self.locale
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn active_code(&self) -> Option<String> {
        self.lock_locale().code.clone()
    }

    /// The current (growing) catalog for `code` — covers whatever files have been
    /// loaded so far for it.
    fn lang_catalog(&self, code: &str) -> Option<Arc<LocalizationCatalog>> {
        self.langs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(code)
            .map(|state| state.catalog.clone())
    }

    /// Every localization source file name for the install (cached). The no-index
    /// fallback loads all of these.
    fn all_source_names(&self) -> Vec<String> {
        if let Some(names) = self
            .source_names
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
        {
            return names;
        }
        let names = self
            .asset_store()
            .and_then(|assets| {
                LocalizationLoader::new(&assets, default_language())
                    .source_file_names()
                    .ok()
            })
            .unwrap_or_default();
        *self
            .source_names
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(names.clone());
        names
    }

    /// Source files still to load to resolve `keys` for `code` — targeted via the
    /// index when ready, else the whole language.
    fn needed_files(&self, code: &str, keys: &[LocalizationKey]) -> Vec<String> {
        let loaded = self
            .langs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(code)
            .map(|state| state.loaded.clone())
            .unwrap_or_default();
        let wanted: HashSet<String> = match self
            .key_index
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
        {
            Some(index) => keys
                .iter()
                .filter_map(|key| index.file_name(key).map(str::to_string))
                .collect(),
            None => self.all_source_names().into_iter().collect(),
        };
        wanted
            .into_iter()
            .filter(|name| !loaded.contains(name))
            .collect()
    }

    /// Load the files holding `keys` for `code` in the background, then bump the
    /// locale generation so the grid re-renders with the resolved text. One ensure
    /// runs at a time; the generation bump re-triggers `load` for anything still
    /// missing.
    fn ensure_localization(&self, code: String, keys: Vec<LocalizationKey>) {
        if self.needed_files(&code, &keys).is_empty() {
            return;
        }
        if self.loc_ensuring.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(this) = self.me.upgrade() else {
            self.loc_ensuring.store(false, Ordering::SeqCst);
            return;
        };
        std::thread::spawn(move || {
            let names = this.needed_files(&code, &keys);
            this.run_ensure(&code, names);
            this.loc_ensuring.store(false, Ordering::SeqCst);
            this.loc_gen.fetch_add(1, Ordering::SeqCst);
        });
    }

    fn run_ensure(&self, code: &str, names: Vec<String>) {
        if names.is_empty() {
            return;
        }
        let Ok(language) = code.parse::<LanguageCode>() else {
            return;
        };
        let Some(assets) = self.asset_store() else {
            return;
        };
        // Read + parse the needed files in parallel on the --jobs pool.
        let documents = self.runner.map(&names, |name| {
            let path = localization_asset_path(&language, name);
            let bytes = assets.read_path(&path).ok().flatten()?;
            let document = LocalizationDocument::parse_bytes(&bytes).ok()?;
            Some((name.clone(), document))
        });
        let mut langs = self
            .langs
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let state = langs
            .entry(code.to_string())
            .or_insert_with(|| LangState::new(language.clone()));
        for entry in documents.into_iter().flatten() {
            let (name, document) = entry;
            let _ =
                state
                    .builder
                    .add_document(name.into(), &LocalizationTag::init(), &document);
        }
        // Mark every requested file loaded (even absent ones) so we never retry it.
        state.loaded.extend(names);
        state.catalog = Arc::new(state.builder.clone().build());
    }

    /// A fingerprint of the install's paks (size + mtime) used to invalidate a
    /// persisted index when the game updates.
    fn pak_fingerprint(&self) -> u64 {
        let fold = |hash: u64, value: u64| (hash ^ value).wrapping_mul(0x0000_0001_0000_01b3);
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for path in &self.pak_paths {
            if let Ok(meta) = std::fs::metadata(path) {
                hash = fold(hash, meta.len());
                if let Ok(modified) = meta.modified()
                    && let Ok(elapsed) = modified.duration_since(std::time::UNIX_EPOCH)
                {
                    hash = fold(hash, elapsed.as_secs());
                }
            }
        }
        hash
    }

    fn set_key_index(&self, index: KeyFileIndex) {
        *self
            .key_index
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Arc::new(index));
        // Re-render so subsequent per-sheet loads target only the needed files.
        self.loc_gen.fetch_add(1, Ordering::SeqCst);
    }
}

impl crate::tui::SheetSource for DatasheetWorkspace {
    fn load(&self, sheet: u32) -> Option<crate::tui::SheetData> {
        let bytes = self.read_bytes(sheet as usize)?;
        let mut doc = nw_datasheet::Datasheet::parse(&bytes).ok()?;
        let code = self.active_code();
        let catalog = code.as_ref().and_then(|code| self.lang_catalog(code));
        if let Some(catalog) = &catalog {
            doc.set_localization(Some(catalog.as_ref()));
        }
        let has_localization = code.is_some();
        // Kick off loading this sheet's localization files in the background;
        // until they land, in-progress keys render as plain keys (not red).
        if let Some(code) = code {
            self.ensure_localization(code, collect_sheet_keys(&doc));
        }
        let in_progress = self.loc_ensuring.load(Ordering::SeqCst);
        let columns = doc
            .columns()
            .iter()
            .map(|column| crate::tui::GridColumn {
                name: column.name().to_string(),
                kind: grid_type(column.column_type()),
            })
            .collect();
        let mut rows = Vec::with_capacity(doc.len());
        for row in doc.rows() {
            rows.push(
                row.cells()
                    .iter()
                    .map(|cell| grid_cell(&doc, cell, has_localization, in_progress))
                    .collect(),
            );
        }
        Some(crate::tui::SheetData {
            label: self.label(sheet),
            columns,
            rows,
            has_localization,
        })
    }

    fn label(&self, sheet: u32) -> String {
        self.sheet_lock()
            .labels
            .get(sheet as usize)
            .cloned()
            .unwrap_or_default()
    }

    fn references(&self, value: &str) -> Vec<crate::tui::Loc> {
        self.index
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(value)
            .cloned()
            .unwrap_or_default()
    }

    fn progress(&self) -> crate::tui::IndexProgress {
        crate::tui::IndexProgress {
            indexed: self.indexed.load(Ordering::Relaxed),
            total: self.len(),
            done: self.index_done.load(Ordering::Relaxed),
        }
    }

    fn sheets(&self) -> Vec<(String, u64)> {
        let sheets = self.sheet_lock();
        sheets
            .labels
            .iter()
            .cloned()
            .zip(sheets.sizes.iter().copied())
            .collect()
    }

    fn discovery(&self) -> crate::tui::IndexProgress {
        crate::tui::IndexProgress {
            indexed: self.scanned.load(Ordering::Relaxed),
            total: self.discover_total,
            done: self.discover_done.load(Ordering::Relaxed),
        }
    }

    fn locales(&self) -> Vec<String> {
        self.cached_languages()
    }

    fn locale(&self) -> crate::tui::LocaleState {
        let slot = self.lock_locale();
        crate::tui::LocaleState {
            code: slot.code.clone(),
            loading: self.loc_ensuring.load(Ordering::SeqCst),
            generation: self.loc_gen.load(Ordering::Relaxed),
            error: slot.error.clone(),
        }
    }

    fn set_locale(&self, code: Option<&str>) {
        // Just record the active language and re-render. The grid's reload calls
        // `load`, which resolves the visible sheet's keys (loading their files on
        // demand). Per-language catalogs persist, so revisiting a language reuses
        // whatever was already loaded.
        {
            let mut slot = self.lock_locale();
            slot.code = code.map(str::to_string);
            slot.error = None;
        }
        self.loc_gen.fetch_add(1, Ordering::SeqCst);
    }
}

/// Background pipeline: discover datasheets across the paks in parallel (the
/// picker streams as they appear), then build the cross-reference index in
/// parallel. Both phases run on a shared thread pool so nothing blocks the UI.
fn discover_and_index(workspace: Arc<DatasheetWorkspace>) {
    let runner = &workspace.runner;

    if workspace.pak_paths.is_empty() {
        // File mode: no pak sweep, so enumerate languages directly (off the UI thread).
        workspace.warm_languages();
    } else {
        runner.map(&workspace.pak_paths, |path| discover_pak(&workspace, path));
    }
    workspace.discover_done.store(true, Ordering::Relaxed);

    let ids = (0..workspace.len()).collect::<Vec<_>>();
    runner.map(&ids, |&id| index_sheet(&workspace, id));
    workspace.index_done.store(true, Ordering::Relaxed);
}

/// Open one pak, append its datasheet entries to the workspace, and bump the
/// scan counter. Runs on a worker thread.
fn discover_pak(workspace: &DatasheetWorkspace, path: &Path) {
    if !workspace.cancel.load(Ordering::Relaxed)
        && let Ok(reader) = PakMmapReader::open(path)
    {
        let reader = Arc::new(reader);
        // Grab the language list for free if this pak holds the manifest.
        workspace.capture_languages(&reader);
        // Classify entries in one pass: datasheets feed the picker, localization
        // files feed the asset-store fast-path index.
        let mut found = Vec::new();
        let mut loc = Vec::new();
        for entry in reader.entries() {
            let name = entry.name();
            if nw_datasheet::is_datasheet_path(Path::new(name)) {
                found.push((entry.index(), name.to_string(), entry.uncompressed_size()));
            } else if is_localization_entry(name) {
                loc.push(name.to_string());
            }
        }
        if !found.is_empty() {
            let mut sheets = workspace.sheet_lock();
            for (index, name, size) in found {
                sheets.sources.push(SheetSrc::Pak {
                    reader: reader.clone(),
                    index,
                });
                sheets.labels.push(name);
                sheets.sizes.push(size);
            }
        }
        if !loc.is_empty() {
            let mut index = workspace
                .loc_index
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for name in loc {
                index.insert(name, reader.clone());
            }
        }
    }
    workspace.scanned.fetch_add(1, Ordering::Relaxed);
}

/// Whether an archive entry is a localization file or manifest — the assets the
/// locale loader reads. Cheap suffix check (no allocation) over many entries.
fn is_localization_entry(name: &str) -> bool {
    fn ends_ci(value: &str, suffix: &str) -> bool {
        value.len() >= suffix.len()
            && value[value.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    }
    ends_ci(name, ".loc.xml") || ends_ci(name, ".loc") || ends_ci(name, "localization.xml")
}

/// Index one sheet's string cells into the cross-reference map. Runs on a worker
/// thread.
fn index_sheet(workspace: &DatasheetWorkspace, id: usize) {
    if !workspace.cancel.load(Ordering::Relaxed)
        && let Some(bytes) = workspace.read_bytes(id)
        && let Ok(doc) = nw_datasheet::Datasheet::parse(&bytes)
    {
        let mut local: Vec<(String, crate::tui::Loc)> = Vec::new();
        for (row_index, row) in doc.rows().enumerate() {
            for (col_index, cell) in row.cells().iter().enumerate() {
                if let Some(value) = cell.as_str()
                    && !value.is_empty()
                {
                    local.push((
                        value.to_string(),
                        crate::tui::Loc {
                            sheet: id as u32,
                            row: row_index as u32,
                            col: col_index as u32,
                        },
                    ));
                }
            }
        }
        if !local.is_empty() {
            let mut index = workspace
                .index
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for (value, loc) in local {
                index.entry(value).or_default().push(loc);
            }
        }
    }
    workspace.indexed.fetch_add(1, Ordering::Relaxed);
}

/// Open the datasheet grid TUI. With an explicit `path` the workspace is the
/// loose `.datasheet` files there; with no path it streams every datasheet entry
/// out of the located game install's paks — that's the front door.
fn browse_datasheets(
    path: Option<PathBuf>,
    loc_root: Option<PathBuf>,
    initial_locale: Option<String>,
    mode: u8,
    jobs: Option<usize>,
) -> Result<()> {
    let workspace = match path {
        Some(path) => {
            // Loose extracted datasheets under a directory (or a single file).
            let paths = collect_matching(&path, nw_datasheet::is_datasheet_path)?;
            if paths.is_empty() {
                Report::new("datasheet").stat("files", 0usize).print();
                return Ok(());
            }
            // Localization still auto-locates (or uses --loc-root) for these.
            let asset_root = loc_root.or_else(|| {
                nw_locator::Install::locate()
                    .ok()
                    .map(|install| install.assets())
            });
            DatasheetWorkspace::from_files(paths, asset_root, jobs)
        }
        None => {
            // No path: locate the install and stream datasheets from its paks.
            // We only enumerate the pak *files* here (fast); opening and scanning
            // them happens in parallel on the background pool.
            let install = match nw_locator::Install::locate() {
                Ok(install) => install,
                Err(_) => {
                    Report::new("datasheet")
                        .note("no New World install found — pass a directory of .datasheet files")
                        .print();
                    return Ok(());
                }
            };
            let pak_root = install.assets();
            let asset_root = Some(loc_root.unwrap_or_else(|| install.assets()));
            let paks = PakSet::collect(pak_root, Vec::new())?;
            if paks.paths().is_empty() {
                Report::new("datasheet")
                    .note("no pak archives found in the install")
                    .print();
                return Ok(());
            }
            DatasheetWorkspace::from_paks(paks.paths().to_vec(), asset_root, jobs)
        }
    };

    if let Some(code) = initial_locale {
        workspace.set_locale(Some(&code));
    }
    // Enumerate languages on their own thread so the `L` picker is populated
    // quickly, independent of (and in parallel with) the full discovery sweep.
    let warm = Arc::clone(&workspace);
    std::thread::spawn(move || warm.warm_languages());
    // Load (or build + persist) the key→file index so localization can resolve
    // per sheet by reading only the files it needs.
    let indexer = Arc::clone(&workspace);
    std::thread::spawn(move || load_or_build_key_index(indexer));
    let worker = Arc::clone(&workspace);
    std::thread::spawn(move || discover_and_index(worker));
    let source: Arc<dyn crate::tui::SheetSource> = workspace.clone();

    let result = crate::tui::datasheet_browser(source, mode);
    workspace.cancel.store(true, Ordering::Relaxed);
    Ok(result?)
}

/// The distinct localization keys referenced by a sheet's `@`-prefixed cells.
fn collect_sheet_keys(sheet: &nw_datasheet::Datasheet<'_>) -> Vec<LocalizationKey> {
    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    for row in sheet.rows() {
        for cell in row.cells() {
            if let Some(value) = cell.as_str()
                && value.starts_with('@')
                && let Ok(key) = LocalizationKey::from_label(value)
                && seen.insert(key.crc32())
            {
                keys.push(key);
            }
        }
    }
    keys
}

/// English is always present in a New World install; used to build the
/// language-invariant key→file index and to enumerate source file names.
fn default_language() -> LanguageCode {
    LanguageCode::new("en-US").expect("en-US is a valid language code")
}

/// On-disk location for the cached key→file index.
fn key_index_cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    Some(base.join("nw-tools").join("loc-key-index.bin"))
}

fn read_cached_key_index(path: &Path, version: u64) -> Option<KeyFileIndex> {
    let index = KeyFileIndex::from_bytes(&std::fs::read(path).ok()?)?;
    (index.version() == version).then_some(index)
}

fn write_cached_key_index(path: &Path, index: &KeyFileIndex) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, index.to_bytes());
}

/// Load the persisted key→file index, or build it once (reading every loc file)
/// and persist it. Runs in the background; until it lands, per-sheet loads use the
/// load-everything fallback.
fn load_or_build_key_index(workspace: Arc<DatasheetWorkspace>) {
    if workspace.pak_paths.is_empty() {
        return; // file mode: no index, the fallback covers it
    }
    let version = workspace.pak_fingerprint();
    let Some(path) = key_index_cache_path() else {
        return;
    };
    if let Some(index) = read_cached_key_index(&path, version) {
        workspace.set_key_index(index);
        return;
    }
    let Some(assets) = workspace.asset_store() else {
        return;
    };
    let built = workspace
        .runner
        .install(|| LocalizationLoader::new(&assets, default_language()).build_key_index(version));
    if let Ok(index) = built {
        write_cached_key_index(&path, &index);
        workspace.set_key_index(index);
    }
}

fn grid_type(kind: nw_datasheet::ColumnType) -> crate::tui::GridType {
    match kind {
        nw_datasheet::ColumnType::String => crate::tui::GridType::String,
        nw_datasheet::ColumnType::Number => crate::tui::GridType::Number,
        nw_datasheet::ColumnType::Boolean => crate::tui::GridType::Boolean,
    }
}

fn grid_cell(
    sheet: &nw_datasheet::Datasheet<'_>,
    cell: &nw_datasheet::Cell<'_>,
    has_localization: bool,
    in_progress: bool,
) -> crate::tui::GridCell {
    let kind = grid_type(cell.column_type());
    if let Some(value) = cell.as_str() {
        let is_key = value.starts_with('@');
        let (localized, unresolved) = if is_key && has_localization {
            let text = sheet.localized(value);
            if text.as_ref() == value {
                // Not resolved: while its file is still loading show a plain key
                // (not the red "unresolved" state); flag it only once loading is done.
                if in_progress {
                    (None, false)
                } else {
                    (Some(text.into_owned()), true)
                }
            } else {
                (Some(text.into_owned()), false)
            }
        } else {
            (None, false)
        };
        crate::tui::GridCell {
            display: value.to_string(),
            localized,
            kind,
            is_key,
            unresolved,
        }
    } else {
        crate::tui::GridCell {
            display: cell.to_string(),
            localized: None,
            kind,
            is_key: false,
            unresolved: false,
        }
    }
}

fn sheet_summary_report(scans: &[SheetScan], limit: usize) -> Report {
    let mut totals = SheetTotals::default();
    for scan in scans {
        totals.add(&scan.summary);
    }
    let mut report = Report::new("datasheet")
        .stat("files", totals.files)
        .stat("rows", totals.rows)
        .stat("columns", totals.columns)
        .stat("cells", totals.cells);

    let mut table = Table::new([
        "Source", "Version", "Rows", "Columns", "Cells", "Strings", "Numbers", "Booleans", "Name",
        "Type",
    ])
    .right([2, 3, 4, 5, 6, 7]);
    for scan in scans.iter().take(limit) {
        let summary = &scan.summary;
        table.push([
            Cell::path(scan.source.clone()),
            Cell::text(format!("0x{:x}", summary.version)),
            Cell::text(summary.rows.to_string()),
            Cell::text(summary.columns.to_string()),
            Cell::text(summary.cells.to_string()),
            Cell::text(summary.string_columns.to_string()),
            Cell::text(summary.number_columns.to_string()),
            Cell::text(summary.boolean_columns.to_string()),
            Cell::text(summary.name.clone()),
            Cell::text(summary.type_name.clone()),
        ]);
    }
    if !table.is_empty() {
        report.table(table);
    }
    if scans.len() > limit {
        report.more(scans.len() - limit, "files");
    }
    report
}

fn row_values(
    sheet: &nw_datasheet::Datasheet<'_>,
    row: &nw_datasheet::Row<'_, '_>,
    options: &SheetOptions<'_>,
) -> String {
    let mut values = Vec::new();
    for (column, cell) in row.columns().iter().zip(row.cells()) {
        let value = cell_text(sheet, cell, options);
        if !options.show_empty && value.is_empty() {
            continue;
        }
        values.push(format!("{}={}", column.name(), trim_cell(value)));
    }
    values.join(", ")
}

fn find_sheet_cells(
    source: &str,
    sheet: &nw_datasheet::Datasheet<'_>,
    options: &SheetOptions<'_>,
) -> Vec<SheetHit> {
    let mut search = options
        .fuzzy
        .then(|| crate::fuzzy::MultiSearch::new(&options.find));
    let mut hits = Vec::new();
    for (row_index, row) in sheet.rows().enumerate() {
        for (column, cell) in row.columns().iter().zip(row.cells()) {
            let value = cell_text(sheet, cell, options);
            let score = match &mut search {
                Some(search) => match search.score_any([value.as_ref(), column.name()]) {
                    Some(score) => score,
                    None => continue,
                },
                None => {
                    if !text_matches(&value, &options.find)
                        && !text_matches(column.name(), &options.find)
                    {
                        continue;
                    }
                    0
                }
            };
            hits.push(SheetHit {
                source: source.to_string(),
                row: row_index.to_string(),
                column: column.name().to_string(),
                value: trim_cell(value),
                score,
            });
        }
    }
    hits
}

fn cell_text<'a>(
    sheet: &nw_datasheet::Datasheet<'_>,
    cell: &'a nw_datasheet::Cell<'a>,
    options: &SheetOptions<'_>,
) -> Cow<'a, str> {
    let Some(value) = cell.as_str() else {
        return Cow::Owned(cell.to_string());
    };

    match options.localize {
        LocalizeArg::Key => Cow::Borrowed(value),
        LocalizeArg::Text => sheet.localized(value),
        LocalizeArg::Both => {
            let localized = sheet.localized(value);
            if localized == value {
                Cow::Borrowed(value)
            } else {
                Cow::Owned(format!("{value} | {localized}"))
            }
        }
    }
}

fn objectstream_paths(
    root: &Path,
    extensions: &[String],
    selector: &PathSelector,
) -> Result<Vec<PathBuf>> {
    if root.is_file() {
        return Ok(path_selected(root, root, selector)
            .then(|| root.to_path_buf())
            .into_iter()
            .collect());
    }

    let extensions = lowered(extensions.to_vec());
    collect_matching(root, |path| {
        (extensions.is_empty()
            || path_ext(path).is_some_and(|extension| extensions.contains(&extension)))
            && path_selected(root, path, selector)
    })
}

fn objectstream_output_path(
    root: &Path,
    source: &Path,
    out: Option<&Path>,
    encoding: ObjectStreamEncoding,
) -> PathBuf {
    let Some(out) = out else {
        return objectstream_encoded_path(source.to_path_buf(), encoding);
    };
    if root.is_file() {
        return out.to_path_buf();
    }

    let relative = source.strip_prefix(root).unwrap_or(source);
    objectstream_encoded_path(out.join(relative), encoding)
}

fn objectstream_encoded_path(path: PathBuf, encoding: ObjectStreamEncoding) -> PathBuf {
    let stripped = strip_objectstream_text_extension(&path);
    match encoding.extension() {
        "" => stripped,
        extension => {
            let mut output = stripped.clone();
            if let Some(file_name) = stripped.file_name().and_then(|name| name.to_str()) {
                output.set_file_name(format!("{file_name}.{extension}"));
            }
            output
        }
    }
}

fn strip_objectstream_text_extension(path: &Path) -> PathBuf {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return path.to_path_buf();
    };
    if !matches!(extension.to_ascii_lowercase().as_str(), "json" | "xml") {
        return path.to_path_buf();
    }

    let Some(stem) = path.file_stem() else {
        return path.to_path_buf();
    };
    let mut stripped = path.to_path_buf();
    stripped.set_file_name(stem);
    stripped
}

fn path_selected(root: &Path, path: &Path, selector: &PathSelector) -> bool {
    let relative = nw_filesystem::display_relative(root, path);
    if !relative.is_empty() && selector.matches(&relative) {
        return true;
    }
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| selector.matches(name))
    {
        return true;
    }
    selector.matches(&path.display().to_string())
}

fn objectstream_payload(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    if nw_objectstream::looks_like_objectstream(bytes) {
        return Ok(Some(bytes.to_vec()));
    }

    if !nw_pak::azcs::is_azcs(bytes) {
        return Ok(None);
    }

    let mut cursor = Cursor::new(bytes);
    let mut reader = nw_pak::azcs::decompress(&mut cursor)?;
    let mut decoded = Vec::new();
    reader.read_to_end(&mut decoded)?;
    Ok(nw_objectstream::looks_like_objectstream(&decoded).then_some(decoded))
}

fn object_source(scan: &ObjectScan) -> &str {
    match scan {
        ObjectScan::Stats(scan) => &scan.source,
        ObjectScan::Dom(scan) => &scan.source,
        ObjectScan::Search { source, .. } => source,
    }
}

fn path_label(path: &Path) -> String {
    path.display().to_string()
}

impl From<EncodingArg> for ObjectStreamEncoding {
    fn from(value: EncodingArg) -> Self {
        match value {
            EncodingArg::Binary => Self::Binary,
            EncodingArg::Xml => Self::Xml,
            EncodingArg::Json => Self::Json,
        }
    }
}

fn lowered(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn text_matches(value: &str, needles: &[String]) -> bool {
    let value = value.to_ascii_lowercase();
    needles.iter().any(|needle| value.contains(needle))
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

fn trim_cell(value: impl AsRef<str>) -> String {
    const MAX: usize = 160;
    let value = value.as_ref().replace(['\r', '\n', '\t'], " ");
    if value.chars().count() <= MAX {
        value
    } else {
        format!("{}...", value.chars().take(MAX).collect::<String>())
    }
}

fn finish_scan(
    cancelled: bool,
    skipped: usize,
    errors: &[anyhow::Error],
    label: &str,
) -> Result<()> {
    if cancelled {
        bail!("{label} scan cancelled ({skipped} queued file(s) skipped)");
    }
    if !errors.is_empty() {
        for error in errors.iter().take(12) {
            eprintln!("{error:#}");
        }
        bail!("{} {label} file(s) failed", errors.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objectstream_directory_conversion_paths_use_target_encoding() {
        let root = Path::new("in");
        let source = Path::new("in/slices/player.slice.json");
        let out = Path::new("out");

        assert_eq!(
            objectstream_output_path(root, source, Some(out), ObjectStreamEncoding::Xml),
            PathBuf::from("out/slices/player.slice.xml")
        );
        assert_eq!(
            objectstream_output_path(root, source, Some(out), ObjectStreamEncoding::Binary),
            PathBuf::from("out/slices/player.slice")
        );
    }

    #[test]
    fn objectstream_conversion_defaults_next_to_input() {
        let source = Path::new("in/slices/player.slice.json");

        assert_eq!(
            objectstream_output_path(Path::new("in"), source, None, ObjectStreamEncoding::Binary),
            PathBuf::from("in/slices/player.slice")
        );
        assert_eq!(
            objectstream_output_path(
                Path::new("in"),
                Path::new("in/slices/player.slice"),
                None,
                ObjectStreamEncoding::Xml
            ),
            PathBuf::from("in/slices/player.slice.xml")
        );
    }
}
