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
pub struct ActionConditionIfTakingTooLongToMount;

impl AzRtti for ActionConditionIfTakingTooLongToMount {
    const NAME: &'static str = "ActionConditionIfTakingTooLongToMount";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA8119370_A5BC_4D5F_80CD_899E1775A3D3);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
