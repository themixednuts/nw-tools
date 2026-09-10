//! Asset identities, catalogs, and content access.

#![forbid(unsafe_code)]

mod id;
#[cfg(feature = "assets")]
mod reference;
#[cfg(feature = "assets")]
mod store;

#[cfg(feature = "assets")]
pub mod catalog;
pub mod uuid;

#[cfg(feature = "assets")]
pub use catalog::{
    ASSET_CATALOG_OPTIMIZED_PATH, ASSET_CATALOG_PATH, AssetCatalog, Catalog,
    CompatibilityResolutionError, Error, GuidAssetInfo, Kind, LegacyAssetIdMapping, PathId,
    RAOC_SIGNATURE, RAOC_VERSION, RASC_SIGNATURE, Raoc, RaocEntry, Rasc, RascEntry,
    SerializedAssetReferenceResolutionError, TypeInfo, TypedAssetResolutionError, asset_path_hash,
    detect, is_asset_catalog_path, normalize_virtual_path,
};
pub use id::{
    AssetId, AssetIdParseError, AssetReference, AssetType, SourceAssetId, SourceAssetIdError,
};
#[cfg(feature = "assets")]
pub use reference::{AssetDependencies, AssetDependency, AssetDependencyTarget};
#[cfg(feature = "assets")]
pub use store::{AssetInfo, AssetStore, AssetStoreError, load_catalog};
