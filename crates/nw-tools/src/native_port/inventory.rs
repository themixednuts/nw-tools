//! Pass-1 inventory: normalize, consult exceptions, route, and assign identity.
//!
//! [`Mapper::map_all`] applies the deterministic mapper to a batch of legacy
//! inputs and accumulates a full [`Inventory`] report (counts per legacy tree,
//! collisions, exceptions hit, uncataloged fallbacks, and every hard error)
//! without writing any output. The port is publishable only when the inventory
//! has no blocking errors.

use std::collections::BTreeMap;
use std::collections::HashMap;

use super::exception::{ExceptionError, ExceptionOutcome, ExceptionTable};
use super::identity::{AssetId, IdentitySource, assign_asset_id};
use super::normalize::{NormalizeError, normalize_legacy_path};
use super::route::{DecodedClass, RouteError, map_native_path};

/// A single legacy source presented to the mapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInput {
    /// The raw legacy key/path, in original spelling.
    pub legacy_key: String,
    /// The decoded content class.
    pub class: DecodedClass,
    /// Catalog identity, when the RASC/RAOC catalog provides one.
    pub catalog_identity: Option<AssetId>,
}

impl SourceInput {
    /// Construct a source input.
    #[must_use]
    pub fn new(
        legacy_key: impl Into<String>,
        class: DecodedClass,
        catalog_identity: Option<AssetId>,
    ) -> Self {
        Self {
            legacy_key: legacy_key.into(),
            class,
            catalog_identity,
        }
    }
}

/// A legacy source that was successfully mapped to a native destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedSource {
    /// Normalized legacy path (the identity key and UUIDv5 name).
    pub normalized_legacy_path: String,
    /// Original legacy spelling (offline report only).
    pub original_spelling: String,
    /// Canonical native destination path under `assets/`.
    pub native_path: String,
    /// Assigned/preserved identity.
    pub asset_id: AssetId,
    /// Whether identity came from the catalog or the UUIDv5 fallback.
    pub identity_source: IdentitySource,
    /// Native asset type (`AssetData::STABLE_NAME`).
    pub asset_type: String,
    /// Decoded class.
    pub class: DecodedClass,
    /// Logical subproduct name, when applicable.
    pub subproduct: Option<String>,
}

/// A source whose destination is fixed but whose publication is blocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedSource {
    /// Normalized legacy path.
    pub normalized_legacy_path: String,
    /// Fixed canonical destination.
    pub canonical_path: String,
    /// Reviewed reason publication is blocked.
    pub reason: String,
}

/// Two inputs collide onto one normalized key or one native path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    /// What kind of collision occurred.
    pub kind: CollisionKind,
    /// The colliding normalized key or native path.
    pub key: String,
    /// Original spelling of the first input.
    pub first_input: String,
    /// Original spelling of the second input.
    pub second_input: String,
}

/// The domain in which a collision occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionKind {
    /// Two raw keys normalized to the same source path.
    NormalizedKey,
    /// Two sources mapped to the same native path.
    NativePath,
}

/// A source that failed to route (unmapped) or normalize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapFailure {
    /// Original legacy spelling.
    pub original_spelling: String,
    /// Human-readable failure detail.
    pub detail: String,
}

/// The accumulated pass-1 inventory.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    /// Publishable mapped sources.
    pub sources: Vec<MappedSource>,
    /// Publication-blocked sources (exception hard-fail).
    pub blocked: Vec<BlockedSource>,
    /// Normalized-key and native-path collisions.
    pub collisions: Vec<Collision>,
    /// Unmapped route failures.
    pub route_errors: Vec<MapFailure>,
    /// Normalization failures.
    pub normalize_errors: Vec<MapFailure>,
    /// Count of sources per legacy top-level tree.
    pub tree_counts: BTreeMap<String, usize>,
    /// Normalized legacy paths that matched an exception entry.
    pub exceptions_hit: Vec<String>,
    /// Number of sources whose identity came from the UUIDv5 fallback.
    pub uncataloged_fallbacks: usize,
}

