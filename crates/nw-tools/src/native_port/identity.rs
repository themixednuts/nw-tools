//! Asset identity assignment.
//!
//! Per ADR 0028: catalog identity wins. When the New World catalog (RASC/RAOC)
//! or a typed legacy reference provides an `AssetId`, the native artifact
//! preserves that GUID and sub-ID (written into the sidecar's
//! `preserved_asset_id`). Otherwise the identity is derived by
//! `AZ::Uuid::CreateName` over the **normalized native path** — bit-identical to
//! what the engine's asset processor mints for an uncataloged source, so the
//! fallback here and the engine agree without a preserved id in the sidecar.
//!
//! `nw-tools` must never depend on `az-rs` (ADR 0028); the engine owns the
//! `AZ::Uuid::CreateName` derivation and this crate mirrors it (duplication is
//! intended) via [`nw_asset::uuid::AzUuidExt`], verified bit-exact against the
//! `NewWorld 3-26` binary's `AZ::Uuid::CreateName` (RVA `0x13e6d80` /
//! `CreateData` RVA `0x13e3180`).

use nw_asset::uuid::AzUuidExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// `AZ::Data::AssetId` `(guid, sub_id)`.
///
/// Serializes as `{ "guid": "<uuid>", "sub_id": <u32> }`, matching
/// `az_core::AssetId` exactly (the shape the engine's source-meta sidecar
/// reader expects).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct AssetId {
    /// Source GUID (preserved from the catalog, or derived by `CreateName`).
    pub guid: Uuid,
    /// Sub-product identifier within the source GUID.
    pub sub_id: u32,
}

impl AssetId {
    /// Construct an [`AssetId`] from its parts.
    #[must_use]
    pub const fn new(guid: Uuid, sub_id: u32) -> Self {
        Self { guid, sub_id }
    }

    /// The nil identity.
    #[must_use]
    pub const fn nil() -> Self {
        Self {
            guid: Uuid::from_u128(0),
            sub_id: 0,
        }
    }

    /// Whether this identity is nil.
    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.sub_id == 0 && self.guid.is_nil()
    }
}

/// How an [`AssetId`] was obtained for a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    /// Preserved from the New World catalog or a typed legacy reference.
    Catalog,
    /// Derived by `AZ::Uuid::CreateName` over the normalized native path.
    Fallback,
}

/// Normalize a virtual source path the way the engine catalog does before
/// hashing it into an `AssetId`.
///
/// Converts separators to `/`, trims surrounding whitespace, lowercases (ASCII),
/// strips leading `./`, drops leading/trailing slashes, and collapses repeated
/// internal slashes. Two spellings differing only in redundant/trailing
/// separators normalize identically and hash to the same GUID.
#[must_use]
pub fn normalize_source_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/").trim().to_ascii_lowercase();
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    let trimmed = normalized.trim_matches('/');
    let mut collapsed = String::with_capacity(trimmed.len());
    let mut previous_was_slash = false;
    for ch in trimmed.chars() {
        if ch == '/' {
            if previous_was_slash {
                continue;
            }
            previous_was_slash = true;
        } else {
            previous_was_slash = false;
        }
        collapsed.push(ch);
    }
    collapsed
}

/// Derive the path-based source GUID for a native source path.
///
/// Equivalent to the engine's `az_asset_builder::source_guid`: normalize the
/// path, then apply `AZ::Uuid::CreateName`.
#[must_use]
pub fn source_guid(native_path: &str) -> Uuid {
    Uuid::create_name(normalize_source_path(native_path).as_bytes())
}

/// `AZ::Uuid::CreateName` fallback identity for a source with no catalog id.
///
/// The name is the source's **native destination path** (not the legacy path,
/// machine path, PAK order, or content hash), matching the engine asset
/// processor, which scans the native `assets/` tree and derives the source
/// identity from that native path. Sub-ID `0` is the primary product.
#[must_use]
pub fn fallback_asset_id(native_path: &str) -> AssetId {
    AssetId::new(source_guid(native_path), 0)
}

/// Assign identity: catalog identity wins, otherwise the `CreateName` fallback
/// over the native path.
#[must_use]
pub fn assign_asset_id(
    native_path: &str,
    catalog_identity: Option<AssetId>,
) -> (AssetId, IdentitySource) {
    match catalog_identity {
        Some(id) if !id.is_nil() => (id, IdentitySource::Catalog),
        _ => (fallback_asset_id(native_path), IdentitySource::Fallback),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_name_matches_azcore_hello_vector() {
        // Identical to `nw_asset::uuid::AzUuidExt::create_name(b"hello")`.
        assert_eq!(
            Uuid::create_name(b"hello").to_string(),
            "aaf4c61d-dcc5-58a2-9abe-de0f3b482cd9"
        );
    }

    #[test]
    fn create_name_empty_is_nil() {
        assert!(Uuid::create_name(b"").is_nil());
    }

    #[test]
    fn normalize_is_case_and_separator_stable() {
        assert_eq!(
            normalize_source_path(r".\Objects\\Foo.CGF\"),
            "objects/foo.cgf"
        );
        assert_eq!(
            source_guid("objects/Foo.cgf"),
            source_guid(r"Objects\foo.CGF")
        );
    }

    #[test]
    fn source_guid_matches_engine_for_native_path() {
        // Pinned against az-rs `source_guid(...)` for the same native path so an
        // uncataloged fallback is bit-identical to what the asset processor mints.
        assert_eq!(
            source_guid("rendering/textures/items/weapons/sword_diff.png").to_string(),
            "4885e9c3-12c8-5a73-ace2-dcd3ddf68043"
        );
    }

    #[test]
    fn fallback_is_deterministic_for_a_given_native_path() {
        let a = fallback_asset_id("rendering/textures/items/weapons/sword_diff.png");
        let b = fallback_asset_id("rendering/textures/items/weapons/sword_diff.png");
        assert_eq!(a, b);
        assert_eq!(a.sub_id, 0);
    }

    #[test]
    fn fallback_uses_create_name_over_the_native_path() {
        let native = "rendering/textures/items/weapons/sword_diff.png";
        // Bit-identical to what the engine asset processor mints (create_name
        // of the normalized native path); pinned against az-rs source_guid.
        assert_eq!(
            fallback_asset_id(native).guid.to_string(),
            "4885e9c3-12c8-5a73-ace2-dcd3ddf68043"
        );
    }

    #[test]
    fn different_native_paths_get_different_ids() {
        let a = fallback_asset_id("rendering/textures/a.png");
        let b = fallback_asset_id("rendering/textures/b.png");
        assert_ne!(a, b);
    }

    #[test]
    fn catalog_identity_wins_over_fallback() {
        let native = "rendering/textures/a.png";
        let catalog = AssetId::new(Uuid::from_u128(0x1234), 7);
        let (id, source) = assign_asset_id(native, Some(catalog));
        assert_eq!(id, catalog);
        assert_eq!(source, IdentitySource::Catalog);

        let (id, source) = assign_asset_id(native, None);
        assert_eq!(id, fallback_asset_id(native));
        assert_eq!(source, IdentitySource::Fallback);

        // A nil catalog id is treated as absent.
        let (id, source) = assign_asset_id(native, Some(AssetId::nil()));
        assert_eq!(source, IdentitySource::Fallback);
        assert_eq!(id, fallback_asset_id(native));
    }
}
