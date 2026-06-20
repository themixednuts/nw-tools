use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use humansize::{DECIMAL, format_size};
use nw_objectstream::lookup::NameLookup;
use nw_objectstream::{ObjectStream, ObjectStreamEncoding};
use nw_pak::{Compression, EntryInfo, PakMmapReader, azcs, crypak, shape};

use crate::jobs::JobArgs;
use crate::output::Table;
use crate::support::{AssetRootArg, GlobSet, PakSet, ScanIssues, load_lookup};

const DEFAULT_MAX_ENTRY_SIZE: u64 = 128 * 1024 * 1024;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    #[command(about = "Summarize archive entries by observed extension key")]
    Inventory(Inventory),
    #[command(about = "Search assets across one or more pak archives")]
    Search(Search),
    #[command(about = "Extract selected assets from pak archives")]
    Extract(Extract),
    #[command(about = "Replace structured assets in a pak archive")]
    Update(Update),
}

impl Cmd {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Inventory(cmd) => cmd.run(),
            Self::Search(cmd) => cmd.run(),
            Self::Extract(cmd) => cmd.run(),
            Self::Update(cmd) => cmd.run(),
        }
    }
}

#[derive(Debug, Args)]
pub struct Inventory {
    #[command(flatten)]
    root: AssetRootArg,

    #[arg(long = "pak")]
    paks: Vec<String>,

    #[arg(long, value_enum, default_value_t = InventorySort::Count)]
    sort: InventorySort,

    #[arg(long, value_enum, default_value_t = InventoryGroup::Ext)]
    group: InventoryGroup,

    #[arg(long, default_value_t = DEFAULT_MAX_ENTRY_SIZE)]
    max_entry_size: u64,

    #[arg(long, default_value_t = 40)]
    limit: usize,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Search {
    #[command(subcommand)]
    command: SearchCmd,
}

#[derive(Debug, Subcommand)]
pub enum SearchCmd {
    #[command(about = "Find archive entries by path")]
    Path(SearchPath),
    #[command(name = "objectstream", about = "Search decoded ObjectStream payloads")]
    ObjectStream(SearchObjectStream),
}

#[derive(Debug, Args)]
pub struct SearchPath {
    query: String,

    #[command(flatten)]
    root: AssetRootArg,

    #[arg(long = "pak")]
    paks: Vec<String>,

    #[arg(long)]
    glob: bool,

    #[arg(long)]
    case_sensitive: bool,

    #[arg(long, default_value_t = 100)]
    limit: usize,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct SearchObjectStream {
    query: String,

    #[command(flatten)]
    root: AssetRootArg,

    #[arg(long = "pak")]
    paks: Vec<String>,

    #[arg(long, default_value_t = 100)]
    limit: usize,

    #[arg(long, default_value_t = DEFAULT_MAX_ENTRY_SIZE)]
    max_entry_size: u64,

    #[arg(long)]
    no_names: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Extract {
    #[command(subcommand)]
    command: ExtractCmd,
}

#[derive(Debug, Args)]
pub struct Update {
    #[command(subcommand)]
    command: UpdateCmd,
}

#[derive(Debug, Subcommand)]
pub enum UpdateCmd {
    #[command(name = "objectstream", about = "Replace one ObjectStream pak entry")]
    ObjectStream(UpdateObjectStream),
}

#[derive(Debug, Args)]
pub struct UpdateObjectStream {
    input_pak: PathBuf,
    output_pak: PathBuf,
    entry: String,
    input: PathBuf,

    #[arg(long)]
    no_names: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Subcommand)]
pub enum ExtractCmd {
    #[command(about = "Extract entries with an observed extension key")]
    Ext(ExtractExt),
    #[command(name = "objectstream", about = "Extract decoded ObjectStream payloads")]
    ObjectStream(ExtractObjectStream),
}

#[derive(Debug, Args)]
pub struct ExtractExt {
    extension: String,
    out: PathBuf,

    #[command(flatten)]
    root: AssetRootArg,

    #[arg(long = "pak")]
    paks: Vec<String>,

    #[arg(long)]
    overwrite: bool,

