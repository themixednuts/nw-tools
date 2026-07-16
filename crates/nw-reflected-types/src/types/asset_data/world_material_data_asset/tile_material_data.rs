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
pub struct TileMaterialData {
    #[serde(rename = "Tile X", default)]
    pub tile_x: i32,
    #[serde(rename = "Tile Y", default)]
    pub tile_y: i32,
    #[serde(rename = "Layers", default)]
    pub layers: AzAsset,
}

impl AzRtti for TileMaterialData {
    const NAME: &'static str = "TileMaterialData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7C65441F_6B36_444F_A722_BE103F85BFAE);
}
