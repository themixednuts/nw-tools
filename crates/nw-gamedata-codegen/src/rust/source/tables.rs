use std::collections::BTreeSet;
use std::io::Write;

use anyhow::Result;
use flate2::Compression;
use flate2::write::GzEncoder;
use nw_datasheet::ColumnType;

use crate::compiler::GameDataCompileUnit;
use crate::emit::GameDataCodegenFile;
use crate::game_system_schema::GameSystemTableSchema;
use crate::manager_records::{ManagerSurface, manager_surfaces};
use crate::naming::to_snake_ident;
use crate::rust::source::format_rust_source;

pub(super) fn emit_schema_files(unit: &GameDataCompileUnit) -> Result<Vec<GameDataCodegenFile>> {
    Ok(vec![
        GameDataCodegenFile::new("src/table_manifest.rs", table_manifest_module_source()?),
        GameDataCodegenFile::binary(
            "src/table_manifest.json.gz",
            compressed_table_manifest_json(unit)?,
        ),
    ])
}

fn table_manifest_module_source() -> Result<String> {
    let source = r#"
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasheetCellKind {
    String,
    Number,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ColumnDescriptor {
    pub name: String,
    pub field_name: String,
    pub crc: u32,
    pub kind: DatasheetCellKind,
    pub row_key: bool,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TableDescriptor {
    pub name: String,
    pub name_crc: u32,
    pub row_type: String,
    pub row_type_crc: u32,
    pub row_count: usize,
    pub sources: Vec<String>,
    pub columns: Vec<ColumnDescriptor>,
}

pub static TABLES: LazyLock<Vec<TableDescriptor>> = LazyLock::new(|| {
    let mut decoder = GzDecoder::new(include_bytes!("table_manifest.json.gz").as_slice());
    let mut json = String::new();
    std::io::Read::read_to_string(&mut decoder, &mut json)
        .expect("generated table_manifest.json.gz is readable");
    serde_json::from_str(&json).expect("generated table_manifest.json.gz contains valid JSON")
});
"#;
    format_rust_source(source).map_err(Into::into)
}

fn compressed_table_manifest_json(unit: &GameDataCompileUnit) -> Result<Vec<u8>> {
    let json = table_manifest_json_source(unit)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(json.as_bytes())?;
    Ok(encoder.finish()?)
}

fn table_manifest_json_source(unit: &GameDataCompileUnit) -> Result<String> {
    let report = unit.schema_report();
    let required = RequiredSchemas::from_unit(unit)?;
    let tables = report
        .tables
        .iter()
        .filter(|table| required.includes(table))
        .map(|table| {
            serde_json::json!({
                "name": table.table_name,
                "name_crc": table.table_name_crc,
                "row_type": table.row_type_name,
                "row_type_crc": table.row_type_crc,
                "row_count": table.row_count,
                "sources": table.sources,
                "columns": table
                    .columns
                    .iter()
                    .map(|column| serde_json::json!({
                        "name": column.name,
                        "field_name": to_snake_ident(&column.name, "column"),
                        "crc": column.crc,
                        "kind": cell_kind_name(column.declared_type),
                        "row_key": column.row_key,
                        "required": column.required,
                    }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let mut source = serde_json::to_string_pretty(&tables)?;
    source.push('\n');
    Ok(source)
}

#[derive(Debug, Default)]
struct RequiredSchemas {
    table_rows: BTreeSet<(String, String)>,
    table_names: BTreeSet<String>,
}

impl RequiredSchemas {
    fn from_unit(unit: &GameDataCompileUnit) -> Result<Self> {
        let mut required = Self::default();
        for surface in manager_surfaces(unit)? {
            match surface {
                ManagerSurface::Direct(manager) => {
                    required.table_rows.extend(
                        manager
                            .tables
                            .into_iter()
                            .map(|table| (table.table_name, table.row_type_name)),
                    );
                }
                ManagerSurface::Semantic(record) => {
                    required
                        .table_names
                        .extend(record.tables.into_iter().map(|table| table.table_name));
                }
            }
        }
        Ok(required)
    }

    fn includes(&self, table: &GameSystemTableSchema) -> bool {
        self.table_names.contains(&table.table_name)
            || self
                .table_rows
                .contains(&(table.table_name.clone(), table.row_type_name.clone()))
    }
}

fn cell_kind_name(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::String => "string",
        ColumnType::Number => "number",
        ColumnType::Boolean => "boolean",
    }
}
