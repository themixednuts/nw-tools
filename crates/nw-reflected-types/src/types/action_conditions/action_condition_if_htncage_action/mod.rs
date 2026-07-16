use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod action_condition_if_behavior_tree_task;

pub use self::action_condition_if_behavior_tree_task::ActionConditionIfBehaviorTreeTask;

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
pub struct ActionConditionIfHTNCAGEAction;

impl AzRtti for ActionConditionIfHTNCAGEAction {
    const NAME: &'static str = "ActionConditionIfHTNCAGEAction";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE1E78583_D02D_4717_9266_F600098D6ED4);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
