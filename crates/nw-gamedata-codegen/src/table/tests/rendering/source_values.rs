use super::*;

#[test]
fn numeric_source_values_follow_schema_affinity() {
    let schema = GameSystemTableSchema {
        table_name: "PlayerDamageTable".to_owned(),
        table_name_crc: 1,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["damagedata/playerdamagetable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "CritPowerLevel".to_owned(),
            crc: 3,
            declared_type: ColumnType::Number,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::Number {
                number_shape: GameSystemNumberShape::Float,
            },
        }],
    };
    let column = &schema.columns[0];

    let from_number = native_dev_cell_value(&schema, column, &OwnedCellValue::Number(7.9))
        .expect("native float number")
        .expect("cell value");

    assert!(matches!(
        from_number,
        NativeDevCellValue::Scalar(NativeDevScalarValue::F32(value)) if value == 7.9
    ));
}

#[test]
fn range_source_text_values_emit_core_range_records() {
    let schema = GameSystemTableSchema {
        table_name: "FishingCatchablesMastersheet".to_owned(),
        table_name_crc: 1,
        row_type_name: "FishingCatchablesData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["fishingcatchablesdata/fishingcatchablesmastersheet.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "FishWeightRange".to_owned(),
            crc: 3,
            declared_type: ColumnType::String,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::Range {
                bounds: GameSystemRangeBounds::Inclusive,
                number_shape: GameSystemNumberShape::Float,
            },
        }],
    };
    let column = &schema.columns[0];

    let from_text = native_dev_cell_value(
        &schema,
        column,
        &OwnedCellValue::String("2.5-1.5".to_owned()),
    )
    .expect("range text")
    .expect("cell value");

    assert!(matches!(
        from_text,
        NativeDevCellValue::Range(NativeDevRangeValue::F32(NativeDevRange::Inclusive(value)))
            if value.start == 1.5 && value.last == 2.5
    ));
    let rendered = render_test_table(&schema);
    assert!(rendered.contains("type Cell<'cell> = ::core::range::RangeInclusive<f32>;"));
    assert!(!rendered.contains("const DATA_"));
}

#[test]
fn crc32_string_lists_keep_designer_tokens_in_ron() {
    let schema = GameSystemTableSchema {
        table_name: "FishingCatchablesMastersheet".to_owned(),
        table_name_crc: 1,
        row_type_name: "FishingCatchablesData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["fishingcatchablesdata/fishingcatchablesmastersheet.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "FishBehaviors".to_owned(),
            crc: 3,
            declared_type: ColumnType::String,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: true,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: Some(GameSystemListShape {
                    separators: vec![",".to_owned()],
                    rows_with_lists: 1,
                    total_entries: 2,
                    preserve_empty_entries: false,
                    element_shape: Some(GameSystemListElementShape::Crc32),
                }),
                foreign_keys: Vec::new(),
            },
        }],
    };
    let column = &schema.columns[0];

    let from_text = native_dev_cell_value(
        &schema,
        column,
        &OwnedCellValue::String("Lazy,Sporadic".to_owned()),
    )
    .expect("crc32 list text")
    .expect("cell value");

    assert!(matches!(
        from_text,
        NativeDevCellValue::List(values)
            if matches!(
                &values[..],
                [
                    NativeDevCellValue::Scalar(NativeDevScalarValue::String(first)),
                    NativeDevCellValue::Scalar(NativeDevScalarValue::String(second)),
                ] if first == "Lazy" && second == "Sporadic"
            )
    ));
    let rendered = render_test_table(&schema);
    assert!(rendered.contains(
        "type Cell<'cell> = gamedata::List<'cell, FishBehaviorsColumn, az_core::crc::Crc32>;"
    ));
    assert!(!rendered.contains("const LIST_ELEMENT_"));
}

