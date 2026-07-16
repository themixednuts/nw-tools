use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SlayerScriptLiteral;
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
pub struct TestSequenceEvent {
    #[serde(rename = "m_name", default)]
    pub name: SlayerScriptLiteral,
    #[serde(rename = "m_enterRefCount", default)]
    pub enter_ref_count: i32,
    #[serde(rename = "m_exitRefCount", default)]
    pub exit_ref_count: i32,
    #[serde(rename = "m_enterExitRefCount", default)]
    pub enter_exit_ref_count: i32,
}

impl AzRtti for TestSequenceEvent {
    const NAME: &'static str = "TestSequenceEvent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x30E7B94A_CD2D_4733_8054_3FBC00FD7223);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
