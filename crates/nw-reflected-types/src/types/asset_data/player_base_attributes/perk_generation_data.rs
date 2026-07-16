use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::PerkTierData;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct PerkGenerationData {
    #[serde(rename = "Perk Data Per Tier", default)]
    pub perk_data_per_tier: Vec<PerkTierData>,
    #[serde(rename = "Crafting Result Loot Bucket Id", default)]
    pub crafting_result_loot_bucket_id: AzCrc32,
    #[serde(rename = "Crafting Result Loot Bucket", default)]
    pub crafting_result_loot_bucket: String,
    #[serde(rename = "Roll Perk On Upgrade GS", default)]
    pub roll_perk_on_upgrade_gs: i32,
    #[serde(rename = "Roll Perk On Upgrade Tier", default)]
    pub roll_perk_on_upgrade_tier: i32,
    #[serde(rename = "Roll Perk On Upgrade Perk Count", default)]
    pub roll_perk_on_upgrade_perk_count: i32,
}

impl AzRtti for PerkGenerationData {
    const NAME: &'static str = "PerkGenerationData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xF9944C92_939C_4F11_B809_106EE60E48E9);
}
