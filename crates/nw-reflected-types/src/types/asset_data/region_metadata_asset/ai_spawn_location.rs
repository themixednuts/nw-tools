use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct AISpawnLocation {
    #[serde(rename = "TerritoryIds", default)]
    pub territory_ids: Vec<u16>,
    #[serde(rename = "VitalsId", default)]
    pub vitals_id: AzCrc32,
    #[serde(rename = "VitalsCategoryId", default)]
    pub vitals_category_id: AzCrc32,
    #[serde(rename = "VitalsLevel", default)]
    pub vitals_level: u32,
    #[serde(rename = "WorldPosition", default)]
    pub world_position: bevy_math::Vec3,
    #[serde(rename = "SpawnedByCoatlicue", default)]
    pub spawned_by_coatlicue: bool,
    #[serde(rename = "IsAlias", default)]
    pub is_alias: bool,
    #[serde(rename = "IsOverride", default)]
    pub is_override: bool,
    #[serde(rename = "IsEncounter", default)]
    pub is_encounter: bool,
}

impl AzRtti for AISpawnLocation {
    const NAME: &'static str = "AISpawnLocation";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBB8141BA_FDF3_4EC3_AD76_34063DDC2BC1);
}
