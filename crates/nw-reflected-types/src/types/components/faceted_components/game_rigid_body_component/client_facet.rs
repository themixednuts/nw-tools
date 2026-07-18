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
pub struct GameRigidBodyComponentClientFacet {
    #[serde(rename = "BaseClass1", default)]
    pub client_facet: ClientFacet,
}

impl AzRtti for GameRigidBodyComponentClientFacet {
    const NAME: &'static str = "GameRigidBodyComponentClientFacet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4A3DF78E_C81D_4F7C_BFFA_CDD2A16B329E);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x0643CDC7_B1C9_4721_92CE_7AC02E6175C9)];
}
