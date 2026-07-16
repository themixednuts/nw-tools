use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct MaterialProperties {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Friction", default)]
    pub friction: f32,
    #[serde(rename = "Restitution", default)]
    pub restitution: f32,
    #[serde(rename = "Traversable", default)]
    pub traversable: bool,
    #[serde(rename = "SurfaceType", default)]
    pub surface_type: String,
}

impl AzRtti for MaterialProperties {
    const NAME: &'static str = "MaterialProperties";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x8807CAA1_AD08_4238_8FDB_2154ADD084A1);
}
