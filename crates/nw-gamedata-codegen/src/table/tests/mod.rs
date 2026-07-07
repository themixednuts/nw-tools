use std::io;

use nw_asset::AssetId;
use nw_datasheet::{
    ColumnType,
    game_system::{
        GameSystemAsset, GameSystemCell, GameSystemColumn,
        GameSystemDataTables as GameSystemCatalog, GameSystemTable, OwnedCellValue,
    },
};

use crate::game_system_schema::{
    GameSystemColorShape, GameSystemColumnSchema, GameSystemColumnValueShape,
    GameSystemForeignKeyCandidate, GameSystemListAtomShape, GameSystemListElementShape,
    GameSystemListShape, GameSystemNumberShape, GameSystemRangeBounds, GameSystemTableSchema,
};
use crate::schema::{GameDataCompileMode, schema_report_for_mode};

use super::*;

mod native_mode;
mod projections;
mod rendering;
mod resource_transform;

fn render_test_table(schema: &GameSystemTableSchema) -> String {
    render_test_table_with_schemas(schema, &[])
}

fn render_test_schema(schema: &GameSystemTableSchema) -> String {
    let report = GameSystemCatalogSchemaReport {
        tables: vec![schema.clone()],
        type_affinities: Vec::new(),
        diagnostics: Vec::new(),
    };
    let table_code_columns = table_code_column_index(&report);
    render_table_schema_code_files(schema, &table_code_columns)
        .expect("render schema rs")
        .root_rs
}

fn render_test_table_with_schemas(
    schema: &GameSystemTableSchema,
    extra_schemas: &[GameSystemTableSchema],
) -> String {
    let mut tables = extra_schemas.to_vec();
    tables.push(schema.clone());
    let report = GameSystemCatalogSchemaReport {
        tables,
        type_affinities: Vec::new(),
        diagnostics: Vec::new(),
    };
    let table_code_columns = table_code_column_index(&report);
    render_table_code_rs(schema, &table_code_columns, &table_source_path(schema))
        .expect("render table rs")
}

fn compact_rust_source(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

fn rust_source_contains(source: &str, needle: &str) -> bool {
    compact_rust_source(source).contains(&compact_rust_source(needle))
}

fn render_test_type_mod(schemas: &[GameSystemTableSchema]) -> String {
    render_test_type_group(schemas, |group| {
        render_table_code_type_mod(group).expect("render type mod")
    })
}

fn render_test_family_mod(schemas: &[GameSystemTableSchema]) -> String {
    render_test_type_group(schemas, |group| {
        render_table_code_type_family_mod(group)
            .expect("render type family mod")
            .expect("multi-table family module")
    })
}

fn render_test_type_group(
    schemas: &[GameSystemTableSchema],
    render: impl FnOnce(&TableCodeTypeGroup) -> String,
) -> String {
    let first = schemas.first().expect("at least one schema");
    let mut group = TableCodeTypeGroup {
        row_type_name: first.row_type_name.clone(),
        row_type_crc: first.row_type_crc,
        used_table_modules: BTreeSet::new(),
        used_table_markers: BTreeSet::new(),
        schema_modules: Vec::new(),
        tables: Vec::new(),
    };
    for schema in schemas {
        group.tables.push(EmittedTableModule {
            module_name: table_module_name(schema),
            marker_name: table_marker_name(schema),
            source_paths: vec![table_source_path(schema)],
            schema: schema.clone(),
            columns: Vec::new(),
        });
    }
    render(&group)
}

fn table_source_path(schema: &GameSystemTableSchema) -> String {
    native_table_source_path_from_modules(&row_type_module_name(schema), &table_module_name(schema))
}
