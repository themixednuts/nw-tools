use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ExecutionActivity;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod behavior_tree_task_activity;

pub use self::behavior_tree_task_activity::BehaviorTreeTaskActivity;

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
pub struct HTNPlanDoneActivity {
    #[serde(rename = "BaseClass1", default)]
    pub execution_activity: ExecutionActivity,
}

impl AzRtti for HTNPlanDoneActivity {
    const NAME: &'static str = "HTNPlanDoneActivity";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x94E92357_4890_47C2_A9CC_C41DEE733B4D);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x544B9BF1_0EBF_4786_B4A6_A026628B9E7F)];
}
