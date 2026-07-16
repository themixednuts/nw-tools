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
pub struct VegetationSurfaceTagDepth {
    #[serde(rename = "SurfaceTag", default)]
    pub surface_tag: VegetationSurfaceTag,
    #[serde(rename = "MinDepthInMeters", default)]
    pub min_depth_in_meters: f32,
}

impl AzRtti for VegetationSurfaceTagDepth {
    const NAME: &'static str = "VegetationSurfaceTagDepth";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x5E1B71A4_090D_4744_895B_87966D826386);
}
