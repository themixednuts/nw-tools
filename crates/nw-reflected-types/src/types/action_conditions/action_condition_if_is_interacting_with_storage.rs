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
pub struct ActionConditionIfIsInteractingWithStorage;

impl AzRtti for ActionConditionIfIsInteractingWithStorage {
    const NAME: &'static str = "ActionConditionIfIsInteractingWithStorage";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xCF6868C6_8C8F_494C_BC9D_597C5DFAEC65);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
