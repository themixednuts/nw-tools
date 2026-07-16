use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod progression_category_entry;
pub mod progression_spawner_entry;

pub use self::progression_category_entry::ProgressionCategoryEntry;
pub use self::progression_spawner_entry::ProgressionSpawnerEntry;

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
pub struct SettlementProgressionData {
    #[serde(rename = "Settlement Progression Categories", default)]
    pub settlement_progression_categories: Vec<ProgressionCategoryEntry>,
}

impl AzRtti for SettlementProgressionData {
    const NAME: &'static str = "SettlementProgressionData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0543759C_4CF0_4EBA_B0DD_F0F020B480B3);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
