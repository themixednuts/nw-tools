//! Deterministic legacy-to-native route registry (ADR 0027).
//!
//! [`map_native_path`] takes an already-normalized legacy path plus its decoded
//! class and returns the canonical native destination. The registry is closed:
//! an input whose top-level tree is not one of the 17 legacy trees, or whose
//! decoded-class/path combination is outside the table, is a hard error that
//! must be resolved by a reviewed mapping/ADR amendment, not by an incremental
//! `misc` route.
//!
//! Rules are ordered by decoded type before path, so explicit material rows
//! override image/prefix rows for the same tree (e.g. `objects`, `slices`,
//! `textures`).

use thiserror::Error;

/// Decoded content class of a legacy source, used to disambiguate trees that
/// hold more than one decoded type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodedClass {
    /// Motion clip.
    Motion,
    /// Mannequin ADB controller/database.
    MannequinDatabase,
    /// Static/skinned mesh model.
    Model,
    /// Material document.
    Material,
    /// World/generated Prefab.
    Prefab,
    /// LyShine UI Prefab.
    UiPrefab,
    /// Image source.
    Image,
    /// Texture settings sidecar.
    TextureSettings,
    /// UI texture atlas document.
    Atlas,
    /// GameData table document.
    GameDataTable,
    /// ATL audio controls.
    AudioControls,
    /// Audio event mappings.
    AudioEvents,
    /// Audio tag data.
    AudioTags,
    /// Audio tract/bank mappings.
    AudioTracts,
    /// Wwise bank payload.
    AudioBank,
    /// Wwise media (`.wem`) payload.
    AudioMedia,
    /// CAGE action list document.
    CageActionList,
    /// CAGE controller/grid document.
    CageGrid,
    /// Terrain world/region layer membership.
    TerrainLayers,
    /// Terrain payload (`.heightmap`/`.surfacemap`/`.waterqt`).
    TerrainPayload,
    /// Terrain region source.
    TerrainRegion,
    /// Terrain world source.
    TerrainWorld,
    /// Level Prefab.
    LevelPrefab,
    /// Region composition Prefab.
    RegionPrefab,
}

impl DecodedClass {
    /// The native `AssetData::STABLE_NAME` token emitted for this class.
    ///
    /// `mesh`, `material`, and `motion` match the existing Azoth native stable
    /// names; the remainder are Phase-1 provisional native tokens pending the
    /// corresponding native asset types.
    #[must_use]
    pub const fn native_asset_type(self) -> &'static str {
        match self {
            Self::Motion => "azoth.animation.motion",
            Self::MannequinDatabase => "azoth.animation.mannequin-database",
            Self::Model => "azoth.rendering.mesh",
            Self::Material => "azoth.material.material",
            Self::Prefab => "azoth.prefab.prefab",
            Self::UiPrefab => "azoth.ui.prefab",
            Self::Image => "azoth.rendering.texture",
            Self::TextureSettings => "azoth.rendering.texture-settings",
            Self::Atlas => "azoth.ui.texture-atlas",
            Self::GameDataTable => "azoth.gameplay.gamedata-table",
            Self::AudioControls => "azoth.audio.controls",
            Self::AudioEvents => "azoth.audio.events",
            Self::AudioTags => "azoth.audio.tags",
            Self::AudioTracts => "azoth.audio.tracts",
            Self::AudioBank => "azoth.audio.bank",
            Self::AudioMedia => "azoth.audio.media",
            Self::CageActionList => "azoth.animation.cage-action-list",
            Self::CageGrid => "azoth.animation.cage-controller",
            Self::TerrainLayers => "azoth.world.terrain-layers",
            Self::TerrainPayload => "azoth.world.terrain-payload",
            Self::TerrainRegion => "azoth.world.terrain-region",
            Self::TerrainWorld => "azoth.world.terrain-world",
            Self::LevelPrefab => "azoth.world.level",
            Self::RegionPrefab => "azoth.world.region",
        }
    }
}