    #[arg(long, default_value_t = 25)]
    limit: usize,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct ExtractObjectStream {
    out: PathBuf,

    #[command(flatten)]
    root: AssetRootArg,

    #[arg(long = "pak")]
    paks: Vec<String>,

    #[arg(long, value_enum, default_value_t = EncodingArg::Json)]
    encoding: EncodingArg,

    #[arg(long, default_value_t = DEFAULT_MAX_ENTRY_SIZE)]
    max_entry_size: u64,

    #[arg(long)]
    no_names: bool,

    #[arg(long)]
    overwrite: bool,

    #[arg(long, default_value_t = 25)]
    limit: usize,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InventorySort {
    Count,
    Size,
    Packed,
    Key,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InventoryGroup {
    Ext,
    Kind,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum EncodingArg {
    Binary,
    Xml,
    Json,
}

#[derive(Debug, Clone, Default)]
struct InventoryReport {
    paks: usize,
    entries: u64,
    stats: BTreeMap<String, InventoryStat>,
}

#[derive(Debug, Clone)]
struct InventoryStat {
    key: String,
    entries: u64,
    unpacked_bytes: u64,
    packed_bytes: u64,
    paks: BTreeSet<String>,
    sample: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InventoryRow {
    key: String,
    entries: u64,
    unpacked_bytes: u64,
    packed_bytes: u64,
    paks: usize,
    sample: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PathHit {
    pak: String,
    method: String,
    size: String,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectHit {
    pak: String,
    name: String,
    envelope: String,
    kind: String,
    count: u64,
    score: u32,
    value: String,
}

#[derive(Debug, Clone, Default)]
struct ExtractReport {
    matched: u64,
    written: u64,
    skipped: u64,
    bytes_written: u64,
    rows: Vec<ExtractRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExtractRow {
    pak: String,
    size: String,
    path: String,
}

#[derive(Debug, Clone)]
struct TextQuery {
    raw: String,
    lowered: String,
    case_sensitive: bool,
    glob: Option<GlobSet>,
}

#[derive(Debug, Clone)]
struct ObjectPayload {
    bytes: Vec<u8>,
    envelope: bool,
    encoding: ObjectStreamEncoding,
}

impl Inventory {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let root = self.root.resolve()?;
        let paks = PakSet::collect(root, self.paks)?;
        let batch = ctx
            .runner
            .map_until_cancelled(paks.paths(), &ctx.cancel, |path| {
                InventoryReport::from_pak(&paks, path, self.group, self.max_entry_size)
            });
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut report = InventoryReport::default();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(scan) => report.merge(scan),
                Err(error) => errors.push(error),
            }
        }

        report.print(self.sort, self.limit);
        ScanIssues::new("asset inventory", skipped, cancelled, errors).finish()
    }
}

impl Search {
    fn run(self) -> Result<()> {
        match self.command {
            SearchCmd::Path(cmd) => cmd.run(),
            SearchCmd::ObjectStream(cmd) => cmd.run(),
        }
    }
}

impl SearchPath {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let root = self.root.resolve()?;
        let paks = PakSet::collect(root, self.paks)?;
        let query = TextQuery::new(self.query, self.case_sensitive, self.glob);
        let batch = ctx
            .runner
            .map_until_cancelled(paks.paths(), &ctx.cancel, |path| {
                Self::scan_pak(&paks, path, &query)
            });
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut rows = Vec::new();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(mut hits) => rows.append(&mut hits),
                Err(error) => errors.push(error),
            }
        }

        rows.sort();
        let matched = rows.len();
        rows.truncate(self.limit);

        println!(
            "archives: {}  matched: {}  shown: {}",
            paks.paths().len(),
            matched,
            rows.len()
        );
        let mut table = Table::new(["Pak", "Method", "Size", "Name"]);
        for row in rows {
            table.push([row.pak, row.method, row.size, row.name]);
        }
        if table.is_empty() {
            println!("no path matches");
        } else {
            print!("{table}");
        }

        ScanIssues::new("asset path search", skipped, cancelled, errors).finish()
    }

    fn scan_pak(paks: &PakSet, path: &Path, query: &TextQuery) -> Result<Vec<PathHit>> {
        let pak = PakMmapReader::open(path)?;
        let pak_name = paks.relative(path);
        let mut rows = Vec::new();

        for entry in pak.entries() {
            if !query.matches(entry.name()) {
                continue;
            }
            rows.push(PathHit {
                pak: pak_name.clone(),
                method: entry.compression().to_string(),
                size: format_size(entry.uncompressed_size(), DECIMAL),
                name: entry.name().to_string(),
            });
        }

        Ok(rows)
    }
}

impl SearchObjectStream {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let root = self.root.resolve()?;
        let paks = PakSet::collect(root, self.paks)?;
        let lookup = load_lookup(self.no_names)?;
        let query = TextQuery::new(self.query, false, false);
        let batch = ctx
            .runner
            .map_until_cancelled(paks.paths(), &ctx.cancel, |path| {
                Self::scan_pak(&paks, path, &query, self.max_entry_size, lookup.as_ref())
            });
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut rows = Vec::new();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(mut hits) => rows.append(&mut hits),
                Err(error) => errors.push(error),
            }
        }

