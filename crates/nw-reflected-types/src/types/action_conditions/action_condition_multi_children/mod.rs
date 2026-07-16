use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod action_condition_and;
pub mod action_condition_or;
pub mod action_condition_xor;

pub use self::action_condition_and::ActionConditionAnd;
pub use self::action_condition_or::ActionConditionOr;
pub use self::action_condition_xor::ActionConditionXor;

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
pub struct ActionConditionMultiChild;

impl AzRtti for ActionConditionMultiChild {
    const NAME: &'static str = "ActionConditionMultiChild";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEAAF81BC_EF4C_470E_AE8E_0EAAA7FE501A);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
