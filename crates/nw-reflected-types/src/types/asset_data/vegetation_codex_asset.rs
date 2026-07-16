use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::VegetationDescriptor;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct VegetationCodexAsset {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Vegetation", default)]
    pub vegetation: Vec<VegetationDescriptor>,
}

impl AzRtti for VegetationCodexAsset {
    const NAME: &'static str = "VegetationCodexAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4117DE18_CC44_441A_8C2A_023AB6A6C6AB);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
