use super::*;

#[test]
fn row_type_float_affinity_repairs_variation_weight_only() {
    let mut tables = vec![
        table_schema(
            "HouseItems",
            "VariationData",
            "Weight",
            GameSystemNumberShape::PositiveInteger,
        ),
        table_schema(
            "MissionWeights",
            "MissionWeightsData",
            "Weight",
            GameSystemNumberShape::PositiveInteger,
        ),
    ];
    let affinities = apply_test_type_affinity(&mut tables);

    assert_eq!(
        number_shape(&tables[0].columns[0]),
        GameSystemNumberShape::Float
    );
    assert_eq!(
        number_shape(&tables[1].columns[0]),
        GameSystemNumberShape::PositiveInteger
    );
    assert!(affinities[0].repairable);
    assert_eq!(affinities[0].confidence, 0.85);
    assert!(!affinities[1].repairable);
}

#[test]
fn row_type_float_affinity_repairs_encumbrance_full_threshold() {
    let mut tables = vec![table_schema(
        "EncumbranceDataTable",
        "EncumbranceData",
        "FullWhenEncumbered",
        GameSystemNumberShape::NonNegativeInteger,
    )];
    let affinities = apply_test_type_affinity(&mut tables);

    assert_eq!(
        number_shape(&tables[0].columns[0]),
        GameSystemNumberShape::Float
    );
    assert_eq!(affinities[0].confidence, 0.85);
    assert!(affinities[0].repairable);
    assert_eq!(
        affinities[0].repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_float_affinity_repairs_empty_mutation_perk_bucket_weights() {
    let data_tables = test_data_tables(
        "MutationPerks",
        "MutationPerksStaticData",
        vec![
            ("ElementalMutationTypeId", ColumnType::String),
            ("InjectedPerkBucketWeight1", ColumnType::String),
            ("InjectedPerkBucketWeight2", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("Fire".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
            ],
            vec![
                OwnedCellValue::String("Ice".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);

    for column_name in ["InjectedPerkBucketWeight1", "InjectedPerkBucketWeight2"] {
        let column = report_column(&report, column_name);
        assert_eq!(number_shape(column), GameSystemNumberShape::Float);

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(affinity.confidence, 0.75);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn semantic_float_affinity_repairs_empty_reward_progression_currency_amount() {
    let data_tables = test_data_tables(
        "RewardModifiers",
        "RewardModifierData",
        vec![
            ("Modifiers", ColumnType::String),
            ("ProgressionCurrencyAmount", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("Boss".to_owned()),
                OwnedCellValue::String(String::new()),
            ],
            vec![
                OwnedCellValue::String("Darkness".to_owned()),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "ProgressionCurrencyAmount");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);

    let affinity = report_affinity(&report, "ProgressionCurrencyAmount");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.75);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_number_affinity_repairs_empty_dungeon_tile_weight() {
    let data_tables = test_data_tables(
        "DungeonTile",
        "DungeonTileStaticData",
        vec![
            ("DungeonTileId", ColumnType::String),
            ("Rotations", ColumnType::Number),
            ("TileSize", ColumnType::Number),
            ("Weight", ColumnType::String),
            ("SupportedRoomTypes", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("StraightRoom".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(64.0),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String("Small,Large".to_owned()),
            ],
            vec![
                OwnedCellValue::String("CornerRoom".to_owned()),
                OwnedCellValue::Number(3.0),
                OwnedCellValue::Number(64.0),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String("Small".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let rotations = report_column(&report, "Rotations");
    assert_eq!(number_shape(rotations), GameSystemNumberShape::U8);

    let tile_size = report_column(&report, "TileSize");
    assert_eq!(number_shape(tile_size), GameSystemNumberShape::NonZeroU8);

    let column = report_column(&report, "Weight");
    assert_eq!(
        number_shape(column),
        GameSystemNumberShape::NonNegativeInteger
    );

    let affinity = report_affinity(&report, "Weight");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.75);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );

    let room_types = report_column(&report, "SupportedRoomTypes");
    assert_eq!(
        list_element_shape(room_types),
        Some(&GameSystemListElementShape::Crc32)
    );

    let affinity = report_affinity(&report, "SupportedRoomTypes");
    assert!(affinity.repairable);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn pcg_dungeon_grammar_columns_repair_to_native_shapes() {
    let data_tables = test_data_tables(
        "DungeonGrammar",
        "DungeonGrammarStaticData",
        vec![
            ("GrammaReplacementId", ColumnType::String),
            ("FeatureId", ColumnType::String),
            ("SeedGraph", ColumnType::String),
            ("MinDepth", ColumnType::String),
            ("MaxDepth", ColumnType::String),
            ("ThemeTags", ColumnType::String),
            ("Weight", ColumnType::Number),
            ("GrammarReplacements", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("CycleA_Lost".to_owned()),
            OwnedCellValue::String("Catacombs".to_owned()),
            OwnedCellValue::String("HallwayRoom_1-NamedRoom_1;LoopA_1".to_owned()),
            OwnedCellValue::String("1".to_owned()),
            OwnedCellValue::String("4".to_owned()),
            OwnedCellValue::String("Lost,Catacombs".to_owned()),
            OwnedCellValue::Number(100.0),
            OwnedCellValue::String("CycleA,CycleB".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    assert_eq!(
        report_column(&report, "FeatureId").value_shape,
        GameSystemColumnValueShape::Crc32
    );
    assert!(matches!(
        report_column(&report, "SeedGraph").value_shape,
        GameSystemColumnValueShape::String { list: None, .. }
    ));
    assert_eq!(
        number_shape(report_column(&report, "MinDepth")),
        GameSystemNumberShape::U8
    );
    assert_eq!(
        number_shape(report_column(&report, "MaxDepth")),
        GameSystemNumberShape::U8
    );
    assert_eq!(
        list_element_shape(report_column(&report, "ThemeTags")),
        Some(&GameSystemListElementShape::Crc32)
    );
    assert_eq!(
        list_element_shape(report_column(&report, "GrammarReplacements")),
        Some(&GameSystemListElementShape::Crc32)
    );
}

#[test]
fn pcg_dungeon_room_columns_repair_to_native_shapes() {
    let data_tables = test_data_tables(
        "DungeonRoom",
        "DungeonRoomStaticData",
        vec![
            ("RoomId", ColumnType::String),
            ("FeatureId", ColumnType::String),
            ("RoomType", ColumnType::String),
            ("StartingState", ColumnType::String),
            ("AliasCategory1", ColumnType::String),
            ("AliasTag1", ColumnType::String),
            ("AliasCategory2", ColumnType::String),
            ("AliasTag2", ColumnType::String),
            ("RoomPassthroughCost", ColumnType::Number),
        ],
        vec![vec![
            OwnedCellValue::String("BossRoom".to_owned()),
            OwnedCellValue::String("Catacombs".to_owned()),
            OwnedCellValue::String("EncounterBoss".to_owned()),
            OwnedCellValue::String("StateBoss".to_owned()),
            OwnedCellValue::String("EncounterBoss".to_owned()),
            OwnedCellValue::String("Boss,Catacombs".to_owned()),
            OwnedCellValue::String("EncounterNamedEnemy".to_owned()),
            OwnedCellValue::String("Named,Catacombs".to_owned()),
            OwnedCellValue::Number(3.0),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "FeatureId",
        "RoomType",
        "StartingState",
        "AliasCategory1",
        "AliasCategory2",
    ] {
        assert_eq!(
            report_column(&report, column_name).value_shape,
            GameSystemColumnValueShape::Crc32,
            "{column_name}"
        );
    }
    for column_name in ["AliasTag1", "AliasTag2"] {
        assert_eq!(
            list_element_shape(report_column(&report, column_name)),
            Some(&GameSystemListElementShape::Crc32),
            "{column_name}"
        );
    }
    assert_eq!(
        number_shape(report_column(&report, "RoomPassthroughCost")),
        GameSystemNumberShape::Float
    );
}

#[test]
fn semantic_number_affinity_keeps_string_row_keys_as_lookup_keys() {
    let data_tables = test_data_tables(
        "RewardModifiers",
        "RewardModifierData",
        vec![
            ("Modifiers", ColumnType::String),
            ("Experience", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("Boss".to_owned()),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("Darkness".to_owned()),
                OwnedCellValue::Number(2.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "Modifiers");
    assert!(column.row_key);
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { .. }
    ));
}

#[test]
fn number_affinity_spreads_across_same_row_type_family() {
    let mut tables = vec![
        table_schema(
            "DamageTable",
            "DamageData",
            "ImpactRating",
            GameSystemNumberShape::NonNegativeInteger,
        ),
        table_schema(
            "BossDamageTable",
            "DamageData",
            "ImpactRating",
            GameSystemNumberShape::Float,
        ),
        table_schema(
            "OtherDamageTable",
            "OtherData",
            "ImpactRating",
            GameSystemNumberShape::NonNegativeInteger,
        ),
    ];

    let affinities = apply_test_type_affinity(&mut tables);

    assert_eq!(
        number_shape(&tables[0].columns[0]),
        GameSystemNumberShape::Float
    );
    assert_eq!(
        number_shape(&tables[1].columns[0]),
        GameSystemNumberShape::Float
    );
    assert_eq!(
        number_shape(&tables[2].columns[0]),
        GameSystemNumberShape::NonNegativeInteger
    );
    let repaired = affinities
        .iter()
        .find(|affinity| affinity.table_name == "DamageTable")
        .expect("family repair");
    assert_eq!(repaired.confidence, 0.95);
    assert_eq!(
        repaired.repairs[0].kind,
        GameSystemColumnTypeRepairKind::Family
    );
}

#[test]
fn number_affinity_uses_signed_string_evidence_across_family() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "StatusEffects_Common",
            1,
            "StatusEffectData",
            vec![
                ("StatusEffectId", ColumnType::String),
                ("UiNameplatePriority", ColumnType::Number),
            ],
            vec![
                vec![
                    OwnedCellValue::String("common_a".to_owned()),
                    OwnedCellValue::Number(0.0),
                ],
                vec![
                    OwnedCellValue::String("common_b".to_owned()),
                    OwnedCellValue::Number(1.0),
                ],
            ],
        ))
        .expect("insert common status effects");
    data_tables
        .insert(test_table(
            "StatusEffects_Items",
            2,
            "StatusEffectData",
            vec![
                ("StatusEffectId", ColumnType::String),
                ("UiNameplatePriority", ColumnType::Number),
            ],
            vec![vec![
                OwnedCellValue::String("item_a".to_owned()),
                OwnedCellValue::Number(2.0),
            ]],
        ))
        .expect("insert item status effects");
    data_tables
        .insert(test_table(
            "StatusEffects_AI",
            3,
            "StatusEffectData",
            vec![
                ("StatusEffectId", ColumnType::String),
                ("UiNameplatePriority", ColumnType::String),
            ],
            vec![
                vec![
                    OwnedCellValue::String("ai_a".to_owned()),
                    OwnedCellValue::String("1".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("ai_b".to_owned()),
                    OwnedCellValue::String("-1".to_owned()),
                ],
            ],
        ))
        .expect("insert ai status effects");

    let report = infer_data_tables_schema(&data_tables);
    for table_name in [
        "StatusEffects_Common",
        "StatusEffects_Items",
        "StatusEffects_AI",
    ] {
        let column = report_table_column(&report, table_name, "UiNameplatePriority");
        assert_eq!(number_shape(column), GameSystemNumberShape::Integer);
    }

    let affinity = report_table_affinity(&report, "StatusEffects_AI", "UiNameplatePriority");
    assert!(affinity.repairable);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::Family
    );
}

#[test]
fn mutation_difficulty_hashed_scalar_tokens_repair_to_crc32() {
    let data_tables = test_data_tables(
        "MutationDifficulty",
        "MutationDifficultyStaticData",
        vec![
            ("MutationDifficulty", ColumnType::Number),
            ("HealthIncreaseMod", ColumnType::String),
            ("DamageIncreaseMod", ColumnType::String),
            ("CompletionEvent1", ColumnType::String),
            ("CompletionEvent2", ColumnType::String),
            ("CompletionEvent3", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::Number(1.0),
            OwnedCellValue::String("Mut_HealthScale".to_owned()),
            OwnedCellValue::String("Mut_DamageScale".to_owned()),
            OwnedCellValue::String("MutDiff1T1".to_owned()),
            OwnedCellValue::String("MutDiff1T2".to_owned()),
            OwnedCellValue::String("MutDiff1T3".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "HealthIncreaseMod",
        "DamageIncreaseMod",
        "CompletionEvent1",
        "CompletionEvent2",
        "CompletionEvent3",
    ] {
        let column = report_column(&report, column_name);
        assert_eq!(column.value_shape, GameSystemColumnValueShape::Crc32);

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn crafting_recipe_native_cache_id_columns_repair_to_crc32() {
    let data_tables = test_data_tables(
        "CraftingRecipes",
        "CraftingRecipeData",
        vec![
            ("RecipeID", ColumnType::String),
            ("CraftingCategory", ColumnType::String),
            ("ItemID", ColumnType::String),
            ("Ingredient1", ColumnType::String),
            ("Ingredient2", ColumnType::String),
            ("Qty1", ColumnType::Number),
            ("Qty2", ColumnType::Number),
        ],
        vec![vec![
            OwnedCellValue::String("Recipe_RepairKit".to_owned()),
            OwnedCellValue::String("Repair".to_owned()),
            OwnedCellValue::String("RepairKitT1".to_owned()),
            OwnedCellValue::String("Resource_IngotT1".to_owned()),
            OwnedCellValue::String("CraftingCategory_RepairParts".to_owned()),
            OwnedCellValue::Number(2.0),
            OwnedCellValue::Number(1.0),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    assert_eq!(
        report_column(&report, "RecipeID").value_shape,
        GameSystemColumnValueShape::String {
            identifier_like: true,
            localized_key_like: false,
            asset_path_like: false,
            expression_like: false,
            qualified_reference_like: false,
            list: None,
            foreign_keys: Vec::new(),
        }
    );

    for column_name in ["CraftingCategory", "ItemID", "Ingredient1", "Ingredient2"] {
        let column = report_column(&report, column_name);
        assert_eq!(column.value_shape, GameSystemColumnValueShape::Crc32);

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(affinity.confidence, 0.95);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn fishing_range_columns_repair_to_inclusive_ranges() {
    let data_tables = test_data_tables(
        "FishingCatchablesMastersheet",
        "FishingCatchablesData",
        vec![
            ("FishId", ColumnType::String),
            ("FishWeightRange", ColumnType::String),
            ("FishLengthRange", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("FishA".to_owned()),
                OwnedCellValue::String("0-1".to_owned()),
                OwnedCellValue::Number(57.0),
            ],
            vec![
                OwnedCellValue::String("FishB".to_owned()),
                OwnedCellValue::String("2.5-1.5".to_owned()),
                OwnedCellValue::Number(12.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    assert_eq!(
        range_bounds(report_column(&report, "FishWeightRange")),
        GameSystemRangeBounds::Inclusive
    );
    assert_eq!(
        range_bounds(report_column(&report, "FishLengthRange")),
        GameSystemRangeBounds::Inclusive
    );

    let affinity = report_affinity(&report, "FishWeightRange");
    assert!(affinity.repairable);
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::NativeRangeText
            && repair.to
                == (GameSystemColumnValueShape::Range {
                    bounds: GameSystemRangeBounds::Inclusive,
                    number_shape: GameSystemNumberShape::Float,
                })
    }));
}

#[test]
fn xp_levels_gear_score_columns_repair_to_typed_cells() {
    let data_tables = test_data_tables(
        "XPLevels",
        "ExperienceData",
        vec![
            ("Level Number", ColumnType::Number),
            ("GSLimitT1", ColumnType::String),
            ("GSBonusSolo+", ColumnType::String),
            ("GSBonusGroup+", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::Number(1.0),
                OwnedCellValue::String("100-120".to_owned()),
                OwnedCellValue::String("5".to_owned()),
                OwnedCellValue::String("7".to_owned()),
            ],
            vec![
                OwnedCellValue::Number(2.0),
                OwnedCellValue::String("120-140".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let gear_score_limit = report_column(&report, "GSLimitT1");
    assert_eq!(
        gear_score_limit.value_shape,
        GameSystemColumnValueShape::Range {
            bounds: GameSystemRangeBounds::Inclusive,
            number_shape: GameSystemNumberShape::NonZeroU16,
        }
    );
    assert_eq!(
        number_shape(report_column(&report, "GSBonusSolo+")),
        GameSystemNumberShape::NonNegativeInteger
    );
    assert_eq!(
        number_shape(report_column(&report, "GSBonusGroup+")),
        GameSystemNumberShape::NonNegativeInteger
    );

    let limit_affinity = report_affinity(&report, "GSLimitT1");
    assert!(limit_affinity.repairable);
    assert!(
        limit_affinity
            .repairs
            .iter()
            .any(|repair| repair.kind == GameSystemColumnTypeRepairKind::NativeRangeText)
    );

    let bonus_affinity = report_affinity(&report, "GSBonusSolo+");
    assert!(bonus_affinity.repairable);
    assert!(
        bonus_affinity
            .repairs
            .iter()
            .any(|repair| repair.kind == GameSystemColumnTypeRepairKind::SemanticName)
    );

    let group_bonus_affinity = report_affinity(&report, "GSBonusGroup+");
    assert!(group_bonus_affinity.repairable);
    assert!(
        group_bonus_affinity
            .repairs
            .iter()
            .any(|repair| repair.kind == GameSystemColumnTypeRepairKind::SemanticName)
    );
}

#[test]
fn leaderboard_reward_data_hashed_columns_repair_to_crc32() {
    let data_tables = test_data_tables(
        "LeaderboardRewardsDataTable",
        "LeaderboardRewardsData",
        vec![
            ("LeaderboardRewardId", ColumnType::String),
            ("LeaderboardRewardIdNoRotation", ColumnType::String),
            ("EntitlementRewards", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("LB_Invasion_Rank1_Season1".to_owned()),
            OwnedCellValue::String("LB_Invasion_Rank1".to_owned()),
            OwnedCellValue::String("EntitlementSkin_Chest_Rank1_LB_Invasion".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in ["LeaderboardRewardIdNoRotation", "EntitlementRewards"] {
        let column = report_column(&report, column_name);
        assert_eq!(column.value_shape, GameSystemColumnValueShape::Crc32);

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn semantic_float_affinity_repairs_dynamic_difficulty_potency_columns() {
    let data_tables = test_data_tables(
        "DynamicDifficulty",
        "DynamicDifficultyStaticData",
        vec![
            ("DynamicDifficultyId", ColumnType::String),
            ("StatusEffect_1_Potency_Catacombs", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("difficulty_a".to_owned()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("difficulty_b".to_owned()),
                OwnedCellValue::Number(75.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "StatusEffect_1_Potency_Catacombs");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);

    let affinity = report_affinity(&report, "StatusEffect_1_Potency_Catacombs");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.85);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_float_affinity_repairs_mutation_base_curse_damage_mod_zero_column() {
    let data_tables = test_data_tables(
        "MutationDifficulty",
        "MutationDifficultyStaticData",
        vec![
            ("MutationDifficulty", ColumnType::Number),
            ("BaseCurseDamageMod", ColumnType::Number),
        ],
        vec![
            vec![OwnedCellValue::Number(1.0), OwnedCellValue::Number(0.0)],
            vec![OwnedCellValue::Number(2.0), OwnedCellValue::Number(0.0)],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "BaseCurseDamageMod");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);

    let affinity = report_affinity(&report, "BaseCurseDamageMod");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.85);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_float_affinity_repairs_plural_modifier_words() {
    let data_tables = test_data_tables(
        "AffixStatData",
        "AffixStatData",
        vec![
            ("StatusID", ColumnType::String),
            ("AttributeModifiers", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("status_a".to_owned()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("status_b".to_owned()),
                OwnedCellValue::Number(1.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "AttributeModifiers");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);

    let affinity = report_affinity(&report, "AttributeModifiers");
    assert!(affinity.repairable);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_float_affinity_repairs_territory_upkeep_earnings_distribution_tid_columns() {
    let data_tables = test_data_tables(
        "TerritoryUpkeep",
        "TerritoryUpkeepDefinition",
        vec![
            ("Level", ColumnType::Number),
            ("EarningsDistributionTID9", ColumnType::Number),
        ],
        vec![
            vec![OwnedCellValue::Number(1.0), OwnedCellValue::Number(100.0)],
            vec![OwnedCellValue::Number(2.0), OwnedCellValue::Number(0.0)],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "EarningsDistributionTID9");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);

    let affinity = report_affinity(&report, "EarningsDistributionTID9");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.85);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_float_affinity_repairs_parseable_string_cells() {
    let data_tables = test_data_tables(
        "DamageTable",
        "DamageData",
        vec![
            ("DamageId", ColumnType::String),
            ("DmgCoefHead", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::String("1".to_owned()),
            ],
            vec![
                OwnedCellValue::String("damage_b".to_owned()),
                OwnedCellValue::String("1.25".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "DmgCoefHead");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);
    let affinity = report_affinity(&report, "DmgCoefHead");
    assert_eq!(affinity.confidence, 0.80);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}
