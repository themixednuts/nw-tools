//! The located New World install as an asset source: its paks indexed by virtual
//! path, plus the asset catalog's material map.
//!
//! Several commands need the same plumbing — locate the install, open every pak,
//! build a path → (reader, entry) table of contents, and resolve `MtlName` GUIDs
//! through the catalog. [`Install`] is that shared backbone; the catalog material
//! map is cached on disk and only rebuilt when `Engine.pak` changes (see
//! [`crate::cache`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use nw_asset::{AssetId, Rasc};
use nw_pak::PakMmapReader;
use uuid::Uuid;

use crate::cache::{Cache, CatalogRecord};
use crate::jobs::RunCtx;
use crate::support::PakSet;

/// Locate the New World install, with the standard "not found" guidance.
///
/// # Errors
///
/// Returns an error if no install can be detected.
pub fn locate() -> Result<nw_locator::Install> {
    nw_locator::Install::locate().context(
        "no New World install found; pass a path, or run `nw-tools locate` to check detection",
    )
}

/// Read the asset catalog (rasc, plus the optional raoc) straight from the
/// install's `Engine.pak`, where New World ships it — without building the full
/// table of contents.
///
/// # Errors
///
/// Returns an error if `Engine.pak` cannot be opened or does not contain the
/// primary catalog entry.
pub fn install_catalog_bytes(assets: &Path) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
    let engine = assets.join("Engine.pak");
    let reader =
        PakMmapReader::open(&engine).with_context(|| format!("open {}", engine.display()))?;
    let rasc = reader.read_wrapped(nw_asset::ASSET_CATALOG_PATH).with_context(|| {
        format!("{} not found in {}", nw_asset::ASSET_CATALOG_PATH, engine.display())
    })?;
    let raoc = reader.read_wrapped(nw_asset::ASSET_CATALOG_OPTIMIZED_PATH).ok();
    Ok((rasc, raoc))
}

/// The install's paks indexed by virtual path, plus the catalog material map.
///
/// `read` resolves a virtual path through the pak table-of-contents; `material_path`
/// resolves a `MtlName` GUID to its `.mtl` path through the cached catalog map.
pub struct Install {
    toc: HashMap<String, (Arc<PakMmapReader>, usize)>,
    materials: HashMap<String, String>,
}

impl Install {
    /// Open every pak under `assets`, building the table of contents and loading
    /// the catalog material map (from cache when `Engine.pak` is unchanged).
    ///
    /// # Errors
    ///
    /// Returns an error if no paks are found or the catalog cannot be loaded.
    pub fn open(ctx: &RunCtx, assets: &Path) -> Result<Self> {
        let paks = PakSet::collect(assets.to_path_buf(), Vec::new())?;
        if paks.paths().is_empty() {
            bail!("no pak archives found in {}", assets.display());
        }
        let toc = build_toc(ctx, paks.paths());
        let materials = ensure_catalog_cache(assets)?.material_map();
        Ok(Self { toc, materials })
    }

    /// Read an asset's bytes by virtual path (forward slashes, case-insensitive).
    #[must_use]
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        let (reader, entry) = self.toc.get(&path.to_ascii_lowercase())?;
        reader.read_wrapped_by_index(*entry).ok()
    }

    /// Resolve a `MtlName`-chunk GUID to its `.mtl` path via the cached catalog map.
    #[must_use]
    pub fn material_path(&self, guid: &str) -> Option<String> {
        let id = guid_to_asset_id(guid)?;
        self.materials.get(&id.to_string()).cloned()
    }

    /// Sorted virtual paths whose extension is one of `extensions` (lowercase, no
    /// dot), optionally narrowed to those containing `filter` (case-insensitive).
    #[must_use]
    pub fn paths_with_extensions(&self, extensions: &[&str], filter: Option<&str>) -> Vec<String> {
        let filter = filter.map(str::to_ascii_lowercase);
        let mut paths = self
            .toc
            .keys()
            .filter(|key| {
                key.rsplit_once('.')
                    .is_some_and(|(_, ext)| extensions.contains(&ext))
            })
            .filter(|key| filter.as_ref().is_none_or(|needle| key.contains(needle.as_str())))
            .cloned()
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }
}

/// Build the pak table of contents: every entry's virtual path → (reader, index),
/// enumerated across all archives in parallel.
fn build_toc(ctx: &RunCtx, pak_paths: &[PathBuf]) -> HashMap<String, (Arc<PakMmapReader>, usize)> {
    let per_pak = ctx.runner.map(pak_paths, |path| {
        let mut found = Vec::new();
        if let Ok(reader) = PakMmapReader::open(path) {
            let reader = Arc::new(reader);
            for entry in reader.entries() {
                found.push((entry.name().to_ascii_lowercase(), entry.index(), reader.clone()));
            }
        }
        found
    });
    let mut toc = HashMap::new();
    for list in per_pak {
        for (name, entry, reader) in list {
            toc.insert(name, (reader, entry));
        }
    }
    toc
}

