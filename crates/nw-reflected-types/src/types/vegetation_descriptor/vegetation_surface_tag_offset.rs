use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::VegetationSurfaceTag;
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
pub struct VegetationSurfaceTagOffset {
    #[serde(rename = "SurfaceTag", default)]
    pub surface_tag: VegetationSurfaceTag,
    #[serde(rename = "Offset", default)]
    pub offset: bevy_math::Vec3,
}

impl AzRtti for VegetationSurfaceTagOffset {
    const NAME: &'static str = "VegetationSurfaceTagOffset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBFBC4B26_3E75_4DAE_A208_2C58D4BA4CE4);
}
