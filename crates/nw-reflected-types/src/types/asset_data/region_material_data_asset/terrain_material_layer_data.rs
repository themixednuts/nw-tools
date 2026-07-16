use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct TerrainMaterialLayerData {
    #[serde(rename = "Material", default)]
    pub material: AzAsset,
    #[serde(rename = "SplatMap", default)]
    pub splat_map: AzAsset,
    #[serde(rename = "AffectedTiles", default)]
    pub affected_tiles: u64,
    #[serde(rename = "Priority", default)]
    pub priority: u8,
}

impl AzRtti for TerrainMaterialLayerData {
    const NAME: &'static str = "TerrainMaterialLayerData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x180454CF_AD7E_440B_91F9_A071574422F4);
}
