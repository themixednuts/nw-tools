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
pub struct ActionConditionIfMountSlowWalk {}

impl AzRtti for ActionConditionIfMountSlowWalk {
    const NAME: &'static str = "ActionConditionIfMountSlowWalk";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x316D9996_6637_43EB_A018_7F71D0343AE5);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
