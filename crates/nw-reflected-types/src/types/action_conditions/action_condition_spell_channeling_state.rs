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
pub struct ActionConditionSpellChannelingState;

impl AzRtti for ActionConditionSpellChannelingState {
    const NAME: &'static str = "ActionConditionSpellChannelingState";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x813BE9EE_AE7A_4FB8_8130_3110D04C6A5B);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
