use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use thiserror::Error as ThisError;
use uuid::Uuid;

use crate::uuid::AzUuidExt;
use crate::{AssetId, AssetType, SourceAssetId};

pub const ASSET_CATALOG_PATH: &str = "assetcatalog.catalog";
pub const ASSET_CATALOG_OPTIMIZED_PATH: &str = "assetcatalog_optimized.catalog";
pub const RASC_SIGNATURE: &[u8; 4] = b"RASC";
pub const RAOC_SIGNATURE: &[u8; 4] = b"RAOC";
pub const RASC_VERSION: u32 = 1;
pub const RAOC_VERSION: u32 = 2;

const RASC_HEADER_LEN: usize = 40;
const RASC_ENTRY_LEN: usize = 40;
/// `source-guid -> product` grouping record: `{guidIdx, subId, count, base}`.
const RASC_SOURCE_GROUP_LEN: usize = 16;
/// `path-hash -> AssetId` record: `{pathHashGuidIdx, assetGuidIdx, subId}`.
const RASC_PATH_ID_LEN: usize = 12;
/// `legacy -> real` redirect record: `{legacyGuidIdx, legacySubId, realGuidIdx, realSubId}`.
const RASC_LEGACY_MAPPING_LEN: usize = 16;
/// Product-array record referenced by the source-group table: `{guidIdx, subId, u64}`.
const RASC_PRODUCT_REF_LEN: usize = 16;
const RAOC_HEADER_LEN: usize = 20;
const RAOC_ENTRY_LEN: usize = 48;
const RAOC_GUID_INFO_LEN: usize = 32;
const RAOC_PATH_LEN: usize = 48;
const RAOC_LEGACY_MAPPING_LEN: usize = 32;
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

    #[error("RASC declared file size {declared} exceeds input length {len}")]
    RascSizeExceedsInput { declared: u64, len: usize },

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

/// A compatibility-catalog inconsistency encountered while following a legacy
/// [`AssetId`] to the product identity used by the shipping catalog.
///
/// Missing identities are deliberately not errors: Lumberyard returns an empty
/// `AssetInfo` for a catalog miss, and shipping content contains dangling
/// references. Cycles and disagreement between RASC and RAOC are different:
/// they make the identity ambiguous and must fail closed.
#[derive(Debug, ThisError, Clone, PartialEq, Eq)]
pub enum CompatibilityResolutionError {
    #[error("compatibility catalog redirect cycle while resolving {asset_id}")]
    RedirectCycle { asset_id: AssetId },

    #[error(
        "RASC/RAOC legacy redirects disagree for {asset_id}: {rasc_asset_id} vs {raoc_asset_id}"
    )]
    RedirectDisagreement {
        asset_id: AssetId,
        rasc_asset_id: AssetId,
        raoc_asset_id: AssetId,
    },
}

#[derive(Debug, ThisError, Clone, PartialEq, Eq)]
pub enum TypedAssetResolutionError {
    #[error(transparent)]
    Compatibility(#[from] CompatibilityResolutionError),

    #[error("asset {asset_id} has type {actual}, expected {expected}")]
    TypeMismatch {
        asset_id: AssetId,
        expected: AssetType,
        actual: AssetType,
    },

    #[error("source asset {source_id} has multiple products of type {asset_type}: {products:?}")]
    AmbiguousSourceProducts {
        source_id: SourceAssetId,
        asset_type: AssetType,
        products: Vec<AssetId>,
    },
}

#[derive(Debug, ThisError, Clone, PartialEq, Eq)]
pub enum SerializedAssetReferenceResolutionError {
    #[error(transparent)]
    Compatibility(#[from] CompatibilityResolutionError),

    #[error("source asset {source_id} has multiple products of type {asset_type}: {products:?}")]
    AmbiguousSourceProducts {
        source_id: SourceAssetId,
        asset_type: AssetType,
        products: Vec<AssetId>,
    },
}

#[derive(Debug)]
struct SourceProductAmbiguity {
    source_id: SourceAssetId,
    asset_type: AssetType,
    products: Vec<AssetId>,
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
        Self::parse_with_runner(bytes, &nw_jobs::JobRunner::automatic())
    }

