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
pub struct ActionConditionIfCameraLockActive {}

impl AzRtti for ActionConditionIfCameraLockActive {
    const NAME: &'static str = "ActionConditionIfCameraLockActive";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7B10A0C8_5872_4AE3_91A3_26B7322A1F04);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
