//! The located New World install as an asset source: its paks indexed by virtual
//! path, plus the complete product and dependency catalog.
//!
//! Several commands need the same plumbing — locate the install, open every pak,
//! build a path → (reader, entry) table of contents, and resolve `MtlName` GUIDs
//! through the catalog. [`Install`] is that shared backbone; the parsed catalog is
//! cached on disk and only rebuilt when `Engine.pak` changes (see [`crate::cache`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use nw_asset::{AssetCatalog, AssetId, Raoc, Rasc, RascEntry};
use nw_pak::PakMmapReader;
use uuid::Uuid;

use crate::cache::Cache;
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
    let rasc = reader
        .read_wrapped(nw_asset::ASSET_CATALOG_PATH)
        .with_context(|| {
            format!(
                "{} not found in {}",
                nw_asset::ASSET_CATALOG_PATH,
                engine.display()
            )
        })?;
    let raoc = reader
        .read_wrapped(nw_asset::ASSET_CATALOG_OPTIMIZED_PATH)
        .ok();
    Ok((rasc, raoc))
}

/// A pak table-of-contents: virtual path (lowercased) → owning reader + entry
/// index, enumerated across a set of archives. The shared seam for commands that
/// stream assets out of the install without needing the catalog.
pub struct Toc {
    entries: HashMap<String, (Arc<PakMmapReader>, usize)>,
}

impl Toc {
    /// Enumerate every pak's entries in parallel, keeping those whose (lowercased)
    /// name satisfies `keep`. On duplicate names the last archive wins.
    #[must_use]
    pub fn build(ctx: &RunCtx, pak_paths: &[PathBuf], keep: impl Fn(&str) -> bool + Sync) -> Self {
        let per_pak = ctx.runner.map(pak_paths, |path| {
            let mut found = Vec::new();
            if let Ok(reader) = PakMmapReader::open(path) {
                let reader = Arc::new(reader);
                for entry in reader.entries() {
                    let name = entry.name().to_ascii_lowercase();
                    if keep(&name) {
                        found.push((name, entry.index(), reader.clone()));
                    }
                }
            }
            found
        });
        let mut entries = HashMap::new();
        for list in per_pak {
            for (name, entry, reader) in list {
                entries.insert(name, (reader, entry));
            }
        }
        Self { entries }
    }

    /// Read an asset's bytes by virtual path (forward slashes, case-insensitive).
    #[must_use]
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        let (reader, entry) = self.entries.get(&path.to_ascii_lowercase())?;
        reader.read_wrapped_by_index(*entry).ok()
    }

    /// The indexed virtual paths.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Consume into the raw path → (reader, index) map (e.g. for a lazy reader).
    #[must_use]
    pub fn into_entries(self) -> HashMap<String, (Arc<PakMmapReader>, usize)> {
        self.entries
    }

    /// Sorted virtual paths whose extension is one of `extensions` (lowercase, no
    /// dot), optionally narrowed to those containing ANY of `filters`
    /// (case-insensitive substring union). An empty `filters` slice matches every
    /// path.
    #[must_use]
    pub fn paths_with_extensions(&self, extensions: &[&str], filters: &[String]) -> Vec<String> {
        let needles = filters
            .iter()
            .map(|needle| needle.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let mut paths = self
            .names()
            .filter(|key| {
                key.rsplit_once('.')
                    .is_some_and(|(_, ext)| extensions.contains(&ext))
            })
            .filter(|key| {
                needles.is_empty() || needles.iter().any(|needle| key.contains(needle.as_str()))
            })
            .map(str::to_string)
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }
}

/// The install's paks indexed by virtual path, plus the resolved asset catalog.
///
/// `read` resolves a virtual path through the pak table-of-contents; `material_path`
/// and dependency queries resolve through the same cached catalog abstraction.
pub struct Install {
    toc: Toc,
    catalog: AssetCatalog,
    /// Hot path index for recursive animation/material/audio glob resolution.
    indexed_paths: HashMap<String, Vec<String>>,
}

