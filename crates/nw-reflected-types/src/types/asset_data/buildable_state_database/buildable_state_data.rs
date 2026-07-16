use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::BuildableStateEnum;
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
pub struct BuildableStateData {
    #[serde(rename = "My State", default)]
    pub my_state: BuildableStateEnum,
    #[serde(rename = "Does Deteriorate", default)]
    pub does_deteriorate: bool,
    #[serde(rename = "Valid Transitions", default)]
    pub valid_transitions: Vec<BuildableStateEnum>,
}

impl AzRtti for BuildableStateData {
    const NAME: &'static str = "BuildableStateData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAAE18229_4474_4687_9E77_F33176719E9D);
}
