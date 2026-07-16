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
pub struct SlayerScriptLiteral {
    #[serde(rename = "m_crc", default)]
    pub crc: u32,
}

impl AzRtti for SlayerScriptLiteral {
    const NAME: &'static str = "SlayerScriptLiteral";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xF4F725DD_D22B_4DC1_8CC2_FD99E7B4CD66);
}
