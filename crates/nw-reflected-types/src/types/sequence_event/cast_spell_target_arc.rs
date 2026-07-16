use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SlayerScriptLiteral;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CastSpellTargetArc {
    #[serde(rename = "m_attachedAnimationAlias", default)]
    pub attached_animation_alias: SlayerScriptLiteral,
    #[serde(rename = "m_sliceName", default)]
    pub slice_name: String,
    #[serde(rename = "m_spellName", default)]
    pub spell_name: SlayerScriptLiteral,
    #[serde(rename = "m_arcEffectName", default)]
    pub arc_effect_name: String,
    #[serde(rename = "m_accel", default)]
    pub accel: bevy_math::Vec3,
    #[serde(rename = "m_speed", default)]
    pub speed: f32,
    #[serde(rename = "m_alignToNormal", default)]
    pub align_to_normal: bool,
}

impl AzRtti for CastSpellTargetArc {
    const NAME: &'static str = "CastSpellTargetArc";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xB2382426_8B37_4BA1_A3B9_0EB6BB2C6429);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
