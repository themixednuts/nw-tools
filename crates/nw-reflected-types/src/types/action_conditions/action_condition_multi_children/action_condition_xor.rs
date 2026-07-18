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
pub struct ActionConditionXor {}

impl AzRtti for ActionConditionXor {
    const NAME: &'static str = "ActionConditionXor";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x48B299B9_FF06_4BC4_84E7_1A2F88C4E35D);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEAAF81BC_EF4C_470E_AE8E_0EAAA7FE501A)];
}
