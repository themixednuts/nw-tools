//! Asset identities, catalogs, and content access.

#![forbid(unsafe_code)]

mod id;
mod store;

pub mod catalog;

pub use catalog::{
    ASSET_CATALOG_OPTIMIZED_PATH, ASSET_CATALOG_PATH, AssetCatalog, AuxIndex, Catalog, Dependency,
    Error, Kind, PathId, RAOC_SIGNATURE, RAOC_VERSION, RASC_SIGNATURE, Raoc, RaocEntry, Rasc,
    RascEntry, TypeInfo, asset_path_hash, detect, is_asset_catalog_path, normalize_virtual_path,
};
pub use id::{AssetId, AssetIdParseError, AssetReference, AssetType};
pub use store::{AssetInfo, AssetStore, AssetStoreError, load_catalog};
