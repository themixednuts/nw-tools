use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct StructureAttributeData {
    #[serde(rename = "Demolish Min Percent", default)]
    pub demolish_min_percent: f32,
    #[serde(rename = "Demolish Max Percent", default)]
    pub demolish_max_percent: f32,
    #[serde(rename = "Demolish Min Quantity", default)]
    pub demolish_min_quantity: i32,
}

impl AzRtti for StructureAttributeData {
    const NAME: &'static str = "StructureAttributeData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x1E5E78CF_C590_412A_853E_1EFD1CD11694);
}
