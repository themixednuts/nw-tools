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
pub struct MotionParameterSmoothingSettings {
    #[serde(rename = "MovementSpeedEpsilon", default)]
    pub movement_speed_epsilon: f32,
    #[serde(rename = "GroundAngleConvergeTime", default)]
    pub ground_angle_converge_time: f32,
    #[serde(rename = "TravelAngleConvergeTime", default)]
    pub travel_angle_converge_time: f32,
    #[serde(rename = "TravelDistanceConvergeTime", default)]
    pub travel_distance_converge_time: f32,
    #[serde(rename = "TravelSpeedConvergeTime", default)]
    pub travel_speed_converge_time: f32,
    #[serde(rename = "TurnAngleConvergeTime", default)]
    pub turn_angle_converge_time: f32,
    #[serde(rename = "TurnSpeedConvergeTime", default)]
    pub turn_speed_converge_time: f32,
    #[serde(rename = "TurnSpeedScale", default)]
    pub turn_speed_scale: f32,
}

impl AzRtti for MotionParameterSmoothingSettings {
    const NAME: &'static str = "MotionParameterSmoothingSettings";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7DB44746_EA1D_4A53_9270_7600A5AA8027);
}
