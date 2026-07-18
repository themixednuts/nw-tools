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
pub struct AudioTriggerCC4062C6 {}

impl AzRtti for AudioTriggerCC4062C6 {
    const NAME: &'static str = "AudioTrigger";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xCC4062C6_59DF_4FD3_BED1_33524B7A596F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
