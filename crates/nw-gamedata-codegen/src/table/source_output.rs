use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use nw_datasheet::game_system::{
    GameSystemAsset, GameSystemCell, GameSystemDataTables as GameSystemCatalog, GameSystemTable,
};

use super::audit::{RonTransformAuditReport, ron_transform_audit};
use super::compile::{GameDataTableSourceFormat, TableSourceOutput};
use super::identity::{
    native_table_source_path_from_modules, row_type_module_name, schema_by_table, schema_for_table,
    sorted_tables, table_marker_name, table_module_name,
};
use super::model::{EmittedTableColumn, EmittedTableModule, TableCodeTypeGroup};
use super::native_value::{
    NativeDevCellValue, native_dev_cell_value, render_native_dev_cell_value,
};
use super::{
    GAMEDATA_TABLE_SOURCE_PREFIX, clear_codegen_output_dir, emitted_table_columns,
    rust_fields_for_schema, table_code_column_index,
};
use crate::game_system_schema::{
    GameSystemDataTablesSchemaReport as GameSystemCatalogSchemaReport, GameSystemTableSchema,
};
use crate::schema::{GameDataCompileMode, schema_report_for_mode};
use crate::source_index::{render_source_index_chunks, render_source_index_mod};
use nw_generated_guard::GeneratedRootWriter;
use rayon::prelude::*;

type NativeDevTableFile = Vec<NativeDevRow>;
type NativeDevRow = Vec<NativeDevField>;

#[derive(Debug)]
pub(super) struct NativeDevTableFileOutput {
    pub(super) rows: NativeDevTableFile,
    pub(super) audits: RonTransformAuditReport,
}

/// In-memory editable GameData RON emission for a whole catalog.
#[derive(Debug, Clone)]
pub struct EditableGameDataRonEmission {
    pub schema_report: GameSystemCatalogSchemaReport,
    pub files: Vec<EditableGameDataRonFile>,
    pub audits: RonTransformAuditReport,
}