    /// Parse a catalog using the caller's worker policy for fixed-record tables.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] under the same conditions as [`Self::parse`].
    pub fn parse_with_runner(bytes: &[u8], runner: &nw_jobs::JobRunner) -> Result<Self, Error> {
        match detect(bytes)? {
            Kind::Rasc => Rasc::parse_with_runner(bytes, runner)
                .map(Box::new)
                .map(Self::Rasc),
            Kind::Raoc => Raoc::parse_with_runner(bytes, runner)
                .map(Box::new)
                .map(Self::Raoc),
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

    /// Follow the legacy-id redirect tables shared by RASC and RAOC.
    ///
    /// A catalog can contain a redirect chain, so resolution continues until
    /// neither catalog supplies a replacement. If both catalogs contain a
    /// redirect for the same identity, they must agree.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibilityResolutionError`] when the redirect graph cycles
    /// or RASC and RAOC disagree about the next identity.
    pub fn resolve_compatibility_id(
        &self,
        asset_id: AssetId,
    ) -> Result<AssetId, CompatibilityResolutionError> {
        let mut current = asset_id;
        let mut visited = std::collections::HashSet::new();
        loop {
            if !visited.insert(current) {
                return Err(CompatibilityResolutionError::RedirectCycle { asset_id });
            }

            let rasc = self.rasc.resolve_legacy_id(current);
            let raoc = self
                .raoc
                .as_ref()
                .and_then(|raoc| raoc.resolve_legacy_id(current));
            if let (Some(rasc_asset_id), Some(raoc_asset_id)) = (rasc, raoc)
                && rasc_asset_id != raoc_asset_id
            {
                return Err(CompatibilityResolutionError::RedirectDisagreement {
                    asset_id: current,
                    rasc_asset_id,
                    raoc_asset_id,
                });
            }

            let Some(next) = rasc.or(raoc) else {
                return Ok(current);
            };
            if next == current {
                return Ok(current);
            }
            current = next;
        }
    }

    /// Resolve a legacy identity and return its canonical RASC product entry.
    ///
    /// A missing product entry remains `Ok(None)` to match Lumberyard's
    /// tolerated dangling-reference behavior.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibilityResolutionError`] under the same conditions as
    /// [`Self::resolve_compatibility_id`].
    pub fn entry_by_compatibility_id(
        &self,
        asset_id: AssetId,
    ) -> Result<Option<&RascEntry>, CompatibilityResolutionError> {
        let resolved = self.resolve_compatibility_id(asset_id)?;
        Ok(self.rasc.entry(resolved))
    }

    /// Resolve a real, legacy, or source asset identity to one typed product.
    ///
    /// Source identities carry sub-ID zero. When no exact product exists, the
    /// catalog's same-source entries are filtered by the reflected `Asset<T>`
    /// type and must yield at most one product.
    ///
    /// # Errors
    ///
    /// Returns [`TypedAssetResolutionError`] when legacy catalogs disagree or
    /// cycle, an exact product has the wrong type, or a source emits multiple
    /// products of the requested type.
    pub fn entry_by_typed_asset_id(
        &self,
        asset_id: AssetId,
        expected_type: AssetType,
    ) -> Result<Option<&RascEntry>, TypedAssetResolutionError> {
        let resolved = self.resolve_compatibility_id(asset_id)?;
        if let Some(entry) = self.rasc.entry(resolved) {
            if entry.asset_type() == expected_type {
                return Ok(Some(entry));
            }
            return Err(TypedAssetResolutionError::TypeMismatch {
                asset_id: resolved,
                expected: expected_type,
                actual: entry.asset_type(),
            });
        }
        let Ok(source_id) = SourceAssetId::try_from(resolved) else {
            return Ok(None);
        };
        self.entry_by_unique_source_product(source_id, expected_type)
            .map_err(
                |ambiguity| TypedAssetResolutionError::AmbiguousSourceProducts {
                    source_id: ambiguity.source_id,
                    asset_type: ambiguity.asset_type,
                    products: ambiguity.products,
                },
            )
    }

    fn entry_by_unique_source_product(
        &self,
        source_id: SourceAssetId,
        asset_type: AssetType,
    ) -> Result<Option<&RascEntry>, SourceProductAmbiguity> {
        let mut matching = self
            .rasc
            .entries_by_source(source_id)
            .filter(|entry| entry.asset_type() == asset_type);
        let Some(first) = matching.next() else {
            return Ok(None);
        };
        let Some(second) = matching.next() else {
            return Ok(Some(first));
        };
        let products = std::iter::once(first)
            .chain(std::iter::once(second))
            .chain(matching)
            .map(RascEntry::asset_id)
            .collect();
        Err(SourceProductAmbiguity {
            source_id,
            asset_type,
            products,
        })
    }

    /// Resolve a serialized Lumberyard Asset<T> reference to its exact
    /// compatibility-catalog product or one same-source product.
    ///
    /// Compatibility remapping changes only identity and hint. The serialized
    /// reflected type remains T, so an exact redirected product is returned
    /// even when its catalog product type differs. When the resolved identity
    /// names a source instead of an exact product, the source must have exactly
    /// one product matching the serialized reflected type.
    ///
    /// # Errors
    ///
    /// Returns SerializedAssetReferenceResolutionError when redirects cannot
    /// be resolved consistently or a source has multiple matching products.
    pub fn entry_for_serialized_asset_reference(
        &self,
        asset_id: AssetId,
        serialized_type: AssetType,
    ) -> Result<Option<&RascEntry>, SerializedAssetReferenceResolutionError> {
        let resolved = self.resolve_compatibility_id(asset_id)?;
        if let Some(entry) = self.rasc.entry(resolved) {
            return Ok(Some(entry));
        }
        let Ok(source_id) = SourceAssetId::try_from(resolved) else {
            return Ok(None);
        };
        self.entry_by_unique_source_product(source_id, serialized_type)
            .map_err(
                |ambiguity| SerializedAssetReferenceResolutionError::AmbiguousSourceProducts {
                    source_id: ambiguity.source_id,
                    asset_type: ambiguity.asset_type,
                    products: ambiguity.products,
                },
            )
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

/// One `(product AssetId, payload)` referenced by a [`RascSourceGroup`].
///
/// `extra` is the raw 8-byte tail the binary stores after the product
/// `(guid, sub_id)` in the `@32` product array. Its meaning is unconfirmed
/// because the shipping New World catalog carries an empty product array
/// (`@32` == file size, so every group is empty); it is preserved verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RascProductRef {
    asset_id: AssetId,
    extra: u64,
}

impl RascProductRef {
    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }

    #[must_use]
    pub const fn extra(self) -> u64 {
        self.extra
    }
}

/// A source GUID and the product [`AssetId`]s grouped under it (RASC table a).
///
/// This is the catalog's explicit `source-guid -> products` grouping, distinct
/// from the [`Rasc::entries_by_source`] view derived from the product entries.
/// The New World catalog ships this table empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RascSourceGroup {
    source: AssetId,
    products: Vec<RascProductRef>,
}

impl RascSourceGroup {
    #[must_use]
    pub const fn source(&self) -> AssetId {
        self.source
    }

