use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{GameRigidBodyServerFacetConfig, ServerFacet};
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
pub struct GameRigidBodyComponentServerFacet {
    #[serde(rename = "BaseClass1", default)]
    pub server_facet: ServerFacet,
    #[serde(rename = "m_logGridInfo", default)]
    pub log_grid_info: bool,
    #[serde(rename = "m_configuration", default)]
    pub configuration: GameRigidBodyServerFacetConfig,
}

impl AzRtti for GameRigidBodyComponentServerFacet {
    const NAME: &'static str = "GameRigidBodyComponentServerFacet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xF57E125F_FF7A_43E9_9C01_8069C35632ED);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x0392E589_5B61_47CC_835B_C3C254E76493)];
}