#[test]
fn numeric_source_text_values_parse_native_numeric_prefixes() {
    let schema = GameSystemTableSchema {
        table_name: "MasterItemDefinitions_PVP".to_owned(),
        table_name_crc: 1,
        row_type_name: "MasterItemDefinitions".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["masteritemdefinitions/masteritemdefinitions_pvp.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "MaxStackSize".to_owned(),
            crc: 3,
            declared_type: ColumnType::String,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::Number {
                number_shape: GameSystemNumberShape::NonNegativeInteger,
            },
        }],
    };
    let column = &schema.columns[0];

    let from_text = native_dev_cell_value(
        &schema,
        column,
        &OwnedCellValue::String("10000+RC".to_owned()),
    )
    .expect("native integer text")
    .expect("cell value");

    assert!(matches!(
        from_text,
        NativeDevCellValue::Scalar(NativeDevScalarValue::U64(value)) if value == 10_000
    ));
}

#[test]
fn numeric_source_text_lists_follow_repaired_element_affinity() {
    let schema = GameSystemTableSchema {
        table_name: "ElementalMutation".to_owned(),
        table_name_crc: 1,
        row_type_name: "ElementalMutationStaticData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["elementalmutationstaticdata/elementalmutation.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "TextColor".to_owned(),
            crc: 3,
            declared_type: ColumnType::String,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: false,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: Some(GameSystemListShape {
                    separators: vec![",".to_owned()],
                    rows_with_lists: 1,
                    total_entries: 3,
                    preserve_empty_entries: false,
                    element_shape: Some(GameSystemListElementShape::Number {
                        number_shape: GameSystemNumberShape::Float,
                    }),
                }),
                foreign_keys: Vec::new(),
            },
        }],
    };
    let column = &schema.columns[0];

    let from_text = native_dev_cell_value(
        &schema,
        column,
        &OwnedCellValue::String("198,67,67".to_owned()),
    )
    .expect("native float list text")
    .expect("cell value");

    assert!(
        matches!(
            from_text,
            NativeDevCellValue::List(values)
                if matches!(
                    &values[..],
                    [
                        NativeDevCellValue::Scalar(NativeDevScalarValue::F32(first)),
                        NativeDevCellValue::Scalar(NativeDevScalarValue::F32(second)),
                        NativeDevCellValue::Scalar(NativeDevScalarValue::F32(third)),
                    ] if *first == 198.0 && *second == 67.0 && *third == 67.0
                )
        ),
        "numeric list source text must emit numeric RON list values"
    );

    let from_number = native_dev_cell_value(&schema, column, &OwnedCellValue::Number(198.0))
        .expect("native float list number")
        .expect("cell value");

    assert!(
        matches!(
            from_number,
            NativeDevCellValue::List(values)
                if matches!(
                    &values[..],
                    [NativeDevCellValue::Scalar(NativeDevScalarValue::F32(value))]
                        if *value == 198.0
                )
        ),
        "numeric source cells for numeric lists must emit a single typed RON list entry"
    );

    let rendered = render_test_table(&schema);
    assert!(rendered.contains("type Cell<'cell> = gamedata::List<'cell, TextColorColumn, f32>;"));
    assert!(!rendered.contains("const LIST_ELEMENT_"));
}

#[test]
fn color_source_text_stays_human_readable_and_emits_typed_table_cell() {
    let schema = GameSystemTableSchema {
        table_name: "Crests".to_owned(),
        table_name_crc: 1,
        row_type_name: "CrestPartData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["crest_part_data/crests.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "Color".to_owned(),
            crc: 3,
            declared_type: ColumnType::String,
            row_key: false,
            required: false,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::Color {
                color_shape: GameSystemColorShape::LinearRgba,
            },
        }],
    };
    let column = &schema.columns[0];

    let from_text = native_dev_cell_value(
        &schema,
        column,
        &OwnedCellValue::String("#c31818".to_owned()),
    )
    .expect("color source text")
    .expect("cell value");

    assert!(
        matches!(
            from_text,
            NativeDevCellValue::Scalar(NativeDevScalarValue::String(value))
                if value == "#c31818"
        ),
        "color source must stay human-readable in RON"
    );

    let rendered = render_test_table(&schema);
    assert!(rendered.contains("type Cell<'cell> = bevy_color::LinearRgba;"));
}

