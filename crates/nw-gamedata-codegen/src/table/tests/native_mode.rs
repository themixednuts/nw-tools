use super::*;

fn editable_ron_test_catalog() -> GameSystemCatalog {
    let mut catalog = GameSystemCatalog::default();
    catalog
        .insert(
            GameSystemTable::from_native_columns(
                "SampleTable",
                1,
                "SampleRow",
                2,
                vec![
                    GameSystemColumn::new(3, "SampleId", ColumnType::String),
                    GameSystemColumn::new(4, "DisplayName", ColumnType::String),
                    GameSystemColumn::new(5, "Level", ColumnType::Number),
                    GameSystemColumn::new(6, "Enabled", ColumnType::Boolean),
                ],
                [(
                    7,
                    vec![
                        GameSystemCell::new(3, OwnedCellValue::String("sample".to_owned())),
                        GameSystemCell::new(4, OwnedCellValue::String("Sample".to_owned())),
                        GameSystemCell::new(5, OwnedCellValue::Number(12.0)),
                        GameSystemCell::new(6, OwnedCellValue::Boolean(true)),
                    ],
                )],
            )
            .with_source_asset(GameSystemAsset::with_asset_id(
                "resources/datasheets/sample.datasheet",
                AssetId::new(Uuid::from_u128(0x1234), 0),
            )),
        )
        .expect("insert sample table");
    catalog
}

fn strip_generated_header(bytes: &[u8]) -> &[u8] {
    bytes
        .strip_prefix(nw_generated_guard::GENERATED_HEADER.as_bytes())
        .unwrap_or(bytes)
}

#[test]
fn editable_ron_emission_returns_authored_path_and_provenance() {
    let catalog = editable_ron_test_catalog();
    let emission = emit_editable_gamedata_ron_sources(&catalog, GameDataCompileMode::Strict)
        .expect("emit editable RON");

    assert_eq!(emission.files.len(), 1);
    assert_eq!(emission.schema_report.tables.len(), 1);
    let file = &emission.files[0];
    assert_eq!(file.source_path, "gamedata/sample_row/sample_table.ron");
    assert_eq!(file.table_name, "SampleTable");
    assert_eq!(file.row_type_name, "SampleRow");
    assert_eq!(file.datasheet_sources.len(), 1);
    assert_eq!(
        file.datasheet_sources[0].path(),
        std::path::Path::new("resources/datasheets/sample.datasheet")
    );
    assert_eq!(
        file.datasheet_sources[0].asset_id(),
        Some(AssetId::new(Uuid::from_u128(0x1234), 0))
    );

    let ron = std::str::from_utf8(&file.bytes).expect("RON is UTF-8");
    assert!(ron.contains("sample_id: \"sample\""));
    assert!(ron.contains("display_name: \"Sample\""));
    assert!(ron.contains("level: 12"));
    assert!(ron.contains("enabled: true"));
}

#[test]
fn editable_ron_emission_matches_authored_ron_write_path() {
    let catalog = editable_ron_test_catalog();
    let emission = emit_editable_gamedata_ron_sources(&catalog, GameDataCompileMode::Strict)
        .expect("emit editable RON");
    let file = emission.files.first().expect("emitted RON file");

    let temp = tempfile::tempdir().expect("temp output");
    let output = compile_catalog_with_mode_and_source_format(
        &catalog,
        GameDataOutputRoots {
            table_code_root: &temp.path().join("table_code"),
            source_index_code_root: None,
            table_source_root: &temp.path().join("assets"),
            diagnostics_root: &temp.path().join("diagnostics"),
        },
        GameDataCompileMode::Strict,
        GameDataTableSourceFormat::AuthoredRon,
    )
    .expect("compile authored RON output");

    let authored_path = output.table_source_root_path.join(&file.source_path);
    let authored_bytes = fs::read(&authored_path).expect("read authored RON output");
    assert_eq!(
        strip_generated_header(&authored_bytes),
        file.bytes.as_slice()
    );
}

