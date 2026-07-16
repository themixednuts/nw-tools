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
pub struct ActionConditionNot;

impl AzRtti for ActionConditionNot {
    const NAME: &'static str = "ActionConditionNot";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xCBEE0870_DD5A_4669_9B8B_ACD16397DE0F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x89837E1A_40E2_4066_B017_4E17E3BC8BA6)];
}
