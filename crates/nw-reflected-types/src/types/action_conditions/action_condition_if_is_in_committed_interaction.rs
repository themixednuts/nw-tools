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
pub struct ActionConditionIfIsInCommittedInteraction {}

impl AzRtti for ActionConditionIfIsInCommittedInteraction {
    const NAME: &'static str = "ActionConditionIfIsInCommittedInteraction";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x918FC185_5217_4AED_9C3A_E3E532B55145);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
