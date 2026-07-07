use super::*;

#[test]
fn family_number_affinity_repairs_native_numeric_text_zero_fallbacks() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "DamageTable",
            1,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("CritTargetCameraShakeID", ColumnType::String),
                ("ImpactRating", ColumnType::Number),
            ],
            vec![
                vec![
                    OwnedCellValue::String("damage_a".to_owned()),
                    OwnedCellValue::String(String::new()),
                    OwnedCellValue::Number(0.1),
                ],
                vec![
                    OwnedCellValue::String("damage_b".to_owned()),
                    OwnedCellValue::String(String::new()),
                    OwnedCellValue::Number(3.0),
                ],
            ],
        ))
        .expect("insert numeric damage table");
    data_tables
        .insert(test_table(
            "HumanSpearDamageTable",
            2,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("CritTargetCameraShakeID", ColumnType::String),
                ("ImpactRating", ColumnType::String),
            ],
            vec![
                vec![
                    OwnedCellValue::String("spear_a".to_owned()),
                    OwnedCellValue::String(String::new()),
                    OwnedCellValue::String("3".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("spear_b".to_owned()),
                    OwnedCellValue::String(String::new()),
                    OwnedCellValue::String("Small".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("spear_c".to_owned()),
                    OwnedCellValue::String(String::new()),
                    OwnedCellValue::String("0".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("spear_d".to_owned()),
                    OwnedCellValue::String("Small".to_owned()),
                    OwnedCellValue::String("1".to_owned()),
                ],
            ],
        ))
        .expect("insert repaired damage table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "HumanSpearDamageTable", "ImpactRating");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);

    let affinity = report_table_affinity(&report, "HumanSpearDamageTable", "ImpactRating");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.75);
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::NativeNumericText
            && repair.row_index.is_none()
    }));
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::NativeNumericText
            && repair.row_index == Some(1)
            && repair.value.as_deref() == Some("Small")
    }));
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::AdjacentColumn
            && repair.row_index == Some(1)
            && repair.adjacent_column.as_deref() == Some("CritTargetCameraShakeID")
            && repair.adjacent_direction == Some(GameSystemAdjacentColumnDirection::Left)
    }));
}

#[test]
fn family_number_affinity_keeps_all_native_numeric_text_zero_fallbacks_as_text() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "DamageTable",
            1,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("ImpactRating", ColumnType::Number),
            ],
            vec![vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::Number(0.1),
            ]],
        ))
        .expect("insert numeric damage table");
    data_tables
        .insert(test_table(
            "CorruptedLegion_Cyclops_DamageTable",
            2,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("ImpactRating", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("Bloodshot".to_owned()),
                OwnedCellValue::String("Small".to_owned()),
            ]],
        ))
        .expect("insert repaired damage table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(
        &report,
        "CorruptedLegion_Cyclops_DamageTable",
        "ImpactRating",
    );
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { .. }
    ));

    let affinity = report_table_affinity(
        &report,
        "CorruptedLegion_Cyclops_DamageTable",
        "ImpactRating",
    );
    assert!(!affinity.repairable);
    assert_eq!(affinity.confidence, 1.0);
    assert!(affinity.repairs.is_empty());
}

#[test]
fn family_number_affinity_repairs_empty_string_tables() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "DamageTable",
            1,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("DurabilityCostOverride", ColumnType::Number),
            ],
            vec![vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::Number(0.0),
            ]],
        ))
        .expect("insert numeric damage table");
    data_tables
        .insert(test_table(
            "DamageTable_Perks",
            2,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("DurabilityCostOverride", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("damage_b".to_owned()),
                OwnedCellValue::String(String::new()),
            ]],
        ))
        .expect("insert empty-string damage table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "DamageTable_Perks", "DurabilityCostOverride");
    assert_eq!(
        number_shape(column),
        GameSystemNumberShape::NonNegativeInteger
    );
    let affinity = report_table_affinity(&report, "DamageTable_Perks", "DurabilityCostOverride");
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::Family
    );
}

#[test]
fn family_number_affinity_repairs_empty_id_string_tables() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "WarHammerAbilityTable",
            1,
            "AbilityData",
            vec![
                ("AbilityID", ColumnType::String),
                ("TreeId", ColumnType::Number),
            ],
            vec![
                vec![
                    OwnedCellValue::String("ability_a".to_owned()),
                    OwnedCellValue::Number(0.0),
                ],
                vec![
                    OwnedCellValue::String("ability_b".to_owned()),
                    OwnedCellValue::Number(1.0),
                ],
            ],
        ))
        .expect("insert numeric ability table");
    data_tables
        .insert(test_table(
            "EquipmentSetBonusesAbilityTable",
            2,
            "AbilityData",
            vec![
                ("AbilityID", ColumnType::String),
                ("TreeId", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("ability_empty".to_owned()),
                OwnedCellValue::String(String::new()),
            ]],
        ))
        .expect("insert empty-string ability table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "EquipmentSetBonusesAbilityTable", "TreeId");
    assert_eq!(
        number_shape(column),
        GameSystemNumberShape::NonNegativeInteger
    );
    let affinity = report_table_affinity(&report, "EquipmentSetBonusesAbilityTable", "TreeId");
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::Family
    );
}

