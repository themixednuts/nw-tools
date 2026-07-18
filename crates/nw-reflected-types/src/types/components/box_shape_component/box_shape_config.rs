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
pub struct BoxShapeConfig {
    #[serde(rename = "Dimensions", default)]
    pub dimensions: bevy_math::Vec3,
}

impl AzRtti for BoxShapeConfig {
    const NAME: &'static str = "BoxShapeConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xF034FBA2_AC2F_4E66_8152_14DFB90D6283);
}
