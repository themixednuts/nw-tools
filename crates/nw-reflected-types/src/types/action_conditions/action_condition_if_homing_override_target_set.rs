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
pub struct ActionConditionIfHomingOverrideTargetSet;

impl AzRtti for ActionConditionIfHomingOverrideTargetSet {
    const NAME: &'static str = "ActionConditionIfHomingOverrideTargetSet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x07B2B873_C2AE_4579_B0AC_3E71C9A9411C);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
