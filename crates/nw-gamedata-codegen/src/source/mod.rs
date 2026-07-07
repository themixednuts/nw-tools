use std::path::Path;

use anyhow::{Context, Result, bail};
use nw_datasheet::game_system::GameSystemDataTables as GameSystemCatalog;

mod filesystem;
mod pak;

pub use filesystem::FilesystemDatasheetGameDataSource;
pub use pak::PakDatasheetGameDataSource;

pub trait GameDataSourceProvider {
    fn load_catalog(&self) -> Result<GameSystemCatalog>;
}

pub fn load_catalog_from_asset_root(asset_root: &Path) -> Result<GameSystemCatalog> {
    if pak::contains_pak_files(asset_root)
        .with_context(|| format!("scan {} for pak-backed GameData", asset_root.display()))?
    {
        return PakDatasheetGameDataSource::from_asset_root(asset_root)?.load_catalog();
    }

    if filesystem::contains_loose_datasheets(asset_root).with_context(|| {
        format!(
            "scan {} for loose GameData datasheets",
            asset_root.display()
        )
    })? {
        return FilesystemDatasheetGameDataSource::new(asset_root).load_catalog();
    }

    bail!(
        "expected New World .pak files or a loose datasheet snapshot under {}",
        asset_root.display()
    )
}
