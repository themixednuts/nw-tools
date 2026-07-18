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
pub struct ActionConditionIfForcingMountWalk {}

impl AzRtti for ActionConditionIfForcingMountWalk {
    const NAME: &'static str = "ActionConditionIfForcingMountWalk";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x882615B9_8F53_47E5_B2C4_54641E9A8CAE);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
