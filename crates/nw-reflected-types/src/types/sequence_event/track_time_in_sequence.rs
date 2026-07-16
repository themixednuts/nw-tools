use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
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
pub struct TrackTimeInSequence {
    #[serde(rename = "m_optionOnEnter", default)]
    pub option_on_enter: i32,
    #[serde(rename = "m_optionOnExit", default)]
    pub option_on_exit: i32,
}

impl AzRtti for TrackTimeInSequence {
    const NAME: &'static str = "TrackTimeInSequence";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3BA007EA_555B_4D5D_B9E6_A6F331CD7900);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
