use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use humansize::{DECIMAL, format_size};
use nw_localization::{
    LanguageCode, LocalizationCatalog, LocalizationKey, LocalizationLoader, LocalizationTag,
    localization_keys,
};
use nw_objectstream::ObjectStreamEncoding;
use nw_objectstream::lookup::NameLookup;

use crate::jobs::{JobArgs, RunCtx};
use crate::output::Table;
use crate::support::{PathSelector, collect_matching, load_lookup, path_ext};

#[derive(Debug, Subcommand)]
pub enum Cmd {
    #[command(about = "Inspect asset catalog files")]
    Catalog(Catalog),
    #[command(about = "Inspect datasheet files")]
    Datasheet(Datasheet),
    #[command(name = "dds", about = "Inspect or convert DDS texture files")]
    Dds(Dds),
    #[command(name = "objectstream", about = "Inspect ObjectStream files")]
    ObjectStream(ObjectStream),
}

impl Cmd {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Catalog(cmd) => cmd.run(),
            Self::Datasheet(cmd) => cmd.run(),
            Self::Dds(cmd) => cmd.run(),
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
    path: PathBuf,

    #[arg(long, default_value_t = 25)]
    show: usize,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct CatalogFind {
    path: PathBuf,
    query: Vec<String>,

    #[arg(long, default_value_t = 25)]
    show: usize,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct CatalogGet {
    path: PathBuf,
    query: Vec<String>,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct CatalogExport {
    path: PathBuf,
    out: PathBuf,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Datasheet {
    path: PathBuf,

    #[arg(long, default_value_t = 25)]
    show: usize,

    #[arg(long)]
    columns: bool,

    #[arg(long)]
    rows: Option<usize>,

    #[arg(long)]
    find: Vec<String>,

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
    path: PathBuf,

    #[arg(long, default_value_t = 40)]
    show: usize,

    /// Convert DDS textures to KTX2 files under this path.
    #[arg(long = "to-ktx2", value_name = "PATH")]
    to_ktx2: Option<PathBuf>,

    /// Replace existing KTX2 outputs.
    #[arg(long)]
    overwrite: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct ObjectStream {
    path: PathBuf,

    #[arg(long, conflicts_with = "to")]
    dom: bool,

    #[arg(long, conflicts_with = "to")]
    query: Option<String>,

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
        let paths = collect_matching(&self.path, |path| nw_asset::is_asset_catalog_path(path))?;
        let batch = ctx.map_results_compact(
            "catalog",
            &paths,
            |path| path_label(path),
            |path, progress| progress.step(|| scan_catalog(path, self.show, &[])),
        );
        print_catalog_scans(batch, paths.len(), "catalog")
    }
}

impl CatalogFind {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let paths = collect_matching(&self.path, |path| nw_asset::is_asset_catalog_path(path))?;
        let find = lowered(self.query);
        let batch = ctx.map_results_compact(
            "catalog find",
            &paths,
            |path| path_label(path),
            |path, progress| progress.step(|| scan_catalog(path, self.show, &find)),
        );
        print_catalog_scans(batch, paths.len(), "catalog find")
    }
}

impl CatalogGet {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let paths = collect_matching(&self.path, |path| nw_asset::is_asset_catalog_path(path))?;
        let queries = lowered(self.query);
        let batch = ctx.map_results_compact(
            "catalog get",
            &paths,
            |path| path_label(path),
            |path, progress| progress.step(|| get_catalog(path, &queries)),
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

        println!("catalogs: {}  matched: {}", paths.len(), rows.len());
        let mut table = Table::new([
            "Catalog", "Kind", "Size", "AssetId", "Type", "Flags", "Path",
        ]);
        for row in rows {
            table.push([
                row.source,
                row.kind,
                row.size,
                row.asset_id,
                row.asset_type,
                row.flags,
                row.path,
            ]);
        }
        if table.is_empty() {
            println!("no catalog rows to show");
        } else {
            print!("{table}");
        }

        finish_scan(cancelled, skipped, &errors, "catalog get")
    }
}

impl CatalogExport {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let paths = collect_matching(&self.path, |path| nw_asset::is_asset_catalog_path(path))?;
        let batch = ctx.map_results_compact(
            "catalog export",
            &paths,
            |path| path_label(path),
            |path, progress| progress.step(|| export_catalog(path)),
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
        println!(
            "catalogs: {}  exported: {}  path: {}",
            paths.len(),
            rows.len(),
            self.out.display()
        );

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
    println!("catalogs: {path_count}  entries: {total_entries}  matched: {total_matched}");
    for scan in &scans {
        let details = if scan.details.is_empty() {
            String::new()
        } else {
            format!("  {}", scan.details)
        };
        println!(
            "{}: {} v{}  entries {}  matched {}{}",
            scan.source, scan.kind, scan.version, scan.entries, scan.matched, details
        );
    }

    let mut table = Table::new([
        "Catalog", "Kind", "Size", "AssetId", "Type", "Flags", "Path",
    ]);
    for row in scans.into_iter().flat_map(|scan| scan.rows) {
        table.push([
            row.source,
            row.kind,
            row.size,
            row.asset_id,
            row.asset_type,
            row.flags,
            row.path,
        ]);
    }
    if table.is_empty() {
        println!("no catalog rows to show");
    } else {
        print!("{table}");
    }

    finish_scan(cancelled, skipped, &errors, label)
}

impl Datasheet {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let paths = collect_matching(&self.path, nw_datasheet::is_datasheet_path)?;
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
                Ok::<LocalizationCatalog, anyhow::Error>(
                    LocalizationLoader::new(&assets, locale)
                        .tags(self.loc_tags.clone())
                        .keys(needed_keys.iter().cloned())
                        .load()?,
                )
            })
            .transpose()?;
        if let Some(catalog) = localization.as_ref() {
            let report = catalog.report();
            println!(
                "localization: {}  needed: {}  files: {}  entries: {}  duplicates: {}",
                catalog.language(),
                needed_keys.len(),
                report.source_files(),
                report.entries(),
                report.duplicates().len()
            );
        }
        let options = SheetOptions {
            columns: self.columns,
            rows: self.rows,
            find,
            show_empty: self.show_empty,
            localization: localization.as_ref(),
            localize: self.localize,
        };
        if let Some(out) = self.csv.as_ref() {
            return export_datasheets(&ctx, &self.path, &paths, out, &options, self.overwrite);
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
        print_sheet_summary(&scans, self.show);

        if self.columns {
            let mut table = Table::new(["Source", "Index", "Type", "CRC", "Name"]);
            for row in scans.iter().flat_map(|scan| scan.columns.clone()) {
                table.push([row.source, row.index, row.kind, row.crc, row.name]);
            }
            if !table.is_empty() {
                print!("{table}");
            }
        }

        if self.rows.is_some() {
            let mut table = Table::new(["Source", "Row", "Values"]);
            for row in scans.iter().flat_map(|scan| scan.rows.clone()) {
                table.push([row.source, row.row, row.values]);
            }
            if !table.is_empty() {
                print!("{table}");
            }
        }

        if !options.find.is_empty() {
            let mut table = Table::new(["Source", "Row", "Column", "Value"]);
            for hit in scans.into_iter().flat_map(|scan| scan.hits) {
                table.push([hit.source, hit.row, hit.column, hit.value]);
            }
            if table.is_empty() {
                println!("no datasheet matches");
            } else {
                print!("{table}");
            }
        }

        finish_scan(cancelled, skipped, &errors, "datasheet")
    }
}

impl Dds {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        if let Some(out) = self.to_ktx2.as_ref() {
            return convert_dds_to_ktx2(&ctx, &self.path, out, self.overwrite);
        }

        let paths = collect_matching(&self.path, |path| nw_dds::is_dds_path(path))?;
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

        println!(
            "dds files: {}  shown: {}",
            scans.len(),
            scans.len().min(self.show)
        );
        let mut table = Table::new([
            "Source",
            "Kind",
            "Dimensions",
            "Mips",
            "Format",
            "DX10",
            "Cry",
            "Bytes",
        ]);
        for scan in scans.iter().take(self.show) {
            table.push([
                scan.source.clone(),
                scan.kind.clone(),
                scan.dimensions.clone(),
                scan.mipmaps.clone(),
                scan.format.clone(),
                scan.dx10.clone(),
                scan.cry.clone(),
                scan.bytes.clone(),
            ]);
        }
        if table.is_empty() {
            println!("no DDS files to show");
        } else {
            print!("{table}");
        }
        if scans.len() > self.show {
            println!("... {} more file(s)", scans.len() - self.show);
        }

        finish_scan(cancelled, skipped, &errors, "dds")
    }
}

impl ObjectStream {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let lookup = load_lookup(self.no_names)?;
        let selector = PathSelector::new(self.filter, self.glob);
        let paths = objectstream_paths(&self.path, &self.extensions, &selector)?;
        if let Some(encoding) = self.to {
            return convert_objectstreams(
                &ctx,
                &self.path,
                &paths,
                self.out.as_deref(),
                encoding.into(),
                self.overwrite,
                lookup.as_ref(),
            );
        }

