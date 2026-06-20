use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use humansize::{DECIMAL, format_size};
use nw_pak::{Compression, PakFile, PakMmapReader, azcs, crypak, oodle, shape};

use crate::jobs::JobArgs;
use crate::output::Table;
use crate::support::{GlobSet, collect_paks};

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
    root: PathBuf,

    #[arg(long, value_enum)]
    method: Option<MethodArg>,

    #[arg(long, value_enum)]
    family: Option<FamilyArg>,

    #[arg(long)]
    name: Vec<String>,

    #[arg(long)]
    azcs: bool,

    #[arg(long)]
    limit: Option<usize>,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Shape {
    root: PathBuf,

    #[arg(long, default_value_t = 20)]
    samples: usize,

    #[arg(long)]
    azcs: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Args)]
pub struct Extract {
    pak: PathBuf,
    out: PathBuf,

    #[arg(long)]
    filter: Option<String>,

    #[arg(long)]
    glob: Vec<String>,

    #[arg(long)]
    overwrite: bool,

    #[arg(long)]
    sequential: bool,

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
    azcs: String,
    size: String,
    name: String,
}

impl List {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let paks = collect_paks(&self.root)?;
        let filter = ListFilter {
            method: self.method,
            family: self.family,
            names: GlobSet::archive(self.name),
            azcs: self.azcs,
        };
        let root = self.root;
        let batch = ctx
            .runner
            .map_until_cancelled(&paks, &ctx.cancel, |pak| scan_entries(&root, pak, &filter));
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
        if let Some(limit) = self.limit {
            rows.truncate(limit);
        }

        println!(
            "archives: {}  matched: {}  shown: {}",
            paks.len(),
            total_rows,
            rows.len()
        );
        let mut table = Table::new(["Pak", "Method", "Family", "AZCS", "Size", "Name"]);
        for row in rows {
            table.push([
                row.pak, row.method, row.family, row.azcs, row.size, row.name,
            ]);
        }
        if table.is_empty() {
            println!("no matching entries");
        } else {
            print!("{table}");
        }

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

impl Shape {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let report = shape::Scanner::new()
            .max_samples(self.samples)
            .azcs(self.azcs)
            .scan_with(self.root, &ctx.runner, &ctx.cancel)?;
        println!("{report}");
        Ok(())
    }
}

impl Extract {
    fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let globs = self.glob.iter().map(String::as_str).collect::<Vec<_>>();
        let options = nw_pak::PakExtractOptions {
            filter: self.filter.as_deref(),
            globs: &globs,
            sequential: self.sequential,
            overwrite: self.overwrite,
        };
        let report =
            PakFile::extract_to_dir_with(self.pak, self.out, options, &ctx.runner, &ctx.cancel)?;
        println!("{report}");
        report.ensure_success()?;
        Ok(())
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

        let report = crypak::Repacker::new(self.input_dir, self.output_pak)
            .options(options)
            .run(&ctx.runner, &ctx.cancel)?;
        println!(
            "wrote {} entries, {}",
            report.entries,
            format_size(report.bytes_written, DECIMAL)
        );
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

        let report = crypak::Updater::new(self.input_pak, self.output_pak)
            .options(options)
            .patches(patches)
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

fn scan_entries(root: &Path, pak_path: &Path, filter: &ListFilter) -> Result<Vec<EntryRow>> {
    let pak = PakMmapReader::open(pak_path)?;
    let pak_name = nw_filesystem::display_relative(root, pak_path);
    let mut rows = Vec::new();

    for entry in pak.entries() {
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
            "yes"
        } else {
            "-"
        };

        rows.push(EntryRow {
            pak: pak_name.clone(),
            method: entry.compression().to_string(),
            family: family.to_string(),
            azcs: azcs.to_string(),
            size: format_size(entry.uncompressed_size(), DECIMAL),
            name: entry.name().to_string(),
        });
    }

    Ok(rows)
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
