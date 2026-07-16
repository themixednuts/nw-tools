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
pub struct ActionConditionIfMannequinTagInItemSlot;

impl AzRtti for ActionConditionIfMannequinTagInItemSlot {
    const NAME: &'static str = "ActionConditionIfMannequinTagInItemSlot";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xCAAE497E_3434_48D0_BC41_626AC9E7D8C2);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
