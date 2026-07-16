use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SlayerScriptLiteral;
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
pub struct CastSpellTargeting {
    #[serde(rename = "m_sliceName", default)]
    pub slice_name: String,
    #[serde(rename = "m_spellName", default)]
    pub spell_name: SlayerScriptLiteral,
}

impl AzRtti for CastSpellTargeting {
    const NAME: &'static str = "CastSpellTargeting";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xFDC8FCCC_EAC6_4B1D_BA0A_7B068C5B2377);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
