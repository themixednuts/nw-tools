use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use sha1::{Digest, Sha1};
use thiserror::Error as ThisError;
use uuid::Uuid;

use crate::{AssetId, AssetType};

pub const ASSET_CATALOG_PATH: &str = "assetcatalog.catalog";
pub const ASSET_CATALOG_OPTIMIZED_PATH: &str = "assetcatalog_optimized.catalog";
pub const RASC_SIGNATURE: &[u8; 4] = b"RASC";
pub const RAOC_SIGNATURE: &[u8; 4] = b"RAOC";
pub const RAOC_VERSION: u32 = 2;

const RASC_HEADER_LEN: usize = 40;
const RASC_ENTRY_LEN: usize = 40;
const RAOC_HEADER_LEN: usize = 20;
const RAOC_ENTRY_LEN: usize = 48;
const RAOC_AUX_LEN: usize = 32;
const RAOC_PATH_LEN: usize = 48;
const RAOC_DEP_LEN: usize = 32;
const RAOC_TYPE_LEN: usize = 48;
const GUID_LEN: usize = 16;

#[derive(Debug, ThisError)]
#[non_exhaustive]
pub enum Error {
    #[error("catalog input too small: {len} bytes")]
    InputTooSmall { len: usize },

    #[error("invalid catalog signature {signature:02x?}")]
    InvalidSignature { signature: [u8; 4] },

    #[error("unsupported {kind} version {actual}; expected {expected}")]
    UnsupportedVersion {
        kind: Kind,
        actual: u32,
        expected: u32,
    },

    #[error("RASC size sentinel mismatch: file_size={file_size} end_sentinel={end_sentinel}")]
    SizeSentinelMismatch { file_size: u64, end_sentinel: u32 },

    #[error("RAOC file size mismatch: declared={declared} actual={actual}")]
    FileSizeMismatch { declared: u64, actual: u64 },

    #[error("{label} out of bounds at 0x{offset:x}: need {needed} bytes, input has {len}")]
    OutOfBounds {
        label: &'static str,
        offset: usize,
        needed: usize,
        len: usize,
    },

