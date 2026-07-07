use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use gamedata::release::{GameDataDependency, GameDataRelease};
use nw_datasheet::game_system::{
    GameSystemDataTables as GameSystemCatalog, GameSystemValidationDiagnostic,
};

use crate::game_system_schema::GameSystemDataTablesSchemaReport as GameSystemCatalogSchemaReport;
use crate::schema::{GameDataCompileMode, schema_report_for_mode};
use crate::source::load_catalog_from_asset_root;

use super::audit::{RonTransformAuditReport, report_ron_transform_audits};
use super::identity::{schema_by_table, schema_for_table, schema_hash};
use super::release::{
    TableAssetEntry, TableAssetIndex, default_release, deterministic_release_id,
    table_index_catalog_hash, table_index_projection_hash, table_set_fingerprint,
};
use super::{
    GAMEDATA_RON_TRANSFORM_AUDIT_PATH, GAMEDATA_SCHEMA_REPORT_PATH,
    GAMEDATA_TRANSFORM_DIAGNOSTICS_PATH, write_native_table_source_outputs,
};

#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct GameDataTransformOutput {
    pub(super) table_index: TableAssetIndex,
    pub(super) release: GameDataRelease,
    pub(super) schema_report: GameSystemCatalogSchemaReport,
    pub(super) diagnostics: Vec<GameSystemValidationDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct GameDataCompileOutput {
    pub schema_report_path: PathBuf,
    pub diagnostics_path: PathBuf,
    pub table_code_root_path: PathBuf,
    pub table_source_root_path: PathBuf,
    pub table_source_format: GameDataTableSourceFormat,
    pub table_source_files: usize,
    pub table_code_files: usize,
    pub ron_transform_audit_path: Option<PathBuf>,
    pub ron_transform_audit_entries: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GameDataTableSourceFormat {
    AuthoredRon,
    Datasheet,
}

#[derive(Debug)]
pub(super) struct TableSourceOutput {
    pub(super) code_root_path: PathBuf,
    pub(super) source_root_path: PathBuf,
    pub(super) source_format: GameDataTableSourceFormat,
    pub(super) source_files: usize,
    pub(super) code_files: usize,
    pub(super) ron_transform_audits: RonTransformAuditReport,
}

#[derive(Debug, Clone, Copy)]
pub struct GameDataOutputRoots<'a> {
    pub table_code_root: &'a Path,
    pub source_index_code_root: Option<&'a Path>,
    pub table_source_root: &'a Path,
    pub diagnostics_root: &'a Path,
}

pub fn compile_from_asset_root(
    asset_root: &Path,
    output_roots: GameDataOutputRoots<'_>,
) -> Result<GameDataCompileOutput> {
    compile_from_asset_root_with_mode(asset_root, output_roots, GameDataCompileMode::SourceFormat)
}

pub fn compile_from_asset_root_with_mode(
    asset_root: &Path,
    output_roots: GameDataOutputRoots<'_>,
    mode: GameDataCompileMode,
) -> Result<GameDataCompileOutput> {
    compile_from_asset_root_with_mode_and_source_format(
        asset_root,
        output_roots,
        mode,
        GameDataTableSourceFormat::default_for_mode(mode),
    )
}

pub fn compile_from_asset_root_with_mode_and_source_format(
    asset_root: &Path,
    output_roots: GameDataOutputRoots<'_>,
    mode: GameDataCompileMode,
    source_format: GameDataTableSourceFormat,
) -> Result<GameDataCompileOutput> {
    let catalog = load_catalog_from_asset_root(asset_root)?;
    compile_catalog_with_mode_and_source_format(&catalog, output_roots, mode, source_format)
}

pub fn compile_catalog(
    catalog: &GameSystemCatalog,
    output_roots: GameDataOutputRoots<'_>,
) -> Result<GameDataCompileOutput> {
    compile_catalog_with_mode(catalog, output_roots, GameDataCompileMode::SourceFormat)
}

pub fn compile_catalog_with_mode(
    catalog: &GameSystemCatalog,
    output_roots: GameDataOutputRoots<'_>,
    mode: GameDataCompileMode,
) -> Result<GameDataCompileOutput> {
    compile_catalog_with_mode_and_source_format(
        catalog,
        output_roots,
        mode,
        GameDataTableSourceFormat::default_for_mode(mode),
    )
}

pub fn compile_catalog_with_mode_and_source_format(
    catalog: &GameSystemCatalog,
    output_roots: GameDataOutputRoots<'_>,
    mode: GameDataCompileMode,
    source_format: GameDataTableSourceFormat,
) -> Result<GameDataCompileOutput> {
    let transform = transform_catalog(catalog, default_release(), mode)?;
    write_transform_output(&output_roots, catalog, &transform, source_format)
}

pub fn compile_summary(paths: &GameDataCompileOutput) -> String {
    match paths.table_source_format {
        GameDataTableSourceFormat::AuthoredRon => format!(
            "wrote {} RON table sources under {} and {} schema descriptor files under {}; wrote schema report to {}, transform diagnostics to {}, and {} RON transform audit entries to {}",
            paths.table_source_files,
            paths.table_source_root_path.display(),
            paths.table_code_files,
            paths.table_code_root_path.display(),
            paths.schema_report_path.display(),
            paths.diagnostics_path.display(),
            paths.ron_transform_audit_entries,
            paths
                .ron_transform_audit_path
                .as_ref()
                .expect("authored RON output writes a RON transform audit")
                .display()
        ),
        GameDataTableSourceFormat::Datasheet => format!(
            "indexed {} datasheet source paths rooted at {} and wrote {} schema descriptor files under {}; wrote schema report to {} and transform diagnostics to {}",
            paths.table_source_files,
            paths.table_source_root_path.display(),
            paths.table_code_files,
            paths.table_code_root_path.display(),
            paths.schema_report_path.display(),
            paths.diagnostics_path.display()
        ),
    }
}

