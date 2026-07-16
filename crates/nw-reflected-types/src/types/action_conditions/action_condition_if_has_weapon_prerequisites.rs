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
pub struct ActionConditionIfHasWeaponPrerequisites;

impl AzRtti for ActionConditionIfHasWeaponPrerequisites {
    const NAME: &'static str = "ActionConditionIfHasWeaponPrerequisites";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x08034288_BFA9_48BD_8FC7_5960D3FFA124);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
