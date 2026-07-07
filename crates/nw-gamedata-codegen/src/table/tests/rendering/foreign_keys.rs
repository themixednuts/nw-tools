use super::*;

#[test]
fn table_code_foreign_keys_reference_exact_columns() {
    let target_schema = GameSystemTableSchema {
        table_name: "Backstory".to_owned(),
        table_name_crc: 20,
        row_type_name: "BackstoryDefinition".to_owned(),
        row_type_crc: 21,
        row_count: 1,
        sources: vec!["backstorydefinition/backstory.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "BackstoryID".to_owned(),
            crc: 22,
            declared_type: ColumnType::String,
            row_key: true,
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
    let schema = GameSystemTableSchema {
        table_name: "ArchetypeDataTable".to_owned(),
        table_name_crc: 10,
        row_type_name: "ArchetypeData".to_owned(),
        row_type_crc: 11,
        row_count: 1,
        sources: vec!["archetypedata/archetypedatatable.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "ArchetypeID".to_owned(),
                crc: 12,
                declared_type: ColumnType::String,
                row_key: true,
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
            },
            GameSystemColumnSchema {
                name: "BackstoryID".to_owned(),
                crc: 13,
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
                    list: None,
                    foreign_keys: vec![
                        GameSystemForeignKeyCandidate {
                            target_table: "MissingBackstoryDefinition".to_owned(),
                            target_column: "BackstoryID".to_owned(),
                            checked_values: 1,
                            matched_values: 1,
                            missing_values: 0,
                            confidence: 1.0,
                        },
                        GameSystemForeignKeyCandidate {
                            target_table: "BackstoryDefinition".to_owned(),
                            target_column: "BackstoryID".to_owned(),
                            checked_values: 1,
                            matched_values: 1,
                            missing_values: 0,
                            confidence: 0.95,
                        },
                    ],
                },
            },
        ],
    };

    let rendered = render_test_table_with_schemas(&schema, &[target_schema.clone()]);
    assert!(rust_source_contains(
        &rendered,
        "type Cell<'cell> = gamedata::ForeignKey<'cell, super::super::backstory_definition::backstory::BackstoryIdColumn"
    ));
    assert!(rust_source_contains(
        &rendered,
        "const FOREIGN_KEYS: &'static [super::super::ForeignKeyMeta] = &[super::super::ForeignKeyMeta::of::<super::super::backstory_definition::backstory::BackstoryIdColumn"
    ));
    assert!(
        !rendered.contains("target_table:") && !rendered.contains("target_column:"),
        "table FK metadata must not contain literal target strings"
    );
}

#[test]
fn table_code_foreign_keys_keep_low_confidence_partial_matches_string() {
    let target_schema = GameSystemTableSchema {
        table_name: "VitalsCategories".to_owned(),
        table_name_crc: 20,
        row_type_name: "VitalsCategoryData".to_owned(),
        row_type_crc: 21,
        row_count: 1,
        sources: vec!["vitalscategorydata/vitalscategories.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "VitalsCategoryID".to_owned(),
            crc: 22,
            declared_type: ColumnType::String,
            row_key: true,
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
    let schema = GameSystemTableSchema {
        table_name: "HunterSight".to_owned(),
        table_name_crc: 10,
        row_type_name: "HunterSightData".to_owned(),
        row_type_crc: 11,
        row_count: 2,
        sources: vec!["huntersightdata/huntersight.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "SightID".to_owned(),
                crc: 12,
                declared_type: ColumnType::String,
                row_key: true,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "SightCategoryFlag".to_owned(),
                crc: 13,
                declared_type: ColumnType::String,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: false,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: vec![GameSystemForeignKeyCandidate {
                        target_table: "VitalsCategoryData".to_owned(),
                        target_column: "VitalsCategoryID".to_owned(),
                        checked_values: 2,
                        matched_values: 1,
                        missing_values: 1,
                        confidence: 0.5,
                    }],
                },
            },
        ],
    };

    let rendered = render_test_table_with_schemas(&schema, &[target_schema]);
    assert!(rendered.contains("type Cell<'cell> = &'cell str;"));
    assert!(!rendered.contains("ForeignKeyMeta::of::<"));
    assert!(!rendered.contains("gamedata::ForeignKey<'cell"));
}

