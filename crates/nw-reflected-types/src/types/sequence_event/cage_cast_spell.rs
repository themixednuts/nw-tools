use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SlayerScriptLiteral;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CAGECastSpell {
    #[serde(rename = "m_spellName", default)]
    pub spell_name: SlayerScriptLiteral,
    #[serde(rename = "m_takeDurability", default)]
    pub take_durability: bool,
    #[serde(rename = "m_aiAimAtTarget", default)]
    pub ai_aim_at_target: bool,
    #[serde(rename = "m_aiLeadTarget", default)]
    pub ai_lead_target: bool,
    #[serde(rename = "m_aiProjectilePredictionSpeed", default)]
    pub ai_projectile_prediction_speed: f32,
    #[serde(rename = "m_aiHitScanPredictionSpeed", default)]
    pub ai_hit_scan_prediction_speed: f32,
    #[serde(rename = "m_aiMissMinDistance", default)]
    pub ai_miss_min_distance: f32,
    #[serde(rename = "m_aiMissMaxDistance", default)]
    pub ai_miss_max_distance: f32,
    #[serde(rename = "m_aiLeadTargetMaxAngle", default)]
    pub ai_lead_target_max_angle: f32,
    #[serde(rename = "m_aiAimMaxAngle", default)]
    pub ai_aim_max_angle: f32,
    #[serde(rename = "m_aiTrackMinionSpawns", default)]
    pub ai_track_minion_spawns: bool,
    #[serde(rename = "m_useAllAvailableTargets", default)]
    pub use_all_available_targets: bool,
    #[serde(rename = "m_aiRandomlySelectATarget", default)]
    pub ai_randomly_select_a_target: bool,
    #[serde(rename = "m_aiTargetOffsetInMeters", default)]
    pub ai_target_offset_in_meters: bevy_math::Vec3,
    #[serde(rename = "m_aiUseTargetFacingForOffset", default)]
    pub ai_use_target_facing_for_offset: bool,
    #[serde(rename = "m_aiNoVerticalLaunchVelocity", default)]
    pub ai_no_vertical_launch_velocity: bool,
}

impl AzRtti for CAGECastSpell {
    const NAME: &'static str = "CAGECastSpell";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x497AC2A4_C162_43F9_BA28_5979203675EF);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