impl Inventory {
    /// Whether the inventory has any error that blocks publication.
    #[must_use]
    pub fn has_blocking_errors(&self) -> bool {
        !self.blocked.is_empty()
            || !self.collisions.is_empty()
            || !self.route_errors.is_empty()
            || !self.normalize_errors.is_empty()
    }
}

/// The deterministic legacy-to-native mapper.
#[derive(Debug, Clone)]
pub struct Mapper {
    exceptions: ExceptionTable,
}

impl Mapper {
    /// Build a mapper from a loaded exception table.
    #[must_use]
    pub const fn new(exceptions: ExceptionTable) -> Self {
        Self { exceptions }
    }

    /// Build a mapper from the pinned reviewed exception table.
    ///
    /// # Errors
    ///
    /// Returns [`ExceptionError`] if the pinned exception data is invalid.
    pub fn pinned() -> Result<Self, ExceptionError> {
        Ok(Self::new(ExceptionTable::pinned()?))
    }

    /// The reviewed exception-table revision.
    #[must_use]
    pub const fn exception_revision(&self) -> u32 {
        self.exceptions.revision()
    }

    /// Map a batch of inputs into a full inventory report.
    #[must_use]
    pub fn map_all<I>(&self, inputs: I) -> Inventory
    where
        I: IntoIterator<Item = SourceInput>,
    {
        let mut inv = Inventory::default();
        // normalized key -> original spelling (normalized-key collision guard)
        let mut by_key: HashMap<String, String> = HashMap::new();
        // native path -> original spelling (native-path collision guard)
        let mut by_native: HashMap<String, String> = HashMap::new();

        for input in inputs {
            let normalized = match normalize_legacy_path(&input.legacy_key) {
                Ok(normalized) => normalized,
                Err(err) => {
                    inv.normalize_errors.push(MapFailure {
                        original_spelling: input.legacy_key.clone(),
                        detail: normalize_detail(&err),
                    });
                    continue;
                }
            };

            if let Some(previous) = by_key.get(&normalized) {
                inv.collisions.push(Collision {
                    kind: CollisionKind::NormalizedKey,
                    key: normalized.clone(),
                    first_input: previous.clone(),
                    second_input: input.legacy_key.clone(),
                });
                continue;
            }
            by_key.insert(normalized.clone(), input.legacy_key.clone());

            // Exceptions win over the generic route.
            let (native_path, subproduct_note) = match self.exceptions.lookup(&normalized) {
                ExceptionOutcome::Blocked(entry) => {
                    inv.exceptions_hit.push(normalized.clone());
                    inv.blocked.push(BlockedSource {
                        normalized_legacy_path: normalized.clone(),
                        canonical_path: entry.canonical_path.clone(),
                        reason: entry.reason.clone(),
                    });
                    continue;
                }
                ExceptionOutcome::Route(entry) => {
                    inv.exceptions_hit.push(normalized.clone());
                    (entry.canonical_path.clone(), None)
                }
                ExceptionOutcome::None => match map_native_path(&normalized, input.class) {
                    Ok(path) => (path, None),
                    Err(err) => {
                        inv.route_errors.push(MapFailure {
                            original_spelling: input.legacy_key.clone(),
                            detail: route_detail(&err),
                        });
                        continue;
                    }
                },
            };

            if let Some(previous) = by_native.get(&native_path) {
                inv.collisions.push(Collision {
                    kind: CollisionKind::NativePath,
                    key: native_path.clone(),
                    first_input: previous.clone(),
                    second_input: input.legacy_key.clone(),
                });
                continue;
            }
            by_native.insert(native_path.clone(), input.legacy_key.clone());

            // The fallback identity is minted over the NATIVE destination path
            // (via AZ::Uuid::CreateName), matching what the engine asset
            // processor derives when it later scans this native source.
            let (asset_id, identity_source) = assign_asset_id(&native_path, input.catalog_identity);
            if identity_source == IdentitySource::Fallback {
                inv.uncataloged_fallbacks += 1;
            }

            let tree = normalized
                .split('/')
                .next()
                .unwrap_or(&normalized)
                .to_owned();
            *inv.tree_counts.entry(tree).or_insert(0) += 1;

            inv.sources.push(MappedSource {
                normalized_legacy_path: normalized,
                original_spelling: input.legacy_key,
                native_path,
                asset_id,
                identity_source,
                asset_type: input.class.native_asset_type().to_owned(),
                class: input.class,
                subproduct: subproduct_note,
            });
        }

        inv
    }
}

