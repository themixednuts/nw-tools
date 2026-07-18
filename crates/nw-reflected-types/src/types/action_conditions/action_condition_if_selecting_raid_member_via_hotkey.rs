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
pub struct ActionConditionIfSelectingRaidMemberViaHotkey {}

impl AzRtti for ActionConditionIfSelectingRaidMemberViaHotkey {
    const NAME: &'static str = "ActionConditionIfSelectingRaidMemberViaHotkey";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4B4245AC_1825_4BD9_84F2_3A767FC77C8F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
