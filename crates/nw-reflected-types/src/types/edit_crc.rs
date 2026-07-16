use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
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
pub struct EditCrc {
    #[serde(rename = "m_valueStr", default)]
    pub value_str: String,
    #[serde(rename = "m_valueCrc", default)]
    pub value_crc: AzCrc32,
}

impl AzRtti for EditCrc {
    const NAME: &'static str = "EditCrc";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9A339DE9_0D6E_4708_922F_F46AF04370E9);
}
