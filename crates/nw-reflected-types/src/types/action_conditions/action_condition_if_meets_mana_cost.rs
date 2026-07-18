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
pub struct ActionConditionIfMeetsManaCost {}

impl AzRtti for ActionConditionIfMeetsManaCost {
    const NAME: &'static str = "ActionConditionIfMeetsManaCost";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD7A92D7E_31D4_4C84_AC78_64FC996A54C1);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
