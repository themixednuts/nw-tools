use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod crest_color_data;
pub mod crest_data;

pub use self::crest_color_data::CrestColorData;
pub use self::crest_data::CrestData;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CrestDatabase {
    #[serde(rename = "Background Data", default)]
    pub background_data: Vec<CrestData>,
    #[serde(rename = "Background Colors", default)]
    pub background_colors: Vec<CrestColorData>,
    #[serde(rename = "Foreground Data", default)]
    pub foreground_data: Vec<CrestData>,
    #[serde(rename = "Foreground Colors", default)]
    pub foreground_colors: Vec<CrestColorData>,
    #[serde(default)]
    pub descriptions: Vec<String>,
    #[serde(default)]
    pub origins: Vec<String>,
    #[serde(default)]
    pub missions: Vec<String>,
    #[serde(default)]
    pub groups: Vec<String>,
}

impl AzRtti for CrestDatabase {
    const NAME: &'static str = "CrestDatabase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2DE9E46E_703F_4708_990F_C45A6D08EDB8);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
