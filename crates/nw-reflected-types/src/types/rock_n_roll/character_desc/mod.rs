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
pub struct CharacterDesc {
    #[serde(rename = "Up Direction", default)]
    pub up_direction: bevy_math::Vec3,
    #[serde(rename = "Max Slope", default)]
    pub max_slope: f32,
    #[serde(rename = "Contact distance", default)]
    pub contact_distance: f32,
    #[serde(rename = "Solver max iterations", default)]
    pub solver_max_iterations: u32,
    #[serde(rename = "Asynchronous", default)]
    pub asynchronous: bool,
}

impl AzRtti for CharacterDesc {
    const NAME: &'static str = "RockNRoll::CharacterDesc";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD03BD2CC_5E87_49A7_B490_47A6F6EBDC22);
}
