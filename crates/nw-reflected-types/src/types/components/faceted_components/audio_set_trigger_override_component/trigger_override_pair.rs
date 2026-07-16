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
pub struct TriggerOverridePair {
    #[serde(rename = "m_baseTriggerName", default)]
    pub base_trigger_name: String,
    #[serde(rename = "m_overrideTriggerName", default)]
    pub override_trigger_name: String,
}

impl AzRtti for TriggerOverridePair {
    const NAME: &'static str = "TriggerOverridePair";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2399A1E1_6396_4905_9DF2_462DA21FE17E);
}
