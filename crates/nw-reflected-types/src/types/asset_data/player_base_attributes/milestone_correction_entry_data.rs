use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::PlayerTeleportContext;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct MilestoneCorrectionEntryData {
    #[serde(rename = "MilestoneAchievement", default)]
    pub milestone_achievement: String,
    #[serde(rename = "MilestonePlayerLevel", default)]
    pub milestone_player_level: i32,
    #[serde(rename = "InvalidTerritories", default)]
    pub invalid_territories: Vec<i32>,
    #[serde(rename = "Relocation Territory Ids", default)]
    pub relocation_territory_ids: Vec<i32>,
    #[serde(rename = "AlwaysCheckMilestone", default)]
    pub always_check_milestone: bool,
    #[serde(rename = "MilestoneVersionAdded", default)]
    pub milestone_version_added: i32,
    #[serde(rename = "TeleportContext", default)]
    pub teleport_context: PlayerTeleportContext,
}

impl AzRtti for MilestoneCorrectionEntryData {
    const NAME: &'static str = "MilestoneCorrectionEntryData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7511B879_B09F_4F6B_8C0A_742DB3C0E7BD);
}
