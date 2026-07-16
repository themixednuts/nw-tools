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
pub struct DailyBonusData {
    #[serde(rename = "BonusResetHour", default)]
    pub bonus_reset_hour: u8,
}

impl AzRtti for DailyBonusData {
    const NAME: &'static str = "DailyBonusData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x815AB639_EF70_43F7_9E96_EB1B2E511087);
}
