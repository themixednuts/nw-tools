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
pub struct ActionConditionIfAbilityNumHits {}

impl AzRtti for ActionConditionIfAbilityNumHits {
    const NAME: &'static str = "ActionConditionIfAbilityNumHits";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7C64AEBE_CF04_4216_8046_2D34C070A755);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
