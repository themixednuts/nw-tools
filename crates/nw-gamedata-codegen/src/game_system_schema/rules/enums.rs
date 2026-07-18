use super::catalog::{ColumnRule, matching_rule_value};
use crate::game_system_schema::{
    GameSystemEnumRepresentation, GameSystemEnumShape, GameSystemEnumVariant,
    semantic::ColumnSemanticProfile,
};

type EnumShapeFactory = fn() -> GameSystemEnumShape;

pub(in crate::game_system_schema) fn scalar_enum_column_affinity(
    row_type_name: &str,
    column_name: &str,
) -> Option<GameSystemEnumShape> {
    matching_rule_value(
        SCALAR_ENUM_COLUMN_RULES,
        &ColumnSemanticProfile::new(row_type_name, column_name),
    )
    .map(|factory| factory())
}

pub(in crate::game_system_schema) fn scalar_enum_shape_by_name(
    name: &str,
) -> Option<GameSystemEnumShape> {
    SCALAR_ENUM_COLUMN_RULES
        .iter()
        .map(|rule| (rule.value())())
        .find(|shape| shape.name == name)
}

const SCALAR_ENUM_COLUMN_RULES: &[ColumnRule<EnumShapeFactory>] = &[
    enum_rule(
        "ParticleContextualPriorityOverrideData",
        "PriorityOverride",
        particle_priority_override_enum_shape,
    ),
    enum_rule(
        "RewardMilestoneData",
        "MilestoneType",
        reward_milestone_type_enum_shape,
    ),
    enum_rule(
        "LeaderboardData",
        "Rotation",
        leaderboard_rotations_enum_shape,
    ),
    enum_rule(
        "LeaderboardStatData",
        "Rotation",
        leaderboard_rotations_enum_shape,
    ),
    enum_rule(
        "LeaderboardRewardsData",
        "Rotation",
        leaderboard_rotations_enum_shape,
    ),
    enum_rule(
        "LeaderboardStatData",
        "StatType",
        leaderboard_stat_types_enum_shape,
    ),
    enum_rule(
        "LeaderboardStatData",
        "Aggregation",
        leaderboard_stat_aggregations_enum_shape,
    ),
    enum_rule("LeaderboardStatData", "Scope", leaderboard_scope_enum_shape),
    ColumnRule::any_row_exact("ExpansionIdUnlock", expansion_id_enum_shape),
    enum_rule("ExpansionData", "ExpansionId", expansion_id_enum_shape),
    enum_rule(
        "ReusableScoreboardTabData",
        "RowType",
        scoreboard_row_type_enum_shape,
    ),
    enum_rule(
        "ReusableScoreboardTabData",
        "StatSource",
        scoreboard_stat_source_enum_shape,
    ),
    enum_rule(
        "ReusableScoreboardTabData",
        "TabDataFilter",
        scoreboard_tab_enum_shape,
    ),
    enum_rule(
        "ReusableScoreboardTabData",
        "DefaultColumnSortMode",
        scoreboard_sort_mode_enum_shape,
    ),
    enum_rule(
        "ReusableScoreboardTabData",
        "RankDeterminingStat",
        warboard_stat_type_enum_shape,
    ),
    enum_rule(
        "SeasonsRewardsStats",
        "StatType",
        seasons_tracked_stat_type_enum_shape,
    ),
    enum_rule(
        "SeasonsRewardsStats",
        "ChapterType",
        seasons_chapter_type_enum_shape,
    ),
    enum_rule(
        "SeasonsRewardsStats",
        "SongDifficulty",
        musical_performance_pattern_enum_shape,
    ),
    enum_rule(
        "SeasonsRewardsStats",
        "MutatorRank",
        seasons_mutator_rank_enum_shape,
    ),
    enum_rule(
        "SeasonsRewardData",
        "RewardType",
        seasons_rewards_reward_type_enum_shape,
    ),
    enum_rule(
        "MissionData",
        "MissionGoalType",
        mission_goal_type_enum_shape,
    ),
    enum_rule(
        "MissionWeightsData",
        "MissionGoalType",
        mission_goal_type_enum_shape,
    ),
    enum_rule("PlayerTitleData", "TitleType", title_type_enum_shape),
    enum_rule("TutorialData", "Type", tutorial_type_enum_shape),
    enum_rule(
        "TutorialData",
        "ConditionIdsRelation",
        tutorial_condition_ids_relation_enum_shape,
    ),
    enum_rule(
        "TutorialData",
        "Classification",
        tutorial_classification_enum_shape,
    ),
    enum_rule(
        "TutorialData",
        "PromptStyle",
        tutorial_prompt_style_enum_shape,
    ),
    enum_rule(
        "TutorialData",
        "ExitDuration",
        tutorial_prompt_exit_duration_enum_shape,
    ),
    enum_rule(
        "TutorialConditionData",
        "Operation",
        tutorial_condition_operation_enum_shape,
    ),
    enum_rule("ProgressionPoolData", "Category", pool_category_enum_shape),
];

