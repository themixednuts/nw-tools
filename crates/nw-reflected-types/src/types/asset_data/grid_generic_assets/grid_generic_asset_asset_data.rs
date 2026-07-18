use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ChunkEntry;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct GridGenericAssetAssetData {
    #[serde(rename = "Chunks", default)]
    pub chunks: Vec<ChunkEntry>,
}

impl AzRtti for GridGenericAssetAssetData {
    const NAME: &'static str = "GridGenericAsset<AssetData >";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAC608BE6_77F3_5AF5_A7A9_607621389D91);
}
