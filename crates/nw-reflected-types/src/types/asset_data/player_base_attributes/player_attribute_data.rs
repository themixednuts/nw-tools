use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{
    CampTierData, EditCrc, EditEnumItemClasses, ItemRarityData, PerkGenerationData,
};

use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct PlayerAttributeData {
    #[serde(rename = "Base Amount To Apply", default)]
    pub base_amount_to_apply: i32,
    #[serde(rename = "Base Apply Rate", default)]
    pub base_apply_rate: f32,
    #[serde(rename = "Structure Rotation Amount ( Deg )", default)]
    pub structure_rotation_amount_deg: f32,
    #[serde(rename = "Base Gather Durability Cost", default)]
    pub base_gather_durability_cost: f32,
    #[serde(rename = "Repair Max Durability Cost", default)]
    pub repair_max_durability_cost: f32,
    #[serde(rename = "One Handed Gathering Distance", default)]
    pub one_handed_gathering_distance: f32,
    #[serde(rename = "Two Handed Gathering Distance", default)]
    pub two_handed_gathering_distance: f32,
    #[serde(rename = "Stamina Cost Entry", default)]
    pub stamina_cost_entry: String,
    #[serde(rename = "Post-FTUE Achievement ID Unlocks", default)]
    pub post_ftue_achievement_id_unlocks: Vec<String>,
    #[serde(rename = "Camping Achievement ID", default)]
    pub camping_achievement_id: Vec<CampTierData>,
    #[serde(rename = "Camping Unlock By Level", default)]
    pub camping_unlock_by_level: Vec<i32>,
    #[serde(rename = "Salvage Gold Tier Modifiers", default)]
    pub salvage_gold_tier_modifiers: Vec<f32>,
    #[serde(rename = "Salvage Min Percent", default)]
    pub salvage_min_percent: f32,
    #[serde(rename = "Salvage Max Percent", default)]
    pub salvage_max_percent: f32,
    #[serde(rename = "Chance of Salvage Success", default)]
    pub chance_of_salvage_success: f32,
    #[serde(rename = "Minimum Salvage Quantity", default)]
    pub minimum_salvage_quantity: i32,
    #[serde(rename = "Salvage Dust Modifier", default)]
    pub salvage_dust_modifier: f32,
    #[serde(rename = "Repair Resource Modifier", default)]
    pub repair_resource_modifier: f32,
    #[serde(rename = "User Camera Min Sensitivity", default)]
    pub user_camera_min_sensitivity: f32,
    #[serde(rename = "User Camera Max Sensitivity", default)]
    pub user_camera_max_sensitivity: f32,
    #[serde(rename = "Base Deployable Limit", default)]
    pub base_deployable_limit: i32,
    #[serde(rename = "Player Age Display String", default)]
    pub player_age_display_string: String,
    #[serde(rename = "Encumbrance Immobilization Modifier", default)]
    pub encumbrance_immobilization_modifier: f32,
    #[serde(rename = "Encumbrance Max Limit Modifier", default)]
    pub encumbrance_max_limit_modifier: f32,
    #[serde(rename = "Max Camp Respawn Distance Meters", default)]
    pub max_camp_respawn_distance_meters: i32,
    #[serde(rename = "Pvp Camp Respawn Cooldown Seconds", default)]
    pub pvp_camp_respawn_cooldown_seconds: u32,
    #[serde(rename = "Max Pvp Respawn Extend Deaths", default)]
    pub max_pvp_respawn_extend_deaths: u32,
    #[serde(rename = "Pvp Death Extend Timeout Seconds", default)]
    pub pvp_death_extend_timeout_seconds: u32,
    #[serde(rename = "Player Display Level Unlock Free Gear Sets", default)]
    pub player_display_level_unlock_free_gear_sets: i32,
    #[serde(rename = "Max Instanced Loot Chest Count", default)]
    pub max_instanced_loot_chest_count: i32,
    #[serde(rename = "Max Instanced SlayerScript State Count", default)]
    pub max_instanced_slayer_script_state_count: i32,
    #[serde(rename = "Instanced Loot Chest Reset Time Mins", default)]
    pub instanced_loot_chest_reset_time_mins: i32,
    #[serde(rename = "Instanced SlayerScript State Reset Time Mins", default)]
    pub instanced_slayer_script_state_reset_time_mins: i32,
    #[serde(rename = "Instanced AI Loot Clear Time Mins", default)]
    pub instanced_ai_loot_clear_time_mins: i32,
    #[serde(rename = "Max AI Loot Receiver Count", default)]
    pub max_ai_loot_receiver_count: i32,
    #[serde(rename = "Min Level Roll Perks", default)]
    pub min_level_roll_perks: i32,
    #[serde(rename = "Min Level Roll Gem Slot", default)]
    pub min_level_roll_gem_slot: i32,
    #[serde(rename = "Drop Probability Falloff", default)]
    pub drop_probability_falloff: f32,
    #[serde(rename = "Drop Probability Min", default)]
    pub drop_probability_min: f32,
    #[serde(rename = "POI Level Loot Tag", default)]
    pub poi_level_loot_tag: EditCrc,
    #[serde(rename = "Min Content Level Loot Tag", default)]
    pub min_content_level_loot_tag: EditCrc,
    #[serde(rename = "Level Loot Tag", default)]
    pub level_loot_tag: EditCrc,
    #[serde(rename = "Enemy Level Loot Tag", default)]
    pub enemy_level_loot_tag: EditCrc,
    #[serde(rename = "Container Level Loot Tag", default)]
    pub container_level_loot_tag: EditCrc,
    #[serde(rename = "Min POI Content Level Loot Tag", default)]
    pub min_poi_content_level_loot_tag: EditCrc,
    #[serde(rename = "Min Enemy Content Level Loot Tag", default)]
    pub min_enemy_content_level_loot_tag: EditCrc,
    #[serde(rename = "Global Mod Loot Tag", default)]
    pub global_mod_loot_tag: EditCrc,
    #[serde(rename = "Fishing Fresh Water Loot Tag", default)]
    pub fishing_fresh_water_loot_tag: EditCrc,
    #[serde(rename = "Fishing Salt Water Loot Tag", default)]
    pub fishing_salt_water_loot_tag: EditCrc,
    #[serde(rename = "Fish Size Loot Tag", default)]
    pub fish_size_loot_tag: EditCrc,
    #[serde(rename = "Fish Rarity Loot Tag", default)]
    pub fish_rarity_loot_tag: EditCrc,
    #[serde(rename = "Summer Fish Rarity Loot Tag", default)]
    pub summer_fish_rarity_loot_tag: EditCrc,
    #[serde(rename = "Loot Table Diverted Loot Tag", default)]
    pub loot_table_diverted_loot_tag: EditCrc,
    #[serde(rename = "Salvage Item Rarity Loot Tag", default)]
    pub salvage_item_rarity_loot_tag: EditCrc,
    #[serde(rename = "Salvage Item Tier Loot Tag", default)]
    pub salvage_item_tier_loot_tag: EditCrc,
    #[serde(rename = "Salvage Item Gear Score Loot Tag", default)]
    pub salvage_item_gear_score_loot_tag: EditCrc,
    #[serde(rename = "Equipped Item Loot Tag", default)]
    pub equipped_item_loot_tag: EditCrc,
    #[serde(rename = "Equipped Tag Ignore Classes", default)]
    pub equipped_tag_ignore_classes: Vec<EditEnumItemClasses>,
    #[serde(rename = "Loot Biasing Item Classes", default)]
    pub loot_biasing_item_classes: Vec<EditEnumItemClasses>,
    #[serde(rename = "Loot Biasing Exception Item Classes", default)]
    pub loot_biasing_exception_item_classes: Vec<EditEnumItemClasses>,
    #[serde(rename = "Attribute Bias Exclusive Label", default)]
    pub attribute_bias_exclusive_label: EditCrc,
    #[serde(rename = "Attribute Exclusive Label Map", default)]
    pub attribute_exclusive_label_map: std::collections::HashMap<i32, EditCrc>,
    #[serde(rename = "Azoth Currency", default)]
    pub azoth_currency: String,
    #[serde(rename = "Azoth Currency Id", default)]
    pub azoth_currency_id: AzCrc32,
    #[serde(rename = "Kill Game Event Id", default)]
    pub kill_game_event_id: EditCrc,
    #[serde(rename = "Broken Item Efficiency Data", default)]
    pub broken_item_efficiency_data: Vec<(i32, f32)>,
    #[serde(rename = "Categorical Progression RankUp Game Event Id", default)]
    pub categorical_progression_rank_up_game_event_id: EditCrc,
    #[serde(rename = "Dynamic Poi Objective ItemIds", default)]
    pub dynamic_poi_objective_item_ids: Vec<EditCrc>,
    #[serde(rename = "Dynamic Poi Objective Reward Modifier Ids", default)]
    pub dynamic_poi_objective_reward_modifier_ids: Vec<EditCrc>,
    #[serde(rename = "Dynamic Poi Objective GameEventId", default)]
    pub dynamic_poi_objective_game_event_id: EditCrc,
    #[serde(rename = "Objective GameEventId", default)]
    pub objective_game_event_id: EditCrc,
    #[serde(rename = "Durability Repair Cost Data", default)]
    pub durability_repair_cost_data: Vec<(i32, i32)>,
    #[serde(rename = "Durability To Coin Rate", default)]
    pub durability_to_coin_rate: f32,
    #[serde(rename = "Inventory Durability Loss Ratio", default)]
    pub inventory_durability_loss_ratio: f32,
    #[serde(rename = "Pvp Paperdoll Durability Loss Multiplier", default)]
    pub pvp_paperdoll_durability_loss_multiplier: f32,
    #[serde(rename = "Pvp Inventory Durability Loss Multiplier", default)]
    pub pvp_inventory_durability_loss_multiplier: f32,
    #[serde(rename = "Chat Max Message Size", default)]
    pub chat_max_message_size: u32,
    #[serde(rename = "Min Armor Mitigation", default)]
    pub min_armor_mitigation: f32,
    #[serde(rename = "Max Armor Mitigation", default)]
    pub max_armor_mitigation: f32,
    #[serde(rename = "Physical Armor Scale Factor", default)]
    pub physical_armor_scale_factor: f32,
    #[serde(rename = "Elemental Armor Scale Factor", default)]
    pub elemental_armor_scale_factor: f32,
    #[serde(rename = "Armor Set Rating Exponent", default)]
    pub armor_set_rating_exponent: f32,
    #[serde(rename = "Armor Mitigation Exponent", default)]
    pub armor_mitigation_exponent: f32,
    #[serde(rename = "Armor Rating Decimal Accuracy", default)]
    pub armor_rating_decimal_accuracy: i32,
    #[serde(rename = "Base Damage Compound Increase", default)]
    pub base_damage_compound_increase: f32,
    #[serde(rename = "Compound Increase Diminishing Multiplier", default)]
    pub compound_increase_diminishing_multiplier: f32,
    #[serde(rename = "Base Damage Gear Score Interval", default)]
    pub base_damage_gear_score_interval: u32,
    #[serde(rename = "Min Possible Weapon Gear Score", default)]
    pub min_possible_weapon_gear_score: u32,
    #[serde(rename = "Diminishing Gear Score Threshold", default)]
    pub diminishing_gear_score_threshold: u32,
    #[serde(rename = "Round Gearscore Up?", default)]
    pub round_gearscore_up: bool,
    #[serde(rename = "Gear Score Rounding Interval", default)]
    pub gear_score_rounding_interval: i32,
    #[serde(rename = "Item Rarity Data", default)]
    pub item_rarity_data: Vec<ItemRarityData>,
    #[serde(rename = "Perk Generation Data", default)]
    pub perk_generation_data: PerkGenerationData,
    #[serde(rename = "Perk Chance Modifier", default)]
    pub perk_chance_modifier: f32,
    #[serde(rename = "Attribute Chance Modifier", default)]
    pub attribute_chance_modifier: f32,
    #[serde(rename = "Gem Slot Chance Modifier", default)]
    pub gem_slot_chance_modifier: f32,
    #[serde(rename = "Perk Chance ItemId", default)]
    pub perk_chance_item_id: String,
    #[serde(rename = "Rested Exp Percentage Per Hour", default)]
    pub rested_exp_percentage_per_hour: f32,
    #[serde(rename = "Rested Exp Max Percentage", default)]
    pub rested_exp_max_percentage: f32,
    #[serde(rename = "Rested Exp Modifier", default)]
    pub rested_exp_modifier: f32,
    #[serde(rename = "Rested Exp Threshold Hours", default)]
    pub rested_exp_threshold_hours: u32,
    #[serde(rename = "Minimum AI Level For Territory Standing", default)]
    pub minimum_ai_level_for_territory_standing: u32,
    #[serde(rename = "Base XP Value for finding lore", default)]
    pub base_xp_value_for_finding_lore: u32,
    #[serde(
        rename = "Number of entries that need to be discovered to advance to the next step",
        default
    )]
    pub number_of_entries_that_need_to_be_discovered_to_advance_to_the_next_step: u32,
    #[serde(rename = "The amount of xp gained per step", default)]
    pub the_amount_of_xp_gained_per_step: u32,
    #[serde(rename = "Max Points Per Attribute", default)]
    pub max_points_per_attribute: u32,
    #[serde(rename = "Level Damage Multiplier", default)]
    pub level_damage_multiplier: f32,
    #[serde(rename = "Paperdoll Slot Unlocks By Level", default)]
    pub paperdoll_slot_unlocks_by_level: Vec<(i32, i32)>,
    #[serde(rename = "Paperdoll Slot Unlocks By Tradeskill Rank", default)]
    pub paperdoll_slot_unlocks_by_tradeskill_rank: Vec<(i32, String, i32)>,
    #[serde(rename = "Gear Set Storage Excluded Paperdoll Slots", default)]
    pub gear_set_storage_excluded_paperdoll_slots: Vec<i32>,
    #[serde(
        rename = "Ability Points Required In Tree to Unlock Final Row",
        default
    )]
    pub ability_points_required_in_tree_to_unlock_final_row: i32,
    #[serde(rename = "Ability Point Row Requirements", default)]
    pub ability_point_row_requirements: Vec<(i32, i32)>,
    #[serde(rename = "Status Effect Category Limits", default)]
    pub status_effect_category_limits: std::collections::HashMap<String, i32>,
    #[serde(rename = "Status Effect Category Limit Crcs", default)]
    pub status_effect_category_limit_crcs: std::collections::HashMap<AzCrc32, i32>,
    #[serde(rename = "PropertyTaxRateMin", default)]
    pub property_tax_rate_min: f32,
    #[serde(rename = "PropertyTaxRateMax", default)]
    pub property_tax_rate_max: f32,
    #[serde(rename = "TradingTaxRateMin", default)]
    pub trading_tax_rate_min: f32,
    #[serde(rename = "TradingTaxRateMax", default)]
    pub trading_tax_rate_max: f32,
    #[serde(rename = "CraftingFeeRateMin", default)]
    pub crafting_fee_rate_min: f32,
    #[serde(rename = "CraftingFeeRateMax", default)]
    pub crafting_fee_rate_max: f32,
    #[serde(rename = "RefiningFeeRateMin", default)]
    pub refining_fee_rate_min: f32,
    #[serde(rename = "RefiningFeeRateMax", default)]
    pub refining_fee_rate_max: f32,
    #[serde(rename = "SetTaxOrFeeCoolDownInMin", default)]
    pub set_tax_or_fee_cool_down_in_min: u32,
    #[serde(rename = "ControllingCompanyTaxModifier", default)]
    pub controlling_company_tax_modifier: f32,
    #[serde(rename = "ControllingCompanyHouseCostModifier", default)]
    pub controlling_company_house_cost_modifier: f32,
    #[serde(rename = "ControllingFactionLuckModifier", default)]
    pub controlling_faction_luck_modifier: i32,
    #[serde(rename = "ControllingFactionGatherModifier", default)]
    pub controlling_faction_gather_modifier: f32,
    #[serde(rename = "AlwaysAvailableTownProjectId", default)]
    pub always_available_town_project_id: EditCrc,
    #[serde(rename = "AllProjectPoolId", default)]
    pub all_project_pool_id: EditCrc,
    #[serde(rename = "Blocking Threat Mulitplier", default)]
    pub blocking_threat_mulitplier: f32,
    #[serde(rename = "Final Starting Beach Achievement", default)]
    pub final_starting_beach_achievement: EditCrc,
    #[serde(rename = "First Time FFA Achievement", default)]
    pub first_time_ffa_achievement: EditCrc,
    #[serde(rename = "First House Flat Discount", default)]
    pub first_house_flat_discount: u32,
}

impl AzRtti for PlayerAttributeData {
    const NAME: &'static str = "PlayerAttributeData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x46113BED_540D_4584_92AA_B9223D83875A);
}
