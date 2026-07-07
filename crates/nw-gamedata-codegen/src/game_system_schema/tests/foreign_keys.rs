use super::*;

#[test]
fn foreign_key_affinity_spreads_exact_family_targets_to_singleton_tables() {
    let data_tables = foreign_key_family_data_tables("Achievement_A");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "Quest_Singleton", "RequiredAchievementId");
    let GameSystemColumnValueShape::String {
        list, foreign_keys, ..
    } = &column.value_shape
    else {
        panic!("expected string column");
    };

    assert_eq!(
        list.as_ref().map(|list| &list.separators),
        Some(&vec!["+".to_owned()])
    );
    assert_eq!(foreign_keys.len(), 1);
    assert_eq!(foreign_keys[0].target_table, "AchievementData");
    assert_eq!(foreign_keys[0].target_column, "AchievementID");
    assert_eq!(foreign_keys[0].checked_values, 1);
    assert_eq!(foreign_keys[0].matched_values, 1);
    assert_eq!(foreign_keys[0].missing_values, 0);
    assert_eq!(foreign_keys[0].confidence, 1.0);
}

#[test]
fn foreign_key_affinity_reports_missing_singleton_values_and_keeps_family_type() {
    let data_tables = foreign_key_family_data_tables("Achievement_Missing");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "Quest_Singleton", "RequiredAchievementId");
    let GameSystemColumnValueShape::String {
        list, foreign_keys, ..
    } = &column.value_shape
    else {
        panic!("expected string column");
    };
    assert_eq!(
        list.as_ref().map(|list| &list.separators),
        Some(&vec!["+".to_owned()])
    );
    assert_eq!(foreign_keys.len(), 1);
    assert_eq!(foreign_keys[0].target_table, "AchievementData");
    assert_eq!(foreign_keys[0].target_column, "AchievementID");
    assert_eq!(foreign_keys[0].checked_values, 1);
    assert_eq!(foreign_keys[0].matched_values, 0);
    assert_eq!(foreign_keys[0].missing_values, 1);
    assert_eq!(foreign_keys[0].confidence, 1.0);

    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.source_table == "Quest_Singleton"
                && diagnostic.source_column == "RequiredAchievementId"
                && diagnostic.value == "Achievement_Missing"
                && matches!(
                    &diagnostic.kind,
                    GameSystemValidationDiagnosticKind::MissingForeignKey {
                        target_table,
                        target_column,
                    } if target_table == "AchievementData" && target_column == "AchievementID"
                )
        })
        .expect("missing foreign-key diagnostic");
    assert_eq!(diagnostic.source_row, "objective_singleton");
    assert_eq!(diagnostic.occurrences, 1);

    let affinity = report_table_affinity(&report, "Quest_Singleton", "RequiredAchievementId");
    assert!(affinity.repairable);
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::ForeignKey && repair.row_index.is_none()
    }));
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::ForeignKey
            && repair.row_index == Some(0)
            && repair.value.as_deref() == Some("Achievement_Missing")
    }));
}

