use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use nw_asset::{ASSET_CATALOG_PATH, Error as AssetCatalogError, Rasc};
use nw_datasheet::game_system::{
    GameSystemAsset, GameSystemAssetSource, GameSystemDataTables as GameSystemCatalog,
    GameSystemTable,
};
use nw_datasheet::is_datasheet_path;
use nw_pak::{PakError, PakMmapReader};
use rayon::prelude::*;
use thiserror::Error;

use super::GameDataSourceProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PakDatasheetGameDataSource {
    asset_root: PathBuf,
    pak_paths: Vec<PathBuf>,
}

impl PakDatasheetGameDataSource {
    #[must_use]
    pub fn new<I, P>(asset_root: impl AsRef<Path>, pak_paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut pak_paths = pak_paths
            .into_iter()
            .map(|path| path.as_ref().to_path_buf())
            .collect::<Vec<_>>();
        pak_paths.sort();
        pak_paths.dedup();
        Self {
            asset_root: asset_root.as_ref().to_path_buf(),
            pak_paths,
        }
    }

    pub fn from_asset_root(asset_root: impl AsRef<Path>) -> Result<Self> {
        let asset_root = asset_root.as_ref();
        let mut pak_paths = Vec::new();
        collect_pak_paths(asset_root, &mut pak_paths)
            .with_context(|| format!("collect paks under {}", asset_root.display()))?;
        if pak_paths.is_empty() {
            bail!("no .pak files found under {}", asset_root.display());
        }
        Ok(Self::new(asset_root, pak_paths))
    }

    #[must_use]
    pub fn asset_root(&self) -> &Path {
        &self.asset_root
    }

    #[must_use]
    pub fn pak_paths(&self) -> &[PathBuf] {
        &self.pak_paths
    }
}

impl GameDataSourceProvider for PakDatasheetGameDataSource {
    fn load_catalog(&self) -> Result<GameSystemCatalog> {
        let source = PakDatasheetSource::open(&self.asset_root, &self.pak_paths)
            .context("open pak-backed game-system datasheet source")?;
        source.load_catalog_parallel()
    }
}

pub(crate) fn contains_pak_files(root: &Path) -> io::Result<bool> {
    let mut entries = fs::read_dir(root)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if contains_pak_files(&path)? {
                return Ok(true);
            }
        } else if is_pak_path(&path) {
            return Ok(true);
        }
    }
    Ok(false)
}

struct PakDatasheetSource {
    archives: Vec<PakDatasheetArchive>,
    entries_by_path: BTreeMap<String, PakEntryRef>,
    assets: Vec<GameSystemAsset>,
}

struct PakDatasheetArchive {
    path: PathBuf,
    reader: PakMmapReader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PakEntryRef {
    pak_index: usize,
    entry_name: String,
}

#[derive(Debug, Error)]
enum PakDatasheetSourceError {
    #[error("canonicalize asset root {path:?}: {source}")]
    CanonicalizeAssetRoot {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("canonicalize pak path {path:?}: {source}")]
    CanonicalizePak {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("pak {pak:?} is not under asset root {asset_root:?}")]
    PakOutsideAssetRoot { asset_root: PathBuf, pak: PathBuf },

    #[error("open pak {path:?}: {source}")]
    OpenPak {
        path: PathBuf,
        #[source]
        source: PakError,
    },

    #[error("read pak entry `{entry}` from {pak:?}: {source}")]
    ReadPakEntry {
        pak: PathBuf,
        entry: String,
        #[source]
        source: PakError,
    },

    #[error("parse {entry} from {pak:?}: {source}")]
    Catalog {
        pak: PathBuf,
        entry: &'static str,
        #[source]
        source: AssetCatalogError,
    },

    #[error("{entry} was not found in {pak_count} pak(s) under {asset_root:?}")]
    MissingCatalog {
        asset_root: PathBuf,
        pak_count: usize,
        entry: &'static str,
    },

    #[error("catalog datasheet asset {path:?} was not present in the selected paks")]
    MissingDatasheetAsset { path: PathBuf },

    #[error("internal pak entry pointed at archive index {index}, but only {len} paks are open")]
    ArchiveIndex { index: usize, len: usize },
}

impl PakDatasheetSource {
    fn open(asset_root: &Path, pak_paths: &[PathBuf]) -> Result<Self, PakDatasheetSourceError> {
        let asset_root = canonical_asset_root(asset_root)?;
        let mut archives = Vec::with_capacity(pak_paths.len());
        let mut entries_by_path = BTreeMap::new();
        let mut claimed_paths = BTreeSet::new();

        for pak_path in pak_paths {
            let pak_path = canonical_pak_path(pak_path)?;
            let mount_root = pak_mount_root(&asset_root, &pak_path)?;
            let reader = PakMmapReader::open(&pak_path).map_err(|source| {
                PakDatasheetSourceError::OpenPak {
                    path: pak_path.clone(),
                    source,
                }
            })?;
            let pak_index = archives.len();

            for entry in reader.entries() {
                let path = mounted_entry_path(&mount_root, entry.name());
                let key = normalized_virtual_path(&path);
                if claimed_paths.insert(key.clone()) {
                    entries_by_path.insert(
                        key,
                        PakEntryRef {
                            pak_index,
                            entry_name: entry.name().to_owned(),
                        },
                    );
                }
            }

            archives.push(PakDatasheetArchive {
                path: pak_path,
                reader,
            });
        }

        let catalog = load_catalog_from_paks(&archives, &asset_root)?;
        let assets = catalog
            .iter()
            .filter(|info| is_datasheet_path(info.relative_path()))
            .map(|info| {
                GameSystemAsset::with_asset_id(info.relative_path().to_path_buf(), info.asset_id())
            })
            .collect();

        Ok(Self {
            archives,
            entries_by_path,
            assets,
        })
    }

