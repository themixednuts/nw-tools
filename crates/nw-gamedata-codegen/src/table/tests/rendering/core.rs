use super::*;

#[test]
fn shared_schema_module_emits_authored_row_schema() {
    let schema = GameSystemTableSchema {
        table_name: "Backstory".to_owned(),
        table_name_crc: 1,
        row_type_name: "BackstoryDefinition".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["backstorydefinition/backstory.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "BackstoryID".to_owned(),
                crc: 3,
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
                crc: 4,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: false,
                    localized_key_like: true,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "PowerLevel".to_owned(),
                crc: 5,
                declared_type: ColumnType::Number,
                row_key: false,
                required: true,
                non_empty_rows: 1,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::Float,
                },
            },
            GameSystemColumnSchema {
                name: "Tags".to_owned(),
                crc: 6,
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
                        separators: vec![",".to_owned()],
                        rows_with_lists: 1,
                        total_entries: 2,
                        preserve_empty_entries: false,
                        element_shape: Some(GameSystemListElementShape::String),
                    }),
                    foreign_keys: Vec::new(),
                },
            },
        ],
    };

    let rendered = render_test_schema(&schema);

    assert!(rust_source_contains(
        &rendered,
        "#[derive(Debug, Clone, PartialEq, gamedata::authoring::az_schema::AzSchema)]"
    ));
    assert!(rust_source_contains(
        &rendered,
        "#[schema(name = \"newworld.gamedata.BackstoryDefinition\", version = 1)]"
    ));
    assert!(rust_source_contains(
        &rendered,
        "pub struct BackstoryDefinitionRow {
            #[schema(id = 3)]
            pub backstory_id: String,
            #[schema(id = 4)]
            pub display_name: Option<String>,
            #[schema(id = 5)]
            pub power_level: f32,
            #[schema(id = 6)]
            pub tags: Option<Vec<String>>,
        }",
    ));
    assert!(rust_source_contains(
        &rendered,
        "impl gamedata::authoring::GameDataRow for BackstoryDefinitionRow"
    ));
    assert!(rust_source_contains(
        &rendered,
        "type PrimaryKeyValue = String;"
    ));
    assert!(rendered.contains("PRIMARY_KEY_FIELD"));
    assert!(rust_source_contains(&rendered, "FieldId(3,)"));
    assert!(rust_source_contains(
        &rendered,
        "const PRIMARY_KEY_FIELD_NAME: &'static str = stringify!(backstory_id);"
    ));
    assert!(rust_source_contains(
        &rendered,
        "fn primary_key(&self) -> &Self::PrimaryKeyValue {
            &self.backstory_id
        }"
    ));
}

