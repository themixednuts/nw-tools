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
pub struct ActionConditionIfMusicalPerformanceState;

impl AzRtti for ActionConditionIfMusicalPerformanceState {
    const NAME: &'static str = "ActionConditionIfMusicalPerformanceState";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xDBC1AA5B_43E7_4105_8077_C714C17B2D8F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
