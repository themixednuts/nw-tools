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
pub struct ActionConditionIfVelocityWithinInputAngle {}

impl AzRtti for ActionConditionIfVelocityWithinInputAngle {
    const NAME: &'static str = "ActionConditionIfVelocityWithinInputAngle";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x51FEF494_D906_44A9_A709_23646BF5ADA1);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
