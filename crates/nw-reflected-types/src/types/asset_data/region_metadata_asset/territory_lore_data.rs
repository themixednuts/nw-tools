use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct TerritoryLoreData {
    #[serde(rename = "TerritoryIds", default)]
    pub territory_ids: Vec<u16>,
    #[serde(rename = "LoreId", default)]
    pub lore_id: AzCrc32,
    #[serde(rename = "WorldPosition", default)]
    pub world_position: bevy_math::Vec3,
}

impl AzRtti for TerritoryLoreData {
    const NAME: &'static str = "TerritoryLoreData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAE22A998_43F4_4466_8CF9_B12AA2F7A8B2);
}
