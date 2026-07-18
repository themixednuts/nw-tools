use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ClientFacet;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    bevy_ecs::component::Component,
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
#[reflect(Component, Serialize, Deserialize)]
pub struct HitVolumeComponentClientFacet {
    #[serde(rename = "BaseClass1", default)]
    pub client_facet: ClientFacet,
}

impl AzRtti for HitVolumeComponentClientFacet {
    const NAME: &'static str = "HitVolumeComponentClientFacet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7C3A4D83_E2E4_4692_8828_C2D5BFF843D3);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x0643CDC7_B1C9_4721_92CE_7AC02E6175C9)];
}
