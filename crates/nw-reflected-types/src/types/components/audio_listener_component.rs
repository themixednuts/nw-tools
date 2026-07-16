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
pub struct AudioListenerComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Rotation Entity", default)]
    pub rotation_entity: u64,
    #[serde(rename = "Position Entity", default)]
    pub position_entity: u64,
    #[serde(rename = "Fixed offset", default)]
    pub fixed_offset: bevy_math::Vec3,
    #[serde(rename = "Offset Ratio", default)]
    pub offset_ratio: f32,
}

impl AzRtti for AudioListenerComponent {
    const NAME: &'static str = "AudioListenerComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x00B5358C_3EEE_4012_93FC_6222B0004404);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