    fn load_catalog_parallel(&self) -> Result<GameSystemCatalog> {
        let mut assets = self.assets.clone();
        assets.sort_by(|left, right| {
            left.path()
                .cmp(right.path())
                .then(left.asset_id().cmp(&right.asset_id()))
        });
        let mut catalog = GameSystemCatalog::default();
        let batch_size = rayon::current_num_threads().max(1) * 2;
        for batch in assets.chunks(batch_size) {
            let tables = batch
                .par_iter()
                .cloned()
                .map(|asset| {
                    let bytes = self
                        .read_datasheet(&asset)
                        .with_context(|| format!("read datasheet {}", asset.path().display()))?;
                    GameSystemTable::parse_asset(asset, &bytes)
                        .context("parse game-system datasheet")
                })
                .collect::<Result<Vec<_>>>()?;
            for table in tables {
                catalog.insert(table)?;
            }
        }
        Ok(catalog)
    }
}

impl GameSystemAssetSource for PakDatasheetSource {
    type Error = PakDatasheetSourceError;

    fn datasheet_assets(&self) -> std::result::Result<Vec<GameSystemAsset>, Self::Error> {
        Ok(self.assets.clone())
    }

    fn read_datasheet(&self, asset: &GameSystemAsset) -> std::result::Result<Vec<u8>, Self::Error> {
        let key = normalized_virtual_path(asset.path());
        let entry = self.entries_by_path.get(&key).ok_or_else(|| {
            PakDatasheetSourceError::MissingDatasheetAsset {
                path: asset.path().to_path_buf(),
            }
        })?;
        let archive =
            self.archives
                .get(entry.pak_index)
                .ok_or(PakDatasheetSourceError::ArchiveIndex {
                    index: entry.pak_index,
                    len: self.archives.len(),
                })?;
        archive.reader.read(&entry.entry_name).map_err(|source| {
            PakDatasheetSourceError::ReadPakEntry {
                pak: archive.path.clone(),
                entry: entry.entry_name.clone(),
                source,
            }
        })
    }
}

fn load_catalog_from_paks(
    archives: &[PakDatasheetArchive],
    asset_root: &Path,
) -> Result<Rasc, PakDatasheetSourceError> {
    for archive in archives {
        if archive.reader.entry(ASSET_CATALOG_PATH).is_none() {
            continue;
        }
        let bytes = archive.reader.read(ASSET_CATALOG_PATH).map_err(|source| {
            PakDatasheetSourceError::ReadPakEntry {
                pak: archive.path.clone(),
                entry: ASSET_CATALOG_PATH.to_owned(),
                source,
            }
        })?;
        return Rasc::parse(&bytes).map_err(|source| PakDatasheetSourceError::Catalog {
            pak: archive.path.clone(),
            entry: ASSET_CATALOG_PATH,
            source,
        });
    }

    Err(PakDatasheetSourceError::MissingCatalog {
        asset_root: asset_root.to_path_buf(),
        pak_count: archives.len(),
        entry: ASSET_CATALOG_PATH,
    })
}

fn canonical_asset_root(path: &Path) -> Result<PathBuf, PakDatasheetSourceError> {
    fs::canonicalize(path).map_err(|source| PakDatasheetSourceError::CanonicalizeAssetRoot {
        path: path.to_path_buf(),
        source,
    })
}

fn canonical_pak_path(path: &Path) -> Result<PathBuf, PakDatasheetSourceError> {
    fs::canonicalize(path).map_err(|source| PakDatasheetSourceError::CanonicalizePak {
        path: path.to_path_buf(),
        source,
    })
}

fn pak_mount_root(asset_root: &Path, pak_path: &Path) -> Result<String, PakDatasheetSourceError> {
    let relative = pak_path.strip_prefix(asset_root).map_err(|_| {
        PakDatasheetSourceError::PakOutsideAssetRoot {
            asset_root: asset_root.to_path_buf(),
            pak: pak_path.to_path_buf(),
        }
    })?;
    Ok(relative
        .parent()
        .map(normalized_virtual_path)
        .unwrap_or_default())
}

fn mounted_entry_path(mount_root: &str, entry: &str) -> PathBuf {
    let entry = entry.replace('\\', "/");
    let entry = entry.trim_start_matches('/');
    if mount_root.is_empty() {
        PathBuf::from(entry)
    } else if entry.is_empty() {
        PathBuf::from(mount_root)
    } else {
        PathBuf::from(format!("{mount_root}/{entry}"))
    }
}

fn normalized_virtual_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_matches('/')
        .to_ascii_lowercase()
}

fn collect_pak_paths(root: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries = fs::read_dir(root)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_pak_paths(&path, out)?;
        } else if is_pak_path(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_pak_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pak"))
}
