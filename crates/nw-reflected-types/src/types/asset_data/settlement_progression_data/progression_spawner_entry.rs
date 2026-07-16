use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SimpleAssetReferenceTextureAsset;
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
pub struct ProgressionSpawnerEntry {
    #[serde(rename = "Settlement Progression Category Level", default)]
    pub settlement_progression_category_level: i32,
    #[serde(rename = "Slice", default)]
    pub slice: AzAsset,
    #[serde(rename = "Alternate Slice", default)]
    pub alternate_slice: AzAsset,
    #[serde(rename = "Display LocString", default)]
    pub display_loc_string: String,
    #[serde(rename = "Icon", default)]
    pub icon: SimpleAssetReferenceTextureAsset,
}

impl AzRtti for ProgressionSpawnerEntry {
    const NAME: &'static str = "ProgressionSpawnerEntry";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD91778A1_A110_46E4_8B9A_30402D8996D6);
}
