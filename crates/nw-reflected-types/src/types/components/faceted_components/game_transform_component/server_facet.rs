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
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct GameTransformComponentServerFacet {
    #[serde(rename = "BaseClass1", default)]
    pub server_facet: ServerFacet,
}

impl AzRtti for GameTransformComponentServerFacet {
    const NAME: &'static str = "GameTransformComponentServerFacet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9D091F11_1CF1_4859_962B_BD3C182C23AA);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x0392E589_5B61_47CC_835B_C3C254E76493)];
}