#[test]
fn table_code_uses_typed_table_columns() {
    let schema = GameSystemTableSchema {
        table_name: "Backstory".to_owned(),
        table_name_crc: 1,
        row_type_name: "BackstoryDefinition".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["backstorydefinition/backstory.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "BackstoryID".to_owned(),
            crc: 3,
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

    let rendered = render_test_table(&schema);
    assert!(
        !rendered.contains("SOURCE_PATH"),
        "runtime table markers must not expose authored source paths"
    );
    assert!(
        rendered.contains("impl gamedata::TableColumn for BackstoryIdColumn"),
        "expected typed column marker in table code output"
    );
    assert!(
        rendered.contains("ColumnMeta::<Backstory>::of::<BackstoryIdColumn>()"),
        "expected typed column metadata in table code output"
    );
    assert!(
        rendered.contains("ColumnSlot<BackstoryIdColumn>"),
        "expected ColumnSlot in table-code view"
    );
    assert!(
        rendered.contains("impl gamedata::KeyColumn for BackstoryIdColumn {")
            && rendered.contains("type Key<'key> = &'key str;"),
        "expected row-key column marker"
    );
    assert!(
        rendered.contains("type Cell<'cell> = gamedata::RowKey<'cell, BackstoryIdColumn>;"),
        "expected row-key cells to use borrowed typed key refs"
    );
    assert!(
        rendered.contains("pub type BackstoryKey = gamedata::TableKey<BackstoryIdColumn>;"),
        "expected typed table key alias"
    );
    assert!(rendered.contains("pub fn key(key: &'_ str) -> BackstoryKey"));
    assert!(
        rendered.contains("pub fn get(&self, key: &BackstoryKey)"),
        "expected typed row lookup in table-code view"
    );
    assert!(
        rendered.contains("pub fn len(&self) -> usize")
            && rendered.contains("pub fn is_empty(&self) -> bool")
            && !rendered.contains("pub fn row_count(&self) -> usize"),
        "expected table views to use Rust collection naming"
    );
    assert!(
        rendered.contains("pub fn row_at(&self, row: gamedata::RowIndex)"),
        "expected row-index traversal to use the RowIndex newtype"
    );
    assert!(
        rendered.contains("row: gamedata::game_system::RowRef<'a, Backstory>"),
        "expected row refs to store typed game-system row refs"
    );
    assert!(
        !rendered.contains("pub fn row(&self, row: u32)")
            && !rendered.contains("pub fn row_index(&self) -> u32"),
        "table views must not expose raw row-index escape hatches"
    );
    assert!(
        !rendered.contains("impl AsRef<str>") && !rendered.contains("Crc32::from_str_lower(key"),
        "table row lookup must not accept stringly keys"
    );
    assert!(
        !rendered.contains("FieldKind"),
        "table code output must not use FieldKind schema"
    );
}

#[test]
fn multi_table_type_modules_emit_exact_family_dispatch() {
    let first = GameSystemTableSchema {
        table_name: "PlayerDamageTable".to_owned(),
        table_name_crc: 1,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["damagedata/playerdamagetable.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "DamageID".to_owned(),
                crc: 3,
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
                name: "PowerLevel".to_owned(),
                crc: 4,
                declared_type: ColumnType::Number,
                row_key: false,
                required: true,
                non_empty_rows: 1,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::Float,
                },
            },
        ],
    };
    let second = GameSystemTableSchema {
        table_name: "AiDamageTable".to_owned(),
        table_name_crc: 5,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["damagedata/aidamagetable.ron".to_owned()],
        columns: first.columns.clone(),
    };
    let rendered = render_test_family_mod(&[first, second]);

    assert!(rendered.contains("pub enum DamageDataTable"));
    assert!(rendered.contains("pub struct DamageDataHandle"));
    assert!(rendered.contains("pub const fn name(self) -> &'static str"));
    assert!(rendered.contains("pub const fn crc(self) -> u32"));
    assert!(rendered.contains("pub fn from_name(name: &str) -> Option<Self>"));
    assert!(
        rendered
            .contains("pub const fn new(table: DamageDataTable, row: gamedata::RowIndex) -> Self")
    );
    assert!(!rendered.contains("pub struct DamageDataTables<'a>"));
    assert!(!rendered.contains("pub fn visit_rows<F>"));
    assert!(!rendered.contains("pub enum DamageDataRow<'t, 'a>"));
    assert!(!rendered.contains("DamageDataDamageIdCell"));
    assert!(!rendered.contains("DamageDataPowerLevelCell"));
    assert!(
        !rendered.contains("macro_rules!")
            && !rendered.contains(&["required", "_u32"].concat())
            && !rendered.contains(&["optional", "_u32"].concat())
            && !rendered.contains("parse_u32_cell")
            && !rendered.contains("to_f32")
            && !rendered.contains("to_bool")
            && !rendered.contains("to_u64"),
        "family identity output must stay typed and macro-free"
    );
}

#[test]
fn mixed_family_scalar_columns_do_not_emit_family_cell_enums() {
    let numeric = GameSystemTableSchema {
        table_name: "DamageTable".to_owned(),
        table_name_crc: 1,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["damagedata/damagetable.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "DamageID".to_owned(),
                crc: 3,
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
                name: "ImpactRating".to_owned(),
                crc: 4,
                declared_type: ColumnType::Number,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::Float,
                },
            },
        ],
    };
    let mut text = numeric.clone();
    text.table_name = "CorruptedLegionCyclopsDamageTable".to_owned();
    text.table_name_crc = 5;
    text.sources = vec!["damagedata/corruptedlegioncyclopsdamagetable.ron".to_owned()];
    text.columns[1].declared_type = ColumnType::String;
    text.columns[1].value_shape = GameSystemColumnValueShape::String {
        identifier_like: true,
        localized_key_like: false,
        asset_path_like: false,
        expression_like: false,
        list: None,
        foreign_keys: Vec::new(),
    };

    let rendered = render_test_family_mod(&[numeric, text]);

    assert!(!rendered.contains("pub enum DamageDataImpactRatingCell<'a>"));
    assert!(!rendered.contains("pub enum DamageDataImpactRatingValue<'a>"));
    assert!(
        !rendered.contains("pub fn as_f32(self)"),
        "family identity output must not expose lossy cast helpers"
    );
}

#[test]
fn multi_table_type_modules_do_not_reexport_family_enum_collision() {
    let first = GameSystemTableSchema {
        table_name: "CameraShakeDataTable".to_owned(),
        table_name_crc: 1,
        row_type_name: "CameraShakeData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["camerashakedata/camerashakedatatable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "CameraShakeID".to_owned(),
            crc: 3,
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
    let mut second = first.clone();
    second.table_name = "CameraShakeDataTableIsleOfNight".to_owned();
    second.table_name_crc = 4;
    second.sources = vec!["camerashakedata/camerashakedatatable_isleofnight.ron".to_owned()];
    let schemas = [first, second];
    let family = render_test_family_mod(&schemas);
    let rendered = render_test_type_mod(&schemas);

    assert!(family.contains("pub enum CameraShakeDataTable"));
    assert!(!rendered.contains("pub use camera_shake_data_table::CameraShakeDataTable;"));
    assert!(rendered.contains(
        "pub use camera_shake_data_table_isle_of_night::CameraShakeDataTableIsleOfNight;"
    ));
}
