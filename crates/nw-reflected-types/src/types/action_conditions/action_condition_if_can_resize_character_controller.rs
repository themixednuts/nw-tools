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
pub struct ActionConditionIfCanResizeCharacterController;

impl AzRtti for ActionConditionIfCanResizeCharacterController {
    const NAME: &'static str = "ActionConditionIfCanResizeCharacterController";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x5815F702_11EC_4439_AA20_6C5F9A4D3637);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
