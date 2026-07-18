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
pub struct ActionConditionIfPlayerIsLoggedOff {}

impl AzRtti for ActionConditionIfPlayerIsLoggedOff {
    const NAME: &'static str = "ActionConditionIfPlayerIsLoggedOff";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0BA072C3_E7BE_4FAE_8849_A7628235D135);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
