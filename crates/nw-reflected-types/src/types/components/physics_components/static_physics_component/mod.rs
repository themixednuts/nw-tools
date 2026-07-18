use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::PhysicsComponent;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod static_physics_config;

pub use self::static_physics_config::StaticPhysicsConfig;

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
pub struct StaticPhysicsComponent {
    #[serde(rename = "BaseClass1", default)]
    pub physics_component: PhysicsComponent,
    #[serde(rename = "Configuration", default)]
    pub configuration: StaticPhysicsConfig,
    #[serde(rename = "CollisionFilter", default)]
    pub collision_filter: String,
}

impl AzRtti for StaticPhysicsComponent {
    const NAME: &'static str = "StaticPhysicsComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x95D89791_6397_41BC_AAC5_95282C8AD9D4);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x6C2A2397_C33D_4ACA_8813_42B99E7B84DB)];
}
