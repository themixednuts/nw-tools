use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use nw_jobs::{CancellationToken, JobRunner};
use thiserror::Error;

use crate::azcs::AzcsHeader;
use crate::decompress::{Compression, decompress_bytes_raw_into};

const LOCAL_FILE_HEADER: u32 = 0x0403_4b50;
const CENTRAL_DIRECTORY_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL_DIRECTORY: u32 = 0x0605_4b50;
const EOCD_LEN: usize = 22;
const EOCD_MAX_SEARCH: usize = 65_557;
const LOCAL_HEADER_LEN: usize = 30;
const CENTRAL_DIRECTORY_HEADER_LEN: usize = 46;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub max_samples: usize,
    pub scan_azcs: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_samples: 12,
            scan_azcs: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Scanner {
    options: Options,
}

impl Scanner {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    #[must_use]
    pub fn max_samples(mut self, max_samples: usize) -> Self {
        self.options.max_samples = max_samples;
        self
    }

    #[must_use]
    pub fn azcs(mut self, scan_azcs: bool) -> Self {
        self.options.scan_azcs = scan_azcs;
        self
    }

    pub fn scan(&self, path: impl AsRef<Path>) -> Result<Report, Error> {
        let cancel = CancellationToken::new();
        self.scan_with(path, &JobRunner::inline(), &cancel)
    }

    pub fn scan_with(
        &self,
        path: impl AsRef<Path>,
        runner: &JobRunner,
        cancel: &CancellationToken,
    ) -> Result<Report, Error> {
        let root = path.as_ref().to_path_buf();
        let mut paks = Vec::new();
        collect_paks(path.as_ref(), &mut paks)?;
        paks.sort();

        let mut report = Report::new(root, paks.len());
        let batch = runner.map_until_cancelled(&paks, cancel, |pak| {
            let mut partial = Report::new(pak.clone(), 0);
            match scan_pak(pak, &mut partial, self.options, cancel) {
                Ok(()) => {
                    partial.parsed_archives = 1;
                    Ok(partial)
                }
                Err(error) => Err(format!("{}: {error}", pak.display())),
            }
        });

        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        for result in batch.into_completed() {
            match result {
                Ok(partial) => report.merge(partial, self.options.max_samples),
                Err(error) => report.samples.push_error(error, self.options.max_samples),
            }
        }

        if cancelled {
            return Err(Error::Cancelled { skipped });
        }
        Ok(report)
    }
}

#[derive(Debug, Clone)]
pub struct Report {
    pub root: PathBuf,
    pub archives: usize,
    pub parsed_archives: usize,
    pub entries: u64,
    pub methods: BTreeMap<String, u64>,
    pub versions: BTreeMap<String, u64>,
    pub flags: BTreeMap<String, u64>,
    pub central_directory_extra_lengths: BTreeMap<String, u64>,
    pub central_directory_extra_values: BTreeMap<String, u64>,
    pub extra_by_method: BTreeMap<String, u64>,
    pub extra_by_family: BTreeMap<String, u64>,
    pub method_by_family: BTreeMap<String, u64>,
    pub azcs: BTreeMap<String, u64>,
    pub azcs_by_method: BTreeMap<String, u64>,
    pub azcs_by_extra: BTreeMap<String, u64>,
    pub azcs_by_family: BTreeMap<String, u64>,
    pub local_extra_lengths: BTreeMap<String, u64>,
    pub central_directory_comment_lengths: BTreeMap<String, u64>,
    pub disk_starts: BTreeMap<String, u64>,
    pub internal_attributes: BTreeMap<String, u64>,
    pub external_attributes: BTreeMap<String, u64>,
    pub separators: BTreeMap<String, u64>,
    pub uppercase_names: u64,
    pub zip64_archives: usize,
    pub eocd_comment_archives: usize,
    pub multi_disk_archives: usize,
    pub samples: Samples,
}

