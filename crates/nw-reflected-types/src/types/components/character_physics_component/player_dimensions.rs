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
pub struct PlayerDimensions {
    #[serde(rename = "Use Capsule", default)]
    pub use_capsule: bool,
    #[serde(rename = "Collider Radius", default)]
    pub collider_radius: f32,
    #[serde(rename = "Collider Half-Height", default)]
    pub collider_half_height: f32,
    #[serde(rename = "Height Collider", default)]
    pub height_collider: f32,
    #[serde(rename = "Height Pivot", default)]
    pub height_pivot: f32,
    #[serde(rename = "Height Eye", default)]
    pub height_eye: f32,
    #[serde(rename = "Height Head", default)]
    pub height_head: f32,
    #[serde(rename = "Head Radius", default)]
    pub head_radius: f32,
    #[serde(rename = "Unprojection Direction", default)]
    pub unprojection_direction: bevy_math::Vec3,
    #[serde(rename = "Max Unprojection", default)]
    pub max_unprojection: f32,
    #[serde(rename = "Ground Contact Epsilon", default)]
    pub ground_contact_epsilon: f32,
}

impl AzRtti for PlayerDimensions {
    const NAME: &'static str = "PlayerDimensions";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x64B9DBDA_90D4_4D3D_88EA_36810AF2C98F);
}
