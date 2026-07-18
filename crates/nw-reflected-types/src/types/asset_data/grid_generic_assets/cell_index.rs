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
pub struct CellIndex {
    #[serde(default)]
    pub x: u64,
    #[serde(default)]
    pub y: u64,
    #[serde(default)]
    pub z: u64,
}

impl AzRtti for CellIndex {
    const NAME: &'static str = "CellIndex";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x452A5517_CE93_4914_AC36_99D09742B552);
}
