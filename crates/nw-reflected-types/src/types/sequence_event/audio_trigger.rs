use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{EAudioObjectObstructionCalcType, SlayerScriptLiteral};
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
pub struct AudioTriggerB73C9B69 {
    #[serde(rename = "m_startTrigger", default)]
    pub start_trigger: SlayerScriptLiteral,
    #[serde(rename = "m_stopTrigger", default)]
    pub stop_trigger: SlayerScriptLiteral,
    #[serde(rename = "m_soundObstructionType", default)]
    pub sound_obstruction_type: EAudioObjectObstructionCalcType,
    #[serde(rename = "m_attachmentJoint", default)]
    pub attachment_joint: SlayerScriptLiteral,
    #[serde(rename = "m_usePosOffset", default)]
    pub use_pos_offset: bool,
    #[serde(rename = "m_followPosOffset", default)]
    pub follow_pos_offset: bool,
    #[serde(rename = "m_posOffset", default)]
    pub pos_offset: bevy_math::Vec3,
}

impl AzRtti for AudioTriggerB73C9B69 {
    const NAME: &'static str = "AudioTrigger";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xB73C9B69_BA5B_4A4B_8ACF_0F250AD07B18);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
