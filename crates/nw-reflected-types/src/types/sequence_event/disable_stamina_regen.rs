use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SequenceEventOptions;
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
pub struct DisableStaminaRegen {
    #[serde(rename = "m_optionOnEnter", default)]
    pub option_on_enter: SequenceEventOptions,
    #[serde(rename = "m_optionOnExit", default)]
    pub option_on_exit: SequenceEventOptions,
}

impl AzRtti for DisableStaminaRegen {
    const NAME: &'static str = "DisableStaminaRegen";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x95974A63_FA1A_4F44_9771_BF222E7AAD8D);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
