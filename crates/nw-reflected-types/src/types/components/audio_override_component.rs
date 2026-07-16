use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    bevy_ecs::component::Component,
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AudioOverrideComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Impact Override Material", default)]
    pub impact_override_material: String,
    #[serde(rename = "InteractableOverrideSwitchName", default)]
    pub interactable_override_switch_name: String,
    #[serde(rename = "InteractableOverrideSwitchOnStateName", default)]
    pub interactable_override_switch_on_state_name: String,
    #[serde(rename = "InteractableOverrideSwitchOffStateName", default)]
    pub interactable_override_switch_off_state_name: String,
}

impl AzRtti for AudioOverrideComponent {
    const NAME: &'static str = "AudioOverrideComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6B28DF87_D282_4E5B_A817_0B115C72280B);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
