use super::*;

#[test]
fn family_identity_does_not_emit_string_sequence_dispatch_for_mixed_row_keys_and_string_lists() {
    let first = GameSystemTableSchema {
        table_name: "PlayerDamageTable".to_owned(),
        table_name_crc: 1,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["damagedata/playerdamagetable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
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
        }],
    };
    let second = GameSystemTableSchema {
        table_name: "DynastyEmpressDamageTable".to_owned(),
        table_name_crc: 4,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 2,
        row_count: 1,
        sources: vec!["damagedata/dynastyempressdamagetable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "DamageID".to_owned(),
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
                    element_shape: Some(GameSystemListElementShape::String),
                }),
                foreign_keys: Vec::new(),
            },
        }],
    };
    let rendered = render_test_family_mod(&[first, second]);

    assert!(rendered.contains("pub enum DamageDataTable"));
    assert!(rendered.contains("pub struct DamageDataHandle"));
    assert!(!rendered.contains("pub enum DamageDataDamageIdCell<'a>"));
    assert!(!rendered.contains("pub fn strings(self)"));
    assert!(!rendered.contains("collect_string_list(values).map(non_empty_strings)"));
}

#[test]
fn family_identity_does_not_emit_string_sequence_dispatch_for_mixed_text_and_foreign_keys() {
    let damage_table = GameSystemTableSchema {
        table_name: "DamageTable".to_owned(),
        table_name_crc: 4,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 5,
        row_count: 1,
        sources: vec!["damagedata/damagetable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "Affliction".to_owned(),
            crc: 6,
            declared_type: ColumnType::String,
            row_key: false,
            required: false,
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
                    target_table: "Afflictions".to_owned(),
                    target_column: "AfflictionID".to_owned(),
                    checked_values: 1,
                    matched_values: 1,
                    missing_values: 0,
                    confidence: 1.0,
                }],
            },
        }],
    };
    let elemental_damage_table = GameSystemTableSchema {
        table_name: "ElementalDamageTable".to_owned(),
        table_name_crc: 7,
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 5,
        row_count: 1,
        sources: vec!["damagedata/elementaldamagetable.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "Affliction".to_owned(),
            crc: 6,
            declared_type: ColumnType::String,
            row_key: false,
            required: false,
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

    let mut group = TableCodeTypeGroup {
        row_type_name: "DamageData".to_owned(),
        row_type_crc: 5,
        used_table_modules: BTreeSet::new(),
        used_table_markers: BTreeSet::new(),
        schema_modules: Vec::new(),
        tables: Vec::new(),
    };
    for schema in [&damage_table, &elemental_damage_table] {
        group.tables.push(EmittedTableModule {
            module_name: table_module_name(schema),
            marker_name: table_marker_name(schema),
            source_paths: vec![table_source_path(schema)],
            schema: schema.clone(),
            columns: Vec::new(),
        });
    }

    let rendered = render_table_code_type_family_mod(&group)
        .expect("render type family mod")
        .expect("multi-table family module");

    assert!(rendered.contains("pub enum DamageDataTable"));
    assert!(rendered.contains("pub struct DamageDataHandle"));
    assert!(rendered.contains("DamageTable"));
    assert!(!rendered.contains("pub enum DamageDataAfflictionCell<'a>"));
    assert!(!rendered.contains("gamedata::ForeignKey"));
    assert!(!rendered.contains("pub fn strings(self)"));
}
