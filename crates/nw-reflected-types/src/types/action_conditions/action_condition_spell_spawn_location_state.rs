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
pub struct ActionConditionSpellSpawnLocationState {}

impl AzRtti for ActionConditionSpellSpawnLocationState {
    const NAME: &'static str = "ActionConditionSpellSpawnLocationState";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEB2ABF46_9FFE_4139_BB13_B2B2E25937EA);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
