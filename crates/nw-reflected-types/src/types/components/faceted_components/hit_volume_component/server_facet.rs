use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ServerFacet;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    bevy_ecs::component::Component,
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct HitVolumeComponentServerFacet {
    #[serde(rename = "BaseClass1", default)]
    pub server_facet: ServerFacet,
    #[serde(rename = "m_hitVolumeUpdateFrequency", default)]
    pub hit_volume_update_frequency: f32,
}

impl AzRtti for HitVolumeComponentServerFacet {
    const NAME: &'static str = "HitVolumeComponentServerFacet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6789290B_3BB1_4F88_A70B_7DB0CD9C6FAA);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x0392E589_5B61_47CC_835B_C3C254E76493)];
}