#[test]
fn table_code_foreign_keys_reference_strong_partial_list_columns() {
    let target_schema = GameSystemTableSchema {
        table_name: "AchievementDataTable".to_owned(),
        table_name_crc: 20,
        row_type_name: "AchievementData".to_owned(),
        row_type_crc: 21,
        row_count: 1,
        sources: vec!["achievementdata/achievementdatatable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "AchievementID".to_owned(),
            crc: 22,
            declared_type: ColumnType::String,
            row_key: true,
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
    let schema = GameSystemTableSchema {
        table_name: "Backstory".to_owned(),
        table_name_crc: 10,
        row_type_name: "BackstoryDefinition".to_owned(),
        row_type_crc: 11,
        row_count: 2,
        sources: vec!["backstorydefinition/backstory.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "BackstoryID".to_owned(),
                crc: 12,
                declared_type: ColumnType::String,
                row_key: true,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "AchievementUnlockOverride".to_owned(),
                crc: 13,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: Some(GameSystemListShape {
                        separators: vec![",".to_owned()],
                        rows_with_lists: 2,
                        total_entries: 4,
                        preserve_empty_entries: false,
                        element_shape: Some(GameSystemListElementShape::String),
                    }),
                    foreign_keys: vec![GameSystemForeignKeyCandidate {
                        target_table: "AchievementData".to_owned(),
                        target_column: "AchievementID".to_owned(),
                        checked_values: 240,
                        matched_values: 232,
                        missing_values: 8,
                        confidence: 0.9666666666666667,
                    }],
                },
            },
        ],
    };

    let rendered = render_test_table_with_schemas(&schema, &[target_schema]);

    assert!(rust_source_contains(
        &rendered,
        "type Cell<'cell> = gamedata::List<'cell, AchievementUnlockOverrideColumn, gamedata::ForeignKey<'cell, super::super::achievement_data::achievement_data_table::AchievementIdColumn"
    ));
    assert!(!rendered.contains("const LIST_ELEMENT_"));
    assert!(rust_source_contains(
        &rendered,
        "const FOREIGN_KEYS: &'static [super::super::ForeignKeyMeta] = &[super::super::ForeignKeyMeta::of::<super::super::achievement_data::achievement_data_table::AchievementIdColumn"
    ));
    assert!(
        !rendered.contains("target_table:") && !rendered.contains("target_column:"),
        "table FK metadata must not contain literal target strings"
    );
}

#[test]
fn table_code_foreign_keys_keep_non_row_key_targets_string_with_metadata() {
    let target_schema = GameSystemTableSchema {
        table_name: "Backstory".to_owned(),
        table_name_crc: 20,
        row_type_name: "BackstoryDefinition".to_owned(),
        row_type_crc: 21,
        row_count: 1,
        sources: vec!["backstorydefinition/backstory.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "BackstoryID".to_owned(),
                crc: 22,
                declared_type: ColumnType::String,
                row_key: true,
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
            },
            GameSystemColumnSchema {
                name: "DisplayName".to_owned(),
                crc: 23,
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
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
        ],
    };
    let schema = GameSystemTableSchema {
        table_name: "ArchetypeDataTable".to_owned(),
        table_name_crc: 10,
        row_type_name: "ArchetypeData".to_owned(),
        row_type_crc: 11,
        row_count: 1,
        sources: vec!["archetypedata/archetypedatatable.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "ArchetypeID".to_owned(),
                crc: 12,
                declared_type: ColumnType::String,
                row_key: true,
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
            },
            GameSystemColumnSchema {
                name: "BackstoryDisplayName".to_owned(),
                crc: 13,
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
                    list: None,
                    foreign_keys: vec![GameSystemForeignKeyCandidate {
                        target_table: "BackstoryDefinition".to_owned(),
                        target_column: "DisplayName".to_owned(),
                        checked_values: 1,
                        matched_values: 1,
                        missing_values: 0,
                        confidence: 1.0,
                    }],
                },
            },
        ],
    };
    let rendered = render_test_table_with_schemas(&schema, &[target_schema]);

    assert!(
        rendered.contains("type Cell<'cell> = &'cell str;"),
        "non-row-key inferred FK targets must stay string cells"
    );
    assert!(!rendered.contains("const DATA_"));
    assert!(
        !rendered.contains("gamedata::ForeignKey<'cell"),
        "non-row-key inferred FK targets must not generate row-index FK cells"
    );
    assert!(
        rust_source_contains(
            &rendered,
            "const FOREIGN_KEYS: &'static [super::super::ForeignKeyMeta] = &[super::super::ForeignKeyMeta::of::<super::super::backstory_definition::backstory::DisplayNameColumn"
        ),
        "non-row-key inferred FK targets should generate typed metadata"
    );
}

