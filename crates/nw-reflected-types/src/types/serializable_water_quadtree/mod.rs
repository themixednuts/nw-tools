use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod water_node_data;

pub use self::water_node_data::WaterNodeData;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct SerializableWaterQuadtree {
    #[serde(rename = "regionSize", default)]
    pub region_size: i32,
    #[serde(rename = "quadtreeNodes", default)]
    pub quadtree_nodes: Vec<WaterNodeData>,
}

impl AzRtti for SerializableWaterQuadtree {
    const NAME: &'static str = "SerializableWaterQuadtree";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x23082A77_84B8_423E_B4CD_F601AA5D1D44);
}
