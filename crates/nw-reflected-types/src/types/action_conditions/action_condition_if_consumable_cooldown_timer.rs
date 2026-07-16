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
pub struct ActionConditionIfConsumableCooldownTimer;

impl AzRtti for ActionConditionIfConsumableCooldownTimer {
    const NAME: &'static str = "ActionConditionIfConsumableCooldownTimer";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBE1966F6_BD55_49EA_A58F_AA2226C28491);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