const fn enum_rule(
    row_type_name: &'static str,
    column_name: &'static str,
    factory: EnumShapeFactory,
) -> ColumnRule<EnumShapeFactory> {
    ColumnRule::exact(row_type_name, column_name, factory)
}

pub(in crate::game_system_schema) fn reflected_u8_enum_shape(
    name: &str,
    variants: impl IntoIterator<Item = (&'static str, i64)>,
) -> GameSystemEnumShape {
    GameSystemEnumShape {
        name: name.to_owned(),
        representation: GameSystemEnumRepresentation::U8,
        variants: variants
            .into_iter()
            .map(|(name, discriminant)| GameSystemEnumVariant {
                name: name.to_owned(),
                discriminant,
                source_tokens: vec![name.to_owned()],
            })
            .collect(),
    }
}

pub(in crate::game_system_schema) fn seasons_tracked_stat_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "SeasonsTrackedStatType",
        [
            ("Invalid", 0),
            ("Kill", 1),
            ("Craft", 2),
            ("Fishing", 3),
            ("Salvage", 4),
            ("CompleteQuest", 5),
            ("CompleteJourneyTask", 6),
            ("CompleteSong", 7),
            ("ObtainLevel", 8),
            ("CaptureFcp", 9),
            ("CompleteExpedition", 10),
            ("CompleteDuel", 11),
            ("CompleteOutpostRush", 12),
            ("CompleteArena", 13),
            ("CompleteActivityCard", 14),
            ("CompleteWar", 15),
            ("GameEvent", 16),
            ("Achievement", 17),
            ("CommitResources", 18),
            ("CategoricalProgression", 19),
            ("Contribution", 20),
            ("Combat", 21),
            ("Consume", 22),
            ("Gather", 23),
            ("EquipItem", 24),
        ],
    )
}

pub(in crate::game_system_schema) fn game_event_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "GameEventType",
        [
            ("None", 0),
            ("Crafting", 1),
            ("Gathering", 2),
            ("War", 3),
            ("Invasion", 4),
            ("OutpostRush", 5),
            ("Darkness", 6),
            ("Arena", 7),
            ("PvPKill", 8),
            ("PvPArenas", 9),
            ("EventEncounter", 10),
            ("Scenario", 11),
            ("EliteTrialCompletion", 12),
            ("SeasonTrialCompletion", 13),
        ],
    )
}

pub(in crate::game_system_schema) fn tradeskill_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "TradeskillType",
        [
            ("Weaponsmithing", 0),
            ("Armoring", 1),
            ("Jewelcrafting", 2),
            ("Arcana", 3),
            ("Cooking", 4),
            ("Furnishing", 5),
            ("Engineering", 6),
            ("Smelting", 7),
            ("Woodworking", 8),
            ("Leatherworking", 9),
            ("Weaving", 10),
            ("Stonecutting", 11),
            ("Skinning", 12),
            ("Mining", 13),
            ("Logging", 14),
            ("Harvesting", 15),
            ("WildernessSurvival", 16),
            ("Fishing", 17),
            ("AzothStaff", 18),
            ("Musician", 19),
            ("Riding", 20),
            ("None", 255),
        ],
    )
}