#[test]
fn family_number_affinity_repairs_zero_row_string_tables() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "DamageTable",
            1,
            "DamageData",
            vec![
                ("DamageID", ColumnType::String),
                ("ImpactRating", ColumnType::Number),
            ],
            vec![vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::Number(0.1),
            ]],
        ))
        .expect("insert numeric damage table");
    data_tables
        .insert(test_table(
            "DamageTable_Empty",
            2,
            "DamageData",
            vec![
                ("DamageID", ColumnType::String),
                ("ImpactRating", ColumnType::String),
            ],
            Vec::new(),
        ))
        .expect("insert zero-row damage table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "DamageTable_Empty", "ImpactRating");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);
    let affinity = report_table_affinity(&report, "DamageTable_Empty", "ImpactRating");
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::Family
    );
}

#[test]
fn family_number_affinity_keeps_nonempty_text_id_columns_as_text() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "WarHammerAbilityTable",
            1,
            "AbilityData",
            vec![
                ("AbilityID", ColumnType::String),
                ("TreeId", ColumnType::Number),
            ],
            vec![vec![
                OwnedCellValue::String("ability_a".to_owned()),
                OwnedCellValue::Number(1.0),
            ]],
        ))
        .expect("insert numeric ability table");
    data_tables
        .insert(test_table(
            "SharedAbilityTable",
            2,
            "AbilityData",
            vec![
                ("AbilityID", ColumnType::String),
                ("TreeId", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("ability_b".to_owned()),
                OwnedCellValue::String("shared_tree".to_owned()),
            ]],
        ))
        .expect("insert text ability table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "SharedAbilityTable", "TreeId");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { list: None, .. }
    ));
    let affinity = report_table_affinity(&report, "SharedAbilityTable", "TreeId");
    assert!(!affinity.repairable);
}

#[test]
fn family_boolean_affinity_repairs_empty_string_tables() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "HercyneTyphonDamageTable",
            1,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("EnableSharedDamage", ColumnType::Boolean),
            ],
            vec![vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::Boolean(true),
            ]],
        ))
        .expect("insert boolean damage table");
    data_tables
        .insert(test_table(
            "DamageTable",
            2,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("EnableSharedDamage", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("damage_b".to_owned()),
                OwnedCellValue::String(String::new()),
            ]],
        ))
        .expect("insert empty-string damage table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "DamageTable", "EnableSharedDamage");
    assert_eq!(column.value_shape, GameSystemColumnValueShape::Boolean);
    let affinity = report_table_affinity(&report, "DamageTable", "EnableSharedDamage");
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::Family
    );
}

#[test]
fn family_boolean_affinity_repairs_empty_string_tables_without_semantic_name() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "DefaultItemTransforms",
            1,
            "ItemTransform",
            vec![
                ("FromItemId", ColumnType::String),
                ("KeepPerks", ColumnType::Boolean),
            ],
            vec![vec![
                OwnedCellValue::String("item_a".to_owned()),
                OwnedCellValue::Boolean(true),
            ]],
        ))
        .expect("insert boolean item-transform table");
    data_tables
        .insert(test_table(
            "GameModesItemTransforms",
            2,
            "ItemTransform",
            vec![
                ("FromItemId", ColumnType::String),
                ("KeepPerks", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("item_b".to_owned()),
                OwnedCellValue::String(String::new()),
            ]],
        ))
        .expect("insert empty-string item-transform table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "GameModesItemTransforms", "KeepPerks");
    assert_eq!(column.value_shape, GameSystemColumnValueShape::Boolean);
    let affinity = report_table_affinity(&report, "GameModesItemTransforms", "KeepPerks");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.90);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::Family
    );
}

#[test]
fn family_boolean_affinity_keeps_non_boolean_text_as_text() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "DamageTable",
            1,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("IsRanged", ColumnType::Boolean),
            ],
            vec![vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::Boolean(true),
            ]],
        ))
        .expect("insert boolean damage table");
    data_tables
        .insert(test_table(
            "DungeonDamageTable",
            2,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("IsRanged", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("SunTurretBeam".to_owned()),
                OwnedCellValue::String("Fire".to_owned()),
            ]],
        ))
        .expect("insert repaired damage table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "DungeonDamageTable", "IsRanged");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { .. }
    ));

    let affinity = report_table_affinity(&report, "DungeonDamageTable", "IsRanged");
    assert!(!affinity.repairable);
}

