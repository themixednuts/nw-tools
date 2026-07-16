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
pub struct ActionConditionGroup;

impl AzRtti for ActionConditionGroup {
    const NAME: &'static str = "ActionConditionGroup";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xF1A34B9B_0814_402C_BAE5_2EC9D539BC71);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x89837E1A_40E2_4066_B017_4E17E3BC8BA6)];
}
