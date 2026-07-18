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
pub struct StaticPhysicsConfig {
    #[serde(rename = "EnabledInitially", default)]
    pub enabled_initially: bool,
    #[serde(rename = "InteractsWithTriggers", default)]
    pub interacts_with_triggers: bool,
}

impl AzRtti for StaticPhysicsConfig {
    const NAME: &'static str = "StaticPhysicsConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2129576B_A548_4F3E_A2A1_87851BF48838);
}
