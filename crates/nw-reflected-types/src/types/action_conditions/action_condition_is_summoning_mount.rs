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
pub struct ActionConditionIsSummoningMount {}

impl AzRtti for ActionConditionIsSummoningMount {
    const NAME: &'static str = "ActionConditionIsSummoningMount";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x745176D5_E570_4FED_9C3A_DA3DD45D54E8);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
