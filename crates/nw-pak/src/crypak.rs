use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use crc32fast::Hasher;
use flate2::Compression as FlateCompression;
use flate2::write::DeflateEncoder;
use nw_filesystem::{display_relative, normalize_archive_path};
use nw_jobs::{CancellationToken, JobRunner};
use thiserror::Error;

use crate::{Compression, EntryInfo, PakMmapReader, azcs, crypak_format, oodle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Method {
    #[default]
    Store,
    Deflate,
    Oodle(oodle::Options),
}

impl Method {
    #[must_use]
    pub const fn id(self) -> u16 {
        match self {
            Self::Store => 0,
            Self::Deflate => 8,
            Self::Oodle(_) => 15,
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store => f.write_str("store"),
            Self::Deflate => f.write_str("deflate"),
            Self::Oodle(options) => {
                write!(f, "oodle:{}:{}", options.compressor, options.level)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Extra {
    #[default]
    Auto,
    None,
    Marker(u8),
    Raw(Vec<u8>),
}

impl Extra {
    #[must_use]
    pub fn bytes_for(&self, method: Method) -> Vec<u8> {
        match self {
            Self::Auto => default_extra(method),
            Self::None => Vec::new(),
            Self::Marker(marker) => crypak_format::marker_extra(*marker).to_vec(),
            Self::Raw(bytes) => bytes.clone(),
        }
    }
}

fn default_extra(method: Method) -> Vec<u8> {
    match method {
        Method::Store | Method::Oodle(_) => crypak_format::marker_extra(0x15).to_vec(),
        Method::Deflate => Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Level {
    Fastest,
    Faster,
    #[default]
    Default,
    Normal,
    Better,
    Best,
}

impl Level {
    fn flate2(self) -> FlateCompression {
        match self {
            Self::Fastest => FlateCompression::new(0),
            Self::Faster => FlateCompression::new(2),
            Self::Default => FlateCompression::default(),
            Self::Normal | Self::Better => FlateCompression::new(8),
            Self::Best => FlateCompression::new(9),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub method: Method,
    pub extra: Extra,
    pub level: Level,
    pub oodle: oodle::Options,
    pub oodle_patterns: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            method: Method::Store,
            extra: Extra::Auto,
            level: Level::default(),
            oodle: oodle::Options::default(),
            oodle_patterns: Vec::new(),
        }
    }
}

impl Options {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    #[must_use]
    pub fn extra(mut self, extra: Extra) -> Self {
        self.extra = extra;
        self
    }

    #[must_use]
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    #[must_use]
    pub fn oodle(mut self, options: oodle::Options) -> Self {
        self.oodle = options;
        self
    }

    #[must_use]
    pub fn oodle_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.oodle_patterns
            .push(normalize_crypak_path(&pattern.into()));
        self
    }

    #[must_use]
    pub fn method_for(&self, archive_path: &str) -> Method {
        if self
            .oodle_patterns
            .iter()
            .any(|pattern| wildcard_matches(pattern, archive_path))
        {
            return Method::Oodle(self.oodle);
        }

        self.method
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
    pub method: Method,
    pub extra: Extra,
}

impl Entry {
    #[must_use]
    pub fn new(name: impl Into<String>, data: Vec<u8>, method: Method) -> Self {
        Self {
            name: normalize_crypak_path(&name.into()),
            data,
            method,
            extra: Extra::Auto,
        }
    }

    #[must_use]
    pub fn extra(mut self, extra: Extra) -> Self {
        self.extra = extra;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub entries: usize,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateReport {
    pub entries: usize,
    pub changed: usize,
    pub bytes_written: u64,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("oodle error: {0}")]
    Oodle(#[from] oodle::Error),

    #[error("azcs error: {0}")]
    Azcs(#[from] azcs::AzcsError),

    #[error("pak error: {0}")]
    Pak(#[from] crate::PakError),

    #[error("archive path is empty")]
    EmptyArchivePath,

    #[error("archive path is too long: {0}")]
    NameTooLong(String),

    #[error("archive contains too many files: {0}")]
    TooManyFiles(usize),

    #[error("CryPak format limit exceeded for {field}: {value}")]
    FormatLimit { field: &'static str, value: u64 },

    #[error("repack cancelled ({skipped} queued item(s) skipped)")]
    Cancelled { skipped: usize },

    #[error("cannot preserve unsupported source compression method {method} for {entry}")]
    UnsupportedSourceMethod { entry: String, method: u16 },

    #[error("update item is missing both source entry and patch data: {0}")]
    MissingUpdateData(String),
}

impl From<crypak_format::Error> for Error {
    fn from(value: crypak_format::Error) -> Self {
        match value {
            crypak_format::Error::Io(error) => Self::Io(error),
            crypak_format::Error::Limit { field, value } => Self::FormatLimit { field, value },
        }
    }
}

#[derive(Debug, Clone)]
struct WorkItem {
    absolute_path: PathBuf,
    archive_path: String,
    method: Method,
    extra: Extra,
    level: Level,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AzcsMode {
    #[default]
    Preserve,
    Raw,
    Plain,
    Zlib,
}

impl AzcsMode {
    const fn needs_source(self) -> bool {
        matches!(self, Self::Preserve)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    name: String,
    data: Vec<u8>,
    method: Option<Method>,
    extra: Option<Extra>,
    azcs: AzcsMode,
}

impl Patch {
    #[must_use]
    pub fn new(name: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            name: normalize_crypak_path(&name.into()),
            data,
            method: None,
            extra: None,
            azcs: AzcsMode::Preserve,
        }
    }

    #[must_use]
    pub fn method(mut self, method: Method) -> Self {
        self.method = Some(method);
        self
    }

    #[must_use]
    pub fn extra(mut self, extra: Extra) -> Self {
        self.extra = Some(extra);
        self
    }

    #[must_use]
    pub fn azcs(mut self, mode: AzcsMode) -> Self {
        self.azcs = mode;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
struct UpdateItem {
    source_index: Option<usize>,
    name: String,
    patch: Option<Patch>,
    source_method: Option<Method>,
    source_extra: Option<Vec<u8>>,
    source_modified: Option<crypak_format::DosTime>,
    default_method: Method,
    default_extra: Extra,
    level: Level,
}

#[derive(Debug, Clone)]
struct PreparedEntry {
    name: String,
    method: Method,
    modified: crypak_format::DosTime,
    crc32: u32,
    uncompressed_size: u32,
    compressed_size: u32,
    central_extra: Vec<u8>,
    compressed_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Repacker {
    input_dir: PathBuf,
    output_pak: PathBuf,
    options: Options,
}

#[derive(Debug, Clone)]
pub struct Updater {
    input_pak: PathBuf,
    output_pak: PathBuf,
    options: Options,
    patches: Vec<Patch>,
}

impl Updater {
    #[must_use]
    pub fn new(input_pak: impl Into<PathBuf>, output_pak: impl Into<PathBuf>) -> Self {
        Self {
            input_pak: input_pak.into(),
            output_pak: output_pak.into(),
            options: Options::default(),
            patches: Vec::new(),
        }
    }

    #[must_use]
    pub fn options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    #[must_use]
    pub fn patch(mut self, patch: Patch) -> Self {
        self.patches.push(patch);
        self
    }

    #[must_use]
    pub fn patches(mut self, patches: impl IntoIterator<Item = Patch>) -> Self {
        self.patches.extend(patches);
        self
    }

    pub fn run(
        self,
        runner: &JobRunner,
        cancel: &CancellationToken,
    ) -> Result<UpdateReport, Error> {
        let Self {
            input_pak,
            output_pak,
            options,
            patches,
        } = self;
        update_pak(&input_pak, &output_pak, &options, patches, runner, cancel)
    }
}

impl Repacker {
    #[must_use]
    pub fn new(input_dir: impl Into<PathBuf>, output_pak: impl Into<PathBuf>) -> Self {
        Self {
            input_dir: input_dir.into(),
            output_pak: output_pak.into(),
            options: Options::default(),
        }
    }

    #[must_use]
    pub fn options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    #[must_use]
    pub fn method(mut self, method: Method) -> Self {
        self.options.method = method;
        self
    }

    #[must_use]
    pub fn extra(mut self, extra: Extra) -> Self {
        self.options.extra = extra;
        self
    }

    #[must_use]
    pub fn level(mut self, level: Level) -> Self {
        self.options.level = level;
        self
    }

    #[must_use]
    pub fn oodle(mut self, options: oodle::Options) -> Self {
        self.options.oodle = options;
        self
    }

    #[must_use]
    pub fn oodle_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.options
            .oodle_patterns
            .push(normalize_crypak_path(&pattern.into()));
        self
    }

    pub fn run(self, runner: &JobRunner, cancel: &CancellationToken) -> Result<Report, Error> {
        let Self {
            input_dir,
            output_pak,
            options,
        } = self;
        repack_directory(&input_dir, &output_pak, &options, runner, cancel)
    }
}

#[derive(Debug)]
pub struct Writer<W> {
    inner: W,
    level: Level,
}

impl<W: Write> Writer<W> {
    #[must_use]
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            level: Level::default(),
        }
    }

    #[must_use]
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    pub fn write<I>(&mut self, entries: I) -> Result<Report, Error>
    where
        I: IntoIterator<Item = Entry>,
    {
        write_entries(&mut self.inner, entries, self.level)
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

fn repack_directory(
    input_dir: impl AsRef<Path>,
    output_pak: impl AsRef<Path>,
    options: &Options,
    runner: &JobRunner,
    cancel: &CancellationToken,
) -> Result<Report, Error> {
    let input_dir = input_dir.as_ref();
    let output_pak = output_pak.as_ref();
    let work = collect_work(input_dir, options, cancel)?;

    let batch = runner.map_until_cancelled(&work, cancel, prepare_work_item);
    let cancelled = batch.was_cancelled();
    let skipped = batch.skipped();
    let mut prepared = Vec::with_capacity(batch.completed().len());
    for result in batch.into_completed() {
        prepared.push(result?);
    }

    if cancelled {
        return Err(Error::Cancelled { skipped });
    }

    prepared.sort_by(|left, right| left.name.cmp(&right.name));

    if let Some(parent) = output_pak.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let mut output = File::create(output_pak)?;
    write_prepared_entries_with_cancel(&mut output, prepared, Some(cancel))
}

fn update_pak(
    input_pak: &Path,
    output_pak: &Path,
    options: &Options,
    patches: Vec<Patch>,
    runner: &JobRunner,
    cancel: &CancellationToken,
) -> Result<UpdateReport, Error> {
    let in_place = same_file(input_pak, output_pak);
    let write_path = if in_place {
        temp_update_path(output_pak)
    } else {
        output_pak.to_path_buf()
    };

    let report = {
        let reader = PakMmapReader::open(input_pak)?;
        let mut pending = patches
            .into_iter()
            .map(|patch| (patch.name.clone(), patch))
            .collect::<HashMap<_, _>>();
        let mut work = Vec::with_capacity(reader.len() + pending.len());

        for (index, entry) in reader.entries().enumerate() {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled {
                    skipped: reader.len().saturating_sub(index),
                });
            }
            let patch = pending.remove(entry.name());
            work.push(UpdateItem {
                source_index: Some(entry.index()),
                name: entry.name().to_string(),
                source_method: Some(method_for_entry(entry, options.oodle)?),
                source_extra: Some(entry.central_extra().to_vec()),
                source_modified: Some(crypak_format::DosTime::from_raw(
                    entry.modified_time(),
                    entry.modified_date(),
                )),
                default_method: options.method_for(entry.name()),
                default_extra: options.extra.clone(),
                level: options.level,
                patch,
            });
        }

        let mut inserts = pending.into_values().collect::<Vec<_>>();
        inserts.sort_by(|left, right| left.name.cmp(&right.name));
        for patch in inserts {
            work.push(UpdateItem {
                source_index: None,
                default_method: options.method_for(&patch.name),
                default_extra: options.extra.clone(),
                level: options.level,
                name: patch.name.clone(),
                patch: Some(patch),
                source_method: None,
                source_extra: None,
                source_modified: None,
            });
        }

        let batch =
            runner.map_until_cancelled(&work, cancel, |item| prepare_update_item(&reader, item));
        let cancelled = batch.was_cancelled();
        let skipped = batch.skipped();
        let mut prepared = Vec::with_capacity(batch.completed().len());
        for result in batch.into_completed() {
            prepared.push(result?);
        }

        if cancelled {
            return Err(Error::Cancelled { skipped });
        }

        if let Some(parent) = write_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        let changed = work.iter().filter(|item| item.patch.is_some()).count();
        let mut output = File::create(&write_path)?;
        let report = write_prepared_entries_with_cancel(&mut output, prepared, Some(cancel))?;
        UpdateReport {
            entries: report.entries,
            changed,
            bytes_written: report.bytes_written,
        }
    };

    if in_place {
        replace_with_temp(&write_path, output_pak)?;
    }

    Ok(report)
}

fn write_entries<W, I>(writer: &mut W, entries: I, level: Level) -> Result<Report, Error>
where
    W: Write,
    I: IntoIterator<Item = Entry>,
{
    let mut prepared = entries
        .into_iter()
        .map(|entry| prepare_entry(entry, level))
        .collect::<Result<Vec<_>, _>>()?;
    prepared.sort_by(|left, right| left.name.cmp(&right.name));
    write_prepared_entries(writer, prepared)
}

fn collect_work(
    input_dir: &Path,
    options: &Options,
    cancel: &CancellationToken,
) -> Result<Vec<WorkItem>, Error> {
    let mut files = Vec::new();
    collect_files(input_dir, &mut files, cancel)?;
    if cancel.is_cancelled() {
        return Err(Error::Cancelled { skipped: 0 });
    }
    files.sort();

    files
        .into_iter()
        .map(|absolute_path| {
            let archive_path = normalize_crypak_path(&display_relative(input_dir, &absolute_path));
            if archive_path.is_empty() {
                return Err(Error::EmptyArchivePath);
            }
            Ok(WorkItem {
                method: options.method_for(&archive_path),
                extra: options.extra.clone(),
                level: options.level,
                absolute_path,
                archive_path,
            })
        })
        .collect()
}

fn collect_files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    if cancel.is_cancelled() {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::path);

    for entry in entries {
        if cancel.is_cancelled() {
            break;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(&entry.path(), out, cancel)?;
        } else if file_type.is_file() {
            out.push(entry.path());
        }
    }
    Ok(())
}

fn prepare_work_item(item: &WorkItem) -> Result<PreparedEntry, Error> {
    let data = fs::read(&item.absolute_path)?;
    prepare_data(
        item.archive_path.clone(),
        data,
        item.method,
        &item.extra,
        item.level,
    )
}

fn prepare_update_item(reader: &PakMmapReader, item: &UpdateItem) -> Result<PreparedEntry, Error> {
    let source_wrapped = if item.patch.is_none()
        || item
            .patch
            .as_ref()
            .is_some_and(|patch| patch.azcs.needs_source())
    {
        match item.source_index {
            Some(index) => Some(reader.read_wrapped_by_index(index)?),
            None => None,
        }
    } else {
        None
    };

    let (data, method, extra) = match &item.patch {
        Some(patch) => (
            apply_azcs_mode(patch.azcs, patch.data.clone(), source_wrapped.as_deref())?,
            patch
                .method
                .or(item.source_method)
                .unwrap_or(item.default_method),
            patch
                .extra
                .clone()
                .or_else(|| item.source_extra.clone().map(Extra::Raw))
                .unwrap_or_else(|| item.default_extra.clone()),
        ),
        None => (
            source_wrapped.ok_or_else(|| Error::MissingUpdateData(item.name.clone()))?,
            item.source_method.unwrap_or(item.default_method),
            item.source_extra
                .clone()
                .map_or_else(|| item.default_extra.clone(), Extra::Raw),
        ),
    };

    let mut prepared = prepare_data(item.name.clone(), data, method, &extra, item.level)?;
    if let Some(modified) = item.source_modified {
        prepared.modified = modified;
    }
    Ok(prepared)
}

fn apply_azcs_mode(
    mode: AzcsMode,
    data: Vec<u8>,
    source_wrapped: Option<&[u8]>,
) -> Result<Vec<u8>, Error> {
    match mode {
        AzcsMode::Raw => Ok(data),
        AzcsMode::Preserve => {
            if azcs::is_azcs(&data) || !source_wrapped.is_some_and(azcs::is_azcs) {
                Ok(data)
            } else {
                Ok(azcs::compress_zlib(&data)?)
            }
        }
        AzcsMode::Plain => {
            if azcs::is_azcs(&data) {
                peel_azcs(data)
            } else {
                Ok(data)
            }
        }
        AzcsMode::Zlib => {
            if azcs::is_azcs(&data) {
                Ok(data)
            } else {
                Ok(azcs::compress_zlib(&data)?)
            }
        }
    }
}

fn peel_azcs(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    let mut cursor = Cursor::new(data);
    let mut reader = azcs::decompress(&mut cursor)?;
    let mut decoded = Vec::new();
    reader.read_to_end(&mut decoded)?;
    Ok(decoded)
}

fn method_for_entry(entry: EntryInfo<'_>, oodle_options: oodle::Options) -> Result<Method, Error> {
    match entry.compression() {
        Compression::Stored => Ok(Method::Store),
        Compression::Deflated => Ok(Method::Deflate),
        Compression::Oodle => Ok(Method::Oodle(oodle_options)),
        Compression::Other(method) => Err(Error::UnsupportedSourceMethod {
            entry: entry.name().to_string(),
            method,
        }),
    }
}

fn prepare_entry(entry: Entry, level: Level) -> Result<PreparedEntry, Error> {
    let Entry {
        name,
        data,
        method,
        extra,
    } = entry;
    prepare_data(name, data, method, &extra, level)
}

fn prepare_data(
    name: String,
    data: Vec<u8>,
    method: Method,
    extra: &Extra,
    level: Level,
) -> Result<PreparedEntry, Error> {
    if name.is_empty() {
        return Err(Error::EmptyArchivePath);
    }
    crypak_format::checked_u16("file name length", name.len() as u64)
        .map_err(|_| Error::NameTooLong(name.clone()))?;

    let crc32 = {
        let mut hasher = Hasher::new();
        hasher.update(&data);
        hasher.finalize()
    };
    let uncompressed_size = crypak_format::checked_u32("uncompressed size", data.len() as u64)?;
    let method = if data.is_empty() {
        Method::Store
    } else {
        method
    };
    let compressed_data = match method {
        Method::Store => data,
        Method::Deflate => deflate_to_vec(&data, level)?,
        Method::Oodle(options) => oodle::Codec::new(options).compress_to_vec(&data)?,
    };
    let compressed_size =
        crypak_format::checked_u32("compressed size", compressed_data.len() as u64)?;
    let central_extra = extra.bytes_for(method);

    Ok(PreparedEntry {
        name,
        method,
        modified: crypak_format::DosTime::now(),
        crc32,
        uncompressed_size,
        compressed_size,
        central_extra,
        compressed_data,
    })
}

fn deflate_to_vec(data: &[u8], level: Level) -> Result<Vec<u8>, Error> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    let mut encoder = DeflateEncoder::new(Vec::new(), level.flate2());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

fn write_prepared_entries<W: Write>(
    writer: &mut W,
    entries: Vec<PreparedEntry>,
) -> Result<Report, Error> {
    write_prepared_entries_with_cancel(writer, entries, None)
}

fn write_prepared_entries_with_cancel<W: Write>(
    writer: &mut W,
    entries: Vec<PreparedEntry>,
    cancel: Option<&CancellationToken>,
) -> Result<Report, Error> {
    if entries.len() > u16::MAX as usize {
        return Err(Error::TooManyFiles(entries.len()));
    }

    let entry_count = entries.len();
    let mut pak = crypak_format::Writer::new(writer);

    for (index, entry) in entries.into_iter().enumerate() {
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            return Err(Error::Cancelled {
                skipped: entry_count.saturating_sub(index),
            });
        }
        pak.push(crypak_format::Entry {
            name: &entry.name,
            method: entry.method.id(),
            modified: entry.modified,
            crc32: entry.crc32,
            uncompressed_size: entry.uncompressed_size,
            compressed_size: entry.compressed_size,
            central_extra: &entry.central_extra,
            compressed_data: &entry.compressed_data,
        })?;
    }

    let (_, bytes_written) = pak.finish()?;

    Ok(Report {
        entries: entry_count,
        bytes_written,
    })
}

fn normalize_crypak_path(path: &str) -> String {
    normalize_archive_path(path)
}

fn same_file(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn temp_update_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(|| "pak".into(), std::ffi::OsStr::to_string_lossy);
    path.with_file_name(format!("{name}.nwtmp"))
}

fn backup_update_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(|| "pak".into(), std::ffi::OsStr::to_string_lossy);
    path.with_file_name(format!("{name}.nwbak"))
}

fn replace_with_temp(temp: &Path, target: &Path) -> Result<(), Error> {
    let backup = backup_update_path(target);
    if backup.exists() {
        fs::remove_file(&backup)?;
    }

    fs::rename(target, &backup)?;
    match fs::rename(temp, target) {
        Ok(()) => {
            fs::remove_file(backup)?;
            Ok(())
        }
        Err(error) => {
            let _ = fs::rename(&backup, target);
            Err(Error::Io(error))
        }
    }
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    wildcard_matches_bytes(
        pattern.to_ascii_lowercase().as_bytes(),
        value.to_ascii_lowercase().as_bytes(),
    )
}

fn wildcard_matches_bytes(pattern: &[u8], value: &[u8]) -> bool {
    let (mut pattern_index, mut value_index) = (0usize, 0usize);
    let (mut star_index, mut retry_value_index) = (None, 0usize);

    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == value[value_index])
        {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            retry_value_index = value_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            retry_value_index += 1;
            value_index = retry_value_index;
        } else {
            return false;
        }
    }

    pattern[pattern_index..].iter().all(|byte| *byte == b'*')
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::{PakFile, PakMmapReader};

    #[test]
    fn writes_store_and_deflate_crypak_entries() {
        let mut bytes = Vec::new();
        let report = {
            let mut writer = Writer::new(&mut bytes);
            writer.write([
                Entry::new("Textures\\A.txt", b"alpha".to_vec(), Method::Store),
                Entry::new("Data/B.bin", b"bbbbbbbbbbbb".to_vec(), Method::Deflate),
            ])
        }
        .unwrap();

        assert_eq!(report.entries, 2);
        assert_eq!(report.bytes_written, bytes.len() as u64);

        let root = temp_test_dir("nw-pak-repack");
        fs::create_dir_all(&root).unwrap();
        let pak_path = root.join("test.pak");
        fs::write(&pak_path, bytes).unwrap();

        let mut pak = PakFile::open(&pak_path).unwrap();
        assert_eq!(
            pak.entry("textures/a.txt").unwrap().name(),
            "textures/a.txt"
        );
        assert_eq!(pak.read("textures/a.txt").unwrap(), b"alpha");
        assert_eq!(pak.read("textures\\a.txt").unwrap(), b"alpha");
        assert_eq!(pak.read("data/b.bin").unwrap(), b"bbbbbbbbbbbb");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wildcard_patterns_match_archive_paths() {
        assert!(wildcard_matches("*.dds", "textures/foo.dds"));
        assert!(wildcard_matches("textures/*", "textures/foo.dds"));
        assert!(wildcard_matches("TEXTURES/?.DDS", "textures/a.dds"));
        assert!(!wildcard_matches("textures/?.dds", "textures/ab.dds"));
    }

    #[test]
    fn update_preserves_source_extra_by_default() {
        let root = temp_test_dir("nw-pak-update-extra");
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.pak");
        let output = root.join("updated.pak");
        let marker = crypak_format::marker_extra(0x10);

        {
            let mut file = File::create(&source).unwrap();
            let mut writer = Writer::new(&mut file);
            writer
                .write([
                    Entry::new("a.bin", b"old".to_vec(), Method::Store)
                        .extra(Extra::Raw(marker.to_vec())),
                    Entry::new("b.bin", b"same".to_vec(), Method::Deflate),
                ])
                .unwrap();
        }

        let report = Updater::new(&source, &output)
            .patch(Patch::new("a.bin", b"new".to_vec()))
            .run(&JobRunner::inline(), &CancellationToken::new())
            .unwrap();
        assert_eq!(report.entries, 2);
        assert_eq!(report.changed, 1);

        let pak = PakMmapReader::open(&output).unwrap();
        assert_eq!(
            pak.entry("a.bin").unwrap().central_extra(),
            marker.as_slice()
        );
        assert_eq!(pak.read("a.bin").unwrap(), b"new");
        assert_eq!(pak.read("b.bin").unwrap(), b"same");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_preserves_azcs_wrapper_by_default() {
        let root = temp_test_dir("nw-pak-update-azcs");
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.pak");
        let output = root.join("updated.pak");
        let old_wrapped = azcs::compress_zlib(b"old object").unwrap();

        {
            let mut file = File::create(&source).unwrap();
            let mut writer = Writer::new(&mut file);
            writer
                .write([Entry::new("asset.slice", old_wrapped, Method::Store)])
                .unwrap();
        }

        Updater::new(&source, &output)
            .patch(Patch::new("asset.slice", b"new object".to_vec()))
            .run(&JobRunner::inline(), &CancellationToken::new())
            .unwrap();

        let pak = PakMmapReader::open(&output).unwrap();
        let wrapped = pak.read_wrapped("asset.slice").unwrap();
        assert!(azcs::is_azcs(&wrapped));
        assert_eq!(pak.read("asset.slice").unwrap(), b"new object");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_supports_same_input_and_output_path() {
        let root = temp_test_dir("nw-pak-update-in-place");
        fs::create_dir_all(&root).unwrap();
        let pak_path = root.join("source.pak");

        {
            let mut file = File::create(&pak_path).unwrap();
            let mut writer = Writer::new(&mut file);
            writer
                .write([Entry::new("a.bin", b"old".to_vec(), Method::Store)])
                .unwrap();
        }

        Updater::new(&pak_path, &pak_path)
            .patch(Patch::new("a.bin", b"new".to_vec()))
            .run(&JobRunner::inline(), &CancellationToken::new())
            .unwrap();

        let pak = PakMmapReader::open(&pak_path).unwrap();
        assert_eq!(pak.read("a.bin").unwrap(), b"new");
        assert!(!temp_update_path(&pak_path).exists());
        assert!(!backup_update_path(&pak_path).exists());

        fs::remove_dir_all(root).unwrap();
    }

    fn temp_test_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }
}
