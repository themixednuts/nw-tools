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
pub struct ActionConditionIfItemInSlotBroken;

impl AzRtti for ActionConditionIfItemInSlotBroken {
    const NAME: &'static str = "ActionConditionIfItemInSlotBroken";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x68ED13B5_E23E_4CA6_AF0D_8D577D125533);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
