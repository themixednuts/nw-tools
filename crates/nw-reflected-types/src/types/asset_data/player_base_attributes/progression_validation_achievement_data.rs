use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::EditCrc;
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
pub struct ProgressionValidationAchievementData {
    #[serde(rename = "AzothStaffAchievements", default)]
    pub azoth_staff_achievements: Vec<String>,
    #[serde(rename = "AzothStaffItemIds", default)]
    pub azoth_staff_item_ids: Vec<EditCrc>,
    #[serde(rename = "HeartGemSlotAchievements", default)]
    pub heart_gem_slot_achievements: Vec<EditCrc>,
    #[serde(rename = "SyndicateRankupTrialAchievements", default)]
    pub syndicate_rankup_trial_achievements: Vec<String>,
    #[serde(rename = "MarauderRankupTrialAchievements", default)]
    pub marauder_rankup_trial_achievements: Vec<String>,
    #[serde(rename = "CovenantRankupTrialAchievements", default)]
    pub covenant_rankup_trial_achievements: Vec<String>,
    #[serde(rename = "SyndicateRankupObjectiveIds", default)]
    pub syndicate_rankup_objective_ids: Vec<EditCrc>,
    #[serde(rename = "MarauderRankupObjectiveIds", default)]
    pub marauder_rankup_objective_ids: Vec<EditCrc>,
    #[serde(rename = "CovenantRankupObjectiveIds", default)]
    pub covenant_rankup_objective_ids: Vec<EditCrc>,
    #[serde(rename = "SyndicateSliceSwapAchievementId", default)]
    pub syndicate_slice_swap_achievement_id: EditCrc,
    #[serde(rename = "MarauderSliceSwapAchievementId", default)]
    pub marauder_slice_swap_achievement_id: EditCrc,
    #[serde(rename = "CovenantSliceSwapAchievementId", default)]
    pub covenant_slice_swap_achievement_id: EditCrc,
    #[serde(rename = "CampingRankupAchievements", default)]
    pub camping_rankup_achievements: Vec<String>,
    #[serde(rename = "FtueCompletionAchievementId", default)]
    pub ftue_completion_achievement_id: EditCrc,
    #[serde(rename = "FtueObjectiveIds", default)]
    pub ftue_objective_ids: Vec<EditCrc>,
}

impl AzRtti for ProgressionValidationAchievementData {
    const NAME: &'static str = "ProgressionValidationAchievementData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA80AB26A_3364_42D8_B1E3_A6828561F04D);
}
