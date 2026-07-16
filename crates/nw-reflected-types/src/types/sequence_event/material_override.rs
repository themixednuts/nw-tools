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
pub struct MaterialOverride199A67B1 {
    #[serde(rename = "m_overrideName", default)]
    pub override_name: SlayerScriptLiteral,
    #[serde(rename = "m_stopOnExit", default)]
    pub stop_on_exit: bool,
    #[serde(rename = "m_stopOnEnterOverrideName", default)]
    pub stop_on_enter_override_name: SlayerScriptLiteral,
}

impl AzRtti for MaterialOverride199A67B1 {
    const NAME: &'static str = "MaterialOverride";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x199A67B1_C9A5_45C6_8CE4_7C7298AD6EE1);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
