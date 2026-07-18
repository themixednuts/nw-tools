use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{FacetedComponent, QueryShape};
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod client_facet;
pub mod server_facet;

pub use self::client_facet::HitVolumeComponentClientFacet;
pub use self::server_facet::HitVolumeComponentServerFacet;

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
pub struct HitVolumeComponent {
    #[serde(rename = "BaseClass1", default)]
    pub faceted_component: FacetedComponent,
    #[serde(rename = "m_center", default)]
    pub center: bevy_math::Vec3,
    #[serde(rename = "m_shape", default)]
    pub shape: Option<QueryShape>,
    #[serde(rename = "m_damageMult", default)]
    pub damage_mult: f32,
    #[serde(rename = "m_isHeadshot", default)]
    pub is_headshot: bool,
    #[serde(rename = "m_isLegshot", default)]
    pub is_legshot: bool,
    #[serde(rename = "m_volumeName", default)]
    pub volume_name: String,
    #[serde(rename = "m_strFilter", default)]
    pub str_filter: String,
    #[serde(rename = "m_targetBoneName", default)]
    pub target_bone_name: String,
    #[serde(rename = "m_hitCategory", default)]
    pub hit_category: String,
    #[serde(rename = "m_lightweightCharacterEntityId", default)]
    pub lightweight_character_entity_id: u64,
}

impl AzRtti for HitVolumeComponent {
    const NAME: &'static str = "HitVolumeComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6D3E998C_FC19_4C9D_B8F6_77C3FE985D29);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x65CD8F3E_73AA_43E9_8D9A_B5AE43F624F9)];
}
