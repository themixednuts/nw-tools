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
pub struct GuildRankData {
    #[serde(rename = "m_name", default)]
    pub name: String,
    #[serde(rename = "m_securityLevel", default)]
    pub security_level: u32,
    #[serde(rename = "m_allPrivileges", default)]
    pub all_privileges: bool,
    #[serde(rename = "m_privilegeIds", default)]
    pub privilege_ids: std::collections::HashSet<u32>,
}

impl AzRtti for GuildRankData {
    const NAME: &'static str = "GuildRankData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE756A995_93ED_F487_1A76_23B1AD74DF11);
}