pub(in crate::game_system_schema) fn game_mode_participant_result_enum_shape() -> GameSystemEnumShape
{
    reflected_u8_enum_shape(
        "GameModeParticipantResult",
        [
            ("Unknown", 0),
            ("Victory", 1),
            ("Defeat", 2),
            ("Tie", 3),
            ("FlawlessVictory", 4),
        ],
    )
}

pub(in crate::game_system_schema) fn seasons_chapter_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "SeasonsChapterType",
        [
            ("Invalid", 0),
            ("Chapter", 1),
            ("Challenge", 2),
            ("SeasonalServerChapter", 3),
            ("SeasonalServerChallenge", 4),
        ],
    )
}

pub(in crate::game_system_schema) fn musical_performance_pattern_enum_shape() -> GameSystemEnumShape
{
    reflected_u8_enum_shape(
        "MusicalPerformancePattern",
        [
            ("Novice", 0),
            ("Skilled", 1),
            ("Expert", 2),
            ("NumPatterns", 3),
        ],
    )
}

pub(in crate::game_system_schema) fn seasons_mutator_rank_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape("MutatorRank", [("Bronze", 0), ("Silver", 1), ("Gold", 2)])
}

pub(in crate::game_system_schema) fn seasons_rewards_reward_type_enum_shape() -> GameSystemEnumShape
{
    reflected_u8_enum_shape(
        "SeasonsRewardsRewardType",
        [
            ("Unknown", 0),
            ("BattlePass_Premium", 1),
            ("BattlePass_Free", 2),
            ("Journey", 3),
            ("Chapter", 4),
            ("SeasonXp", 5),
            ("ActivityReroll", 6),
            ("PurchasedLevelConsumption", 7),
            ("NumTypes", 8),
        ],
    )
}

pub(in crate::game_system_schema) fn seasons_tracked_stat_reason_enum_shape() -> GameSystemEnumShape
{
    reflected_u8_enum_shape(
        "SeasonsTrackedStatReason",
        [
            ("ClearingFailedEntries", 1),
            ("Debug", 2),
            ("UnitTest", 3),
            ("ConsoleCommand", 4),
            ("Imgui", 5),
            ("TradeSkillGain", 6),
            ("SetTradeSkill", 7),
            ("ProgressionSet", 8),
            ("BotCraftItem", 9),
            ("BotTask", 10),
            ("BotRepairItem", 11),
            ("ContributionReward", 12),
            ("Respec", 13),
            ("PassiveAbilityActivated", 14),
            ("CraftingIngredient", 15),
            ("Salvage", 16),
            ("Crafting", 17),
            ("Repair", 18),
            ("PvpKill", 19),
            ("Event", 20),
            ("DungeonComplete", 21),
            ("RejoinedRaid", 22),
            ("FastTravel", 23),
            ("ResetFastTravelCooldown", 24),
            ("HouseBonus", 25),
            ("PurchaseReward", 26),
            ("SaltRefund", 27),
            ("CurrencyConversion", 28),
            ("CurrencyConversionAdd", 29),
            ("CurrencyConversionSubtract", 30),
            ("ObjectiveReward", 31),
            ("SaltFromXp", 32),
            ("SeasonsXpDisplayOnly", 33),
            ("SeasonRewardActivityReroll", 34),
            ("SeasonsXpNotModifiable", 35),
            ("SeasonsXpFromXp", 36),
        ],
    )
}

pub(in crate::game_system_schema) fn scoreboard_row_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "ScoreboardRowType",
        [("Invalid", 0), ("PlayerBased", 1), ("StatBased", 2)],
    )
}

pub(in crate::game_system_schema) fn scoreboard_stat_source_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "ScoreboardStatSource",
        [("Invalid", 0), ("WarboardStat", 1), ("Catacombs", 2)],
    )
}

