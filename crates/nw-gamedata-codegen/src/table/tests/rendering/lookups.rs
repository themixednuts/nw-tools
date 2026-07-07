use super::*;

#[test]
fn table_code_row_key_strings_take_precedence_over_list_shape() {
    let schema = GameSystemTableSchema {
        table_name: "EntitlementData".to_owned(),
        table_name_crc: 1,
        row_type_name: "EntitlementData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["javelindata_entitlements.datasheet".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "UniqueTagID".to_owned(),
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
                list: Some(GameSystemListShape {
                    separators: vec!["+".to_owned()],
                    rows_with_lists: 1,
                    total_entries: 2,
                    preserve_empty_entries: false,
                    element_shape: Some(GameSystemListElementShape::String),
                }),
                foreign_keys: Vec::new(),
            },
        }],
    };

    let rendered = render_test_table(&schema);
    assert!(
        rendered.contains("type Cell<'cell> = gamedata::RowKey<'cell, UniqueTagIdColumn>;"),
        "row-key columns must keep typed RowKey cells even when list-shaped source values were inferred"
    );
    assert!(
        !rendered.contains("type Cell<'cell> = gamedata::List<'cell, UniqueTagIdColumn"),
        "row-key columns must not expose list cells"
    );
    assert!(
        !rendered.contains("const LIST_ELEMENT_"),
        "row-key columns must not emit list element metadata"
    );
    let rendered_schema = render_test_schema(&schema);
    assert!(rust_source_contains(
        &rendered_schema,
        "pub struct EntitlementDataRow {
            #[schema(id = 3)]
            pub unique_tag_id: String,
        }",
    ));
    assert!(rust_source_contains(
        &rendered_schema,
        "type PrimaryKeyValue = String;",
    ));

    let value = native_dev_string_cell_value(&schema.columns[0], "Ent_A+Ent_B")
        .expect("row-key value")
        .expect("row-key cell");
    assert!(
        matches!(
            value,
            NativeDevCellValue::Scalar(NativeDevScalarValue::String(value))
                if value == "Ent_A+Ent_B"
        ),
        "row-key RON must keep the scalar source key instead of splitting it"
    );
}

#[test]
fn table_code_story_progress_emits_keyless_crc_list_surface() {
    let schema = GameSystemTableSchema {
        table_name: "StoryProgress".to_owned(),
        table_name_crc: 1,
        row_type_name: "StoryProgressData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["datatables/javelindata_storyprogress.datasheet".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "AchievementIds".to_owned(),
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
                        rows_with_lists: 0,
                        total_entries: 1,
                        preserve_empty_entries: false,
                        element_shape: Some(GameSystemListElementShape::Crc32),
                    }),
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "ActivityTaskName".to_owned(),
                crc: 4,
                declared_type: ColumnType::String,
                row_key: false,
                required: true,
                non_empty_rows: 1,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Crc32,
            },
        ],
    };

    let rendered = render_test_table(&schema);
    assert!(rendered.contains(
        "type Cell<'cell> = gamedata::List<'cell, AchievementIdsColumn, az_core::crc::Crc32>;"
    ));
    assert!(rendered.contains("type Cell<'cell> = az_core::crc::Crc32;"));
    assert!(rendered.contains("const ROW_KEY: bool = false;"));
    assert!(!rendered.contains("pub type StoryProgressKey"));
    assert!(!rendered.contains("impl gamedata::KeyColumn for AchievementIdsColumn"));
    assert!(!rendered.contains("impl<'a> gamedata::game_system::KeyedTableView<'a>"));
    assert!(!rendered.contains("pub fn get(&self, key:"));

    let value = native_dev_string_cell_value(&schema.columns[0], "09A_M15")
        .expect("AchievementIds value")
        .expect("AchievementIds cell");
    assert!(
        matches!(
            value,
            NativeDevCellValue::List(values)
                if matches!(
                    &values[..],
                    [NativeDevCellValue::Scalar(NativeDevScalarValue::String(value))]
                        if value == "09A_M15"
                )
        ),
        "CRC list RON must keep designer tokens instead of serialized CRC integers"
    );
}

#[test]
fn table_code_keyless_tables_do_not_emit_keyed_lookup() {
    let schema = GameSystemTableSchema {
        table_name: "NpcConversation".to_owned(),
        table_name_crc: 1,
        row_type_name: "ConversationTopicData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["conversationtopicdata/npc_conversation.ron".to_owned()],
        columns: Vec::new(),
    };

    let rendered = render_test_table(&schema);

    assert!(
        rust_source_contains(&rendered, "pub fn row_at(")
            && rust_source_contains(&rendered, "row: gamedata::RowIndex")
            && rust_source_contains(&rendered, "pub fn rows(&self)")
            && rendered.contains("impl<'a> gamedata::game_system::SystemView<'a>")
            && rust_source_contains(&rendered, "token: gamedata::game_system::SystemViewToken",)
            && rust_source_contains(&rendered, "system.table::<NpcConversation>(token)?")
            && rust_source_contains(&rendered, "fn from_system("),
        "expected keyless tables to keep typed row traversal and token-gated view construction"
    );
    assert!(
        !rendered.contains("pub type NpcConversationKey")
            && !rendered.contains("pub fn key(name:")
            && !rendered.contains("pub fn get(&self, key:")
            && !rendered.contains("fn new(system: &'a gamedata::game_system::System)")
            && !rendered.contains("fn new(")
            && !rendered.contains("impl<'a> gamedata::game_system::KeyedTableView<'a>")
            && !rendered.contains("impl gamedata::KeyColumn"),
        "keyless tables must not generate a fake keyed API"
    );
}

#[test]
fn table_code_derives_crcs_from_names() {
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
    let compact = compact_rust_source(&rendered);
    let crc_derivation = compact_rust_source(
        "const CRC: u32 = az_core::crc::Crc32::from_str_lower(Self::NAME).value();",
    );
    assert!(compact.matches(&crc_derivation).count() == 2);
    assert!(
        !rendered.contains("const CRC: u32 = 1;") && !rendered.contains("const CRC: u32 = 2;"),
        "table CRCs must not be raw literals"
    );
}
