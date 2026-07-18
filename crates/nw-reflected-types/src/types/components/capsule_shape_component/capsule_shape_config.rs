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
pub struct CapsuleShapeConfig {
    #[serde(rename = "Height", default)]
    pub height: f32,
    #[serde(rename = "Radius", default)]
    pub radius: f32,
}

impl AzRtti for CapsuleShapeConfig {
    const NAME: &'static str = "CapsuleShapeConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x00931AEB_2AD8_42CE_B1DC_FA4332F51501);
}
