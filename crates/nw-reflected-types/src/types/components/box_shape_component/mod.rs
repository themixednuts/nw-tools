use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod box_shape_config;

pub use self::box_shape_config::BoxShapeConfig;

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
pub struct BoxShapeComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Configuration", default)]
    pub configuration: BoxShapeConfig,
}

impl AzRtti for BoxShapeComponent {
    const NAME: &'static str = "BoxShapeComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x5EDF4B9E_0D3D_40B8_8C91_5142BCFC30A6);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
