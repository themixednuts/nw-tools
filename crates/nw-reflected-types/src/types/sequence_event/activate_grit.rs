use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{SequenceEventOptions, SlayerScriptLiteral};
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
pub struct ActivateGrit {
    #[serde(rename = "m_optionOnEnter", default)]
    pub option_on_enter: SequenceEventOptions,
    #[serde(rename = "m_optionOnExit", default)]
    pub option_on_exit: SequenceEventOptions,
    #[serde(rename = "m_setNoReactionOnEnter", default)]
    pub set_no_reaction_on_enter: SequenceEventOptions,
    #[serde(rename = "m_setNoReactionOnExit", default)]
    pub set_no_reaction_on_exit: SequenceEventOptions,
    #[serde(rename = "m_damageTableRow", default)]
    pub damage_table_row: SlayerScriptLiteral,
}

impl AzRtti for ActivateGrit {
    const NAME: &'static str = "ActivateGrit";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x43DB38B0_3C38_4E73_BC29_662EFDF5405F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
