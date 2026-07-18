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
pub struct SpawnParticleEffect {}

impl AzRtti for SpawnParticleEffect {
    const NAME: &'static str = "SpawnParticleEffect";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAD2CD7AC_4CE1_420C_A3B8_F69F2D44A33C);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