#[test]
fn zero_numeric_source_pair_lists_emit_empty_list() {
    let schema = GameSystemTableSchema {
        table_name: "StatusEffects_AI".to_owned(),
        table_name_crc: 1,
        row_type_name: "StatusEffectData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["statuseffectdata/statuseffects_ai.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "XPIncreases".to_owned(),
            crc: 3,
            declared_type: ColumnType::Number,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: false,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: Some(GameSystemListShape {
                    separators: vec!["+".to_owned()],
                    rows_with_lists: 0,
                    total_entries: 0,
                    preserve_empty_entries: false,
                    element_shape: Some(GameSystemListElementShape::Pair {
                        separator: '=',
                        first: GameSystemListAtomShape::Crc32,
                        second: GameSystemListAtomShape::Number {
                            number_shape: GameSystemNumberShape::Float,
                        },
                        default_second_source_token: None,
                    }),
                }),
                foreign_keys: Vec::new(),
            },
        }],
    };
    let column = &schema.columns[0];

    let from_scalar = native_dev_cell_value(&schema, column, &OwnedCellValue::Number(0.0))
        .expect("native pair list zero")
        .expect("cell value");

    assert!(
        matches!(
            from_scalar,
            NativeDevCellValue::List(values) if values.is_empty()
        ),
        "numeric zero source cells for pair lists must emit an empty typed RON list"
    );
}

#[test]
fn nonzero_numeric_source_pair_lists_error() {
    let schema = GameSystemTableSchema {
        table_name: "StatusEffects_AI".to_owned(),
        table_name_crc: 1,
        row_type_name: "StatusEffectData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["statuseffectdata/statuseffects_ai.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "XPIncreases".to_owned(),
            crc: 3,
            declared_type: ColumnType::Number,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: false,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: Some(GameSystemListShape {
                    separators: vec!["+".to_owned()],
                    rows_with_lists: 0,
                    total_entries: 0,
                    preserve_empty_entries: false,
                    element_shape: Some(GameSystemListElementShape::Pair {
                        separator: '=',
                        first: GameSystemListAtomShape::Crc32,
                        second: GameSystemListAtomShape::Number {
                            number_shape: GameSystemNumberShape::Float,
                        },
                        default_second_source_token: None,
                    }),
                }),
                foreign_keys: Vec::new(),
            },
        }],
    };
    let column = &schema.columns[0];

    let error = native_dev_cell_value(&schema, column, &OwnedCellValue::Number(-0.05))
        .expect_err("nonzero numeric scalar must not be dropped from pair-list field");

    assert!(
        error.to_string().contains("outside list schema"),
        "unexpected error: {error}"
    );
}

#[test]
fn pair_lists_repair_comma_authored_entries_to_schema_separator() {
    let schema = GameSystemTableSchema {
        table_name: "AffixStatDataTable".to_owned(),
        table_name_crc: 1,
        row_type_name: "AffixStatData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["affixstatdata/affix_stat_data_table.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "ABSVitalsCategory".to_owned(),
            crc: 3,
            declared_type: ColumnType::String,
            row_key: false,
            required: false,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: false,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: Some(GameSystemListShape {
                    separators: vec!["+".to_owned()],
                    rows_with_lists: 0,
                    total_entries: 1,
                    preserve_empty_entries: false,
                    element_shape: Some(GameSystemListElementShape::Pair {
                        separator: '=',
                        first: GameSystemListAtomShape::Crc32,
                        second: GameSystemListAtomShape::Number {
                            number_shape: GameSystemNumberShape::Float,
                        },
                        default_second_source_token: None,
                    }),
                }),
                foreign_keys: Vec::new(),
            },
        }],
    };
    let column = &schema.columns[0];

    let repaired = native_dev_cell_value(
        &schema,
        column,
        &OwnedCellValue::String("Ancient=0.025,AngryEarth=0.025".to_owned()),
    )
    .expect("comma-authored pair list")
    .expect("cell value");

    assert!(
        matches!(
            repaired,
            NativeDevCellValue::List(values)
                if values.len() == 2
                    && values.iter().all(|value| matches!(value, NativeDevCellValue::Pair(_)))
        ),
        "comma-authored pair list entries must emit typed pair RON values"
    );
}

