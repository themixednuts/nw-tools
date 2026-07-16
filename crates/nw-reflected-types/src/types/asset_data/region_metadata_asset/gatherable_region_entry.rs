use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::InstancedLootType;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct GatherableRegionEntry {
    #[serde(rename = "TerritoryIds", default)]
    pub territory_ids: Vec<u16>,
    #[serde(rename = "GatherableId", default)]
    pub gatherable_id: AzCrc32,
    #[serde(rename = "LootTableId", default)]
    pub loot_table_id: String,
    #[serde(rename = "WorldPosition", default)]
    pub world_position: bevy_math::Vec3,
    #[serde(rename = "SpawnedByCoatlicue", default)]
    pub spawned_by_coatlicue: bool,
    #[serde(rename = "HasVariant", default)]
    pub has_variant: bool,
    #[serde(rename = "IsVariantOverride", default)]
    pub is_variant_override: bool,
    #[serde(rename = "InstancedLootType", default)]
    pub instanced_loot_type: InstancedLootType,
    #[serde(rename = "IsEncounter", default)]
    pub is_encounter: bool,
}

impl AzRtti for GatherableRegionEntry {
    const NAME: &'static str = "GatherableRegionEntry";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9C1DC9AD_AD7D_41BA_8A9B_9533F76C68A7);
}