/// What the catalog knows about one asset, for enriching a pak entry.
#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub asset_id: String,
}

/// The RASC catalog indexed by virtual path, for enriching pak entries with their
/// catalog identity.
pub struct CatalogIndex {
    by_path: HashMap<String, AssetInfo>,
}

impl CatalogIndex {
    fn from_records(records: Vec<CatalogRecord>) -> Self {
        let by_path = records
            .into_iter()
            .map(|record| {
                (record.path.to_ascii_lowercase(), AssetInfo { asset_id: record.asset_id })
            })
            .collect();
        Self { by_path }
    }

    /// The catalog identity of a virtual path, if any.
    #[must_use]
    pub fn info(&self, path: &str) -> Option<&AssetInfo> {
        self.by_path.get(&path.to_ascii_lowercase())
    }
}

/// Load the install's RASC catalog index (path → `AssetId`/type), from cache when
/// `Engine.pak` is unchanged.
///
/// # Errors
///
/// Returns an error if `Engine.pak` cannot be read or the catalog cannot be parsed.
pub fn catalog_index(assets: &Path) -> Result<CatalogIndex> {
    Ok(CatalogIndex::from_records(ensure_catalog_cache(assets)?.catalog_records()))
}

/// Ensure the RASC catalog cache is current for `assets`, rebuilding it from
/// `Engine.pak` when the fingerprint changed, and return the opened cache.
///
/// The cache is replaced wholesale on a rebuild (a fresh file), so re-storing can
/// never collide with stale rows; the fingerprint row is written last in the same
/// transaction, so its presence means the rebuild committed.
fn ensure_catalog_cache(assets: &Path) -> Result<Cache> {
    // The leading version forces a rebuild when the cached projection's shape
    // changes, even if Engine.pak itself is unchanged.
    let fingerprint = crate::cache::file_fingerprint(&assets.join("Engine.pak"))
        .map(|fp| format!("v2:{fp}"));
    let db_path = crate::cache::default_path();

    // Fast path: reuse the cache while Engine.pak is unchanged.
    if let Some(fp) = &fingerprint
        && let Ok(cache) = Cache::open(&db_path)
        && cache.fingerprint().as_ref() == Some(fp)
    {
        return Ok(cache);
    }

    // Rebuild from Engine.pak's RASC catalog.
    let (rasc_bytes, _raoc) = install_catalog_bytes(assets)?;
    let records = build_catalog_records(&rasc_bytes)?;
    tracing::debug!("catalog index: {} entries (rebuilt)", records.len());

    match &fingerprint {
        Some(fp) => {
            let _ = std::fs::remove_file(&db_path);
            let mut cache = Cache::open(&db_path)?;
            cache.store(fp, &records)?;
            Ok(cache)
        }
        // Can't fingerprint Engine.pak (rare) — serve this run from a transient cache.
        None => {
            let mut cache = Cache::open_in_memory()?;
            cache.store("transient", &records)?;
            Ok(cache)
        }
    }
}

/// Parse the RASC catalog and project every entry to a [`CatalogRecord`].
fn build_catalog_records(rasc_bytes: &[u8]) -> Result<Vec<CatalogRecord>> {
    let rasc = Rasc::parse(rasc_bytes).context("parse asset catalog (rasc) from Engine.pak")?;
    Ok(rasc
        .entries()
        .iter()
        .map(|entry| CatalogRecord {
            asset_id: entry.asset_id().to_string(),
            path: entry.path().to_string(),
            asset_type: entry.asset_type().to_string(),
            size: i64::from(entry.size_bytes()),
        })
        .collect())
}

/// Map a Cry `MtlName` GUID to a Lumberyard catalog [`AssetId`].
///
/// AzCore stores UUIDs as straight big-endian bytes (`Uuid::CreateName` →
/// `from_be_bytes`), but the `MtlName` chunk records the GUID in Microsoft display
/// form, where Data1/Data2/Data3 are little-endian. Reinterpreting those three
/// fields (`swap_bytes`) yields the AZ-canonical UUID. A `.mtl` is a single-product
/// source asset, so its product sub-id is 0.
#[must_use]
pub fn guid_to_asset_id(guid: &str) -> Option<AssetId> {
    let uuid = Uuid::parse_str(guid.trim()).ok()?;
    let (d1, d2, d3, d4) = uuid.as_fields();
    let canonical = Uuid::from_fields(d1.swap_bytes(), d2.swap_bytes(), d3.swap_bytes(), d4);
    Some(AssetId::new(canonical, 0))
}
