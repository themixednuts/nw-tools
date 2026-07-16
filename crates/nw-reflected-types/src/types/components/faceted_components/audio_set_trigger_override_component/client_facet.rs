use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{ClientFacet, TriggerOverridePair};
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    bevy_ecs::component::Component,
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AudioSetTriggerOverrideComponentClientFacet {
    #[serde(rename = "BaseClass1", default)]
    pub client_facet: ClientFacet,
    #[serde(rename = "m_audioTriggerOverrides", default)]
    pub audio_trigger_overrides: Vec<TriggerOverridePair>,
}

impl AzRtti for AudioSetTriggerOverrideComponentClientFacet {
    const NAME: &'static str = "AudioSetTriggerOverrideComponentClientFacet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xFD29133E_348C_4AAB_AB0B_C431471791A9);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x0643CDC7_B1C9_4721_92CE_7AC02E6175C9)];
}
