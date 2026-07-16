use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::MilestoneCorrectionEntryData;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct MilestoneCorrectionData {
    #[serde(rename = "CurrentMilestoneVersion", default)]
    pub current_milestone_version: i32,
    #[serde(rename = "MilestoneCorrections", default)]
    pub milestone_corrections: Vec<MilestoneCorrectionEntryData>,
}

impl AzRtti for MilestoneCorrectionData {
    const NAME: &'static str = "MilestoneCorrectionData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE1AFB1E4_F50C_48AA_8635_3518C7DB71AD);
}
