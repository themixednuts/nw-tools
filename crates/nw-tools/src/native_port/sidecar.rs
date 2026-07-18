//! Writer-side mirror of the engine's `azoth.source-meta/v1` sidecar contract.
//!
//! The engine's asset processor owns the authoritative definition
//! (`az_asset_processor::source_meta`); this is the `nw-tools` writer conforming
//! to it byte-for-byte. There is no shared crate and `nw-tools` never depends on
//! `az-rs` (ADR 0028); the golden fixtures prove writer output and reader input
//! agree.
//!
//! # File name
//!
//! For a native source at `<dir>/<file>` (extension included), the sidecar is
//! `<dir>/<file>.azmeta.json`:
//!
//! ```text
//! rendering/textures/items/weapons/sword_diff.png
//! rendering/textures/items/weapons/sword_diff.png.azmeta.json
//! ```
//!
//! # Shape (canonical UTF-8 JSON)
//!
//! ```json
//! { "spec": "azoth.source-meta/v1", "preserved_asset_id": { "guid": "...", "sub_id": 0 } }
//! ```
//!
//! - `spec` (required) must equal [`SOURCE_META_SPEC`].
//! - `preserved_asset_id` (optional) is present **only** for cataloged legacy
//!   assets, carrying the id the native source must keep. It is omitted for
//!   uncataloged assets — the engine mints their identity from the native path
//!   via `AZ::Uuid::CreateName`, which the [`super::identity::fallback_asset_id`]
//!   fallback reproduces exactly.

use serde::{Deserialize, Serialize};

use super::identity::AssetId;

/// Spec identifier carried by every source-metadata sidecar.
pub const SOURCE_META_SPEC: &str = "azoth.source-meta/v1";

/// File-name suffix appended to a native source to name its sidecar.
pub const SIDECAR_SUFFIX: &str = ".azmeta.json";

/// A native source-metadata sidecar (`azoth.source-meta/v1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAssetMeta {
    /// Spec identifier, always [`SOURCE_META_SPEC`].
    pub spec: String,
    /// The preserved catalog identity, present only for cataloged assets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserved_asset_id: Option<AssetId>,
}

impl SourceAssetMeta {
    /// A sidecar that preserves a cataloged identity.
    #[must_use]
    pub fn preserving(preserved_asset_id: AssetId) -> Self {
        Self {
            spec: SOURCE_META_SPEC.to_owned(),
            preserved_asset_id: Some(preserved_asset_id),
        }
    }

    /// A sidecar for an uncataloged source (no preserved identity).
    #[must_use]
    pub fn uncataloged() -> Self {
        Self {
            spec: SOURCE_META_SPEC.to_owned(),
            preserved_asset_id: None,
        }
    }
}

/// Serialize a sidecar to canonical bytes (compact JSON + a trailing newline).
///
/// # Errors
///
/// Returns [`serde_json::Error`] only on an internal serialization fault.
pub fn serialize_sidecar(meta: &SourceAssetMeta) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(meta)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

    #[test]
    fn preserving_sidecar_has_exact_shape() {
        let meta = SourceAssetMeta::preserving(AssetId::new(
            uuid!("11112222-3333-4444-5555-666677778888"),
            0,
        ));
        let text = String::from_utf8(serialize_sidecar(&meta).unwrap()).unwrap();
        assert_eq!(
            text,
            "{\"spec\":\"azoth.source-meta/v1\",\"preserved_asset_id\":{\"guid\":\"11112222-3333-4444-5555-666677778888\",\"sub_id\":0}}\n"
        );
    }

    #[test]
    fn uncataloged_sidecar_omits_preserved_id() {
        let text =
            String::from_utf8(serialize_sidecar(&SourceAssetMeta::uncataloged()).unwrap()).unwrap();
        assert_eq!(text, "{\"spec\":\"azoth.source-meta/v1\"}\n");
    }

    #[test]
    fn round_trips_through_serde() {
        for meta in [
            SourceAssetMeta::preserving(AssetId::new(
                uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
                3,
            )),
            SourceAssetMeta::uncataloged(),
        ] {
            let bytes = serialize_sidecar(&meta).unwrap();
            let parsed: SourceAssetMeta = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(parsed, meta);
            assert_eq!(parsed.spec, SOURCE_META_SPEC);
        }
    }
}
