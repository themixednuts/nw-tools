use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::FactionType;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct NPCData {
    #[serde(rename = "TerritoryIds", default)]
    pub territory_ids: Vec<u16>,
    #[serde(rename = "NpcId", default)]
    pub npc_id: AzCrc32,
    #[serde(rename = "WorldPosition", default)]
    pub world_position: bevy_math::Vec3,
    #[serde(rename = "FactionType", default)]
    pub faction_type: FactionType,
    #[serde(rename = "SwapAchievementId", default)]
    pub swap_achievement_id: String,
    #[serde(rename = "ShowOnAchievementLocked", default)]
    pub show_on_achievement_locked: bool,
    #[serde(rename = "DisablePvpMissions", default)]
    pub disable_pvp_missions: bool,
    #[serde(rename = "VariantId", default)]
    pub variant_id: AzCrc32,
}

impl AzRtti for NPCData {
    const NAME: &'static str = "NPCData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA0E23BA5_775A_4C07_B70C_ED9078F62D23);
}
