//! Writer-side contract for Azoth native source identity metadata.
//!
//! Azoth owns the reader for `azoth.source-meta/v1`. Offline New World tools
//! use this crate as their single writer-side representation so every
//! transformed authoring source preserves the same catalog identity without
//! linking legacy readers into the engine.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Spec identifier carried by every source-metadata sidecar.
pub const SOURCE_META_SPEC: &str = "azoth.source-meta/v1";

/// File-name suffix appended to a native source to name its sidecar.
pub const SIDECAR_SUFFIX: &str = ".azmeta.json";

/// Catalog identity retained by a transformed native source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreservedAssetId {
    pub guid: Uuid,
    pub sub_id: u32,
}

impl PreservedAssetId {
    #[must_use]
    pub const fn new(guid: Uuid, sub_id: u32) -> Self {
        Self { guid, sub_id }
    }
}

/// A native source-metadata sidecar (`azoth.source-meta/v1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAssetMeta {
    pub spec: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserved_asset_id: Option<PreservedAssetId>,
}

impl SourceAssetMeta {
    #[must_use]
    pub fn preserving(preserved_asset_id: PreservedAssetId) -> Self {
        Self {
            spec: SOURCE_META_SPEC.to_owned(),
            preserved_asset_id: Some(preserved_asset_id),
        }
    }

    #[must_use]
    pub fn uncataloged() -> Self {
        Self {
            spec: SOURCE_META_SPEC.to_owned(),
            preserved_asset_id: None,
        }
    }
}

/// Serialize canonical compact JSON followed by one newline.
///
/// # Errors
///
/// Returns a serialization error if the metadata cannot be encoded.
pub fn serialize_sidecar(meta: &SourceAssetMeta) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(meta)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Return the sidecar path for `source_file`.
#[must_use]
pub fn sidecar_path(source_file: &Path) -> PathBuf {
    let mut sidecar = source_file.as_os_str().to_os_string();
    sidecar.push(SIDECAR_SUFFIX);
    PathBuf::from(sidecar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

    #[test]
    fn preserving_sidecar_has_exact_engine_shape() {
        let meta = SourceAssetMeta::preserving(PreservedAssetId::new(
            uuid!("11112222-3333-4444-5555-666677778888"),
            7,
        ));
        assert_eq!(
            String::from_utf8(serialize_sidecar(&meta).unwrap()).unwrap(),
            "{\"spec\":\"azoth.source-meta/v1\",\"preserved_asset_id\":{\"guid\":\"11112222-3333-4444-5555-666677778888\",\"sub_id\":7}}\n"
        );
    }

    #[test]
    fn uncataloged_sidecar_omits_identity() {
        assert_eq!(
            String::from_utf8(serialize_sidecar(&SourceAssetMeta::uncataloged()).unwrap()).unwrap(),
            "{\"spec\":\"azoth.source-meta/v1\"}\n"
        );
    }

    #[test]
    fn sidecar_suffix_is_appended_after_the_source_extension() {
        assert_eq!(
            sidecar_path(Path::new("sharedassets/player.grid.ron")),
            PathBuf::from("sharedassets/player.grid.ron.azmeta.json")
        );
    }
}
