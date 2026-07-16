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
pub struct GatherGameData {
    #[serde(rename = "Perfect Hit Time Amount To Take", default)]
    pub perfect_hit_time_amount_to_take: f32,
    #[serde(rename = "Average Hit Time Amount To Take", default)]
    pub average_hit_time_amount_to_take: f32,
}

impl AzRtti for GatherGameData {
    const NAME: &'static str = "GatherGameData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBE1D2815_14DA_48ED_B11C_D40172FBBB34);
}
