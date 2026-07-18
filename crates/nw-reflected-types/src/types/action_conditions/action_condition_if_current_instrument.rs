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
pub struct ActionConditionIfCurrentInstrument {}

impl AzRtti for ActionConditionIfCurrentInstrument {
    const NAME: &'static str = "ActionConditionIfCurrentInstrument";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7A2D42B5_BF7E_423E_BD08_545CF8FB0C1E);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