impl GameDataTableSourceFormat {
    #[must_use]
    pub const fn default_for_mode(mode: GameDataCompileMode) -> Self {
        match mode {
            GameDataCompileMode::Strict => Self::AuthoredRon,
            GameDataCompileMode::SourceFormat => Self::Datasheet,
        }
    }
}

pub(super) fn transform_catalog(
    catalog: &GameSystemCatalog,
    mut release: GameDataRelease,
    mode: GameDataCompileMode,
) -> Result<GameDataTransformOutput> {
    let schema_report = schema_report_for_mode(catalog, mode);
    let diagnostics = schema_report.diagnostics.clone();
    release.table_set_hash = table_set_fingerprint(&schema_report)?;
    let table_index = table_index_from_catalog(catalog, &schema_report)?;
    release.asset_catalog_hash = table_index_catalog_hash(&table_index)?;
    release.projection_hash = table_index_projection_hash(&table_index)?;
    release.id = deterministic_release_id(&release, &table_index)?;

    Ok(GameDataTransformOutput {
        table_index,
        release,
        schema_report,
        diagnostics,
    })
}

fn table_index_from_catalog(
    catalog: &GameSystemCatalog,
    schema_report: &GameSystemCatalogSchemaReport,
) -> Result<TableAssetIndex> {
    let schema_by_table = schema_by_table(schema_report);
    let mut tables = Vec::new();
    for table in catalog.tables() {
        let schema = schema_for_table(&schema_by_table, table)?;
        let Some(asset_id) = table.source_asset_id() else {
            continue;
        };
        tables.push(TableAssetEntry {
            logical_name: table.name().to_owned(),
            source_path: table
                .source()
                .with_context(|| {
                    format!(
                        "table {} ({}) is missing a catalog source path",
                        table.name(),
                        table.type_name()
                    )
                })
                .map(|path| az_asset_builder::normalize_source_path(&path.to_string_lossy()))?,
            asset_id,
            schema_hash: schema_hash(schema)?,
            row_count: u32::try_from(table.len()).context("table row count exceeds u32")?,
        });
    }

    let dependencies = table_dependencies(catalog, schema_report)?;

    Ok(TableAssetIndex {
        tables,
        dependencies,
    })
}

fn table_dependencies(
    catalog: &GameSystemCatalog,
    schema_report: &GameSystemCatalogSchemaReport,
) -> Result<Vec<GameDataDependency>> {
    crate::dependencies::infer_table_dependencies(catalog, schema_report)
        .context("infer dependency edges from datasheet schema")
}

pub(super) fn write_transform_output(
    output_roots: &GameDataOutputRoots<'_>,
    catalog: &GameSystemCatalog,
    transform: &GameDataTransformOutput,
    source_format: GameDataTableSourceFormat,
) -> Result<GameDataCompileOutput> {
    let schema_report_path = output_roots
        .diagnostics_root
        .join(GAMEDATA_SCHEMA_REPORT_PATH);
    if let Some(parent) = schema_report_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(
        &schema_report_path,
        serde_json::to_vec(&transform.schema_report)
            .context("serialize game-data schema report")?,
    )
    .with_context(|| format!("write {}", schema_report_path.display()))?;

    let diagnostics_path = output_roots
        .diagnostics_root
        .join(GAMEDATA_TRANSFORM_DIAGNOSTICS_PATH);
    if let Some(parent) = diagnostics_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(
        &diagnostics_path,
        serde_json::to_vec(&transform.diagnostics)
            .context("serialize game-data transform diagnostics")?,
    )
    .with_context(|| format!("write {}", diagnostics_path.display()))?;

    let table_output = write_native_table_source_outputs(
        output_roots.table_code_root,
        output_roots.source_index_code_root,
        output_roots.table_source_root,
        catalog,
        &transform.schema_report,
        source_format,
    )?;

    let ron_transform_audit_path = if table_output.source_format
        == GameDataTableSourceFormat::AuthoredRon
    {
        let path = output_roots
            .diagnostics_root
            .join(GAMEDATA_RON_TRANSFORM_AUDIT_PATH);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(
            &path,
            serde_json::to_vec(&table_output.ron_transform_audits)
                .context("serialize RON transform audit")?,
        )
        .with_context(|| format!("write {}", path.display()))?;
        report_ron_transform_audits(&path, &table_output.ron_transform_audits);
        Some(path)
    } else {
        None
    };

    Ok(GameDataCompileOutput {
        schema_report_path,
        diagnostics_path,
        table_code_root_path: table_output.code_root_path,
        table_source_root_path: table_output.source_root_path,
        table_source_format: table_output.source_format,
        table_source_files: table_output.source_files,
        table_code_files: table_output.code_files,
        ron_transform_audit_path,
        ron_transform_audit_entries: table_output.ron_transform_audits.total_count(),
    })
}
