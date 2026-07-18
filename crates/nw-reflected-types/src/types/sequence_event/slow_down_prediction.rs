use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct SlowDownPrediction {}

impl AzRtti for SlowDownPrediction {
    const NAME: &'static str = "SlowDownPrediction";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7E1DB649_F1EE_4B4E_9D5C_2D3A3E542C8F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
