use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod action_condition_if_fragment_playing;

pub use self::action_condition_if_fragment_playing::ActionConditionIfFragmentPlaying;

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
pub struct ActionConditionIfFragmentDone {}

impl AzRtti for ActionConditionIfFragmentDone {
    const NAME: &'static str = "ActionConditionIfFragmentDone";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7594CAE2_BBD4_4262_A177_9C6442334EE6);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
