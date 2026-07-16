use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::FacetedComponent;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod client_facet;
pub mod server_facet;
pub mod trigger_override_pair;

pub use self::client_facet::AudioSetTriggerOverrideComponentClientFacet;
pub use self::server_facet::AudioSetTriggerOverrideComponentServerFacet;
pub use self::trigger_override_pair::TriggerOverridePair;

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
pub struct AudioSetTriggerOverrideComponent {
    #[serde(rename = "BaseClass1", default)]
    pub faceted_component: FacetedComponent,
}

impl AzRtti for AudioSetTriggerOverrideComponent {
    const NAME: &'static str = "AudioSetTriggerOverrideComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7D46F849_F35D_4FF6_848F_5764C87AFDD8);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x65CD8F3E_73AA_43E9_8D9A_B5AE43F624F9)];
}
