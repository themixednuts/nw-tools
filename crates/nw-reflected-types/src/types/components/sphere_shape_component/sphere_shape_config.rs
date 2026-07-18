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
pub struct SphereShapeConfig {
    #[serde(rename = "Radius", default)]
    pub radius: f32,
}

impl AzRtti for SphereShapeConfig {
    const NAME: &'static str = "SphereShapeConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4AADFD75_48A7_4F31_8F30_FE4505F09E35);
}
