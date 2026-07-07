use super::*;

const NON_ZERO_U8: &str = "std::num::NonZeroU8";
const NON_ZERO_U16: &str = "std::num::NonZeroU16";

pub(super) fn mutation_difficulty_static_data_manager_spec() -> NativeManagerSpec {
    numeric_projection(NumericProjectionSpec {
        module: "mutation_difficulty_static_data",
        table_name: "MutationDifficulty",
        row_type_name: "MutationDifficultyStaticData",
        data_type: "MutationDifficultyStaticData",
        entries_field: "mutation_difficulties",
        index_field: "mutation_difficulties_by_id",
        key_field: "mutation_difficulty",
        key_column: "MutationDifficulty",
        key_getter: "mutation_difficulty",
        key_type: NativeNumericKeyType::NonZeroU8,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("mutation_difficulty_for_source_row"),
        fields: mutation_difficulty_fields(),
        lookup_methods: vec![numeric_lookup(
            "mutation_difficulty",
            "mutation_difficulty",
            NativeNumericLookupParameterKind::NonZeroU8,
        )],
        rows_method: Some("mutation_difficulties"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::MutationDifficultyStaticDataManager",
        rust_type: "crate::MutationDifficultyStaticDataManager",
        ghidra_functions: vec![
            "Javelin::MutationDifficultyStaticDataManager::MutationDifficultyStaticDataManager",
            "Javelin::MutationDifficultyStaticDataManager::CacheAllDataTables",
            "Javelin::MutationDifficultyStaticDataManager::GetGameSystemData",
            "Javelin::MutationDifficultyStaticDataManager::FindGameSystemDataByKey",
        ],
    })
}

fn mutation_difficulty_fields() -> Vec<NativeProjectionField> {
    let mut fields = vec![
        typed_projection_field(
            "difficulty_tier",
            "DifficultyTier",
            "difficulty_tier",
            NativeProjectionTransform::TypedCell,
            NON_ZERO_U8,
        ),
        projection_field(
            "req_items_to_enter",
            "ReqItemsToEnter",
            "req_items_to_enter",
            NativeProjectionTransform::OptionalCrcListDefaultEmpty,
        ),
        projection_field(
            "health_increase_mod",
            "HealthIncreaseMod",
            "health_increase_mod",
            NativeProjectionTransform::Crc32,
        ),
        projection_field(
            "damage_increase_mod",
            "DamageIncreaseMod",
            "damage_increase_mod",
            NativeProjectionTransform::Crc32,
        ),
    ];

    fields.extend(creature_potency_fields());
    fields.extend(curse_and_loot_fields());
    fields.extend(completion_event_fields());
    fields.extend(perk_roll_fields());
    fields.extend(matchmaking_fields());

    fields
}

fn creature_potency_fields() -> Vec<NativeProjectionField> {
    vec![
        projection_field(
            "health_potency_dungeon",
            "HealthPotency_Dungeon-",
            "health_potency_dungeon",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "damage_potency_dungeon",
            "DamagePotency_Dungeon-",
            "damage_potency_dungeon",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "health_potency_dungeon_2",
            "HealthPotency_Dungeon",
            "health_potency_dungeon_2",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "damage_potency_dungeon_2",
            "DamagePotency_Dungeon",
            "damage_potency_dungeon_2",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "health_potency_dungeon_3",
            "HealthPotency_Dungeon+",
            "health_potency_dungeon_3",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "damage_potency_dungeon_3",
            "DamagePotency_Dungeon+",
            "damage_potency_dungeon_3",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "health_potency_dungeon_mini_boss",
            "HealthPotency_DungeonMiniBoss",
            "health_potency_dungeon_mini_boss",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "damage_potency_dungeon_mini_boss",
            "DamagePotency_DungeonMiniBoss",
            "damage_potency_dungeon_mini_boss",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "health_potency_dungeon_boss",
            "HealthPotency_DungeonBoss",
            "health_potency_dungeon_boss",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "damage_potency_dungeon_boss",
            "DamagePotency_DungeonBoss",
            "damage_potency_dungeon_boss",
            NativeProjectionTransform::F32,
        ),
    ]
}

fn curse_and_loot_fields() -> Vec<NativeProjectionField> {
    vec![
        projection_field(
            "has_minor_curse",
            "HasMinorCurse",
            "has_minor_curse",
            NativeProjectionTransform::Bool,
        ),
        projection_field(
            "has_major_curse",
            "HasMajorCurse",
            "has_major_curse",
            NativeProjectionTransform::Bool,
        ),
        typed_projection_field(
            "recommended_gear_score",
            "RecommendedGearScore",
            "recommended_gear_score",
            NativeProjectionTransform::TypedCell,
            NON_ZERO_U16,
        ),
        projection_field(
            "gear_score_delta_modifier",
            "GearScoreDeltaModifier",
            "gear_score_delta_modifier",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "base_curse_damage_mod",
            "BaseCurseDamageMod",
            "base_curse_damage_mod",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "injected_loot_tags",
            "InjectedLootTags",
            "injected_loot_tags",
            NativeProjectionTransform::CrcList,
        ),
        projection_field(
            "injected_creature_loot",
            "InjectedCreatureLoot",
            "injected_creature_loot",
            NativeProjectionTransform::CrcList,
        ),
        projection_field(
            "injected_creature_loot_roll",
            "InjectedCreatureLootRoll",
            "injected_creature_loot_roll",
            NativeProjectionTransform::OptionalU32,
        ),
        projection_field(
            "injected_container_loot",
            "InjectedContainerLoot",
            "injected_container_loot",
            NativeProjectionTransform::CrcList,
        ),
        projection_field(
            "injected_container_loot_roll",
            "InjectedContainerLootRoll",
            "injected_container_loot_roll",
            NativeProjectionTransform::OptionalU32,
        ),
        projection_field(
            "loot_gs_range_override",
            "LootGSRangeOverride",
            "loot_gs_range_override",
            NativeProjectionTransform::U32RangeInclusive,
        ),
        projection_field(
            "use_level_gs",
            "UseLevelGS",
            "use_level_gs",
            NativeProjectionTransform::OptionalBoolDefaultFalse,
        ),
        projection_field(
            "gs_clamp_level_req_delta",
            "GSClampLevelReqDelta",
            "gs_clamp_level_req_delta",
            NativeProjectionTransform::NonZeroU32,
        ),
        projection_field(
            "possible_item_drop_ids",
            "PossibleItemDropIds",
            "possible_item_drop_ids",
            NativeProjectionTransform::OptionalString,
        ),
    ]
}

fn completion_event_fields() -> Vec<NativeProjectionField> {
    vec![
        projection_field(
            "completion_event1",
            "CompletionEvent1",
            "completion_event1",
            NativeProjectionTransform::Crc32,
        ),
        projection_field(
            "completion_event2",
            "CompletionEvent2",
            "completion_event2",
            NativeProjectionTransform::Crc32,
        ),
        projection_field(
            "completion_event3",
            "CompletionEvent3",
            "completion_event3",
            NativeProjectionTransform::Crc32,
        ),
    ]
}

fn perk_roll_fields() -> Vec<NativeProjectionField> {
    vec![
        projection_field(
            "perk_roll_mult1",
            "PerkRollMult1",
            "perk_roll_mult1",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "perk_roll_mult2",
            "PerkRollMult2",
            "perk_roll_mult2",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "perk_roll_mult3",
            "PerkRollMult3",
            "perk_roll_mult3",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "perk_roll_mult4",
            "PerkRollMult4",
            "perk_roll_mult4",
            NativeProjectionTransform::F32,
        ),
        projection_field(
            "perk_roll_mult5",
            "PerkRollMult5",
            "perk_roll_mult5",
            NativeProjectionTransform::F32,
        ),
    ]
}

fn matchmaking_fields() -> Vec<NativeProjectionField> {
    vec![
        projection_field(
            "matchmaking_min_level",
            "MatchmakingMinLevel",
            "matchmaking_min_level",
            NativeProjectionTransform::NonZeroU32,
        ),
        projection_field(
            "matchmaking_max_level",
            "MatchmakingMaxLevel",
            "matchmaking_max_level",
            NativeProjectionTransform::NonZeroU32,
        ),
        projection_field(
            "matchmaking_min_gs",
            "MatchmakingMinGS",
            "matchmaking_min_gs",
            NativeProjectionTransform::NonZeroU32,
        ),
        projection_field(
            "uses_role_selection",
            "UsesRoleSelection",
            "uses_role_selection",
            NativeProjectionTransform::Bool,
        ),
        projection_field(
            "matchmaking_required_healing_focus",
            "MatchmakingRequiredHealingFocus",
            "matchmaking_required_healing_focus",
            NativeProjectionTransform::NonZeroU32,
        ),
        projection_field(
            "matchmaking_required_healing_mastery",
            "MatchmakingRequiredHealingMastery",
            "matchmaking_required_healing_mastery",
            NativeProjectionTransform::NonZeroU32,
        ),
        projection_field(
            "matchmaking_requires_taunt_gem",
            "MatchmakingRequiresTauntGem",
            "matchmaking_requires_taunt_gem",
            NativeProjectionTransform::Bool,
        ),
        typed_projection_field(
            "matchmaking_min_tanks",
            "MatchmakingMinTanks",
            "matchmaking_min_tanks",
            NativeProjectionTransform::TypedCell,
            NON_ZERO_U8,
        ),
        typed_projection_field(
            "matchmaking_max_tanks",
            "MatchmakingMaxTanks",
            "matchmaking_max_tanks",
            NativeProjectionTransform::TypedCell,
            NON_ZERO_U8,
        ),
        typed_projection_field(
            "matchmaking_min_healers",
            "MatchmakingMinHealers",
            "matchmaking_min_healers",
            NativeProjectionTransform::TypedCell,
            NON_ZERO_U8,
        ),
        typed_projection_field(
            "matchmaking_max_healers",
            "MatchmakingMaxHealers",
            "matchmaking_max_healers",
            NativeProjectionTransform::TypedCell,
            NON_ZERO_U8,
        ),
        typed_projection_field(
            "matchmaking_min_dps",
            "MatchmakingMinDPS",
            "matchmaking_min_dps",
            NativeProjectionTransform::TypedCell,
            NON_ZERO_U8,
        ),
        typed_projection_field(
            "matchmaking_max_dps",
            "MatchmakingMaxDPS",
            "matchmaking_max_dps",
            NativeProjectionTransform::TypedCell,
            NON_ZERO_U8,
        ),
        projection_field(
            "use_mm_buff",
            "UseMMBuff",
            "use_mm_buff",
            NativeProjectionTransform::Bool,
        ),
    ]
}
