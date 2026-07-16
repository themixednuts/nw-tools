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
pub struct CombatDebugSettings {
    #[serde(rename = "Disable Player Loot Drop On Death", default)]
    pub disable_player_loot_drop_on_death: bool,
    #[serde(rename = "Disable Weapon Durability", default)]
    pub disable_weapon_durability: bool,
    #[serde(rename = "Disable Item Durability", default)]
    pub disable_item_durability: bool,
    #[serde(rename = "Disable Durability Penalty On Death", default)]
    pub disable_durability_penalty_on_death: bool,
}

impl AzRtti for CombatDebugSettings {
    const NAME: &'static str = "CombatDebugSettings";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3C0E5DC7_06B9_4411_893E_DAAC101731D3);
}
