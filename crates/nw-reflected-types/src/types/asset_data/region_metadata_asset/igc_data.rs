use crate::az::crc::Crc32 as AzCrc32;
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
pub struct IGCData {
    #[serde(rename = "IGCLocation", default)]
    pub igc_location: bevy_math::Vec3,
    #[serde(rename = "IGCId", default)]
    pub igc_id: AzCrc32,
    #[serde(rename = "Duration", default)]
    pub duration: f32,
    #[serde(rename = "IgnoreAI", default)]
    pub ignore_ai: bool,
    #[serde(rename = "PlayerInvincibility", default)]
    pub player_invincibility: bool,
}

impl AzRtti for IGCData {
    const NAME: &'static str = "IGCData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x99F04285_6549_45E5_A1C0_D3D5932B4467);
}
