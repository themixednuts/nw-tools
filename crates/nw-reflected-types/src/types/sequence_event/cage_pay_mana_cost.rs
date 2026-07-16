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
pub struct CAGEPayManaCost {
    #[serde(rename = "m_costID", default)]
    pub cost_id: SlayerScriptLiteral,
    #[serde(rename = "m_offhandWeapon", default)]
    pub offhand_weapon: bool,
}

impl AzRtti for CAGEPayManaCost {
    const NAME: &'static str = "CAGEPayManaCost";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xF1F6C1FA_A347_46C9_B008_19844D0F8DE7);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
