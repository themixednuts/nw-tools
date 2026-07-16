use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod combat_debug_settings;

pub use self::combat_debug_settings::CombatDebugSettings;

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
pub struct GameDebugSettings {
    #[serde(rename = "Combat Settings", default)]
    pub combat_settings: CombatDebugSettings,
}

impl AzRtti for GameDebugSettings {
    const NAME: &'static str = "GameDebugSettings";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3E5DB037_AE49_43E4_8BCC_67F8C511A091);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
