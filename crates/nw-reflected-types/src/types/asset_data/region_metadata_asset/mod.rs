use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod ai_spawn_location;
pub mod encounter_entry;
pub mod gatherable_region_entry;
pub mod igc_data;
pub mod instanced_slayer_script_part;
pub mod npc_data;
pub mod territory_landmark_data;
pub mod territory_lore_data;

pub use self::ai_spawn_location::AISpawnLocation;
pub use self::encounter_entry::EncounterEntry;
pub use self::gatherable_region_entry::GatherableRegionEntry;
pub use self::igc_data::IGCData;
pub use self::instanced_slayer_script_part::InstancedSlayerScriptPart;
pub use self::npc_data::NPCData;
pub use self::territory_landmark_data::TerritoryLandmarkData;
pub use self::territory_lore_data::TerritoryLoreData;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct RegionMetadataAsset {
    #[serde(rename = "NpcData", default)]
    pub npc_data: Vec<NPCData>,
    #[serde(rename = "AiSpawnLocations", default)]
    pub ai_spawn_locations: Vec<AISpawnLocation>,
    #[serde(rename = "GatherableLocations", default)]
    pub gatherable_locations: Vec<GatherableRegionEntry>,
    #[serde(rename = "TerritoryLandmarks", default)]
    pub territory_landmarks: Vec<TerritoryLandmarkData>,
    #[serde(rename = "EncounterLocations", default)]
    pub encounter_locations: Vec<EncounterEntry>,
    #[serde(rename = "LoreData", default)]
    pub lore_data: Vec<TerritoryLoreData>,
    #[serde(rename = "IGCData", default)]
    pub igc_data: Vec<IGCData>,
    #[serde(rename = "InstancedScriptParts", default)]
    pub instanced_script_parts: Vec<InstancedSlayerScriptPart>,
}

impl AzRtti for RegionMetadataAsset {
    const NAME: &'static str = "RegionMetadataAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBDA6ADF9_991E_489D_B0F5_796CC24D7AFB);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
