use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{PaperdollSlotAlias, PaperdollSlotTypes, SlayerScriptLiteral};
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod melee_attack_shape_cast_type;

pub use self::melee_attack_shape_cast_type::MeleeAttackShapeCastType;

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
pub struct CAGEDamage {
    #[serde(rename = "m_damageKey", default)]
    pub damage_key: SlayerScriptLiteral,
    #[serde(rename = "m_damageTableRow", default)]
    pub damage_table_row: SlayerScriptLiteral,
    #[serde(rename = "m_damageSelf", default)]
    pub damage_self: bool,
    #[serde(rename = "m_scale", default)]
    pub scale: bevy_math::Vec3,
    #[serde(rename = "m_offset", default)]
    pub offset: bevy_math::Vec3,
    #[serde(rename = "m_meleeAttackShapeCastType", default)]
    pub melee_attack_shape_cast_type: MeleeAttackShapeCastType,
    #[serde(rename = "m_meleeAttackShapeRadius", default)]
    pub melee_attack_shape_radius: f32,
    #[serde(rename = "m_meleeAttackCapsuleHalfHeight", default)]
    pub melee_attack_capsule_half_height: f32,
    #[serde(rename = "m_meleeAttackBoxDimensions", default)]
    pub melee_attack_box_dimensions: bevy_math::Vec3,
    #[serde(rename = "m_attachedAnimationAlias", default)]
    pub attached_animation_alias: SlayerScriptLiteral,
    #[serde(rename = "m_jointName", default)]
    pub joint_name: SlayerScriptLiteral,
    #[serde(rename = "m_overrideWeaponSlotAlias", default)]
    pub override_weapon_slot_alias: bool,
    #[serde(rename = "m_weaponSlotOverride", default)]
    pub weapon_slot_override: PaperdollSlotAlias,
    #[serde(rename = "m_useOffhandWeapon", default)]
    pub use_offhand_weapon: bool,
    #[serde(rename = "m_rotationOffset", default)]
    pub rotation_offset: bevy_math::Vec3,
    #[serde(rename = "m_ammoSlotForScaling", default)]
    pub ammo_slot_for_scaling: PaperdollSlotTypes,
    #[serde(rename = "m_shapeAxesModifierCommands", default)]
    pub shape_axes_modifier_commands: SlayerScriptLiteral,
    #[serde(rename = "m_disableLOSCheck", default)]
    pub disable_los_check: bool,
    #[serde(rename = "m_useEndAsCenter", default)]
    pub use_end_as_center: bool,
    #[serde(rename = "m_useMaxEnvironmentImpactAngle", default)]
    pub use_max_environment_impact_angle: bool,
    #[serde(rename = "m_useCameraPitch", default)]
    pub use_camera_pitch: bool,
    #[serde(rename = "m_takeDurability", default)]
    pub take_durability: bool,
    #[serde(rename = "m_takeDurabilityOnUse", default)]
    pub take_durability_on_use: bool,
    #[serde(rename = "m_pulseLength", default)]
    pub pulse_length: f32,
    #[serde(rename = "m_affectAlliesOnly", default)]
    pub affect_allies_only: bool,
    #[serde(rename = "m_intervalLength", default)]
    pub interval_length: f32,
}

impl AzRtti for CAGEDamage {
    const NAME: &'static str = "CAGEDamage";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7C829D13_1328_4D7B_BD62_62386294F97E);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
