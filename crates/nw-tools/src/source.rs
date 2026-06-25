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
use nw_asset::{AssetCatalog, AssetId, Raoc, Rasc};
use nw_pak::PakMmapReader;
use uuid::Uuid;

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
        let materials = load_material_map(ctx, assets, &toc)?;
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

/// Load the catalog's material map (asset-id → `.mtl` path), reusing the on-disk
/// cache when `Engine.pak` is unchanged and otherwise rebuilding it.
fn load_material_map(
    ctx: &RunCtx,
    assets: &Path,
    toc: &HashMap<String, (Arc<PakMmapReader>, usize)>,
) -> Result<HashMap<String, String>> {
    let fingerprint = crate::cache::file_fingerprint(&assets.join("Engine.pak"));
    let db_path = crate::cache::default_path();

    // Fast path: reuse the cache while Engine.pak is unchanged.
    if let Some(fp) = &fingerprint
        && let Ok(cache) = crate::cache::Cache::open(&db_path)
        && cache.fingerprint().as_ref() == Some(fp)
    {
        let map = cache.material_map();
        if !map.is_empty() {
            tracing::debug!("catalog material map: {} entries (cached)", map.len());
            return Ok(map);
        }
    }

    // Rebuild from the catalog, then persist into a fresh cache file (replacing any
    // stale one, so a game patch rebuilds cleanly).
    let map = build_material_map(ctx, toc).context("load asset catalog from Engine.pak")?;
    tracing::debug!("catalog material map: {} entries (rebuilt)", map.len());
    if let Some(fp) = &fingerprint {
        let _ = std::fs::remove_file(&db_path);
        match crate::cache::Cache::open(&db_path).and_then(|cache| Ok(cache.store(fp, &map)?)) {
            Ok(()) => {}
            Err(error) => tracing::warn!("could not write catalog cache: {error}"),
        }
    }
    Ok(map)
}

/// Parse the asset catalog from the pak TOC (rasc + raoc in parallel) and project
/// it to the `.mtl` material map: `AssetId` string → asset path.
fn build_material_map(
    ctx: &RunCtx,
    toc: &HashMap<String, (Arc<PakMmapReader>, usize)>,
) -> Result<HashMap<String, String>> {
    let read = |key: &str| -> Option<Vec<u8>> {
        let (reader, entry) = toc.get(key)?;
        reader.read_wrapped_by_index(*entry).ok()
    };
    let rasc_bytes =
        read(nw_asset::ASSET_CATALOG_PATH).context("assetcatalog.catalog not found in paks")?;
    let raoc_bytes = read(nw_asset::ASSET_CATALOG_OPTIMIZED_PATH);

    let (rasc, raoc) = ctx.runner.join(
        || Rasc::parse(&rasc_bytes),
        || raoc_bytes.as_deref().map(Raoc::parse).transpose(),
    );
    let catalog = AssetCatalog::new(rasc?, raoc?);

    Ok(catalog
        .entries()
        .iter()
        .filter(|entry| entry.path().ends_with(".mtl"))
        .map(|entry| (entry.asset_id().to_string(), entry.path().to_string()))
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
