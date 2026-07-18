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
pub struct ActionConditionHeightToGround {}

impl AzRtti for ActionConditionHeightToGround {
    const NAME: &'static str = "ActionConditionHeightToGround";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x84D5D1F4_4407_4215_AE1B_2F1464210C2F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
