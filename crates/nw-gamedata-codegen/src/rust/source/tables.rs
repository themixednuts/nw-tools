use std::collections::BTreeSet;
use std::io::Write;

use anyhow::Result;
use flate2::Compression;
use flate2::write::GzEncoder;

use crate::compiler::GameDataCompileUnit;
use crate::emit::GameDataCodegenFile;
use crate::game_system_schema::GameSystemTableSchema;
use crate::manager_records::{ManagerSurface, manager_surfaces};
use crate::rust::source::format_rust_source;

pub(super) fn emit_schema_files(unit: &GameDataCompileUnit) -> Result<Vec<GameDataCodegenFile>> {
    Ok(vec![
        GameDataCodegenFile::new(
            "src/datasheet_catalog.rs",
            datasheet_catalog_module_source()?,
        ),
        GameDataCodegenFile::binary(
            "src/datasheet_catalog.json.gz",
            compressed_datasheet_catalog_json(unit)?,
        ),
    ])
}

fn datasheet_catalog_module_source() -> Result<String> {
    let source = r#"
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ColumnDescriptor {
    pub name: String,
    pub crc: u32,
    pub row_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TableDescriptor {
    pub name: String,
    pub row_type: String,
    pub sources: Vec<String>,
    pub columns: Vec<ColumnDescriptor>,
}

pub static TABLES: LazyLock<Vec<TableDescriptor>> = LazyLock::new(|| {
    let mut decoder = GzDecoder::new(include_bytes!("datasheet_catalog.json.gz").as_slice());
    let mut json = String::new();
    std::io::Read::read_to_string(&mut decoder, &mut json)
        .expect("generated datasheet_catalog.json.gz is readable");
    serde_json::from_str(&json).expect("generated datasheet_catalog.json.gz contains valid JSON")
});
"#;
    format_rust_source(source).map_err(Into::into)
}

pub(crate) fn compressed_datasheet_catalog_json(unit: &GameDataCompileUnit) -> Result<Vec<u8>> {
    let json = datasheet_catalog_json_source(unit)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(json.as_bytes())?;
    Ok(encoder.finish()?)
}

fn datasheet_catalog_json_source(unit: &GameDataCompileUnit) -> Result<String> {
    let report = unit.schema_report();
    let required = RequiredSchemas::from_unit(unit)?;
    let tables = report
        .tables
        .iter()
        .filter(|table| required.includes(table))
        .map(|table| {
            serde_json::json!({
                "name": table.table_name,
                "row_type": table.row_type_name,
                "sources": table.sources,
                "columns": table
                    .columns
                    .iter()
                    .map(|column| serde_json::json!({
                        "name": column.name,
                        "crc": column.crc,
                        "row_key": column.row_key,
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
}

impl RequiredSchemas {
    fn from_unit(unit: &GameDataCompileUnit) -> Result<Self> {
        let mut required = Self::default();
        for surface in manager_surfaces(unit)? {
            match surface {
                ManagerSurface::Direct(manager)
                | ManagerSurface::Native { manager, .. }
                | ManagerSurface::ProductBacked(manager) => {
                    required.table_rows.extend(
                        manager
                            .tables
                            .into_iter()
                            .map(|table| (table.table_name, table.row_type_name)),
                    );
                }
                ManagerSurface::Semantic(record) => {
                    required.table_rows.extend(
                        record
                            .tables
                            .into_iter()
                            .map(|table| (table.table_name, table.row_type_name)),
                    );
                }
                ManagerSurface::ItemData(manager) => {
                    required.table_rows.extend(
                        manager
                            .tables
                            .into_iter()
                            .map(|table| (table.table_name, table.row_type_name)),
                    );
                }
                ManagerSurface::Composition(_) => {}
            }
        }
        Ok(required)
    }

    fn includes(&self, table: &GameSystemTableSchema) -> bool {
        self.table_rows
            .contains(&(table.table_name.clone(), table.row_type_name.clone()))
    }
}
