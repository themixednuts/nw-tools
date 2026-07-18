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
pub struct ActionConditionIfGatherableIsInState {}

impl AzRtti for ActionConditionIfGatherableIsInState {
    const NAME: &'static str = "ActionConditionIfGatherableIsInState";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBA0E8EA9_B8AE_4069_8EB8_EC4A802F5549);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
