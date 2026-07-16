use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct ValidGroupData {
    #[serde(rename = "Names", default)]
    pub names: Vec<String>,
    #[serde(rename = "Objectives", default)]
    pub objectives: Vec<String>,
    #[serde(rename = "IconPaths", default)]
    pub icon_paths: Vec<String>,
    #[serde(rename = "Colors", default)]
    pub colors: Vec<bevy_color::LinearRgba>,
}

impl AzRtti for ValidGroupData {
    const NAME: &'static str = "ValidGroupData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4F986681_3060_4A47_9A45_694A027E5F46);
}
