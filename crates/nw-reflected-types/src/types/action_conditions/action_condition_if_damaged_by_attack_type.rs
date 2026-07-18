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
pub struct ActionConditionIfDamagedByAttackType {}

impl AzRtti for ActionConditionIfDamagedByAttackType {
    const NAME: &'static str = "ActionConditionIfDamagedByAttackType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAC45DA25_F2C3_452F_B295_57DF87CDE016);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
