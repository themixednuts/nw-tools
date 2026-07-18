use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod cry_player_physics_configuration;
pub mod player_dimensions;
pub mod player_dynamics;

pub use self::cry_player_physics_configuration::CryPlayerPhysicsConfiguration;
pub use self::player_dimensions::PlayerDimensions;
pub use self::player_dynamics::PlayerDynamics;

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
pub struct CharacterPhysicsComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Configuration", default)]
    pub configuration: CryPlayerPhysicsConfiguration,
}

impl AzRtti for CharacterPhysicsComponent {
    const NAME: &'static str = "CharacterPhysicsComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD707D6C5_3EFA_4275_82EB_A954F845D324);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
