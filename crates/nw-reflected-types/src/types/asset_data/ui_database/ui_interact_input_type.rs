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
pub struct UiInteractInputType {
    #[serde(rename = "Interact Input Type", default)]
    pub interact_input_type: i32,
}

impl AzRtti for UiInteractInputType {
    const NAME: &'static str = "UiInteractInputType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x54315677_23C8_48BC_BE6B_F39E9E8097D4);
}