        rows.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then(right.count.cmp(&left.count))
                .then(left.pak.cmp(&right.pak))
                .then(left.name.cmp(&right.name))
                .then(left.kind.cmp(&right.kind))
                .then(left.value.cmp(&right.value))
        });
        let matched = rows.len();
        rows.truncate(self.limit);

        println!(
            "archives: {}  matched: {}  shown: {}  names: {}",
            paks.paths().len(),
            matched,
            rows.len(),
            if lookup.is_some() { "loaded" } else { "off" }
        );
        let mut table = Table::new(["Pak", "Name", "AZCS", "Kind", "Count", "Score", "Value"]);
        for row in rows {
            table.push([
                row.pak,
                row.name,
                row.envelope,
                row.kind,
                row.count.to_string(),
                row.score.to_string(),
                row.value,
            ]);
        }
        if table.is_empty() {
            println!("no ObjectStream matches");
        } else {
            print!("{table}");
        }

        ScanIssues::new("asset objectstream search", skipped, cancelled, errors).finish()
    }

    fn scan_pak(
        paks: &PakSet,
        path: &Path,
        query: &TextQuery,
        max_entry_size: u64,
        lookup: Option<&NameLookup>,
    ) -> Result<Vec<ObjectHit>> {
        let pak = PakMmapReader::open(path)?;
        let pak_name = paks.relative(path);
        let mut rows = Vec::new();

        for entry in pak.entries() {
            if entry.uncompressed_size() > max_entry_size {
                continue;
            }
            let Some(payload) = ObjectPayload::read(&pak, entry)
                .with_context(|| format!("read {} from {}", entry.name(), path.display()))?
            else {
                continue;
            };

            let hits =
                nw_objectstream::query::collect_search_matches(&payload.bytes, lookup, |value| {
                    query.score(value)
                })
                .with_context(|| format!("search {} in {}", entry.name(), path.display()))?;

            for (hit, stats) in hits {
                rows.push(ObjectHit {
                    pak: pak_name.clone(),
                    name: entry.name().to_string(),
                    envelope: Cell::yes_no(payload.envelope),
                    kind: hit.kind.label().to_string(),
                    count: stats.count,
                    score: stats.score,
                    value: Cell::trim(hit.value),
                });
            }
        }

        Ok(rows)
    }
}

impl Extract {
    fn run(self) -> Result<()> {
        match self.command {
            ExtractCmd::Ext(cmd) => cmd.run(),
            ExtractCmd::ObjectStream(cmd) => cmd.run(),
        }
    }
}

impl Update {
    fn run(self) -> Result<()> {
        match self.command {
            UpdateCmd::ObjectStream(cmd) => cmd.run(),
        }
    }
}

impl UpdateObjectStream {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let lookup = load_lookup(self.no_names)?;
        let pak = PakMmapReader::open(&self.input_pak)?;
        let entry = pak
            .entry(&self.entry)
            .with_context(|| format!("pak entry not found: {}", self.entry))?;
        let entry_name = entry.name().to_string();
        let original = ObjectPayload::read(&pak, entry)?
            .with_context(|| format!("{} is not an ObjectStream payload", entry.name()))?;
        let replacement =
            std::fs::read(&self.input).with_context(|| format!("read {}", self.input.display()))?;
        let replacement = ObjectPayload::from_wrapped(replacement)?
            .with_context(|| format!("{} is not an ObjectStream payload", self.input.display()))?;
        let bytes = replacement
            .into_encoding(original.encoding, lookup.as_ref())
            .with_context(|| format!("encode replacement as {}", original.encoding))?;

