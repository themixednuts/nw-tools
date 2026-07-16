use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct AnimationEvent {
    #[serde(default)]
    pub name: i8,
    #[serde(default)]
    pub time: f32,
    #[serde(rename = "endTime", default)]
    pub end_time: f32,
    #[serde(rename = "animName", default)]
    pub anim_name: i8,
    #[serde(default)]
    pub parameter: i8,
    #[serde(rename = "boneName1", default)]
    pub bone_name_1: i8,
    #[serde(rename = "boneName2", default)]
    pub bone_name_2: i8,
    #[serde(default)]
    pub offset: bevy_math::Vec3,
    #[serde(default)]
    pub direction: bevy_math::Vec3,
}

impl AzRtti for AnimationEvent {
    const NAME: &'static str = "AnimationEvent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x1D927664_8C19_4EA9_A59F_23A81EC486F2);
}
