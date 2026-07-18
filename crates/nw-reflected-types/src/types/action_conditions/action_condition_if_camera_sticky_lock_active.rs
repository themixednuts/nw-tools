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
pub struct ActionConditionIfCameraStickyLockActive {}

impl AzRtti for ActionConditionIfCameraStickyLockActive {
    const NAME: &'static str = "ActionConditionIfCameraStickyLockActive";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xCB7F0B2C_23ED_44D9_810A_A72EE7FF4349);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
