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
pub struct ActionConditionCanUseItemInSlot {}

impl AzRtti for ActionConditionCanUseItemInSlot {
    const NAME: &'static str = "ActionConditionCanUseItemInSlot";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x8C770252_19EF_413F_92B9_902FCA874B21);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
