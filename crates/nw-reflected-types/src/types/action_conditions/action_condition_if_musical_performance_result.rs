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
pub struct ActionConditionIfMusicalPerformanceResult {}

impl AzRtti for ActionConditionIfMusicalPerformanceResult {
    const NAME: &'static str = "ActionConditionIfMusicalPerformanceResult";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x1BB639DD_AC2B_4BA4_B3D0_438C50885791);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