        drop(pak);
        let report = crypak::Updater::new(self.input_pak, self.output_pak)
            .patch(crypak::Patch::new(entry_name, bytes).azcs(crypak::AzcsMode::Preserve))
            .run(&ctx.runner, &ctx.cancel)?;
        println!(
            "wrote {} entries, changed {}, {}",
            report.entries,
            report.changed,
            format_size(report.bytes_written, DECIMAL)
        );
        Ok(())
    }
}

impl ExtractExt {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let root = self.root.resolve()?;
        let paks = PakSet::collect(root, self.paks)?;
        let extension = Extension::new(&self.extension);
        let batch = ctx
            .runner
            .map_until_cancelled(paks.paths(), &ctx.cancel, |path| {
                Self::extract_pak(&paks, path, &extension, &self.out, self.overwrite)
            });
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut report = ExtractReport::default();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(scan) => report.merge(scan, self.limit),
                Err(error) => errors.push(error),
            }
        }

        report.print("extension extract", self.limit);
        ScanIssues::new("asset extension extract", skipped, cancelled, errors).finish()
    }

    fn extract_pak(
        paks: &PakSet,
        path: &Path,
        extension: &Extension,
        out: &Path,
        overwrite: bool,
    ) -> Result<ExtractReport> {
        let pak = PakMmapReader::open(path)?;
        let pak_name = paks.relative(path);
        let mut report = ExtractReport::default();

        for entry in pak.entries() {
            if !extension.matches(entry.name()) {
                continue;
            }
            report.matched += 1;
            let bytes = pak
                .read_by_index(entry.index())
                .with_context(|| format!("read {} from {}", entry.name(), path.display()))?;
            let target = ExtractTarget::new(out, &pak_name, entry.name())?;
            if report.write(&target, &bytes, overwrite)? == WriteOutcome::Written {
                report.rows.push(ExtractRow {
                    pak: pak_name.clone(),
                    size: format_size(bytes.len(), DECIMAL),
                    path: target.display(),
                });
            }
        }

        Ok(report)
    }
}

impl ExtractObjectStream {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let root = self.root.resolve()?;
        let paks = PakSet::collect(root, self.paks)?;
        let lookup = load_lookup(self.no_names)?;
        let encoding = ObjectStreamEncoding::from(self.encoding);
        let batch = ctx
            .runner
            .map_until_cancelled(paks.paths(), &ctx.cancel, |path| {
                Self::extract_pak(
                    &paks,
                    path,
                    &self.out,
                    encoding,
                    self.max_entry_size,
                    lookup.as_ref(),
                    self.overwrite,
                )
            });
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut report = ExtractReport::default();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(scan) => report.merge(scan, self.limit),
                Err(error) => errors.push(error),
            }
        }

        report.print("ObjectStream extract", self.limit);
        ScanIssues::new("asset objectstream extract", skipped, cancelled, errors).finish()
    }

    fn extract_pak(
        paks: &PakSet,
        path: &Path,
        out: &Path,
        encoding: ObjectStreamEncoding,
        max_entry_size: u64,
        lookup: Option<&NameLookup>,
        overwrite: bool,
    ) -> Result<ExtractReport> {
        let pak = PakMmapReader::open(path)?;
        let pak_name = paks.relative(path);
        let mut report = ExtractReport::default();

        for entry in pak.entries() {
            if entry.uncompressed_size() > max_entry_size {
                continue;
            }
            let Some(payload) = ObjectPayload::read(&pak, entry)
                .with_context(|| format!("read {} from {}", entry.name(), path.display()))?
            else {
                continue;
            };
            report.matched += 1;

            let bytes = payload
                .into_encoding(encoding, lookup)
                .with_context(|| format!("transcode {} from {}", entry.name(), path.display()))?;
            let target = ExtractTarget::with_added_extension(
                out,
                &pak_name,
                entry.name(),
                EncodingArg::extension_for(encoding),
            )?;
            if report.write(&target, &bytes, overwrite)? == WriteOutcome::Written {
                report.rows.push(ExtractRow {
                    pak: pak_name.clone(),
                    size: format_size(bytes.len(), DECIMAL),
                    path: target.display(),
                });
            }
        }

        Ok(report)
    }
}

