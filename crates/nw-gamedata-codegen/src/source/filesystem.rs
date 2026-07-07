use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Context, Result};
use nw_datasheet::game_system::{
    GameSystemAsset, GameSystemAssetSource, GameSystemDataTables as GameSystemCatalog,
};
use nw_datasheet::is_datasheet_path;

use super::GameDataSourceProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemDatasheetGameDataSource {
    root: PathBuf,
}

impl FilesystemDatasheetGameDataSource {
    #[must_use]
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl GameDataSourceProvider for FilesystemDatasheetGameDataSource {
    fn load_catalog(&self) -> Result<GameSystemCatalog> {
        let source = FilesystemDatasheetSource::open(&self.root)
            .context("open filesystem game-system datasheet source")?;
        GameSystemCatalog::load_from_source(&source).context("load game-system datasheets")
    }
}

#[derive(Debug)]
struct FilesystemDatasheetSource {
    root: PathBuf,
    assets: Vec<GameSystemAsset>,
}

impl FilesystemDatasheetSource {
    fn open(root: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        let mut assets = Vec::new();
        collect_datasheet_assets(&root, &root, &mut assets)?;
        Ok(Self { root, assets })
    }
}

impl GameSystemAssetSource for FilesystemDatasheetSource {
    type Error = std::io::Error;

    fn datasheet_assets(&self) -> std::result::Result<Vec<GameSystemAsset>, Self::Error> {
        Ok(self.assets.clone())
    }

    fn read_datasheet(&self, asset: &GameSystemAsset) -> std::result::Result<Vec<u8>, Self::Error> {
        fs::read(self.root.join(asset.path()))
    }
}

pub(crate) fn contains_loose_datasheets(root: &Path) -> io::Result<bool> {
    let mut entries = fs::read_dir(root)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::path);

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if contains_loose_datasheets(&path)? {
                return Ok(true);
            }
            continue;
        }
        if !is_datasheet_path(&path) {
            continue;
        }
        let bytes = fs::read(&path)?;
        if is_datasheet_binary(&bytes) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn collect_datasheet_assets(
    root: &Path,
    dir: &Path,
    assets: &mut Vec<GameSystemAsset>,
) -> std::io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::path);

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_datasheet_assets(root, &path, assets)?;
            continue;
        }
        if !is_datasheet_path(&path) {
            continue;
        }
        let bytes = fs::read(&path)?;
        if !is_datasheet_binary(&bytes) {
            continue;
        }

        let relative = path.strip_prefix(root).expect("datasheet path under root");
        assets.push(GameSystemAsset::new(relative.to_path_buf()));
    }
    Ok(())
}

fn is_datasheet_binary(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    version == 0x11 || version == 0x12
}
