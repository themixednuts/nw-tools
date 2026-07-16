use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct AnimatedLayer {
    #[serde(rename = "Animation Name", default)]
    pub animation_name: String,
    #[serde(rename = "Layer Id", default)]
    pub layer_id: i32,
    #[serde(rename = "Looping", default)]
    pub looping: bool,
    #[serde(rename = "Playback Speed", default)]
    pub playback_speed: f32,
    #[serde(rename = "Layer Weight", default)]
    pub layer_weight: f32,
    #[serde(rename = "AnimDrivenMotion", default)]
    pub anim_driven_motion: bool,
}

impl AzRtti for AnimatedLayer {
    const NAME: &'static str = "AnimatedLayer";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x147EAB48_2D6E_41CF_8414_CEABF3F1E59B);
}
