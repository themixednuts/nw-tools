use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{DEPRECATEDCollisionType, FacetedComponent, GameRigidBodyConfig, QueryShape};

use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod client_facet;
pub mod game_rigid_body_server_facet_config;
pub mod server_facet;

pub use self::client_facet::GameRigidBodyComponentClientFacet;
pub use self::game_rigid_body_server_facet_config::GameRigidBodyServerFacetConfig;
pub use self::server_facet::GameRigidBodyComponentServerFacet;

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
pub struct GameRigidBodyComponent {
    #[serde(rename = "BaseClass1", default)]
    pub faceted_component: FacetedComponent,
    #[serde(rename = "m_center", default)]
    pub center: bevy_math::Vec3,
    #[serde(rename = "m_collisionType", default)]
    pub collision_type: DEPRECATEDCollisionType,
    #[serde(rename = "m_rnrAsset", default)]
    pub rnr_asset: AzAsset,
    #[serde(rename = "m_materialOverrideAsset", default)]
    pub material_override_asset: AzAsset,
    #[serde(rename = "m_collisionShape", default)]
    pub collision_shape: Option<QueryShape>,
    #[serde(rename = "m_shapeEntity", default)]
    pub shape_entity: u64,
    #[serde(rename = "m_setPrismAsset", default)]
    pub set_prism_asset: bool,
    #[serde(rename = "m_isDynamic", default)]
    pub is_dynamic: bool,
    #[serde(rename = "m_mass", default)]
    pub mass: f32,
    #[serde(rename = "m_linearDamping", default)]
    pub linear_damping: f32,
    #[serde(rename = "m_angularDamping", default)]
    pub angular_damping: f32,
    #[serde(rename = "m_sleepMinEnergy", default)]
    pub sleep_min_energy: f32,
    #[serde(rename = "m_interactWithTriggers", default)]
    pub interact_with_triggers: bool,
    #[serde(rename = "m_strFilter", default)]
    pub str_filter: String,
    #[serde(rename = "m_gameplayFlags", default)]
    pub gameplay_flags: Vec<i32>,
    #[serde(rename = "m_scaleShapes", default)]
    pub scale_shapes: bool,
    #[serde(rename = "m_deprecationWarning", default)]
    pub deprecation_warning: String,
    #[serde(rename = "m_convertToEditorGameRigidBodyComponent", default)]
    pub convert_to_editor_game_rigid_body_component: bool,
    #[serde(rename = "m_configuration", default)]
    pub configuration: GameRigidBodyConfig,
}

impl AzRtti for GameRigidBodyComponent {
    const NAME: &'static str = "GameRigidBodyComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x5BA2D1FC_DB9D_4B81_88C9_89787D5C19F1);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x65CD8F3E_73AA_43E9_8D9A_B5AE43F624F9)];
}
