use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use humansize::{DECIMAL, format_size};
use nw_jobs::CancellationToken;
use nw_pak::{Compression, PakMmapReader, azcs, crypak, oodle, shape};

use crate::extract::{MountedPath, PathClaims};
use crate::jobs::JobArgs;
use crate::progress::Job;
use crate::support::{AssetRootArg, GlobSet, PakSet, PathSelector, ScanIssues};
use crate::ui::{Cell, Report, Table, theme};

#[derive(Debug, Subcommand)]
pub enum Cmd {
    #[command(about = "List pak entries and archive metadata")]
    List(List),
    #[command(about = "Summarize pak compression and wrapper shape")]
    Shape(Shape),
    #[command(about = "Extract one pak archive")]
    Extract(Extract),
    #[command(about = "Build a CryPak-compatible archive from a directory")]
    Repack(Repack),
    #[command(about = "Replace or insert entries in a CryPak-compatible archive")]
    Update(Update),
}

impl Cmd {
    pub fn run(self) -> Result<()> {
        match self {
            Self::List(cmd) => cmd.run(),
            Self::Shape(cmd) => cmd.run(),
            Self::Extract(cmd) => cmd.run(),
            Self::Repack(cmd) => cmd.run(),
            Self::Update(cmd) => cmd.run(),
        }
    }
}

#[derive(Debug, Args)]
pub struct List {
    #[command(flatten)]
    root: AssetRootArg,

    #[arg(long, value_enum)]
    method: Option<MethodArg>,

    #[arg(long, value_enum)]
    family: Option<FamilyArg>,

    #[arg(long)]
    name: Vec<String>,

    #[arg(long)]
    azcs: bool,