impl InventoryReport {
    fn from_pak(
        paks: &PakSet,
        path: &Path,
        group: InventoryGroup,
        max_entry_size: u64,
    ) -> Result<Self> {
        let pak = PakMmapReader::open(path)?;
        let pak_name = paks.relative(path);
        let mut report = Self {
            paks: 1,
            ..Self::default()
        };

        for entry in pak.entries() {
            let key = inventory_key(&pak, entry, group, max_entry_size)
                .with_context(|| format!("classify {} in {}", entry.name(), path.display()))?;
            report.add(&pak_name, entry, key);
        }

        Ok(report)
    }

    fn add(&mut self, pak: &str, entry: EntryInfo<'_>, key: String) {
        self.entries += 1;
        let stat = self
            .stats
            .entry(key.clone())
            .or_insert_with(|| InventoryStat::new(key, entry.name()));
        stat.add(pak, entry);
    }

    fn merge(&mut self, other: Self) {
        self.paks += other.paks;
        self.entries += other.entries;
        for (key, incoming) in other.stats {
            self.stats
                .entry(key)
                .and_modify(|stat| stat.merge(&incoming))
                .or_insert(incoming);
        }
    }

    fn rows(&self, sort: InventorySort) -> Vec<InventoryRow> {
        let mut rows = self
            .stats
            .values()
            .map(InventoryRow::from)
            .collect::<Vec<_>>();
        match sort {
            InventorySort::Count => rows.sort_by(|left, right| {
                right
                    .entries
                    .cmp(&left.entries)
                    .then(left.key.cmp(&right.key))
            }),
            InventorySort::Size => rows.sort_by(|left, right| {
                right
                    .unpacked_bytes
                    .cmp(&left.unpacked_bytes)
                    .then(left.key.cmp(&right.key))
            }),
            InventorySort::Packed => rows.sort_by(|left, right| {
                right
                    .packed_bytes
                    .cmp(&left.packed_bytes)
                    .then(left.key.cmp(&right.key))
            }),
            InventorySort::Key => rows.sort_by(|left, right| left.key.cmp(&right.key)),
        }
        rows
    }

    fn print(&self, sort: InventorySort, limit: usize) {
        println!(
            "archives: {}  entries: {}  groups: {}",
            self.paks,
            self.entries,
            self.stats.len()
        );
        let mut table = Table::new(["Key", "Entries", "Unpacked", "Packed", "Paks", "Sample"]);
        for row in self.rows(sort).into_iter().take(limit) {
            table.push([
                row.key,
                row.entries.to_string(),
                format_size(row.unpacked_bytes, DECIMAL),
                format_size(row.packed_bytes, DECIMAL),
                row.paks.to_string(),
                row.sample,
            ]);
        }
        if table.is_empty() {
            println!("no entries");
        } else {
            print!("{table}");
        }
        if self.stats.len() > limit {
            println!("... {} more group(s)", self.stats.len() - limit);
        }
    }
}

fn inventory_key(
    pak: &PakMmapReader,
    entry: EntryInfo<'_>,
    group: InventoryGroup,
    max_entry_size: u64,
) -> Result<String> {
    match group {
        InventoryGroup::Ext => Ok(nw_filesystem::archive_extension_key(entry.name())
            .unwrap_or_else(|| "<none>".to_string())),
        InventoryGroup::Kind => classify_entry(pak, entry, max_entry_size),
    }
}

fn classify_entry(
    pak: &PakMmapReader,
    entry: EntryInfo<'_>,
    max_entry_size: u64,
) -> Result<String> {
    if entry.compression() == Compression::Oodle {
        return Ok("oodle".to_string());
    }

    if entry.uncompressed_size() > max_entry_size {
        return Ok(format!("large/{}", shape::path_family(entry.name())));
    }

    let wrapped = pak.read_wrapped_by_index(entry.index())?;
    let wrapped_azcs = azcs::is_azcs(&wrapped);
    if nw_dds::is_dds_name(entry.name()) {
        return Ok(nw_dds::Asset::parse(entry.name(), &wrapped).map_or_else(
            |_| "texture/dds:unparsed".to_string(),
            |asset| match asset.kind() {
                nw_dds::AssetKind::Header(dds) => format!("texture/dds:{}", dds.format_name()),
                nw_dds::AssetKind::Split(payload) => {
                    format!("texture/dds:{}", payload.part())
                }
            },
        ));
    }
    if let Some(payload) = ObjectPayload::from_wrapped(wrapped)? {
        let prefix = if payload.envelope {
            "azcs/objectstream"
        } else {
            "objectstream"
        };
        return Ok(format!("{prefix}:{}", payload.encoding));
    }
    if wrapped_azcs {
        return Ok("azcs".to_string());
    }
    if nw_catalog::is_asset_catalog_path(Path::new(entry.name())) {
        return Ok("catalog".to_string());
    }
    if nw_datasheet::is_datasheet_path(Path::new(entry.name())) {
        return Ok("datasheet".to_string());
    }
    Ok(shape::path_family(entry.name()).to_string())
}

