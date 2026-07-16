use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CollisionFilterColor {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Color", default)]
    pub color: bevy_color::LinearRgba,
}

impl AzRtti for CollisionFilterColor {
    const NAME: &'static str = "CollisionFilterColor";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD6F2C792_D886_4600_B81C_548DF895A5E6);
}
