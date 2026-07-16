use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::WarDeployableLimitData;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct WarData {
    #[serde(rename = "War Cost", default)]
    pub war_cost: [u32; 3],
    #[serde(rename = "War Extension Cost Modifier", default)]
    pub war_extension_cost_modifier: f32,
    #[serde(rename = "War Extension Max Cost Percentage", default)]
    pub war_extension_max_cost_percentage: f32,
    #[serde(rename = "Prewar Duration Minutes", default)]
    pub prewar_duration_minutes: u32,
    #[serde(rename = "Conquest Window Duration Minutes", default)]
    pub conquest_window_duration_minutes: u32,
    #[serde(rename = "War Duration Minutes", default)]
    pub war_duration_minutes: u32,
    #[serde(rename = "War Conquest Duration Minutes", default)]
    pub war_conquest_duration_minutes: u32,
    #[serde(rename = "Invasion Conquest Duration Minutes", default)]
    pub invasion_conquest_duration_minutes: u32,
    #[serde(rename = "Resolution Duration Minutes", default)]
    pub resolution_duration_minutes: u32,
    #[serde(rename = "Resolution Phase Buffer Minutes", default)]
    pub resolution_phase_buffer_minutes: u32,
    #[serde(rename = "Claim Lock Duration Minutes", default)]
    pub claim_lock_duration_minutes: u32,
    #[serde(rename = "Siege Window Cooldown Minutes", default)]
    pub siege_window_cooldown_minutes: u32,
    #[serde(rename = "Attacker War Count Modifier", default)]
    pub attacker_war_count_modifier: f32,
    #[serde(rename = "Defender War Count Modifier", default)]
    pub defender_war_count_modifier: f32,
    #[serde(rename = "Counter Attack Discount", default)]
    pub counter_attack_discount: f32,
    #[serde(rename = "Siege Window Rounding Hours", default)]
    pub siege_window_rounding_hours: f32,
    #[serde(rename = "Base Upkeep War Declaration Penalty", default)]
    pub base_upkeep_war_declaration_penalty: f32,
    #[serde(rename = "Destroyed Claim Ends War", default)]
    pub destroyed_claim_ends_war: bool,
    #[serde(rename = "Larger Guild Multiplier", default)]
    pub larger_guild_multiplier: f32,
    #[serde(rename = "Member War Modifier Map", default)]
    pub member_war_modifier_map: std::collections::BTreeMap<i32, f32>,
    #[serde(rename = "Require Defender Territory", default)]
    pub require_defender_territory: bool,
    #[serde(rename = "Attacker Buildable Team Limits", default)]
    pub attacker_buildable_team_limits: [u32; 3],
    #[serde(rename = "Reset Effect Categories", default)]
    pub reset_effect_categories: Vec<String>,
    #[serde(rename = "Reset Effect Categories Crc", default)]
    pub reset_effect_categories_crc: Vec<AzCrc32>,
    #[serde(rename = "Post Siege Status Effects", default)]
    pub post_siege_status_effects: Vec<String>,
    #[serde(rename = "Post Siege Status Effects Crc", default)]
    pub post_siege_status_effects_crc: Vec<AzCrc32>,
    #[serde(rename = "Deployable Limits Editable", default)]
    pub deployable_limits_editable: std::collections::BTreeMap<String, WarDeployableLimitData>,
    #[serde(rename = "Deployable Limits", default)]
    pub deployable_limits: std::collections::HashMap<AzCrc32, WarDeployableLimitData>,
}

impl AzRtti for WarData {
    const NAME: &'static str = "WarData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4FEBCF31_140C_4EF1_8C53_814DAA4426AC);
}
