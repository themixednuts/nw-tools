use super::*;

#[test]
fn stat_modifier_grouped_prefix_columns_repair_to_float() {
    let data_tables = test_data_tables(
        "ArmorItemDefinitions",
        "ArmorItemDefinitions",
        vec![
            ("WeaponID", ColumnType::String),
            ("DEFStrike", ColumnType::Number),
            ("ABSFire", ColumnType::Number),
            ("RESPoison", ColumnType::Number),
            ("DMGSiege", ColumnType::Number),
            ("PhysicalArmor", ColumnType::Number),
            ("ElementalArmor", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("armor_a".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("armor_b".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "DEFStrike",
        "ABSFire",
        "RESPoison",
        "DMGSiege",
        "PhysicalArmor",
        "ElementalArmor",
    ] {
        let column = report_column(&report, column_name);
        assert_eq!(number_shape(column), GameSystemNumberShape::Float);

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn stat_modifier_grouped_prefix_acid_suffix_is_not_id_text() {
    let data_tables = test_data_tables(
        "AffixStatDataTable",
        "AffixStatData",
        vec![
            ("StatusID", ColumnType::String),
            ("RESAcid", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("status_a".to_owned()),
            OwnedCellValue::String(String::new()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "RESAcid");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);

    let affinity = report_affinity(&report, "RESAcid");
    assert!(affinity.repairable);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn stat_modifier_scalar_columns_repair_to_float() {
    let data_tables = test_data_tables(
        "StatusEffects",
        "StatusEffectData",
        vec![
            ("StatusID", ColumnType::String),
            ("Health", ColumnType::Number),
            ("HealthMin", ColumnType::String),
            ("ManaModifierDamageBased", ColumnType::Number),
            ("EncumbrancePerGS", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("status_a".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("status_b".to_owned()),
                OwnedCellValue::Number(100.0),
                OwnedCellValue::String("5".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(2.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "Health",
        "HealthMin",
        "ManaModifierDamageBased",
        "EncumbrancePerGS",
    ] {
        let column = report_column(&report, column_name);
        assert_eq!(number_shape(column), GameSystemNumberShape::Float);

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn stat_modifier_signed_scalar_columns_repair_to_integer() {
    let data_tables = test_data_tables(
        "StatusEffects",
        "StatusEffectData",
        vec![
            ("StatusID", ColumnType::String),
            ("CoreTempMod", ColumnType::Number),
            ("TempMod", ColumnType::String),
            ("InventorySlots", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("status_a".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("status_b".to_owned()),
                OwnedCellValue::Number(-1.0),
                OwnedCellValue::String("-2".to_owned()),
                OwnedCellValue::Number(3.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in ["CoreTempMod", "TempMod", "InventorySlots"] {
        let column = report_column(&report, column_name);
        assert_eq!(number_shape(column), GameSystemNumberShape::Integer);

        let affinity = report_affinity(&report, column_name);
        if column_name == "CoreTempMod" {
            assert!(!affinity.repairable);
        } else {
            assert!(affinity.repairable);
        }
    }
}

#[test]
fn stat_modifier_scalar_columns_repair_native_numeric_text_defaults() {
    let data_tables = test_data_tables(
        "StatusEffects_AI",
        "StatusEffectData",
        vec![
            ("StatusID", ColumnType::String),
            ("ManaModifierDamageBased", ColumnType::String),
            ("DamageType", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("status_a".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String("Arcane".to_owned()),
            ],
            vec![
                OwnedCellValue::String("status_b".to_owned()),
                OwnedCellValue::String("Arcane".to_owned()),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "ManaModifierDamageBased");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);

    let affinity = report_affinity(&report, "ManaModifierDamageBased");
    assert!(affinity.repairable);
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::NativeNumericText
            && repair.row_index == Some(1)
            && repair.value.as_deref() == Some("Arcane")
    }));
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::AdjacentColumn
            && repair.row_index == Some(1)
            && repair.adjacent_column.as_deref() == Some("DamageType")
            && repair.adjacent_direction == Some(GameSystemAdjacentColumnDirection::Right)
    }));
}

#[test]
fn stat_modifier_mod_prefix_columns_repair_to_inclusive_float_ranges() {
    let data_tables = test_data_tables(
        "StatusEffects",
        "StatusEffectData",
        vec![
            ("StatusID", ColumnType::String),
            ("MODStrength", ColumnType::Number),
            ("MODDexterity", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("status_a".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::String("0.1-0.25".to_owned()),
            ],
            vec![
                OwnedCellValue::String("status_b".to_owned()),
                OwnedCellValue::Number(2.0),
                OwnedCellValue::String("0.5".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in ["MODStrength", "MODDexterity"] {
        let column = report_column(&report, column_name);
        assert_eq!(
            column.value_shape,
            GameSystemColumnValueShape::Range {
                bounds: GameSystemRangeBounds::Inclusive,
                number_shape: GameSystemNumberShape::Float,
            }
        );

        let affinity = report_affinity(&report, column_name);
        assert!(affinity.repairable);
        assert!(
            affinity
                .repairs
                .iter()
                .any(|repair| repair.kind == GameSystemColumnTypeRepairKind::NativeRangeText)
        );
    }
}
