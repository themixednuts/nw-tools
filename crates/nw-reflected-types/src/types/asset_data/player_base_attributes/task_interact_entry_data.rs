use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::TerritoryLandmarkType;
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
pub struct TaskInteractEntryData {
    #[serde(rename = "Interact Tag", default)]
    pub interact_tag: String,
    #[serde(rename = "Landmark Type", default)]
    pub landmark_type: TerritoryLandmarkType,
    #[serde(rename = "Landmark Data", default)]
    pub landmark_data: Vec<String>,
}

impl AzRtti for TaskInteractEntryData {
    const NAME: &'static str = "TaskInteractEntryData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6D541438_BA60_46E4_A4C5_9B6BE645275B);
}
