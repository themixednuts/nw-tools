use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::HTNPlanDoneActivity;
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
pub struct BehaviorTreeTaskActivity {
    #[serde(rename = "BaseClass1", default)]
    pub htn_plan_done_activity: HTNPlanDoneActivity,
}

impl AzRtti for BehaviorTreeTaskActivity {
    const NAME: &'static str = "BehaviorTreeTaskActivity";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3BEDA126_684E_425B_82A0_20AA4100F4FA);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x94E92357_4890_47C2_A9CC_C41DEE733B4D)];
}
