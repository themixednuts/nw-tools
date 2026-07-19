use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod az_lighting_params;
pub mod az_material_layer;
pub mod az_texture_slot;
pub mod az_texture_slot_settings;

pub use self::az_lighting_params::AzLightingParams;
pub use self::az_material_layer::AzMaterialLayer;
pub use self::az_texture_slot::AzTextureSlot;
pub use self::az_texture_slot_settings::AzTextureSlotSettings;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct AzMaterialAssetData {
    #[serde(rename = "MaterialName", default)]
    pub material_name: String,
    #[serde(rename = "Flags", default)]
    pub flags: i32,
    #[serde(rename = "ShaderName", default)]
    pub shader_name: String,
    #[serde(rename = "MaskGenFX", default)]
    pub mask_gen_fx: u64,
    #[serde(rename = "GenMaskStr", default)]
    pub gen_mask_str: String,
    #[serde(rename = "SurfaceType", default)]
    pub surface_type: String,
    #[serde(rename = "LightingParams", default)]
    pub lighting_params: AzLightingParams,
    #[reflect(ignore, clone)]
    #[serde(rename = "Textures", default)]
    pub textures: Vec<AzTextureSlot>,
    #[serde(rename = "MaterialLinkName", default)]
    pub material_link_name: String,
    #[serde(rename = "PropagationFlags", default)]
    pub propagation_flags: i32,
    #[serde(rename = "PublicParams", default)]
    pub public_params: Vec<(String, String)>,
    #[serde(rename = "MaterialLayers", default)]
    pub material_layers: Vec<AzMaterialLayer>,
}

impl AzRtti for AzMaterialAssetData {
    const NAME: &'static str = "AzMaterialAssetData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x442FE283_A905_41AA_83B1_3847E3327B13);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
