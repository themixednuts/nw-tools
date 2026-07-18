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
pub struct GameTransformComponentClientFacet {
    #[serde(rename = "BaseClass1", default)]
    pub client_facet: ClientFacet,
}

impl AzRtti for GameTransformComponentClientFacet {
    const NAME: &'static str = "GameTransformComponentClientFacet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xADF97E47_BA34_44D2_9A51_6CBA6C8A51DD);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x0643CDC7_B1C9_4721_92CE_7AC02E6175C9)];
}