pub(in crate::game_system_schema) fn scoreboard_tab_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "ScoreboardTab",
        [
            ("All", 0),
            ("Personal", 4),
            ("Group", 3),
            ("Allies", 1),
            ("Enemies", 2),
            ("Overview", 5),
        ],
    )
}

pub(in crate::game_system_schema) fn scoreboard_sort_mode_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape("ScoreboardSortMode", [("Ascending", 0), ("Descending", 1)])
}

pub(in crate::game_system_schema) fn stat_multiplier_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "StatMultiplierType",
        [
            ("MaxHealth", 1),
            ("HealthRate", 2),
            ("HealMod", 3),
            ("ConsumableHealMod", 4),
            ("NonConsumableHealMod", 5),
            ("DmgPctToHealthHealMod", 6),
            ("FoodBurn", 7),
            ("MaxFood", 8),
            ("DrinkBurn", 9),
            ("MaxDrink", 10),
            ("StaminaRate", 11),
            ("MaxStamina", 12),
            ("MaxGrit", 13),
            ("ComfortRange", 14),
            ("Luck", 15),
            ("BountyBonus", 16),
            ("BlacksmithingChance", 17),
            ("OutfitterChance", 18),
            ("CookingChance", 19),
            ("AlchemyChance", 20),
            ("WorkshopChance", 21),
            ("ConstructionEfficiency", 22),
            ("WoodsmanChance", 23),
            ("BrewingChance", 24),
            ("HorticultureChance", 25),
            ("MiningChance", 26),
            ("ConsumptionSpeed", 27),
            ("FishingChance", 28),
            ("DeathsDoorTime", 29),
            ("DeathsDoorDelay", 30),
            ("HelpUpHealthPct", 31),
            ("OneHandSwordDamage", 32),
            ("TwoHandSwordDamage", 33),
            ("OneHandAxeDamage", 34),
            ("TwoHandAxeDamage", 35),
            ("OneHandSpearDamage", 36),
            ("TwoHandSpearDamage", 37),
            ("OneHandClubDamage", 38),
            ("TwoHandClubDamage", 39),
            ("OneHandPickDamage", 40),
            ("TwoHandPickDamage", 41),
            ("KnifeDamage", 42),
            ("RifleDamage", 43),
            ("PistolDamage", 44),
            ("BowDamage", 45),
            ("UnarmedDamage", 46),
            ("StaminaDamage", 47),
            ("ManaRate", 48),
            ("MaxMana", 49),
            ("BlockStability", 50),
            ("ManaCost", 51),
            ("MoveSpeedMod", 52),
            ("SprintSpeedMod", 53),
            ("FastTravelEncumbrance", 54),
            ("FastTravelInnCooldown", 55),
            ("FastTravelAzothCost", 56),
            ("FishSize", 57),
            ("FishRarity", 58),
            ("MaxCastDistance", 59),
            ("FishingLineStrength", 60),
            ("PhysicalArmorMaxHealth", 61),
            ("RepairDustYield", 62),
            ("ToolDurabilityLoss", 63),
            ("AzothMod", 64),
            ("FactionReputationMod", 65),
            ("FactionTokensMod", 66),
            ("LootTableDiverted", 67),
            ("SummerFishRarity", 68),
            ("TerritoryStandingMod", 69),
            ("MaxMountStaminaMod", 70),
            ("MountStaminaRate", 71),
            ("MountOnRoadDashSpeedMod", 72),
            ("MountOffRoadDashSpeedMod", 73),
            ("MountSummonTimeMod", 74),
            ("FallDamageMod", 75),
            ("MountBonusEncumbranceMod", 76),
            ("MountStaminaDrainRate", 77),
            ("BaseDamage", 78),
            ("CooldownReduction", 79),
            ("ReviveSpeed", 80),
            ("BeingRevivedSpeed", 81),
            ("ReviveHealthGainedInPctOfMaxHealth", 82),
            ("ReviveHealthGivenInPctOfMaxHealth", 83),
            ("AoeSpellRadiusScaling", 84),
        ],
    )
}

