use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod az_material_asset_data;
pub mod buildable_state_database;
pub mod cage_action_list_asset;
pub mod cage_grid_asset;
pub mod character_creation_database;
pub mod chunk_trace_asset;
pub mod collision_filters_asset;
pub mod crest_database;
pub mod event_notification_database;
pub mod game_debug_settings;
pub mod game_event_database;
pub mod gathering_action_database;
pub mod gathering_database;
pub mod input_event_bindings_asset;
pub mod input_map_asset;
pub mod player_base_attributes;
pub mod rank_database;
pub mod region_material_data_asset;
pub mod region_metadata_asset;
pub mod serializable_macro_material_params;
pub mod settlement_progression_data;
pub mod territory_landmark_type;
pub mod ui_database;
pub mod vegetation_codex_asset;
pub mod vegetation_image_asset;
pub mod world_material_data_asset;

pub use self::az_material_asset_data::{
    AzLightingParams, AzMaterialAssetData, AzMaterialLayer, AzTextureSlot, AzTextureSlotSettings,
};

pub use self::buildable_state_database::{
    BuildableStateData, BuildableStateDatabase, BuildableStateEnum,
};

pub use self::cage_action_list_asset::CAGEActionListAsset;
pub use self::cage_grid_asset::CAGEGridAsset;
pub use self::character_creation_database::{
    CharacterCreationData, CharacterCreationDatabase, DefaultAppearanceData,
};

pub use self::chunk_trace_asset::{ChunkEntityTrace, ChunkTraceAsset, SliceData};
pub use self::collision_filters_asset::{
    CollisionFilterColor, CollisionFiltersAsset, EditableCollisionFilter,
};

pub use self::crest_database::{CrestColorData, CrestData, CrestDatabase};
pub use self::event_notification_database::{EventNotificationData, EventNotificationDatabase};

pub use self::game_debug_settings::{CombatDebugSettings, GameDebugSettings};
pub use self::game_event_database::GameEventDatabase;
pub use self::gathering_action_database::{GatheringActionData, GatheringActionDatabase};
pub use self::gathering_database::{
    GatheringAction, GatheringData, GatheringDatabase, GatheringTypeData,
};

pub use self::input_event_bindings_asset::{
    InputEventBindings, InputEventBindingsAsset, InputEventGroup, InputSubComponent,
};

pub use self::input_map_asset::InputMapAsset;
pub use self::player_base_attributes::{
    AdditiveConversationCameraMovementData, CampTierData, ContractBuySellFeeData,
    ContractConfigData, CreditModifierData, DailyBonusData, EventCreditData, FactionData,
    FactionInfluenceConfigData, FishingData, GatherGameData, GuaranteedItemTransferData,
    GuildSiegeWindowRegionData, GuildTreasuryData, ItemRarityData, ItemType,
    MilestoneCorrectionData, MilestoneCorrectionEntryData, PerkGenerationData, PerkTierData,
    PlayerAttributeData, PlayerBaseAttributes, PlayerTeleportContext,
    ProgressionValidationAchievementData, PvpValueEntry, RemoteStorageItemTransferFeeData,
    RemoteStorageItemTypeMultiplierData, StructureAttributeData, StructurePlacementData,
    TaskInteractData, TaskInteractEntryData, TerritoryBonus, TerritoryEntryData, ValidGroupData,
    WarColorData, WarData, WarDeployableLimitData,
};

pub use self::rank_database::{GuildRankData, RankData, RankDatabase};
pub use self::region_material_data_asset::{RegionMaterialDataAsset, TerrainMaterialLayerData};

pub use self::region_metadata_asset::{
    AISpawnLocation, EncounterEntry, GatherableRegionEntry, IGCData, InstancedSlayerScriptPart,
    NPCData, RegionMetadataAsset, TerritoryLandmarkData, TerritoryLoreData,
};

pub use self::serializable_macro_material_params::SerializableMacroMaterialParams;
pub use self::settlement_progression_data::{
    ProgressionCategoryEntry, ProgressionSpawnerEntry, SettlementProgressionData,
};

pub use self::territory_landmark_type::TerritoryLandmarkType;
pub use self::ui_database::{
    InteractOptionData, UiAdditionalInfoType, UiDatabase, UiDelayedInteractionData,
    UiInteractActionType, UiInteractAvailabilityData, UiInteractInputType,
    UiInteractOptionCategory, UiInteractPrivilegeId, UnifiedInteractData,
};

pub use self::vegetation_codex_asset::VegetationCodexAsset;
pub use self::vegetation_image_asset::VegetationImageAsset;
pub use self::world_material_data_asset::{TileMaterialData, WorldMaterialDataAsset};

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
pub struct AssetData;

impl AzRtti for AssetData {
    const NAME: &'static str = "AssetData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C);
}
