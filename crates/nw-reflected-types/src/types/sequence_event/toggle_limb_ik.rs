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
pub struct ToggleLimbIK {
    #[serde(rename = "m_enableOnEnter", default)]
    pub enable_on_enter: bool,
    #[serde(rename = "m_disableOnEnter", default)]
    pub disable_on_enter: bool,
    #[serde(rename = "m_enableOnExit", default)]
    pub enable_on_exit: bool,
    #[serde(rename = "m_disableOnExit", default)]
    pub disable_on_exit: bool,
}

impl AzRtti for ToggleLimbIK {
    const NAME: &'static str = "ToggleLimbIK";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x5B878AF0_C0FF_4CA6_B8E4_AA5F77F8273B);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
