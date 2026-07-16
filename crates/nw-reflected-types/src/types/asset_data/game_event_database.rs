use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct GameEventDatabase {
    #[serde(rename = "Unarmed Attack Event Name", default)]
    pub unarmed_attack_event_name: String,
    #[serde(rename = "Player Attack XP Mod.", default)]
    pub player_attack_xp_mod: f32,
    #[serde(rename = "Default Attack XP Mod.", default)]
    pub default_attack_xp_mod: f32,
    #[serde(rename = "Structure Attack XP Mod.", default)]
    pub structure_attack_xp_mod: f32,
    #[serde(rename = "Self Damage XP Mod.", default)]
    pub self_damage_xp_mod: f32,
}

impl AzRtti for GameEventDatabase {
    const NAME: &'static str = "GameEventDatabase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3B7D0A86_3451_423B_B3B0_8548796B7D1C);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
