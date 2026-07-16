use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct UiDelayedInteractionData {
    #[serde(rename = "Delay Time", default)]
    pub delay_time: f32,
    #[serde(rename = "Delay Mannequin Tag", default)]
    pub delay_mannequin_tag: String,
}

impl AzRtti for UiDelayedInteractionData {
    const NAME: &'static str = "UiDelayedInteractionData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA1119A25_48F3_4585_A078_26EF4CBB23E6);
}
