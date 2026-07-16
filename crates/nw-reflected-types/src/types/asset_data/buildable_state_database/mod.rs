use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod buildable_state_data;
pub mod buildable_state_enum;

pub use self::buildable_state_data::BuildableStateData;
pub use self::buildable_state_enum::BuildableStateEnum;

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
pub struct BuildableStateDatabase {
    #[serde(rename = "Buildable States", default)]
    pub buildable_states: Vec<BuildableStateData>,
}

impl AzRtti for BuildableStateDatabase {
    const NAME: &'static str = "BuildableStateDatabase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x78AA0C25_92B9_4F30_A3F7_C43980D46784);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
