use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct WaterNodeData {
    #[serde(rename = "Height", default)]
    pub height: f32,
    #[serde(rename = "FloorHeight", default)]
    pub floor_height: f32,
    #[serde(rename = "Flags", default)]
    pub flags: [u32; 1],
}

impl AzRtti for WaterNodeData {
    const NAME: &'static str = "WaterNodeData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x79BCCE0C_D451_47C0_B2A1_5CAD1D7313BD);
}