#[test]
fn table_code_foreign_keys_emit_same_table_non_row_key_metadata() {
    let schema = GameSystemTableSchema {
        table_name: "PerkBuckets".to_owned(),
        table_name_crc: 30,
        row_type_name: "PerkBucketData".to_owned(),
        row_type_crc: 31,
        row_count: 2,
        sources: vec!["perkbucketdata/perkbuckets.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "RowID".to_owned(),
                crc: 32,
                declared_type: ColumnType::String,
                row_key: true,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "BucketAliasID".to_owned(),
                crc: 33,
                declared_type: ColumnType::String,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "ParentBucketAliasID".to_owned(),
                crc: 34,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: vec![GameSystemForeignKeyCandidate {
                        target_table: "PerkBucketData".to_owned(),
                        target_column: "BucketAliasID".to_owned(),
                        checked_values: 1,
                        matched_values: 1,
                        missing_values: 0,
                        confidence: 1.0,
                    }],
                },
            },
        ],
    };

    let rendered = render_test_table_with_schemas(&schema, &[]);

    assert!(rendered.contains("type Cell<'cell> = &'cell str;"));
    assert!(!rendered.contains("gamedata::ForeignKey<'cell"));
    assert!(rust_source_contains(
        &rendered,
        "const FOREIGN_KEYS: &'static [super::super::ForeignKeyMeta] = &[super::super::ForeignKeyMeta::of::<super::super::perk_bucket_data::perk_buckets::BucketAliasIdColumn"
    ));
}

#[test]
fn table_code_multi_table_foreign_keys_stay_string_until_exact_target() {
    let target_schema_a = GameSystemTableSchema {
        table_name: "BackstoryA".to_owned(),
        table_name_crc: 20,
        row_type_name: "BackstoryDefinition".to_owned(),
        row_type_crc: 21,
        row_count: 1,
        sources: vec!["backstorydefinition/backstorya.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "BackstoryID".to_owned(),
            crc: 22,
            declared_type: ColumnType::String,
            row_key: true,
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
    let target_schema_b = GameSystemTableSchema {
        table_name: "BackstoryB".to_owned(),
        table_name_crc: 23,
        row_type_name: "BackstoryDefinition".to_owned(),
        row_type_crc: 21,
        row_count: 1,
        sources: vec!["backstorydefinition/backstoryb.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "BackstoryID".to_owned(),
            crc: 22,
            declared_type: ColumnType::String,
            row_key: true,
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
    let schema = GameSystemTableSchema {
        table_name: "ArchetypeDataTable".to_owned(),
        table_name_crc: 10,
        row_type_name: "ArchetypeData".to_owned(),
        row_type_crc: 11,
        row_count: 1,
        sources: vec!["archetypedata/archetypedatatable.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "ArchetypeID".to_owned(),
                crc: 12,
                declared_type: ColumnType::String,
                row_key: true,
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
            },
            GameSystemColumnSchema {
                name: "BackstoryID".to_owned(),
                crc: 13,
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
                    list: None,
                    foreign_keys: vec![GameSystemForeignKeyCandidate {
                        target_table: "BackstoryDefinition".to_owned(),
                        target_column: "BackstoryID".to_owned(),
                        checked_values: 1,
                        matched_values: 1,
                        missing_values: 0,
                        confidence: 1.0,
                    }],
                },
            },
        ],
    };
    let report = GameSystemCatalogSchemaReport {
        tables: vec![target_schema_a, target_schema_b, schema.clone()],
        type_affinities: Vec::new(),
        diagnostics: Vec::new(),
    };
    let table_code_columns = table_code_column_index(&report);
    let rendered_table =
        render_table_code_rs(&schema, &table_code_columns, &table_source_path(&schema))
            .expect("render table rs");

    assert!(rendered_table.contains("type Cell<'cell> = &'cell str;"));
    assert!(!rendered_table.contains("const DATA_"));
    assert!(
        !rendered_table.contains("ForeignKeyMeta::of::<")
            && !rendered_table.contains("foreign_key_targets")
            && !rendered_table.contains("gamedata::ForeignKey<'cell"),
        "ambiguous FK candidates must not generate a pretend typed target"
    );
}
