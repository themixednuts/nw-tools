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
pub struct HideAttachment {
    #[serde(rename = "m_attachmentName", default)]
    pub attachment_name: SlayerScriptLiteral,
    #[serde(rename = "m_forceVisibleOnExit", default)]
    pub force_visible_on_exit: bool,
}

impl AzRtti for HideAttachment {
    const NAME: &'static str = "HideAttachment";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x46D028E9_1504_49BC_A86E_45171BDF04CD);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
