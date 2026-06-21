use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{
    ASSET_CATALOG_OPTIMIZED_PATH, ASSET_CATALOG_PATH, AssetCatalog, AssetId, AssetType,
    Error as CatalogError, Raoc, Rasc, normalize_virtual_path,
};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AssetStoreError {
    #[error("read asset catalog {path:?}: {source}")]
    ReadCatalog { path: PathBuf, source: io::Error },
    #[error("parse asset catalog {path:?}: {source}")]
    ParseCatalog { path: PathBuf, source: CatalogError },
    #[error("read asset {path:?}: {source}")]
    ReadFile { path: PathBuf, source: io::Error },
    #[error("read directory {path:?}: {source}")]
    ReadDir { path: PathBuf, source: io::Error },
    #[error("read pak {path:?}: {source}")]
    Pak {
        path: PathBuf,
        source: nw_pak::PakError,
    },
    #[error("asset path `{path}` is not relative: {source}")]
    UnsafePath {
        path: Box<str>,
        source: nw_filesystem::SafeJoinError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetInfo {
    path: String,
    asset_id: Option<AssetId>,
    asset_type: Option<AssetType>,
    size_bytes: Option<u64>,
}

impl AssetInfo {
    #[must_use]
    pub fn new(path: impl AsRef<str>) -> Self {
        Self {
            path: normalize_virtual_path(path.as_ref()),
            asset_id: None,
            asset_type: None,
            size_bytes: None,
        }
    }

    #[must_use]
    pub fn from_catalog(entry: &crate::RascEntry) -> Self {
        Self {
            path: entry.path().to_string(),
            asset_id: Some(entry.asset_id()),
            asset_type: Some(entry.asset_type()),
            size_bytes: Some(u64::from(entry.size_bytes())),
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn asset_id(&self) -> Option<AssetId> {
        self.asset_id
    }

    #[must_use]
    pub const fn asset_type(&self) -> Option<AssetType> {
        self.asset_type
    }

    #[must_use]
    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    #[must_use]
    pub const fn is_cataloged(&self) -> bool {
        self.asset_id.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct AssetStore {
    root: PathBuf,
    catalog: Option<AssetCatalog>,
    paks: Vec<PathBuf>,
}

impl AssetStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, AssetStoreError> {
        let root = root.into();
        let catalog = load_catalog(&root)?;
        let paks = collect_paks(&root)?;
        Ok(Self {
            root,
            catalog,
            paks,
        })
    }

    #[must_use]
    pub fn new(
        root: impl Into<PathBuf>,
        catalog: Option<AssetCatalog>,
        paks: Vec<PathBuf>,
    ) -> Self {
        Self {
            root: root.into(),
            catalog,
            paks,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn catalog(&self) -> Option<&AssetCatalog> {
        self.catalog.as_ref()
    }

    #[must_use]
    pub fn paks(&self) -> &[PathBuf] {
        &self.paks
    }

    pub fn catalog_paths(&self) -> impl Iterator<Item = &str> {
        self.catalog
            .as_ref()
            .into_iter()
            .flat_map(|catalog| catalog.entries().iter().map(crate::RascEntry::path))
    }

    #[must_use]
    pub fn info(&self, path: &str) -> AssetInfo {
        self.resolve_path(path)
            .unwrap_or_else(|| AssetInfo::new(path))
    }

    #[must_use]
    pub fn resolve_path(&self, path: &str) -> Option<AssetInfo> {
        let normalized = normalize_virtual_path(path);
        self.catalog
            .as_ref()
            .and_then(|catalog| catalog.entry_by_path(&normalized))
            .map(AssetInfo::from_catalog)
    }

    #[must_use]
    pub fn resolve_id(&self, asset_id: AssetId) -> Option<AssetInfo> {
        self.catalog
            .as_ref()
            .and_then(|catalog| catalog.entry_by_id(asset_id))
            .map(AssetInfo::from_catalog)
    }

    pub fn path(&self, path: &str) -> Result<PathBuf, AssetStoreError> {
        let normalized = normalize_virtual_path(path);
        nw_filesystem::safe_join(&self.root, &normalized).map_err(|source| {
            AssetStoreError::UnsafePath {
                path: normalized.into_boxed_str(),
                source,
            }
        })
    }

    pub fn read_path(&self, path: &str) -> Result<Option<Vec<u8>>, AssetStoreError> {
        self.read(&self.info(path))
    }

    pub fn read(&self, asset: &AssetInfo) -> Result<Option<Vec<u8>>, AssetStoreError> {
        let loose = self.path(asset.path())?;
        if loose.is_file() {
            return fs::read(&loose)
                .map(Some)
                .map_err(|source| AssetStoreError::ReadFile {
                    path: loose,
                    source,
                });
        }

        for pak_path in &self.paks {
            let pak =
                nw_pak::PakMmapReader::open(pak_path).map_err(|source| AssetStoreError::Pak {
                    path: pak_path.clone(),
                    source,
                })?;
            if let Some(entry) = pak.entry(asset.path()) {
                return pak
                    .read_wrapped_by_index(entry.index())
                    .map(Some)
                    .map_err(|source| AssetStoreError::Pak {
                        path: pak_path.clone(),
                        source,
                    });
            }
        }

        Ok(None)
    }
}

pub fn load_catalog(root: &Path) -> Result<Option<AssetCatalog>, AssetStoreError> {
    let rasc_path = root.join(ASSET_CATALOG_PATH);
    if !rasc_path.is_file() {
        return Ok(None);
    }

    let rasc_bytes = fs::read(&rasc_path).map_err(|source| AssetStoreError::ReadCatalog {
        path: rasc_path.clone(),
        source,
    })?;
    let rasc = Rasc::parse(&rasc_bytes).map_err(|source| AssetStoreError::ParseCatalog {
        path: rasc_path,
        source,
    })?;
    let raoc_path = root.join(ASSET_CATALOG_OPTIMIZED_PATH);
    let raoc = if raoc_path.is_file() {
        let bytes = fs::read(&raoc_path).map_err(|source| AssetStoreError::ReadCatalog {
            path: raoc_path.clone(),
            source,
        })?;
        Some(
            Raoc::parse(&bytes).map_err(|source| AssetStoreError::ParseCatalog {
                path: raoc_path,
                source,
            })?,
        )
    } else {
        None
    };

    Ok(Some(AssetCatalog::new(rasc, raoc)))
}

fn collect_paks(root: &Path) -> Result<Vec<PathBuf>, AssetStoreError> {
    let mut paths = Vec::new();
    collect_paks_inner(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_paks_inner(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), AssetStoreError> {
    if path.is_file() {
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pak"))
        {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(AssetStoreError::ReadDir {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    for entry in entries {
        let entry = entry.map_err(|source| AssetStoreError::ReadDir {
            path: path.to_path_buf(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| AssetStoreError::ReadDir {
                path: entry.path(),
                source,
            })?;
        if file_type.is_dir() {
            collect_paks_inner(&entry.path(), out)?;
        } else if file_type.is_file()
            && entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pak"))
        {
            out.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loose_info_normalizes_paths_without_catalog() {
        let store = AssetStore::new("root", None, Vec::new());
        let info = store.info("Objects\\Foo.DDS");

        assert_eq!(info.path(), "objects/foo.dds");
        assert!(!info.is_cataloged());
    }

    #[test]
    fn catalog_info_uses_catalog_identity() {
        let asset_id = AssetId::new(uuid::Uuid::from_u128(1), 2);
        let asset_type = AssetType::new(uuid::Uuid::from_u128(3));
        let rasc = Rasc::from_entries(
            1,
            vec![crate::RascEntry::new(asset_id, asset_type, "a/b.dds", 7)],
        );
        let store = AssetStore::new("root", Some(AssetCatalog::new(rasc, None)), Vec::new());
        let info = store.info("A\\B.dds");

        assert_eq!(info.path(), "a/b.dds");
        assert_eq!(info.asset_id(), Some(asset_id));
        assert_eq!(info.asset_type(), Some(asset_type));
        assert_eq!(info.size_bytes(), Some(7));
    }
}
