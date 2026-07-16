use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{SimpleAssetReferenceSkinAsset, SimpleAssetReferenceTextureAsset};
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
pub struct CharacterCreationData {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Skin Tone Database Style Sheet", default)]
    pub skin_tone_database_style_sheet: AzAsset,
    #[serde(rename = "Hair Style Database Style Sheet", default)]
    pub hair_style_database_style_sheet: AzAsset,
    #[serde(rename = "Facial Hair Database Style Sheet", default)]
    pub facial_hair_database_style_sheet: AzAsset,
    #[serde(rename = "Eye Color Database Style Sheet", default)]
    pub eye_color_database_style_sheet: AzAsset,
    #[serde(rename = "Face Mark Database Style Sheet", default)]
    pub face_mark_database_style_sheet: AzAsset,
    #[serde(rename = "Scar Database Style Sheet", default)]
    pub scar_database_style_sheet: AzAsset,
    #[serde(rename = "Tattoo Database Style Sheet", default)]
    pub tattoo_database_style_sheet: AzAsset,
    #[serde(rename = "Head Skin", default)]
    pub head_skin: SimpleAssetReferenceSkinAsset,
    #[serde(rename = "Default Skin Tone", default)]
    pub default_skin_tone: String,
    #[serde(rename = "UI Selection Image", default)]
    pub ui_selection_image: SimpleAssetReferenceTextureAsset,
}

impl AzRtti for CharacterCreationData {
    const NAME: &'static str = "CharacterCreationData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD1E63EA6_8BD3_4381_B564_DBE47335DA44);
}
