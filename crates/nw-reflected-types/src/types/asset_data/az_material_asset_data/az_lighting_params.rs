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
pub struct AzLightingParams {
    #[serde(rename = "Diffuse", default)]
    pub diffuse: bevy_math::Vec4,
    #[serde(rename = "Specular", default)]
    pub specular: bevy_math::Vec4,
    #[serde(rename = "Emittance", default)]
    pub emittance: bevy_math::Vec4,
    #[serde(rename = "Opacity", default)]
    pub opacity: f32,
    #[serde(rename = "Smoothness", default)]
    pub smoothness: f32,
    #[serde(rename = "AlphaRef", default)]
    pub alpha_ref: f32,
    #[serde(rename = "VoxelCoverage", default)]
    pub voxel_coverage: u8,
}

impl AzRtti for AzLightingParams {
    const NAME: &'static str = "AzLightingParams";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE42A0431_9880_47DA_AC87_81B94E33C5F5);
}