impl Install {
    /// Open every pak under `assets`, building the table of contents and loading
    /// the complete catalog (from cache when `Engine.pak` is unchanged).
    ///
    /// # Errors
    ///
    /// Returns an error if no paks are found or the catalog cannot be loaded.
    pub fn open(ctx: &RunCtx, assets: &Path) -> Result<Self> {
        let paks = PakSet::collect(assets.to_path_buf(), Vec::new())?;
        if paks.paths().is_empty() {
            bail!("no pak archives found in {}", assets.display());
        }
        let toc = Toc::build(ctx, paks.paths(), |_| true);
        let catalog = load_install_catalog(assets, &ctx.runner)?;
        let indexed_extensions = ["caf", "i_caf", "bspace", "comb", "dba", "mtl", "bnk", "wem"];
        let mut indexed_paths = HashMap::<String, Vec<String>>::new();
        for path in toc.names() {
            let Some((_, extension)) = path.rsplit_once('.') else {
                continue;
            };
            if indexed_extensions.contains(&extension) {
                indexed_paths
                    .entry(extension.to_owned())
                    .or_default()
                    .push(path.to_owned());
            }
        }
        for paths in indexed_paths.values_mut() {
            paths.sort_unstable();
        }
        Ok(Self {
            toc,
            catalog,
            indexed_paths,
        })
    }

    /// Read an asset's bytes by virtual path (forward slashes, case-insensitive).
    #[must_use]
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.toc.read(path)
    }

    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        self.toc.entries.contains_key(&path.to_ascii_lowercase())
    }

    /// Resolve a `MtlName`-chunk GUID to its `.mtl` path via the cached catalog map.
    #[must_use]
    pub fn material_path(&self, guid: &str) -> Option<String> {
        let id = guid_to_asset_id(guid)?;
        self.catalog
            .entry_by_id(id)
            .filter(|entry| entry.path().ends_with(".mtl"))
            .map(|entry| entry.path().to_owned())
    }

    #[must_use]
    pub const fn catalog(&self) -> &AssetCatalog {
        &self.catalog
    }

    /// Resolve a virtual path or textual `AssetId` to its product entry.
    #[must_use]
    pub fn asset(&self, query: &str) -> Option<&RascEntry> {
        query
            .parse::<AssetId>()
            .ok()
            .and_then(|asset_id| self.catalog.entry_by_id(asset_id))
            .or_else(|| self.catalog.entry_by_path(query))
    }

    /// Sorted virtual paths whose extension is one of `extensions`, optionally
    /// narrowed to those containing ANY of `filters` (case-insensitive union).
    #[must_use]
    pub fn paths_with_extensions(&self, extensions: &[&str], filters: &[String]) -> Vec<String> {
        self.toc.paths_with_extensions(extensions, filters)
    }

    /// Pre-indexed paths for recursive model dependency globs.
    #[must_use]
    pub fn indexed_paths(&self, extension: &str) -> Option<&[String]> {
        self.indexed_paths.get(extension).map(Vec::as_slice)
    }
}

/// Load the install's product catalog from cache when
/// `Engine.pak` is unchanged.
///
/// # Errors
///
/// Returns an error if `Engine.pak` cannot be read or the catalog cannot be parsed.
pub fn catalog_index(assets: &Path) -> Result<AssetCatalog> {
    load_install_catalog(assets, &nw_jobs::JobRunner::automatic())
}

/// Ensure the catalog cache is current for `assets`, rebuilding it from
/// `Engine.pak` when the fingerprint changed.
///
/// The cache is replaced wholesale on a rebuild (a fresh file), so re-storing can
/// never collide with stale rows; the fingerprint row is written last in the same
/// transaction, so its presence means the rebuild committed.
fn load_install_catalog(assets: &Path, runner: &nw_jobs::JobRunner) -> Result<AssetCatalog> {
    // The leading version forces a rebuild when the cached projection's shape
    // changes, even if Engine.pak itself is unchanged.
    let fingerprint =
        crate::cache::file_fingerprint(&assets.join("Engine.pak")).map(|fp| format!("v5:{fp}"));
    let db_path = crate::cache::default_path();

    // Fast path: reuse the cache while Engine.pak is unchanged.
    if let Some(fp) = &fingerprint
        && let Ok(cache) = Cache::open(&db_path)
        && cache.fingerprint().as_ref() == Some(fp)
        && let Ok(catalog) = cache.catalog()
    {
        return Ok(catalog);
    }

    // Rebuild from Engine.pak's RASC and RAOC catalogs.
    let (rasc_bytes, raoc_bytes) = install_catalog_bytes(assets)?;
    let (rasc, raoc) = runner.join(
        || Rasc::parse_with_runner(&rasc_bytes, runner),
        || {
            raoc_bytes
                .as_deref()
                .map(|bytes| Raoc::parse_with_runner(bytes, runner))
                .transpose()
        },
    );
    let rasc = rasc.context("parse asset catalog (rasc) from Engine.pak")?;
    let raoc = raoc.context("parse optimized asset catalog (raoc) from Engine.pak")?;
    let catalog = AssetCatalog::new(rasc, raoc);
    tracing::debug!("catalog index: {} products (rebuilt)", catalog.len());

    if let Some(fp) = &fingerprint {
        let mut cache = Cache::open(&db_path)?;
        cache.store(fp, &catalog)?;
    }
    Ok(catalog)
}