/// A normalized legacy path could not be routed by the closed registry.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RouteError {
    /// The `(top-level tree, decoded class, path)` combination is outside the
    /// closed ADR-0027 registry.
    #[error(
        "legacy path `{path}` (top-level `{top_level}`, class {class:?}) is outside the closed ADR-0027 route registry; a reviewed mapping/ADR amendment is required"
    )]
    Unmapped {
        /// The offending normalized legacy path.
        path: String,
        /// Its top-level legacy tree segment.
        top_level: String,
        /// Its decoded class.
        class: DecodedClass,
    },

    /// The path had no remaining component after the routed prefix was stripped.
    #[error("legacy path `{path}` has no component after the `{strip}` prefix")]
    EmptyAfterStrip {
        /// The offending normalized legacy path.
        path: String,
        /// The prefix that was stripped.
        strip: &'static str,
    },
}

/// The 17 legacy top-level trees from the 2026-07-16 inventory.
pub const LEGACY_TREES: [&str; 17] = [
    "animations",
    "characters",
    "coatgen",
    "engineassets",
    "gamedata",
    "libs",
    "materials",
    "meshes",
    "objects",
    "prefabs",
    "sharedassets",
    "slices",
    "sounds",
    "terrain",
    "textures",
    "ui",
    "world",
];

/// Map a normalized legacy path and decoded class to its canonical native path.
///
/// The input must already be normalized by [`crate::normalize_legacy_path`].
///
/// # Errors
///
/// Returns [`RouteError::Unmapped`] when the combination is outside the closed
/// ADR-0027 registry, or [`RouteError::EmptyAfterStrip`] when nothing remains
/// after the routed prefix.
pub fn map_native_path(normalized: &str, class: DecodedClass) -> Result<String, RouteError> {
    let top_level = normalized.split('/').next().unwrap_or(normalized);
    let unmapped = || RouteError::Unmapped {
        path: normalized.to_owned(),
        top_level: top_level.to_owned(),
        class,
    };

    match top_level {
        "animations" => route_animations(normalized, class).ok_or_else(unmapped),
        "characters" => route_characters(normalized, class).ok_or_else(unmapped),
        "coatgen" => route_coatgen(normalized, class).ok_or_else(unmapped),
        "engineassets" => route_engineassets(normalized, class).ok_or_else(unmapped),
        "gamedata" => route_gamedata(normalized, class).ok_or_else(unmapped),
        "libs" => route_libs(normalized, class).ok_or_else(unmapped),
        "materials" => route_materials(normalized, class).ok_or_else(unmapped),
        "meshes" => route_meshes(normalized, class).ok_or_else(unmapped),
        "objects" => route_objects(normalized, class).ok_or_else(unmapped),
        "prefabs" => route_prefabs(normalized, class).ok_or_else(unmapped),
        "sharedassets" => route_sharedassets(normalized, class).ok_or_else(unmapped),
        "slices" => route_slices(normalized, class).ok_or_else(unmapped),
        "sounds" => route_sounds(normalized, class).ok_or_else(unmapped),
        "terrain" => route_terrain(normalized, class).ok_or_else(unmapped),
        "textures" => route_textures(normalized, class).ok_or_else(unmapped),
        "ui" => route_ui(normalized, class).ok_or_else(unmapped),
        "world" => route_world(normalized, class).ok_or_else(unmapped),
        _ => Err(unmapped()),
    }
    .and_then(|mapped| match mapped {
        Mapped::Path(path) => Ok(path),
        Mapped::EmptyAfterStrip(strip) => Err(RouteError::EmptyAfterStrip {
            path: normalized.to_owned(),
            strip,
        }),
    })
}

/// Internal route outcome distinguishing "unmapped" from "empty after strip".
enum Mapped {
    Path(String),
    EmptyAfterStrip(&'static str),
}

/// Strip `strip` from `path` and prepend `prefix`.
fn remap(path: &str, strip: &'static str, prefix: &str) -> Option<Mapped> {
    let rest = path.strip_prefix(strip)?;
    if rest.is_empty() {
        return Some(Mapped::EmptyAfterStrip(strip));
    }
    Some(Mapped::Path(format!("{prefix}{rest}")))
}

/// Identity route for trees whose native destination equals the legacy path.
fn identity(path: &str) -> Option<Mapped> {
    Some(Mapped::Path(path.to_owned()))
}

fn route_animations(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::MannequinDatabase => remap(
            path,
            "animations/mannequin/adb/",
            "animation/controllers/mannequin/databases/",
        ),
        DecodedClass::Motion => {
            if path.starts_with("animations/cinematic/") {
                remap(
                    path,
                    "animations/cinematic/",
                    "animation/cinematics/motions/",
                )
            } else if path.starts_with("animations/gameplay/") {
                remap(path, "animations/gameplay/", "animation/motions/gameplay/")
            } else if path.starts_with("animations/igc/") {
                remap(path, "animations/igc/", "animation/cinematics/igc/")
            } else if path.starts_with("animations/localization/animations/vo/") {
                remap(
                    path,
                    "animations/localization/animations/vo/",
                    "animation/motions/voice/",
                )
            } else if path.starts_with("animations/marketing/") {
                remap(
                    path,
                    "animations/marketing/",
                    "animation/motions/marketing/",
                )
            } else if path.starts_with("animations/vo/poses/") {
                remap(
                    path,
                    "animations/vo/poses/",
                    "animation/motions/voice/poses/",
                )
            } else {
                None
            }
        }
        _ => None,
    }
}