#[test]
fn paired_bound_affinity_repairs_stat_multiplier_min_to_float() {
    let data_tables = test_data_tables(
        "StatMultiplierTable",
        "StatMultiplierData",
        vec![
            ("ID", ColumnType::String),
            ("Min", ColumnType::Number),
            ("Max", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("Strength".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.5),
            ],
            vec![
                OwnedCellValue::String("Dexterity".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let min = report_column(&report, "Min");
    let max = report_column(&report, "Max");
    assert_eq!(number_shape(min), GameSystemNumberShape::Float);
    assert_eq!(number_shape(max), GameSystemNumberShape::Float);

    let affinity = report_affinity(&report, "Min");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.90);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn family_numeric_affinity_repairs_incidental_string_list_outliers() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "MasterItemDefinitions_Common",
            1,
            "MasterItemDefinitions",
            vec![
                ("ItemID", ColumnType::String),
                ("MaxStackSize", ColumnType::Number),
            ],
            vec![
                vec![
                    OwnedCellValue::String("ItemA".to_owned()),
                    OwnedCellValue::Number(1.0),
                ],
                vec![
                    OwnedCellValue::String("ItemB".to_owned()),
                    OwnedCellValue::Number(0.0),
                ],
            ],
        ))
        .expect("insert numeric master items table");
    data_tables
        .insert(test_table(
            "MasterItemDefinitions_PVP",
            2,
            "MasterItemDefinitions",
            vec![
                ("ItemID", ColumnType::String),
                ("MaxStackSize", ColumnType::String),
            ],
            vec![
                vec![
                    OwnedCellValue::String("PvpItemA".to_owned()),
                    OwnedCellValue::String("1".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("PvpItemB".to_owned()),
                    OwnedCellValue::String("2".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("PvpItemC".to_owned()),
                    OwnedCellValue::String("3".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("PvpItemD".to_owned()),
                    OwnedCellValue::String("4".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("PvpItemE".to_owned()),
                    OwnedCellValue::String("5+5".to_owned()),
                ],
            ],
        ))
        .expect("insert string master items table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "MasterItemDefinitions_PVP", "MaxStackSize");
    let affinity = report_table_affinity(&report, "MasterItemDefinitions_PVP", "MaxStackSize");

    assert_eq!(
        number_shape(column),
        GameSystemNumberShape::NonNegativeInteger
    );
    assert!(affinity.repairable);
    assert!(matches!(
        affinity.observed_shape,
        GameSystemColumnValueShape::String { list: Some(_), .. }
    ));
    assert_eq!(
        affinity.effective_shape,
        GameSystemColumnValueShape::Number {
            number_shape: GameSystemNumberShape::NonNegativeInteger
        }
    );
}

#[test]
fn family_text_affinity_repairs_boolean_reference_outliers() {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "DamageTable",
            1,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("DeflectDamageID", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::String("damage_b".to_owned()),
            ]],
        ))
        .expect("insert text damage table");
    data_tables
        .insert(test_table(
            "DungeonDamageTable",
            2,
            "DamageData",
            vec![
                ("DamageId", ColumnType::String),
                ("DeflectDamageID", ColumnType::Boolean),
            ],
            vec![vec![
                OwnedCellValue::String("SunTurretBeam".to_owned()),
                OwnedCellValue::Boolean(false),
            ]],
        ))
        .expect("insert repaired damage table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "DungeonDamageTable", "DeflectDamageID");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { list: None, .. }
    ));

    let affinity = report_table_affinity(&report, "DungeonDamageTable", "DeflectDamageID");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.75);
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::NativeText && repair.row_index.is_none()
    }));
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::NativeText
            && repair.row_index == Some(0)
            && repair.value.as_deref() == Some("false")
    }));
}

#[test]
fn scalar_hex_text_columns_do_not_repair_to_family_numeric() {
    let mut data_tables = GameSystemDataTables::default();
    for (table_name, table_id, value) in [
        ("AfflictionsNumericA", 1, 809.0),
        ("AfflictionsNumericB", 2, 80339128.0),
    ] {
        data_tables
            .insert(test_table(
                table_name,
                table_id,
                "AfflictionData",
                vec![
                    ("AfflictionID", ColumnType::String),
                    ("ColorHex", ColumnType::Number),
                ],
                vec![vec![
                    OwnedCellValue::String(format!("{table_name}Id")),
                    OwnedCellValue::Number(value),
                ]],
            ))
            .expect("insert numeric affliction table");
    }
    data_tables
        .insert(test_table(
            "AfflictionsText",
            3,
            "AfflictionData",
            vec![
                ("AfflictionID", ColumnType::String),
                ("ColorHex", ColumnType::String),
            ],
            vec![
                vec![
                    OwnedCellValue::String("AfflictionFrostbite".to_owned()),
                    OwnedCellValue::String("80566598762496".to_owned()),
                ],
                vec![
                    OwnedCellValue::String("AfflictionBleed".to_owned()),
                    OwnedCellValue::String("#80FF80".to_owned()),
                ],
            ],
        ))
        .expect("insert string affliction table");

    let report = infer_data_tables_schema(&data_tables);
    let column = report_table_column(&report, "AfflictionsText", "ColorHex");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { .. }
    ));
    let affinity = report_table_affinity(&report, "AfflictionsText", "ColorHex");
    assert!(!affinity.repairable);
}