#[test]
fn pair_lists_reject_numeric_scalar_text_without_pair_key() {
    let schema = GameSystemTableSchema {
        table_name: "StatusEffects_Items".to_owned(),
        table_name_crc: 1,
        row_type_name: "StatusEffectData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["statuseffectdata/status_effects_items.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "DMGVitalsCategory".to_owned(),
            crc: 3,
            declared_type: ColumnType::String,
            row_key: false,
            required: false,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: false,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: Some(GameSystemListShape {
                    separators: vec!["+".to_owned()],
                    rows_with_lists: 0,
                    total_entries: 1,
                    preserve_empty_entries: false,
                    element_shape: Some(GameSystemListElementShape::Pair {
                        separator: '=',
                        first: GameSystemListAtomShape::Crc32,
                        second: GameSystemListAtomShape::Number {
                            number_shape: GameSystemNumberShape::Float,
                        },
                        default_second_source_token: None,
                    }),
                }),
                foreign_keys: Vec::new(),
            },
        }],
    };
    let column = &schema.columns[0];

    let error = native_dev_cell_value(&schema, column, &OwnedCellValue::String("0.12".to_owned()))
        .expect_err("numeric scalar text must not be dropped from a pair-list field");

    assert!(
        error.to_string().contains("expected pair value"),
        "unexpected error: {error}"
    );
}

#[test]
fn boolean_source_text_values_reject_non_boolean_text() {
    let schema = GameSystemTableSchema {
        table_name: "DungeonDamageTable".to_owned(),
        table_name_crc: 1,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["damagedata/dungeondamagetable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "IsRanged".to_owned(),
            crc: 3,
            declared_type: ColumnType::String,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::Boolean,
        }],
    };
    let column = &schema.columns[0];

    let error = native_dev_cell_value(&schema, column, &OwnedCellValue::String("Fire".to_owned()))
        .expect_err("non-boolean text must not be coerced to false");

    assert!(error.to_string().contains("expected boolean text"));
}

#[test]
fn boolean_source_text_values_accept_yes_no_literals() {
    let schema = GameSystemTableSchema {
        table_name: "DataPointDataTable".to_owned(),
        table_name_crc: 1,
        row_type_name: "DataPointData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["datapointdata/datapointdatatable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "IsMajor".to_owned(),
            crc: 3,
            declared_type: ColumnType::String,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::Boolean,
        }],
    };
    let column = &schema.columns[0];

    let from_text =
        native_dev_cell_value(&schema, column, &OwnedCellValue::String("YES".to_owned()))
            .expect("yes bool text")
            .expect("cell value");

    assert!(matches!(
        from_text,
        NativeDevCellValue::Scalar(NativeDevScalarValue::Boolean(true))
    ));
}

#[test]
fn boolean_source_values_follow_repaired_text_affinity() {
    let schema = GameSystemTableSchema {
        table_name: "DungeonDamageTable".to_owned(),
        table_name_crc: 1,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["damagedata/dungeondamagetable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "DeflectDamageID".to_owned(),
            crc: 3,
            declared_type: ColumnType::Boolean,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: true,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: None,
                foreign_keys: Vec::new(),
            },
        }],
    };
    let column = &schema.columns[0];

    let from_bool = native_dev_cell_value(&schema, column, &OwnedCellValue::Boolean(false))
        .expect("native text bool")
        .expect("cell value");

    assert!(matches!(
        from_bool,
        NativeDevCellValue::Scalar(NativeDevScalarValue::String(value)) if value == "false"
    ));
}
