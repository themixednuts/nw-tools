use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct EncounterEntry {
    #[serde(rename = "TerritoryIds", default)]
    pub territory_ids: Vec<u16>,
    #[serde(rename = "EncounterId", default)]
    pub encounter_id: String,
    #[serde(rename = "EncounterSuccessGameEventId", default)]
    pub encounter_success_game_event_id: AzCrc32,
    #[serde(rename = "WorldPosition", default)]
    pub world_position: bevy_math::Vec3,
}

impl AzRtti for EncounterEntry {
    const NAME: &'static str = "EncounterEntry";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xFDB45D86_E9C7_4345_AA82_A3DAAF80F58C);
}
