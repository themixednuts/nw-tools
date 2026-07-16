use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ActionConditionIfFragmentDone;
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
pub struct ActionConditionIfFragmentPlaying {
    #[serde(rename = "BaseClass1", default)]
    pub action_condition_if_fragment_done: ActionConditionIfFragmentDone,
}

impl AzRtti for ActionConditionIfFragmentPlaying {
    const NAME: &'static str = "ActionConditionIfFragmentPlaying";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x845A39DA_DFE7_4EF2_A915_39AC314AB462);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x7594CAE2_BBD4_4262_A177_9C6442334EE6)];
}