pub(in crate::game_system_schema) fn leaderboard_rotations_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "LeaderboardRotations",
        [("Season", 0), ("Week", 1), ("Month", 2)],
    )
}

pub(in crate::game_system_schema) fn leaderboard_scope_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "LeaderboardScope",
        [
            ("Character", 0),
            ("Group", 1),
            ("Company", 2),
            ("Global", 3),
        ],
    )
}

pub(in crate::game_system_schema) fn leaderboard_stat_aggregations_enum_shape()
-> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "LeaderboardStatAggregations",
        [("Sum", 0), ("Min", 1), ("Max", 2)],
    )
}

pub(in crate::game_system_schema) fn leaderboard_stat_types_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "LeaderboardStatTypes",
        [
            ("ExpeditionScore", 1),
            ("ExpeditionClearTime", 2),
            ("TrialCompletions", 3),
            ("TrialClearTime", 4),
            ("DarknessBreachesCompleted", 5),
            ("TerritoryControlScore", 6),
            ("TerritoryControlStreak", 7),
            ("ProgressionXp", 8),
            ("TradeskillLegendaries", 9),
            ("FishScore", 10),
            ("PvpOpenWorldInfluenceEarned", 11),
            ("PvpControlPointCaptures", 12),
            ("AttackerWins", 13),
            ("DefenderWins", 14),
            ("Wins", 15),
            ("Losses", 16),
            ("Score", 17),
            ("Kills", 18),
            ("NpcKills", 19),
            ("PlayerKills", 20),
            ("Deaths", 21),
            ("Heals", 22),
            ("DamageTaken", 23),
            ("DamageDealt", 24),
            ("Resources", 25),
            ("CompanyWins", 26),
            ("CompanyAttackerWins", 27),
            ("CompanyDefenderWins", 28),
            ("GameModeFinalKills", 29),
            ("GameModeFinalNpcKills", 30),
            ("GameModeFinalPlayerKills", 31),
            ("GameModeFinalDeaths", 32),
            ("GameModeFinalHeals", 33),
            ("GameModeFinalAlliedHeals", 34),
            ("GameModeFinalDamageTaken", 35),
            ("GameModeFinalDamageDealt", 36),
            ("GameModeFinalPlayerTakedowns", 38),
            ("GameEvent", 39),
            ("GameModeFinalResources", 37),
            ("PvpOpenWorldModifiedInfluenceEarned", 40),
            ("PvpExp", 41),
            ("SeasonalPvpExp", 42),
            ("SeasonalConquerorCrafting", 43),
            ("SeasonalTerritoryControlScore", 44),
            ("SeasonalTerritoryControlStreak", 45),
            ("PlayerAssists", 46),
            ("FlagPoints", 47),
            ("FlagReturns", 48),
            ("FlagCarriersKilled", 49),
        ],
    )
}

