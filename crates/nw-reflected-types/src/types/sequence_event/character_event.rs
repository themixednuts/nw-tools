use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{EAudioObjectObstructionCalcType, SequenceEventOptions, SlayerScriptLiteral};

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
pub struct CharacterEvent {
    #[serde(rename = "m_characterEventName", default)]
    pub character_event_name: i8,
    #[serde(rename = "m_attachmentJoint", default)]
    pub attachment_joint: SlayerScriptLiteral,
    #[serde(rename = "m_soundObstructionType", default)]
    pub sound_obstruction_type: EAudioObjectObstructionCalcType,
    #[serde(rename = "m_optionOnEnter", default)]
    pub option_on_enter: SequenceEventOptions,
    #[serde(rename = "m_optionOnExit", default)]
    pub option_on_exit: SequenceEventOptions,
}

impl AzRtti for CharacterEvent {
    const NAME: &'static str = "CharacterEvent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x686E9AD7_A071_453F_BF11_54A430330D6E);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
