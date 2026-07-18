use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod rigid_body_configuration;

pub use self::rigid_body_configuration::RigidBodyConfiguration;

#[derive(
    bevy_ecs::component::Component,
    Debug,
    Default,
    Clone,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RigidBodyComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Physical Parameters", default)]
    pub physical_parameters: RigidBodyConfiguration,
}

impl AzRtti for RigidBodyComponent {
    const NAME: &'static str = "RigidBodyComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x51F92E5E_BD1A_4F9B_89F7_174205E4CBC7);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
