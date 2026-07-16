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
pub struct ActionConditionIfStructureIsInValidLocation;

impl AzRtti for ActionConditionIfStructureIsInValidLocation {
    const NAME: &'static str = "ActionConditionIfStructureIsInValidLocation";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x8D52D33D_28E7_4C80_B2D9_6D17E77B55C5);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
