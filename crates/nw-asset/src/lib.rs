//! Asset identities, catalogs, and content access.

#![forbid(unsafe_code)]

mod id;
mod reference;
mod store;

pub mod catalog;
pub mod uuid;

pub use catalog::{
    ASSET_CATALOG_OPTIMIZED_PATH, ASSET_CATALOG_PATH, AssetCatalog, Catalog,
    CompatibilityResolutionError, Error, GuidAssetInfo, Kind, LegacyAssetIdMapping, PathId,
    RAOC_SIGNATURE, RAOC_VERSION, RASC_SIGNATURE, Raoc, RaocEntry, Rasc, RascEntry, TypeInfo,
    TypedAssetResolutionError, asset_path_hash, detect, is_asset_catalog_path,
    normalize_virtual_path,
};
pub use id::{
    AssetId, AssetIdParseError, AssetReference, AssetType, SourceAssetId, SourceAssetIdError,
};
pub use reference::{AssetDependencies, AssetDependency, AssetDependencyTarget};
pub use store::{AssetInfo, AssetStore, AssetStoreError, load_catalog};
