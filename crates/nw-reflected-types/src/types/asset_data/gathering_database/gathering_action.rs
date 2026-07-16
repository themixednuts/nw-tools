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
pub struct GatheringAction {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Mannequin Tag", default)]
    pub mannequin_tag: String,
}

impl AzRtti for GatheringAction {
    const NAME: &'static str = "GatheringAction";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x5CFD353D_418D_4421_A207_2C748CFBDD16);
}