fn normalize_detail(err: &NormalizeError) -> String {
    err.to_string()
}

fn route_detail(err: &RouteError) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn maps_textures_and_reports_tree_counts_and_fallbacks() {
        let inv = Mapper::pinned().unwrap().map_all([
            SourceInput::new("Textures/Items/Sword.png", DecodedClass::Image, None),
            SourceInput::new(
                "materials/decals/blood.material.ron",
                DecodedClass::Material,
                Some(AssetId::new(Uuid::from_u128(9), 3)),
            ),
        ]);

        assert!(!inv.has_blocking_errors());
        assert_eq!(inv.sources.len(), 2);
        // Counts are per legacy tree (ADR 0028), not per native destination.
        assert_eq!(inv.tree_counts.get("textures"), Some(&1));
        assert_eq!(inv.tree_counts.get("materials"), Some(&1));
        assert_eq!(inv.uncataloged_fallbacks, 1);

        let texture = &inv.sources[0];
        assert_eq!(texture.native_path, "rendering/textures/items/sword.png");
        assert_eq!(texture.identity_source, IdentitySource::Fallback);

        let material = &inv.sources[1];
        assert_eq!(material.asset_id, AssetId::new(Uuid::from_u128(9), 3));
        assert_eq!(material.identity_source, IdentitySource::Catalog);
    }

    #[test]
    fn normalized_key_collision_names_both_inputs() {
        let inv = Mapper::pinned().unwrap().map_all([
            SourceInput::new("Textures/Items/Sword.png", DecodedClass::Image, None),
            SourceInput::new(r"textures\items\sword.png", DecodedClass::Image, None),
        ]);
        assert_eq!(inv.collisions.len(), 1);
        let collision = &inv.collisions[0];
        assert_eq!(collision.kind, CollisionKind::NormalizedKey);
        assert_eq!(collision.first_input, "Textures/Items/Sword.png");
        assert_eq!(collision.second_input, r"textures\items\sword.png");
        assert!(inv.has_blocking_errors());
    }

    #[test]
    fn native_path_collision_names_both_inputs() {
        // Two distinct legacy keys (a `materials/` material and a decoded-type
        // `textures/` material) route to the same native path.
        let inv = Mapper::pinned().unwrap().map_all([
            SourceInput::new(
                "materials/decals/x.material.ron",
                DecodedClass::Material,
                None,
            ),
            SourceInput::new(
                "textures/decals/x.material.ron",
                DecodedClass::Material,
                None,
            ),
        ]);
        assert_eq!(inv.collisions.len(), 1);
        assert_eq!(inv.collisions[0].kind, CollisionKind::NativePath);
        assert_eq!(
            inv.collisions[0].key,
            "rendering/materials/decals/x.material.ron"
        );
        assert_eq!(
            inv.collisions[0].first_input,
            "materials/decals/x.material.ron"
        );
        assert_eq!(
            inv.collisions[0].second_input,
            "textures/decals/x.material.ron"
        );
        assert!(inv.has_blocking_errors());
    }

    #[test]
    fn blocked_exception_is_reported_and_blocks_publication() {
        let inv = Mapper::pinned().unwrap().map_all([SourceInput::new(
            "gamedata/unknown/socketables.ron",
            DecodedClass::GameDataTable,
            None,
        )]);
        assert_eq!(inv.blocked.len(), 1);
        assert_eq!(inv.exceptions_hit.len(), 1);
        assert!(inv.has_blocking_errors());
        assert_eq!(
            inv.blocked[0].canonical_path,
            "gameplay/gamedata/building/socketables.ron"
        );
    }

    #[test]
    fn unmapped_input_is_reported_as_route_error() {
        let inv = Mapper::pinned().unwrap().map_all([SourceInput::new(
            "misc/orphan.ron",
            DecodedClass::Material,
            None,
        )]);
        assert_eq!(inv.route_errors.len(), 1);
        assert!(inv.has_blocking_errors());
    }
}
