use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod chunk_entity_trace;
pub mod slice_data;

pub use self::chunk_entity_trace::ChunkEntityTrace;
pub use self::slice_data::SliceData;

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct ChunkTraceAsset {
    #[serde(rename = "TraceMap", default)]
    pub trace_map: std::collections::HashMap<u64, ChunkEntityTrace>,
    #[serde(rename = "TraceHierarchy", default)]
    pub trace_hierarchy: std::collections::HashMap<u64, u64>,
    #[serde(rename = "Slices", default)]
    pub slices: Vec<SliceData>,
    #[serde(rename = "Chunks", default)]
    pub chunks: Vec<String>,
    #[serde(rename = "LayerName", default)]
    pub layer_name: String,
    #[serde(rename = "RegionX", default)]
    pub region_x: i32,
    #[serde(rename = "RegionY", default)]
    pub region_y: i32,
}

impl AzRtti for ChunkTraceAsset {
    const NAME: &'static str = "ChunkTraceAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAEE40729_33DE_41A9_AF45_B175EFD09DF5);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
