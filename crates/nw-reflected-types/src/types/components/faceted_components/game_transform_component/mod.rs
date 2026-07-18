use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::FacetedComponent;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod client_facet;
pub mod server_facet;

pub use self::client_facet::GameTransformComponentClientFacet;
pub use self::server_facet::GameTransformComponentServerFacet;

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
pub struct GameTransformComponent {
    #[serde(rename = "BaseClass1", default)]
    pub faceted_component: FacetedComponent,
    #[serde(rename = "m_worldTM", default)]
    pub world_tm: bevy_transform::components::Transform,
    #[serde(rename = "m_parentId", default)]
    pub parent_id: u64,
    #[serde(rename = "m_localTM", default)]
    pub local_tm: bevy_transform::components::Transform,
    #[serde(rename = "m_onNewParentKeepWorldTM", default)]
    pub on_new_parent_keep_world_tm: bool,
    #[serde(rename = "m_isStatic", default)]
    pub is_static: bool,
}

impl AzRtti for GameTransformComponent {
    const NAME: &'static str = "GameTransformComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x484AE67D_ABD0_4D9C_B2C8_9BB0EEC900E0);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x65CD8F3E_73AA_43E9_8D9A_B5AE43F624F9)];
}
