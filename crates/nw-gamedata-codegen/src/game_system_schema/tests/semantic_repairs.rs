use super::*;

#[test]
fn numeric_name_affinity_keeps_quantity_columns_float() {
    let mut tables = vec![table_schema(
        "Afflictions",
        "AfflictionData",
        "AfflictionFrac",
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

    let mut tables = vec![table_schema(
        "AbilityData",
        "AbilityData",
        "CooldownDuration",
        GameSystemNumberShape::PositiveInteger,
    )];
    apply_test_type_affinity(&mut tables);
    assert_eq!(
        number_shape(&tables[0].columns[0]),
        GameSystemNumberShape::Float
    );
}

#[test]
fn numeric_name_affinity_does_not_reclassify_discrete_columns() {
    let mut tables = vec![
        table_schema(
            "DamageTable",
            "DamageData",
            "MaxAllowedDamageShareCount",
            GameSystemNumberShape::NonNegativeInteger,
        ),
        table_schema(
            "DamageTable",
            "DamageData",
            "PowerLevel",
            GameSystemNumberShape::PositiveInteger,
        ),
    ];
    let affinities = apply_test_type_affinity(&mut tables);

    assert_eq!(
        number_shape(&tables[0].columns[0]),
        GameSystemNumberShape::NonNegativeInteger
    );
    assert_eq!(
        number_shape(&tables[1].columns[0]),
        GameSystemNumberShape::PositiveInteger
    );
    assert!(affinities.iter().all(|affinity| !affinity.repairable));
    assert!(affinities.iter().all(|affinity| affinity.confidence == 1.0));
}

#[test]
fn literal_boolean_text_affinity_repairs_clean_string_booleans() {
    let data_tables = test_data_tables(
        "CutsceneCameraPresets",
        "CutsceneCameraStaticData",
        vec![
            ("CutsceneCameraId", ColumnType::String),
            ("HidePlayerAvatar", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("camera_a".to_owned()),
                OwnedCellValue::String("true".to_owned()),
            ],
            vec![
                OwnedCellValue::String("camera_b".to_owned()),
                OwnedCellValue::String("false".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "HidePlayerAvatar");
    assert_eq!(column.value_shape, GameSystemColumnValueShape::Boolean);

    let affinity = report_affinity(&report, "HidePlayerAvatar");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 1.0);
    assert!(affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::NativeBooleanText
            && repair.row_index.is_none()
    }));
}

#[test]
fn declared_boolean_columns_with_non_boolean_text_stay_text() {
    let data_tables = test_data_tables(
        "DungeonDamageTable",
        "DamageData",
        vec![
            ("DamageId", ColumnType::String),
            ("IgnoreDisabledAttackTypes", ColumnType::Boolean),
        ],
        vec![vec![
            OwnedCellValue::String("damage_a".to_owned()),
            OwnedCellValue::String("Fire".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "IgnoreDisabledAttackTypes");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { .. }
    ));

    let affinity = report_affinity(&report, "IgnoreDisabledAttackTypes");
    assert!(!affinity.repairable);
}

#[test]
fn literal_boolean_text_affinity_keeps_row_keys_as_strings() {
    let data_tables = test_data_tables(
        "TruthLabels",
        "TruthLabelData",
        vec![("TruthLabelId", ColumnType::String)],
        vec![
            vec![OwnedCellValue::String("true".to_owned())],
            vec![OwnedCellValue::String("false".to_owned())],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "TruthLabelId");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { .. }
    ));

    let affinity = report_affinity(&report, "TruthLabelId");
    assert!(!affinity.repairable);
    assert_eq!(affinity.confidence, 1.0);
}

#[test]
fn territory_definition_id_keeps_numeric_column_width_from_values() {
    let data_tables = test_data_tables(
        "Territories",
        "TerritoryDefinition",
        vec![
            ("TerritoryID", ColumnType::Number),
            ("NameLocalizationKey", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::Number(100_000.0),
            OwnedCellValue::String("@ui_poi_territory".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "TerritoryID");
    assert_eq!(number_shape(column), GameSystemNumberShape::PositiveInteger);
    let affinity = report_affinity(&report, "TerritoryID");
    assert!(!affinity.repairable);
    assert_eq!(
        affinity.effective_shape,
        GameSystemColumnValueShape::Number {
            number_shape: GameSystemNumberShape::PositiveInteger
        },
        "TerritoryDefinition/TerritoryID is a physical number column; native manager short ids are derived in manager cache code, not by narrowing the column",
    );
}

#[test]
fn notification_secondary_text_stays_scalar_text() {
    let data_tables = test_data_tables(
        "Notifications",
        "NotificationData",
        vec![
            ("NotificationId", ColumnType::String),
            ("SecondaryText", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("Place_Camp".to_owned()),
            OwnedCellValue::String(
                "Use this camp to respawn, recover, do basic crafting.".to_owned(),
            ),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let secondary_text = report_column(&report, "SecondaryText");
    assert!(matches!(
        secondary_text.value_shape,
        GameSystemColumnValueShape::String { list: None, .. }
    ));
}

#[test]
fn notification_track_display_count_repairs_to_bool() {
    let data_tables = test_data_tables(
        "Notifications",
        "NotificationData",
        vec![
            ("NotificationId", ColumnType::String),
            ("TrackDisplayCount", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("Currency_Sent_To_Player".to_owned()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("Complete_Structure".to_owned()),
                OwnedCellValue::Number(1.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let track_display_count = report_column(&report, "TrackDisplayCount");
    assert_eq!(
        track_display_count.value_shape,
        GameSystemColumnValueShape::Boolean
    );

    let affinity = report_affinity(&report, "TrackDisplayCount");
    assert!(affinity.repairable);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_boolean_affinity_uses_cell_confidence() {
    let data_tables = test_data_tables(
        "DamageTable",
        "DamageData",
        vec![
            ("DamageId", ColumnType::String),
            ("IgnoreInvulnerable", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("damage_b".to_owned()),
                OwnedCellValue::Number(0.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "IgnoreInvulnerable");
    assert_eq!(column.value_shape, GameSystemColumnValueShape::Boolean);
    let affinity = report_affinity(&report, "IgnoreInvulnerable");
    assert_eq!(affinity.confidence, 0.80);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_boolean_affinity_repairs_empty_string_columns() {
    let data_tables = test_data_tables(
        "CutsceneCameraPresets",
        "CutsceneCameraStaticData",
        vec![
            ("CutsceneCameraId", ColumnType::String),
            ("CancelInventory", ColumnType::String),
            ("InterruptInCombat", ColumnType::String),
            ("InterruptOnMovement", ColumnType::String),
            ("CanSkip", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("camera_a".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
            ],
            vec![
                OwnedCellValue::String("camera_b".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "CancelInventory",
        "InterruptInCombat",
        "InterruptOnMovement",
        "CanSkip",
    ] {
        let column = report_column(&report, column_name);
        assert_eq!(column.value_shape, GameSystemColumnValueShape::Boolean);

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
fn semantic_boolean_affinity_repairs_native_item_bool_columns() {
    let data_tables = test_data_tables(
        "MasterItemDefinitions_Common",
        "MasterItemDefinitions",
        vec![
            ("ItemId", ColumnType::String),
            ("BindOnPickup", ColumnType::Number),
            ("ConfirmDestroy", ColumnType::Number),
            ("Nonremovable", ColumnType::Number),
            ("SalvageResources", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("item_a".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("item_b".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(1.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "BindOnPickup",
        "ConfirmDestroy",
        "Nonremovable",
        "SalvageResources",
    ] {
        let column = report_column(&report, column_name);
        assert_eq!(column.value_shape, GameSystemColumnValueShape::Boolean);
        let affinity = report_affinity(&report, column_name);
        assert_eq!(affinity.confidence, 0.80);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn semantic_boolean_affinity_repairs_item_currency_conversion_flags() {
    let data_tables = test_data_tables(
        "ItemCurrencyConversions",
        "ItemCurrencyConversionData",
        vec![
            ("ConversionID", ColumnType::String),
            ("Bought", ColumnType::Number),
            ("Sold", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("conversion_a".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("conversion_b".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(0.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in ["Bought", "Sold"] {
        let column = report_column(&report, column_name);
        assert_eq!(column.value_shape, GameSystemColumnValueShape::Boolean);
        let affinity = report_affinity(&report, column_name);
        assert_eq!(affinity.confidence, 0.80);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn semantic_boolean_affinity_repairs_native_variation_bool_columns() {
    let data_tables = test_data_tables(
        "VariationData_Common",
        "VariationData",
        vec![
            ("VariantId", ColumnType::String),
            ("ExcludeFromGame", ColumnType::Number),
            ("ShowOnCompass", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("variant_a".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("variant_b".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(1.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in ["ExcludeFromGame", "ShowOnCompass"] {
        let column = report_column(&report, column_name);
        assert_eq!(column.value_shape, GameSystemColumnValueShape::Boolean);
        let affinity = report_affinity(&report, column_name);
        assert_eq!(affinity.confidence, 0.80);
        assert_eq!(
            affinity.repairs[0].kind,
            GameSystemColumnTypeRepairKind::SemanticName
        );
    }
}

#[test]
fn semantic_boolean_affinity_repairs_native_gatherable_bool_columns() {
    let data_tables = test_data_tables(
        "Gatherables",
        "GatherableData",
        vec![
            ("GatherableID", ColumnType::String),
            ("RequireLootItems", ColumnType::String),
            ("RestrictSuspectedBots", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("gatherable_a".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String("false".to_owned()),
            ],
            vec![
                OwnedCellValue::String("gatherable_b".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String("true".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let require_loot_items = report_column(&report, "RequireLootItems");
    assert_eq!(
        require_loot_items.value_shape,
        GameSystemColumnValueShape::Boolean
    );
    let require_loot_items_affinity = report_affinity(&report, "RequireLootItems");
    assert!(require_loot_items_affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::SemanticName
            && repair.to == GameSystemColumnValueShape::Boolean
    }));

    let restrict_suspected_bots = report_column(&report, "RestrictSuspectedBots");
    assert_eq!(
        restrict_suspected_bots.value_shape,
        GameSystemColumnValueShape::Boolean
    );
    assert!(report_affinity(&report, "RestrictSuspectedBots").repairable);
}

#[test]
fn semantic_boolean_affinity_rejects_conflicting_cells() {
    let data_tables = test_data_tables(
        "DamageTable",
        "DamageData",
        vec![
            ("DamageId", ColumnType::String),
            ("IgnoreInvulnerable", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("damage_b".to_owned()),
                OwnedCellValue::Number(2.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "IgnoreInvulnerable");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::Number { .. }
    ));
    let affinity = report_affinity(&report, "IgnoreInvulnerable");
    assert!(!affinity.repairable);
}

#[test]
fn semantic_boolean_affinity_rejects_probability_columns() {
    let data_tables = test_data_tables(
        "MasterItemDefinitions_Common",
        "MasterItemDefinitions",
        vec![
            ("ItemId", ColumnType::String),
            ("NoBindOnPickupChance", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("item_a".to_owned()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("item_b".to_owned()),
                OwnedCellValue::Number(0.5),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "NoBindOnPickupChance");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);
    let affinity = report_affinity(&report, "NoBindOnPickupChance");
    assert!(!affinity.repairs.iter().any(|repair| {
        repair.kind == GameSystemColumnTypeRepairKind::SemanticName
            && repair.to == GameSystemColumnValueShape::Boolean
    }));
}

#[test]
fn semantic_boolean_affinity_requires_complete_cell_evidence() {
    let data_tables = test_data_tables(
        "ItemData",
        "ItemData",
        vec![
            ("ItemId", ColumnType::String),
            ("IsStackableMax", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("item_a".to_owned()),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("item_b".to_owned()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("item_c".to_owned()),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("item_d".to_owned()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("item_e".to_owned()),
                OwnedCellValue::Number(5.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "IsStackableMax");
    assert_eq!(
        number_shape(column),
        GameSystemNumberShape::NonNegativeInteger
    );
    let affinity = report_affinity(&report, "IsStackableMax");
    assert!(!affinity.repairable);
    assert_eq!(affinity.confidence, 1.0);
}

#[test]
fn semantic_integer_affinity_repairs_empty_progression_initial_points() {
    let data_tables = test_data_tables(
        "ProgressionPools",
        "ProgressionPoolData",
        vec![
            ("ProgressionPoolId", ColumnType::String),
            ("PointCap", ColumnType::Number),
            ("InitialPoints", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("AttributeStrength".to_owned()),
                OwnedCellValue::Number(250.0),
                OwnedCellValue::String(String::new()),
            ],
            vec![
                OwnedCellValue::String("AttributeDexterity".to_owned()),
                OwnedCellValue::Number(250.0),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "InitialPoints");
    assert_eq!(
        number_shape(column),
        GameSystemNumberShape::NonNegativeInteger
    );
    let affinity = report_affinity(&report, "InitialPoints");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.75);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_integer_affinity_repairs_empty_progression_required_character_level() {
    let data_tables = test_data_tables(
        "ProgressionPoints",
        "ProgressionPointData",
        vec![
            ("ProgressionPointId", ColumnType::String),
            ("MaxLevel", ColumnType::Number),
            ("RequiredCharacterLevel", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("AttributeStrength_0".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::String(String::new()),
            ],
            vec![
                OwnedCellValue::String("AttributeDexterity_0".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "RequiredCharacterLevel");
    assert_eq!(
        number_shape(column),
        GameSystemNumberShape::NonNegativeInteger
    );
    let affinity = report_affinity(&report, "RequiredCharacterLevel");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.75);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn semantic_integer_affinity_repairs_item_currency_conversion_numbers() {
    let data_tables = test_data_tables(
        "ItemCurrencyConversions",
        "ItemCurrencyConversionData",
        vec![
            ("ConversionID", ColumnType::String),
            ("ItemQty", ColumnType::String),
            ("BuyProgression3Cost", ColumnType::String),
            ("SellAzothCost", ColumnType::String),
            ("BuyCooldownSeconds", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("conversion_a".to_owned()),
                OwnedCellValue::String("1".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::Number(0.0),
            ],
            vec![
                OwnedCellValue::String("conversion_b".to_owned()),
                OwnedCellValue::String("5".to_owned()),
                OwnedCellValue::String("100".to_owned()),
                OwnedCellValue::String("0".to_owned()),
                OwnedCellValue::Number(86_400.0),
            ],
            vec![
                OwnedCellValue::String("achievement_only".to_owned()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::Number(604_800.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    assert_eq!(
        number_shape(report_column(&report, "ItemQty")),
        GameSystemNumberShape::PositiveInteger
    );
    for column_name in ["BuyProgression3Cost", "SellAzothCost", "BuyCooldownSeconds"] {
        assert_eq!(
            number_shape(report_column(&report, column_name)),
            GameSystemNumberShape::NonNegativeInteger
        );
    }
    assert_eq!(report_affinity(&report, "ItemQty").confidence, 0.80);
    assert!(report_affinity(&report, "BuyProgression3Cost").repairable);
    assert!(report_affinity(&report, "SellAzothCost").repairable);
}

#[test]
fn semantic_integer_affinity_keeps_dye_color_indices_integral() {
    let data_tables = test_data_tables(
        "DyeItemDefinitions",
        "DyeItemDefinitions",
        vec![
            ("DyeItemId", ColumnType::String),
            ("ColorIndex", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("DyeA004".to_owned()),
                OwnedCellValue::Number(4.0),
            ],
            vec![
                OwnedCellValue::String("DyeA006".to_owned()),
                OwnedCellValue::Number(6.0),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "ColorIndex");
    assert_eq!(number_shape(column), GameSystemNumberShape::PositiveInteger);
}

#[test]
fn dye_color_text_columns_do_not_repair_to_numeric_zero() {
    let data_tables = test_data_tables(
        "DyeColorDataTable",
        "DyeColorData",
        vec![
            ("Index", ColumnType::Number),
            ("Color", ColumnType::String),
            ("SpecColor", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::Number(4.0),
                OwnedCellValue::String("#31608c".to_owned()),
                OwnedCellValue::String("#121d2a".to_owned()),
            ],
            vec![
                OwnedCellValue::Number(6.0),
                OwnedCellValue::String("#e9c73a".to_owned()),
                OwnedCellValue::String("#a58f23".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in ["Color", "SpecColor"] {
        let column = report_column(&report, column_name);
        assert!(
            matches!(
                column.value_shape,
                GameSystemColumnValueShape::String { .. }
            ),
            "{column_name}: {:?}",
            column.value_shape
        );
        let affinity = report_affinity(&report, column_name);
        assert!(!affinity.repairable);
    }
}

#[test]
fn dye_color_amount_columns_keep_float_affinity() {
    let data_tables = test_data_tables(
        "DyeColorDataTable",
        "DyeColorData",
        vec![
            ("Index", ColumnType::Number),
            ("ColorAmount", ColumnType::Number),
            ("ColorOverride", ColumnType::Number),
            ("SpecAmount", ColumnType::Number),
            ("MaskGlossShift", ColumnType::Number),
        ],
        vec![vec![
            OwnedCellValue::Number(4.0),
            OwnedCellValue::Number(1.0),
            OwnedCellValue::Number(0.0),
            OwnedCellValue::Number(1.0),
            OwnedCellValue::Number(0.0),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "ColorAmount",
        "ColorOverride",
        "SpecAmount",
        "MaskGlossShift",
    ] {
        assert_eq!(
            number_shape(report_column(&report, column_name)),
            GameSystemNumberShape::Float,
            "{column_name}"
        );
    }
}

#[test]
fn crest_part_color_hex_column_repairs_to_linear_rgba() {
    let data_tables = test_data_tables(
        "Crests",
        "CrestPartData",
        vec![("Index", ColumnType::Number), ("Color", ColumnType::String)],
        vec![
            vec![
                OwnedCellValue::Number(1.0),
                OwnedCellValue::String("#c31818".to_owned()),
            ],
            vec![
                OwnedCellValue::Number(2.0),
                OwnedCellValue::String("#ca5711".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "Color");
    assert_eq!(
        column.value_shape,
        GameSystemColumnValueShape::Color {
            color_shape: GameSystemColorShape::LinearRgba
        }
    );
    let affinity = report_affinity(&report, "Color");
    assert!(affinity.repairable);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticColor
    );
}

#[test]
fn native_text_classifier_columns_do_not_repair_to_numeric_zero() {
    let data_tables = test_data_tables(
        "DifficultyScaling_WorldEncounter_Participants",
        "DifficultyScalingData",
        vec![
            ("WorldEncounterID", ColumnType::String),
            ("MaxHealthMod", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("WE_CutlassKeys_00".to_owned()),
            OwnedCellValue::String("Linear".to_owned()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "MaxHealthMod");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { list: None, .. }
    ));
    let affinity = report_affinity(&report, "MaxHealthMod");
    assert!(!affinity.repairable);
}

#[test]
fn native_timestamp_text_columns_do_not_repair_to_numeric_zero() {
    let data_tables = test_data_tables(
        "RotationalQueue",
        "RotationalQueueData",
        vec![
            ("RotationalQueueId", ColumnType::String),
            ("QueueStartTime", ColumnType::String),
            ("QueueEndTime", ColumnType::String),
            ("GameModeTimeSpan", ColumnType::Number),
        ],
        vec![vec![
            OwnedCellValue::String("RotationalQueueSeason01".to_owned()),
            OwnedCellValue::String("2025-09-01T12:00:00".to_owned()),
            OwnedCellValue::String("2035-09-01T12:00:00".to_owned()),
            OwnedCellValue::Number(3600.0),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in ["QueueStartTime", "QueueEndTime"] {
        let column = report_column(&report, column_name);
        assert!(
            matches!(
                column.value_shape,
                GameSystemColumnValueShape::String { list: None, .. }
            ),
            "{column_name}: {:?}",
            column.value_shape
        );
        let affinity = report_affinity(&report, column_name);
        assert!(!affinity.repairable);
    }
    assert_eq!(
        number_shape(report_column(&report, "GameModeTimeSpan")),
        GameSystemNumberShape::Float
    );
}

#[test]
fn simple_tree_icon_background_path_does_not_repair_to_numeric_zero() {
    let data_tables = test_data_tables(
        "MetaAchievementCategoryDataTable",
        "SimpleTreeCategoryData",
        vec![
            ("MetaAchievementCategoryId", ColumnType::String),
            ("Icon Color Background", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("Enemies_By_Type".to_owned()),
            OwnedCellValue::String(
                "LyShineUI\\Images\\Icons\\Achievements\\achievement_icon_background01.dds"
                    .to_owned(),
            ),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "Icon Color Background");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { list: None, .. }
    ));
    let affinity = report_affinity(&report, "Icon Color Background");
    assert!(!affinity.repairable);
}

#[test]
fn scalar_reference_text_columns_do_not_repair_to_numeric_zero() {
    let data_tables = test_data_tables(
        "DarknessDataTable",
        "DarknessData",
        vec![
            ("DarknessId", ColumnType::String),
            ("DifficultyScalingGroup", ColumnType::String),
            ("DifficultyScalingTable", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("test".to_owned()),
            OwnedCellValue::String("Tier2".to_owned()),
            OwnedCellValue::String(
                "sharedassets/springboardentitites/datatables/javelindata_difficulty_darkness"
                    .to_owned(),
            ),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in ["DifficultyScalingGroup", "DifficultyScalingTable"] {
        let column = report_column(&report, column_name);
        assert!(
            matches!(
                column.value_shape,
                GameSystemColumnValueShape::String { .. }
            ),
            "{column_name}: {:?}",
            column.value_shape
        );
        let affinity = report_affinity(&report, column_name);
        assert!(!affinity.repairable);
    }
}

#[test]
fn bare_affliction_column_does_not_use_float_affinity() {
    let data_tables = test_data_tables(
        "DamageTypes",
        "DamageTypeData",
        vec![
            ("TypeID", ColumnType::String),
            ("Affliction", ColumnType::String),
        ],
        vec![vec![
            OwnedCellValue::String("Fire".to_owned()),
            OwnedCellValue::String(String::new()),
        ]],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "Affliction");
    assert!(matches!(
        column.value_shape,
        GameSystemColumnValueShape::String { .. }
    ));
    let affinity = report_affinity(&report, "Affliction");
    assert!(!affinity.repairable);
}

#[test]
fn semantic_float_affinity_keeps_numeric_surface_with_corrupt_string_cell() {
    let data_tables = test_data_tables(
        "BearCubDamageTable",
        "DamageData",
        vec![
            ("DamageId", ColumnType::String),
            ("BlockAbsorptionModifier", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::String("1".to_owned()),
            ],
            vec![
                OwnedCellValue::String("damage_b".to_owned()),
                OwnedCellValue::String("0.5".to_owned()),
            ],
            vec![
                OwnedCellValue::String("damage_c".to_owned()),
                OwnedCellValue::String("1.25".to_owned()),
            ],
            vec![
                OwnedCellValue::String("damage_d".to_owned()),
                OwnedCellValue::String("0".to_owned()),
            ],
            vec![
                OwnedCellValue::String("damage_e".to_owned()),
                OwnedCellValue::String("Large".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let column = report_column(&report, "BlockAbsorptionModifier");
    assert_eq!(number_shape(column), GameSystemNumberShape::Float);
    let affinity = report_affinity(&report, "BlockAbsorptionModifier");
    assert!(affinity.repairable);
    assert_eq!(affinity.confidence, 0.80);
    assert_eq!(
        affinity.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );
}

#[test]
fn adjacent_column_shift_reports_suspect_value() {
    let data_tables = test_data_tables(
        "DamageTable",
        "DamageData",
        vec![
            ("DamageId", ColumnType::String),
            ("IgnoreInvulnerable", ColumnType::Number),
            ("DmgCoef", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("damage_b".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(1.5),
            ],
            vec![
                OwnedCellValue::String("damage_c".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("damage_d".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("damage_e".to_owned()),
                OwnedCellValue::Number(1.5),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let affinity = report_affinity(&report, "IgnoreInvulnerable");
    let shift = affinity
        .repairs
        .iter()
        .find(|repair| repair.kind == GameSystemColumnTypeRepairKind::AdjacentColumn)
        .expect("adjacent-column shift diagnostic");
    assert_eq!(shift.row_index, Some(4));
    assert_eq!(shift.value.as_deref(), Some("1.5"));
    assert_eq!(shift.adjacent_column.as_deref(), Some("DmgCoef"));
    assert_eq!(
        shift.adjacent_direction,
        Some(GameSystemAdjacentColumnDirection::Right)
    );
}

#[test]
fn adjacent_column_shift_requires_neighbor_column_domain_evidence() {
    let data_tables = test_data_tables(
        "DamageTable",
        "DamageData",
        vec![
            ("DamageId", ColumnType::String),
            ("IgnoreInvulnerable", ColumnType::Number),
            ("DmgCoef", ColumnType::Number),
        ],
        vec![
            vec![
                OwnedCellValue::String("damage_a".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("damage_b".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("damage_c".to_owned()),
                OwnedCellValue::Number(1.0),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("damage_d".to_owned()),
                OwnedCellValue::Number(0.0),
                OwnedCellValue::Number(1.0),
            ],
            vec![
                OwnedCellValue::String("damage_e".to_owned()),
                OwnedCellValue::Number(1.5),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    let affinity = report_affinity(&report, "IgnoreInvulnerable");
    assert!(
        !affinity
            .repairs
            .iter()
            .any(|repair| repair.kind == GameSystemColumnTypeRepairKind::AdjacentColumn),
        "shift repair should require the neighboring column to already contain the suspect value"
    );
}

#[test]
fn semantic_perc_affinity_repairs_empty_post_skill_cap_reward_slots() {
    let data_tables = test_data_tables(
        "TradeSkillPostCap",
        "TradeSkillPostCapData",
        vec![
            ("TradeSkillType", ColumnType::String),
            ("TradeSkillRewardXP", ColumnType::Number),
            ("SubRewardPerc1", ColumnType::Number),
            ("SubRewardPerc2", ColumnType::Number),
            ("SubRewardPerc3", ColumnType::String),
            ("SubRewardPerc4", ColumnType::String),
            ("Level01SubReward3", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("Arcana_PostCap".to_owned()),
                OwnedCellValue::Number(3_965_415.0),
                OwnedCellValue::Number(0.33),
                OwnedCellValue::Number(0.66),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String("RewardPostCapArcanaT3".to_owned()),
            ],
            vec![
                OwnedCellValue::String("Armoring_PostCap".to_owned()),
                OwnedCellValue::Number(4_860_800.0),
                OwnedCellValue::Number(0.33),
                OwnedCellValue::Number(0.66),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String("RewardPostCapArmoringT3".to_owned()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    for column_name in [
        "SubRewardPerc1",
        "SubRewardPerc2",
        "SubRewardPerc3",
        "SubRewardPerc4",
    ] {
        assert_eq!(
            number_shape(report_column(&report, column_name)),
            GameSystemNumberShape::Float
        );
    }

    let repaired = report_affinity(&report, "SubRewardPerc3");
    assert!(repaired.repairable);
    assert_eq!(repaired.confidence, 0.75);
    assert_eq!(
        repaired.repairs[0].kind,
        GameSystemColumnTypeRepairKind::SemanticName
    );

    assert!(matches!(
        report_column(&report, "Level01SubReward3").value_shape,
        GameSystemColumnValueShape::String { list: None, .. }
    ));
}

#[test]
fn cooldown_text_keys_do_not_repair_to_numeric_zero_fallbacks() {
    let data_tables = test_data_tables(
        "ConsumableItemDefinitions",
        "ConsumableItemDefinitions",
        vec![
            ("ItemID", ColumnType::String),
            ("CooldownDuration", ColumnType::Number),
            ("CooldownId", ColumnType::String),
            ("SharedCooldownLocString", ColumnType::String),
        ],
        vec![
            vec![
                OwnedCellValue::String("consumable_a".to_owned()),
                OwnedCellValue::Number(15.0),
                OwnedCellValue::String("PotionMana".to_owned()),
                OwnedCellValue::String("@ui_consumables_shared_cooldowns_potion".to_owned()),
            ],
            vec![
                OwnedCellValue::String("consumable_b".to_owned()),
                OwnedCellValue::Number(30.0),
                OwnedCellValue::String(String::new()),
                OwnedCellValue::String(String::new()),
            ],
        ],
    );

    let report = infer_data_tables_schema(&data_tables);
    assert!(matches!(
        report_column(&report, "CooldownId").value_shape,
        GameSystemColumnValueShape::String { list: None, .. }
    ));
    assert!(matches!(
        report_column(&report, "SharedCooldownLocString").value_shape,
        GameSystemColumnValueShape::String { list: None, .. }
    ));
    assert_eq!(
        number_shape(report_column(&report, "CooldownDuration")),
        GameSystemNumberShape::Float
    );

    assert!(!report_affinity(&report, "CooldownId").repairable);
    assert!(!report_affinity(&report, "SharedCooldownLocString").repairable);
}
