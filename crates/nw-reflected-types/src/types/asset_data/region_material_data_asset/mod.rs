use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SerializableMacroMaterialParams;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod terrain_material_layer_data;

pub use self::terrain_material_layer_data::TerrainMaterialLayerData;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct RegionMaterialDataAsset {
    #[serde(rename = "Layers", default)]
    pub layers: Vec<TerrainMaterialLayerData>,
    #[serde(rename = "Default Material", default)]
    pub default_material: AzAsset,
    #[serde(rename = "Macro ColorMap", default)]
    pub macro_color_map: AzAsset,
    #[serde(rename = "Macro GlossMap", default)]
    pub macro_gloss_map: AzAsset,
    #[serde(rename = "Macro NormalMap", default)]
    pub macro_normal_map: AzAsset,
    #[serde(rename = "PertinentLayersMipChain", default)]
    pub pertinent_layers_mip_chain: Vec<u64>,
    #[serde(rename = "EnableCustomBackgroundParams", default)]
    pub enable_custom_background_params: bool,
    #[serde(rename = "MacroMaterialParams", default)]
    pub macro_material_params: SerializableMacroMaterialParams,
    #[serde(rename = "EnableCustomForegroundParams", default)]
    pub enable_custom_foreground_params: bool,
    #[serde(rename = "CustomMacroMaterialCompositingParams", default)]
    pub custom_macro_material_compositing_params: SerializableMacroMaterialParams,
}

impl AzRtti for RegionMaterialDataAsset {
    const NAME: &'static str = "RegionMaterialDataAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9A623978_DFB6_4CC1_A649_1F172637E52A);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
