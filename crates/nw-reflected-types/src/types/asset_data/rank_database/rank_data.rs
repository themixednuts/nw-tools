use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::GuildRankData;
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
pub struct RankData {
    #[serde(rename = "GuildRankData", default)]
    pub guild_rank_data: GuildRankData,
}

impl AzRtti for RankData {
    const NAME: &'static str = "RankData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2F2C2714_E932_43BF_A702_CACD8C9AE544);
}
