use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
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
pub struct Homing {
    #[serde(rename = "m_drawDebug", default)]
    pub draw_debug: bool,
    #[serde(rename = "m_lockInitialTargetPos", default)]
    pub lock_initial_target_pos: bool,
    #[serde(rename = "m_useExponential", default)]
    pub use_exponential: bool,
    #[serde(rename = "m_fallbackToInputDir", default)]
    pub fallback_to_input_dir: bool,
    #[serde(rename = "m_repeatInitialInput", default)]
    pub repeat_initial_input: bool,
    #[serde(rename = "m_turnRate", default)]
    pub turn_rate: f32,
    #[serde(rename = "m_radius", default)]
    pub radius: f32,
    #[serde(rename = "m_maxAngle", default)]
    pub max_angle: f32,
    #[serde(rename = "m_height", default)]
    pub height: f32,
    #[serde(rename = "m_radiusWeight", default)]
    pub radius_weight: f32,
    #[serde(rename = "m_angleWeight", default)]
    pub angle_weight: f32,
    #[serde(rename = "m_heightWeight", default)]
    pub height_weight: f32,
    #[serde(rename = "m_minConeWidth", default)]
    pub min_cone_width: f32,
    #[serde(rename = "m_moveToTarget", default)]
    pub move_to_target: bool,
    #[serde(rename = "m_moveToDistance", default)]
    pub move_to_distance: f32,
    #[serde(rename = "m_moveToDuration", default)]
    pub move_to_duration: f32,
    #[serde(rename = "m_moveToVelocity", default)]
    pub move_to_velocity: f32,
    #[serde(rename = "m_maxMoveDistance", default)]
    pub max_move_distance: f32,
    #[serde(rename = "m_allowRotationAfterReachingTargetAngle", default)]
    pub allow_rotation_after_reaching_target_angle: bool,
    #[serde(rename = "m_allowMovementAfterReachingTargetPos", default)]
    pub allow_movement_after_reaching_target_pos: bool,
    #[serde(rename = "m_allowUpwardsMovement", default)]
    pub allow_upwards_movement: bool,
    #[serde(rename = "m_useRadiusforAITarget", default)]
    pub use_radiusfor_ai_target: bool,
    #[serde(rename = "m_cameraTargetLock", default)]
    pub camera_target_lock: bool,
    #[serde(rename = "m_useArc", default)]
    pub use_arc: bool,
    #[serde(rename = "m_allowTargetSwitching", default)]
    pub allow_target_switching: bool,
    #[serde(rename = "m_arcLookAhead", default)]
    pub arc_look_ahead: f32,
    #[serde(rename = "m_arcTargetAdjustZ", default)]
    pub arc_target_adjust_z: f32,
    #[serde(rename = "m_aiTargetBlackboardPosition", default)]
    pub ai_target_blackboard_position: AzCrc32,
}

impl AzRtti for Homing {
    const NAME: &'static str = "Homing";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x408FBD8E_6D32_479E_8EC4_D96893F9788A);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
