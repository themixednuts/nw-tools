use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod additive_conversation_camera_movement_data;
pub mod camp_tier_data;
pub mod contract_buy_sell_fee_data;
pub mod contract_config_data;
pub mod credit_modifier_data;
pub mod daily_bonus_data;
pub mod event_credit_data;
pub mod faction_data;
pub mod faction_influence_config_data;
pub mod fishing_data;
pub mod gather_game_data;
pub mod guaranteed_item_transfer_data;
pub mod guild_siege_window_region_data;
pub mod guild_treasury_data;
pub mod item_rarity_data;
pub mod item_type;
pub mod milestone_correction_data;
pub mod milestone_correction_entry_data;
pub mod perk_generation_data;
pub mod perk_tier_data;
pub mod player_attribute_data;
pub mod player_teleport_context;
pub mod progression_validation_achievement_data;
pub mod pvp_value_entry;
pub mod remote_storage_item_transfer_fee_data;
pub mod remote_storage_item_type_multiplier_data;
pub mod structure_attribute_data;
pub mod structure_placement_data;
pub mod task_interact_data;
pub mod task_interact_entry_data;
pub mod territory_bonus;
pub mod territory_entry_data;
pub mod valid_group_data;
pub mod war_color_data;
pub mod war_data;
pub mod war_deployable_limit_data;

pub use self::additive_conversation_camera_movement_data::AdditiveConversationCameraMovementData;
pub use self::camp_tier_data::CampTierData;
pub use self::contract_buy_sell_fee_data::ContractBuySellFeeData;
pub use self::contract_config_data::ContractConfigData;
pub use self::credit_modifier_data::CreditModifierData;
pub use self::daily_bonus_data::DailyBonusData;
pub use self::event_credit_data::EventCreditData;
pub use self::faction_data::FactionData;
pub use self::faction_influence_config_data::FactionInfluenceConfigData;
pub use self::fishing_data::FishingData;
pub use self::gather_game_data::GatherGameData;
pub use self::guaranteed_item_transfer_data::GuaranteedItemTransferData;
pub use self::guild_siege_window_region_data::GuildSiegeWindowRegionData;
pub use self::guild_treasury_data::GuildTreasuryData;
pub use self::item_rarity_data::ItemRarityData;
pub use self::item_type::ItemType;
pub use self::milestone_correction_data::MilestoneCorrectionData;
pub use self::milestone_correction_entry_data::MilestoneCorrectionEntryData;
pub use self::perk_generation_data::PerkGenerationData;
pub use self::perk_tier_data::PerkTierData;
pub use self::player_attribute_data::PlayerAttributeData;
pub use self::player_teleport_context::PlayerTeleportContext;
pub use self::progression_validation_achievement_data::ProgressionValidationAchievementData;
pub use self::pvp_value_entry::PvpValueEntry;
pub use self::remote_storage_item_transfer_fee_data::RemoteStorageItemTransferFeeData;
pub use self::remote_storage_item_type_multiplier_data::RemoteStorageItemTypeMultiplierData;
pub use self::structure_attribute_data::StructureAttributeData;
pub use self::structure_placement_data::StructurePlacementData;
pub use self::task_interact_data::TaskInteractData;
pub use self::task_interact_entry_data::TaskInteractEntryData;
pub use self::territory_bonus::TerritoryBonus;
pub use self::territory_entry_data::TerritoryEntryData;
pub use self::valid_group_data::ValidGroupData;
pub use self::war_color_data::WarColorData;
pub use self::war_data::WarData;
pub use self::war_deployable_limit_data::WarDeployableLimitData;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct PlayerBaseAttributes {
    #[serde(rename = "Player Attribute Data", default)]
    pub player_attribute_data: PlayerAttributeData,
    #[serde(rename = "Structure Placement Data", default)]
    pub structure_placement_data: StructurePlacementData,
    #[serde(rename = "Structure Attribute Data", default)]
    pub structure_attribute_data: StructureAttributeData,
    #[serde(rename = "Gather Game Data", default)]
    pub gather_game_data: GatherGameData,
    #[serde(rename = "Fishing Data", default)]
    pub fishing_data: FishingData,
    #[serde(rename = "Faction Data", default)]
    pub faction_data: FactionData,
    #[serde(rename = "Event Credit Data", default)]
    pub event_credit_data: EventCreditData,
    #[serde(rename = "War Data", default)]
    pub war_data: WarData,
    #[serde(rename = "Contract Data", default)]
    pub contract_data: ContractConfigData,
    #[serde(rename = "War Color Data", default)]
    pub war_color_data: WarColorData,
    #[serde(rename = "Guild Siege Window Region Data", default)]
    pub guild_siege_window_region_data:
        std::collections::HashMap<String, GuildSiegeWindowRegionData>,
    #[serde(rename = "Guild Treasury Data", default)]
    pub guild_treasury_data: GuildTreasuryData,
    #[serde(rename = "Territory Bonus Data", default)]
    pub territory_bonus_data: Vec<TerritoryEntryData>,
    #[serde(rename = "Remote Storage Item Transfer Fee Data", default)]
    pub remote_storage_item_transfer_fee_data: RemoteStorageItemTransferFeeData,
    #[serde(rename = "Faction Influence Config Data", default)]
    pub faction_influence_config_data: FactionInfluenceConfigData,
    #[serde(rename = "Guaranteed Equipped Item Transfer Data", default)]
    pub guaranteed_equipped_item_transfer_data: GuaranteedItemTransferData,
    #[serde(rename = "Guaranteed Inventory Item Transfer Data", default)]
    pub guaranteed_inventory_item_transfer_data: GuaranteedItemTransferData,
    #[serde(rename = "Valid Group Data", default)]
    pub valid_group_data: ValidGroupData,
    #[serde(rename = "TaskInteract Data", default)]
    pub task_interact_data: TaskInteractData,
    #[serde(rename = "Daily Bonus Data", default)]
    pub daily_bonus_data: DailyBonusData,
    #[serde(rename = "Progression Validation Achievement Data", default)]
    pub progression_validation_achievement_data: ProgressionValidationAchievementData,
    #[serde(rename = "Milestone Correction Data", default)]
    pub milestone_correction_data: MilestoneCorrectionData,
    #[serde(rename = "Additive Conversation Camera Movement Data", default)]
    pub additive_conversation_camera_movement_data: AdditiveConversationCameraMovementData,
}

impl AzRtti for PlayerBaseAttributes {
    const NAME: &'static str = "PlayerBaseAttributes";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0F40ECC6_ACE9_476A_9A5C_B83BE6129A4B);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
