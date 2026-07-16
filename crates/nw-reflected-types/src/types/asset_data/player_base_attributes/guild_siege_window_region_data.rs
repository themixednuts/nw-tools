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
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct GuildSiegeWindowRegionData {
    #[serde(rename = "Start Hour", default)]
    pub start_hour: u32,
    #[serde(rename = "End Hour", default)]
    pub end_hour: u32,
    #[serde(rename = "UTCOffset", default)]
    pub utc_offset: i32,
    #[serde(rename = "DstRuleId", default)]
    pub dst_rule_id: AzCrc32,
    #[serde(rename = "DstRule", default)]
    pub dst_rule: String,
    #[serde(rename = "ObservesDst", default)]
    pub observes_dst: bool,
}

impl AzRtti for GuildSiegeWindowRegionData {
    const NAME: &'static str = "GuildSiegeWindowRegionData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x1B34AF34_C6AC_4360_BA4A_60F06485710B);
}