pub(in crate::game_system_schema) fn warboard_stat_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "WarboardStatType",
        [
            ("DamageToPlayers", 7),
            ("DamageToAI", 8),
            ("DamageToOther", 9),
            ("DamageWithSiegeWeapons", 10),
            ("DamageFromWeapons", 13),
            ("DamageFromSiegeWeapons", 14),
            ("KillsWithWeapons", 11),
            ("KillsWithSiegeWeapons", 12),
            ("SiegeWeaponsDestroyed", 15),
            ("HealingDoneToSelf", 19),
            ("HealingDoneToAllies", 20),
            ("HealingDone", 4),
            ("AlliesRevived", 17),
            ("RepairsDone", 18),
            ("SiegeWeaponReloaded", 16),
            ("Deaths", 6),
            ("Assists", 5),
            ("PlayerKills", 22),
            ("AIKills", 23),
            ("InfusedWoodGathered", 33),
            ("InfusedOreGathered", 34),
            ("InfusedHideGathered", 35),
            ("AzothEssenceGathered", 36),
            ("InfusedWoodDeposited", 29),
            ("InfusedOreDeposited", 30),
            ("InfusedHideDeposited", 31),
            ("AzothEssenceDeposited", 32),
            ("AIAssists", 24),
            ("PlayerAssists", 25),
            ("TeamFlagCapture", 38),
            ("NeutralFlagCapture", 39),
            ("FlagCarrierKilled", 42),
            ("FlagReturnedToBase", 41),
            ("FlagsTaken", 40),
            ("BossesKilled", 44),
            ("NamedEnemiesKilled", 45),
            ("ChestsOpened", 46),
            ("Extracted", 47),
            ("SilversExtracted", 48),
            ("CrownsExtracted", 49),
            ("ChestsOpenedSilvers", 50),
            ("ChestsOpenedCrowns", 51),
            ("BossesSilvers", 52),
            ("BossesCrowns", 53),
            ("NamedEnemiesSilvers", 54),
            ("NamedEnemiesCrowns", 55),
            ("CrownsDeducted", 56),
            ("WeeklyCrownsRemaining", 57),
            ("Score", 0),
            ("TotalDamageDealt", 1),
            ("Kills", 3),
            ("DamageTaken", 2),
            ("AITakedowns", 26),
            ("PlayerTakedowns", 27),
            ("CapturePointContests", 28),
            ("TotalResourcesDeposited", 37),
            ("DamageBlocked", 21),
            ("KDA", 43),
            ("NumStats", 58),
        ],
    )
}

pub(in crate::game_system_schema) fn darkness_threshold_enum_shape() -> GameSystemEnumShape {
    GameSystemEnumShape {
        name: "DarknessThreshold".to_owned(),
        representation: GameSystemEnumRepresentation::I32,
        variants: vec![
            GameSystemEnumVariant {
                name: "None".to_owned(),
                discriminant: 0,
                source_tokens: vec!["None".to_owned()],
            },
            GameSystemEnumVariant {
                name: "Low".to_owned(),
                discriminant: 1,
                source_tokens: vec!["Low".to_owned()],
            },
            GameSystemEnumVariant {
                name: "Medium".to_owned(),
                discriminant: 2,
                source_tokens: vec!["Medium".to_owned()],
            },
            GameSystemEnumVariant {
                name: "High".to_owned(),
                discriminant: 3,
                source_tokens: vec!["High".to_owned()],
            },
        ],
    }
}

pub(in crate::game_system_schema) fn particle_priority_override_enum_shape() -> GameSystemEnumShape
{
    GameSystemEnumShape {
        name: "ParticlePriorityOverride".to_owned(),
        representation: GameSystemEnumRepresentation::U8,
        variants: vec![
            GameSystemEnumVariant {
                name: "Required".to_owned(),
                discriminant: 0,
                source_tokens: vec!["Required".to_owned()],
            },
            GameSystemEnumVariant {
                name: "High".to_owned(),
                discriminant: 1,
                source_tokens: vec!["High".to_owned()],
            },
            GameSystemEnumVariant {
                name: "Normal".to_owned(),
                discriminant: 2,
                source_tokens: vec!["Normal".to_owned()],
            },
            GameSystemEnumVariant {
                name: "Low".to_owned(),
                discriminant: 3,
                source_tokens: vec!["Low".to_owned()],
            },
        ],
    }
}

pub(in crate::game_system_schema) fn reward_milestone_type_enum_shape() -> GameSystemEnumShape {
    GameSystemEnumShape {
        name: "RewardMilestoneType".to_owned(),
        representation: GameSystemEnumRepresentation::U8,
        variants: vec![
            GameSystemEnumVariant {
                name: "None".to_owned(),
                discriminant: 0,
                source_tokens: vec!["minor".to_owned(), "None".to_owned()],
            },
            GameSystemEnumVariant {
                name: "Major".to_owned(),
                discriminant: 1,
                source_tokens: vec!["major".to_owned()],
            },
            GameSystemEnumVariant {
                name: "Challenge".to_owned(),
                discriminant: 2,
                source_tokens: vec!["challenge".to_owned()],
            },
            GameSystemEnumVariant {
                name: "Territory".to_owned(),
                discriminant: 3,
                source_tokens: vec!["territory".to_owned()],
            },
        ],
    }
}

