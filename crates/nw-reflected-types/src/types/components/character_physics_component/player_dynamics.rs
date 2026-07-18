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
pub struct PlayerDynamics {
    #[serde(rename = "Mass", default)]
    pub mass: f32,
    #[serde(rename = "Inertia", default)]
    pub inertia: f32,
    #[serde(rename = "Inertia Acceleration", default)]
    pub inertia_acceleration: f32,
    #[serde(rename = "Time Impulse Recover", default)]
    pub time_impulse_recover: f32,
    #[serde(rename = "Air Control", default)]
    pub air_control: f32,
    #[serde(rename = "Air Resistance", default)]
    pub air_resistance: f32,
    #[serde(rename = "Use Custom Gravity", default)]
    pub use_custom_gravity: bool,
    #[serde(rename = "Gravity", default)]
    pub gravity: bevy_math::Vec3,
    #[serde(rename = "Nod Speed", default)]
    pub nod_speed: f32,
    #[serde(rename = "Is Active", default)]
    pub is_active: bool,
    #[serde(rename = "Release Ground Collider When Not Active", default)]
    pub release_ground_collider_when_not_active: bool,
    #[serde(rename = "Is Swimming", default)]
    pub is_swimming: bool,
    #[serde(rename = "Surface Index", default)]
    pub surface_index: i32,
    #[serde(rename = "Min Fall Angle", default)]
    pub min_fall_angle: f32,
    #[serde(rename = "Min Slide Angle", default)]
    pub min_slide_angle: f32,
    #[serde(rename = "Max Climb Angle", default)]
    pub max_climb_angle: f32,
    #[serde(rename = "Max Jump Angle", default)]
    pub max_jump_angle: f32,
    #[serde(rename = "Max Velocity Ground", default)]
    pub max_velocity_ground: f32,
    #[serde(rename = "Collide With Terrain", default)]
    pub collide_with_terrain: bool,
    #[serde(rename = "Collide With Static", default)]
    pub collide_with_static: bool,
    #[serde(rename = "Collide With Rigid", default)]
    pub collide_with_rigid: bool,
    #[serde(rename = "Collide With Sleeping Rigid", default)]
    pub collide_with_sleeping_rigid: bool,
    #[serde(rename = "Collide With Living", default)]
    pub collide_with_living: bool,
    #[serde(rename = "Collide With Independent", default)]
    pub collide_with_independent: bool,
    #[serde(rename = "RecordCollisions", default)]
    pub record_collisions: bool,
    #[serde(rename = "MaxRecordedCollisions", default)]
    pub max_recorded_collisions: i32,
}

impl AzRtti for PlayerDynamics {
    const NAME: &'static str = "PlayerDynamics";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xB1237004_14E1_4327_8774_D2C5796230E7);
}
