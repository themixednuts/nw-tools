use std::collections::{BTreeMap, BTreeSet};

use crate::game_system_schema::{GameSystemColumnSchema, GameSystemTableSchema};

#[derive(Debug)]
pub(crate) struct TableCodeTypeGroup {
    pub(crate) row_type_name: String,
    pub(crate) row_type_crc: u32,
    pub(crate) used_table_modules: BTreeSet<String>,
    pub(crate) used_table_markers: BTreeSet<String>,
    pub(crate) schema_modules: Vec<EmittedSchemaModule>,
    pub(crate) tables: Vec<EmittedTableModule>,
}

#[derive(Debug)]
pub(crate) struct EmittedSchemaModule {
    pub(crate) module_name: String,
}

#[derive(Debug)]
pub(crate) struct EmittedTableModule {
    pub(crate) module_name: String,
    pub(crate) marker_name: String,
    pub(crate) source_paths: Vec<String>,
    pub(crate) schema: GameSystemTableSchema,
    pub(crate) columns: Vec<EmittedTableColumn>,
}

#[derive(Debug, Clone)]
pub(crate) struct EmittedTableColumn {
    pub(crate) schema: GameSystemColumnSchema,
    pub(crate) field: RustField,
    pub(crate) cell_type: gamedata::CellType,
}

#[derive(Debug, Clone)]
pub(crate) struct RustField {
    pub(crate) rust_name: String,
    pub(crate) rust_column_name: String,
    pub(crate) rust_column_marker: String,
    pub(crate) foreign_key_column: Option<RustForeignKeyColumn>,
    pub(crate) foreign_key_meta_columns: Vec<RustForeignKeyColumn>,
}

#[derive(Debug, Clone)]
pub(crate) struct RustForeignKeyColumn {
    pub(crate) rust_marker: String,
}

pub(crate) const SOURCE_INDEX_CHUNK_SIZE: usize = 16;

pub(super) type TableCodeColumnIndex = BTreeMap<(String, String), BTreeMap<String, bool>>;