/// One editable GameData RON source file.
#[derive(Debug, Clone)]
pub struct EditableGameDataRonFile {
    pub source_path: String,
    pub table_name: String,
    pub row_type_name: String,
    pub datasheet_sources: Vec<GameSystemAsset>,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct NativeDevField {
    name: String,
    value: NativeDevCellValue,
}

#[derive(Debug)]
struct PlannedTableOutput {
    type_module: String,
    table_module: String,
    marker_name: String,
    schema: GameSystemTableSchema,
    columns: Vec<EmittedTableColumn>,
    source_paths: Vec<String>,
    game_source_path: Option<String>,
    ron: Option<PlannedRonOutput>,
}

#[derive(Debug)]
struct PlannedRonOutput {
    path: PathBuf,
    text: String,
    audits: RonTransformAuditReport,
}

/// Emit editable RON table source files in memory for every table in `catalog`.
pub fn emit_editable_gamedata_ron_sources(
    catalog: &GameSystemCatalog,
    mode: GameDataCompileMode,
) -> Result<EditableGameDataRonEmission> {
    let schema_report = schema_report_for_mode(catalog, mode);
    let table_schemas = schema_by_table(&schema_report);

    let rendered = sorted_tables(catalog)
        .into_par_iter()
        .map(|table| -> Result<_> {
            let schema = schema_for_table(&table_schemas, table)?;
            let type_module = row_type_module_name(schema);
            let table_module = table_module_name(schema);
            let source_path = native_table_source_path_from_modules(&type_module, &table_module);
            let ron_file = native_dev_table_file(table, schema, &source_path)?;
            let ron_text = render_native_dev_table_ron(&ron_file.rows)?;
            Ok((
                EditableGameDataRonFile {
                    source_path,
                    table_name: schema.table_name.clone(),
                    row_type_name: schema.row_type_name.clone(),
                    datasheet_sources: table.sources().to_vec(),
                    bytes: ron_text.into_bytes(),
                },
                ron_file.audits,
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut files = Vec::with_capacity(rendered.len());
    let mut audits = RonTransformAuditReport::default();
    for (file, table_audits) in rendered {
        files.push(file);
        audits.extend(table_audits);
    }

    Ok(EditableGameDataRonEmission {
        schema_report,
        files,
        audits,
    })
}

pub(super) fn write_native_table_source_outputs(
    code_root: &Path,
    source_index_code_root: Option<&Path>,
    source_root: &Path,
    catalog: &GameSystemCatalog,
    schema_report: &GameSystemCatalogSchemaReport,
    source_format: GameDataTableSourceFormat,
) -> Result<TableSourceOutput> {
    let source_root_path = source_root.to_path_buf();
    fs::create_dir_all(&source_root_path)
        .with_context(|| format!("create {}", source_root_path.display()))?;
    let authored_ron_root = match source_format {
        GameDataTableSourceFormat::AuthoredRon => {
            let table_source_root = source_root_path.join(GAMEDATA_TABLE_SOURCE_PREFIX);
            fs::create_dir_all(&table_source_root)
                .with_context(|| format!("create {}", table_source_root.display()))?;
            clear_codegen_output_dir(&table_source_root, "RON table source")?;
            Some(table_source_root)
        }
        GameDataTableSourceFormat::Datasheet => None,
    };
    let mut ron_writer = authored_ron_root.as_ref().map(GeneratedRootWriter::new);

    let schema_by_table = schema_by_table(schema_report);
    let table_code_columns = table_code_column_index(schema_report);
    let mut groups: BTreeMap<String, TableCodeTypeGroup> = BTreeMap::new();
    let mut authored_source_files = 0usize;
    let mut game_source_paths = BTreeSet::new();
    let mut descriptor_files = 0usize;
    let mut ron_transform_audits = RonTransformAuditReport::default();

    let planned_tables = sorted_tables(catalog)
        .into_par_iter()
        .map(|table| -> Result<PlannedTableOutput> {
            let schema = schema_for_table(&schema_by_table, table)?;
            let type_module = row_type_module_name(schema);
            let table_module = table_module_name(schema);
            let marker_name = table_marker_name(schema);

            let (source_path, ron_path, game_source_path) =
                match (&authored_ron_root, source_format) {
                    (Some(table_source_root), GameDataTableSourceFormat::AuthoredRon) => (
                        native_table_source_path_from_modules(&type_module, &table_module),
                        Some(
                            table_source_root
                                .join(&type_module)
                                .join(format!("{table_module}.ron")),
                        ),
                        None,
                    ),
                    (None, GameDataTableSourceFormat::Datasheet) => {
                        let source_path = datasheet_source_path(table)?;
                        (source_path.clone(), None, Some(source_path))
                    }
                    _ => unreachable!("source format and root are planned together"),
                };

            let columns = emitted_table_columns(schema, &table_code_columns);
            let ron = ron_path
                .map(|path| -> Result<PlannedRonOutput> {
                    let ron_file = native_dev_table_file(table, schema, &source_path)?;
                    let text = render_native_dev_table_ron(&ron_file.rows)?;
                    Ok(PlannedRonOutput {
                        path,
                        text,
                        audits: ron_file.audits,
                    })
                })
                .transpose()?;
            let source_paths = table_source_paths_for_format(
                schema,
                source_format,
                std::slice::from_ref(&source_path),
            )?;

            Ok(PlannedTableOutput {
                type_module,
                table_module,
                marker_name,
                schema: schema.clone(),
                columns,
                source_paths,
                game_source_path,
                ron,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    for planned in planned_tables {
        let PlannedTableOutput {
            type_module,
            table_module,
            marker_name,
            schema,
            columns,
            source_paths,
            game_source_path,
            ron,
        } = planned;

        let group = groups
            .entry(type_module.clone())
            .or_insert_with(|| TableCodeTypeGroup {
                row_type_name: schema.row_type_name.clone(),
                row_type_crc: schema.row_type_crc,
                used_table_modules: BTreeSet::new(),
                used_table_markers: BTreeSet::new(),
                schema_modules: Vec::new(),
                tables: Vec::new(),
            });
        if group.row_type_crc != schema.row_type_crc || group.row_type_name != schema.row_type_name
        {
            bail!(
                "table row-type module collision for {type_module}: {} ({}) vs {} ({})",
                group.row_type_name,
                group.row_type_crc,
                schema.row_type_name,
                schema.row_type_crc
            );
        }
        if !group.used_table_modules.insert(table_module.clone()) {
            bail!(
                "table module collision for {type_module}/{table_module} from {}",
                schema.table_name
            );
        }
        if !group.used_table_markers.insert(marker_name.clone()) {
            bail!(
                "table marker collision for {type_module}::{marker_name} from {}",
                schema.table_name
            );
        }

        if let Some(game_source_path) = game_source_path {
            game_source_paths.insert(game_source_path);
        }
        if let Some(ron) = ron {
            ron_writer
                .as_mut()
                .expect("authored RON root exists when RON paths are planned")
                .write(&ron.path, ron.text.as_bytes())
                .with_context(|| format!("write {}", ron.path.display()))?;
            authored_source_files += 1;
            ron_transform_audits.extend(ron.audits);
        }

        let group = groups
            .get_mut(&type_module)
            .expect("rendered table type module is planned");
        group.tables.push(EmittedTableModule {
            module_name: table_module,
            marker_name,
            source_paths,
            schema,
            columns,
        });
    }

    let source_index_path = match source_index_code_root {
        Some(source_index_code_root) => source_index_code_root.to_path_buf(),
        None => code_root.to_path_buf(),
    };
    fs::create_dir_all(&source_index_path)
        .with_context(|| format!("create {}", source_index_path.display()))?;
    clear_codegen_output_dir(&source_index_path, "GameData descriptor catalog")?;
    let mut index_writer = GeneratedRootWriter::new(&source_index_path);
    let table_index_mod_path = source_index_path.join("mod.rs");
    let table_index_mod_rs = render_source_index_mod(&groups)?;
    index_writer
        .write(&table_index_mod_path, table_index_mod_rs.as_bytes())
        .with_context(|| format!("write {}", table_index_mod_path.display()))?;
    descriptor_files += 1;

    for (chunk_name, chunk_rs) in render_source_index_chunks(&groups)? {
        let chunk_path = source_index_path.join(format!("{chunk_name}.rs"));
        index_writer
            .write(&chunk_path, chunk_rs.as_bytes())
            .with_context(|| format!("write {}", chunk_path.display()))?;
        descriptor_files += 1;
    }
    index_writer
        .finish()
        .context("write GameData descriptor catalog generated manifest")?;
    if let Some(ron_writer) = ron_writer {
        ron_writer
            .finish()
            .context("write RON table-source generated manifest")?;
    }

    let source_files = match source_format {
        GameDataTableSourceFormat::AuthoredRon => authored_source_files,
        GameDataTableSourceFormat::Datasheet => game_source_paths.len(),
    };

    Ok(TableSourceOutput {
        code_root_path: source_index_path,
        source_root_path,
        source_format,
        source_files,
        code_files: descriptor_files,
        ron_transform_audits,
    })
}

fn table_source_paths_for_format(
    schema: &GameSystemTableSchema,
    source_format: GameDataTableSourceFormat,
    authored_paths: &[String],
) -> Result<Vec<String>> {
    let source_paths = match source_format {
        GameDataTableSourceFormat::AuthoredRon => authored_paths.to_vec(),
        GameDataTableSourceFormat::Datasheet => schema.sources.clone(),
    };
    if source_paths.is_empty() {
        bail!("table `{}` has no source path", schema.table_name);
    }
    Ok(source_paths)
}

fn datasheet_source_path(table: &GameSystemTable) -> Result<String> {
    table
        .source()
        .map(|path| az_asset_builder::normalize_source_path(&path.to_string_lossy()))
        .with_context(|| {
            format!(
                "table {} ({}) is missing a datasheet source path",
                table.name(),
                table.type_name()
            )
        })
}

pub(super) fn native_dev_table_file(
    table: &GameSystemTable,
    schema: &GameSystemTableSchema,
    ron_path: &str,
) -> Result<NativeDevTableFileOutput> {
    let rust_fields = rust_fields_for_schema(schema);

    let mut rows = Vec::with_capacity(table.len());
    let mut audits = RonTransformAuditReport::default();
    let source_path = table
        .source()
        .map(|path| az_asset_builder::normalize_source_path(&path.to_string_lossy()));
    for (row_index, row) in table.row_refs().enumerate() {
        let row_key = row_key_source_value(schema, row.cells());
        let mut authored_row = Vec::new();
        for ((column, rust_field), cell) in schema
            .columns
            .iter()
            .zip(rust_fields.iter())
            .zip(row.cells())
        {
            let Some(value) =
                native_dev_cell_value(schema, column, cell.value()).with_context(|| {
                    format!(
                        "emit {} ({}) row {row_index} column {} value {}",
                        table.name(),
                        table.type_name(),
                        column.name,
                        cell.value()
                    )
                })?
            else {
                continue;
            };
            if let Some(audit) = ron_transform_audit(
                table,
                schema,
                ron_path,
                source_path.as_deref(),
                row_index,
                row_key.as_deref(),
                column,
                rust_field,
                cell.value(),
                &value,
            )? {
                audits.record(audit);
            }
            authored_row.push(NativeDevField {
                name: rust_field.rust_name.clone(),
                value,
            });
        }
        rows.push(authored_row);
    }

    Ok(NativeDevTableFileOutput { rows, audits })
}

pub(super) fn row_key_source_value(
    schema: &GameSystemTableSchema,
    cells: &[GameSystemCell],
) -> Option<String> {
    schema
        .columns
        .iter()
        .zip(cells.iter())
        .find_map(|(column, cell)| column.row_key.then(|| cell.value().to_string()))
}

fn render_native_dev_table_ron(table: &NativeDevTableFile) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "[")?;
    for row in table {
        if row.is_empty() {
            writeln!(out, "    (),")?;
            continue;
        }
        writeln!(out, "    (")?;
        for field in row {
            writeln!(
                out,
                "        {}: {},",
                field.name,
                render_native_dev_cell_value(&field.value)?
            )?;
        }
        writeln!(out, "    ),")?;
    }
    writeln!(out, "]")?;
    Ok(out)
}
