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
pub struct ActionConditionIfAmmoTypeInSlot;

impl AzRtti for ActionConditionIfAmmoTypeInSlot {
    const NAME: &'static str = "ActionConditionIfAmmoTypeInSlot";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4DE0CD4A_CBC3_443C_9C2D_86DCAC465A3A);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
