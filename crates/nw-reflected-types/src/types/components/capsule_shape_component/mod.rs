use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod capsule_shape_config;

pub use self::capsule_shape_config::CapsuleShapeConfig;

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
pub struct CapsuleShapeComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Configuration", default)]
    pub configuration: CapsuleShapeConfig,
}

impl AzRtti for CapsuleShapeComponent {
    const NAME: &'static str = "CapsuleShapeComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x967EC13D_364D_4696_AB5C_C00CC05A2305);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
