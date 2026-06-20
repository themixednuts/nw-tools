use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use humansize::{DECIMAL, format_size};
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

    #[arg(long)]
    dom: bool,

    #[arg(long)]
    query: Option<String>,

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
struct SheetOptions {
    columns: bool,
    rows: Option<usize>,
    find: Vec<String>,
    show_empty: bool,
}

#[derive(Debug, Clone)]
struct SheetScan {
    source: String,
    summary: nw_datasheet::OwnedDatasheetSummary,
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
    Inspect {
        source: String,
        report: String,
    },
    Search {
        source: String,
        hits: Vec<ObjectHit>,
    },
}

#[derive(Debug, Clone)]
struct ObjectHit {
    kind: String,
    count: u64,
    score: u32,
    value: String,
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
        let paths = collect_matching(&self.path, |path| nw_catalog::is_asset_catalog_path(path))?;
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
        let paths = collect_matching(&self.path, |path| nw_catalog::is_asset_catalog_path(path))?;
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
        let paths = collect_matching(&self.path, |path| nw_catalog::is_asset_catalog_path(path))?;
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
        let paths = collect_matching(&self.path, |path| nw_catalog::is_asset_catalog_path(path))?;
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
        let options = SheetOptions {
            columns: self.columns,
            rows: self.rows,
            find: lowered(self.find),
            show_empty: self.show_empty,
        };
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
        let mode = if self.dom {
            nw_objectstream::stats::ObjectStreamInspectionMode::dom(self.show)
        } else {
            nw_objectstream::stats::ObjectStreamInspectionMode::streaming()
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
                ObjectScan::Inspect { report, .. } => {
                    print!("{report}");
                    if !report.ends_with('\n') {
                        println!();
                    }
                }
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
        nw_catalog::Catalog::parse(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let source = path.display().to_string();

    match catalog {
        nw_catalog::Catalog::Rasc(catalog) => {
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
        nw_catalog::Catalog::Raoc(catalog) => {
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
        nw_catalog::Catalog::parse(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let source = path.display().to_string();
    let mut rows = Vec::new();

    match catalog {
        nw_catalog::Catalog::Rasc(catalog) => {
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
        nw_catalog::Catalog::Raoc(catalog) => {
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
        nw_catalog::Catalog::parse(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let source = path.display().to_string();

    Ok(match catalog {
        nw_catalog::Catalog::Rasc(catalog) => catalog
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
        nw_catalog::Catalog::Raoc(catalog) => catalog
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

fn scan_sheet(path: &Path, options: &SheetOptions) -> Result<SheetScan> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let sheet = nw_datasheet::Datasheet::parse(&bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    let summary = nw_datasheet::OwnedDatasheetSummary::from(
        nw_datasheet::DatasheetSummary::from_datasheet(&sheet),
    );
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
                    values: row_values(&row, options.show_empty),
                })
                .collect()
        })
        .unwrap_or_default();
    let hits = if options.find.is_empty() {
        Vec::new()
    } else {
        find_sheet_cells(&source, &sheet, &options.find)
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
    mode: nw_objectstream::stats::ObjectStreamInspectionMode,
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

    let report = nw_objectstream::stats::inspect_file_bytes_with_mode(path, &bytes, mode, lookup)
        .with_context(|| format!("inspect {}", path.display()))?
        .to_string();
    Ok(Some(ObjectScan::Inspect { source, report }))
}

fn print_sheet_summary(scans: &[SheetScan], limit: usize) {
    let mut table = Table::new([
        "Source", "Version", "Rows", "Columns", "Cells", "Strings", "Numbers", "Booleans", "Name",
        "Type",
    ]);
    let mut totals = nw_datasheet::DatasheetTotals::default();
    for scan in scans {
        totals.add_summary(scan.summary.as_borrowed());
    }
    for scan in scans.iter().take(limit) {
        let summary = scan.summary.as_borrowed();
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

fn row_values(row: &nw_datasheet::Row<'_, '_>, show_empty: bool) -> String {
    let mut values = Vec::new();
    for (column, cell) in row.columns().iter().zip(row.cells()) {
        let value = cell.to_string();
        if !show_empty && value.is_empty() {
            continue;
        }
        values.push(format!("{}={}", column.name(), trim_cell(value)));
    }
    values.join(", ")
}

fn find_sheet_cells(
    source: &str,
    sheet: &nw_datasheet::Datasheet<'_>,
    find: &[String],
) -> Vec<SheetHit> {
    let mut hits = Vec::new();
    for (row_index, row) in sheet.rows().enumerate() {
        for (column, cell) in row.columns().iter().zip(row.cells()) {
            let value = cell.to_string();
            if text_matches(&value, find) || text_matches(column.name(), find) {
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
        ObjectScan::Inspect { source, .. } | ObjectScan::Search { source, .. } => source,
    }
}

fn path_label(path: &Path) -> String {
    path.display().to_string()
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

fn raoc_text_matches(entry: &nw_catalog::RaocEntry, needles: &[String]) -> bool {
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
