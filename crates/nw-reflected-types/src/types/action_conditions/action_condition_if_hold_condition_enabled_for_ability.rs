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
pub struct ActionConditionIfHoldConditionEnabledForAbility {}

impl AzRtti for ActionConditionIfHoldConditionEnabledForAbility {
    const NAME: &'static str = "ActionConditionIfHoldConditionEnabledForAbility";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA73EB8B7_31EF_47E2_BB07_1C3A1EB23B83);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
