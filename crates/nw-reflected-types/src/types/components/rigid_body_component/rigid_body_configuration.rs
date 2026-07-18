use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct RigidBodyConfiguration {
    #[serde(rename = "Shape Type", default)]
    pub shape_type: u32,
    #[serde(rename = "Shape Entity", default)]
    pub shape_entity: u64,
    #[serde(rename = "RnR Asset", default)]
    pub rn_r_asset: AzAsset,
    #[serde(rename = "Material", default)]
    pub material: String,
    #[serde(rename = "Physics behavior", default)]
    pub physics_behavior: i32,
    #[serde(rename = "Mass", default)]
    pub mass: f32,
    #[serde(rename = "Initially active", default)]
    pub initially_active: bool,
    #[serde(rename = "Initial linear velocity", default)]
    pub initial_linear_velocity: bevy_math::Vec3,
    #[serde(rename = "Initial angular velocity", default)]
    pub initial_angular_velocity: bevy_math::Vec3,
    #[serde(rename = "Restitution", default)]
    pub restitution: f32,
    #[serde(rename = "Friction", default)]
    pub friction: f32,
    #[serde(rename = "Linear damping", default)]
    pub linear_damping: f32,
    #[serde(rename = "Angular damping", default)]
    pub angular_damping: f32,
    #[serde(rename = "Sleeping conditions", default)]
    pub sleeping_conditions: i32,
    #[serde(rename = "Sleep linear velocity", default)]
    pub sleep_linear_velocity: f32,
    #[serde(rename = "Sleep angular velocity", default)]
    pub sleep_angular_velocity: f32,
    #[serde(rename = "Sleep energy", default)]
    pub sleep_energy: f32,
    #[serde(rename = "Sleep duration", default)]
    pub sleep_duration: f32,
    #[serde(rename = "Continuous physics", default)]
    pub continuous_physics: i32,
    #[serde(rename = "Continuous Distance factor", default)]
    pub continuous_distance_factor: f32,
    #[serde(rename = "Continuous Sphere radius", default)]
    pub continuous_sphere_radius: f32,
    #[serde(rename = "Auto inertia tensor", default)]
    pub auto_inertia_tensor: bool,
}

impl AzRtti for RigidBodyConfiguration {
    const NAME: &'static str = "RigidBodyConfiguration";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x96C23E11_A0CB_43F9_9554_73470CF201A3);
}