pub(in crate::game_system_schema) fn expansion_id_enum_shape() -> GameSystemEnumShape {
    GameSystemEnumShape {
        name: "ExpansionId".to_owned(),
        representation: GameSystemEnumRepresentation::U8,
        variants: vec![
            GameSystemEnumVariant {
                name: "None".to_owned(),
                discriminant: 0,
                source_tokens: vec!["None".to_owned()],
            },
            GameSystemEnumVariant {
                name: "Expansion2023".to_owned(),
                discriminant: 1,
                source_tokens: vec!["Expansion2023".to_owned()],
            },
            GameSystemEnumVariant {
                name: "Count".to_owned(),
                discriminant: 2,
                source_tokens: vec!["Count".to_owned()],
            },
        ],
    }
}

pub(in crate::game_system_schema) fn mission_goal_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "MissionGoalType",
        [
            ("Gather", 1),
            ("Courier", 2),
            ("Creative", 3),
            ("Kill", 4),
            ("Explore", 5),
            ("Raid", 6),
            ("Loot", 7),
            ("Hunt", 8),
            ("Fish", 9),
            ("Mine", 10),
            ("Harvest", 11),
            ("Log", 12),
            ("Craft", 13),
            ("Espionage", 14),
            ("Intercept", 15),
            ("Control", 16),
            ("Poach", 17),
            ("Expedition_Raid", 18),
            ("Expedition_Loot", 19),
            ("Expedition_Special", 20),
        ],
    )
}

pub(in crate::game_system_schema) fn title_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "TitleType",
        [("Invalid", 0), ("Character", 1), ("Account", 2)],
    )
}

pub(in crate::game_system_schema) fn tutorial_type_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "TutorialType",
        [("Invalid", 0), ("Prompt", 1), ("Dialogue", 2), ("Both", 3)],
    )
}

pub(in crate::game_system_schema) fn tutorial_condition_ids_relation_enum_shape()
-> GameSystemEnumShape {
    reflected_u8_enum_shape("TutorialConditionIdsRelation", [("OR", 0), ("AND", 1)])
}

pub(in crate::game_system_schema) fn tutorial_classification_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "TutorialClassification",
        [("All", 0), ("Core", 1), ("Mandatory", 2)],
    )
}

pub(in crate::game_system_schema) fn tutorial_prompt_style_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "TutorialPromptStyle",
        [
            ("None", 0),
            ("MandatoryActionCenter", 1),
            ("MandatoryActionSide", 2),
            ("VoluntaryAction", 3),
            ("InformationalHint", 4),
            ("ContextualReminder", 5),
            ("NewFeature", 6),
        ],
    )
}

pub(in crate::game_system_schema) fn tutorial_prompt_exit_duration_enum_shape()
-> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "TutorialPromptExitDuration",
        [("None", 0), ("Short", 1), ("Medium", 2), ("Long", 3)],
    )
}

pub(in crate::game_system_schema) fn tutorial_condition_operation_enum_shape() -> GameSystemEnumShape
{
    reflected_u8_enum_shape(
        "TutorialConditionOperation",
        [
            ("Equals", 0),
            ("GreaterThan", 1),
            ("LessThan", 2),
            ("HasUnlocked", 3),
            ("HasNotUnlocked", 4),
            ("EquipItemInAnySlot", 5),
            ("EquipItemClassAny", 6),
            ("EquipItemClassAll", 7),
            ("EquipItemClassNone", 8),
            ("ReceiveItemClassAny", 9),
            ("ReceiveItemClassAll", 10),
            ("ReceiveItemClassNone", 11),
            ("ReceiveItemAnyId", 12),
        ],
    )
}

pub(in crate::game_system_schema) fn pool_category_enum_shape() -> GameSystemEnumShape {
    reflected_u8_enum_shape(
        "PoolCategory",
        [("Invalid", 0), ("Player", 1), ("Territory", 2)],
    )
}