impl InventoryStat {
    fn new(key: String, sample: &str) -> Self {
        Self {
            key,
            entries: 0,
            unpacked_bytes: 0,
            packed_bytes: 0,
            paks: BTreeSet::new(),
            sample: sample.to_string(),
        }
    }

    fn add(&mut self, pak: &str, entry: EntryInfo<'_>) {
        self.entries += 1;
        self.unpacked_bytes += entry.uncompressed_size();
        self.packed_bytes += entry.compressed_size();
        self.paks.insert(pak.to_string());
        if self.sample.is_empty() {
            self.sample = entry.name().to_string();
        }
    }

    fn merge(&mut self, other: &Self) {
        self.entries += other.entries;
        self.unpacked_bytes += other.unpacked_bytes;
        self.packed_bytes += other.packed_bytes;
        self.paks.extend(other.paks.iter().cloned());
        if self.sample.is_empty() {
            self.sample.clone_from(&other.sample);
        }
    }
}

impl From<&InventoryStat> for InventoryRow {
    fn from(value: &InventoryStat) -> Self {
        Self {
            key: value.key.clone(),
            entries: value.entries,
            unpacked_bytes: value.unpacked_bytes,
            packed_bytes: value.packed_bytes,
            paks: value.paks.len(),
            sample: value.sample.clone(),
        }
    }
}

impl TextQuery {
    fn new(raw: String, case_sensitive: bool, glob: bool) -> Self {
        Self {
            lowered: raw.to_ascii_lowercase(),
            glob: glob.then(|| GlobSet::archive(vec![raw.clone()])),
            raw,
            case_sensitive,
        }
    }

    fn matches(&self, value: &str) -> bool {
        if let Some(glob) = &self.glob {
            return glob.matches(value);
        }
        if self.case_sensitive {
            value.contains(&self.raw)
        } else {
            value.to_ascii_lowercase().contains(&self.lowered)
        }
    }

    fn score(&self, value: &str) -> Option<u32> {
        self.matches(value).then_some(1)
    }
}

impl ObjectPayload {
    fn read(pak: &PakMmapReader, entry: EntryInfo<'_>) -> Result<Option<Self>> {
        let bytes = pak.read_wrapped_by_index(entry.index())?;
        Self::from_wrapped(bytes)
    }

    fn from_wrapped(bytes: Vec<u8>) -> Result<Option<Self>> {
        if let Some(encoding) = nw_objectstream::sniff_encoding(&bytes) {
            return Ok(Some(Self {
                bytes,
                envelope: false,
                encoding,
            }));
        }

        if !azcs::is_azcs(&bytes) {
            return Ok(None);
        }

        let mut cursor = Cursor::new(bytes);
        let mut reader = azcs::decompress(&mut cursor)?;
        let mut decoded = Vec::new();
        reader.read_to_end(&mut decoded)?;
        Ok(
            nw_objectstream::sniff_encoding(&decoded).map(|encoding| Self {
                bytes: decoded,
                envelope: true,
                encoding,
            }),
        )
    }

    fn into_encoding(
        self,
        encoding: ObjectStreamEncoding,
        lookup: Option<&NameLookup>,
    ) -> Result<Vec<u8>, nw_objectstream::ObjectStreamError> {
        if self.encoding == encoding {
            return Ok(self.bytes);
        }
        ObjectStream::transcode_bytes(&self.bytes, encoding, lookup)
    }
}

#[derive(Debug, Clone)]
struct Extension {
    value: String,
}

impl Extension {
    fn new(value: &str) -> Self {
        Self {
            value: value.trim_start_matches('.').trim().to_ascii_lowercase(),
        }
    }

