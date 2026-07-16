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
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AudioAreaEnvironmentComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Broad-phase Trigger Area entity", default)]
    pub broad_phase_trigger_area_entity: u64,
    #[serde(rename = "Environment name", default)]
    pub environment_name: String,
    #[serde(rename = "Environment fade distance", default)]
    pub environment_fade_distance: f32,
}

impl AzRtti for AudioAreaEnvironmentComponent {
    const NAME: &'static str = "AudioAreaEnvironmentComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x52300012_FFCD_4559_9479_20F463940320);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
