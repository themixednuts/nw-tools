use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{PaperdollSlotTypes, SlayerScriptLiteral};
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CAGERangedAttack {
    #[serde(rename = "m_rangedAttackName", default)]
    pub ranged_attack_name: SlayerScriptLiteral,
    #[serde(rename = "m_chargeValue", default)]
    pub charge_value: f32,
    #[serde(rename = "m_damageTableRow", default)]
    pub damage_table_row: SlayerScriptLiteral,
    #[serde(rename = "m_fireJoint", default)]
    pub fire_joint: String,
    #[serde(rename = "m_posOffset", default)]
    pub pos_offset: bevy_math::Vec3,
    #[serde(rename = "m_rotOffset", default)]
    pub rot_offset: bevy_math::Vec3,
    #[serde(rename = "m_useJointTransformForOffsets", default)]
    pub use_joint_transform_for_offsets: bool,
    #[serde(rename = "m_accuracyBonus", default)]
    pub accuracy_bonus: f32,
    #[serde(rename = "m_sliceOverride", default)]
    pub slice_override: i8,
    #[serde(rename = "m_forwardSpawnInfo", default)]
    pub forward_spawn_info: bool,
    #[serde(rename = "m_useAmmo", default)]
    pub use_ammo: bool,
    #[serde(rename = "m_useActiveWeaponSlotAmmo", default)]
    pub use_active_weapon_slot_ammo: bool,
    #[serde(rename = "m_ammoSlot", default)]
    pub ammo_slot: PaperdollSlotTypes,
    #[serde(rename = "m_consumeAmmo", default)]
    pub consume_ammo: bool,
    #[serde(rename = "m_ammoCountToConsume", default)]
    pub ammo_count_to_consume: i32,
    #[serde(rename = "m_aiAimAtTarget", default)]
    pub ai_aim_at_target: bool,
    #[serde(rename = "m_aiUseSelectedPositionAction", default)]
    pub ai_use_selected_position_action: bool,
    #[serde(rename = "m_aiLeadTarget", default)]
    pub ai_lead_target: bool,
    #[serde(rename = "m_projectileSpeed", default)]
    pub projectile_speed: f32,
    #[serde(rename = "m_hitScanPredictionSpeed", default)]
    pub hit_scan_prediction_speed: f32,
    #[serde(rename = "m_aimJoint", default)]
    pub aim_joint: SlayerScriptLiteral,
    #[serde(rename = "m_aiMissMinDistance", default)]
    pub ai_miss_min_distance: f32,
    #[serde(rename = "m_aiMissMaxDistance", default)]
    pub ai_miss_max_distance: f32,
    #[serde(rename = "m_aiLeadTargetMaxAngle", default)]
    pub ai_lead_target_max_angle: f32,
    #[serde(rename = "m_aiAimMaxAngle", default)]
    pub ai_aim_max_angle: f32,
    #[serde(rename = "m_aiUseTargetGroundPos", default)]
    pub ai_use_target_ground_pos: bool,
    #[serde(rename = "m_aiAddTargetOffset", default)]
    pub ai_add_target_offset: bool,
    #[serde(rename = "m_aiTargetOffset", default)]
    pub ai_target_offset: bevy_math::Vec3,
    #[serde(rename = "m_forwardRotOffset", default)]
    pub forward_rot_offset: bevy_math::Vec3,
    #[serde(rename = "m_aiUseAllAvailableTargets", default)]
    pub ai_use_all_available_targets: bool,
    #[serde(rename = "m_aiRandomlySelectATarget", default)]
    pub ai_randomly_select_a_target: bool,
    #[serde(rename = "m_aiUseTargetFacingForOffset", default)]
    pub ai_use_target_facing_for_offset: bool,
    #[serde(rename = "m_useForwardFiringOffset", default)]
    pub use_forward_firing_offset: bool,
    #[serde(rename = "m_aiNoVerticalLaunchVelocity", default)]
    pub ai_no_vertical_launch_velocity: bool,
}

impl AzRtti for CAGERangedAttack {
    const NAME: &'static str = "CAGERangedAttack";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEC5F9BA1_1B85_44AE_A7AD_E88D1F7AA69C);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
