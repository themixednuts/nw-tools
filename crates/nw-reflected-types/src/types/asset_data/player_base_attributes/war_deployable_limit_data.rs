use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct WarDeployableLimitData {
    #[serde(rename = "m_id", default)]
    pub id: AzCrc32,
    #[serde(rename = "m_displayName", default)]
    pub display_name: String,
    #[serde(rename = "m_buildableNames", default)]
    pub buildable_names: Vec<String>,
    #[serde(rename = "m_buildableIds", default)]
    pub buildable_ids: std::collections::HashSet<AzCrc32>,
    #[serde(rename = "m_attackerLimits", default)]
    pub attacker_limits: [i32; 3],
    #[serde(rename = "m_defenderLimit", default)]
    pub defender_limit: i32,
}

impl AzRtti for WarDeployableLimitData {
    const NAME: &'static str = "WarDeployableLimitData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xFDA4A41D_1FC4_4038_B3B8_A8725DB08A24);
}
