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
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct SetSequenceByCondition {
    #[serde(rename = "m_sequenceWhenConditionActive", default)]
    pub sequence_when_condition_active: i32,
    #[serde(rename = "m_sequenceWhenConditionInactive", default)]
    pub sequence_when_condition_inactive: i32,
    #[serde(rename = "m_animNameWhenConditionActive", default)]
    pub anim_name_when_condition_active: SlayerScriptLiteral,
    #[serde(rename = "m_animNameWhenConditionInactive", default)]
    pub anim_name_when_condition_inactive: SlayerScriptLiteral,
}

impl AzRtti for SetSequenceByCondition {
    const NAME: &'static str = "SetSequenceByCondition";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEFDC781D_512C_45EE_A0A9_B080F9E294EE);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
