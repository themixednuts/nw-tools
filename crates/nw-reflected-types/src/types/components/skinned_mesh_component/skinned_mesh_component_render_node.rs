use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{SimpleAssetReferenceMaterialDataAsset, SkinnedRenderOptions};
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct SkinnedMeshComponentRenderNode {
    #[serde(rename = "Visible", default)]
    pub visible: bool,
    #[serde(rename = "Skinned Mesh", default)]
    pub skinned_mesh: AzAsset,
    #[serde(rename = "Material Override", default)]
    pub material_override: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Material Overcoat", default)]
    pub material_overcoat: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Material Override Asset", default)]
    pub material_override_asset: AzAsset,
    #[serde(rename = "Material Overcoat Asset", default)]
    pub material_overcoat_asset: AzAsset,
    #[serde(rename = "Render Options", default)]
    pub render_options: SkinnedRenderOptions,
}

impl AzRtti for SkinnedMeshComponentRenderNode {
    const NAME: &'static str = "SkinnedMeshComponentRenderNode";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAE5CFE2B_6CFF_4B66_9B9C_C514BFDB8A88);
}
