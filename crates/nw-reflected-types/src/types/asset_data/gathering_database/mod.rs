use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod gathering_action;
pub mod gathering_data;
pub mod gathering_type_data;

pub use self::gathering_action::GatheringAction;
pub use self::gathering_data::GatheringData;
pub use self::gathering_type_data::GatheringTypeData;

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
pub struct GatheringDatabase {
    #[serde(rename = "Gathering Data", default)]
    pub gathering_data: GatheringData,
}

impl AzRtti for GatheringDatabase {
    const NAME: &'static str = "GatheringDatabase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x1EF311CC_A16E_426D_9763_A40473495330);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
