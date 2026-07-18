use super::*;

#[test]
fn progression_pool_category_uses_reflected_enum() {
    let data_tables = test_data_tables(
        "ProgressionPools",
        "ProgressionPoolData",
        vec![
            ("ProgressionPoolId", ColumnType::String),
            ("Category", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("sword".to_owned()),
                OwnedCellValue::String("Player".to_owned()),
            ],
            vec![
                OwnedCellValue::String("territory".to_owned()),
                OwnedCellValue::String("Territory".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "Category");
    let GameSystemColumnValueShape::Enum { enum_shape } = &column.value_shape else {
        panic!("expected enum column")
    };
    assert_eq!(enum_shape.name, "PoolCategory");
    assert_eq!(
        enum_shape
            .variants
            .iter()
            .map(|variant| (variant.name.as_str(), variant.discriminant))
            .collect::<Vec<_>>(),
        [("Invalid", 0), ("Player", 1), ("Territory", 2)]
    );
}

#[test]
fn semantic_enum_affinity_repairs_reward_milestone_type() {
    let data_tables = test_data_tables(
        "RewardMilestones",
        "RewardMilestoneData",
        vec![
            ("RewardID", ColumnType::String),
            ("MilestoneType", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("reward_a".to_owned()),
                OwnedCellValue::String("major".to_owned()),
            ],
            vec![
                OwnedCellValue::String("reward_b".to_owned()),
                OwnedCellValue::String("minor".to_owned()),
            ],
            vec![
                OwnedCellValue::String("reward_c".to_owned()),
                OwnedCellValue::String("territory".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "MilestoneType");
    let GameSystemColumnValueShape::Enum { enum_shape } = &column.value_shape else {
        panic!("expected enum column")
    };
    assert_eq!(enum_shape.name, "RewardMilestoneType");
    assert_eq!(enum_shape.representation, GameSystemEnumRepresentation::U8);

    let affinity = report_affinity(&report, "MilestoneType");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.95);
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::SemanticName
            && repair.to == column.value_shape
    }));
}

#[test]
fn semantic_enum_affinity_repairs_particle_priority_override() {
    let data_tables = test_data_tables(
        "ParticleContextualPriorityOverrides",
        "ParticleContextualPriorityOverrideData",
        vec![
            ("EffectName", ColumnType::String),
            ("PriorityOverride", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("fx_a".to_owned()),
                OwnedCellValue::String("Required".to_owned()),
            ],
            vec![
                OwnedCellValue::String("fx_b".to_owned()),
                OwnedCellValue::String("High".to_owned()),
            ],
            vec![
                OwnedCellValue::String("fx_c".to_owned()),
                OwnedCellValue::String("Normal".to_owned()),
            ],
            vec![
                OwnedCellValue::String("fx_d".to_owned()),
                OwnedCellValue::String("Low".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "PriorityOverride");
    let GameSystemColumnValueShape::Enum { enum_shape } = &column.value_shape else {
        panic!("expected enum column")
    };
    assert_eq!(enum_shape.name, "ParticlePriorityOverride");
    assert_eq!(enum_shape.representation, GameSystemEnumRepresentation::U8);
    assert_eq!(
        enum_shape
            .variants
            .iter()
            .map(|variant| (variant.name.as_str(), variant.discriminant))
            .collect::<Vec<_>>(),
        [("Required", 0), ("High", 1), ("Normal", 2), ("Low", 3),]
    );

    let affinity = report_affinity(&report, "PriorityOverride");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.95);
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::SemanticName
            && repair.to == column.value_shape
    }));
}

#[test]
fn semantic_enum_affinity_repairs_leaderboard_stat_reflected_enums() {
    let data_tables = test_data_tables(
        "LeaderboardStatDataTable",
        "LeaderboardStatData",
        vec![
            ("LeaderboardStatId", ColumnType::String),
            ("Rotation", ColumnType::String),
            ("StatType", ColumnType::String),
            ("Aggregation", ColumnType::String),
            ("Scope", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("lb_stat_a".to_owned()),
                OwnedCellValue::String("Season".to_owned()),
                OwnedCellValue::String("ExpeditionScore".to_owned()),
                OwnedCellValue::String("Sum".to_owned()),
                OwnedCellValue::String("Character".to_owned()),
            ],
            vec![
                OwnedCellValue::String("lb_stat_b".to_owned()),
                OwnedCellValue::String("Week".to_owned()),
                OwnedCellValue::String("GameModeFinalResources".to_owned()),
                OwnedCellValue::String("Max".to_owned()),
                OwnedCellValue::String("Company".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for (column_name, enum_name) in [
        ("Rotation", "LeaderboardRotations"),
        ("StatType", "LeaderboardStatTypes"),
        ("Aggregation", "LeaderboardStatAggregations"),
        ("Scope", "LeaderboardScope"),
    ] {
        let column = report_column(&report, column_name);
        let GameSystemColumnValueShape::Enum { enum_shape } = &column.value_shape else {
            panic!("expected enum column for {column_name}")
        };
        assert_eq!(enum_shape.name, enum_name);
        assert_eq!(enum_shape.representation, GameSystemEnumRepresentation::U8);

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(affinity.confidence, 0.95);
    }

    let stat_type = enum_shape_for_column(&report, "StatType");
    assert!(
        stat_type.variants.iter().any(|variant| {
            variant.name == "GameModeFinalResources" && variant.discriminant == 37
        })
    );
    assert!(stat_type.variants.iter().any(|variant| {
        variant.name == "GameModeFinalPlayerTakedowns" && variant.discriminant == 38
    }));
    assert!(
        stat_type
            .variants
            .iter()
            .any(|variant| variant.name == "GameEvent" && variant.discriminant == 39)
    );
}

#[test]
fn semantic_enum_affinity_repairs_leaderboard_data_rotation() {
    let data_tables = test_data_tables(
        "LeaderboardDataTable",
        "LeaderboardData",
        vec![
            ("LeaderboardId", ColumnType::String),
            ("Rotation", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("lb_a".to_owned()),
            OwnedCellValue::String("Month".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let enum_shape = enum_shape_for_column(&report, "Rotation");
    assert_eq!(enum_shape.name, "LeaderboardRotations");

    let affinity = report_affinity(&report, "Rotation");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.95);
}

#[test]
fn semantic_enum_affinity_counts_none_as_expansion_id_value() {
    let data_tables = test_data_tables(
        "RewardMilestones",
        "RewardMilestoneData",
        vec![
            ("RewardID", ColumnType::String),
            ("ExpansionIdUnlock", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("reward_a".to_owned()),
                OwnedCellValue::String("None".to_owned()),
            ],
            vec![
                OwnedCellValue::String("reward_b".to_owned()),
                OwnedCellValue::String("Expansion2023".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "ExpansionIdUnlock");
    let GameSystemColumnValueShape::Enum { enum_shape } = &column.value_shape else {
        panic!("expected enum column")
    };
    assert_eq!(enum_shape.name, "ExpansionId");
    assert_eq!(enum_shape.representation, GameSystemEnumRepresentation::U8);
    assert_eq!(column.non_empty_rows, 2);
    assert_eq!(column.empty_rows, 0);

    let affinity = report_affinity(&report, "ExpansionIdUnlock");
    assert_eq!(affinity.confidence, 0.95);
}

#[test]
fn row_key_none_is_not_an_empty_key() {
    let data_tables = test_data_tables(
        "Expansions",
        "ExpansionData",
        vec![
            ("ExpansionId", ColumnType::String),
            ("DisplayName", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("None".to_owned()),
                OwnedCellValue::String("@ui_expansion_base".to_owned()),
            ],
            vec![
                OwnedCellValue::String("Expansion2023".to_owned()),
                OwnedCellValue::String("@ui_expansion_2023".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "ExpansionId");
    assert_eq!(column.non_empty_rows, 2);
    assert_eq!(column.empty_rows, 0);
    assert_eq!(column.distinct_values, 2);
    assert!(column.required);
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { .. }
    ));
}

#[test]
fn expansion_max_columns_repair_to_nonzero_u16() {
    let data_tables = test_data_tables(
        "Expansions",
        "ExpansionData",
        vec![
            ("ExpansionId", ColumnType::String),
            ("MaxDisplayLevel", ColumnType::Number),
            ("MaxCraftGS", ColumnType::Number),
            ("MaxEquipGS", ColumnType::Number),
            ("MaxTradeskillLevel", ColumnType::Number),
        ],
        vec![vec![
            OwnedCellValue::String("Expansion2023".to_owned()),
            OwnedCellValue::Number(70.0),
            OwnedCellValue::Number(700.0),
            OwnedCellValue::Number(800.0),
            OwnedCellValue::Number(250.0),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "MaxDisplayLevel",
        "MaxCraftGS",
        "MaxEquipGS",
        "MaxTradeskillLevel",
    ] {
        let column = report_column(&report, column_name);
        assert_eq!(number_shape(column), GameSystemNumberShape::NonZeroU16);

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(affinity.confidence, 0.85);
    }
}

#[test]
fn expansion_entitlement_id_repairs_to_crc32() {
    let data_tables = test_data_tables(
        "Expansions",
        "ExpansionData",
        vec![
            ("ExpansionId", ColumnType::String),
            ("EntitlementId", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("Expansion2023".to_owned()),
            OwnedCellValue::String("Ent_Xpack_RiseOfTheAngryEarth_Owner".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "EntitlementId");
    assert_eq!(column.value_shape, GameSystemColumnValueShape::Crc32);

    let affinity = report_affinity(&report, "EntitlementId");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.95);
}

#[test]
fn reusable_scoreboard_enum_affinities_use_reflected_types() {
    let data_tables = test_data_tables(
        "ReusableScoreboard",
        "ReusableScoreboardTabData",
        vec![
            ("ReusableScoreboardTabId", ColumnType::String),
            ("RowType", ColumnType::String),
            ("StatSource", ColumnType::String),
            ("TabDataFilter", ColumnType::String),
            ("DefaultColumnSortMode", ColumnType::String),
            ("RankDeterminingStat", ColumnType::String),
            ("StatsToShowAsBlank", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("OutpostRush_Personal".to_owned()),
            OwnedCellValue::String("PlayerBased".to_owned()),
            OwnedCellValue::String("WarboardStat".to_owned()),
            OwnedCellValue::String("Personal".to_owned()),
            OwnedCellValue::String("Descending".to_owned()),
            OwnedCellValue::String("Score".to_owned()),
            OwnedCellValue::String("NumStats,KDA".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let row_type = report_column(&report, "RowType");
    assert_eq!(scalar_enum_name(row_type), Some("ScoreboardRowType"));
    let stat_source = report_column(&report, "StatSource");
    assert_eq!(scalar_enum_name(stat_source), Some("ScoreboardStatSource"));
    let tab_filter = report_column(&report, "TabDataFilter");
    assert_eq!(scalar_enum_name(tab_filter), Some("ScoreboardTab"));
    let sort_mode = report_column(&report, "DefaultColumnSortMode");
    assert_eq!(scalar_enum_name(sort_mode), Some("ScoreboardSortMode"));
    let rank_stat = report_column(&report, "RankDeterminingStat");
    assert_eq!(scalar_enum_name(rank_stat), Some("WarboardStatType"));

    let blank_stats = report_column(&report, "StatsToShowAsBlank");
    let list = string_list(blank_stats).expect("StatsToShowAsBlank list shape");
    assert_eq!(list.separators, vec![",".to_owned()]);
    assert_eq!(
        list.element_shape.as_ref().and_then(list_enum_name),
        Some("WarboardStatType")
    );
}
