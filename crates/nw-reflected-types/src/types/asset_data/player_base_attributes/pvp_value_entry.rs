use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::EditCrc;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct PvpValueEntry {
    #[serde(rename = "Kill Reward Modifier", default)]
    pub kill_reward_modifier: EditCrc,
    #[serde(rename = "Value Threshold", default)]
    pub value_threshold: f32,
}

impl AzRtti for PvpValueEntry {
    const NAME: &'static str = "PvpValueEntry";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD911F9A5_D0DA_4461_AED6_95A250A49A6B);
}
