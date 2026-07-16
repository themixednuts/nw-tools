use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct ItemRarityData {
    #[serde(rename = "Rarity Level Loc String", default)]
    pub rarity_level_loc_string: String,
    #[serde(rename = "Max Perk Count", default)]
    pub max_perk_count: i32,
    #[serde(rename = "Level Requirement Modifier", default)]
    pub level_requirement_modifier: i32,
}

impl AzRtti for ItemRarityData {
    const NAME: &'static str = "ItemRarityData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x70B5DE69_114F_41B5_993A_2249FDA496DE);
}
