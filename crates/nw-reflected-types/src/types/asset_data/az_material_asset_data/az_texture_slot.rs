use crate::az::asset::AssetId as AzAssetId;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::AzTextureSlotSettings;

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AzTextureSlot {
    #[serde(rename = "TextureMap", default)]
    pub texture_map: String,
    #[serde(rename = "TextureName", default)]
    pub texture_name: String,
    #[serde(rename = "TextureAssetId", default)]
    pub texture_asset_id: AzAssetId,
    #[serde(rename = "UTile", default)]
    pub u_tile: bool,
    #[serde(rename = "VTile", default)]
    pub v_tile: bool,
    #[serde(rename = "TextureType", default)]
    pub texture_type: u8,
    #[serde(rename = "Filter", default)]
    pub filter: i32,
    #[serde(rename = "RotType", default)]
    pub rot_type: i32,
    #[serde(rename = "TGType", default)]
    pub tg_type: i32,
    #[serde(rename = "TexGenProjected", default)]
    pub tex_gen_projected: bool,
    #[serde(rename = "TextureModifications", default)]
    pub texture_modifications: arrayvec::ArrayVec<AzTextureSlotSettings, 3>,
}

impl AzRtti for AzTextureSlot {
    const NAME: &'static str = "AzTextureSlot";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD4B5BEA1_F5F8_4BEA_88BF_984F0DD28CE4);
}
