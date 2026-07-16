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
pub struct ActionConditionAIDistanceToTarget;

impl AzRtti for ActionConditionAIDistanceToTarget {
    const NAME: &'static str = "ActionConditionAIDistanceToTarget";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD1C7E9DC_81E6_471C_A235_3255C79ED8E4);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
