use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod primitive_collider_config;

pub use self::primitive_collider_config::PrimitiveColliderConfig;

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
pub struct PrimitiveColliderComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Configuration", default)]
    pub configuration: PrimitiveColliderConfig,
}

impl AzRtti for PrimitiveColliderComponent {
    const NAME: &'static str = "PrimitiveColliderComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9CB3707A_73B3_4EE5_84EA_3CF86E0E3722);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
