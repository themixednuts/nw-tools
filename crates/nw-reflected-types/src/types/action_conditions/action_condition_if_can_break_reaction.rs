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
pub struct ActionConditionIfCanBreakReaction;

impl AzRtti for ActionConditionIfCanBreakReaction {
    const NAME: &'static str = "ActionConditionIfCanBreakReaction";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD6780CCB_8219_44D9_98A9_9FC6E8E64AED);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
