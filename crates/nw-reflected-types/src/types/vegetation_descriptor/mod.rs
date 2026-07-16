use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{Any, SimpleAssetReferenceMaterialDataAsset, VegetationSurfaceTag};
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod vegetation_surface_tag_depth;
pub mod vegetation_surface_tag_offset;

pub use self::vegetation_surface_tag_depth::VegetationSurfaceTagDepth;
pub use self::vegetation_surface_tag_offset::VegetationSurfaceTagOffset;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct VegetationDescriptor {
    #[serde(rename = "MeshAsset", default)]
    pub mesh_asset: AzAsset,
    #[serde(rename = "MaterialAsset", default)]
    pub material_asset: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Weight", default)]
    pub weight: f32,
    #[serde(rename = "AutoMerge", default)]
    pub auto_merge: bool,
    #[serde(rename = "SurfaceOffsetTags", default)]
    pub surface_offset_tags: Vec<VegetationSurfaceTagOffset>,
    #[serde(rename = "SurfaceDepthTags", default)]
    pub surface_depth_tags: Vec<VegetationSurfaceTagDepth>,
    #[serde(rename = "SurfaceFilterOverrideMode", default)]
    pub surface_filter_override_mode: u8,
    #[serde(rename = "InclusiveSurfaceFilterTags", default)]
    pub inclusive_surface_filter_tags: Vec<VegetationSurfaceTag>,
    #[serde(rename = "ExclusiveSurfaceFilterTags", default)]
    pub exclusive_surface_filter_tags: Vec<VegetationSurfaceTag>,
    #[serde(rename = "SurfaceAlignmentOverrideEnabled", default)]
    pub surface_alignment_override_enabled: bool,
    #[serde(rename = "SurfaceAlignmentMin", default)]
    pub surface_alignment_min: f32,
    #[serde(rename = "SurfaceAlignmentMax", default)]
    pub surface_alignment_max: f32,
    #[serde(rename = "RotationOverrideEnabled", default)]
    pub rotation_override_enabled: bool,
    #[serde(rename = "RotationMin", default)]
    pub rotation_min: bevy_math::Vec3,
    #[serde(rename = "RotationMax", default)]
    pub rotation_max: bevy_math::Vec3,
    #[serde(rename = "PositionOverrideEnabled", default)]
    pub position_override_enabled: bool,
    #[serde(rename = "PositionMin", default)]
    pub position_min: bevy_math::Vec3,
    #[serde(rename = "PositionMax", default)]
    pub position_max: bevy_math::Vec3,
    #[serde(rename = "ScaleOverrideEnabled", default)]
    pub scale_override_enabled: bool,
    #[serde(rename = "ScaleMin", default)]
    pub scale_min: f32,
    #[serde(rename = "ScaleMax", default)]
    pub scale_max: f32,
    #[serde(rename = "AltitudeFilterOverrideEnabled", default)]
    pub altitude_filter_override_enabled: bool,
    #[serde(rename = "AltitudeFilterMin", default)]
    pub altitude_filter_min: f32,
    #[serde(rename = "AltitudeFilterMax", default)]
    pub altitude_filter_max: f32,
    #[serde(rename = "SlopeFilterOverrideEnabled", default)]
    pub slope_filter_override_enabled: bool,
    #[serde(rename = "SlopeFilterMin", default)]
    pub slope_filter_min: f32,
    #[serde(rename = "SlopeFilterMax", default)]
    pub slope_filter_max: f32,
    #[serde(rename = "Bending", default)]
    pub bending: f32,
    #[serde(rename = "UserData", default)]
    pub user_data: Any,
}

impl AzRtti for VegetationDescriptor {
    const NAME: &'static str = "VegetationDescriptor";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE0B4E1E7_BAAC_4540_B9EE_29283A50DC8B);
}