/// Fingerprint the install's complete pak set (paths + sizes + mtimes).
///
/// The install-global authored-dependency index is derived from every pak, so a
/// game patch that adds, resizes, or re-times any archive must invalidate the
/// cached index. The leading version tag also forces a rebuild when the index's
/// build logic changes even if the paks are untouched.
#[must_use]
pub fn dependency_index_fingerprint(assets: &Path) -> Option<String> {
    use std::hash::{Hash, Hasher};

    const VERSION: &str = "depidx-v1";
    let paks = PakSet::collect(assets.to_path_buf(), Vec::new()).ok()?;
    if paks.paths().is_empty() {
        return None;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    VERSION.hash(&mut hasher);
    for path in paks.paths() {
        let metadata = std::fs::metadata(path).ok()?;
        path.to_string_lossy().hash(&mut hasher);
        metadata.len().hash(&mut hasher);
        if let Ok(modified) = metadata.modified()
            && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
        {
            duration.as_secs().hash(&mut hasher);
        }
    }
    Some(format!("{VERSION}:{:016x}", hasher.finish()))
}

/// Load the install-global authored-dependency index from disk cache, rebuilding
/// (and re-caching) it only when the pak-set fingerprint changed.
///
/// Building the whole-install reverse graph costs ~1–2 min and is identical for
/// every character exported from the same paks, so the first export builds it and
/// every later export — each a separate process for a `--filter` run — reuses it.
///
/// # Errors
///
/// Returns an error only if a fresh build fails; a missing or unwritable cache
/// degrades silently to an in-memory build.
pub fn load_or_build_dependency_index(
    assets: &Path,
    source: &dyn nw_asset_graph::AssetSource,
    paths: &[String],
    runner: &nw_jobs::JobRunner,
) -> Result<nw_asset_graph::AssetDependencyIndex> {
    let fingerprint = dependency_index_fingerprint(assets);
    if let Some(fingerprint) = &fingerprint
        && let Some(edges) = crate::cache::load_dependency_index(fingerprint)
    {
        return Ok(nw_asset_graph::AssetDependencyIndex::from_edges(edges));
    }
    let index = nw_asset_graph::AssetDependencyIndex::build_with_runner(source, paths, runner)
        .context("build shared authored-asset dependency index")?;
    if let Some(fingerprint) = &fingerprint
        && let Err(error) = crate::cache::store_dependency_index(fingerprint, index.edges())
    {
        tracing::debug!("dependency index cache store failed: {error:#}");
    }
    Ok(index)
}

/// Map a Cry `MtlName` GUID to a Lumberyard catalog [`AssetId`].
///
/// New World's `MtlName` chunks and asset catalog use the same canonical textual
/// UUID representation. A `.mtl` is a single-product source asset, so its product
/// sub-id is 0.
#[must_use]
pub fn guid_to_asset_id(guid: &str) -> Option<AssetId> {
    Some(AssetId::new(Uuid::parse_str(guid.trim()).ok()?, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cry_material_guid_uses_catalog_uuid_byte_order() {
        let id = guid_to_asset_id("{bca85a99-c440-5e6f-bf24-930333ac3367}").unwrap();

        assert_eq!(
            id.guid,
            Uuid::parse_str("bca85a99-c440-5e6f-bf24-930333ac3367").unwrap()
        );
        assert_eq!(id.sub_id, 0);
    }
}