fn route_characters(path: &str, class: DecodedClass) -> Option<Mapped> {
    if class != DecodedClass::Model {
        return None;
    }
    if path.starts_with("characters/npc/") || path.starts_with("characters/player/") {
        identity(path)
    } else if path.starts_with("characters/objects/") {
        remap(path, "characters/objects/", "characters/animated-props/")
    } else {
        None
    }
}

fn route_coatgen(path: &str, class: DecodedClass) -> Option<Mapped> {
    // `coatgen/<hash>/**/*.material.ron` -> retain the hash bucket as provenance.
    match class {
        DecodedClass::Material => remap(
            path,
            "coatgen/",
            "rendering/materials/world-generated/coatgen/",
        ),
        _ => None,
    }
}

fn route_engineassets(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::Material => remap(path, "engineassets/", "rendering/materials/engine/"),
        _ => None,
    }
}

fn route_gamedata(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::GameDataTable => remap(path, "gamedata/", "gameplay/gamedata/"),
        _ => None,
    }
}

fn route_libs(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::Material => remap(path, "libs/particles/", "rendering/materials/particles/"),
        _ => None,
    }
}

fn route_materials(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::Material => remap(path, "materials/", "rendering/materials/"),
        _ => None,
    }
}

fn route_meshes(path: &str, class: DecodedClass) -> Option<Mapped> {
    if class != DecodedClass::Model {
        return None;
    }
    if path.starts_with("meshes/animations/") {
        remap(
            path,
            "meshes/animations/",
            "rendering/meshes/animated-props/",
        )
    } else if path.starts_with("meshes/coatgen/") {
        remap(
            path,
            "meshes/coatgen/",
            "rendering/meshes/world-generated/coatgen/",
        )
    } else if path.starts_with("meshes/engineassets/") {
        remap(path, "meshes/engineassets/", "rendering/meshes/engine/")
    } else if path.starts_with("meshes/levels/") {
        remap(path, "meshes/levels/", "rendering/meshes/levels/")
    } else if path.starts_with("meshes/objects/") {
        remap(path, "meshes/objects/", "rendering/meshes/objects/")
    } else if path.starts_with("meshes/sharedassets/") {
        remap(
            path,
            "meshes/sharedassets/",
            "rendering/meshes/world/sharedassets/",
        )
    } else if path.starts_with("meshes/slices/") {
        remap(path, "meshes/slices/", "rendering/meshes/world/slices/")
    } else {
        None
    }
}

fn route_objects(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::Material => remap(path, "objects/", "rendering/materials/objects/"),
        _ => None,
    }
}

fn route_prefabs(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::UiPrefab => remap(path, "prefabs/lyshineui/", "ui/prefabs/"),
        DecodedClass::Prefab => {
            if path.starts_with("prefabs/coatgen/") {
                remap(path, "prefabs/coatgen/", "world/prefabs/generated/coatgen/")
            } else if path.starts_with("prefabs/objects/") {
                remap(path, "prefabs/objects/", "world/prefabs/objects/")
            } else if path.starts_with("prefabs/slices/") {
                remap(path, "prefabs/slices/", "world/prefabs/slices/")
            } else if path.starts_with("prefabs/lyshineui/") {
                // Route UI Prefabs even when classified generically.
                remap(path, "prefabs/lyshineui/", "ui/prefabs/")
            } else {
                None
            }
        }
        _ => None,
    }
}

