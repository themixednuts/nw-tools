use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    bevy_ecs::component::Component,
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
#[reflect(Component, Serialize, Deserialize)]
pub struct AudioTriggerComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Play Trigger", default)]
    pub play_trigger: String,
    #[serde(rename = "Stop Trigger", default)]
    pub stop_trigger: String,
    #[serde(rename = "Obstruction Type", default)]
    pub obstruction_type: u32,
    #[serde(rename = "Plays Immediately", default)]
    pub plays_immediately: bool,
    #[serde(rename = "Send Finished Event", default)]
    pub send_finished_event: bool,
    #[serde(rename = "VariationComponent Linked", default)]
    pub variation_component_linked: bool,
    #[serde(rename = "Audio Plays Out On Deactivate", default)]
    pub audio_plays_out_on_deactivate: bool,
    #[serde(rename = "Unload Preload On Completion", default)]
    pub unload_preload_on_completion: bool,
}

impl AzRtti for AudioTriggerComponent {
    const NAME: &'static str = "AudioTriggerComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x8CBBB54B_7435_4D33_844D_E7F201BD581A);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
