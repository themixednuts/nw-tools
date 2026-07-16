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
pub struct ActionConditionIfRangedWeaponObstructed;

impl AzRtti for ActionConditionIfRangedWeaponObstructed {
    const NAME: &'static str = "ActionConditionIfRangedWeaponObstructed";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2C946028_6E0B_4363_AA61_838683F71DBC);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
