use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::TerritoryLandmarkType;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct TerritoryLandmarkData {
    #[serde(rename = "TerritoryIds", default)]
    pub territory_ids: Vec<u16>,
    #[serde(rename = "LandmarkType", default)]
    pub landmark_type: TerritoryLandmarkType,
    #[serde(rename = "LandmarkData", default)]
    pub landmark_data: String,
    #[serde(rename = "WorldPosition", default)]
    pub world_position: bevy_math::Vec3,
    #[serde(rename = "Radius", default)]
    pub radius: f32,
    #[serde(rename = "ActorId", default)]
    pub actor_id: AzUuid,
}

impl AzRtti for TerritoryLandmarkData {
    const NAME: &'static str = "TerritoryLandmarkData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6A3D88E0_E0B8_45BE_B4A3_6E1E690C626B);
}
