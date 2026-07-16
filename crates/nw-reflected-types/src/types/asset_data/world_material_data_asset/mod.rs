use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SerializableMacroMaterialParams;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod tile_material_data;

pub use self::tile_material_data::TileMaterialData;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct WorldMaterialDataAsset {
    #[serde(rename = "Regions", default)]
    pub regions: Vec<TileMaterialData>,
    #[serde(rename = "BackgroundMacroMaterialParams", default)]
    pub background_macro_material_params: SerializableMacroMaterialParams,
    #[serde(rename = "ForegroundMacroMaterialParams", default)]
    pub foreground_macro_material_params: SerializableMacroMaterialParams,
    #[serde(rename = "POMHeightBias", default)]
    pub pom_height_bias: f32,
    #[serde(rename = "POMDisplacement", default)]
    pub pom_displacement: f32,
    #[serde(rename = "POMSelfShadowStrength", default)]
    pub pom_self_shadow_strength: f32,
}

impl AzRtti for WorldMaterialDataAsset {
    const NAME: &'static str = "WorldMaterialDataAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0C5DEBF7_4320_42AB_B77B_B7270D04206A);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
