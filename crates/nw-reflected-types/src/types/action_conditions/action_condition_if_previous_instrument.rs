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
pub struct ActionConditionIfPreviousInstrument;

impl AzRtti for ActionConditionIfPreviousInstrument {
    const NAME: &'static str = "ActionConditionIfPreviousInstrument";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2BEB193F_0E72_4202_B8A7_8A7AB4FFEAB7);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
