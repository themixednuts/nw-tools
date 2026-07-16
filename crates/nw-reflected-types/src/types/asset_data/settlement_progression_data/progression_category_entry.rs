use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ProgressionSpawnerEntry;
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
pub struct ProgressionCategoryEntry {
    #[serde(rename = "Settlement Progression Category", default)]
    pub settlement_progression_category: String,
    #[serde(rename = "Settlement Progression Entries", default)]
    pub settlement_progression_entries: Vec<ProgressionSpawnerEntry>,
}

impl AzRtti for ProgressionCategoryEntry {
    const NAME: &'static str = "ProgressionCategoryEntry";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE1766B2B_75FD_4EB2_AB13_0E5F343B7E68);
}
