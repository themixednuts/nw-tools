use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct WarColorData {
    #[serde(rename = "War Colors Map", default)]
    pub war_colors_map: std::collections::BTreeMap<i32, bevy_color::LinearRgba>,
}

impl AzRtti for WarColorData {
    const NAME: &'static str = "WarColorData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x15829599_0D65_4FB3_ABF7_10830776530D);
}