#[test]
fn foreign_key_inference_detects_same_table_key_like_column_targets() {
    let data_tables = test_data_tables(
        "PerkBuckets",
        "PerkBucketData",
        vec![
            ("RowID", ColumnType::String),
            ("BucketAliasID", ColumnType::String),
            ("ParentBucketAliasID", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("row_a".to_owned()),
                OwnedCellValue::String("Bucket_A".to_owned()),
                OwnedCellValue::String(String::new()),
            ],
            vec![
                OwnedCellValue::String("row_b".to_owned()),
                OwnedCellValue::String("Bucket_B".to_owned()),
                OwnedCellValue::String("Bucket_A".to_owned()),
            ],
            vec![
                OwnedCellValue::String("row_c".to_owned()),
                OwnedCellValue::String("Bucket_C".to_owned()),
                OwnedCellValue::String("Bucket_B".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "PerkBuckets", "ParentBucketAliasID");
    let GameSystemColumnValueShape::String { foreign_keys, .. } = &column.value_shape else {
        panic!("expected string column");
    };

    assert_eq!(foreign_keys.len(), 1);
    assert_eq!(foreign_keys[0].target_table, "PerkBucketData");
    assert_eq!(foreign_keys[0].target_column, "BucketAliasID");
    assert_eq!(foreign_keys[0].checked_values, 2);
    assert_eq!(foreign_keys[0].matched_values, 2);
    assert_eq!(foreign_keys[0].missing_values, 0);
    assert_eq!(foreign_keys[0].confidence, 1.0);
}

#[test]
fn foreign_key_inference_detects_cross_table_key_like_column_targets() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "ItemDefinitions",
            1,
            "ItemData",
            vec![
                ("ItemID", ColumnType::String),
                ("ItemLookupName", ColumnType::String),
            ],
            vec![
                vec![
                    OwnedCellValue::String("item_a".to_owned()),
                    OwnedCellValue::String("Sword_Iron".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("item_b".to_owned()),
                    OwnedCellValue::String("Axe_Iron".to_owned()),
                ],
            ],
        ))
        .expect("insert item table");
    data_tables
        .insert(test_table(
            "RewardDefinitions",
            2,
            "RewardData",
            vec![
                ("RewardID", ColumnType::String),
                ("GrantedItemLookupName", ColumnType::String),
            ],
            vec![
                vec![
                    OwnedCellValue::String("reward_a".to_owned()),
                    OwnedCellValue::String("Sword_Iron".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("reward_b".to_owned()),
                    OwnedCellValue::String("Axe_Iron".to_owned()),
                ],
            ],
        ))
        .expect("insert reward table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "RewardDefinitions", "GrantedItemLookupName");
    let GameSystemColumnValueShape::String { foreign_keys, .. } = &column.value_shape else {
        panic!("expected string column");
    };

    assert_eq!(foreign_keys.len(), 1);
    assert_eq!(foreign_keys[0].target_table, "ItemData");
    assert_eq!(foreign_keys[0].target_column, "ItemLookupName");
    assert_eq!(foreign_keys[0].checked_values, 2);
    assert_eq!(foreign_keys[0].matched_values, 2);
    assert_eq!(foreign_keys[0].missing_values, 0);
    assert_eq!(foreign_keys[0].confidence, 1.0);
}

#[test]
fn cooldown_id_references_cooldown_ability_keys() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "Cooldowns_Player",
            1,
            "CooldownData",
            vec![
                ("AbilityID", ColumnType::String),
                ("Time", ColumnType::Number),
                ("MinTime", ColumnType::Number),
                ("WeaponTag", ColumnType::String),
            ],
            vec![
                vec![
                    OwnedCellValue::String("Ability_Bow_PoisonShot".to_owned()),
                    OwnedCellValue::Number(35.0),
                    OwnedCellValue::Number(0.0),
                    OwnedCellValue::String("Bow".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("Ability_Bow_EvadeShot".to_owned()),
                    OwnedCellValue::Number(15.0),
                    OwnedCellValue::Number(0.0),
                    OwnedCellValue::String("Bow".to_owned()),
                ],
            ],
        ))
        .expect("insert cooldown table");
    data_tables
        .insert(test_table(
            "BowAbilityTable",
            2,
            "AbilityData",
            vec![
                ("AbilityID", ColumnType::String),
                ("CooldownId", ColumnType::String),
                ("CooldownDuration", ColumnType::Number),
            ],
            vec![
                vec![
                    OwnedCellValue::String("Ability_Bow_PoisonShot_Upgrade".to_owned()),
                    OwnedCellValue::String("Ability_Bow_PoisonShot".to_owned()),
                    OwnedCellValue::Number(35.0),
                ],
                vec![
                    OwnedCellValue::String("Ability_Bow_EvadeShot_Upgrade".to_owned()),
                    OwnedCellValue::String("Ability_Bow_EvadeShot".to_owned()),
                    OwnedCellValue::Number(15.0),
                ],
            ],
        ))
        .expect("insert ability table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "BowAbilityTable", "CooldownId");
    let GameSystemColumnValueShape::String { foreign_keys, .. } = &column.value_shape else {
        panic!("expected string column");
    };
    assert_eq!(foreign_keys.len(), 1);
    assert_eq!(foreign_keys[0].target_table, "CooldownData");
    assert_eq!(foreign_keys[0].target_column, "AbilityID");
    assert_eq!(foreign_keys[0].checked_values, 2);
    assert_eq!(foreign_keys[0].matched_values, 2);
    assert_eq!(foreign_keys[0].missing_values, 0);
    assert_eq!(foreign_keys[0].confidence, 1.0);

    let affinity = report_table_affinity(&report, "BowAbilityTable", "CooldownId");
    assert!(!affinity.repairable);
}

#[test]
fn foreign_key_inference_detects_numbered_source_columns() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "RewardTrackItems",
            1,
            "RewardTrackItemData",
            vec![("RewardId", ColumnType::String)],
            vec![
                vec![OwnedCellValue::String("LB_BasicArmorFilter".to_owned())],
                vec![OwnedCellValue::String("GE_CoinSmall".to_owned())],
                vec![OwnedCellValue::String("GE_CoinSmall_20".to_owned())],
            ],
        ))
        .expect("insert reward item table");
    data_tables
        .insert(test_table(
            "PvPStore",
            2,
            "PvPStoreData",
            vec![
                ("RowPlaceholders", ColumnType::String),
                ("RewardId1", ColumnType::String),
            ],
            vec![
                vec![
                    OwnedCellValue::String("FIRSTROW".to_owned()),
                    OwnedCellValue::String("LB_BasicArmorFilter".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("DATA1".to_owned()),
                    OwnedCellValue::String("GE_CoinSmall".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("DATA2".to_owned()),
                    OwnedCellValue::String("GE_CoinSmall_20".to_owned()),
                ],
            ],
        ))
        .expect("insert pvp store table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "PvPStore", "RewardId1");
    let GameSystemColumnValueShape::String { foreign_keys, .. } = &column.value_shape else {
        panic!("expected string column");
    };

    assert_eq!(foreign_keys.len(), 1);
    assert_eq!(foreign_keys[0].target_table, "RewardTrackItemData");
    assert_eq!(foreign_keys[0].target_column, "RewardId");
    assert_eq!(foreign_keys[0].checked_values, 3);
    assert_eq!(foreign_keys[0].matched_values, 3);
    assert_eq!(foreign_keys[0].missing_values, 0);
    assert_eq!(foreign_keys[0].confidence, 1.0);
}

#[test]
fn foreign_key_family_target_prefers_broad_high_confidence_coverage_over_tiny_exact_matches() {
    let target = best_foreign_key_family_target(HashMap::from([
        (
            ForeignKeyTarget {
                table_name: "MasterItemDefinitions".to_owned(),
                column_name: "ItemID".to_owned(),
            },
            ForeignKeyFamilyEvidence {
                checked_values: 7_320,
                matched_values: 7_262,
                missing_values: 58,
                source_columns: 13,
            },
        ),
        (
            ForeignKeyTarget {
                table_name: "VariationData".to_owned(),
                column_name: "HouseItemID".to_owned(),
            },
            ForeignKeyFamilyEvidence {
                checked_values: 57,
                matched_values: 57,
                missing_values: 0,
                source_columns: 2,
            },
        ),
    ]))
    .expect("strong family target");

    assert_eq!(target.0.table_name, "MasterItemDefinitions");
    assert_eq!(target.0.column_name, "ItemID");
}
