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
pub struct AzTextureSlotSettings {
    #[serde(rename = "Rot", default)]
    pub rot: f32,
    #[serde(rename = "RotOscRate", default)]
    pub rot_osc_rate: f32,
    #[serde(rename = "RotOscPhase", default)]
    pub rot_osc_phase: f32,
    #[serde(rename = "RotOscAmplitude", default)]
    pub rot_osc_amplitude: f32,
    #[serde(rename = "RotOscCenter", default)]
    pub rot_osc_center: f32,
    #[serde(rename = "RotTiling", default)]
    pub rot_tiling: f32,
    #[serde(rename = "Offset", default)]
    pub offset: f32,
    #[serde(rename = "MoveType", default)]
    pub move_type: i32,
    #[serde(rename = "OscRate", default)]
    pub osc_rate: f32,
    #[serde(rename = "OscPhase", default)]
    pub osc_phase: f32,
    #[serde(rename = "OscAmplitude", default)]
    pub osc_amplitude: f32,
}

impl AzRtti for AzTextureSlotSettings {
    const NAME: &'static str = "AzTextureSlotSettings";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x04B64C14_D20C_43DD_9B6D_90368A5D0FDD);
}
