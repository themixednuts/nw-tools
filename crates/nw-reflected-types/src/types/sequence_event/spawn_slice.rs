use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SlayerScriptLiteral;
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
pub struct SpawnSlice {
    #[serde(rename = "m_sliceToSpawn", default)]
    pub slice_to_spawn: i8,
    #[serde(rename = "m_despawnOnExit", default)]
    pub despawn_on_exit: bool,
    #[serde(rename = "m_sliceDestructionCageAction", default)]
    pub slice_destruction_cage_action: SlayerScriptLiteral,
    #[serde(rename = "m_useAreaSpawner", default)]
    pub use_area_spawner: bool,
    #[serde(rename = "m_spawnOnClientsOnly", default)]
    pub spawn_on_clients_only: bool,
    #[serde(rename = "m_canBeDisabledByEmoteFxSettings", default)]
    pub can_be_disabled_by_emote_fx_settings: bool,
    #[serde(rename = "m_targetPosBlackboardKey", default)]
    pub target_pos_blackboard_key: SlayerScriptLiteral,
    #[serde(rename = "m_spawnCount", default)]
    pub spawn_count: i32,
    #[serde(rename = "m_spawnLocationOverrideIndex", default)]
    pub spawn_location_override_index: i32,
}

impl AzRtti for SpawnSlice {
    const NAME: &'static str = "SpawnSlice";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x582B2419_C0F7_4D50_9FE7_222780454AF1);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
