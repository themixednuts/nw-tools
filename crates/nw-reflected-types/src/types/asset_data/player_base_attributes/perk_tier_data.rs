use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct PerkTierData {
    #[serde(rename = "Max Perk Channel", default)]
    pub max_perk_channel: i32,
    #[serde(rename = "Gem Slot Probability", default)]
    pub gem_slot_probability: f32,
    #[serde(rename = "Attribute Perk Probability", default)]
    pub attribute_perk_probability: f32,
    #[serde(rename = "General Gear Score Perk Count", default)]
    pub general_gear_score_perk_count: std::collections::BTreeMap<i32, Vec<(i32, i32)>>,
    #[serde(rename = "Crafting Gear Score Perk Count", default)]
    pub crafting_gear_score_perk_count: std::collections::BTreeMap<i32, Vec<(i32, i32)>>,
    #[serde(rename = "Attribute Perk Bucket", default)]
    pub attribute_perk_bucket: String,
    #[serde(rename = "Attribute Perk Bucket Id", default)]
    pub attribute_perk_bucket_id: AzCrc32,
}

impl AzRtti for PerkTierData {
    const NAME: &'static str = "PerkTierData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x1C7129EC_C7B6_471C_A6FF_278A2B3205A8);
}
