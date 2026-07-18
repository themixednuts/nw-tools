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
pub struct ActionConditionIfMountDashToggle {}

impl AzRtti for ActionConditionIfMountDashToggle {
    const NAME: &'static str = "ActionConditionIfMountDashToggle";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x632D2D61_D639_4F8C_8FB1_74C865FA5FBB);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