fn route_sharedassets(path: &str, class: DecodedClass) -> Option<Mapped> {
    const SB: &str = "sharedassets/springboardentitites/";
    match class {
        // Coatlicue region materials.
        DecodedClass::Material if path.starts_with("sharedassets/coatlicue/") => {
            remap(path, "sharedassets/", "rendering/materials/world/")
        }
        DecodedClass::CageActionList => {
            if path.starts_with(&format!("{SB}characters/")) {
                remap(
                    path,
                    "sharedassets/springboardentitites/characters/",
                    "animation/actions/cage/characters/",
                )
            } else if path.starts_with(&format!("{SB}fxscripts/")) {
                remap(
                    path,
                    "sharedassets/springboardentitites/fxscripts/",
                    "animation/actions/cage/fx/",
                )
            } else if path.starts_with(&format!("{SB}playerdata/cage/")) {
                remap(
                    path,
                    "sharedassets/springboardentitites/playerdata/cage/",
                    "animation/actions/cage/player/",
                )
            } else {
                None
            }
        }
        DecodedClass::CageGrid => {
            if path.starts_with(&format!("{SB}characters/")) {
                remap(
                    path,
                    "sharedassets/springboardentitites/characters/",
                    "animation/controllers/cage/characters/",
                )
            } else if path.starts_with(&format!("{SB}playerdata/cage/")) {
                remap(
                    path,
                    "sharedassets/springboardentitites/playerdata/cage/",
                    "animation/controllers/cage/player/",
                )
            } else {
                None
            }
        }
        _ => None,
    }
}

fn route_slices(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::Material => remap(path, "slices/", "rendering/materials/slices/"),
        _ => None,
    }
}

fn route_sounds(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::AudioControls => remap(path, "sounds/wwise/", "audio/wwise/controls/"),
        DecodedClass::AudioEvents => remap(path, "sounds/wwise/", "audio/wwise/events/"),
        DecodedClass::AudioTags => remap(path, "sounds/wwise/", "audio/wwise/tags/"),
        DecodedClass::AudioTracts => remap(path, "sounds/wwise/", "audio/wwise/tracts/"),
        DecodedClass::AudioBank | DecodedClass::AudioMedia => route_sound_payload(path, class),
        _ => None,
    }
}

/// Route the enumerated Wwise bank/media locale subtrees.
///
/// Bank vs media is chosen by decoded class; locale/profile come from the
/// enumerated legacy subtree. The precise BCP-47/profile derivation is a
/// deterministic Phase-1 mapping over the six checked-in subtrees and is
/// subject to a Phase-2 review of the full Wwise media layout.
fn route_sound_payload(path: &str, class: DecodedClass) -> Option<Mapped> {
    let kind = match class {
        DecodedClass::AudioBank => "banks",
        DecodedClass::AudioMedia => "media",
        _ => return None,
    };
    // (legacy subtree, bcp47 locale, profile)
    const SUBTREES: [(&str, &str, &str); 6] = [
        ("sounds/wwise/media/english(us)/", "en-us", "default"),
        ("sounds/wwise/english(us)/", "en-us", "default"),
        ("sounds/wwise/minspec/english(us)/", "en-us", "minspec"),
        ("sounds/wwise/minspec/italian/", "it", "minspec"),
        ("sounds/wwise/minspec/polish/", "pl", "minspec"),
        ("sounds/wwise/minspec/spanish(mexico)/", "es-mx", "minspec"),
    ];
    for (strip, locale, profile) in SUBTREES {
        if path.starts_with(strip) {
            let prefix = format!("audio/wwise/{kind}/{locale}/{profile}/");
            return remap(path, strip, &prefix);
        }
    }
    None
}

