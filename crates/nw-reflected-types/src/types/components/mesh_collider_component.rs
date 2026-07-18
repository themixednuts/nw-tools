use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
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
pub struct MeshColliderComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
}

impl AzRtti for MeshColliderComponent {
    const NAME: &'static str = "MeshColliderComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2D559EB0_F6FE_46E0_9FCE_E8F375177724);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
