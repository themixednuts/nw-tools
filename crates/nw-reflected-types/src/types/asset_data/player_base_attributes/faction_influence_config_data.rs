use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::EditCrc;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct FactionInfluenceConfigData {
    #[serde(rename = "MaxInfluence", default)]
    pub max_influence: f32,
    #[serde(rename = "DecrementRate", default)]
    pub decrement_rate: f32,
    #[serde(rename = "IncrementRate", default)]
    pub increment_rate: f32,
    #[serde(rename = "MaxIncrementTimeModifier", default)]
    pub max_increment_time_modifier: f32,
    #[serde(rename = "MaxDecrementTimeModifier", default)]
    pub max_decrement_time_modifier: f32,
    #[serde(rename = "MinimumTimeSinceLastWar", default)]
    pub minimum_time_since_last_war: f32,
    #[serde(rename = "MinTerritoryDiffToApplyUDMechanics", default)]
    pub min_territory_diff_to_apply_ud_mechanics: i32,
    #[serde(rename = "MinTimeToApplyUDMechanics", default)]
    pub min_time_to_apply_ud_mechanics: i32,
    #[serde(rename = "UnderDogMissionInfluenceGain", default)]
    pub under_dog_mission_influence_gain: f32,
    #[serde(rename = "UnderDogMissionInfluenceGainCap", default)]
    pub under_dog_mission_influence_gain_cap: f32,
    #[serde(rename = "UderDogFactionRepGain", default)]
    pub uder_dog_faction_rep_gain: f32,
    #[serde(rename = "UnderDogFactionRepGainCap", default)]
    pub under_dog_faction_rep_gain_cap: f32,
    #[serde(rename = "UnderDogPVPInfluenceGain", default)]
    pub under_dog_pvp_influence_gain: f32,
    #[serde(rename = "UnderDogPVPInfluenceGainCap", default)]
    pub under_dog_pvp_influence_gain_cap: f32,
    #[serde(rename = "MinimumInfluenceThresholdForWar", default)]
    pub minimum_influence_threshold_for_war: f32,
    #[serde(rename = "Influence Race Attacker Win GameEventId", default)]
    pub influence_race_attacker_win_game_event_id: EditCrc,
    #[serde(rename = "Influence Race Defender Win GameEventId", default)]
    pub influence_race_defender_win_game_event_id: EditCrc,
    #[serde(rename = "Influence Race Lose GameEventId", default)]
    pub influence_race_lose_game_event_id: EditCrc,
}

impl AzRtti for FactionInfluenceConfigData {
    const NAME: &'static str = "FactionInfluenceConfigData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x8ED959C4_B0E3_4D45_84D1_FCAEC1C7D1A4);
}
