use std::collections::BTreeMap;

use anyhow::{Context, Result};
use gamedata::release::SchemaHash;
use nw_datasheet::game_system::{GameSystemDataTables as GameSystemCatalog, GameSystemTable};
use sha2::{Digest, Sha256};

use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemDataTablesSchemaReport,
    GameSystemTableSchema,
};
use crate::naming::{to_module_ident, to_upper_camel_ident};

use super::GAMEDATA_TABLE_SOURCE_PREFIX;

pub(crate) type SchemaByTable<'a> = BTreeMap<(u32, u32), &'a GameSystemTableSchema>;

pub(crate) fn schema_by_table(
    schema_report: &GameSystemDataTablesSchemaReport,
) -> SchemaByTable<'_> {
    schema_report
        .tables
        .iter()
        .map(|schema| ((schema.table_name_crc, schema.row_type_crc), schema))
        .collect()
}

pub(crate) fn schema_for_table<'a>(
    schemas: &'a SchemaByTable<'a>,
    table: &GameSystemTable,
) -> Result<&'a GameSystemTableSchema> {
    schemas
        .get(&(table.name_crc(), table.type_crc()))
        .copied()
        .with_context(|| {
            format!(
                "missing inferred schema for table {} ({})",
                table.name(),
                table.type_name()
            )
        })
}

pub(crate) fn sorted_tables(catalog: &GameSystemCatalog) -> Vec<&GameSystemTable> {
    let mut tables = catalog.tables().iter().collect::<Vec<_>>();
    tables.sort_by(|left, right| {
        left.type_name()
            .cmp(right.type_name())
            .then(left.name().cmp(right.name()))
    });
    tables
}

pub(crate) fn row_type_module_name(schema: &GameSystemTableSchema) -> String {
    to_module_ident(&schema.row_type_name, "rowtype")
}

pub(crate) fn table_module_name(schema: &GameSystemTableSchema) -> String {
    to_module_ident(&schema.table_name, "table")
}

pub(crate) fn table_marker_name(schema: &GameSystemTableSchema) -> String {
    to_upper_camel_ident(&schema.table_name, "Table")
}

pub(crate) fn native_table_source_path_from_modules(
    type_module: &str,
    table_module: &str,
) -> String {
    format!("{GAMEDATA_TABLE_SOURCE_PREFIX}/{type_module}/{table_module}.ron")
}

pub(crate) fn schema_hash(schema: &GameSystemTableSchema) -> Result<SchemaHash> {
    let encoded = serde_json::to_vec(schema).context("serialize table schema for hash")?;
    let digest = Sha256::digest(encoded);
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    Ok(SchemaHash(u64::from_le_bytes(bytes)))
}

pub(crate) fn column_has_list(column: &GameSystemColumnSchema) -> bool {
    !column.row_key
        && matches!(
            &column.value_shape,
            GameSystemColumnValueShape::String { list: Some(_), .. }
        )
}
