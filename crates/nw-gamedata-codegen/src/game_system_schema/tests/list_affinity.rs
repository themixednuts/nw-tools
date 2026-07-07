use super::*;

#[test]
fn fishing_behavior_lists_repair_to_crc32_elements() {
    let data_tables = test_data_tables(
        "FishingCatchablesMastersheet",
        "FishingCatchablesData",
        vec![
            ("FishId", ColumnType::String),
            ("FishBehaviors", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("FishA".to_owned()),
            OwnedCellValue::String("Lazy,Sporadic,SporadicEasy".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    assert_eq!(
        list_element_shape(report_column(&report, "FishBehaviors")),
        Some(&GameSystemListElementShape::Crc32)
    );
}

#[test]
fn pug_reward_activity_types_repair_to_crc32_list_elements() {
    let data_tables = test_data_tables(
        "PUGRewards",
        "PUGRewardData",
        vec![
            ("IncentiveID", ColumnType::String),
            ("ActivityTypes", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("Vanilla_RandomExpedition".to_owned()),
            OwnedCellValue::String("Dungeon,Mutation".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let activity_types = report_column(&report, "ActivityTypes");
    let list = string_list(activity_types).expect("activity type list shape");
    assert_eq!(list.separators, vec![",".to_owned()]);
    assert_eq!(
        list.element_shape.as_ref(),
        Some(&GameSystemListElementShape::Crc32)
    );
}

#[test]
fn leaderboard_reward_columns_repair_to_ranked_crc32_pair_lists() {
    let data_tables = test_data_tables(
        "LeaderboardDataTable",
        "LeaderboardData",
        vec![
            ("LeaderboardId", ColumnType::String),
            ("Rewards", ColumnType::String),
            ("ItemRewards", ColumnType::String),
            ("EntitlementRewards", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("SeasonInvasionScore".to_owned()),
            OwnedCellValue::String(
                "10:Leaderboard_InvasionScoreTop1,50:Leaderboard_InvasionScoreTop10".to_owned(),
            ),
            OwnedCellValue::String(String::new()),
            OwnedCellValue::String("10:LB_Invasion_Rank1,50:LB_Invasion_Rank2".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let expected = Some(&GameSystemListElementShape::Pair {
        separator: ':',
        first: GameSystemListAtomShape::Number {
            number_shape: GameSystemNumberShape::NonNegativeInteger,
        },
        second: GameSystemListAtomShape::Crc32,
        default_second_source_token: None,
    });

    for column_name in ["Rewards", "ItemRewards", "EntitlementRewards"] {
        let column = report_column(&report, column_name);
        let list = string_list(column).expect("leaderboard reward list shape");
        assert_eq!(list.separators, vec![",".to_owned()]);
        assert_eq!(list.element_shape.as_ref(), expected);

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn semantic_list_affinity_repairs_mutation_difficulty_loot_tokens_to_crc32_elements() {
    let data_tables = test_data_tables(
        "MutationDifficulty",
        "MutationDifficultyStaticData",
        vec![
            ("MutationDifficulty", ColumnType::Number),
            ("ReqItemsToEnter", ColumnType::String),
            ("InjectedLootTags", ColumnType::String),
            ("InjectedCreatureLoot", ColumnType::String),
            ("InjectedContainerLoot", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::Number(1.0),
            OwnedCellValue::String("KeyA,KeyB".to_owned()),
            OwnedCellValue::String("MutDiff,MutDiff1".to_owned()),
            OwnedCellValue::String("MutatorLoot_Difficulty".to_owned()),
            OwnedCellValue::String("MutatorLoot_Difficulty".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "ReqItemsToEnter",
        "InjectedLootTags",
        "InjectedCreatureLoot",
        "InjectedContainerLoot",
    ] {
        let column = report_column(&report, column_name);
        let list = string_list(column).expect("mutation token list shape");
        assert_eq!(list.separators, vec![",".to_owned()]);
        assert_eq!(
            list.element_shape.as_ref(),
            Some(&GameSystemListElementShape::Crc32)
        );

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn seasons_rewards_stats_repairs_tracked_stat_surface_to_native_semantic_shapes() {
    let data_tables = test_data_tables(
        "SeasonsRewardsStats_Fishing",
        "SeasonsRewardsStats",
        vec![
            ("TrackedStatID", ColumnType::String),
            ("StatType", ColumnType::String),
            ("TargetID", ColumnType::String),
            ("MinWeight", ColumnType::String),
            ("Level", ColumnType::Number),
            ("Precision", ColumnType::Number),
            ("ItemRarity", ColumnType::Number),
            ("RequiredWorldTag", ColumnType::String),
            ("Reasons", ColumnType::String),
            ("Tradeskills", ColumnType::String),
            ("GameModeResult", ColumnType::String),
            ("ItemClass", ColumnType::String),
            ("SongDifficulty", ColumnType::String),
            ("MutatorRank", ColumnType::String),
            ("ChapterType", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("FishingLegendaryWeight".to_owned()),
            OwnedCellValue::String("Fishing".to_owned()),
            OwnedCellValue::String("FishA,FishB".to_owned()),
            OwnedCellValue::String("25.5".to_owned()),
            OwnedCellValue::Number(60.0),
            OwnedCellValue::Number(2.0),
            OwnedCellValue::Number(4.0),
            OwnedCellValue::String("Season_05".to_owned()),
            OwnedCellValue::String("PvpKill,ContributionReward".to_owned()),
            OwnedCellValue::String("Arcana,Fishing".to_owned()),
            OwnedCellValue::String("Victory,Tie".to_owned()),
            OwnedCellValue::String("Weapon+Armor".to_owned()),
            OwnedCellValue::String("Expert".to_owned()),
            OwnedCellValue::String("GOLD".to_owned()),
            OwnedCellValue::String("Chapter".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);

    assert_eq!(
        enum_shape_for_column(&report, "StatType").name,
        "SeasonsTrackedStatType"
    );
    assert_eq!(
        enum_shape_for_column(&report, "SongDifficulty").name,
        "MusicalPerformancePattern"
    );
    assert_eq!(
        enum_shape_for_column(&report, "MutatorRank").name,
        "MutatorRank"
    );
    assert_eq!(
        enum_shape_for_column(&report, "ChapterType").name,
        "SeasonsChapterType"
    );
    assert_eq!(
        report_column(&report, "RequiredWorldTag").value_shape,
        GameSystemColumnValueShape::Crc32
    );
    assert_eq!(
        list_element_shape(report_column(&report, "TargetID")),
        Some(&GameSystemListElementShape::Crc32)
    );
    assert_eq!(
        list_enum_name(list_element_shape(report_column(&report, "Reasons")).unwrap()),
        Some("SeasonsTrackedStatReason")
    );
    assert_eq!(
        list_enum_name(list_element_shape(report_column(&report, "Tradeskills")).unwrap()),
        Some("TradeskillType")
    );
    assert_eq!(
        list_enum_name(list_element_shape(report_column(&report, "GameModeResult")).unwrap()),
        Some("GameModeParticipantResult")
    );

    let item_class_list =
        string_list(report_column(&report, "ItemClass")).expect("item class list");
    assert_eq!(item_class_list.separators, vec!["+".to_owned()]);
    assert_eq!(
        item_class_list.element_shape.as_ref(),
        Some(&GameSystemListElementShape::String)
    );

    assert_eq!(
        number_shape(report_column(&report, "MinWeight")),
        GameSystemNumberShape::Float
    );
    assert_eq!(
        number_shape(report_column(&report, "Level")),
        GameSystemNumberShape::U16
    );
    assert_eq!(
        number_shape(report_column(&report, "Precision")),
        GameSystemNumberShape::U8
    );
    assert_eq!(
        number_shape(report_column(&report, "ItemRarity")),
        GameSystemNumberShape::U8
    );

    for column_name in [
        "StatType",
        "TargetID",
        "MinWeight",
        "RequiredWorldTag",
        "Reasons",
        "Tradeskills",
        "GameModeResult",
    ] {
        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable, "{column_name}");
        assert!(
            affinity
                .repairs
                .iter()
                .any(|repair| { repair.kind == GameSystemColumnTypeRepairKind::SemanticName })
        );
    }
}

#[test]
fn string_lists_record_element_shape() {
    let data_tables = test_data_tables(
        "ElementalMutation",
        "ElementalMutationStaticData",
        vec![
            ("ElementalMutationId", ColumnType::String),
            ("TextColor", ColumnType::String),
            ("BackgroundColor", ColumnType::String),
            ("Description", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("Hellfire_1".to_owned()),
            OwnedCellValue::String("198,67,67".to_owned()),
            OwnedCellValue::String("2,15,23".to_owned()),
            OwnedCellValue::String("Fire element mutator category".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let text_color = report_column(&report, "TextColor");
    let background_color = report_column(&report, "BackgroundColor");
    let description = report_column(&report, "Description");

    assert_eq!(
        list_element_shape(text_color),
        Some(&GameSystemListElementShape::Number {
            number_shape: GameSystemNumberShape::Float,
        })
    );
    assert_eq!(
        list_element_shape(background_color),
        Some(&GameSystemListElementShape::Number {
            number_shape: GameSystemNumberShape::Float,
        })
    );
    assert_eq!(list_element_shape(description), None);
}

#[test]
fn negated_reference_lists_keep_text_element_shape() {
    let data_tables = test_data_tables(
        "TerritoryProgressionMissions",
        "MissionData",
        vec![
            ("MissionID", ColumnType::String),
            ("AvailableTerritoryID", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("MissionA".to_owned()),
                OwnedCellValue::String("5,6".to_owned()),
            ],
            vec![
                OwnedCellValue::String("MissionB".to_owned()),
                OwnedCellValue::String("!16".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let available_territory_id = report_column(&report, "AvailableTerritoryID");

    assert_eq!(
        list_element_shape(available_territory_id),
        Some(&GameSystemListElementShape::String)
    );
}

#[test]
fn expression_columns_do_not_become_lists() {
    let data_tables = test_data_tables(
        "ItemPerkFormula",
        "PerkData",
        vec![
            ("PerkID", ColumnType::String),
            ("Formula", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("PerkA".to_owned()),
            OwnedCellValue::String("BaseValue + ItemPerkBonus(PerkB)".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let formula = report_column(&report, "Formula");

    assert_eq!(list_element_shape(formula), None);
}

#[test]
fn semantic_list_affinity_repairs_dynamic_difficulty_game_mode_ids() {
    let data_tables = test_data_tables(
        "DynamicDifficulty",
        "DynamicDifficultyStaticData",
        vec![
            ("DynamicDifficultyId", ColumnType::String),
            ("GameModeIds", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("difficulty_a".to_owned()),
                OwnedCellValue::String("Catacombs".to_owned()),
            ],
            vec![
                OwnedCellValue::String("difficulty_b".to_owned()),
                OwnedCellValue::String("Catacombs".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "GameModeIds");
    let list = string_list(column).expect("GameModeIds list shape");

    assert_eq!(list.separators, vec![",".to_owned()]);
    assert_eq!(list.rows_with_lists, 0);
    assert_eq!(list.total_entries, 2);
    assert_eq!(
        list.element_shape.as_ref(),
        Some(&GameSystemListElementShape::String)
    );

    let affinity = report_affinity(&report, "GameModeIds");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.85);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_list_affinity_preserves_reusable_scoreboard_column_fields() {
    let data_tables = test_data_tables(
        "ReusableScoreboard",
        "ReusableScoreboardTabData",
        vec![
            ("ReusableScoreboardTabId", ColumnType::String),
            ("Columns", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("Catacombs_Personal".to_owned()),
            OwnedCellValue::String(
                "@ui_catacombs_result,,HigherIsBetter | @ui_catacombs_crowns,,HigherIsBetter"
                    .to_owned(),
            ),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "Columns");
    let list = string_list(column).expect("ReusableScoreboard Columns list shape");

    assert_eq!(list.separators, vec!["|".to_owned()]);
    assert_eq!(list.total_entries, 2);
    assert!(!list.preserve_empty_entries);
    assert_eq!(
        list.element_shape.as_ref(),
        Some(&GameSystemListElementShape::String)
    );

    let affinity = report_affinity(&report, "Columns");
    assert!(affinity.repairable);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_list_affinity_repairs_story_progress_achievement_ids() {
    let data_tables = test_data_tables(
        "StoryProgress",
        "StoryProgressData",
        vec![
            ("AchievementIds", ColumnType::String),
            ("ActivityTaskName", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("09A_M15".to_owned()),
            OwnedCellValue::String("msq_complete".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let achievement_ids = report_column(&report, "AchievementIds");
    let achievement_ids_list = string_list(achievement_ids).expect("AchievementIds list shape");

    assert!(!achievement_ids.row_key);
    assert_eq!(achievement_ids_list.separators, vec![",".to_owned()]);
    assert_eq!(achievement_ids_list.rows_with_lists, 0);
    assert_eq!(achievement_ids_list.total_entries, 1);
    assert_eq!(
        achievement_ids_list.element_shape.as_ref(),
        Some(&GameSystemListElementShape::Crc32)
    );

    let achievement_ids_affinity = report_affinity(&report, "AchievementIds");
    assert!(achievement_ids_affinity.observed_row_key);
    assert!(!achievement_ids_affinity.effective_row_key);
    assert!(achievement_ids_affinity.repairable);
    assert_eq!(
        achievement_ids_affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );

    let activity_task_name = report_column(&report, "ActivityTaskName");
    assert!(matches!(
        activity_task_name.value_shape,
        GameSystemColumnValueShape::Crc32
    ));
    let activity_task_name_affinity = report_affinity(&report, "ActivityTaskName");
    assert!(!activity_task_name_affinity.observed_row_key);
    assert!(!activity_task_name_affinity.effective_row_key);
    assert!(activity_task_name_affinity.repairable);
}

#[test]
fn semantic_list_affinity_repairs_meta_achievement_references() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "AchievementDataTable",
            1,
            "AchievementData",
            vec![("AchievementID", ColumnType::String)],
            vec![
                vec![OwnedCellValue::String("Achievement_A".to_owned())],
                vec![OwnedCellValue::String("Achievement_B".to_owned())],
                vec![OwnedCellValue::String("Achievement_C".to_owned())],
            ],
        ))
        .expect("insert achievement table");
    data_tables
        .insert(test_table(
            "MetaAchievementDataTable",
            2,
            "MetaAchievementData",
            vec![
                ("MetaAchievementId", ColumnType::String),
                ("Predecessor MetaAchievementIds", ColumnType::String),
                ("AchievementsID", ColumnType::String),
            ],
            vec![
                vec![
                    OwnedCellValue::String("Meta_A".to_owned()),
                    OwnedCellValue::String(String::new()),
                    OwnedCellValue::String("Achievement_A".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("Meta_B".to_owned()),
                    OwnedCellValue::String("Meta_A".to_owned()),
                    OwnedCellValue::String("Achievement_B,Achievement_C".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("Meta_C".to_owned()),
                    OwnedCellValue::String("Meta_B".to_owned()),
                    OwnedCellValue::String("Achievement_C".to_owned()),
                ],
            ],
        ))
        .expect("insert meta achievement table");

    let report = infer_data_tables_schema(&data_tables);
    let predecessor_ids = report_table_column(
        &report,
        "MetaAchievementDataTable",
        "Predecessor MetaAchievementIds",
    );
    let predecessor_ids_list =
        string_list(predecessor_ids).expect("predecessor meta achievement list shape");
    let achievements_id =
        report_table_column(&report, "MetaAchievementDataTable", "AchievementsID");
    let achievements_id_list = string_list(achievements_id).expect("achievement id list shape");

    assert_eq!(predecessor_ids_list.separators, vec![",".to_owned()]);
    assert_eq!(predecessor_ids_list.rows_with_lists, 0);
    assert_eq!(
        predecessor_ids_list.element_shape.as_ref(),
        Some(&GameSystemListElementShape::String)
    );
    assert_eq!(achievements_id_list.separators, vec![",".to_owned()]);
    assert_eq!(
        achievements_id_list.element_shape.as_ref(),
        Some(&GameSystemListElementShape::String)
    );

    let GameSystemColumnValueShape::String {
        foreign_keys: predecessor_foreign_keys,
        ..
    } = &predecessor_ids.value_shape
    else {
        panic!("expected string predecessor column");
    };
    assert_eq!(predecessor_foreign_keys.len(), 1);
    assert_eq!(
        predecessor_foreign_keys[0].target_table,
        "MetaAchievementData"
    );
    assert_eq!(
        predecessor_foreign_keys[0].target_column,
        "MetaAchievementId"
    );

    let GameSystemColumnValueShape::String { foreign_keys, .. } = &achievements_id.value_shape
    else {
        panic!("expected string achievements column");
    };
    assert_eq!(foreign_keys.len(), 1);
    assert_eq!(foreign_keys[0].target_table, "AchievementData");
    assert_eq!(foreign_keys[0].target_column, "AchievementID");
}

#[test]
fn semantic_list_affinity_repairs_player_title_achievement_id() {
    let data_tables = test_data_tables(
        "PlayerTitleDataTable",
        "PlayerTitleData",
        vec![
            ("TitleID", ColumnType::String),
            ("AchievementId", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("PlayerTitle_Test".to_owned()),
            OwnedCellValue::String("Achievement_Test".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let achievement_id = report_column(&report, "AchievementId");
    let achievement_id_list = string_list(achievement_id).expect("AchievementId list shape");

    assert!(!achievement_id.row_key);
    assert_eq!(achievement_id_list.separators, vec![",".to_owned()]);
    assert_eq!(achievement_id_list.rows_with_lists, 0);
    assert_eq!(achievement_id_list.total_entries, 1);
    assert_eq!(
        achievement_id_list.element_shape.as_ref(),
        Some(&GameSystemListElementShape::String)
    );

    let achievement_id_affinity = report_affinity(&report, "AchievementId");
    assert!(!achievement_id_affinity.observed_row_key);
    assert!(!achievement_id_affinity.effective_row_key);
    assert!(achievement_id_affinity.repairable);
    assert_eq!(
        achievement_id_affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn notification_number_fields_repairs_to_crc32_list() {
    let data_tables = test_data_tables(
        "Notifications",
        "NotificationData",
        vec![
            ("NotificationId", ColumnType::String),
            ("NumberFields", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("Currency_Sent_To_Player".to_owned()),
            OwnedCellValue::String("amount".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let number_fields = report_column(&report, "NumberFields");
    let list = string_list(number_fields).expect("NumberFields list shape");

    assert_eq!(list.separators, vec![",".to_owned()]);
    assert_eq!(list.rows_with_lists, 0);
    assert_eq!(list.total_entries, 1);
    assert_eq!(
        list.element_shape.as_ref(),
        Some(&GameSystemListElementShape::Crc32)
    );

    let affinity = report_affinity(&report, "NumberFields");
    assert!(affinity.repairable);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_list_affinity_repairs_darkness_structured_specs() {
    let data_tables = test_data_tables(
        "DarknessDataTable",
        "DarknessData",
        vec![
            ("DarknessId", ColumnType::String),
            ("DarknessLevels", ColumnType::String),
            ("DarknessActivationSpec", ColumnType::String),
            ("DarknessGroupSpec", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("darkness".to_owned()),
            OwnedCellValue::String("None-0,Low-25,Medium-50,High-75".to_owned()),
            OwnedCellValue::String("0-72".to_owned()),
            OwnedCellValue::String("0-0,50-1,100-2".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);

    assert_eq!(
        list_element_shape(report_column(&report, "DarknessLevels")),
        Some(&GameSystemListElementShape::Pair {
            separator: '-',
            first: GameSystemListAtomShape::Enum {
                enum_shape: darkness_threshold_enum_shape(),
            },
            second: GameSystemListAtomShape::Number {
                number_shape: GameSystemNumberShape::NonNegativeInteger,
            },
            default_second_source_token: None,
        })
    );
    assert_eq!(
        list_element_shape(report_column(&report, "DarknessActivationSpec")),
        Some(&GameSystemListElementShape::Range {
            bounds: GameSystemRangeBounds::Inclusive,
            number_shape: GameSystemNumberShape::NonNegativeInteger,
        })
    );
    assert_eq!(
        list_element_shape(report_column(&report, "DarknessGroupSpec")),
        Some(&GameSystemListElementShape::Pair {
            separator: '-',
            first: GameSystemListAtomShape::Number {
                number_shape: GameSystemNumberShape::NonNegativeInteger,
            },
            second: GameSystemListAtomShape::Number {
                number_shape: GameSystemNumberShape::NonNegativeInteger,
            },
            default_second_source_token: None,
        })
    );
}

#[test]
fn stat_modifier_attribute_placing_mods_repairs_to_float_list() {
    let data_tables = test_data_tables(
        "StatusEffects",
        "StatusEffectData",
        vec![
            ("StatusID", ColumnType::String),
            ("AttributePlacingMods", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("status_a".to_owned()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("status_b".to_owned()),
                OwnedCellValue::Number(30.0),
            ],
            vec![
                OwnedCellValue::String("status_c".to_owned()),
                OwnedCellValue::String("20,80".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "AttributePlacingMods");
    assert_eq!(
        list_element_shape(column),
        Some(&GameSystemListElementShape::Number {
            number_shape: GameSystemNumberShape::Float,
        })
    );
    assert_eq!(
        string_list(column).expect("semantic list").separators,
        vec![",".to_owned()]
    );

    let affinity = report_affinity(&report, "AttributePlacingMods");
    assert!(affinity.repairable);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn stat_modifier_numeric_origin_list_stays_semantic_list() {
    let data_tables = test_data_tables(
        "AffixStatDataTable",
        "AffixStatData",
        vec![
            ("StatusID", ColumnType::String),
            ("AttributePlacingMods", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("affix_a".to_owned()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("affix_b".to_owned()),
                OwnedCellValue::Number(30.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "AttributePlacingMods");
    assert_eq!(
        list_element_shape(column),
        Some(&GameSystemListElementShape::Number {
            number_shape: GameSystemNumberShape::Float,
        })
    );
    assert_eq!(
        string_list(column).expect("semantic list").separators,
        vec![",".to_owned()]
    );
}

#[test]
fn stat_modifier_pair_list_accepts_comma_separated_rows() {
    let data_tables = test_data_tables(
        "AffixStatDataTable",
        "AffixStatData",
        vec![
            ("StatusID", ColumnType::String),
            ("ABSVitalsCategory", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("affix_a".to_owned()),
                OwnedCellValue::String("Ancient=0.025".to_owned()),
            ],
            vec![
                OwnedCellValue::String("affix_b".to_owned()),
                OwnedCellValue::String("AngryEarth=0.025".to_owned()),
            ],
            vec![
                OwnedCellValue::String("affix_c".to_owned()),
                OwnedCellValue::String("Beast=0.025".to_owned()),
            ],
            vec![
                OwnedCellValue::String("affix_d".to_owned()),
                OwnedCellValue::String("Corrupted=0.025".to_owned()),
            ],
            vec![
                OwnedCellValue::String("affix_e".to_owned()),
                OwnedCellValue::String("Lost=0.025".to_owned()),
            ],
            vec![
                OwnedCellValue::String("affix_bad".to_owned()),
                OwnedCellValue::String("Ancient=0.025,AngryEarth=0.025".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "ABSVitalsCategory");
    assert_eq!(
        list_element_shape(column),
        Some(&GameSystemListElementShape::Pair {
            separator: '=',
            first: GameSystemListAtomShape::Crc32,
            second: GameSystemListAtomShape::Number {
                number_shape: GameSystemNumberShape::Float,
            },
            default_second_source_token: None,
        })
    );
    assert_eq!(
        string_list(column).expect("semantic list").separators,
        vec![",".to_owned()]
    );

    let affinity = report_affinity(&report, "ABSVitalsCategory");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.95);
}

#[test]
fn stat_modifier_pair_list_keeps_nonzero_number_declared_columns_numeric() {
    let data_tables = test_data_tables(
        "StatusEffects_Sword",
        "StatusEffectData",
        vec![
            ("StatusID", ColumnType::String),
            ("DMGVitalsCategory", ColumnType::Number),
            ("ABSVitalsCategory", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("status_a".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("status_b".to_owned()),
                OwnedCellValue::Number(0.1),
                OwnedCellValue::Number(0.2),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in ["DMGVitalsCategory", "ABSVitalsCategory"] {
        let column = report_column(&report, column_name);
        assert_eq!(number_shape(column), GameSystemNumberShape::Float);

        let affinity = report_affinity(&report, column_name);
        assert!(!affinity.repairable);
    }
}

#[test]
fn stat_modifier_formula_columns_repair_to_typed_pair_lists() {
    let data_tables = test_data_tables(
        "StatusEffects",
        "StatusEffectData",
        vec![
            ("StatusID", ColumnType::String),
            ("DMGVitalsCategory", ColumnType::String),
            ("ABSVitalsCategory", ColumnType::Number),
            ("XPIncreases", ColumnType::Number),
            ("StatBonuses", ColumnType::String),
            ("EffectDurationMods", ColumnType::Number),
            ("EffectPotencyMods", ColumnType::Number),
            ("StaminaCostReductions", ColumnType::Number),
            ("ItemClassWeightMods", ColumnType::Number),
            ("StatMultipliers", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("status_a".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("status_b".to_owned()),
                OwnedCellValue::String("Ancient=0.08+Beast=0.1".to_owned()),
                OwnedCellValue::String("Ancient=0.025".to_owned()),
                OwnedCellValue::String("Gathering=0.2".to_owned()),
                OwnedCellValue::String("Strength=1.0".to_owned()),
                OwnedCellValue::String("Burn=0.5".to_owned()),
                OwnedCellValue::String("Poison=0.25".to_owned()),
                OwnedCellValue::String("Dodge=0.15".to_owned()),
                OwnedCellValue::String("Light=0.5+Medium=0.25".to_owned()),
                OwnedCellValue::String("MaxHealth=1.25+Luck=0.5".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let crc32_float_pair = GameSystemListElementShape::Pair {
        separator: '=',
        first: GameSystemListAtomShape::Crc32,
        second: GameSystemListAtomShape::Number {
            number_shape: GameSystemNumberShape::Float,
        },
        default_second_source_token: None,
    };
    for column_name in [
        "DMGVitalsCategory",
        "ABSVitalsCategory",
        "XPIncreases",
        "StatBonuses",
        "EffectDurationMods",
        "EffectPotencyMods",
        "StaminaCostReductions",
    ] {
        let column = report_column(&report, column_name);
        assert_eq!(list_element_shape(column), Some(&crc32_float_pair));
        assert_eq!(
            string_list(column).expect("semantic list").separators,
            vec!["+".to_owned()]
        );

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }

    let stat_multipliers = report_column(&report, "StatMultipliers");
    assert_eq!(
        list_element_shape(stat_multipliers),
        Some(&GameSystemListElementShape::Pair {
            separator: '=',
            first: GameSystemListAtomShape::Enum {
                enum_shape: stat_multiplier_type_enum_shape(),
            },
            second: GameSystemListAtomShape::Number {
                number_shape: GameSystemNumberShape::Float,
            },
            default_second_source_token: None,
        })
    );
    assert_eq!(
        string_list(stat_multipliers)
            .expect("semantic list")
            .separators,
        vec!["+".to_owned()]
    );

    let item_class_weight_mods = report_column(&report, "ItemClassWeightMods");
    assert_eq!(
        list_element_shape(item_class_weight_mods),
        Some(&GameSystemListElementShape::Pair {
            separator: '=',
            first: GameSystemListAtomShape::String,
            second: GameSystemListAtomShape::Number {
                number_shape: GameSystemNumberShape::Float,
            },
            default_second_source_token: None,
        })
    );
}
