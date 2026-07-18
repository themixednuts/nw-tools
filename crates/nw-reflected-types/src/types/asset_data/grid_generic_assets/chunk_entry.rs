use crate::az::asset::AssetId as AzAssetId;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::CellIndex;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct ChunkEntry {
    #[serde(rename = "cellIndex", default)]
    pub cell_index: CellIndex,
    #[serde(default)]
    pub size: u64,
    #[serde(rename = "spawnRadius", default)]
    pub spawn_radius: f32,
    #[serde(default)]
    pub layer: String,
    #[serde(rename = "worldPosition", default)]
    pub world_position: bevy_math::Vec3,
    #[serde(rename = "chunkType", default)]
    pub chunk_type: i32,
    #[serde(rename = "assetId", default)]
    pub asset_id: AzAssetId,
}

impl AzRtti for ChunkEntry {
    const NAME: &'static str = "ChunkEntry";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x92CA42B0_450A_49E0_8224_522E7DD9BC73);
}
