use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::CharacterDesc;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CharacterControllerConfig {
    #[serde(rename = "BaseClass1", default)]
    pub rock_n_roll_character_desc: CharacterDesc,
    #[serde(rename = "Shape Type", default)]
    pub shape_type: u32,
    #[serde(rename = "Shape Entity", default)]
    pub shape_entity: u64,
    #[serde(rename = "RnR Asset", default)]
    pub rn_r_asset: AzAsset,
    #[serde(rename = "Filter Name", default)]
    pub filter_name: String,
}

impl AzRtti for CharacterControllerConfig {
    const NAME: &'static str = "CharacterControllerConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xCC8AFEBD_5F7A_4E63_A472_FFC8591DB051);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xD03BD2CC_5E87_49A7_B490_47A6F6EBDC22)];
}
