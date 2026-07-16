use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CreditModifierData {
    #[serde(rename = "Xp Modifier", default)]
    pub xp_modifier: f32,
    #[serde(rename = "Loot Modifier", default)]
    pub loot_modifier: f32,
    #[serde(rename = "Currency Modifier", default)]
    pub currency_modifier: f32,
    #[serde(rename = "Territory Standing Modifier", default)]
    pub territory_standing_modifier: f32,
}

impl AzRtti for CreditModifierData {
    const NAME: &'static str = "CreditModifierData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x4D2A06D4_5686_47C6_AF00_2CC6DAB1DDEB);
}
