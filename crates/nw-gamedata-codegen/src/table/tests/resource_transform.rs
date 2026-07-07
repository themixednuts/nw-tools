use super::*;
use crate::source::{FilesystemDatasheetGameDataSource, GameDataSourceProvider};

#[test]
fn transforms_all_resource_datasheets_to_stable_native_outputs() {
    let root = resource_datasheet_root();
    if !root.exists() {
        return;
    }

    let catalog = FilesystemDatasheetGameDataSource::new(&root)
        .load_catalog()
        .expect("load resource datasheet fixture");
    assert!(
        catalog.tables().len() > 1_000,
        "expected a broad datasheet corpus under {}",
        root.display()
    );
    let schema_report = schema_report_for_mode(&catalog, GameDataCompileMode::Strict);
    let transform = GameDataTransformOutput {
        table_index: TableAssetIndex {
            tables: Vec::new(),
            dependencies: Vec::new(),
        },
        release: default_release(),
        diagnostics: schema_report.diagnostics.clone(),
        schema_report,
    };
    assert!(
        transform.schema_report.tables.len() > 1_000,
        "expected schema inference for every loaded table"
    );
    let achievement_schema = transform
        .schema_report
        .tables
        .iter()
        .find(|table| table.table_name == "AchievementDataTable")
        .expect("achievement schema");
    assert!(
        achievement_schema
            .columns
            .iter()
            .any(|column| column.name == "AchievementID" && column.row_key)
    );
    assert!(
        transform.schema_report.tables.iter().any(|table| {
            table.columns.iter().any(|column| {
                matches!(
                    &column.value_shape,
                    GameSystemColumnValueShape::String { foreign_keys, .. }
                        if !foreign_keys.is_empty()
                )
            })
        }),
        "expected at least one inferred foreign-key candidate across the full datasheet corpus"
    );
    let temp = tempfile::tempdir().expect("temp output");
    let table_code_root = temp.path().join("table_code");
    let table_source_root = temp.path().to_path_buf();
    let diagnostics_root = temp.path().join("diagnostics");
    let output_roots = GameDataOutputRoots {
        table_code_root: &table_code_root,
        source_index_code_root: None,
        table_source_root: &table_source_root,
        diagnostics_root: &diagnostics_root,
    };
    let paths = write_transform_output(
        &output_roots,
        &catalog,
        &transform,
        GameDataTableSourceFormat::AuthoredRon,
    )
    .expect("write transform output");
    let schema_report_json =
        fs::read_to_string(paths.schema_report_path).expect("read schema report json");
    let diagnostics_json =
        fs::read_to_string(paths.diagnostics_path).expect("read diagnostics json");
    let ron_transform_audit_json = fs::read_to_string(
        paths
            .ron_transform_audit_path
            .as_ref()
            .expect("strict RON output should write RON audit json"),
    )
    .expect("read RON audit json");
    assert!(schema_report_json.contains("\"AchievementDataTable\""));
    assert!(schema_report_json.contains("\"foreign_keys\""));
    assert!(diagnostics_json.trim_start().starts_with('['));
    assert!(ron_transform_audit_json.trim_start().starts_with('{'));
    assert_eq!(
        paths.table_source_files,
        transform.schema_report.tables.len()
    );
    assert!(paths.table_code_files > 1);

    if let (Ok(code_dest), Ok(source_dest)) = (
        std::env::var("GAMEDATA_TABLE_CODE_SYNC"),
        std::env::var("GAMEDATA_TABLE_SOURCE_SYNC"),
    ) {
        sync_table_source_output(&paths.table_code_root_path, Path::new(&code_dest))
            .expect("sync gamedata table code output");
        sync_table_source_output(&paths.table_source_root_path, Path::new(&source_dest))
            .expect("sync gamedata table source output");
    }

    let achievement_ron_path = paths
        .table_source_root_path
        .join(GAMEDATA_TABLE_SOURCE_PREFIX)
        .join("achievement_data")
        .join("achievement_data_table.ron");
    let table_index_mod_path = paths.table_code_root_path.join("mod.rs");
    let table_index_mod = fs::read_to_string(table_index_mod_path).expect("read source index mod");
    let mut table_index_chunk_paths = fs::read_dir(&paths.table_code_root_path)
        .expect("read descriptor catalog dir")
        .map(|entry| entry.expect("source index entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("chunk_") && name.ends_with(".rs"))
        })
        .collect::<Vec<_>>();
    table_index_chunk_paths.sort();
    let table_index_chunks = table_index_chunk_paths
        .iter()
        .map(|path| fs::read_to_string(path).expect("read source index chunk"))
        .collect::<Vec<_>>()
        .join("\n");
    let achievement_ron = fs::read_to_string(achievement_ron_path).expect("read achievement RON");
    assert!(table_index_mod.contains("pub(crate) struct TableSourceEntry"));
    assert!(table_index_mod.contains("product_path: &'static str"));
    assert!(table_index_mod.contains("pub(crate) fn tables"));
    assert!(table_index_mod.contains("pub const fn table_count() -> usize"));
    assert!(table_index_mod.contains("pub fn table_source_paths"));
    assert!(table_index_mod.contains("pub fn table_product_paths"));
    assert!(table_index_mod.contains("pub fn table_product_path"));
    assert!(table_index_mod.contains("pub fn table_for_source_path"));
    assert!(table_index_mod.contains("pub fn foreign_key_source_path"));
    assert!(!table_index_mod.contains("product_asset_id"));
    assert!(!table_index_mod.contains("nw_asset::AssetId"));
    assert!(!table_index_mod.contains("SourceColumnEntry"));
    assert!(
        table_index_chunks.contains("\"gamedata/achievement_data/achievement_data_table.ron\"")
    );
    assert!(
        table_index_chunks.contains("\"tables/achievement_data/achievement_data_table.aztbl\"")
    );
    assert!(table_index_chunks.contains("TableSourceEntry::new("));
    assert!(table_index_chunks.contains("gamedata::TableSchemaDescriptor::new("));
    assert!(table_index_chunks.contains("\"AchievementDataTable\""));
    assert!(table_index_chunks.contains("\"AchievementData\""));
    assert!(table_index_chunks.contains("gamedata::TableSourceRoute::new(\"gamedata\""));
    assert!(table_index_chunks.contains("gamedata::TableProductRoute::new(\"tables\""));
    assert!(!table_index_chunks.contains("SourceColumnEntry"));
    assert!(!table_index_chunks.contains("pub(crate) mod achievement_data_t"));
    assert!(!table_index_chunks.contains("pub(crate) struct Row"));
    assert!(!table_index_chunks.contains("pub(crate) struct Table"));
    assert!(
        !table_index_chunks
            .contains("&super::super::achievement_data::achievement_data_table::TABLE")
    );
    assert!(!table_index_mod.contains("table_crc: u32"));
    assert!(!table_index_mod.contains("row_crc: u32"));
    assert!(!table_index_mod.contains("TABLE_SOURCE_PATHS"));
    assert!(!table_index_mod.contains("match source_path"));
    assert!(!table_index_mod.contains("ALL_BUILD_TABLES"));
    assert!(!table_index_mod.contains("BuildField"));
    assert!(!table_index_mod.contains("BuildForeignKey"));
    assert!(!table_index_mod.contains("BuildTable"));
    assert!(!table_index_mod.contains("ROW_TYPE"));
    assert!(!table_index_mod.contains("RowType"));
    assert!(!table_index_mod.contains(".table.ron"));

    assert!(achievement_ron.starts_with(nw_generated_guard::GENERATED_HEADER));
    let achievement_ron_body = achievement_ron
        .strip_prefix(nw_generated_guard::GENERATED_HEADER)
        .expect("generated RON carries the machine-owned header");
    assert!(achievement_ron_body.trim_start().starts_with('['));
    assert!(achievement_ron.contains("    (\n"));
    assert!(achievement_ron.contains("achievement_id"));
    assert!(!achievement_ron.contains("\"achievement_id\""));
    assert!(!achievement_ron.contains("schema_hash"));
    assert!(!achievement_ron.contains("key_crc"));
    assert!(!achievement_ron.contains("foreign_keys"));
}

fn resource_datasheet_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("resources")
        .join("datasheets")
}

fn sync_table_source_output(source: &Path, dest: &Path) -> io::Result<()> {
    if dest.exists() {
        fs::remove_dir_all(dest)?;
    }
    copy_dir_all(source, dest)?;
    Ok(())
}

fn copy_dir_all(source: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_name() == "_diagnostics" {
            continue;
        }
        let file_type = entry.file_type()?;
        let target = dest.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if entry
            .path()
            .extension()
            .is_some_and(|ext| ext == "rs" || ext == "ron")
        {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