fn route_terrain(path: &str, class: DecodedClass) -> Option<Mapped> {
    // Terrain source-set folding is a Phase-1 best-effort deterministic mapping;
    // full `.azterrain.ron` source-set assembly is Phase 2.
    match class {
        DecodedClass::TerrainWorld => {
            let rest = path.strip_prefix("terrain/worlds/")?;
            let world = rest.strip_suffix(".azterrain.ron").unwrap_or(rest);
            if world.is_empty() {
                return Some(Mapped::EmptyAfterStrip("terrain/worlds/"));
            }
            Some(Mapped::Path(format!(
                "world/terrain/{world}/world.azterrain.ron"
            )))
        }
        DecodedClass::TerrainRegion => {
            terrain_named(path, "terrain/regions/", "region.azterrain.ron")
        }
        DecodedClass::TerrainLayers => {
            terrain_named(path, "terrain/layers/", "layers.azterrain.ron")
        }
        DecodedClass::TerrainPayload => {
            let rest = path.strip_prefix("terrain/payloads/")?;
            let (world, tail) = rest.split_once('/')?;
            if world.is_empty() || tail.is_empty() {
                return Some(Mapped::EmptyAfterStrip("terrain/payloads/"));
            }
            Some(Mapped::Path(format!(
                "world/terrain/{world}/payloads/{tail}"
            )))
        }
        _ => None,
    }
}

fn terrain_named(path: &str, strip: &'static str, file: &str) -> Option<Mapped> {
    let rest = path.strip_prefix(strip)?;
    let (world, tail) = rest.split_once('/')?;
    if world.is_empty() {
        return Some(Mapped::EmptyAfterStrip(strip));
    }
    let dir = tail.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mid = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    Some(Mapped::Path(format!("world/terrain/{world}/{mid}{file}")))
}

fn route_textures(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        // Decoded-material override for `textures/{crafting,decals,terrain}/**`.
        DecodedClass::Material => remap(path, "textures/", "rendering/materials/"),
        DecodedClass::Image | DecodedClass::TextureSettings => {
            if path.starts_with("textures/lyshineui/") {
                remap(path, "textures/lyshineui/", "ui/images/")
            } else if path.starts_with("textures/sprites/") {
                remap(path, "textures/sprites/", "ui/sprites/")
            } else if path.starts_with("textures/tools/") {
                remap(path, "textures/tools/", "ui/images/tools/")
            } else {
                remap(path, "textures/", "rendering/textures/")
            }
        }
        _ => None,
    }
}

fn route_ui(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::Atlas => remap(path, "ui/lyshineui/images/textureatlas/", "ui/atlases/"),
        _ => None,
    }
}

