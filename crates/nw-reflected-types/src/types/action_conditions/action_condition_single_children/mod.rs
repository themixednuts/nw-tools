use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod action_condition_group;
pub mod action_condition_not;

pub use self::action_condition_group::ActionConditionGroup;
pub use self::action_condition_not::ActionConditionNot;

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
pub struct ActionConditionSingleChild {}

impl AzRtti for ActionConditionSingleChild {
    const NAME: &'static str = "ActionConditionSingleChild";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x89837E1A_40E2_4066_B017_4E17E3BC8BA6);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