#[test]
fn source_format_mode_keeps_declared_source_cell_kinds() {
    let mut catalog = GameSystemCatalog::default();
    catalog
        .insert(GameSystemTable::from_native_columns(
            "SampleTable",
            1,
            "SampleRow",
            2,
            vec![
                GameSystemColumn::new(3, "SampleId", ColumnType::String),
                GameSystemColumn::new(4, "ReferenceList", ColumnType::String),
                GameSystemColumn::new(5, "Weight", ColumnType::Number),
                GameSystemColumn::new(6, "Enabled", ColumnType::Boolean),
            ],
            [(
                7,
                vec![
                    GameSystemCell::new(3, OwnedCellValue::String("sample".to_owned())),
                    GameSystemCell::new(4, OwnedCellValue::String("A,B".to_owned())),
                    GameSystemCell::new(5, OwnedCellValue::Number(12.5)),
                    GameSystemCell::new(6, OwnedCellValue::Boolean(true)),
                ],
            )],
        ))
        .expect("insert sample table");

    let report = schema_report_for_mode(&catalog, GameDataCompileMode::SourceFormat);
    let table = report.tables.first().expect("schema table");
    let reference_list = &table.columns[1];
    let weight = &table.columns[2];
    let enabled = &table.columns[3];

    assert_eq!(reference_list.declared_type, ColumnType::String);
    assert!(matches!(
        &reference_list.value_shape,
        GameSystemColumnValueShape::String {
            list: None,
            foreign_keys,
            ..
        } if foreign_keys.is_empty()
    ));
    assert!(matches!(
        weight.value_shape,
        GameSystemColumnValueShape::Number {
            number_shape: GameSystemNumberShape::Float
        }
    ));
    assert!(matches!(
        enabled.value_shape,
        GameSystemColumnValueShape::Boolean
    ));
    assert!(report.type_affinities.is_empty());
    assert!(report.diagnostics.is_empty());
}

#[test]
fn source_format_compile_defaults_to_game_source_paths() {
    let mut catalog = GameSystemCatalog::default();
    let source_path = "resources/datasheets/sample.datasheet";
    catalog
        .insert(
            GameSystemTable::from_native_columns(
                "SampleTable",
                1,
                "SampleRow",
                2,
                vec![GameSystemColumn::new(3, "SampleId", ColumnType::String)],
                [(
                    4,
                    vec![GameSystemCell::new(
                        3,
                        OwnedCellValue::String("sample".to_owned()),
                    )],
                )],
            )
            .with_source_asset(GameSystemAsset::with_asset_id(
                source_path,
                AssetId::new(Uuid::from_u128(1), 0),
            )),
        )
        .expect("insert sample table");

    let temp = tempfile::tempdir().expect("temp output");
    let output = compile_catalog_with_mode(
        &catalog,
        GameDataOutputRoots {
            table_code_root: &temp.path().join("table_code"),
            source_index_code_root: None,
            table_source_root: &temp.path().join("assets"),
            diagnostics_root: &temp.path().join("diagnostics"),
        },
        GameDataCompileMode::SourceFormat,
    )
    .expect("compile source format output");

    assert_eq!(
        output.table_source_format,
        GameDataTableSourceFormat::Datasheet
    );
    assert_eq!(output.table_source_files, 1);
    assert!(!output.table_source_root_path.join("gamedata").exists());
    assert!(output.ron_transform_audit_path.is_none());
    assert_eq!(output.ron_transform_audit_entries, 0);
    let summary = compile_summary(&output);
    assert!(summary.contains("datasheet source paths"));
    assert!(!summary.contains("RON"));

    let source_index_chunk = fs::read_to_string(output.table_code_root_path.join("chunk_000.rs"))
        .expect("source index chunk");
    assert!(source_index_chunk.contains(source_path));
    assert!(source_index_chunk.contains("tables/sample_row/sample_table.aztbl"));
    assert!(!source_index_chunk.contains(".ron"));
}

#[test]
fn source_format_mode_emits_numeric_row_keys_as_integer_keys() {
    let mut catalog = GameSystemCatalog::default();
    catalog
        .insert(GameSystemTable::from_native_columns(
            "TerritoryTable",
            1,
            "TerritoryData",
            2,
            vec![
                GameSystemColumn::new(3, "TerritoryID", ColumnType::Number),
                GameSystemColumn::new(4, "Difficulty", ColumnType::Number),
            ],
            [
                (
                    7,
                    vec![
                        GameSystemCell::new(3, OwnedCellValue::Number(1.0)),
                        GameSystemCell::new(4, OwnedCellValue::Number(0.5)),
                    ],
                ),
                (
                    8,
                    vec![
                        GameSystemCell::new(3, OwnedCellValue::Number(2.0)),
                        GameSystemCell::new(4, OwnedCellValue::Number(1.25)),
                    ],
                ),
            ],
        ))
        .expect("insert territory table");

    let report = schema_report_for_mode(&catalog, GameDataCompileMode::SourceFormat);
    let table = report.tables.first().expect("schema table");
    let territory_id = &table.columns[0];
    let difficulty = &table.columns[1];

    assert!(territory_id.row_key);
    assert!(matches!(
        territory_id.value_shape,
        GameSystemColumnValueShape::Number {
            number_shape: GameSystemNumberShape::NonNegativeInteger
        }
    ));
    assert!(matches!(
        difficulty.value_shape,
        GameSystemColumnValueShape::Number {
            number_shape: GameSystemNumberShape::Float
        }
    ));
}