    #[arg(long)]
    show: Option<usize>,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Shape {
    #[command(flatten)]
    root: AssetRootArg,

    #[arg(long, default_value_t = 20)]
    samples: usize,

    #[arg(long)]
    azcs: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Extract {
    out: PathBuf,

    #[command(flatten)]
    root: AssetRootArg,

    #[arg(long = "pak")]
    paks: Vec<String>,

    /// Case-insensitive path substring prefilter.
    #[arg(long)]
    filter: Option<String>,

    /// Archive path glob prefilter; repeat for multiple patterns.
    #[arg(long)]
    glob: Vec<String>,

    #[arg(long)]
    overwrite: bool,

    #[arg(long, default_value_t = 25)]
    show: usize,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Repack {
    input_dir: PathBuf,
    output_pak: PathBuf,

    #[arg(long, value_enum, default_value_t = MethodArg::Deflate)]
    method: MethodArg,

    #[arg(long, value_enum, default_value_t = ExtraArg::Auto)]
    extra: ExtraArg,

    #[arg(long, value_enum, default_value_t = LevelArg::Default)]
    level: LevelArg,

    #[arg(long = "oodle-pattern")]
    oodle_patterns: Vec<String>,

    #[arg(long, value_enum, default_value_t = OodleCompressorArg::Kraken)]
    oodle_compressor: OodleCompressorArg,

    #[arg(long, value_enum, default_value_t = OodleLevelArg::Normal)]
    oodle_level: OodleLevelArg,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Update {
    input_pak: PathBuf,
    output_pak: PathBuf,

    #[arg(long = "replace", value_name = "ENTRY=PATH", required = true)]
    replacements: Vec<ReplaceArg>,

    #[arg(long, value_enum)]
    method: Option<MethodArg>,

    #[arg(long, value_enum)]
    extra: Option<ExtraArg>,

    #[arg(long, value_enum, default_value_t = AzcsArg::Preserve)]
    azcs: AzcsArg,

    #[arg(long, value_enum, default_value_t = LevelArg::Default)]
    level: LevelArg,

    #[arg(long = "oodle-pattern")]
    oodle_patterns: Vec<String>,

    #[arg(long, value_enum, default_value_t = OodleCompressorArg::Kraken)]
    oodle_compressor: OodleCompressorArg,

    #[arg(long, value_enum, default_value_t = OodleLevelArg::Normal)]
    oodle_level: OodleLevelArg,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MethodArg {
    Store,
    Deflate,
    Oodle,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExtraArg {
    Auto,
    None,
    Marker10,
    Marker15,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FamilyArg {
    Audio,
    Data,
    Model,
    Other,
    Root,
    Script,
    Shader,
    Terrain,
    Texture,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LevelArg {
    Fastest,
    Faster,
    Default,
    Normal,
    Better,
    Best,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OodleCompressorArg {
    Kraken,
    Mermaid,
    Selkie,
    Hydra,
    Leviathan,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OodleLevelArg {
    Superfast,
    Fast,
    Normal,
    Optimal1,
    Optimal2,
    Optimal5,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AzcsArg {
    Preserve,
    Raw,
    Plain,
    Zlib,
}

#[derive(Debug, Clone)]
struct ReplaceArg {
    entry: String,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct ListFilter {
    method: Option<MethodArg>,
    family: Option<FamilyArg>,
    names: GlobSet,
    azcs: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EntryRow {
    pak: String,
    method: String,
    family: String,
    azcs: bool,
    size: String,
    name: String,
}

#[derive(Debug, Clone, Copy)]
struct PakExtractRun<'a> {
    out: &'a Path,
    filter: &'a PathSelector,
    overwrite: bool,
    claims: &'a PathClaims,
    cancel: &'a CancellationToken,
}

#[derive(Debug, Clone, Default)]
struct PakExtractReport {
    matched: u64,
    written: u64,
    skipped_existing: u64,
    skipped_duplicate: u64,
    bytes_written: u64,
    rows: Vec<PakExtractRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PakExtractRow {
    pak: String,
    size: String,
    path: String,
}

impl List {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let paks = PakSet::collect(self.root.resolve()?, Vec::new())?;
        let filter = ListFilter {
            method: self.method,
            family: self.family,
            names: GlobSet::archive(self.name),
            azcs: self.azcs,
        };
        // Interactive: open the browser instantly and stream entries in from a
        // background scan, so the UI never blocks on the enumeration.
        if theme::caps().interactive {
            let stats = vec![("archives".to_string(), paks.paths().len().to_string())];
            let template =
                Table::new(["Pak", "Method", "Family", "AZCS", "Size", "Name"]).right([4]);
            let feed = crate::tui::RowFeed::new(paks.paths().len());
            let runner = ctx.runner.clone();
            let cancel = ctx.cancel.clone();
            let feed_bg = feed.clone();
            std::thread::spawn(move || {
                runner.map(paks.paths(), |pak| {
                    if let Ok(rows) = scan_entries(&paks, pak, &filter, &cancel, None) {
                        feed_bg.extend(rows.into_iter().map(entry_cells).collect());
                    }
                    feed_bg.mark_done();
                });
            });
            return Ok(crate::tui::browse_streaming("pak list", stats, template, 5, feed)?);
        }

        // Piped / non-interactive: scan fully, then print a static report.
        let cancel = ctx.cancel.clone();
        let batch = ctx.map_results(
            "pak list",
            paks.paths(),
            |pak| paks.relative(pak),
            |pak, progress| scan_entries(&paks, pak, &filter, &cancel, Some(&progress)),
        );
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut rows = Vec::new();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(mut found) => rows.append(&mut found),
                Err(error) => errors.push(error),
            }
        }
        rows.sort();
        let total_rows = rows.len();
        if let Some(show) = self.show {
            rows.truncate(show);
        }

        let stats = vec![
            ("archives".to_string(), paks.paths().len().to_string()),
            ("matched".to_string(), total_rows.to_string()),
            ("shown".to_string(), rows.len().to_string()),
        ];
        let mut table = Table::new(["Pak", "Method", "Family", "AZCS", "Size", "Name"]).right([4]);
        for row in rows {
            table.push(entry_cells(row));
        }
        let mut report = Report::with_stats("pak list", stats);
        report.table_or(table, "no matching entries");
        report.print();

        if cancelled {
            bail!("pak list cancelled ({skipped} queued archive(s) skipped)");
        }
        if !errors.is_empty() {
            for error in errors.iter().take(12) {
                eprintln!("{error}");
            }
            bail!("{} archive(s) failed", errors.len());
        }
        Ok(())
    }
}

/// The browser/report cells for one listed entry.
fn entry_cells(row: EntryRow) -> Vec<Cell> {
    vec![
        Cell::path(row.pak),
        Cell::method(row.method),
        Cell::text(row.family),
        Cell::yes_no(row.azcs),
        Cell::size(row.size),
        Cell::path(row.name),
    ]
}

impl Shape {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let root = self.root.resolve()?;
        let progress = ctx.progress.stage("pak shape");
        let report = shape::Scanner::new()
            .max_samples(self.samples)
            .azcs(self.azcs)
            .scan_with(root, &ctx.runner, &ctx.cancel);
        progress.finish(if report.is_ok() { "done" } else { "failed" });
        let report = report?;
        print_shape_report(&report);
        Ok(())
    }
}

fn print_shape_report(report: &shape::Report) {
    let mut out = Report::new("pak shape")
        .stat("archives", report.archives)
        .stat("parsed", report.parsed_archives)
        .stat("entries", report.entries);
    out.kv("root", report.root.display().to_string());

    out.section("distribution");
    out.kv("methods", count_line(&report.methods));
    out.kv("versions", count_line(&report.versions));
    out.kv("flags", count_line(&report.flags));
    out.kv(
        "cdr_extra_lengths",
        count_line(&report.central_directory_extra_lengths),
    );
    out.kv(
        "cdr_extra_values",
        count_line(&report.central_directory_extra_values),
    );
    out.kv("extra_by_method", count_line(&report.extra_by_method));
    out.kv("extra_by_family", count_line(&report.extra_by_family));
    out.kv("method_by_family", count_line(&report.method_by_family));

    out.section("azcs");
    out.kv("azcs", count_line(&report.azcs));
    out.kv("azcs_by_method", count_line(&report.azcs_by_method));
    out.kv("azcs_by_extra", count_line(&report.azcs_by_extra));
    out.kv("azcs_by_family", count_line(&report.azcs_by_family));

    out.section("layout");
    out.kv(
        "local_extra_lengths",
        count_line(&report.local_extra_lengths),
    );
    out.kv(
        "cdr_comment_lengths",
        count_line(&report.central_directory_comment_lengths),
    );
    out.kv("disk_starts", count_line(&report.disk_starts));
    out.kv("internal_attrs", count_line(&report.internal_attributes));
    out.kv("external_attrs", count_line(&report.external_attributes));
    out.kv("separators", count_line(&report.separators));
    out.kv("uppercase_names", report.uppercase_names.to_string());
    out.kv("zip64_archives", report.zip64_archives.to_string());
    out.kv(
        "eocd_comment_archives",
        report.eocd_comment_archives.to_string(),
    );
    out.kv(
        "multi_disk_archives",
        report.multi_disk_archives.to_string(),
    );

    let samples = &report.samples;
    let groups = [
        ("errors", &samples.errors),
        ("unknown_methods", &samples.unknown_methods),
        ("nonzero_flags", &samples.nonzero_flags),
        ("nonzero_extra", &samples.nonzero_extra),
        ("comments", &samples.comments),
        ("mismatches", &samples.mismatches),
        ("zip64_entries", &samples.zip64_entries),
        ("azcs_errors", &samples.azcs_errors),
    ];
    if groups.iter().any(|(_, items)| !items.is_empty()) {
        out.section("samples");
        for (title, items) in groups {
            push_sample_group(&mut out, title, items);
        }
    }

    out.print();
}

fn count_line(counts: &BTreeMap<String, u64>) -> String {
    if counts.is_empty() {
        return "<none>".to_string();
    }
    counts
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn push_sample_group(report: &mut Report, title: &str, samples: &[String]) {
    if samples.is_empty() {
        return;
    }
    report.kv(title, format!("{} sample(s)", samples.len()));
    for sample in samples {
        report.note(format!("      {sample}"));
    }
}

impl Extract {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let root = self.root.resolve()?;
        let paks = PakSet::collect(root, self.paks)?;
        let filter = PathSelector::new(self.filter, self.glob);
        let claims = PathClaims::default();
        let cancel = ctx.cancel.clone();
        let run = PakExtractRun {
            out: &self.out,
            filter: &filter,
            overwrite: self.overwrite,
            claims: &claims,
            cancel: &cancel,
        };
        let batch = ctx.map_results(
            "pak extract",
            paks.paths(),
            |path| paks.relative(path),
            |path, progress| extract_pak(&paks, path, &run, &progress),
        );
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut report = PakExtractReport::default();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(scan) => report.merge(scan, self.show),
                Err(error) => errors.push(error),
            }
        }

        report.print("pak extract", self.show);
        ScanIssues::new("pak extract", skipped, cancelled, errors).finish()
    }
}

impl Repack {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let oodle_options =
            oodle::Options::new(self.oodle_compressor.into(), self.oodle_level.into());
        let mut options = crypak::Options::new()
            .method(self.method.with_oodle(oodle_options))
            .extra(self.extra.into())
            .level(self.level.into())
            .oodle(oodle_options);
        for pattern in self.oodle_patterns {
            options = options.oodle_pattern(pattern);
        }

        let progress = ctx.progress.stage("pak repack");
        let report = crypak::Repacker::new(self.input_dir, self.output_pak)
            .options(options)
            .run(&ctx.runner, &ctx.cancel);
        progress.finish(if report.is_ok() { "done" } else { "failed" });
        let report = report?;
        Report::new("pak repack")
            .stat("entries", report.entries)
            .stat("bytes", format_size(report.bytes_written, DECIMAL))
            .print();
        Ok(())
    }
}

impl Update {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let oodle_options =
            oodle::Options::new(self.oodle_compressor.into(), self.oodle_level.into());
        let mut options = crypak::Options::new()
            .method(
                self.method
                    .unwrap_or(MethodArg::Deflate)
                    .with_oodle(oodle_options),
            )
            .extra(self.extra.map_or(crypak::Extra::Auto, Into::into))
            .level(self.level.into())
            .oodle(oodle_options);
        for pattern in self.oodle_patterns {
            options = options.oodle_pattern(pattern);
        }

        let mut patches = Vec::with_capacity(self.replacements.len());
        for replacement in self.replacements {
            let data = std::fs::read(&replacement.path)
                .with_context(|| format!("read {}", replacement.path.display()))?;
            let mut patch = crypak::Patch::new(replacement.entry, data).azcs(self.azcs.into());
            if let Some(method) = self.method {
                patch = patch.method(method.with_oodle(oodle_options));
            }
            if let Some(extra) = self.extra {
                patch = patch.extra(extra.into());
            }
            patches.push(patch);
        }

        let progress = ctx.progress.stage("pak update");
        let report = crypak::Updater::new(self.input_pak, self.output_pak)
            .options(options)
            .patches(patches)
            .run(&ctx.runner, &ctx.cancel);
        progress.finish(if report.is_ok() { "done" } else { "failed" });
        let report = report?;
        Report::new("pak update")
            .stat("entries", report.entries)
            .stat("changed", report.changed)
            .stat("bytes", format_size(report.bytes_written, DECIMAL))
            .print();
        Ok(())
    }
}

fn scan_entries(
    paks: &PakSet,
    pak_path: &Path,
    filter: &ListFilter,
    cancel: &CancellationToken,
    progress: Option<&Job>,
) -> Result<Vec<EntryRow>> {
    let pak = PakMmapReader::open(pak_path)?;
    if let Some(progress) = progress {
        progress.set_len(pak.len());
    }
    let pak_name = paks.relative(pak_path);
    let mut rows = Vec::new();

    for entry in pak.entries() {
        if cancel.is_cancelled() {
            break;
        }
        if let Some(progress) = progress {
            progress.inc(1);
        }
        if filter
            .method
            .is_some_and(|method| !method.matches(entry.compression()))
        {
            continue;
        }

        let family = shape::path_family(entry.name());
        if filter
            .family
            .is_some_and(|expected| expected.as_str() != family)
        {
            continue;
        }

        if !filter.names.is_empty() && !filter.names.matches(entry.name()) {
            continue;
        }

        let azcs = if filter.azcs {
            let wrapped = pak.read_wrapped_by_index(entry.index())?;
            if !azcs::is_azcs(&wrapped) {
                continue;
            }
            true
        } else {
            false
        };

        rows.push(EntryRow {
            pak: pak_name.clone(),
            method: entry.compression().to_string(),
            family: family.to_string(),
            azcs,
            size: format_size(entry.uncompressed_size(), DECIMAL),
            name: entry.name().to_string(),
        });
    }

    Ok(rows)
}

fn extract_pak(
    paks: &PakSet,
    path: &Path,
    run: &PakExtractRun<'_>,
    progress: &Job,
) -> Result<PakExtractReport> {
    let pak = PakMmapReader::open(path)?;
    progress.set_len(pak.len());
    let pak_name = paks.relative(path);
    let mount_root = paks.mount_root(path);
    let mut report = PakExtractReport::default();

    for entry in pak.entries() {
        if run.cancel.is_cancelled() {
            break;
        }
        progress.inc(1);
        if !run.filter.matches(entry.name()) {
            continue;
        }
        report.matched += 1;
        let bytes = pak
            .read_by_index(entry.index())
            .with_context(|| format!("read {} from {}", entry.name(), path.display()))?;
        let target = MountedPath::new(run.out, &mount_root, entry.name())?;
        if report.write(&target, &bytes, run.overwrite, run.claims)? == WriteOutcome::Written {
            report.rows.push(PakExtractRow {
                pak: pak_name.clone(),
                size: format_size(bytes.len(), DECIMAL),
                path: target.display(),
            });
        }
    }

    Ok(report)
}

impl MethodArg {
    fn matches(self, method: Compression) -> bool {
        matches!(
            (self, method),
            (Self::Store, Compression::Stored)
                | (Self::Deflate, Compression::Deflated)
                | (Self::Oodle, Compression::Oodle)
        )
    }

    fn with_oodle(self, oodle_options: oodle::Options) -> crypak::Method {
        match self {
            Self::Store => crypak::Method::Store,
            Self::Deflate => crypak::Method::Deflate,
            Self::Oodle => crypak::Method::Oodle(oodle_options),
        }
    }
}

impl PakExtractReport {
    fn write(
        &mut self,
        target: &MountedPath,
        bytes: &[u8],
        overwrite: bool,
        claims: &PathClaims,
    ) -> Result<WriteOutcome> {
        if target.path().exists() && !overwrite {
            self.skipped_existing += 1;
            return Ok(WriteOutcome::Skipped);
        }
        if !claims.claim(target) {
            self.skipped_duplicate += 1;
            return Ok(WriteOutcome::Skipped);
        }
        if let Some(parent) = target.path().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target.path(), bytes)?;
        self.written += 1;
        self.bytes_written += bytes.len() as u64;
        Ok(WriteOutcome::Written)
    }

    fn merge(&mut self, mut other: Self, row_limit: usize) {
        self.matched += other.matched;
        self.written += other.written;
        self.skipped_existing += other.skipped_existing;
        self.skipped_duplicate += other.skipped_duplicate;
        self.bytes_written += other.bytes_written;
        let remaining = row_limit.saturating_sub(self.rows.len());
        let take = remaining.min(other.rows.len());
        self.rows.extend(other.rows.drain(..take));
    }

    fn print(&self, label: &str, limit: usize) {
        let mut report = Report::new(label)
            .stat("matched", self.matched)
            .stat("written", self.written)
            .stat("skip-existing", self.skipped_existing)
            .stat("skip-duplicate", self.skipped_duplicate)
            .stat("bytes", format_size(self.bytes_written, DECIMAL));
        let mut table = Table::new(["Pak", "Size", "Path"]).right([1]);
        for row in &self.rows {
            table.push([
                Cell::path(row.pak.clone()),
                Cell::size(row.size.clone()),
                Cell::path(row.path.clone()),
            ]);
        }
        if !table.is_empty() {
            report.table(table);
        }
        let remaining = self.written.saturating_sub(limit as u64);
        if remaining > 0 {
            report.more(remaining, "file(s)");
        }
        report.print();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteOutcome {
    Written,
    Skipped,
}

impl FamilyArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Data => "data",
            Self::Model => "model",
            Self::Other => "other",
            Self::Root => "root",
            Self::Script => "script",
            Self::Shader => "shader",
            Self::Terrain => "terrain",
            Self::Texture => "texture",
        }
    }
}

impl From<ExtraArg> for crypak::Extra {
    fn from(value: ExtraArg) -> Self {
        match value {
            ExtraArg::Auto => Self::Auto,
            ExtraArg::None => Self::None,
            ExtraArg::Marker10 => Self::Marker(0x10),
            ExtraArg::Marker15 => Self::Marker(0x15),
        }
    }
}

impl From<LevelArg> for crypak::Level {
    fn from(value: LevelArg) -> Self {
        match value {
            LevelArg::Fastest => Self::Fastest,
            LevelArg::Faster => Self::Faster,
            LevelArg::Default => Self::Default,
            LevelArg::Normal => Self::Normal,
            LevelArg::Better => Self::Better,
            LevelArg::Best => Self::Best,
        }
    }
}

impl From<OodleCompressorArg> for oodle::Compressor {
    fn from(value: OodleCompressorArg) -> Self {
        match value {
            OodleCompressorArg::Kraken => Self::Kraken,
            OodleCompressorArg::Mermaid => Self::Mermaid,
            OodleCompressorArg::Selkie => Self::Selkie,
            OodleCompressorArg::Hydra => Self::Hydra,
            OodleCompressorArg::Leviathan => Self::Leviathan,
        }
    }
}

impl From<OodleLevelArg> for oodle::Level {
    fn from(value: OodleLevelArg) -> Self {
        match value {
            OodleLevelArg::Superfast => Self::SuperFast,
            OodleLevelArg::Fast => Self::Fast,
            OodleLevelArg::Normal => Self::Normal,
            OodleLevelArg::Optimal1 => Self::Optimal1,
            OodleLevelArg::Optimal2 => Self::Optimal2,
            OodleLevelArg::Optimal5 => Self::Optimal5,
        }
    }
}

impl From<AzcsArg> for crypak::AzcsMode {
    fn from(value: AzcsArg) -> Self {
        match value {
            AzcsArg::Preserve => Self::Preserve,
            AzcsArg::Raw => Self::Raw,
            AzcsArg::Plain => Self::Plain,
            AzcsArg::Zlib => Self::Zlib,
        }
    }
}

impl FromStr for ReplaceArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (entry, path) = value
            .split_once('=')
            .ok_or_else(|| "expected ENTRY=PATH".to_string())?;
        let entry = entry.trim();
        if entry.is_empty() {
            return Err("entry path is empty".to_string());
        }
        let path = path.trim();
        if path.is_empty() {
            return Err("replacement filesystem path is empty".to_string());
        }
        Ok(Self {
            entry: entry.to_string(),
            path: PathBuf::from(path),
        })
    }
}
