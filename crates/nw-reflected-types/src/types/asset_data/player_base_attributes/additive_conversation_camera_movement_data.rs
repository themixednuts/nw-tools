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
pub struct AdditiveConversationCameraMovementData {
    #[serde(rename = "YMovementModifier", default)]
    pub y_movement_modifier: f32,
    #[serde(rename = "InverseMovementScaler", default)]
    pub inverse_movement_scaler: f32,
    #[serde(rename = "MaxDeviationFromCenter", default)]
    pub max_deviation_from_center: f32,
    #[serde(rename = "CamSpeedPerSecond", default)]
    pub cam_speed_per_second: f32,
    #[serde(rename = "MaxCamSpeedVariation", default)]
    pub max_cam_speed_variation: f32,
}

impl AzRtti for AdditiveConversationCameraMovementData {
    const NAME: &'static str = "AdditiveConversationCameraMovementData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xDAC8F491_915C_4282_ACB3_E14A793B35F3);
}
