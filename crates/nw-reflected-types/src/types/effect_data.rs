use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
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
pub struct EffectData {
    #[serde(rename = "m_effectId", default)]
    pub effect_id: String,
}

impl AzRtti for EffectData {
    const NAME: &'static str = "EffectData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA297D455_5F26_47FE_9268_BB4526E9917A);
}