impl Report {
    fn new(root: PathBuf, archives: usize) -> Self {
        Self {
            root,
            archives,
            parsed_archives: 0,
            entries: 0,
            methods: BTreeMap::new(),
            versions: BTreeMap::new(),
            flags: BTreeMap::new(),
            central_directory_extra_lengths: BTreeMap::new(),
            central_directory_extra_values: BTreeMap::new(),
            extra_by_method: BTreeMap::new(),
            extra_by_family: BTreeMap::new(),
            method_by_family: BTreeMap::new(),
            azcs: BTreeMap::new(),
            azcs_by_method: BTreeMap::new(),
            azcs_by_extra: BTreeMap::new(),
            azcs_by_family: BTreeMap::new(),
            local_extra_lengths: BTreeMap::new(),
            central_directory_comment_lengths: BTreeMap::new(),
            disk_starts: BTreeMap::new(),
            internal_attributes: BTreeMap::new(),
            external_attributes: BTreeMap::new(),
            separators: BTreeMap::new(),
            uppercase_names: 0,
            zip64_archives: 0,
            eocd_comment_archives: 0,
            multi_disk_archives: 0,
            samples: Samples::default(),
        }
    }

    fn add_count(map: &mut BTreeMap<String, u64>, key: impl Into<String>, by: u64) {
        *map.entry(key.into()).or_default() += by;
    }

    pub fn merge(&mut self, other: Self, max_samples: usize) {
        self.archives += other.archives;
        self.parsed_archives += other.parsed_archives;
        self.entries += other.entries;
        merge_counts(&mut self.methods, other.methods);
        merge_counts(&mut self.versions, other.versions);
        merge_counts(&mut self.flags, other.flags);
        merge_counts(
            &mut self.central_directory_extra_lengths,
            other.central_directory_extra_lengths,
        );
        merge_counts(
            &mut self.central_directory_extra_values,
            other.central_directory_extra_values,
        );
        merge_counts(&mut self.extra_by_method, other.extra_by_method);
        merge_counts(&mut self.extra_by_family, other.extra_by_family);
        merge_counts(&mut self.method_by_family, other.method_by_family);
        merge_counts(&mut self.azcs, other.azcs);
        merge_counts(&mut self.azcs_by_method, other.azcs_by_method);
        merge_counts(&mut self.azcs_by_extra, other.azcs_by_extra);
        merge_counts(&mut self.azcs_by_family, other.azcs_by_family);
        merge_counts(&mut self.local_extra_lengths, other.local_extra_lengths);
        merge_counts(
            &mut self.central_directory_comment_lengths,
            other.central_directory_comment_lengths,
        );
        merge_counts(&mut self.disk_starts, other.disk_starts);
        merge_counts(&mut self.internal_attributes, other.internal_attributes);
        merge_counts(&mut self.external_attributes, other.external_attributes);
        merge_counts(&mut self.separators, other.separators);
        self.uppercase_names += other.uppercase_names;
        self.zip64_archives += other.zip64_archives;
        self.eocd_comment_archives += other.eocd_comment_archives;
        self.multi_disk_archives += other.multi_disk_archives;
        self.samples.extend(other.samples, max_samples);
    }
}

fn merge_counts(target: &mut BTreeMap<String, u64>, source: BTreeMap<String, u64>) {
    for (key, value) in source {
        *target.entry(key).or_default() += value;
    }
}

#[derive(Debug, Clone, Default)]
pub struct Samples {
    pub errors: Vec<String>,
    pub unknown_methods: Vec<String>,
    pub nonzero_flags: Vec<String>,
    pub nonzero_extra: Vec<String>,
    pub comments: Vec<String>,
    pub mismatches: Vec<String>,
    pub zip64_entries: Vec<String>,
    pub azcs_errors: Vec<String>,
}

impl Samples {
    fn push_error(&mut self, sample: String, max: usize) {
        push_sample(&mut self.errors, sample, max);
    }

    fn push_unknown_method(&mut self, sample: String, max: usize) {
        push_sample(&mut self.unknown_methods, sample, max);
    }

    fn push_nonzero_flag(&mut self, sample: String, max: usize) {
        push_sample(&mut self.nonzero_flags, sample, max);
    }

    fn push_nonzero_extra(&mut self, sample: String, max: usize) {
        push_sample(&mut self.nonzero_extra, sample, max);
    }

    fn push_comment(&mut self, sample: String, max: usize) {
        push_sample(&mut self.comments, sample, max);
    }

    fn push_mismatch(&mut self, sample: String, max: usize) {
        push_sample(&mut self.mismatches, sample, max);
    }

