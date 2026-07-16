use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod gathering_action_data;

pub use self::gathering_action_data::GatheringActionData;

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
pub struct GatheringActionDatabase {
    #[serde(rename = "Gathering Actions", default)]
    pub gathering_actions: Vec<GatheringActionData>,
}

impl AzRtti for GatheringActionDatabase {
    const NAME: &'static str = "GatheringActionDatabase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9AC82655_BC8F_4165_AE2F_6D6F3D543D9A);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
