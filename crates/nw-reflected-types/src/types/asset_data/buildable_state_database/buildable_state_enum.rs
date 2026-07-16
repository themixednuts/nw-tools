use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::BuildableState;
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
pub struct BuildableStateEnum {
    #[serde(rename = "m_enum", default)]
    pub enum_: BuildableState,
}

impl AzRtti for BuildableStateEnum {
    const NAME: &'static str = "BuildableStateEnum";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0A7DEDE3_F920_48C6_8544_7DB50B5FD808);
}