    fn push_zip64_entry(&mut self, sample: String, max: usize) {
        push_sample(&mut self.zip64_entries, sample, max);
    }

    fn push_azcs_error(&mut self, sample: String, max: usize) {
        push_sample(&mut self.azcs_errors, sample, max);
    }

    fn extend(&mut self, other: Self, max: usize) {
        extend_samples(&mut self.errors, other.errors, max);
        extend_samples(&mut self.unknown_methods, other.unknown_methods, max);
        extend_samples(&mut self.nonzero_flags, other.nonzero_flags, max);
        extend_samples(&mut self.nonzero_extra, other.nonzero_extra, max);
        extend_samples(&mut self.comments, other.comments, max);
        extend_samples(&mut self.mismatches, other.mismatches, max);
        extend_samples(&mut self.zip64_entries, other.zip64_entries, max);
        extend_samples(&mut self.azcs_errors, other.azcs_errors, max);
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("EOCD not found")]
    EocdNotFound,

    #[error("archive field is out of bounds: {field}")]
    OutOfBounds { field: &'static str },

    #[error("shape scan cancelled ({skipped} queued archive(s) skipped)")]
    Cancelled { skipped: usize },
}

fn collect_paks(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), Error> {
    if path.is_file() {
        if is_pak(path) {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_paks(&entry.path(), out)?;
        } else if file_type.is_file() && is_pak(&entry.path()) {
            out.push(entry.path());
        }
    }
    Ok(())
}

fn is_pak(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pak"))
}

fn scan_pak(
    path: &Path,
    report: &mut Report,
    options: Options,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    let max_samples = options.max_samples;
    let file = fs::File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let bytes: &[u8] = &mmap;
    let eocd = find_eocd(bytes).ok_or(Error::EocdNotFound)?;

    let disk = read_u16(bytes, eocd + 4, "EOCD disk")?;
    let cd_disk = read_u16(bytes, eocd + 6, "EOCD central directory disk")?;
    let entries_on_disk = read_u16(bytes, eocd + 8, "EOCD entries on disk")?;
    let entries = read_u16(bytes, eocd + 10, "EOCD entries")?;
    let cd_size = read_u32(bytes, eocd + 12, "EOCD central directory size")?;
    let cd_offset = read_u32(bytes, eocd + 16, "EOCD central directory offset")?;
    let eocd_comment_len = read_u16(bytes, eocd + 20, "EOCD comment length")?;

    let zip64 = entries == u16::MAX
        || entries_on_disk == u16::MAX
        || cd_size == u32::MAX
        || cd_offset == u32::MAX;
    if zip64 {
        report.zip64_archives += 1;
    }
    if eocd_comment_len != 0 {
        report.eocd_comment_archives += 1;
    }
    if disk != 0 || cd_disk != 0 || entries_on_disk != entries {
        report.multi_disk_archives += 1;
    }

    let cd_start = cd_offset as usize;
    let cd_end = cd_start
        .checked_add(cd_size as usize)
        .ok_or(Error::OutOfBounds {
            field: "central directory range",
        })?;
    let cd = bytes.get(cd_start..cd_end).ok_or(Error::OutOfBounds {
        field: "central directory range",
    })?;

    let mut pos = 0usize;
    let mut actual_entries = 0u64;
    while pos + CENTRAL_DIRECTORY_HEADER_LEN <= cd.len() {
        if cancel.is_cancelled() {
            break;
        }
        if read_u32(cd, pos, "central directory header")? != CENTRAL_DIRECTORY_HEADER {
            report.samples.push_mismatch(
                format!("{}: bad CDR signature at +{pos}", path.display()),
                max_samples,
            );
            break;
        }

        let cdr_made = read_u16(cd, pos + 4, "CDR version made by")?;
        let cdr_need = read_u16(cd, pos + 6, "CDR version needed")?;
        let flags = read_u16(cd, pos + 8, "CDR flags")?;
        let method = read_u16(cd, pos + 10, "CDR method")?;
        let compressed_size = read_u32(cd, pos + 20, "CDR compressed size")?;
        let uncompressed_size = read_u32(cd, pos + 24, "CDR uncompressed size")?;
        let name_len = read_u16(cd, pos + 28, "CDR name length")? as usize;
        let extra_len = read_u16(cd, pos + 30, "CDR extra length")? as usize;
        let comment_len = read_u16(cd, pos + 32, "CDR comment length")? as usize;
        let disk_start = read_u16(cd, pos + 34, "CDR disk start")?;
        let internal_attr = read_u16(cd, pos + 36, "CDR internal attributes")?;
        let external_attr = read_u32(cd, pos + 38, "CDR external attributes")?;
        let local_offset = read_u32(cd, pos + 42, "CDR local header offset")?;

        let name_start = pos + CENTRAL_DIRECTORY_HEADER_LEN;
        let name_end = name_start
            .checked_add(name_len)
            .ok_or(Error::OutOfBounds { field: "CDR name" })?;
        let name = cd
            .get(name_start..name_end)
            .map(String::from_utf8_lossy)
            .ok_or(Error::OutOfBounds { field: "CDR name" })?;
        let extra_start = name_end;
        let extra_end = extra_start
            .checked_add(extra_len)
            .ok_or(Error::OutOfBounds { field: "CDR extra" })?;
        let extra = cd
            .get(extra_start..extra_end)
            .ok_or(Error::OutOfBounds { field: "CDR extra" })?;
        let extra_key = extra_value_key(extra);
        let family = path_family(&name);

        let local = local_header(bytes, local_offset);
        let (local_need, local_flags, local_method, local_name_len, local_extra_len) = match local {
            Some(local) if read_u32(local, 0, "local signature")? == LOCAL_FILE_HEADER => (
                Some(read_u16(local, 4, "local version needed")?),
                Some(read_u16(local, 6, "local flags")?),
                Some(read_u16(local, 8, "local method")?),
                Some(read_u16(local, 26, "local name length")? as usize),
                Some(read_u16(local, 28, "local extra length")? as usize),
            ),
            Some(_) => {
                report.samples.push_mismatch(
                    format!("{}: bad LFH for {name} at {local_offset}", path.display()),
                    max_samples,
                );
                (None, None, None, None, None)
            }
            None => {
                report.samples.push_mismatch(
                    format!(
                        "{}: local header out of bounds for {name} at {local_offset}",
                        path.display()
                    ),
                    max_samples,
                );
                (None, None, None, None, None)
            }
        };

        Report::add_count(&mut report.methods, method.to_string(), 1);
        Report::add_count(
            &mut report.versions,
            format!(
                "method={method} cdrMade={cdr_made} cdrNeed={cdr_need} localNeed={}",
                fmt_optional(local_need)
            ),
            1,
        );
        Report::add_count(
            &mut report.flags,
            format!("cdr=0x{flags:04x} local={}", fmt_optional_hex(local_flags)),
            1,
        );
        Report::add_count(
            &mut report.central_directory_extra_lengths,
            extra_len.to_string(),
            1,
        );
        Report::add_count(
            &mut report.central_directory_extra_values,
            extra_key.clone(),
            1,
        );
        Report::add_count(
            &mut report.extra_by_method,
            format!("extra={extra_key} method={method}"),
            1,
        );
        Report::add_count(
            &mut report.extra_by_family,
            format!("extra={extra_key} family={family}"),
            1,
        );
        Report::add_count(
            &mut report.method_by_family,
            format!("method={method} family={family}"),
            1,
        );
        if options.scan_azcs {
            let azcs_key = match azcs_key_for_entry(
                bytes,
                local_offset,
                compressed_size,
                uncompressed_size,
                method,
                local_name_len,
                local_extra_len,
            ) {
                Ok(key) => key,
                Err(error) => {
                    report.samples.push_azcs_error(
                        format!("{}: {error} name={name}", path.display()),
                        max_samples,
                    );
                    "error".to_string()
                }
            };
            Report::add_count(&mut report.azcs, azcs_key.clone(), 1);
            Report::add_count(
                &mut report.azcs_by_method,
                format!("azcs={azcs_key} method={method}"),
                1,
            );
            Report::add_count(
                &mut report.azcs_by_extra,
                format!("azcs={azcs_key} extra={extra_key}"),
                1,
            );
            Report::add_count(
                &mut report.azcs_by_family,
                format!("azcs={azcs_key} family={family}"),
                1,
            );
        }
        Report::add_count(
            &mut report.local_extra_lengths,
            local_extra_len.map_or_else(|| "missing".to_string(), |value| value.to_string()),
            1,
        );
        Report::add_count(
            &mut report.central_directory_comment_lengths,
            comment_len.to_string(),
            1,
        );
        Report::add_count(&mut report.disk_starts, disk_start.to_string(), 1);
        Report::add_count(
            &mut report.internal_attributes,
            format!("0x{internal_attr:04x}"),
            1,
        );
        Report::add_count(
            &mut report.external_attributes,
            format!("0x{external_attr:08x}"),
            1,
        );
        Report::add_count(&mut report.separators, separator_kind(&name), 1);
        if name_has_uppercase(&name) {
            report.uppercase_names += 1;
        }

        if !matches!(method, 0 | 8 | 15) {
            report.samples.push_unknown_method(
                format!("{}: method={method} name={name}", path.display()),
                max_samples,
            );
        }
        if flags != 0 || local_flags.is_some_and(|value| value != 0) {
            report.samples.push_nonzero_flag(
                format!(
                    "{}: cdr=0x{flags:04x} local={} name={name}",
                    path.display(),
                    fmt_optional_hex(local_flags)
                ),
                max_samples,
            );
        }
        if extra_len != 0 || local_extra_len.is_some_and(|value| value != 0) {
            report.samples.push_nonzero_extra(
                format!(
                    "{}: cdrExtra={extra_len} localExtra={} name={name}",
                    path.display(),
                    fmt_optional_usize(local_extra_len)
                ),
                max_samples,
            );
        }
        if comment_len != 0 {
            report.samples.push_comment(
                format!("{}: cdrComment={comment_len} name={name}", path.display()),
                max_samples,
            );
        }
        if local_method.is_some_and(|value| value != method) {
            report.samples.push_mismatch(
                format!(
                    "{}: method mismatch cdr={method} local={} name={name}",
                    path.display(),
                    fmt_optional(local_method)
                ),
                max_samples,
            );
        }
        if local_flags.is_some_and(|value| value != flags) {
            report.samples.push_mismatch(
                format!(
                    "{}: flag mismatch cdr=0x{flags:04x} local={} name={name}",
                    path.display(),
                    fmt_optional_hex(local_flags)
                ),
                max_samples,
            );
        }
        if local_name_len.is_some_and(|value| value != name_len) {
            report.samples.push_mismatch(
                format!(
                    "{}: name length mismatch cdr={name_len} local={} name={name}",
                    path.display(),
                    local_name_len.map_or_else(|| "missing".to_string(), |value| value.to_string())
                ),
                max_samples,
            );
        }
        if compressed_size == u32::MAX || uncompressed_size == u32::MAX || local_offset == u32::MAX
        {
            report.samples.push_zip64_entry(
                format!("{}: zip64 sentinel name={name}", path.display()),
                max_samples,
            );
        }

        actual_entries += 1;
        pos += CENTRAL_DIRECTORY_HEADER_LEN + name_len + extra_len + comment_len;
    }

    if !zip64 && actual_entries != u64::from(entries) {
        report.samples.push_mismatch(
            format!(
                "{}: EOCD entries={entries} actual={actual_entries}",
                path.display()
            ),
            max_samples,
        );
    }

    report.entries += actual_entries;
    Ok(())
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    let tail_len = bytes.len().min(EOCD_MAX_SEARCH);
    let tail_start = bytes.len().checked_sub(tail_len)?;
    let tail = &bytes[tail_start..];
    if tail.len() < EOCD_LEN {
        return None;
    }

    (0..=tail.len() - EOCD_LEN).rev().find_map(|offset| {
        (read_u32(tail, offset, "EOCD").ok()? == END_OF_CENTRAL_DIRECTORY)
            .then_some(tail_start + offset)
    })
}

fn local_header(bytes: &[u8], offset: u32) -> Option<&[u8]> {
    let start = offset as usize;
    bytes.get(start..start.checked_add(LOCAL_HEADER_LEN)?)
}

fn azcs_key_for_entry(
    bytes: &[u8],
    local_offset: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    method: u16,
    local_name_len: Option<usize>,
    local_extra_len: Option<usize>,
) -> Result<String, String> {
    let local_name_len = local_name_len.ok_or("local header missing")?;
    let local_extra_len = local_extra_len.ok_or("local header missing")?;
    let data_start = (local_offset as usize)
        .checked_add(LOCAL_HEADER_LEN)
        .and_then(|value| value.checked_add(local_name_len))
        .and_then(|value| value.checked_add(local_extra_len))
        .ok_or("entry data offset overflow")?;
    let data_end = data_start
        .checked_add(compressed_size as usize)
        .ok_or("entry data size overflow")?;
    let compressed = bytes
        .get(data_start..data_end)
        .ok_or("entry data out of bounds")?;

    let mut decoded = Vec::new();
    decompress_bytes_raw_into(
        Compression::from_method_id(method),
        compressed,
        uncompressed_size as usize,
        &mut decoded,
    )
    .map_err(|error| error.to_string())?;

    Ok(AzcsHeader::peek(&decoded).map_or_else(
        || "none".to_string(),
        |header| header.compressor_id.to_string(),
    ))
}

fn read_u16(bytes: &[u8], offset: usize, field: &'static str) -> Result<u16, Error> {
    let bytes = bytes
        .get(offset..offset + 2)
        .ok_or(Error::OutOfBounds { field })?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(bytes: &[u8], offset: usize, field: &'static str) -> Result<u32, Error> {
    let bytes = bytes
        .get(offset..offset + 4)
        .ok_or(Error::OutOfBounds { field })?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn separator_kind(name: &str) -> &'static str {
    match (name.contains('/'), name.contains('\\')) {
        (true, true) => "mixed",
        (true, false) => "slash",
        (false, true) => "backslash",
        (false, false) => "none",
    }
}

fn extra_value_key(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "none".to_string();
    }

    if bytes.len() == 5 {
        let id = u16::from_le_bytes([bytes[0], bytes[1]]);
        let len = u16::from_le_bytes([bytes[2], bytes[3]]);
        if id == crate::crypak_format::MARKER_EXTRA_ID && len == 1 {
            return format!("marker=0x{:02x} bytes={}", bytes[4], hex_bytes(bytes));
        }
    }

    format!("bytes={}", hex_bytes(bytes))
}

fn hex_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(bytes.len().saturating_mul(3).saturating_sub(1));
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[must_use]
pub fn path_family(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let file = lower.rsplit(['/', '\\']).next().unwrap_or(lower.as_str());
    let ext = file.rsplit_once('.').map_or("", |(_, ext)| ext);

    if lower.contains("shader") || matches!(ext, "azshader" | "shader" | "cfx" | "fxc") {
        return "shader";
    }
    if lower.contains("terrain")
        || lower.contains("heightmap")
        || lower.contains("tractmap")
        || matches!(ext, "terrain" | "trn")
    {
        return "terrain";
    }
    if matches!(
        ext,
        "dds" | "tif" | "tiff" | "png" | "jpg" | "jpeg" | "bmp" | "image"
    ) {
        return "texture";
    }
    if matches!(
        ext,
        "cgf" | "cga" | "chr" | "skin" | "caf" | "anm" | "fbx" | "assetinfo"
    ) {
        return "model";
    }
    if matches!(ext, "wem" | "bnk" | "wav" | "ogg" | "mp3") {
        return "audio";
    }
    if matches!(ext, "lua" | "luac") {
        return "script";
    }
    if matches!(
        ext,
        "xml" | "json" | "txt" | "cfg" | "ini" | "csv" | "datasheet" | "dat" | "bin"
    ) {
        return "data";
    }
    if !lower.contains('/') && !lower.contains('\\') {
        return "root";
    }

    "other"
}

fn name_has_uppercase(name: &str) -> bool {
    name.bytes().any(|byte| byte.is_ascii_uppercase())
}

fn fmt_optional(value: Option<u16>) -> String {
    value.map_or_else(|| "missing".to_string(), |value| value.to_string())
}

fn fmt_optional_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "missing".to_string(), |value| value.to_string())
}

