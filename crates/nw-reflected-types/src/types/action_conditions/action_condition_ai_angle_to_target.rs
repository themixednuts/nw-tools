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
pub struct ActionConditionAIAngleToTarget {}

impl AzRtti for ActionConditionAIAngleToTarget {
    const NAME: &'static str = "ActionConditionAIAngleToTarget";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9CD0E6E5_2055_4704_8649_61BA73D746D3);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
