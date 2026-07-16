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
pub struct ConsumeLoadedAmmo {
    #[serde(rename = "m_ammoCount", default)]
    pub ammo_count: i32,
    #[serde(rename = "m_consumeFullClip", default)]
    pub consume_full_clip: bool,
    #[serde(rename = "m_consumeOnExit", default)]
    pub consume_on_exit: bool,
}

impl AzRtti for ConsumeLoadedAmmo {
    const NAME: &'static str = "ConsumeLoadedAmmo";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6EA4ECBE_C0DD_4EF0_AD25_A6EF6A32DA25);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