    #[error("{label} is unterminated")]
    UnterminatedString { label: &'static str },

    #[error("{label} is not valid utf-8")]
    InvalidUtf8 {
        label: &'static str,
        #[source]
        source: std::str::Utf8Error,
    },

    #[error("catalog layout overflow")]
    LayoutOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Rasc,
    Raoc,
}

impl Kind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rasc => "RASC",
            Self::Raoc => "RAOC",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Catalog {
    Rasc(Box<Rasc>),
    Raoc(Box<Raoc>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetCatalog {
    rasc: Box<Rasc>,
    raoc: Option<Box<Raoc>>,
}

impl Catalog {
    /// Parse a catalog and select the reader from the file signature.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the input is not a supported `RASC` or `RAOC`
    /// catalog, or if any table, string, GUID, or size field is malformed.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        match detect(bytes)? {
            Kind::Rasc => Rasc::parse(bytes).map(Box::new).map(Self::Rasc),
            Kind::Raoc => Raoc::parse(bytes).map(Box::new).map(Self::Raoc),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Rasc(_) => Kind::Rasc,
            Self::Raoc(_) => Kind::Raoc,
        }
    }

    #[must_use]
    pub fn version(&self) -> u32 {
        match self {
            Self::Rasc(catalog) => catalog.version(),
            Self::Raoc(catalog) => catalog.version(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Rasc(catalog) => catalog.len(),
            Self::Raoc(catalog) => catalog.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AssetCatalog {
    #[must_use]
    pub fn new(rasc: Rasc, raoc: Option<Raoc>) -> Self {
        Self {
            rasc: Box::new(rasc),
            raoc: raoc.map(Box::new),
        }
    }

    #[must_use]
    pub fn rasc(&self) -> &Rasc {
        &self.rasc
    }

    #[must_use]
    pub fn raoc(&self) -> Option<&Raoc> {
        self.raoc.as_deref()
    }

    #[must_use]
    pub fn entries(&self) -> &[RascEntry] {
        self.rasc.entries()
    }

    #[must_use]
    pub fn entry_by_id(&self, asset_id: AssetId) -> Option<&RascEntry> {
        self.rasc.entry(asset_id)
    }

    #[must_use]
    pub fn id_by_path(&self, path: &str) -> Option<AssetId> {
        if let Some(asset_id) = self.raoc.as_ref().and_then(|raoc| raoc.id_by_path(path)) {
            return Some(asset_id);
        }
        self.rasc.id_by_path(path)
    }

    #[must_use]
    pub fn entry_by_path(&self, path: &str) -> Option<&RascEntry> {
        if let Some(asset_id) = self.raoc.as_ref().and_then(|raoc| raoc.id_by_path(path))
            && let Some(entry) = self.rasc.entry(asset_id)
        {
            return Some(entry);
        }
        self.rasc.entry_by_path(path)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rasc.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rasc.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RascEntry {
    asset_id: AssetId,
    asset_type: AssetType,
    path: String,
    size_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rasc {
    version: u32,
    entries: Vec<RascEntry>,
    by_id: HashMap<AssetId, usize>,
    by_path: HashMap<String, usize>,
}

impl RascEntry {
    #[must_use]
    pub fn new(
        asset_id: AssetId,
        asset_type: AssetType,
        path: impl Into<String>,
        size_bytes: u32,
    ) -> Self {
        Self {
            asset_id,
            asset_type,
            path: normalize_virtual_path(path.into()),
            size_bytes,
        }
    }

    #[must_use]
    pub const fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    #[must_use]
    pub const fn asset_type(&self) -> AssetType {
        self.asset_type
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn relative_path(&self) -> &Path {
        Path::new(&self.path)
    }

    #[must_use]
    pub const fn size_bytes(&self) -> u32 {
        self.size_bytes
    }
}

impl Rasc {
    /// Parse a `RASC` binary asset catalog.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the input is not a valid `RASC` catalog or when
    /// any table offset, string, GUID, or size sentinel is malformed.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        parse_rasc(bytes)
    }

    pub(crate) fn from_entries(version: u32, entries: Vec<RascEntry>) -> Self {
        let mut by_id = HashMap::with_capacity(entries.len());
        let mut by_path = HashMap::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            by_id.entry(entry.asset_id()).or_insert(index);
            by_path.entry(entry.path().to_string()).or_insert(index);
        }
        Self {
            version,
            entries,
            by_id,
            by_path,
        }
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub fn entries(&self) -> &[RascEntry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, RascEntry> {
        self.entries.iter()
    }

    #[must_use]
    pub fn entry(&self, asset_id: AssetId) -> Option<&RascEntry> {
        self.by_id
            .get(&asset_id)
            .and_then(|index| self.entries.get(*index))
    }

    #[must_use]
    pub fn entry_by_path(&self, path: &str) -> Option<&RascEntry> {
        let path = normalize_virtual_path(path);
        self.by_path
            .get(&path)
            .and_then(|index| self.entries.get(*index))
    }

    #[must_use]
    pub fn id_by_path(&self, path: &str) -> Option<AssetId> {
        self.entry_by_path(path).map(RascEntry::asset_id)
    }
}

impl<'a> IntoIterator for &'a Rasc {
    type Item = &'a RascEntry;
    type IntoIter = std::slice::Iter<'a, RascEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RaocEntry {
    asset_id: AssetId,
    asset_type: AssetType,
    size_bytes: u32,
    flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuxIndex {
    key: [u8; 16],
    raw: [u8; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathId {
    path_hash: [u8; 16],
    asset_id: AssetId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dependency {
    source: AssetId,
    target: AssetId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeInfo {
    asset_type: AssetType,
    extension_raw: [u8; 16],
    metadata_raw: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raoc {
    version: u32,
    file_size: u64,
    entries: Vec<RaocEntry>,
    aux_index: Vec<AuxIndex>,
    path_ids: Vec<PathId>,
    dependencies: Vec<Dependency>,
    types: Vec<TypeInfo>,
    dir_blob: Vec<u8>,
    file_blob: Vec<u8>,
    by_id: HashMap<AssetId, usize>,
    by_path_hash: HashMap<[u8; 16], AssetId>,
}

impl RaocEntry {
    #[must_use]
    pub const fn new(
        asset_id: AssetId,
        asset_type: AssetType,
        size_bytes: u32,
        flags: u32,
    ) -> Self {
        Self {
            asset_id,
            asset_type,
            size_bytes,
            flags,
        }
    }

    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }

    #[must_use]
    pub const fn asset_type(self) -> AssetType {
        self.asset_type
    }

    #[must_use]
    pub const fn size_bytes(self) -> u32 {
        self.size_bytes
    }

    #[must_use]
    pub const fn flags(self) -> u32 {
        self.flags
    }
}

impl AuxIndex {
    #[must_use]
    pub const fn key(self) -> [u8; 16] {
        self.key
    }

    #[must_use]
    pub const fn raw(self) -> [u8; 16] {
        self.raw
    }
}

impl PathId {
    #[must_use]
    pub const fn path_hash(self) -> [u8; 16] {
        self.path_hash
    }

    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }
}

impl Dependency {
    #[must_use]
    pub const fn source(self) -> AssetId {
        self.source
    }

    #[must_use]
    pub const fn target(self) -> AssetId {
        self.target
    }
}

impl TypeInfo {
    #[must_use]
    pub const fn asset_type(self) -> AssetType {
        self.asset_type
    }

    #[must_use]
    pub const fn extension_raw(self) -> [u8; 16] {
        self.extension_raw
    }

    #[must_use]
    pub const fn metadata_raw(self) -> [u8; 16] {
        self.metadata_raw
    }

    #[must_use]
    pub fn extension(&self) -> Cow<'_, str> {
        let len = self
            .extension_raw
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(self.extension_raw.len());
        String::from_utf8_lossy(&self.extension_raw[..len])
    }
}

impl Raoc {
    /// Parse a `RAOC` optimized asset catalog.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the input is not a supported `RAOC` catalog, if
    /// its declared file size differs from the buffer length, or if any packed
    /// table is truncated.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        parse_raoc(bytes)
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn file_size(&self) -> u64 {
        self.file_size
    }

    #[must_use]
    pub fn entries(&self) -> &[RaocEntry] {
        &self.entries
    }

    #[must_use]
    pub fn aux_index(&self) -> &[AuxIndex] {
        &self.aux_index
    }

    #[must_use]
    pub fn path_ids(&self) -> &[PathId] {
        &self.path_ids
    }

    #[must_use]
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    #[must_use]
    pub fn types(&self) -> &[TypeInfo] {
        &self.types
    }

    #[must_use]
    pub fn dir_blob(&self) -> &[u8] {
        &self.dir_blob
    }

    #[must_use]
    pub fn file_blob(&self) -> &[u8] {
        &self.file_blob
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, RaocEntry> {
        self.entries.iter()
    }

    #[must_use]
    pub fn entry(&self, asset_id: AssetId) -> Option<&RaocEntry> {
        self.by_id
            .get(&asset_id)
            .and_then(|index| self.entries.get(*index))
    }

    #[must_use]
    pub fn id_by_path_hash(&self, path_hash: &[u8; 16]) -> Option<AssetId> {
        self.by_path_hash.get(path_hash).copied()
    }

    #[must_use]
    pub fn id_by_path(&self, path: &str) -> Option<AssetId> {
        self.id_by_path_hash(&asset_path_hash(path))
    }

    #[must_use]
    pub fn dir_str(&self, offset: usize) -> Cow<'_, str> {
        read_cstr(&self.dir_blob, offset)
    }

    #[must_use]
    pub fn file_str(&self, offset: usize) -> Cow<'_, str> {
        read_cstr(&self.file_blob, offset)
    }
}

impl<'a> IntoIterator for &'a Raoc {
    type Item = &'a RaocEntry;
    type IntoIter = std::slice::Iter<'a, RaocEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[must_use]
pub fn is_asset_catalog_path(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case(ASSET_CATALOG_PATH)
                || name.eq_ignore_ascii_case(ASSET_CATALOG_OPTIMIZED_PATH)
        })
}

#[must_use]
pub fn normalize_virtual_path(path: impl AsRef<str>) -> String {
    let path = path.as_ref().replace('\\', "/").to_ascii_lowercase();
    let mut normalized = String::with_capacity(path.len());
    let mut previous_slash = false;
    for ch in path.trim_matches('/').chars() {
        if ch == '/' {
            if !previous_slash {
                normalized.push('/');
            }
            previous_slash = true;
        } else {
            normalized.push(ch);
            previous_slash = false;
        }
    }
    normalized
}

#[must_use]
pub fn asset_path_hash(path: &str) -> [u8; 16] {
    let normalized = normalize_virtual_path(path);
    let digest = Sha1::digest(normalized.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out[8] &= 0xbf;
    out[8] |= 0x80;
    out[6] &= 0x5f;
    out[6] |= 0x50;
    out
}

/// Return the catalog kind from the leading signature bytes.
///
/// # Errors
///
/// Returns [`Error::InputTooSmall`] when fewer than four bytes are available,
/// or [`Error::InvalidSignature`] when the signature is not known.
pub fn detect(bytes: &[u8]) -> Result<Kind, Error> {
    let signature = array_at::<4>(bytes, 0, "catalog signature").map_err(|error| match error {
        Error::OutOfBounds { .. } => Error::InputTooSmall { len: bytes.len() },
        other => other,
    })?;
    match &signature {
        RASC_SIGNATURE => Ok(Kind::Rasc),
        RAOC_SIGNATURE => Ok(Kind::Raoc),
        _ => Err(Error::InvalidSignature { signature }),
    }
}

fn parse_rasc(bytes: &[u8]) -> Result<Rasc, Error> {
    if bytes.len() < RASC_HEADER_LEN {
        return Err(Error::InputTooSmall { len: bytes.len() });
    }
    let signature = array_at::<4>(bytes, 0, "RASC signature")?;
    if &signature != RASC_SIGNATURE {
        return Err(Error::InvalidSignature { signature });
    }

    let version = u32_at(bytes, 4, "RASC version")?;
    let file_size = u64_at(bytes, 8, "RASC file size")?;
    let guid_offset = usize_from_u32(u32_at(bytes, 16, "RASC GUID table offset")?)?;
    let asset_type_offset = usize_from_u32(u32_at(bytes, 20, "RASC asset type table offset")?)?;
    let dir_offset = usize_from_u32(u32_at(bytes, 24, "RASC directory string table offset")?)?;
    let file_name_offset = usize_from_u32(u32_at(bytes, 28, "RASC file string table offset")?)?;
    let end_sentinel = u32_at(bytes, 32, "RASC end sentinel")?;
    let entry_count = usize_from_u32(u32_at(bytes, 36, "RASC entry count")?)?;
    if file_size != u64::from(end_sentinel) {
        return Err(Error::SizeSentinelMismatch {
            file_size,
            end_sentinel,
        });
    }

    let mut entries = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let offset = checked_add(RASC_HEADER_LEN, checked_mul(index, RASC_ENTRY_LEN)?)?;
        let record = slice_at(bytes, offset, RASC_ENTRY_LEN, "RASC entry")?;
        let guid_index = usize_from_u32(le_u32(record, 0))?;
        let sub_id = le_u32(record, 4);
        let asset_type_index = usize_from_u32(le_u32(record, 16))?;
        let size_bytes = le_u32(record, 24);
        let dir_string_offset = usize_from_u32(le_u32(record, 32))?;
        let file_string_offset = usize_from_u32(le_u32(record, 36))?;

        let directory = string_at(
            bytes,
            checked_add(dir_offset, dir_string_offset)?,
            "RASC directory",
        )?;
        let file_name = string_at(
            bytes,
            checked_add(file_name_offset, file_string_offset)?,
            "RASC file name",
        )?;
        let path = if directory.is_empty() {
            file_name.to_owned()
        } else {
            format!("{directory}/{file_name}")
        };
        let asset_id = AssetId::new(
            uuid_le_at(
                bytes,
                checked_add(guid_offset, checked_mul(guid_index, GUID_LEN)?)?,
                "RASC asset id",
            )?,
            sub_id,
        );
        let asset_type = AssetType::new(uuid_le_at(
            bytes,
            checked_add(asset_type_offset, checked_mul(asset_type_index, GUID_LEN)?)?,
            "RASC asset type",
        )?);
        entries.push(RascEntry::new(asset_id, asset_type, path, size_bytes));
    }

    Ok(Rasc::from_entries(version, entries))
}

fn parse_raoc(bytes: &[u8]) -> Result<Raoc, Error> {
    let header = RaocHeader::parse(bytes)?;
    let mut reader = RaocReader::new(bytes, checked_raoc_entries_end(header.entry_count)?);
    let entries = parse_raoc_entries(bytes, header.entry_count)?;
    let aux_index = reader.table(RAOC_AUX_LEN, "RAOC auxiliary index", |record| {
        Ok(AuxIndex {
            key: record_array(record, 0),
            raw: record_array(record, 16),
        })
    })?;
    let path_ids = reader.table(RAOC_PATH_LEN, "RAOC path table", |record| {
        Ok(PathId {
            path_hash: record_array(record, 0),
            asset_id: AssetId::new(
                Uuid::from_bytes(record_array(record, 16)),
                le_u32(record, 32),
            ),
        })
    })?;
    let dependencies = reader.table(RAOC_DEP_LEN, "RAOC dependencies", |record| {
        Ok(Dependency {
            source: AssetId::new(Uuid::from_bytes(record_array(record, 0)), 0),
            target: AssetId::new(Uuid::from_bytes(record_array(record, 16)), 0),
        })
    })?;
    let types = reader.table(RAOC_TYPE_LEN, "RAOC asset type registry", |record| {
        Ok(TypeInfo {
            asset_type: AssetType::new(Uuid::from_bytes(record_array(record, 0))),
            extension_raw: record_array(record, 16),
            metadata_raw: record_array(record, 32),
        })
    })?;
    let dir_blob = reader.blob("RAOC directory blob")?.to_vec();
    let file_blob = reader.blob("RAOC file blob")?.to_vec();

    let mut by_id = HashMap::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        by_id.insert(entry.asset_id(), index);
    }
    let mut by_path_hash = HashMap::with_capacity(path_ids.len());
    for entry in &path_ids {
        by_path_hash.insert(entry.path_hash(), entry.asset_id());
    }

    Ok(Raoc {
        version: header.version,
        file_size: header.file_size,
        entries,
        aux_index,
        path_ids,
        dependencies,
        types,
        dir_blob,
        file_blob,
        by_id,
        by_path_hash,
    })
}

fn parse_raoc_entries(bytes: &[u8], count: usize) -> Result<Vec<RaocEntry>, Error> {
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let offset = checked_add(RAOC_HEADER_LEN, checked_mul(index, RAOC_ENTRY_LEN)?)?;
        let record = slice_at(bytes, offset, RAOC_ENTRY_LEN, "RAOC entry")?;
        entries.push(RaocEntry::new(
            AssetId::new(
                Uuid::from_bytes(record_array(record, 0)),
                le_u32(record, 16),
            ),
            AssetType::nil(),
            le_u32(record, 40),
            le_u32(record, 44),
        ));
    }
    Ok(entries)
}

#[derive(Debug, Clone, Copy)]
struct RaocHeader {
    version: u32,
    file_size: u64,
    entry_count: usize,
}

impl RaocHeader {
    fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < RAOC_HEADER_LEN {
            return Err(Error::InputTooSmall { len: bytes.len() });
        }
        let signature = array_at::<4>(bytes, 0, "RAOC signature")?;
        if &signature != RAOC_SIGNATURE {
            return Err(Error::InvalidSignature { signature });
        }
        let version = u32_at(bytes, 4, "RAOC version")?;
        if version != RAOC_VERSION {
            return Err(Error::UnsupportedVersion {
                kind: Kind::Raoc,
                actual: version,
                expected: RAOC_VERSION,
            });
        }
        let file_size = u64_at(bytes, 8, "RAOC file size")?;
        let actual = usize_to_u64(bytes.len())?;
        if file_size != actual {
            return Err(Error::FileSizeMismatch {
                declared: file_size,
                actual,
            });
        }
        Ok(Self {
            version,
            file_size,
            entry_count: usize_from_u32(u32_at(bytes, 16, "RAOC entry count")?)?,
        })
    }
}

struct RaocReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> RaocReader<'a> {
    const fn new(bytes: &'a [u8], cursor: usize) -> Self {
        Self { bytes, cursor }
    }

    fn table<T>(
        &mut self,
        stride: usize,
        label: &'static str,
        mut parse: impl FnMut(&[u8]) -> Result<T, Error>,
    ) -> Result<Vec<T>, Error> {
        let count = usize_from_u32(self.u32(label)?)?;
        let bytes_len = checked_mul(count, stride)?;
        let start = self.cursor;
        let table = self.slice(bytes_len, label)?;
        let mut rows = Vec::with_capacity(count);
        for index in 0..count {
            let offset = checked_mul(index, stride)?;
            rows.push(parse(slice_at(table, offset, stride, label)?)?);
        }
        debug_assert_eq!(self.cursor, checked_add(start, bytes_len)?);
        Ok(rows)
    }

    fn blob(&mut self, label: &'static str) -> Result<&'a [u8], Error> {
        let len = usize_from_u32(self.u32(label)?)?;
        self.slice(len, label)
    }

    fn u32(&mut self, label: &'static str) -> Result<u32, Error> {
        let value = u32_at(self.bytes, self.cursor, label)?;
        self.cursor = checked_add(self.cursor, 4)?;
        Ok(value)
    }

    fn slice(&mut self, len: usize, label: &'static str) -> Result<&'a [u8], Error> {
        let slice = slice_at(self.bytes, self.cursor, len, label)?;
        self.cursor = checked_add(self.cursor, len)?;
        Ok(slice)
    }
}

fn checked_raoc_entries_end(count: usize) -> Result<usize, Error> {
    checked_add(RAOC_HEADER_LEN, checked_mul(count, RAOC_ENTRY_LEN)?)
}

fn uuid_le_at(bytes: &[u8], offset: usize, label: &'static str) -> Result<Uuid, Error> {
    let guid = array_at::<16>(bytes, offset, label)?;
    Ok(Uuid::from_fields(
        u32::from_le_bytes([guid[0], guid[1], guid[2], guid[3]]),
        u16::from_le_bytes([guid[4], guid[5]]),
        u16::from_le_bytes([guid[6], guid[7]]),
        &[
            guid[8], guid[9], guid[10], guid[11], guid[12], guid[13], guid[14], guid[15],
        ],
    ))
}

fn u32_at(bytes: &[u8], offset: usize, label: &'static str) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(array_at(bytes, offset, label)?))
}

fn u64_at(bytes: &[u8], offset: usize, label: &'static str) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(array_at(bytes, offset, label)?))
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(record_array(bytes, offset))
}

fn string_at<'a>(bytes: &'a [u8], offset: usize, label: &'static str) -> Result<&'a str, Error> {
    let tail = bytes.get(offset..).ok_or(Error::OutOfBounds {
        label,
        offset,
        needed: 1,
        len: bytes.len(),
    })?;
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::UnterminatedString { label })?;
    std::str::from_utf8(&tail[..end]).map_err(|source| Error::InvalidUtf8 { label, source })
}

fn read_cstr(bytes: &[u8], offset: usize) -> Cow<'_, str> {
    let Some(tail) = bytes.get(offset..) else {
        return Cow::Borrowed("");
    };
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(tail.len());
    String::from_utf8_lossy(&tail[..end])
}

fn slice_at<'a>(
    bytes: &'a [u8],
    offset: usize,
    needed: usize,
    label: &'static str,
) -> Result<&'a [u8], Error> {
    let end = checked_add(offset, needed)?;
    bytes.get(offset..end).ok_or(Error::OutOfBounds {
        label,
        offset,
        needed,
        len: bytes.len(),
    })
}

fn array_at<const LEN: usize>(
    bytes: &[u8],
    offset: usize,
    label: &'static str,
) -> Result<[u8; LEN], Error> {
    let mut output = [0; LEN];
    output.copy_from_slice(slice_at(bytes, offset, LEN, label)?);
    Ok(output)
}

fn record_array<const LEN: usize>(record: &[u8], offset: usize) -> [u8; LEN] {
    let mut output = [0; LEN];
    output.copy_from_slice(&record[offset..offset + LEN]);
    output
}

const fn checked_add(left: usize, right: usize) -> Result<usize, Error> {
    match left.checked_add(right) {
        Some(value) => Ok(value),
        None => Err(Error::LayoutOverflow),
    }
}

const fn checked_mul(left: usize, right: usize) -> Result<usize, Error> {
    match left.checked_mul(right) {
        Some(value) => Ok(value),
        None => Err(Error::LayoutOverflow),
    }
}

fn usize_from_u32(value: u32) -> Result<usize, Error> {
    usize::try_from(value).map_err(|_| Error::LayoutOverflow)
}

fn usize_to_u64(value: usize) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::LayoutOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_both_catalog_names() {
        assert!(is_asset_catalog_path("assetcatalog.catalog"));
        assert!(is_asset_catalog_path("assetcatalog_optimized.catalog"));
        assert!(!is_asset_catalog_path("assetcatalog.json"));
    }

    #[test]
    fn normalizes_catalog_paths() {
        assert_eq!(
            normalize_virtual_path(r"\Objects\\Foo//Bar.DDS.1A/"),
            "objects/foo/bar.dds.1a"
        );
    }

    #[test]
    fn asset_path_hash_is_stable_across_case_and_separators() {
        let a = asset_path_hash("LyShineUI/Globals.luac");
        let b = asset_path_hash("lyshineui/globals.luac");
        let c = asset_path_hash("LyShineUI\\Globals.luac");

        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a[6] & 0xf0, 0x50);
        assert_eq!(a[8] & 0xc0, 0x80);
    }

    #[test]
    fn asset_catalog_resolves_paths_through_raoc_then_rasc() {
        let asset_id = AssetId::new(
            Uuid::from_u128(0x699f_a9e5_4f8a_5b01_87b2_d5f7_18c9_27b8),
            7,
        );
        let asset_type = AssetType::new(Uuid::from_u128(0xaabb_ccdd_eeff_0011_2233_4455_6677_8899));
        let path = "localization/en-us/main.loc.xml";
        let rasc = Rasc::from_entries(1, vec![RascEntry::new(asset_id, asset_type, path, 42)]);
        let raoc = Raoc {
            version: RAOC_VERSION,
            file_size: 0,
            entries: vec![RaocEntry::new(asset_id, asset_type, 42, 0)],
            aux_index: Vec::new(),
            path_ids: vec![PathId {
                path_hash: asset_path_hash(path),
                asset_id,
            }],
            dependencies: Vec::new(),
            types: Vec::new(),
            dir_blob: Vec::new(),
            file_blob: Vec::new(),
            by_id: [(asset_id, 0)].into_iter().collect(),
            by_path_hash: [(asset_path_hash(path), asset_id)].into_iter().collect(),
        };
        let catalog = AssetCatalog::new(rasc, Some(raoc));

        assert_eq!(
            catalog.id_by_path("Localization\\EN-US\\Main.loc.xml"),
            Some(asset_id)
        );
        assert_eq!(
            catalog
                .entry_by_path("Localization\\EN-US\\Main.loc.xml")
                .map(RascEntry::asset_type),
            Some(asset_type)
        );
    }

    #[test]
    fn parses_path_table_asset_id() {
        let asset_guid = Uuid::from_u128(0x699f_a9e5_4f8a_5b01_87b2_d5f7_18c9_27b8);
        let asset_id = AssetId::new(asset_guid, 0x181a_6070);
        let path_hash = [0xaf; 16];
        let mut bytes = raoc_header(1);

        bytes.extend_from_slice(asset_guid.as_bytes());
        bytes.extend_from_slice(&asset_id.sub_id.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&0x1111_2222_u32.to_le_bytes());
        bytes.extend_from_slice(&0x3333_4444_u32.to_le_bytes());
        bytes.extend_from_slice(&1668_u32.to_le_bytes());
        bytes.extend_from_slice(&7_u32.to_le_bytes());

        bytes.extend_from_slice(&0_u32.to_le_bytes());

        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&path_hash);
        bytes.extend_from_slice(asset_guid.as_bytes());
        bytes.extend_from_slice(&asset_id.sub_id.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);

        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());

        patch_file_size(&mut bytes);
        let catalog = Raoc::parse(&bytes).unwrap();

        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog.entries()[0].asset_id(), asset_id);
        assert_eq!(catalog.entries()[0].asset_type(), AssetType::nil());
        assert_eq!(catalog.entries()[0].size_bytes(), 1668);
        assert_eq!(catalog.entries()[0].flags(), 7);
        assert_eq!(catalog.id_by_path_hash(&path_hash), Some(asset_id));
        assert_eq!(catalog.entry(asset_id).unwrap().asset_id(), asset_id);
    }

    #[test]
    fn rejects_wrong_raoc_version() {
        let mut bytes = raoc_header(0);
        bytes[4..8].copy_from_slice(&1_u32.to_le_bytes());
        patch_file_size(&mut bytes);

        let error = Raoc::parse(&bytes).unwrap_err();
        assert!(matches!(
            error,
            Error::UnsupportedVersion {
                kind: Kind::Raoc,
                actual: 1,
                expected: RAOC_VERSION
            }
        ));
    }

    #[test]
    fn decodes_type_extension() {
        let mut raw = [0; 16];
        raw[..3].copy_from_slice(b"dds");
        let entry = TypeInfo {
            asset_type: AssetType::nil(),
            extension_raw: raw,
            metadata_raw: [0; 16],
        };

        assert_eq!(entry.extension(), "dds");
    }

    fn raoc_header(entry_count: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(RAOC_SIGNATURE);
        bytes.extend_from_slice(&RAOC_VERSION.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&entry_count.to_le_bytes());
        bytes
    }

    fn patch_file_size(bytes: &mut [u8]) {
        let file_size = u64::try_from(bytes.len()).unwrap();
        bytes[8..16].copy_from_slice(&file_size.to_le_bytes());
    }
}
