use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct FishingData {
    #[serde(rename = "Fish Catch Game Event", default)]
    pub fish_catch_game_event: String,
    #[serde(rename = "Fish Hook Game Event", default)]
    pub fish_hook_game_event: String,
    #[serde(rename = "Fish Catch Durability Loss", default)]
    pub fish_catch_durability_loss: u32,
    #[serde(rename = "Fish Loss Durability Loss", default)]
    pub fish_loss_durability_loss: u32,
    #[serde(rename = "Line Break Durability Loss", default)]
    pub line_break_durability_loss: u32,
    #[serde(rename = "Bait Loss Chance Cast Hit Land", default)]
    pub bait_loss_chance_cast_hit_land: f32,
    #[serde(rename = "Bait Loss Chance Hook Miss", default)]
    pub bait_loss_chance_hook_miss: f32,
    #[serde(rename = "Bait Loss Chance Reeling Miss Distance", default)]
    pub bait_loss_chance_reeling_miss_distance: f32,
    #[serde(rename = "Bait Loss Chance Reeling Miss Tension", default)]
    pub bait_loss_chance_reeling_miss_tension: f32,
    #[serde(rename = "Bait Loss Chance Fish Caught", default)]
    pub bait_loss_chance_fish_caught: f32,
    #[serde(rename = "Min Reel Path Distance", default)]
    pub min_reel_path_distance: f32,
    #[serde(rename = "Bite Window Open Duration Seconds", default)]
    pub bite_window_open_duration_seconds: f32,
    #[serde(rename = "Fish Behavior Time Block Duration Seconds", default)]
    pub fish_behavior_time_block_duration_seconds: f32,
    #[serde(rename = "Fish Fighting Line Tension Multiplier", default)]
    pub fish_fighting_line_tension_multiplier: f32,
    #[serde(rename = "Fish Fighting Reel In Multiplier", default)]
    pub fish_fighting_reel_in_multiplier: f32,
    #[serde(rename = "Fish Fighting Swim Away Multiplier", default)]
    pub fish_fighting_swim_away_multiplier: f32,
    #[serde(rename = "Fish Tired Line Tension Multiplier", default)]
    pub fish_tired_line_tension_multiplier: f32,
    #[serde(rename = "Fish Tired Reel In Multiplier", default)]
    pub fish_tired_reel_in_multiplier: f32,
    #[serde(rename = "Fish Tired Swim Away Multiplier", default)]
    pub fish_tired_swim_away_multiplier: f32,
}

impl AzRtti for FishingData {
    const NAME: &'static str = "FishingData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x83E5FD61_2687_45B6_A7A6_731CE943D74C);
}
