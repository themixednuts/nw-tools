use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod character_creation_data;
pub mod default_appearance_data;

pub use self::character_creation_data::CharacterCreationData;
pub use self::default_appearance_data::DefaultAppearanceData;

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
pub struct CharacterCreationDatabase {
    #[serde(rename = "Gender", default)]
    pub gender: String,
    #[serde(rename = "Default Idle Animation Name", default)]
    pub default_idle_animation_name: String,
    #[serde(rename = "Races", default)]
    pub races: Vec<CharacterCreationData>,
    #[serde(rename = "Default Appearance Data", default)]
    pub default_appearance_data: std::collections::BTreeMap<String, DefaultAppearanceData>,
    #[serde(rename = "Creation Screen Equipment Data", default)]
    pub creation_screen_equipment_data: std::collections::BTreeMap<String, String>,
}

impl AzRtti for CharacterCreationDatabase {
    const NAME: &'static str = "CharacterCreationDatabase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA12F9BAB_1706_4DED_B3C5_70913BE02D1F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
