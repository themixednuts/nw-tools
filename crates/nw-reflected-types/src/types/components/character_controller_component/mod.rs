use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod character_desc;

pub use self::character_desc::CharacterControllerConfig;

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
pub struct CharacterControllerComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Configuration", default)]
    pub configuration: CharacterControllerConfig,
}

impl AzRtti for CharacterControllerComponent {
    const NAME: &'static str = "CharacterControllerComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE9D84B77_422A_4DCB_9EFF_708120B1B1A0);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
