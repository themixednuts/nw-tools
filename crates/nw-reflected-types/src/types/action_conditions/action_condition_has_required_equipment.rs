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
pub struct ActionConditionHasRequiredEquipment {}

impl AzRtti for ActionConditionHasRequiredEquipment {
    const NAME: &'static str = "ActionConditionHasRequiredEquipment";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x8CB55469_D000_41FD_8AB1_AF5BD8E30BA2);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
