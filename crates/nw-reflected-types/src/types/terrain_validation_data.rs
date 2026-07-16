use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct TerrainValidationData {
    #[serde(rename = "m_aboveMinPointValidThreshold", default)]
    pub above_min_point_valid_threshold: f32,
    #[serde(rename = "m_belowMinPointValidThreshold", default)]
    pub below_min_point_valid_threshold: f32,
}

impl AzRtti for TerrainValidationData {
    const NAME: &'static str = "TerrainValidationData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x28C197B6_642F_418E_B5BC_A7A363B13662);
}
