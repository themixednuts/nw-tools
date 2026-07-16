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
pub struct ActionConditionIfAbilityUsedWithinTime;

impl AzRtti for ActionConditionIfAbilityUsedWithinTime {
    const NAME: &'static str = "ActionConditionIfAbilityUsedWithinTime";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xF4206147_668A_49E5_B07C_671AE1BC5E8F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
