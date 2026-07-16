use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{EditCrc, PvpValueEntry};
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct FactionData {
    #[serde(rename = "Sanctuary Enter Notification Id", default)]
    pub sanctuary_enter_notification_id: String,
    #[serde(rename = "Sanctuary Exit Notification Id", default)]
    pub sanctuary_exit_notification_id: String,
    #[serde(rename = "Toggle Pvp Notification Id", default)]
    pub toggle_pvp_notification_id: String,
    #[serde(rename = "Faction Change Cooldown Seconds", default)]
    pub faction_change_cooldown_seconds: u32,
    #[serde(rename = "Faction Change Cost Level", default)]
    pub faction_change_cost_level: u32,
    #[serde(rename = "Faction Change Cost Min", default)]
    pub faction_change_cost_min: u32,
    #[serde(rename = "Faction Change Cost Max", default)]
    pub faction_change_cost_max: u32,
    #[serde(rename = "Faction Change Cost Increment", default)]
    pub faction_change_cost_increment: u32,
    #[serde(rename = "Pvp Kill Value Per Second", default)]
    pub pvp_kill_value_per_second: f32,
    #[serde(rename = "Pvp Kill Value Thresholds", default)]
    pub pvp_kill_value_thresholds: Vec<PvpValueEntry>,
    #[serde(rename = "Pvp Kill GameEventId", default)]
    pub pvp_kill_game_event_id: EditCrc,
    #[serde(rename = "Fort Capture GameEventId", default)]
    pub fort_capture_game_event_id: EditCrc,
    #[serde(rename = "Influence Tower Capture GameEventId", default)]
    pub influence_tower_capture_game_event_id: EditCrc,
    #[serde(rename = "Pvp Kill Faction Token Modifier", default)]
    pub pvp_kill_faction_token_modifier: f32,
}

impl AzRtti for FactionData {
    const NAME: &'static str = "FactionData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x5C98F982_85C3_423C_8CD8_65C55FAADE0C);
}
