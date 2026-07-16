use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ActionConditionIfHTNCAGEAction;
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
pub struct ActionConditionIfBehaviorTreeTask {
    #[serde(rename = "BaseClass1", default)]
    pub action_condition_if_htncage_action: ActionConditionIfHTNCAGEAction,
}

impl AzRtti for ActionConditionIfBehaviorTreeTask {
    const NAME: &'static str = "ActionConditionIfBehaviorTreeTask";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x03334B2C_F4CE_445D_96E4_EDAB6C2922E1);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xE1E78583_D02D_4717_9266_F600098D6ED4)];
}
