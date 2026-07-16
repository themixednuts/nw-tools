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
pub struct RunFxScript {
    #[serde(rename = "m_scriptToRun", default)]
    pub script_to_run: SlayerScriptLiteral,
    #[serde(rename = "m_stopOnExit", default)]
    pub stop_on_exit: bool,
}

impl AzRtti for RunFxScript {
    const NAME: &'static str = "RunFxScript";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x84DC167E_A64B_4CFC_AB2F_F78BF5CD5F5A);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
