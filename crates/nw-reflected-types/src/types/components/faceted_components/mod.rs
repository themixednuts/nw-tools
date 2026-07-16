use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod audio_set_trigger_override_component;
pub mod material_override_component;

pub use self::audio_set_trigger_override_component::{
    AudioSetTriggerOverrideComponent, AudioSetTriggerOverrideComponentClientFacet,
    AudioSetTriggerOverrideComponentServerFacet, TriggerOverridePair,
};

pub use self::material_override_component::MaterialOverrideInfo;

#[derive(
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
#[reflect(Serialize, Deserialize)]
pub struct Facet;

impl AzRtti for Facet {
    const NAME: &'static str = "Facet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9469C437_6529_489D_8CF8_63EEAB723A79);
}

#[derive(
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
#[reflect(Serialize, Deserialize)]
pub struct ClientFacet {
    #[serde(rename = "BaseClass1", default)]
    pub facet: Facet,
}

impl AzRtti for ClientFacet {
    const NAME: &'static str = "ClientFacet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0643CDC7_B1C9_4721_92CE_7AC02E6175C9);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9469C437_6529_489D_8CF8_63EEAB723A79)];
}

#[derive(
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
#[reflect(Serialize, Deserialize)]
pub struct ServerFacet {
    #[serde(rename = "BaseClass1", default)]
    pub facet: Facet,
}

impl AzRtti for ServerFacet {
    const NAME: &'static str = "ServerFacet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0392E589_5B61_47CC_835B_C3C254E76493);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9469C437_6529_489D_8CF8_63EEAB723A79)];
}

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
pub struct FacetedComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "m_clientFacetPtr", default)]
    pub client_facet_ptr: ClientFacet,
    #[serde(rename = "m_serverFacetPtr", default)]
    pub server_facet_ptr: ServerFacet,
    #[serde(rename = "m_replicationIndex", default)]
    pub replication_index: u32,
}

impl AzRtti for FacetedComponent {
    const NAME: &'static str = "FacetedComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x65CD8F3E_73AA_43E9_8D9A_B5AE43F624F9);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
