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
pub struct GuildTreasuryData {
    #[serde(rename = "Default Daily Withdrawal Limit", default)]
    pub default_daily_withdrawal_limit: u64,
}

impl AzRtti for GuildTreasuryData {
    const NAME: &'static str = "GuildTreasuryData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x500578D5_B32D_4773_9027_3CF3041C565D);
}
