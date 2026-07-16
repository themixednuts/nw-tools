//! SerializeContext-owned render, material, skin, and texture configuration.
//!
//! Legacy `.mtl` and Cry chunk behavior remains in [`crate::material`] and
//! [`crate::geometry`]; these are the generated serialized component records.

pub use nw_reflected_types::types::{
    AzMaterialAssetData, AzMaterialLayer, AzTextureSlot, AzTextureSlotSettings, MaterialEntry,
    MaterialHandle, MaterialOverrideInfo, MaterialProperties, MaterialSet, MaterialSetAsset,
    SimpleAssetReferenceMaterialDataAsset, SimpleAssetReferenceMaterialOverrideAsset,
    SimpleAssetReferenceTextureAsset, SimpleAssetReferenceTextureAtlasAsset, SkinnedMeshComponent,
    SkinnedMeshComponentRenderNode, SkinnedRenderOptions,
};
