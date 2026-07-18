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
pub struct ActionConditionIfGatherableIsBeingGatheredFrom {}

impl AzRtti for ActionConditionIfGatherableIsBeingGatheredFrom {
    const NAME: &'static str = "ActionConditionIfGatherableIsBeingGatheredFrom";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xB4D871A6_BB47_4290_A1ED_0F0D1C25D6E0);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
