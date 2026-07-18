use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct RigidPhysicsConfig {
    #[serde(rename = "EnabledInitially", default)]
    pub enabled_initially: bool,
    #[serde(rename = "SpecifyMassOrDensity", default)]
    pub specify_mass_or_density: u32,
    #[serde(rename = "Mass", default)]
    pub mass: f32,
    #[serde(rename = "Density", default)]
    pub density: f32,
    #[serde(rename = "AtRestInitially", default)]
    pub at_rest_initially: bool,
    #[serde(rename = "EnableCollisionResponse", default)]
    pub enable_collision_response: bool,
    #[serde(rename = "InteractsWithTriggers", default)]
    pub interacts_with_triggers: bool,
    #[serde(rename = "RecordCollisions", default)]
    pub record_collisions: bool,
    #[serde(rename = "MaxRecordedCollisions", default)]
    pub max_recorded_collisions: i32,
    #[serde(rename = "SimulationDamping", default)]
    pub simulation_damping: f32,
    #[serde(rename = "SimulationMinEnergy", default)]
    pub simulation_min_energy: f32,
    #[serde(rename = "BuoyancyDamping", default)]
    pub buoyancy_damping: f32,
    #[serde(rename = "BuoyancyDensity", default)]
    pub buoyancy_density: f32,
    #[serde(rename = "BuoyancyResistance", default)]
    pub buoyancy_resistance: f32,
}

impl AzRtti for RigidPhysicsConfig {
    const NAME: &'static str = "RigidPhysicsConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4D4211C2_4539_444F_A8AC_B0C8417AA579);
}
