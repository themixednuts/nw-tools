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
pub struct ActionConditionIfIsPlacingBuilding;

impl AzRtti for ActionConditionIfIsPlacingBuilding {
    const NAME: &'static str = "ActionConditionIfIsPlacingBuilding";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xECDEF30E_1D49_4B28_81B9_B8B580A0541C);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
