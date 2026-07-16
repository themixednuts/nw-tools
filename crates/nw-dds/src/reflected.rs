//! SerializeContext-owned texture references and legacy slot settings.
//!
//! DDS container and pixel decoding is independent of these authored asset
//! references, but both are exposed from the texture-domain crate.

pub use nw_reflected_types::types::{
    AzTextureSlot, AzTextureSlotSettings, SimpleAssetReferenceTextureAsset,
    SimpleAssetReferenceTextureAtlasAsset,
};
