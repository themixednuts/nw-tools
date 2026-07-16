use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{GatheringAction, GatheringTypeData};
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
pub struct GatheringData {
    #[serde(rename = "Gathering Types", default)]
    pub gathering_types: Vec<GatheringTypeData>,
    #[serde(rename = "Gathering Actions", default)]
    pub gathering_actions: Vec<GatheringAction>,
    #[serde(rename = "Required Water Gathering Type", default)]
    pub required_water_gathering_type: String,
    #[serde(rename = "None Gathering Type", default)]
    pub none_gathering_type: String,
}

impl AzRtti for GatheringData {
    const NAME: &'static str = "GatheringData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x579ABCC6_EC1E_4157_ABC5_2569C7624B0A);
}
