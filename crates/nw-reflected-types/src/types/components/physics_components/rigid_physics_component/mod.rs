use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::PhysicsComponent;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod rigid_physics_config;

pub use self::rigid_physics_config::RigidPhysicsConfig;

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
pub struct RigidPhysicsComponent {
    #[serde(rename = "BaseClass1", default)]
    pub physics_component: PhysicsComponent,
    #[serde(rename = "Configuration", default)]
    pub configuration: RigidPhysicsConfig,
}

impl AzRtti for RigidPhysicsComponent {
    const NAME: &'static str = "RigidPhysicsComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBF2ED241_6364_4D78_8008_498EF2A2659C);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x6C2A2397_C33D_4ACA_8813_42B99E7B84DB)];
}