fn fmt_optional_hex(value: Option<u16>) -> String {
    value.map_or_else(|| "missing".to_string(), |value| format!("0x{value:04x}"))
}

fn push_sample(samples: &mut Vec<String>, sample: String, max: usize) {
    if samples.len() < max {
        samples.push(sample);
    }
}

fn extend_samples(samples: &mut Vec<String>, other: Vec<String>, max: usize) {
    for sample in other {
        push_sample(samples, sample, max);
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::crypak_format::{DosTime, Entry, Writer};

    #[test]
    fn scans_current_game_shape_fields() {
        let root = temp_test_dir("nw-pak-shape");
        fs::create_dir_all(&root).unwrap();
        let pak_path = root.join("shape.pak");
        let mut bytes = Vec::new();
        let mut writer = Writer::new(&mut bytes);
        let modified = DosTime::from_ymdhms(2026, 6, 19, 12, 0, 0);
        let marker = crate::crypak_format::marker_extra(0x15);

        writer
            .push(Entry {
                name: "a.txt",
                method: 0,
                modified,
                crc32: 0,
                uncompressed_size: 0,
                compressed_size: 0,
                central_extra: &marker,
                compressed_data: &[],
            })
            .unwrap();
        writer
            .push(Entry {
                name: "b.txt",
                method: 15,
                modified,
                crc32: 0,
                uncompressed_size: 0,
                compressed_size: 0,
                central_extra: &marker,
                compressed_data: &[],
            })
            .unwrap();
        writer.finish().unwrap();
        fs::write(&pak_path, bytes).unwrap();

        let report = Scanner::new().scan(&root).unwrap();
        assert_eq!(report.archives, 1);
        assert_eq!(report.parsed_archives, 1);
        assert_eq!(report.entries, 2);
        assert_eq!(report.methods.get("0"), Some(&1));
        assert_eq!(report.methods.get("15"), Some(&1));
        assert_eq!(
            report
                .versions
                .get("method=0 cdrMade=20 cdrNeed=20 localNeed=10"),
            Some(&1)
        );
        assert_eq!(
            report
                .versions
                .get("method=15 cdrMade=9 cdrNeed=9 localNeed=9"),
            Some(&1)
        );
        assert_eq!(report.central_directory_extra_lengths.get("5"), Some(&2));
        assert_eq!(
            report
                .central_directory_extra_values
                .get("marker=0x15 bytes=20 00 01 00 15"),
            Some(&2)
        );
        assert_eq!(
            report
                .extra_by_method
                .get("extra=marker=0x15 bytes=20 00 01 00 15 method=0"),
            Some(&1)
        );
        assert_eq!(
            report
                .extra_by_method
                .get("extra=marker=0x15 bytes=20 00 01 00 15 method=15"),
            Some(&1)
        );
        assert!(report.samples.mismatches.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scans_azcs_payload_trait() {
        let root = temp_test_dir("nw-pak-shape-azcs");
        fs::create_dir_all(&root).unwrap();
        let pak_path = root.join("shape.pak");
        let mut bytes = Vec::new();
        let mut writer = Writer::new(&mut bytes);
        let modified = DosTime::from_ymdhms(2026, 6, 19, 12, 0, 0);
        let marker = crate::crypak_format::marker_extra(0x15);
        let mut payload = Vec::new();
        payload.extend_from_slice(crate::azcs::AZCS_SIGNATURE);
        payload.extend_from_slice(&crate::azcs::AzcsId::Zlib.as_u32().to_be_bytes());
        payload.extend_from_slice(&0u64.to_be_bytes());

        writer
            .push(Entry {
                name: "data/asset.bin",
                method: 0,
                modified,
                crc32: 0,
                uncompressed_size: u32::try_from(payload.len()).expect("test payload fits u32"),
                compressed_size: u32::try_from(payload.len()).expect("test payload fits u32"),
                central_extra: &marker,
                compressed_data: &payload,
            })
            .unwrap();
        writer.finish().unwrap();
        fs::write(&pak_path, bytes).unwrap();

        let report = Scanner::new().azcs(true).scan(&root).unwrap();
        assert_eq!(report.azcs.get("ZLIB"), Some(&1));
        assert_eq!(
            report
                .azcs_by_extra
                .get("azcs=ZLIB extra=marker=0x15 bytes=20 00 01 00 15"),
            Some(&1)
        );

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
