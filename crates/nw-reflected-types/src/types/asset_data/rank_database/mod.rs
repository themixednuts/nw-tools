use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod guild_rank_data;
pub mod rank_data;

pub use self::guild_rank_data::GuildRankData;
pub use self::rank_data::RankData;

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
pub struct RankDatabase {
    #[serde(rename = "Ranks", default)]
    pub ranks: Vec<RankData>,
}

impl AzRtti for RankDatabase {
    const NAME: &'static str = "RankDatabase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xB0024F1F_651D_48A5_A56A_9DEA80CB487E);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
