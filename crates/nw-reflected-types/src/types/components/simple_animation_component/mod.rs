use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod animated_layer;

pub use self::animated_layer::AnimatedLayer;

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
pub struct SimpleAnimationComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Hide Until Animated", default)]
    pub hide_until_animated: bool,
    #[serde(rename = "Playback Entries", default)]
    pub playback_entries: Vec<AnimatedLayer>,
}

impl AzRtti for SimpleAnimationComponent {
    const NAME: &'static str = "SimpleAnimationComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xFBB470EF_2288_4B62_B41F_D830DD4C5B98);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
