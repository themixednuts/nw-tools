use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SlayerScriptLiteral;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod slayer_script_edit_literal;

pub use self::slayer_script_edit_literal::SlayerScriptEditLiteral;

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
pub struct ParticleEffect {
    #[serde(rename = "m_effectName", default)]
    pub effect_name: SlayerScriptEditLiteral,
    #[serde(rename = "m_attachedAnimationAlias", default)]
    pub attached_animation_alias: SlayerScriptEditLiteral,
    #[serde(rename = "m_jointName", default)]
    pub joint_name: SlayerScriptEditLiteral,
    #[serde(rename = "m_attachmentName", default)]
    pub attachment_name: SlayerScriptEditLiteral,
    #[serde(rename = "m_posOffset", default)]
    pub pos_offset: bevy_math::Vec3,
    #[serde(rename = "m_rotOffset", default)]
    pub rot_offset: bevy_math::Vec3,
    #[serde(rename = "m_ignoreRotation", default)]
    pub ignore_rotation: bool,
    #[serde(rename = "m_emitterFollows", default)]
    pub emitter_follows: bool,
    #[serde(rename = "m_cloneAttachment", default)]
    pub clone_attachment: bool,
    #[serde(rename = "m_killOnExit", default)]
    pub kill_on_exit: bool,
    #[serde(rename = "m_keepEmitterActive", default)]
    pub keep_emitter_active: bool,
    #[serde(rename = "m_sequencePerformsActiveAbility", default)]
    pub sequence_performs_active_ability: bool,
    #[serde(rename = "m_filterName", default)]
    pub filter_name: SlayerScriptLiteral,
    #[serde(rename = "m_tilt", default)]
    pub tilt: bool,
    #[serde(rename = "m_snap", default)]
    pub snap: bool,
    #[serde(rename = "m_maxRotationAngle", default)]
    pub max_rotation_angle: f32,
    #[serde(rename = "m_verticalCastStartOffset", default)]
    pub vertical_cast_start_offset: f32,
    #[serde(rename = "m_verticalCastDist", default)]
    pub vertical_cast_dist: f32,
    #[serde(rename = "m_scale", default)]
    pub scale: f32,
}

impl AzRtti for ParticleEffect {
    const NAME: &'static str = "ParticleEffect";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE82EB9AA_6D26_420D_B1E1_884A561BF9C3);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
