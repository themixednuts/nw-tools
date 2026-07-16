use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::InputEventGroup;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct InputEventBindings {
    #[serde(rename = "Input Event Groups", default)]
    pub input_event_groups: Vec<InputEventGroup>,
}

impl AzRtti for InputEventBindings {
    const NAME: &'static str = "InputEventBindings";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x14FFD4A8_AE46_4E23_B45B_6A7C4F787A91);
}
