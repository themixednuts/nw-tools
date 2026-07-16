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
pub struct ActionConditionIfOwnershipMessageReceived;

impl AzRtti for ActionConditionIfOwnershipMessageReceived {
    const NAME: &'static str = "ActionConditionIfOwnershipMessageReceived";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x62292AA5_3A1C_44F0_94CE_A0F72F3434E4);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
