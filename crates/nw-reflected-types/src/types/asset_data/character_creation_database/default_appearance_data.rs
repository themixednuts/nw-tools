use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{
    SimpleAssetReferenceMaterialDataAsset, SimpleAssetReferenceSkinAsset,
    SimpleAssetReferenceTextureAsset,
};

use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct DefaultAppearanceData {
    #[serde(rename = "Skin", default)]
    pub skin: SimpleAssetReferenceSkinAsset,
    #[serde(rename = "Material Override", default)]
    pub material_override: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Mask", default)]
    pub mask: SimpleAssetReferenceTextureAsset,
    #[serde(rename = "Apply Skin Material", default)]
    pub apply_skin_material: bool,
}

impl AzRtti for DefaultAppearanceData {
    const NAME: &'static str = "DefaultAppearanceData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEC89C545_A600_4452_8C4B_ED049D070EF9);
}
