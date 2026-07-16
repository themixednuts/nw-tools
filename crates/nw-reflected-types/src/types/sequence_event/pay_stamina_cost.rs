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
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct PayStaminaCost {
    #[serde(rename = "m_costID", default)]
    pub cost_id: SlayerScriptLiteral,
    #[serde(rename = "m_costMultiplier", default)]
    pub cost_multiplier: f32,
    #[serde(rename = "m_disableRegenWhileActive", default)]
    pub disable_regen_while_active: bool,
    #[serde(rename = "m_payOverTime", default)]
    pub pay_over_time: bool,
}

impl AzRtti for PayStaminaCost {
    const NAME: &'static str = "PayStaminaCost";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEDE1A0AD_CC2E_45BC_B882_60027BC044BF);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
