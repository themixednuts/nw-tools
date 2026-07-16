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
pub struct Transition {
    #[serde(rename = "m_input", default)]
    pub input: i32,
    #[serde(rename = "m_backTicks", default)]
    pub back_ticks: f32,
    #[serde(rename = "m_transitionKey", default)]
    pub transition_key: SlayerScriptLiteral,
    #[serde(rename = "m_sequence", default)]
    pub sequence: i32,
    #[serde(rename = "m_aliasId", default)]
    pub alias_id: i32,
    #[serde(rename = "m_layer", default)]
    pub layer: i32,
    #[serde(rename = "m_newState", default)]
    pub new_state: i32,
    #[serde(rename = "m_blendFrames", default)]
    pub blend_frames: f32,
    #[serde(rename = "m_skipFrames", default)]
    pub skip_frames: f32,
    #[serde(rename = "m_holdTimeRequired", default)]
    pub hold_time_required: f32,
    #[serde(rename = "m_forceEnterState", default)]
    pub force_enter_state: bool,
    #[serde(rename = "m_singleActivationLockout", default)]
    pub single_activation_lockout: bool,
    #[serde(rename = "m_singleActivationKeyLockout", default)]
    pub single_activation_key_lockout: bool,
    #[serde(rename = "m_isBuffered", default)]
    pub is_buffered: bool,
}

impl AzRtti for Transition {
    const NAME: &'static str = "Transition";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7C95FB51_963B_402B_AD66_7898ACC851CA);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
