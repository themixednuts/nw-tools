use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod input_event_bindings;
pub mod input_event_group;
pub mod input_sub_component;

pub use self::input_event_bindings::InputEventBindings;
pub use self::input_event_group::InputEventGroup;
pub use self::input_sub_component::InputSubComponent;

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
pub struct InputEventBindingsAsset {
    #[serde(rename = "Bindings", default)]
    pub bindings: InputEventBindings,
}

impl AzRtti for InputEventBindingsAsset {
    const NAME: &'static str = "InputEventBindingsAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x25971C7A_26E2_4D08_A146_2EFCC1C36B0C);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
