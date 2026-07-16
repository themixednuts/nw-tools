use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::InputSubComponent;
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
pub struct InputEventGroup {
    #[serde(rename = "Event Name", default)]
    pub event_name: String,
    #[serde(rename = "Event Generators", default)]
    pub event_generators: Vec<InputSubComponent>,
}

impl AzRtti for InputEventGroup {
    const NAME: &'static str = "InputEventGroup";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x25143B7E_2FEC_4CC5_92FE_270B67E79734);
}
