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
pub struct ActionConditionIsEmotePreviewStopped {}

impl AzRtti for ActionConditionIsEmotePreviewStopped {
    const NAME: &'static str = "ActionConditionIsEmotePreviewStopped";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x40011F4C_66D7_4414_9D8F_A4D9B56555F5);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
