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
pub struct SlayerScriptEditLiteral {
    #[serde(rename = "m_string", default)]
    pub string: i8,
    #[serde(rename = "m_crc", default)]
    pub crc: u32,
}

impl AzRtti for SlayerScriptEditLiteral {
    const NAME: &'static str = "SlayerScriptEditLiteral";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4CAC7A1B_5D32_4AEF_9722_7E2F5CB38635);
}
