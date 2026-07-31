//! Re-export the shared writer-side `azoth.source-meta/v1` contract.
//!
//! The engine owns the reader. `nw-source-meta` is the single lightweight
//! writer contract used by both this offline port and project extraction tools.

pub use nw_source_meta::{
    PreservedAssetId, SIDECAR_SUFFIX, SOURCE_META_SPEC, SourceAssetMeta, serialize_sidecar,
    sidecar_path,
};

use super::identity::AssetId;

impl From<AssetId> for PreservedAssetId {
    fn from(value: AssetId) -> Self {
        Self::new(value.guid, value.sub_id)
    }
}
