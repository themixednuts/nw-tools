use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::TerritoryBonus;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct TerritoryEntryData {
    #[serde(rename = "Territory Bonus Id", default)]
    pub territory_bonus_id: TerritoryBonus,
    #[serde(rename = "Progression Point Id", default)]
    pub progression_point_id: String,
    #[serde(rename = "Initial Bonus", default)]
    pub initial_bonus: f32,
    #[serde(rename = "Reduction Modifier", default)]
    pub reduction_modifier: f32,
    #[serde(rename = "Min Value", default)]
    pub min_value: f32,
}

impl AzRtti for TerritoryEntryData {
    const NAME: &'static str = "TerritoryEntryData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xB565FDCA_4CF3_46DC_B2B8_506829883905);
}
