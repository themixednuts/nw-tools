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
pub struct SetAnimationByCondition {
    #[serde(rename = "m_animNameWhenConditionActive", default)]
    pub anim_name_when_condition_active: SlayerScriptLiteral,
    #[serde(rename = "m_animNameWhenConditionInactive", default)]
    pub anim_name_when_condition_inactive: SlayerScriptLiteral,
    #[serde(rename = "m_overrideDefaultBlendFrames", default)]
    pub override_default_blend_frames: f32,
}

impl AzRtti for SetAnimationByCondition {
    const NAME: &'static str = "SetAnimationByCondition";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x76EF97CB_C41A_48C3_A87F_B050B73747E1);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
