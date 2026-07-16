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
pub struct GatheringActionData {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Mannequin Tag", default)]
    pub mannequin_tag: String,
}

impl AzRtti for GatheringActionData {
    const NAME: &'static str = "GatheringActionData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA6B5258C_2984_4225_88E9_B66813457286);
}
