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
pub struct ActionConditionIfUnstuckTeleporting {}

impl AzRtti for ActionConditionIfUnstuckTeleporting {
    const NAME: &'static str = "ActionConditionIfUnstuckTeleporting";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBFCFF7F2_8940_41E6_90C5_1274909C99A0);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