    #[must_use]
    pub fn products(&self) -> &[RascProductRef] {
        &self.products
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rasc {
    version: u32,
    entries: Vec<RascEntry>,
    source_groups: Vec<RascSourceGroup>,
    path_ids: Vec<PathId>,
    legacy_mappings: Vec<LegacyAssetIdMapping>,
    by_id: HashMap<AssetId, usize>,
    by_path: HashMap<String, usize>,
    by_source: HashMap<Uuid, Vec<usize>>,
    by_path_hash: HashMap<[u8; 16], AssetId>,
    by_legacy_id: HashMap<AssetId, AssetId>,
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
        Self::parse_with_runner(bytes, &nw_jobs::JobRunner::automatic())
    }

    /// Parse a `RASC` catalog using the caller's worker policy.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] under the same conditions as [`Self::parse`].
    pub fn parse_with_runner(bytes: &[u8], runner: &nw_jobs::JobRunner) -> Result<Self, Error> {
        parse_rasc(bytes, runner)
    }

    #[must_use]
    pub fn new(version: u32, entries: Vec<RascEntry>) -> Self {
        Self::new_with_runner(version, entries, &nw_jobs::JobRunner::inline())
    }

    /// Build a catalog from product entries and legacy identity redirects.
    #[must_use]
    pub fn new_with_legacy_mappings(
        version: u32,
        entries: Vec<RascEntry>,
        legacy_mappings: Vec<LegacyAssetIdMapping>,
    ) -> Self {
        let mut catalog = Self::new(version, entries);
        catalog.by_legacy_id = legacy_mappings
            .iter()
            .map(|mapping| (mapping.legacy(), mapping.real()))
            .collect();
        catalog.legacy_mappings = legacy_mappings;
        catalog
    }

    /// Build lookup indexes using the caller's worker policy.
    #[must_use]
    pub fn new_with_runner(
        version: u32,
        entries: Vec<RascEntry>,
        runner: &nw_jobs::JobRunner,
    ) -> Self {
        let lane_count = runner.parallelism().saturating_mul(4).max(1);
        let chunk_size = entries.len().div_ceil(lane_count).max(1);
        let ranges = (0..entries.len())
            .step_by(chunk_size)
            .map(|start| start..entries.len().min(start + chunk_size))
            .collect::<Vec<_>>();
        let partial = runner.map(&ranges, |range| {
            let mut by_id = HashMap::with_capacity(range.len());
            let mut by_path = HashMap::with_capacity(range.len());
            let mut by_source = HashMap::<Uuid, Vec<usize>>::new();
            for index in range.clone() {
                let entry = &entries[index];
                by_id.entry(entry.asset_id()).or_insert(index);
                by_path.entry(entry.path().to_string()).or_insert(index);
                by_source
                    .entry(entry.asset_id().guid)
                    .or_default()
                    .push(index);
            }
            (by_id, by_path, by_source)
        });

        let mut by_id = HashMap::with_capacity(entries.len());
        let mut by_path = HashMap::with_capacity(entries.len());
        let mut by_source = HashMap::<Uuid, Vec<usize>>::new();
        for (partial_id, partial_path, partial_source) in partial {
            for (asset_id, index) in partial_id {
                by_id.entry(asset_id).or_insert(index);
            }
            for (path, index) in partial_path {
                by_path.entry(path).or_insert(index);
            }
            for (guid, mut indices) in partial_source {
                by_source.entry(guid).or_default().append(&mut indices);
            }
        }
        Self {
            version,
            entries,
            source_groups: Vec::new(),
            path_ids: Vec::new(),
            legacy_mappings: Vec::new(),
            by_id,
            by_path,
            by_source,
            by_path_hash: HashMap::new(),
            by_legacy_id: HashMap::new(),
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
        // The catalog's own `path-hash -> AssetId` table keys on
        // `AZ::Uuid::CreateName(normalize(path))`; consult it first, then fall
        // back to the string index built from product entry paths.
        if let Some(asset_id) = self.id_by_path_hash(&asset_path_hash(path)) {
            return Some(asset_id);
        }
        self.entry_by_path(path).map(RascEntry::asset_id)
    }

    /// The parsed `source-guid -> products` grouping table (RASC table a).
    #[must_use]
    pub fn source_groups(&self) -> &[RascSourceGroup] {
        &self.source_groups
    }

    /// The parsed `path-hash -> AssetId` table (RASC table b).
    #[must_use]
    pub fn path_ids(&self) -> &[PathId] {
        &self.path_ids
    }

    /// The parsed `legacy-AssetId -> real-AssetId` redirect table (RASC table c).
    #[must_use]
    pub fn legacy_mappings(&self) -> &[LegacyAssetIdMapping] {
        &self.legacy_mappings
    }

    /// Resolve a legacy [`AssetId`] to its real replacement via table c.
    #[must_use]
    pub fn resolve_legacy_id(&self, asset_id: AssetId) -> Option<AssetId> {
        self.by_legacy_id.get(&asset_id).copied()
    }

    /// Look up an [`AssetId`] by a raw path hash (`AZ::Uuid::CreateName` bytes).
    #[must_use]
    pub fn id_by_path_hash(&self, path_hash: &[u8; 16]) -> Option<AssetId> {
        self.by_path_hash.get(path_hash).copied()
    }

    /// Product entries emitted from the same source GUID as `asset_id`.
    pub fn entries_by_source(
        &self,
        source_id: SourceAssetId,
    ) -> impl ExactSizeIterator<Item = &RascEntry> {
        self.by_source
            .get(&source_id.guid())
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|index| &self.entries[*index])
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
    asset_id_padding: [u8; 12],
    metadata_raw: [u8; 8],
    size_bytes: u32,
    reserved: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GuidAssetInfo {
    asset_id: AssetId,
    metadata_raw: [u8; 8],
    size_bytes: u32,
    reserved: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathId {
    path_hash: [u8; 16],
    asset_id: AssetId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LegacyAssetIdMapping {
    legacy: AssetId,
    real: AssetId,
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
    guid_asset_info: Vec<GuidAssetInfo>,
    path_ids: Vec<PathId>,
    legacy_mappings: Vec<LegacyAssetIdMapping>,
    types: Vec<TypeInfo>,
    dir_blob: Vec<u8>,
    file_blob: Vec<u8>,
    by_id: HashMap<AssetId, usize>,
    by_guid: HashMap<Uuid, usize>,
    by_path_hash: HashMap<[u8; 16], AssetId>,
    by_legacy_id: HashMap<AssetId, AssetId>,
}

impl RaocEntry {
    #[must_use]
    pub const fn new(
        asset_id: AssetId,
        asset_id_padding: [u8; 12],
        metadata_raw: [u8; 8],
        size_bytes: u32,
        reserved: u32,
    ) -> Self {
        Self {
            asset_id,
            asset_id_padding,
            metadata_raw,
            size_bytes,
            reserved,
        }
    }

    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }

    #[must_use]
    pub const fn asset_id_padding(self) -> [u8; 12] {
        self.asset_id_padding
    }

    #[must_use]
    pub const fn metadata_raw(self) -> [u8; 8] {
        self.metadata_raw
    }

    #[must_use]
    pub const fn size_bytes(self) -> u32 {
        self.size_bytes
    }

    #[must_use]
    pub const fn reserved(self) -> u32 {
        self.reserved
    }
}

impl GuidAssetInfo {
    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }

    #[must_use]
    pub const fn metadata_raw(self) -> [u8; 8] {
        self.metadata_raw
    }

    #[must_use]
    pub const fn size_bytes(self) -> u32 {
        self.size_bytes
    }

    #[must_use]
    pub const fn reserved(self) -> u32 {
        self.reserved
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

impl LegacyAssetIdMapping {
    #[must_use]
    pub const fn new(legacy: AssetId, real: AssetId) -> Self {
        Self { legacy, real }
    }

    #[must_use]
    pub const fn legacy(self) -> AssetId {
        self.legacy
    }

    #[must_use]
    pub const fn real(self) -> AssetId {
        self.real
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
        Self::parse_with_runner(bytes, &nw_jobs::JobRunner::automatic())
    }

    /// Parse a `RAOC` catalog using the caller's worker policy.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] under the same conditions as [`Self::parse`].
    pub fn parse_with_runner(bytes: &[u8], runner: &nw_jobs::JobRunner) -> Result<Self, Error> {
        parse_raoc(bytes, runner)
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
    pub fn guid_asset_info(&self) -> &[GuidAssetInfo] {
        &self.guid_asset_info
    }

    #[must_use]
    pub fn path_ids(&self) -> &[PathId] {
        &self.path_ids
    }

    #[must_use]
    pub fn legacy_mappings(&self) -> &[LegacyAssetIdMapping] {
        &self.legacy_mappings
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
    pub fn guid_info(&self, guid: Uuid) -> Option<&GuidAssetInfo> {
        self.by_guid
            .get(&guid)
            .and_then(|index| self.guid_asset_info.get(*index))
    }

    #[must_use]
    pub fn resolve_legacy_id(&self, asset_id: AssetId) -> Option<AssetId> {
        self.by_legacy_id.get(&asset_id).copied()
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
    // The catalog's path-hash table keys on `AZ::Uuid::CreateName(normalize(path))`.
    Uuid::create_name(normalized.as_bytes()).into_bytes()
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

/// Parse the `RASC` catalog.
///
/// # Binary layout (verified against `NewWorld 3-26`, `AZ::Uuid::CreateName`
/// asset-catalog parser at RVA `+0x6367ca0`)
///
/// ```text
/// header (40 bytes):
///   @0  magic "RASC"
///   @4  version (== 1)
///   @8  file_size (u64; validated <= input length, NOT an end sentinel)
///   @16 guid-table base       (16-byte GUIDs, indexed by every table below)
///   @20 asset-type-table base (16-byte type GUIDs)
///   @24 directory-string base (NUL-terminated strings)
///   @28 file-string base      (NUL-terminated strings)
///   @32 product-array base    (16-byte {guidIdx, subId, u64} rows; empty in NW)
///   @36 entry_count
/// entry table:            entry_count * 40-byte records @40
/// [a] source-group table: u32 count + count * 16-byte records
/// [b] path-hash table:    u32 count + count * 12-byte records
/// [c] legacy redirect:    u32 count + count * 16-byte records
/// ```
///
/// The header field at `@32` is the base of the product array (5th table
/// pointer, `table = base + hdr[8]`), not an end sentinel; the binary never
/// compares it to `file_size`. In the shipping catalog it happens to equal the
/// file size only because that array is empty.
fn parse_rasc(bytes: &[u8], runner: &nw_jobs::JobRunner) -> Result<Rasc, Error> {
    if bytes.len() < RASC_HEADER_LEN {
        return Err(Error::InputTooSmall { len: bytes.len() });
    }
    let signature = array_at::<4>(bytes, 0, "RASC signature")?;
    if &signature != RASC_SIGNATURE {
        return Err(Error::InvalidSignature { signature });
    }

    let version = u32_at(bytes, 4, "RASC version")?;
    if version != RASC_VERSION {
        return Err(Error::UnsupportedVersion {
            kind: Kind::Rasc,
            actual: version,
            expected: RASC_VERSION,
        });
    }
    let file_size = u64_at(bytes, 8, "RASC file size")?;
    if file_size > usize_to_u64(bytes.len())? {
        return Err(Error::RascSizeExceedsInput {
            declared: file_size,
            len: bytes.len(),
        });
    }

    let guid_base = usize_from_u32(u32_at(bytes, 16, "RASC GUID table offset")?)?;
    let asset_type_base = usize_from_u32(u32_at(bytes, 20, "RASC asset type table offset")?)?;
    let dir_base = usize_from_u32(u32_at(bytes, 24, "RASC directory string table offset")?)?;
    let file_base = usize_from_u32(u32_at(bytes, 28, "RASC file string table offset")?)?;
    let product_base = usize_from_u32(u32_at(bytes, 32, "RASC product array offset")?)?;
    let entry_count = usize_from_u32(u32_at(bytes, 36, "RASC entry count")?)?;

    let entries = runner.try_map_indexed(entry_count, |index| {
        let offset = checked_add(RASC_HEADER_LEN, checked_mul(index, RASC_ENTRY_LEN)?)?;
        let record = slice_at(bytes, offset, RASC_ENTRY_LEN, "RASC entry")?;
        // rec[0] guid index, rec[1] sub id -> the entry AssetId (map key).
        let guid_index = usize_from_u32(le_u32(record, 0))?;
        let sub_id = le_u32(record, 4);
        // rec[2] (@8) and rec[3] (@12) are the O3DE registry value-record's own
        // copy of the entry AssetId (guid index + sub id, stored at the record's
        // +0x20/+0x30). They are byte-identical to the primary AssetId across the
        // entire 1.6M-entry New World catalog (verified), carry no independent
        // information, and no consumer needs them, so they are intentionally not
        // re-materialized here.
        let asset_type_index = usize_from_u32(le_u32(record, 16))?; // rec[4]
        let size_bytes = le_u32(record, 24); // rec[6] (low u32 of the u64 size @24)
        let dir_string_offset = usize_from_u32(le_u32(record, 32))?; // rec[8]
        let file_string_offset = usize_from_u32(le_u32(record, 36))?; // rec[9]

        let directory = string_at(
            bytes,
            checked_add(dir_base, dir_string_offset)?,
            "RASC directory",
        )?;
        let file_name = string_at(
            bytes,
            checked_add(file_base, file_string_offset)?,
            "RASC file name",
        )?;
        let path = if directory.is_empty() {
            file_name.to_owned()
        } else {
            format!("{directory}/{file_name}")
        };
        let asset_id = AssetId::new(
            guid_by_index(bytes, guid_base, guid_index, "RASC asset id")?,
            sub_id,
        );
        let asset_type = AssetType::new(guid_by_index(
            bytes,
            asset_type_base,
            asset_type_index,
            "RASC asset type",
        )?);
        Ok(RascEntry::new(asset_id, asset_type, path, size_bytes))
    })?;

    // The three side tables are packed contiguously right after the entry table.
    let mut cursor = checked_add(RASC_HEADER_LEN, checked_mul(entry_count, RASC_ENTRY_LEN)?)?;

    // [a] source-guid -> product grouping.
    let source_group_count = usize_from_u32(u32_at(bytes, cursor, "RASC source group count")?)?;
    cursor = checked_add(cursor, 4)?;
    let source_group_start = cursor;
    let source_groups = runner.try_map_indexed(source_group_count, |index| {
        let record = slice_at(
            bytes,
            checked_add(
                source_group_start,
                checked_mul(index, RASC_SOURCE_GROUP_LEN)?,
            )?,
            RASC_SOURCE_GROUP_LEN,
            "RASC source group",
        )?;
        let source = asset_id_by_index(
            bytes,
            guid_base,
            usize_from_u32(le_u32(record, 0))?,
            le_u32(record, 4),
            "RASC source group guid",
        )?;
        let group_count = usize_from_u32(le_u32(record, 8))?;
        let group_base = usize_from_u32(le_u32(record, 12))?;
        let mut products = Vec::with_capacity(group_count);
        for offset in 0..group_count {
            let product_index = checked_add(group_base, offset)?;
            let product = slice_at(
                bytes,
                checked_add(
                    product_base,
                    checked_mul(product_index, RASC_PRODUCT_REF_LEN)?,
                )?,
                RASC_PRODUCT_REF_LEN,
                "RASC product ref",
            )?;
            products.push(RascProductRef {
                asset_id: asset_id_by_index(
                    bytes,
                    guid_base,
                    usize_from_u32(le_u32(product, 0))?,
                    le_u32(product, 4),
                    "RASC product guid",
                )?,
                extra: u64_at(product, 8, "RASC product extra")?,
            });
        }
        Ok(RascSourceGroup { source, products })
    })?;
    cursor = checked_add(
        cursor,
        checked_mul(source_group_count, RASC_SOURCE_GROUP_LEN)?,
    )?;

    // [b] path-hash -> AssetId. The hash is `AZ::Uuid::CreateName(normalize(path))`
    // stored as a GUID in the shared guid table.
    let path_id_count = usize_from_u32(u32_at(bytes, cursor, "RASC path id count")?)?;
    cursor = checked_add(cursor, 4)?;
    let path_id_start = cursor;
    let path_ids = runner.try_map_indexed(path_id_count, |index| {
        let record = slice_at(
            bytes,
            checked_add(path_id_start, checked_mul(index, RASC_PATH_ID_LEN)?)?,
            RASC_PATH_ID_LEN,
            "RASC path id",
        )?;
        Ok(PathId {
            path_hash: guid_bytes_by_index(
                bytes,
                guid_base,
                usize_from_u32(le_u32(record, 0))?,
                "RASC path hash guid",
            )?,
            asset_id: asset_id_by_index(
                bytes,
                guid_base,
                usize_from_u32(le_u32(record, 4))?,
                le_u32(record, 8),
                "RASC path asset guid",
            )?,
        })
    })?;
    cursor = checked_add(cursor, checked_mul(path_id_count, RASC_PATH_ID_LEN)?)?;

    // [c] legacy-AssetId -> real-AssetId redirect.
    let legacy_count = usize_from_u32(u32_at(bytes, cursor, "RASC legacy mapping count")?)?;
    cursor = checked_add(cursor, 4)?;
    let legacy_start = cursor;
    let legacy_mappings = runner.try_map_indexed(legacy_count, |index| {
        let record = slice_at(
            bytes,
            checked_add(legacy_start, checked_mul(index, RASC_LEGACY_MAPPING_LEN)?)?,
            RASC_LEGACY_MAPPING_LEN,
            "RASC legacy mapping",
        )?;
        Ok(LegacyAssetIdMapping::new(
            asset_id_by_index(
                bytes,
                guid_base,
                usize_from_u32(le_u32(record, 0))?,
                le_u32(record, 4),
                "RASC legacy guid",
            )?,
            asset_id_by_index(
                bytes,
                guid_base,
                usize_from_u32(le_u32(record, 8))?,
                le_u32(record, 12),
                "RASC real guid",
            )?,
        ))
    })?;

    let (by_path_hash, by_legacy_id) = runner.join(
        || {
            path_ids
                .iter()
                .map(|entry| (entry.path_hash(), entry.asset_id()))
                .collect()
        },
        || {
            legacy_mappings
                .iter()
                .map(|mapping| (mapping.legacy(), mapping.real()))
                .collect()
        },
    );

    let mut rasc = Rasc::new_with_runner(version, entries, runner);
    rasc.source_groups = source_groups;
    rasc.path_ids = path_ids;
    rasc.legacy_mappings = legacy_mappings;
    rasc.by_path_hash = by_path_hash;
    rasc.by_legacy_id = by_legacy_id;
    Ok(rasc)
}

/// Read a 16-byte GUID from the shared guid table by index.
fn guid_by_index(
    bytes: &[u8],
    base: usize,
    index: usize,
    label: &'static str,
) -> Result<Uuid, Error> {
    uuid_at(
        bytes,
        checked_add(base, checked_mul(index, GUID_LEN)?)?,
        label,
    )
}

/// Read the raw 16 bytes of a GUID-table entry by index.
fn guid_bytes_by_index(
    bytes: &[u8],
    base: usize,
    index: usize,
    label: &'static str,
) -> Result<[u8; 16], Error> {
    array_at::<16>(
        bytes,
        checked_add(base, checked_mul(index, GUID_LEN)?)?,
        label,
    )
}

/// Build an [`AssetId`] from a guid-table index plus a sub id.
fn asset_id_by_index(
    bytes: &[u8],
    base: usize,
    index: usize,
    sub_id: u32,
    label: &'static str,
) -> Result<AssetId, Error> {
    Ok(AssetId::new(
        guid_by_index(bytes, base, index, label)?,
        sub_id,
    ))
}

fn parse_raoc(bytes: &[u8], runner: &nw_jobs::JobRunner) -> Result<Raoc, Error> {
    let header = RaocHeader::parse(bytes)?;
    let mut reader = RaocReader::new(bytes, checked_raoc_entries_end(header.entry_count)?);
    let entries = parse_raoc_entries(bytes, header.entry_count, runner)?;
    let guid_asset_info = reader.table(
        runner,
        RAOC_GUID_INFO_LEN,
        "RAOC GUID-only asset info",
        |record| {
            Ok(GuidAssetInfo {
                asset_id: AssetId::from(uuid_at(record, 0, "RAOC GUID-only asset id")?),
                metadata_raw: record_array(record, 16),
                size_bytes: le_u32(record, 24),
                reserved: le_u32(record, 28),
            })
        },
    )?;
    let path_ids = reader.table(runner, RAOC_PATH_LEN, "RAOC path table", |record| {
        Ok(PathId {
            path_hash: record_array(record, 0),
            asset_id: AssetId::new(
                uuid_at(record, 16, "RAOC path asset id")?,
                le_u32(record, 32),
            ),
        })
    })?;
    let legacy_mappings = reader.table(
        runner,
        RAOC_LEGACY_MAPPING_LEN,
        "RAOC legacy asset-id mappings",
        |record| {
            Ok(LegacyAssetIdMapping::new(
                AssetId::from(uuid_at(record, 0, "RAOC legacy asset id")?),
                AssetId::from(uuid_at(record, 16, "RAOC real asset id")?),
            ))
        },
    )?;
    let types = reader.table(
        runner,
        RAOC_TYPE_LEN,
        "RAOC asset type registry",
        |record| {
            Ok(TypeInfo {
                asset_type: AssetType::new(uuid_at(record, 0, "RAOC asset type")?),
                extension_raw: record_array(record, 16),
                metadata_raw: record_array(record, 32),
            })
        },
    )?;
    let dir_blob = reader.blob("RAOC directory blob")?.to_vec();
    let file_blob = reader.blob("RAOC file blob")?.to_vec();

    let ((by_id, by_guid), (by_path_hash, by_legacy_id)) = runner.join(
        || {
            runner.join(
                || {
                    entries
                        .iter()
                        .enumerate()
                        .map(|(index, entry)| (entry.asset_id(), index))
                        .collect()
                },
                || {
                    guid_asset_info
                        .iter()
                        .enumerate()
                        .map(|(index, entry)| (entry.asset_id().guid, index))
                        .collect()
                },
            )
        },
        || {
            runner.join(
                || {
                    path_ids
                        .iter()
                        .map(|entry| (entry.path_hash(), entry.asset_id()))
                        .collect()
                },
                || {
                    legacy_mappings
                        .iter()
                        .map(|mapping| (mapping.legacy(), mapping.real()))
                        .collect()
                },
            )
        },
    );

    Ok(Raoc {
        version: header.version,
        file_size: header.file_size,
        entries,
        guid_asset_info,
        path_ids,
        legacy_mappings,
        types,
        dir_blob,
        file_blob,
        by_id,
        by_guid,
        by_path_hash,
        by_legacy_id,
    })
}

fn parse_raoc_entries(
    bytes: &[u8],
    count: usize,
    runner: &nw_jobs::JobRunner,
) -> Result<Vec<RaocEntry>, Error> {
    runner.try_map_indexed(count, |index| {
        let offset = checked_add(RAOC_HEADER_LEN, checked_mul(index, RAOC_ENTRY_LEN)?)?;
        let record = slice_at(bytes, offset, RAOC_ENTRY_LEN, "RAOC entry")?;
        Ok(RaocEntry::new(
            AssetId::new(
                uuid_at(record, 0, "RAOC entry asset id")?,
                le_u32(record, 16),
            ),
            record_array(record, 20),
            record_array(record, 32),
            le_u32(record, 40),
            le_u32(record, 44),
        ))
    })
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

    fn table<T: Send>(
        &mut self,
        runner: &nw_jobs::JobRunner,
        stride: usize,
        label: &'static str,
        parse: impl Fn(&[u8]) -> Result<T, Error> + Send + Sync,
    ) -> Result<Vec<T>, Error> {
        let count = usize_from_u32(self.u32(label)?)?;
        let bytes_len = checked_mul(count, stride)?;
        let start = self.cursor;
        let table = self.slice(bytes_len, label)?;
        let rows = runner.try_map_indexed(count, |index| {
            let offset = checked_mul(index, stride)?;
            parse(slice_at(table, offset, stride, label)?)
        })?;
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

fn uuid_at(bytes: &[u8], offset: usize, label: &'static str) -> Result<Uuid, Error> {
    let guid = array_at::<16>(bytes, offset, label)?;
    Ok(Uuid::from_bytes(guid))
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
        let rasc = Rasc::new(1, vec![RascEntry::new(asset_id, asset_type, path, 42)]);
        let raoc = Raoc {
            version: RAOC_VERSION,
            file_size: 0,
            entries: vec![RaocEntry::new(asset_id, [0; 12], [0; 8], 42, 0)],
            guid_asset_info: Vec::new(),
            path_ids: vec![PathId {
                path_hash: asset_path_hash(path),
                asset_id,
            }],
            legacy_mappings: Vec::new(),
            types: Vec::new(),
            dir_blob: Vec::new(),
            file_blob: Vec::new(),
            by_id: [(asset_id, 0)].into_iter().collect(),
            by_guid: HashMap::new(),
            by_path_hash: [(asset_path_hash(path), asset_id)].into_iter().collect(),
            by_legacy_id: HashMap::new(),
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
    fn asset_catalog_resolves_legacy_chains_to_canonical_entries() {
        let legacy = AssetId::new(Uuid::from_u128(1), 1);
        let intermediate = AssetId::new(Uuid::from_u128(2), 2);
        let canonical = AssetId::new(Uuid::from_u128(3), 3);
        let asset_type = AssetType::new(Uuid::from_u128(4));
        let mut rasc = Rasc::new(
            RASC_VERSION,
            vec![RascEntry::new(
                canonical,
                asset_type,
                "objects/canonical.cgf",
                42,
            )],
        );
        rasc.legacy_mappings = vec![
            LegacyAssetIdMapping {
                legacy,
                real: intermediate,
            },
            LegacyAssetIdMapping {
                legacy: intermediate,
                real: canonical,
            },
        ];
        rasc.by_legacy_id = rasc
            .legacy_mappings
            .iter()
            .map(|mapping| (mapping.legacy(), mapping.real()))
            .collect();
        let catalog = AssetCatalog::new(rasc, None);

        assert_eq!(catalog.resolve_compatibility_id(legacy), Ok(canonical));
        assert_eq!(
            catalog
                .entry_by_compatibility_id(legacy)
                .unwrap()
                .map(RascEntry::path),
            Some("objects/canonical.cgf")
        );
        assert_eq!(
            catalog.entry_by_compatibility_id(AssetId::new(Uuid::from_u128(5), 5)),
            Ok(None)
        );
    }

    #[test]
    fn asset_catalog_resolves_source_identity_to_unique_product_type() {
        let guid = Uuid::from_u128(1);
        let source_id = AssetId::new(guid, 0);
        let slice_id = AssetId::new(guid, 2);
        let metadata_id = AssetId::new(guid, 6);
        let slice_type = AssetType::new(Uuid::from_u128(2));
        let catalog = AssetCatalog::new(
            Rasc::new(
                RASC_VERSION,
                vec![
                    RascEntry::new(slice_id, slice_type, "slices/test/target.dynamicslice", 42),
                    RascEntry::new(
                        metadata_id,
                        AssetType::nil(),
                        "slices/test/target.slice.meta",
                        7,
                    ),
                ],
            ),
            None,
        );

        assert_eq!(
            catalog
                .entry_by_typed_asset_id(source_id, slice_type)
                .unwrap()
                .map(RascEntry::asset_id),
            Some(slice_id)
        );
    }

    #[test]
    fn typed_asset_lookup_does_not_treat_missing_product_as_source_identity() {
        let guid = Uuid::from_u128(1);
        let missing_product = AssetId::new(guid, 7);
        let available_product = AssetId::new(guid, 2);
        let asset_type = AssetType::new(Uuid::from_u128(2));
        let catalog = AssetCatalog::new(
            Rasc::new(
                RASC_VERSION,
                vec![RascEntry::new(
                    available_product,
                    asset_type,
                    "slices/test/target.dynamicslice",
                    42,
                )],
            ),
            None,
        );

        assert_eq!(
            catalog.entry_by_typed_asset_id(missing_product, asset_type),
            Ok(None)
        );
    }

    #[test]
    fn typed_asset_lookup_rejects_exact_product_with_wrong_type() {
        let product = AssetId::new(Uuid::from_u128(1), 2);
        let actual_type = AssetType::new(Uuid::from_u128(2));
        let expected_type = AssetType::new(Uuid::from_u128(3));
        let catalog = AssetCatalog::new(
            Rasc::new(
                RASC_VERSION,
                vec![RascEntry::new(
                    product,
                    actual_type,
                    "slices/test/target.dynamicslice",
                    42,
                )],
            ),
            None,
        );

        assert_eq!(
            catalog.entry_by_typed_asset_id(product, expected_type),
            Err(TypedAssetResolutionError::TypeMismatch {
                asset_id: product,
                expected: expected_type,
                actual: actual_type,
            })
        );
    }

    #[test]
    fn serialized_asset_reference_resolves_redirected_exact_product_without_retyping() {
        let legacy_id = AssetId::new(Uuid::from_u128(1), 7);
        let lod_product_id = AssetId::new(Uuid::from_u128(2), 4);
        let serialized_mesh_type =
            AssetType::new(uuid::uuid!("C2869E3B-DDA0-4E01-8FE3-6770D788866B"));
        let lod_product_type = AssetType::new(uuid::uuid!("9AAE4926-CB6A-4C60-9948-A1A22F51DB23"));
        let rasc = Rasc::new_with_legacy_mappings(
            RASC_VERSION,
            vec![RascEntry::new(
                lod_product_id,
                lod_product_type,
                "objects/test/mesh_lod1.azmesh",
                42,
            )],
            vec![LegacyAssetIdMapping::new(legacy_id, lod_product_id)],
        );
        let catalog = AssetCatalog::new(rasc, None);

        assert_eq!(
            catalog
                .entry_for_serialized_asset_reference(legacy_id, serialized_mesh_type)
                .unwrap()
                .map(RascEntry::asset_id),
            Some(lod_product_id)
        );
        assert_eq!(
            catalog.entry_by_typed_asset_id(legacy_id, serialized_mesh_type),
            Err(TypedAssetResolutionError::TypeMismatch {
                asset_id: lod_product_id,
                expected: serialized_mesh_type,
                actual: lod_product_type,
            })
        );
    }

    #[test]
    fn serialized_asset_reference_resolves_unique_source_product_and_rejects_ambiguity() {
        let guid = Uuid::from_u128(1);
        let source_id = AssetId::new(guid, 0);
        let first_product = AssetId::new(guid, 2);
        let second_product = AssetId::new(guid, 3);
        let serialized_type = AssetType::new(Uuid::from_u128(2));
        let other_type = AssetType::new(Uuid::from_u128(3));
        let unique_catalog = AssetCatalog::new(
            Rasc::new(
                RASC_VERSION,
                vec![
                    RascEntry::new(
                        first_product,
                        serialized_type,
                        "slices/test/target.dynamicslice",
                        42,
                    ),
                    RascEntry::new(
                        second_product,
                        other_type,
                        "slices/test/target.slice.meta",
                        7,
                    ),
                ],
            ),
            None,
        );

        assert_eq!(
            unique_catalog
                .entry_for_serialized_asset_reference(source_id, serialized_type)
                .unwrap()
                .map(RascEntry::asset_id),
            Some(first_product)
        );

        let ambiguous_catalog = AssetCatalog::new(
            Rasc::new(
                RASC_VERSION,
                vec![
                    RascEntry::new(
                        first_product,
                        serialized_type,
                        "slices/test/first.dynamicslice",
                        42,
                    ),
                    RascEntry::new(
                        second_product,
                        serialized_type,
                        "slices/test/second.dynamicslice",
                        42,
                    ),
                ],
            ),
            None,
        );
        assert_eq!(
            ambiguous_catalog.entry_for_serialized_asset_reference(source_id, serialized_type),
            Err(
                SerializedAssetReferenceResolutionError::AmbiguousSourceProducts {
                    source_id: SourceAssetId::try_from(source_id).unwrap(),
                    asset_type: serialized_type,
                    products: vec![first_product, second_product],
                }
            )
        );
    }

    #[test]
    fn typed_asset_lookup_rejects_ambiguous_source_products() {
        let guid = Uuid::from_u128(1);
        let source_id = AssetId::new(guid, 0);
        let first_product = AssetId::new(guid, 2);
        let second_product = AssetId::new(guid, 3);
        let asset_type = AssetType::new(Uuid::from_u128(2));
        let catalog = AssetCatalog::new(
            Rasc::new(
                RASC_VERSION,
                vec![
                    RascEntry::new(
                        first_product,
                        asset_type,
                        "slices/test/first.dynamicslice",
                        42,
                    ),
                    RascEntry::new(
                        second_product,
                        asset_type,
                        "slices/test/second.dynamicslice",
                        42,
                    ),
                ],
            ),
            None,
        );

        assert_eq!(
            catalog.entry_by_typed_asset_id(source_id, asset_type),
            Err(TypedAssetResolutionError::AmbiguousSourceProducts {
                source_id: SourceAssetId::try_from(source_id).unwrap(),
                asset_type,
                products: vec![first_product, second_product],
            })
        );
    }

    #[test]
    fn typed_asset_lookup_resolves_legacy_redirect_to_source_identity() {
        let guid = Uuid::from_u128(1);
        let legacy_id = AssetId::new(Uuid::from_u128(2), 9);
        let source_id = AssetId::new(guid, 0);
        let product = AssetId::new(guid, 2);
        let asset_type = AssetType::new(Uuid::from_u128(3));
        let mut rasc = Rasc::new(
            RASC_VERSION,
            vec![RascEntry::new(
                product,
                asset_type,
                "slices/test/target.dynamicslice",
                42,
            )],
        );
        rasc.legacy_mappings = vec![LegacyAssetIdMapping {
            legacy: legacy_id,
            real: source_id,
        }];
        rasc.by_legacy_id.insert(legacy_id, source_id);
        let catalog = AssetCatalog::new(rasc, None);

        assert_eq!(
            catalog
                .entry_by_typed_asset_id(legacy_id, asset_type)
                .unwrap()
                .map(RascEntry::asset_id),
            Some(product)
        );
    }

    #[test]
    fn asset_catalog_rejects_redirect_cycles_and_catalog_disagreement() {
        let first = AssetId::new(Uuid::from_u128(1), 1);
        let second = AssetId::new(Uuid::from_u128(2), 2);
        let third = AssetId::new(Uuid::from_u128(3), 3);
        let mut rasc = Rasc::new(RASC_VERSION, Vec::new());
        rasc.legacy_mappings = vec![
            LegacyAssetIdMapping {
                legacy: first,
                real: second,
            },
            LegacyAssetIdMapping {
                legacy: second,
                real: first,
            },
        ];
        rasc.by_legacy_id = rasc
            .legacy_mappings
            .iter()
            .map(|mapping| (mapping.legacy(), mapping.real()))
            .collect();
        let catalog = AssetCatalog::new(rasc, None);
        assert_eq!(
            catalog.resolve_compatibility_id(first),
            Err(CompatibilityResolutionError::RedirectCycle { asset_id: first })
        );

        let mut rasc = Rasc::new(RASC_VERSION, Vec::new());
        rasc.by_legacy_id.insert(first, second);
        let raoc = Raoc {
            version: RAOC_VERSION,
            file_size: 0,
            entries: Vec::new(),
            guid_asset_info: Vec::new(),
            path_ids: Vec::new(),
            legacy_mappings: vec![LegacyAssetIdMapping {
                legacy: first,
                real: third,
            }],
            types: Vec::new(),
            dir_blob: Vec::new(),
            file_blob: Vec::new(),
            by_id: HashMap::new(),
            by_guid: HashMap::new(),
            by_path_hash: HashMap::new(),
            by_legacy_id: [(first, third)].into_iter().collect(),
        };
        let catalog = AssetCatalog::new(rasc, Some(raoc));
        assert_eq!(
            catalog.resolve_compatibility_id(first),
            Err(CompatibilityResolutionError::RedirectDisagreement {
                asset_id: first,
                rasc_asset_id: second,
                raoc_asset_id: third,
            })
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
        assert_eq!(catalog.entries()[0].size_bytes(), 1668);
        assert_eq!(catalog.entries()[0].reserved(), 7);
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
    fn parses_raoc_legacy_guid_mapping_with_az_byte_order() {
        let legacy = Uuid::parse_str("00002a52-40d6-57a0-a717-19088d1f44f2").unwrap();
        let real = Uuid::parse_str("8c7f0c3a-a71b-5d3e-8afa-142be76f97e0").unwrap();
        let mut bytes = raoc_header(0);
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(legacy.as_bytes());
        bytes.extend_from_slice(real.as_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        patch_file_size(&mut bytes);

        let catalog = Raoc::parse(&bytes).unwrap();

        assert_eq!(
            catalog.legacy_mappings(),
            &[LegacyAssetIdMapping::new(
                AssetId::from(legacy),
                AssetId::from(real)
            )]
        );
        assert_eq!(
            catalog.resolve_legacy_id(AssetId::from(legacy)),
            Some(AssetId::from(real))
        );
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

    fn le(value: u32) -> [u8; 4] {
        value.to_le_bytes()
    }

    fn rasc_entry(
        guid: u32,
        sub: u32,
        type_index: u32,
        size: u32,
        dir_off: u32,
        file_off: u32,
    ) -> [u8; 40] {
        let mut record = [0u8; 40];
        record[0..4].copy_from_slice(&le(guid)); // rec[0] guid index
        record[4..8].copy_from_slice(&le(sub)); // rec[1] sub id
        // rec[2]/rec[3] are the value-record's echo of the AssetId; in the real
        // catalog they equal the primary (guid, sub id), so mirror that here.
        record[8..12].copy_from_slice(&le(guid));
        record[12..16].copy_from_slice(&le(sub));
        record[16..20].copy_from_slice(&le(type_index)); // rec[4]
        record[24..28].copy_from_slice(&le(size)); // rec[6]
        record[32..36].copy_from_slice(&le(dir_off)); // rec[8]
        record[36..40].copy_from_slice(&le(file_off)); // rec[9]
        record
    }

    /// Build a self-contained `RASC` exercising entries plus all three side
    /// tables (source-group, path-hash, and legacy redirect). The product
    /// array is placed last so its base (`@32`) is strictly below the file
    /// size, which the old (removed) sentinel check would have rejected.
    fn build_rasc_fixture() -> (Vec<u8>, RascFixture) {
        let path_a = "objects/weapons/sword.dds";
        let path_b = "root.txt";
        let sub_a = 0x0002_0000u32;
        let sub_b = 3u32;
        let prod_sub = 9u32;

        let guid_a = [0xA0u8; 16];
        let guid_b = [0xB0u8; 16];
        let guid_legacy = [0x4Cu8; 16];
        let guid_product = [0x5Du8; 16];
        let type0 = [0x71u8; 16];
        let type1 = [0x72u8; 16];
        let hash_a = asset_path_hash(path_a);
        let hash_b = asset_path_hash(path_b);
        // guid table indices: 0=A 1=B 2=hashA 3=hashB 4=legacy 5=product
        let guid_table = [guid_a, guid_b, hash_a, hash_b, guid_legacy, guid_product];
        let type_table = [type0, type1];

        let mut dir_blob = vec![0u8]; // offset 0 -> "" (entry B)
        let dir_a_off = dir_blob.len() as u32;
        dir_blob.extend_from_slice(b"objects/weapons\0");

        let mut file_blob = Vec::new();
        let file_a_off = file_blob.len() as u32;
        file_blob.extend_from_slice(b"sword.dds\0");
        let file_b_off = file_blob.len() as u32;
        file_blob.extend_from_slice(b"root.txt\0");

        let mut body = Vec::new();
        // entry table
        body.extend_from_slice(&rasc_entry(0, sub_a, 0, 1234, dir_a_off, file_a_off));
        body.extend_from_slice(&rasc_entry(1, sub_b, 1, 7, 0, file_b_off));
        // [a] source-group table: one group with one product.
        body.extend_from_slice(&le(1));
        body.extend_from_slice(&le(0)); // source guid index
        body.extend_from_slice(&le(sub_a)); // source sub id
        body.extend_from_slice(&le(1)); // group count
        body.extend_from_slice(&le(0)); // group base index into product array
        // [b] path-hash table: two rows.
        body.extend_from_slice(&le(2));
        body.extend_from_slice(&le(2)); // path hash guid index
        body.extend_from_slice(&le(0)); // asset guid index
        body.extend_from_slice(&le(sub_a));
        body.extend_from_slice(&le(3));
        body.extend_from_slice(&le(1));
        body.extend_from_slice(&le(sub_b));
        // [c] legacy redirect table: one row.
        body.extend_from_slice(&le(1));
        body.extend_from_slice(&le(4)); // legacy guid index
        body.extend_from_slice(&le(0)); // legacy sub id
        body.extend_from_slice(&le(0)); // real guid index
        body.extend_from_slice(&le(sub_a)); // real sub id
        // guid table
        let guid_base = 40 + body.len();
        for guid in &guid_table {
            body.extend_from_slice(guid);
        }
        // type table
        let type_base = 40 + body.len();
        for kind in &type_table {
            body.extend_from_slice(kind);
        }
        // directory string blob
        let dir_base = 40 + body.len();
        body.extend_from_slice(&dir_blob);
        // file string blob
        let file_base = 40 + body.len();
        body.extend_from_slice(&file_blob);
        // product array (last -> base < file size)
        let product_base = 40 + body.len();
        body.extend_from_slice(&le(5)); // product guid index
        body.extend_from_slice(&le(prod_sub));
        body.extend_from_slice(&0xDEAD_BEEF_u64.to_le_bytes());

        let total = 40 + body.len();
        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(RASC_SIGNATURE);
        bytes.extend_from_slice(&le(RASC_VERSION));
        bytes.extend_from_slice(&(total as u64).to_le_bytes());
        bytes.extend_from_slice(&le(guid_base as u32));
        bytes.extend_from_slice(&le(type_base as u32));
        bytes.extend_from_slice(&le(dir_base as u32));
        bytes.extend_from_slice(&le(file_base as u32));
        bytes.extend_from_slice(&le(product_base as u32));
        bytes.extend_from_slice(&le(2)); // entry count
        bytes.extend_from_slice(&body);

        let fixture = RascFixture {
            asset_a: AssetId::new(Uuid::from_bytes(guid_a), sub_a),
            asset_b: AssetId::new(Uuid::from_bytes(guid_b), sub_b),
            type_a: AssetType::new(Uuid::from_bytes(type0)),
            legacy: AssetId::new(Uuid::from_bytes(guid_legacy), 0),
            product: AssetId::new(Uuid::from_bytes(guid_product), prod_sub),
            hash_a,
            product_base_below_file_size: (product_base as u64) < (total as u64),
        };
        (bytes, fixture)
    }

    struct RascFixture {
        asset_a: AssetId,
        asset_b: AssetId,
        type_a: AssetType,
        legacy: AssetId,
        product: AssetId,
        hash_a: [u8; 16],
        product_base_below_file_size: bool,
    }

    #[test]
    fn parses_rasc_with_all_three_side_tables() {
        let (bytes, fixture) = build_rasc_fixture();
        // The @32 product base is strictly below file size: the old sentinel
        // check (file_size == @32) would have wrongly rejected this catalog.
        assert!(fixture.product_base_below_file_size);

        let rasc = Rasc::parse(&bytes).unwrap();

        // Entries.
        assert_eq!(rasc.len(), 2);
        assert_eq!(
            rasc.entry_by_path("objects/weapons/sword.dds")
                .unwrap()
                .asset_id(),
            fixture.asset_a
        );
        assert_eq!(
            rasc.entry_by_path("objects/weapons/sword.dds")
                .unwrap()
                .asset_type(),
            fixture.type_a
        );
        assert_eq!(rasc.entry(fixture.asset_a).unwrap().size_bytes(), 1234);
        assert_eq!(
            rasc.entry_by_path("root.txt").unwrap().asset_id(),
            fixture.asset_b
        );

        // [b] path-hash lookups (case/separator insensitive through id_by_path).
        assert_eq!(rasc.path_ids().len(), 2);
        assert_eq!(rasc.id_by_path_hash(&fixture.hash_a), Some(fixture.asset_a));
        assert_eq!(
            rasc.id_by_path(r"Objects\Weapons\Sword.DDS"),
            Some(fixture.asset_a)
        );

        // [c] legacy redirect table is parsed and non-empty.
        assert_eq!(rasc.legacy_mappings().len(), 1);
        assert_eq!(
            rasc.resolve_legacy_id(fixture.legacy),
            Some(fixture.asset_a)
        );

        // [a] source-group table with its product from the @32 array.
        assert_eq!(rasc.source_groups().len(), 1);
        let group = &rasc.source_groups()[0];
        assert_eq!(group.source(), fixture.asset_a);
        assert_eq!(group.products().len(), 1);
        assert_eq!(group.products()[0].asset_id(), fixture.product);
        assert_eq!(group.products()[0].extra(), 0xDEAD_BEEF);
    }

    #[test]
    fn rejects_wrong_rasc_version() {
        let (mut bytes, _) = build_rasc_fixture();
        bytes[4..8].copy_from_slice(&le(2));
        let error = Rasc::parse(&bytes).unwrap_err();
        assert!(matches!(
            error,
            Error::UnsupportedVersion {
                kind: Kind::Rasc,
                actual: 2,
                expected: RASC_VERSION
            }
        ));
    }

    #[test]
    fn rejects_rasc_declared_size_larger_than_input() {
        let (mut bytes, _) = build_rasc_fixture();
        let oversized = (bytes.len() as u64) + 1;
        bytes[8..16].copy_from_slice(&oversized.to_le_bytes());
        assert!(matches!(
            Rasc::parse(&bytes).unwrap_err(),
            Error::RascSizeExceedsInput { .. }
        ));
    }

    #[test]
    fn real_rasc_and_raoc_catalogs_parse_when_present() {
        let rasc_path = Path::new(r"E:\Projects\new-world\resources\assetcatalog.catalog");
        if !rasc_path.exists() {
            eprintln!(
                "skipping: real RASC catalog not present at {}",
                rasc_path.display()
            );
            return;
        }
        let bytes = std::fs::read(rasc_path).unwrap();
        let rasc = Rasc::parse(&bytes).unwrap();
        assert_eq!(rasc.version(), RASC_VERSION);
        assert!(!rasc.is_empty(), "real catalog should have product entries");
        // The standalone RASC carries a large legacy-redirect table that the old
        // parser silently dropped; assert it is now recovered and non-empty.
        assert!(
            !rasc.legacy_mappings().is_empty(),
            "real RASC legacy redirect table must be non-empty"
        );
        assert!(!rasc.path_ids().is_empty());
        // The recovered redirect resolves.
        let mapping = rasc.legacy_mappings()[0];
        assert_eq!(
            rasc.resolve_legacy_id(mapping.legacy()),
            Some(mapping.real())
        );

        let raoc_path =
            Path::new(r"E:\Projects\new-world\resources\assetcatalog_optimized.catalog");
        if raoc_path.exists() {
            let raoc_bytes = std::fs::read(raoc_path).unwrap();
            let raoc = Raoc::parse(&raoc_bytes).unwrap();
            assert!(!raoc.is_empty());
        }
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