        let mode = if self.dom {
            ObjectMode::Dom { limit: self.show }
        } else {
            ObjectMode::Stats
        };
        let query = self.query.clone();
        let batch = ctx.map_results_compact(
            "objectstream",
            &paths,
            |path| path_label(path),
            |path, progress| {
                progress.step(|| {
                    scan_objectstream(path, mode, query.as_deref(), self.show, lookup.as_ref())
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
        println!(
            "objectstreams: {}  shown: {}  names: {}",
            scans.len(),
            shown,
            if lookup.is_some() { "loaded" } else { "off" }
        );
        for scan in scans.into_iter().take(self.files) {
            match scan {
                ObjectScan::Stats(scan) => print_object_stats(&scan),
                ObjectScan::Dom(scan) => print_object_dom(&scan),
                ObjectScan::Search { source, hits } => {
                    println!("{source}: {} hit group(s)", hits.len());
                    let mut table = Table::new(["Kind", "Count", "Score", "Value"]);
                    for hit in hits {
                        table.push([
                            hit.kind,
                            hit.count.to_string(),
                            hit.score.to_string(),
                            hit.value,
                        ]);
                    }
                    if table.is_empty() {
                        println!("no ObjectStream matches");
                    } else {
                        print!("{table}");
                    }
                }
            }
        }

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

    println!(
        "objectstreams: {}  converted: {}  encoding: {}",
        paths.len(),
        converted.len(),
        encoding
    );
    let mut table = Table::new(["Source", "Output", "Encoding", "Bytes"]);
    for row in converted {
        table.push([row.source, row.output, row.encoding, row.bytes]);
    }
    if table.is_empty() {
        println!("no ObjectStreams converted");
    } else {
        print!("{table}");
    }

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

    println!(
        "dds textures: {}  converted: {}",
        groups.len(),
        converted.len()
    );
    let mut table = Table::new(["Source", "Output", "Bytes"]);
    for row in converted {
        table.push([row.source, row.output, row.bytes]);
    }
    if table.is_empty() {
        println!("no DDS textures converted");
    } else {
        print!("{table}");
    }

    finish_scan(cancelled, skipped, &errors, "dds conversion")
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

fn scan_catalog(path: &Path, limit: usize, find: &[String]) -> Result<CatalogScan> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let catalog =
        nw_asset::Catalog::parse(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let source = path.display().to_string();

    match catalog {
        nw_asset::Catalog::Rasc(catalog) => {
            let mut rows = Vec::new();
            let mut matched = 0usize;
            for entry in catalog.entries() {
                if !find.is_empty() && !text_matches(entry.path(), find) {
                    continue;
                }
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
                if !find.is_empty() && !raoc_text_matches(entry, find) {
                    continue;
                }
                matched += 1;
                if rows.len() < limit {
                    rows.push(CatalogRow {
                        source: source.clone(),
                        kind: "RAOC".to_string(),
                        size: format_size(u64::from(entry.size_bytes()), DECIMAL),
                        asset_id: entry.asset_id().to_string(),
                        asset_type: entry.asset_type().to_string(),
                        flags: format!("0x{:08x}", entry.flags()),
                        path: String::new(),
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

fn get_catalog(path: &Path, queries: &[String]) -> Result<Vec<CatalogRow>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let catalog =
        nw_asset::Catalog::parse(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let source = path.display().to_string();
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
                });
            }
        }
    }

    Ok(rows)
}

fn export_catalog(path: &Path) -> Result<Vec<CatalogExportRow>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let catalog =
        nw_asset::Catalog::parse(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let source = path.display().to_string();

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

    println!("datasheets: {}  exported: {}", paths.len(), exported.len());
    let mut table = Table::new(["Source", "Output", "Rows"]);
    for row in exported {
        table.push([row.source, row.output, row.rows]);
    }
    if table.is_empty() {
        println!("no datasheets exported");
    } else {
        print!("{table}");
    }

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
        let hits = nw_objectstream::query::collect_search_matches(&bytes, lookup, |value| {
            let value = value.to_ascii_lowercase();
            value.contains(&needle).then_some(1)
        })
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

fn print_object_stats(scan: &ObjectStatsScan) {
    let stats = scan.stats;
    println!("{} ({})", scan.source, stats.mode_label());
    println!("  version:   {}", stats.version);
    println!("  elements:  {}", stats.elements);
    println!("  max depth: {}", stats.max_depth);
    println!("  bytes:     {}", stats.bytes);
    if scan.names_loaded {
        println!(
            "  resolved:  {} elements had a known type, {} fields had a known name",
            stats.resolved_types, stats.resolved_fields
        );
    } else {
        println!("  resolved:  (no serialize.json - names unresolved)");
    }
}

fn print_object_dom(scan: &ObjectDomScan) {
    println!("{} (DOM)", scan.source);
    println!("  version: {}", scan.version);
    println!("  top-level elements: {}", scan.top_level_elements);
    if !scan.rows.is_empty() {
        let mut table = Table::new(["Index", "Flags", "Id", "Type", "Field"]);
        for row in &scan.rows {
            table.push([
                row.index.clone(),
                row.flags.clone(),
                row.id.clone(),
                row.type_name.clone(),
                row.field.clone(),
            ]);
        }
        print!("{table}");
    }
    let remaining = scan.total_elements.saturating_sub(scan.rows.len());
    if remaining > 0 {
        println!("... {remaining} more element(s)");
    }
}

fn print_sheet_summary(scans: &[SheetScan], limit: usize) {
    let mut table = Table::new([
        "Source", "Version", "Rows", "Columns", "Cells", "Strings", "Numbers", "Booleans", "Name",
        "Type",
    ]);
    let mut totals = SheetTotals::default();
    for scan in scans {
        totals.add(&scan.summary);
    }
    for scan in scans.iter().take(limit) {
        let summary = &scan.summary;
        table.push([
            scan.source.clone(),
            format!("0x{:x}", summary.version),
            summary.rows.to_string(),
            summary.columns.to_string(),
            summary.cells.to_string(),
            summary.string_columns.to_string(),
            summary.number_columns.to_string(),
            summary.boolean_columns.to_string(),
            summary.name.to_string(),
            summary.type_name.to_string(),
        ]);
    }
    println!(
        "datasheets: {}  rows: {}  columns: {}  cells: {}",
        totals.files, totals.rows, totals.columns, totals.cells
    );
    if !table.is_empty() {
        print!("{table}");
    }
    if scans.len() > limit {
        println!("... {} more files", scans.len() - limit);
    }
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
    let mut hits = Vec::new();
    for (row_index, row) in sheet.rows().enumerate() {
        for (column, cell) in row.columns().iter().zip(row.cells()) {
            let value = cell_text(sheet, cell, options);
            if text_matches(&value, &options.find) || text_matches(column.name(), &options.find) {
                hits.push(SheetHit {
                    source: source.to_string(),
                    row: row_index.to_string(),
                    column: column.name().to_string(),
                    value: trim_cell(value),
                });
            }
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
