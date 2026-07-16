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
pub struct AudioPreload {
    #[serde(rename = "m_audioPreload", default)]
    pub audio_preload: i8,
}

impl AzRtti for AudioPreload {
    const NAME: &'static str = "AudioPreload";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7A8CFC10_F684_4DE0_B1C0_F841275D02EC);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
