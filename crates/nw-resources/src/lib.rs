pub const SERIALIZE_JSON: &[u8] = include_bytes!("../../../resources/serialize.json");
pub const STEAM_API64_DLL: &[u8] = include_bytes!("../../../resources/steam_api64.dll");

#[derive(Debug, Clone, Copy)]
pub struct EmbeddedResource {
    pub path: &'static str,
    pub bytes: &'static [u8],
}

pub const MODULE_DESCRIPTORS: &[EmbeddedResource] = &[
    EmbeddedResource {
        path: "modules/amazon-games-sdk-module.json",
        bytes: include_bytes!("../../../resources/modules/amazon-games-sdk-module.json"),
    },
    EmbeddedResource {
        path: "modules/bink-module.json",
        bytes: include_bytes!("../../../resources/modules/bink-module.json"),
    },
    EmbeddedResource {
        path: "modules/camera-module.json",
        bytes: include_bytes!("../../../resources/modules/camera-module.json"),
    },
    EmbeddedResource {
        path: "modules/cry-hooks-module.json",
        bytes: include_bytes!("../../../resources/modules/cry-hooks-module.json"),
    },
    EmbeddedResource {
        path: "modules/cry-legacy-animation-module.json",
        bytes: include_bytes!("../../../resources/modules/cry-legacy-animation-module.json"),
    },
    EmbeddedResource {
        path: "modules/cry-legacy-module.json",
        bytes: include_bytes!("../../../resources/modules/cry-legacy-module.json"),
    },
    EmbeddedResource {
        path: "modules/footsteps-module.json",
        bytes: include_bytes!("../../../resources/modules/footsteps-module.json"),
    },
    EmbeddedResource {
        path: "modules/frame-profiler-event-handler-module.json",
        bytes: include_bytes!(
            "../../../resources/modules/frame-profiler-event-handler-module.json"
        ),
    },
    EmbeddedResource {
        path: "modules/graphics-reflect-context-module.json",
        bytes: include_bytes!("../../../resources/modules/graphics-reflect-context-module.json"),
    },
    EmbeddedResource {
        path: "modules/historical-input-module.json",
        bytes: include_bytes!("../../../resources/modules/historical-input-module.json"),
    },
    EmbeddedResource {
        path: "modules/hitch-tracker-module.json",
        bytes: include_bytes!("../../../resources/modules/hitch-tracker-module.json"),
    },
    EmbeddedResource {
        path: "modules/im-gui-module.json",
        bytes: include_bytes!("../../../resources/modules/im-gui-module.json"),
    },
    EmbeddedResource {
        path: "modules/input-management-framework-module.json",
        bytes: include_bytes!("../../../resources/modules/input-management-framework-module.json"),
    },
    EmbeddedResource {
        path: "modules/javelin-collision-filters-module.json",
        bytes: include_bytes!("../../../resources/modules/javelin-collision-filters-module.json"),
    },
    EmbeddedResource {
        path: "modules/javelin-components-ai-module.json",
        bytes: include_bytes!("../../../resources/modules/javelin-components-ai-module.json"),
    },
    EmbeddedResource {
        path: "modules/javelin-components-character-module.json",
        bytes: include_bytes!(
            "../../../resources/modules/javelin-components-character-module.json"
        ),
    },
    EmbeddedResource {
        path: "modules/krag-module.json",
        bytes: include_bytes!("../../../resources/modules/krag-module.json"),
    },
    EmbeddedResource {
        path: "modules/lmbr-central-module.json",
        bytes: include_bytes!("../../../resources/modules/lmbr-central-module.json"),
    },
    EmbeddedResource {
        path: "modules/ly-shine-module.json",
        bytes: include_bytes!("../../../resources/modules/ly-shine-module.json"),
    },
    EmbeddedResource {
        path: "modules/maestro-module.json",
        bytes: include_bytes!("../../../resources/modules/maestro-module.json"),
    },
    EmbeddedResource {
        path: "modules/module.json",
        bytes: include_bytes!("../../../resources/modules/module.json"),
    },
    EmbeddedResource {
        path: "modules/music-sheet-module.json",
        bytes: include_bytes!("../../../resources/modules/music-sheet-module.json"),
    },
    EmbeddedResource {
        path: "modules/new-world-data-sheet-module.json",
        bytes: include_bytes!("../../../resources/modules/new-world-data-sheet-module.json"),
    },
    EmbeddedResource {
        path: "modules/platform-services-module.json",
        bytes: include_bytes!("../../../resources/modules/platform-services-module.json"),
    },
    EmbeddedResource {
        path: "modules/profanity-filter-module.json",
        bytes: include_bytes!("../../../resources/modules/profanity-filter-module.json"),
    },
    EmbeddedResource {
        path: "modules/rad-telemetry-module.json",
        bytes: include_bytes!("../../../resources/modules/rad-telemetry-module.json"),
    },
    EmbeddedResource {
        path: "modules/rain-gem.json",
        bytes: include_bytes!("../../../resources/modules/rain-gem.json"),
    },
    EmbeddedResource {
        path: "modules/roads-and-rivers-module.json",
        bytes: include_bytes!("../../../resources/modules/roads-and-rivers-module.json"),
    },
    EmbeddedResource {
        path: "modules/rock-n-roll-module.json",
        bytes: include_bytes!("../../../resources/modules/rock-n-roll-module.json"),
    },
    EmbeddedResource {
        path: "modules/scripted-entity-tweener-module.json",
        bytes: include_bytes!("../../../resources/modules/scripted-entity-tweener-module.json"),
    },
    EmbeddedResource {
        path: "modules/slayer-script-gem.json",
        bytes: include_bytes!("../../../resources/modules/slayer-script-gem.json"),
    },
    EmbeddedResource {
        path: "modules/snow-gem.json",
        bytes: include_bytes!("../../../resources/modules/snow-gem.json"),
    },
    EmbeddedResource {
        path: "modules/spectator-mode-module.json",
        bytes: include_bytes!("../../../resources/modules/spectator-mode-module.json"),
    },
    EmbeddedResource {
        path: "modules/texture-atlas-module.json",
        bytes: include_bytes!("../../../resources/modules/texture-atlas-module.json"),
    },
    EmbeddedResource {
        path: "modules/water-module.json",
        bytes: include_bytes!("../../../resources/modules/water-module.json"),
    },
    EmbeddedResource {
        path: "modules/watermark-module.json",
        bytes: include_bytes!("../../../resources/modules/watermark-module.json"),
    },
];

pub fn module_descriptors() -> impl Iterator<Item = EmbeddedResource> {
    MODULE_DESCRIPTORS.iter().copied()
}

pub fn all() -> impl Iterator<Item = EmbeddedResource> {
    [
        EmbeddedResource {
            path: "serialize.json",
            bytes: SERIALIZE_JSON,
        },
        EmbeddedResource {
            path: "steam_api64.dll",
            bytes: STEAM_API64_DLL,
        },
    ]
    .into_iter()
    .chain(module_descriptors())
}
