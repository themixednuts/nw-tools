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
pub struct ActionConditionOr {}

impl AzRtti for ActionConditionOr {
    const NAME: &'static str = "ActionConditionOr";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x95EAF0C4_74B9_454D_82E6_D09DF4EED759);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEAAF81BC_EF4C_470E_AE8E_0EAAA7FE501A)];
}
