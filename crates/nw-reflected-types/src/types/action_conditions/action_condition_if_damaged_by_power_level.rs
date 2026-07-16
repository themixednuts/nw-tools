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
pub struct ActionConditionIfDamagedByPowerLevel;

impl AzRtti for ActionConditionIfDamagedByPowerLevel {
    const NAME: &'static str = "ActionConditionIfDamagedByPowerLevel";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3CC3FA1F_C5A1_4654_86CD_3ADB5E7EC3CB);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
