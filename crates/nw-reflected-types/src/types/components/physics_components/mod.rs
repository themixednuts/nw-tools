use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod rigid_physics_component;
pub mod static_physics_component;

pub use self::rigid_physics_component::{RigidPhysicsComponent, RigidPhysicsConfig};
pub use self::static_physics_component::{StaticPhysicsComponent, StaticPhysicsConfig};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct PhysicsComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
}

impl AzRtti for PhysicsComponent {
    const NAME: &'static str = "PhysicsComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6C2A2397_C33D_4ACA_8813_42B99E7B84DB);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
