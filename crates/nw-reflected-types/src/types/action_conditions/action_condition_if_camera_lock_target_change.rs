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
pub struct ActionConditionIfCameraLockTargetChange;

impl AzRtti for ActionConditionIfCameraLockTargetChange {
    const NAME: &'static str = "ActionConditionIfCameraLockTargetChange";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x23037823_4871_4D6A_B781_531331BC9C40);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