    fn matches(&self, name: &str) -> bool {
        nw_filesystem::archive_extension_key(name).is_some_and(|extension| extension == self.value)
    }
}

#[derive(Debug, Clone)]
struct ExtractTarget {
    path: PathBuf,
}

impl ExtractTarget {
    fn new(root: &Path, pak: &str, entry: &str) -> Result<Self> {
        Self::with_added_extension(root, pak, entry, None)
    }

    fn with_added_extension(
        root: &Path,
        pak: &str,
        entry: &str,
        extension: Option<&'static str>,
    ) -> Result<Self> {
        let entry = nw_filesystem::normalize_archive_path(entry);
        let mut relative = format!("{}/{}", pak.replace('\\', "/"), entry);
        if let Some(extension) = extension {
            relative.push('.');
            relative.push_str(extension);
        }
        let path = nw_filesystem::safe_join(root, &relative)?;
        Ok(Self { path })
    }

    fn display(&self) -> String {
        self.path.display().to_string()
    }
}

impl ExtractReport {
    fn write(
        &mut self,
        target: &ExtractTarget,
        bytes: &[u8],
        overwrite: bool,
    ) -> Result<WriteOutcome> {
        if target.path.exists() && !overwrite {
            self.skipped += 1;
            return Ok(WriteOutcome::Skipped);
        }
        if let Some(parent) = target.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target.path, bytes)?;
        self.written += 1;
        self.bytes_written += bytes.len() as u64;
        Ok(WriteOutcome::Written)
    }

    fn merge(&mut self, mut other: Self, row_limit: usize) {
        self.matched += other.matched;
        self.written += other.written;
        self.skipped += other.skipped;
        self.bytes_written += other.bytes_written;
        let remaining = row_limit.saturating_sub(self.rows.len());
        self.rows.extend(other.rows.drain(..remaining));
    }

    fn print(&self, label: &str, limit: usize) {
        println!(
            "{label}: matched {}  written {}  skipped {}  bytes {}",
            self.matched,
            self.written,
            self.skipped,
            format_size(self.bytes_written, DECIMAL)
        );
        let mut table = Table::new(["Pak", "Size", "Path"]);
        for row in &self.rows {
            table.push([row.pak.clone(), row.size.clone(), row.path.clone()]);
        }
        if !table.is_empty() {
            print!("{table}");
        }
        let remaining = self.written.saturating_sub(limit as u64);
        if remaining > 0 {
            println!("... {remaining} more file(s)");
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteOutcome {
    Written,
    Skipped,
}

impl EncodingArg {
    fn extension_for(encoding: ObjectStreamEncoding) -> Option<&'static str> {
        match encoding {
            ObjectStreamEncoding::Binary => None,
            ObjectStreamEncoding::Xml => Some("xml"),
            ObjectStreamEncoding::Json => Some("json"),
        }
    }
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

struct Cell;

impl Cell {
    fn trim(value: impl AsRef<str>) -> String {
        const MAX: usize = 160;
        let value = value.as_ref().replace(['\r', '\n', '\t'], " ");
        if value.chars().count() <= MAX {
            value
        } else {
            format!("{}...", value.chars().take(MAX).collect::<String>())
        }
    }

    fn yes_no(value: bool) -> String {
        if value {
            "yes".to_string()
        } else {
            "-".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_matches_observed_archive_key() {
        let extension = Extension::new(".SLICE");

        assert!(extension.matches("slices/player.slice"));
        assert!(!extension.matches("slices/player.dds"));
    }

    #[test]
    fn path_query_supports_case_modes_and_globs() {
        let insensitive = TextQuery::new("Player".to_string(), false, false);
        let sensitive = TextQuery::new("Player".to_string(), true, false);
        let glob = TextQuery::new("*/player.*".to_string(), false, true);

        assert!(insensitive.matches("slices/player.slice"));
        assert!(!sensitive.matches("slices/player.slice"));
        assert!(glob.matches("slices/player.slice"));
    }

    #[test]
    fn object_payload_sniffs_raw_objectstream_without_extension_hint() -> Result<()> {
        let bytes = ObjectStream::new(3).to_bytes();
        let payload = ObjectPayload::from_wrapped(bytes)?.expect("objectstream");

        assert!(!payload.envelope);
        assert_eq!(payload.encoding, ObjectStreamEncoding::Binary);
        Ok(())
    }
}
