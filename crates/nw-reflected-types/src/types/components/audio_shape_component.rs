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
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AudioShapeComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Exterior Follow Mode", default)]
    pub exterior_follow_mode: i32,
    #[serde(rename = "Interior Follow Mode", default)]
    pub interior_follow_mode: i32,
    #[serde(rename = "Interior Follow Offset", default)]
    pub interior_follow_offset: f32,
    #[serde(rename = "Send Enter/Exit Messages", default)]
    pub send_enter_exit_messages: bool,
    #[serde(rename = "Follow Camera Subject", default)]
    pub follow_camera_subject: bool,
}

impl AzRtti for AudioShapeComponent {
    const NAME: &'static str = "AudioShapeComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x58AABF8E_6954_4634_ACBD_05FE011478E1);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
