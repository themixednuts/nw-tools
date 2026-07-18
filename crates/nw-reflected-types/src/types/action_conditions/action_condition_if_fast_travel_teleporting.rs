use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
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
pub struct ActionConditionIfFastTravelTeleporting {}

impl AzRtti for ActionConditionIfFastTravelTeleporting {
    const NAME: &'static str = "ActionConditionIfFastTravelTeleporting";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2FEE3D94_B533_4D2F_AA68_FECA55DDBB07);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