fn route_world(path: &str, class: DecodedClass) -> Option<Mapped> {
    match class {
        DecodedClass::LevelPrefab => {
            if path.starts_with("world/levels/frontend/")
                || path.starts_with("world/levels/ftue_v2/")
            {
                identity(path)
            } else {
                None
            }
        }
        DecodedClass::RegionPrefab => {
            // world/sharedassets/coatlicue/<world>/regions/<region>/region.prefab.ron
            let rest = path.strip_prefix("world/sharedassets/coatlicue/")?;
            let native = rest.replacen("/regions/", "/", 1);
            if native == rest {
                return None;
            }
            Some(Mapped::Path(format!("world/regions/{native}")))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::DecodedClass::*;
    use super::*;

    fn map(path: &str, class: DecodedClass) -> String {
        map_native_path(path, class).unwrap_or_else(|e| panic!("route `{path}`: {e}"))
    }

    #[test]
    fn every_legacy_top_level_tree_has_a_representative_route() {
        // One representative per legacy tree per ADR 0027's table.
        assert_eq!(
            map("animations/cinematic/intro/shot01.anim.glb", Motion),
            "animation/cinematics/motions/intro/shot01.anim.glb"
        );
        assert_eq!(
            map("characters/npc/natural/wolf/wolf.glb", Model),
            "characters/npc/natural/wolf/wolf.glb"
        );
        assert_eq!(
            map("coatgen/ab/cd/rock.material.ron", Material),
            "rendering/materials/world-generated/coatgen/ab/cd/rock.material.ron"
        );
        assert_eq!(
            map("engineassets/greywizard/grey.material.ron", Material),
            "rendering/materials/engine/greywizard/grey.material.ron"
        );
        assert_eq!(
            map("gamedata/items/weaponitemdefinitions.ron", GameDataTable),
            "gameplay/gamedata/items/weaponitemdefinitions.ron"
        );
        assert_eq!(
            map("libs/particles/fire.material.ron", Material),
            "rendering/materials/particles/fire.material.ron"
        );
        assert_eq!(
            map("materials/decals/blood.material.ron", Material),
            "rendering/materials/decals/blood.material.ron"
        );
        assert_eq!(
            map("meshes/objects/barrel.glb", Model),
            "rendering/meshes/objects/barrel.glb"
        );
        assert_eq!(
            map("objects/alchemy/vial.material.ron", Material),
            "rendering/materials/objects/alchemy/vial.material.ron"
        );
        assert_eq!(
            map("prefabs/objects/camp/camp.prefab.ron", Prefab),
            "world/prefabs/objects/camp/camp.prefab.ron"
        );
        assert_eq!(
            map(
                "sharedassets/coatlicue/ftue_v2/regions/r1.material.ron",
                Material
            ),
            "rendering/materials/world/coatlicue/ftue_v2/regions/r1.material.ron"
        );
        assert_eq!(
            map("slices/nature/tree.material.ron", Material),
            "rendering/materials/slices/nature/tree.material.ron"
        );
        assert_eq!(
            map("sounds/wwise/master.audiocontrols.ron", AudioControls),
            "audio/wwise/controls/master.audiocontrols.ron"
        );
        assert_eq!(
            map("terrain/worlds/newworld.azterrain.ron", TerrainWorld),
            "world/terrain/newworld/world.azterrain.ron"
        );
        assert_eq!(
            map("textures/items/weapons/sword_diff.png", Image),
            "rendering/textures/items/weapons/sword_diff.png"
        );
        assert_eq!(
            map("ui/lyshineui/images/textureatlas/hud.atlas.ron", Atlas),
            "ui/atlases/hud.atlas.ron"
        );
        assert_eq!(
            map("world/levels/frontend/frontend.prefab.ron", LevelPrefab),
            "world/levels/frontend/frontend.prefab.ron"
        );
    }

    #[test]
    fn decoded_type_overrides_misleading_prefixes() {
        // `objects`, `slices`, and `textures` prefixes yield materials.
        assert_eq!(
            map("textures/terrain/rock.material.ron", Material),
            "rendering/materials/terrain/rock.material.ron"
        );
        assert_eq!(
            map("objects/props/crate.material.ron", Material),
            "rendering/materials/objects/props/crate.material.ron"
        );
        // But an image under textures stays an image.
        assert_eq!(
            map("textures/terrain/rock_diff.png", Image),
            "rendering/textures/terrain/rock_diff.png"
        );
    }

    #[test]
    fn animation_and_cage_and_ui_and_audio_variants() {
        assert_eq!(
            map("animations/mannequin/adb/player.adb.ron", MannequinDatabase),
            "animation/controllers/mannequin/databases/player.adb.ron"
        );
        assert_eq!(
            map(
                "animations/localization/animations/vo/en-us/line.anim.glb",
                Motion
            ),
            "animation/motions/voice/en-us/line.anim.glb"
        );
        assert_eq!(
            map(
                "sharedassets/springboardentitites/characters/wolf/attack.actionlist.ron",
                CageActionList
            ),
            "animation/actions/cage/characters/wolf/attack.actionlist.ron"
        );
        assert_eq!(
            map(
                "sharedassets/springboardentitites/playerdata/cage/loco.grid.ron",
                CageGrid
            ),
            "animation/controllers/cage/player/loco.grid.ron"
        );
        assert_eq!(
            map("prefabs/lyshineui/hud/hud.prefab.ron", UiPrefab),
            "ui/prefabs/hud/hud.prefab.ron"
        );
        assert_eq!(
            map("sounds/wwise/minspec/italian/vo.wem", AudioMedia),
            "audio/wwise/media/it/minspec/vo.wem"
        );
        assert_eq!(
            map("sounds/wwise/english(us)/vo.bnk", AudioBank),
            "audio/wwise/banks/en-us/default/vo.bnk"
        );
        assert_eq!(
            map(
                "world/sharedassets/coatlicue/ftue_v2/regions/r1/region.prefab.ron",
                RegionPrefab
            ),
            "world/regions/ftue_v2/r1/region.prefab.ron"
        );
    }

    #[test]
    fn unknown_top_level_tree_is_a_hard_error() {
        let err = map_native_path("misc/whatever.ron", Material).unwrap_err();
        assert!(matches!(err, RouteError::Unmapped { .. }));
    }

    #[test]
    fn wrong_class_for_tree_is_a_hard_error() {
        // `gamedata` only accepts GameDataTable.
        let err = map_native_path("gamedata/items/x.ron", Image).unwrap_err();
        assert!(matches!(err, RouteError::Unmapped { .. }));
    }
}
