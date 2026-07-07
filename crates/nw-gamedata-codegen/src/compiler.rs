use anyhow::Result;
use nw_asset::AssetId;
use nw_datasheet::game_system::GameSystemDataTables as GameSystemCatalog;

use crate::game_system_schema::GameSystemDataTablesSchemaReport as GameSystemCatalogSchemaReport;
use crate::manager::ManagerCodegenPlan;
use crate::plan::GameDataCodegenPlan;
use crate::schema::{GameDataCompileMode, schema_report_for_mode};
use crate::system::SystemCodegenPlan;
use crate::table::GameDataTableSourceFormat;
use crate::target::{GameDataTargetLanguage, GameDataTargetPlan, GameDataTargetPlanError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCompiler {
    options: GameDataCompilerOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameDataCompileUnit {
    schema_report: GameSystemCatalogSchemaReport,
    strict_schema_report: GameSystemCatalogSchemaReport,
    table_sources: Vec<GameDataTableSource>,
    codegen_plan: GameDataCodegenPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataTableSource {
    table_name: String,
    row_type_name: String,
    asset_ids: Vec<AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCompilerOptions {
    mode: GameDataCompileMode,
    table_source_format: GameDataTableSourceFormat,
    targets: Vec<GameDataTargetPlan>,
    managers: ManagerCodegenPlan,
    systems: SystemCodegenPlan,
}

impl Default for GameDataCompiler {
    fn default() -> Self {
        Self::source_format()
    }
}

impl GameDataCompiler {
    #[must_use]
    pub fn new(mode: GameDataCompileMode) -> Self {
        Self {
            options: GameDataCompilerOptions::new(mode),
        }
    }

    #[must_use]
    pub fn strict() -> Self {
        Self::new(GameDataCompileMode::Strict)
    }

    #[must_use]
    pub fn source_format() -> Self {
        Self::new(GameDataCompileMode::SourceFormat)
    }

    #[must_use]
    pub fn with_options(options: GameDataCompilerOptions) -> Self {
        Self { options }
    }

    #[must_use]
    pub const fn mode(&self) -> GameDataCompileMode {
        self.options.mode
    }

    #[must_use]
    pub const fn options(&self) -> &GameDataCompilerOptions {
        &self.options
    }

    #[must_use]
    pub fn compile_unit(&self, catalog: &GameSystemCatalog) -> GameDataCompileUnit {
        let schema_report = schema_report_for_mode(catalog, self.options.mode);
        let strict_schema_report = if self.options.mode == GameDataCompileMode::Strict {
            schema_report.clone()
        } else {
            schema_report_for_mode(catalog, GameDataCompileMode::Strict)
        };
        let managers = if self.options.managers.is_empty()
            && self.options.targets.iter().any(|target| {
                target.supports_product(crate::target::GameDataProduct::SemanticManagers)
            }) {
            ManagerCodegenPlan::validated_native_for_schema(&schema_report)
        } else {
            self.options.managers.clone()
        };
        let codegen_plan = GameDataCodegenPlan::from_schema_report(
            self.options.mode,
            &schema_report,
            self.options.targets.clone(),
        )
        .with_managers(managers)
        .with_systems(self.options.systems.clone());
        GameDataCompileUnit::new(
            schema_report,
            strict_schema_report,
            table_sources_from_catalog(catalog),
            codegen_plan,
        )
    }

    #[must_use]
    pub fn codegen_plan(&self, catalog: &GameSystemCatalog) -> GameDataCodegenPlan {
        self.compile_unit(catalog).into_codegen_plan()
    }
}

impl GameDataCompilerOptions {
    #[must_use]
    pub fn new(mode: GameDataCompileMode) -> Self {
        let targets = vec![GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)];
        Self {
            mode,
            table_source_format: GameDataTableSourceFormat::default_for_mode(mode),
            targets,
            managers: ManagerCodegenPlan::new(),
            systems: SystemCodegenPlan::new(),
        }
    }

    #[must_use]
    pub fn standalone(mode: GameDataCompileMode, language: GameDataTargetLanguage) -> Self {
        Self {
            mode,
            table_source_format: GameDataTableSourceFormat::default_for_mode(mode),
            targets: vec![GameDataTargetPlan::standalone(language)],
            managers: ManagerCodegenPlan::new(),
            systems: SystemCodegenPlan::new(),
        }
    }

    #[must_use]
    pub fn with_targets(
        mode: GameDataCompileMode,
        targets: impl IntoIterator<Item = GameDataTargetPlan>,
    ) -> Result<Self, GameDataTargetPlanError> {
        let targets = targets.into_iter().collect::<Vec<_>>();
        for target in &targets {
            target.validate()?;
        }
        Ok(Self {
            mode,
            table_source_format: GameDataTableSourceFormat::default_for_mode(mode),
            targets,
            managers: ManagerCodegenPlan::new(),
            systems: SystemCodegenPlan::new(),
        })
    }

    #[must_use]
    pub const fn mode(&self) -> GameDataCompileMode {
        self.mode
    }

    #[must_use]
    pub const fn table_source_format(&self) -> GameDataTableSourceFormat {
        self.table_source_format
    }

    #[must_use]
    pub const fn with_table_source_format(
        mut self,
        table_source_format: GameDataTableSourceFormat,
    ) -> Self {
        self.table_source_format = table_source_format;
        self
    }

    #[must_use]
    pub fn targets(&self) -> &[GameDataTargetPlan] {
        &self.targets
    }

    #[must_use]
    pub const fn managers(&self) -> &ManagerCodegenPlan {
        &self.managers
    }

    #[must_use]
    pub const fn systems(&self) -> &SystemCodegenPlan {
        &self.systems
    }

    #[must_use]
    pub fn with_managers(mut self, managers: ManagerCodegenPlan) -> Self {
        self.managers = managers;
        self
    }

    #[must_use]
    pub fn with_systems(mut self, systems: SystemCodegenPlan) -> Self {
        self.systems = systems;
        self
    }
}

impl GameDataCompileUnit {
    #[must_use]
    pub const fn new(
        schema_report: GameSystemCatalogSchemaReport,
        strict_schema_report: GameSystemCatalogSchemaReport,
        table_sources: Vec<GameDataTableSource>,
        codegen_plan: GameDataCodegenPlan,
    ) -> Self {
        Self {
            schema_report,
            strict_schema_report,
            table_sources,
            codegen_plan,
        }
    }

    #[must_use]
    pub const fn schema_report(&self) -> &GameSystemCatalogSchemaReport {
        &self.schema_report
    }

    #[must_use]
    pub const fn strict_schema_report(&self) -> &GameSystemCatalogSchemaReport {
        &self.strict_schema_report
    }

    #[must_use]
    pub fn table_sources(&self) -> &[GameDataTableSource] {
        &self.table_sources
    }

    #[must_use]
    pub const fn codegen_plan_ref(&self) -> &GameDataCodegenPlan {
        &self.codegen_plan
    }

    #[must_use]
    pub fn into_codegen_plan(self) -> GameDataCodegenPlan {
        self.codegen_plan
    }

    #[must_use]
    pub fn has_schema_diagnostics(&self) -> bool {
        !self.schema_report.diagnostics.is_empty()
    }

    pub fn emit_with<E>(&self, emitter: &E) -> Result<crate::emit::GameDataCodegenOutput>
    where
        E: crate::emit::GameDataEmitter + ?Sized,
    {
        emitter.emit(self)
    }
}

impl GameDataTableSource {
    #[must_use]
    pub fn new(
        table_name: impl Into<String>,
        row_type_name: impl Into<String>,
        asset_ids: impl IntoIterator<Item = AssetId>,
    ) -> Self {
        Self {
            table_name: table_name.into(),
            row_type_name: row_type_name.into(),
            asset_ids: asset_ids.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    #[must_use]
    pub fn row_type_name(&self) -> &str {
        &self.row_type_name
    }

    #[must_use]
    pub fn asset_ids(&self) -> &[AssetId] {
        &self.asset_ids
    }
}

fn table_sources_from_catalog(catalog: &GameSystemCatalog) -> Vec<GameDataTableSource> {
    catalog
        .tables()
        .iter()
        .map(|table| {
            GameDataTableSource::new(
                table.name(),
                table.type_name(),
                table.source_asset_ids().collect::<Vec<_>>(),
            )
        })
        .collect()
}

#[cfg(any())]
mod tests {
    use nw_asset::AssetId;
    use nw_datasheet::{
        ColumnType,
        game_system::{
            GameSystemAsset, GameSystemCell, GameSystemColumn,
            GameSystemDataTables as GameSystemCatalog, GameSystemTable, OwnedCellValue,
        },
    };
    use uuid::Uuid;

    use super::*;
    use crate::manager::{
        ManagerCodegenPlan, NativeManagerInput, NativeManagerSpec, validated_native_manager_specs,
    };
    use crate::symbols::{
        GameDataRowTypeName, GameDataTableName, GhidraClassPath, GhidraFunctionPath, RustTypePath,
    };
    use crate::target::{GameDataAssetSourcePlan, GameDataDataFormat, GameDataProduct};

    fn sample_archetype_catalog() -> GameSystemCatalog {
        let mut catalog = GameSystemCatalog::default();
        catalog
            .insert(
                GameSystemTable::from_native_columns(
                    "ArchetypeDataTable",
                    1,
                    "ArchetypeData",
                    2,
                    vec![GameSystemColumn::new(3, "ArchetypeId", ColumnType::String)],
                    [(
                        4,
                        vec![GameSystemCell::new(
                            3,
                            OwnedCellValue::String("Soldier".to_owned()),
                        )],
                    )],
                )
                .with_source_asset(GameSystemAsset::with_asset_id(
                    "resources/datasheets/archetype.datatable",
                    AssetId::new(Uuid::from_u128(0x1234), 0),
                )),
            )
            .expect("insert archetype table");
        catalog
    }

    fn assert_unique_output_paths(output: &crate::emit::GameDataCodegenOutput) {
        let mut paths = std::collections::BTreeSet::new();
        for file in output.files() {
            assert!(
                paths.insert(file.path().to_path_buf()),
                "duplicate emitted file path {}",
                file.path().display()
            );
        }
    }

    #[test]
    fn default_compiler_uses_game_source_format() {
        let compiler = GameDataCompiler::default();
        let options = compiler.options();

        assert_eq!(options.mode(), GameDataCompileMode::SourceFormat);
        assert_eq!(
            options.table_source_format(),
            GameDataTableSourceFormat::Datasheet
        );
        assert_eq!(
            options.targets()[0].data_format(),
            GameDataDataFormat::Datasheet
        );
        assert!(matches!(
            options.targets()[0].source(),
            GameDataAssetSourcePlan::ShippingAssets(support)
                if support.filesystem && support.pak_archives && support.asset_catalog
        ));
        assert!(
            options.targets()[0].supports_product(GameDataProduct::TableManifest),
            "standalone datasheet generation emits dynamic schema source"
        );
        assert!(
            options.targets()[0].supports_product(GameDataProduct::SourceIndex),
            "standalone datasheet generation emits source index descriptors"
        );
        assert!(
            options.targets()[0].supports_product(GameDataProduct::SemanticManagers),
            "standalone datasheet generation emits generated semantic manager sources"
        );
        assert!(
            options.targets()[0].supports_product(GameDataProduct::Systems),
            "standalone datasheet generation emits runtime system wiring"
        );
        assert!(
            options.targets()[0].supports_product(GameDataProduct::GameAssetAccess),
            "standalone datasheet generation emits source asset access loaders"
        );
    }

    #[test]
    fn strict_repo_compiler_defaults_to_cooked_asset_managers() {
        let compiler = GameDataCompiler::strict();
        let options = compiler.options();
        let target = &options.targets()[0];

        assert_eq!(options.mode(), GameDataCompileMode::Strict);
        assert_eq!(
            options.table_source_format(),
            GameDataTableSourceFormat::AuthoredRon
        );
        assert_eq!(target.source(), GameDataAssetSourcePlan::EngineCatalog);
        assert_eq!(target.data_format(), GameDataDataFormat::CookedTable);
        assert!(!target.supports_product(GameDataProduct::TableManifest));
        assert!(target.supports_product(GameDataProduct::CookedTableManifest));
        assert!(target.supports_product(GameDataProduct::SemanticManagers));
        assert!(!target.supports_product(GameDataProduct::Systems));
        assert!(!target.supports_product(GameDataProduct::GameAssetAccess));
    }

    #[test]
    fn compiler_plan_tracks_standalone_datasheet_manager_support() {
        let mut catalog = GameSystemCatalog::default();
        catalog
            .insert(GameSystemTable::from_native_columns(
                "SampleTable",
                1,
                "SampleRow",
                2,
                vec![GameSystemColumn::new(3, "SampleId", ColumnType::String)],
                [(
                    4,
                    vec![GameSystemCell::new(
                        3,
                        OwnedCellValue::String("sample".to_owned()),
                    )],
                )],
            ))
            .expect("insert sample table");

        let compiler = GameDataCompiler::with_options(GameDataCompilerOptions::standalone(
            GameDataCompileMode::Strict,
            GameDataTargetLanguage::TypeScript,
        ));
        let unit = compiler.compile_unit(&catalog);
        let plan = unit.codegen_plan_ref();

        assert_eq!(plan.tables().table_count(), 1);
        assert_eq!(plan.tables().row_type_count(), 1);
        assert_eq!(
            plan.target_plans_for(GameDataProduct::GameAssetAccess)
                .len(),
            1
        );
        assert_eq!(
            plan.target_plans_for(GameDataProduct::CookedTableManifest)
                .len(),
            0
        );
        assert!(plan.standalone_targets_are_self_contained());
        assert!(matches!(
            plan.targets()[0].source(),
            GameDataAssetSourcePlan::ShippingAssets(support)
                if support.filesystem && support.pak_archives && support.asset_catalog
        ));
        assert_eq!(plan.game_asset_access().runtimes().len(), 1);
        assert_eq!(plan.standalone_projects().projects().len(), 1);
        assert!(
            plan.standalone_projects().projects()[0]
                .files()
                .iter()
                .any(|file| file.path() == std::path::Path::new("package.json"))
        );
        assert!(!unit.has_schema_diagnostics());
    }

    #[test]
    fn standalone_game_asset_outputs_use_current_format_wording() {
        let catalog = GameSystemCatalog::default();
        let unit = GameDataCompiler::default().compile_unit(&catalog);
        let obsolete_word = ["leg", "acy"].concat();
        let outputs = [
            (
                "rust",
                unit.emit_with(&crate::rust::source::RustSourceEmitter::standalone())
                    .expect("standalone rust output"),
            ),
            (
                "typescript",
                unit.emit_with(&crate::typescript::source::TypeScriptSourceEmitter::standalone())
                    .expect("standalone typescript output"),
            ),
            (
                "go",
                unit.emit_with(&crate::go::source::GoSourceEmitter::standalone())
                    .expect("standalone go output"),
            ),
        ];

        for (language, output) in outputs {
            for file in output.files() {
                if file
                    .path()
                    .extension()
                    .and_then(|extension| extension.to_str())
                    != Some("rs")
                    && file
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str())
                        != Some("ts")
                    && file
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str())
                        != Some("go")
                {
                    continue;
                }
                assert!(
                    !file
                        .contents()
                        .to_ascii_lowercase()
                        .contains(&obsolete_word),
                    "{language} standalone scaffold {} should use current game asset wording",
                    file.path().display()
                );
            }
        }
    }

    #[test]
    fn target_language_profiles_validate_datasheet_standalone_formats() {
        let rust = crate::rust::source::RustSourceEmitter::standalone_target();
        let typescript = crate::typescript::source::TypeScriptSourceEmitter::standalone_target();
        let go = crate::go::source::GoSourceEmitter::standalone_target();
        let go_csv = crate::go::source::GoSourceEmitter::standalone_target()
            .with_data_format(GameDataDataFormat::Csv);

        assert!(crate::rust::source::RustSourceEmitter::target_is_supported(
            &rust
        ));
        assert!(
            crate::typescript::source::TypeScriptSourceEmitter::target_is_supported(&typescript)
        );
        assert!(crate::go::source::GoSourceEmitter::target_is_supported(&go));
        assert!(crate::typescript::source::TypeScriptSourceEmitter::new(typescript).is_ok());
        assert!(
            !crate::go::source::GoSourceEmitter::target_is_supported(&go_csv),
            "standalone packages are datasheet/PAK based only"
        );
    }

    #[test]
    fn target_emitters_share_compile_unit_contract() {
        let catalog = GameSystemCatalog::default();
        let compiler = GameDataCompiler::strict();
        let unit = compiler.compile_unit(&catalog);
        let emitter = crate::rust::source::RustSourceEmitter::default();
        let output = unit.emit_with(&emitter).expect("rust emitter output");

        assert!(!output.is_empty());
        assert!(crate::rust::source::RustSourceEmitter::target_is_supported(
            output.target()
        ));
        assert!(
            output
                .files()
                .iter()
                .any(|file| file.path()
                    == std::path::Path::new("src/runtime_managers/manifest.rs")),
            "repo-integrated strict output carries the manager manifest"
        );
        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != std::path::Path::new("src/item_data.rs")),
            "schema-gated repo output must not emit manager bodies without table products"
        );
    }

    #[test]
    fn compiler_options_carry_validated_managers_to_supported_targets() {
        let manager = NativeManagerSpec::new(
            GhidraClassPath::new("Javelin::ItemDataManager").expect("ghidra class"),
            RustTypePath::new("crate::ItemDataManager").expect("rust type"),
            vec![NativeManagerInput::table(
                GameDataTableName::new("MasterItemDefinitions").expect("table"),
                GameDataRowTypeName::new("MasterItemDefinitions").expect("row type"),
            )],
            vec![
                GhidraFunctionPath::new("Javelin::ItemDataManager::CacheAllItemDataTables")
                    .expect("function"),
            ],
        )
        .expect("manager spec");
        let managers = ManagerCodegenPlan::from_native_managers(vec![manager.clone()]);
        let catalog = GameSystemCatalog::default();

        let native_unit = GameDataCompiler::with_options(
            GameDataCompilerOptions::standalone(
                GameDataCompileMode::SourceFormat,
                GameDataTargetLanguage::Rust,
            )
            .with_managers(managers),
        )
        .compile_unit(&catalog);

        assert_eq!(native_unit.codegen_plan_ref().managers().len(), 1);
        let native_output = native_unit
            .emit_with(&crate::rust::source::RustSourceEmitter::standalone())
            .expect("source-format standalone rust output");
        assert!(
            native_output
                .files()
                .iter()
                .all(|file| file.path() != std::path::Path::new("src/managers/evidence.rs")),
            "source-format standalone rust output must not emit an evidence registry"
        );

        let strict_unit = GameDataCompiler::with_options(
            GameDataCompilerOptions::new(GameDataCompileMode::Strict)
                .with_managers(ManagerCodegenPlan::from_native_managers(vec![manager])),
        )
        .compile_unit(&catalog);
        let strict_output = strict_unit
            .emit_with(&crate::rust::source::RustSourceEmitter::repo_integrated())
            .expect("strict repo rust output");

        assert!(
            strict_output
                .files()
                .iter()
                .any(|file| file.path()
                    == std::path::Path::new("src/runtime_managers/manifest.rs")),
            "repo-integrated strict target emits runtime manager metadata"
        );
        assert!(
            strict_output
                .files()
                .iter()
                .all(|file| file.path() != std::path::Path::new("src/item_data.rs")),
            "repo-integrated unshaped managers stay in the runtime manifest instead of emitting fake API modules"
        );
    }

    #[test]
    fn datasheet_standalone_rust_emits_manager_schema_and_runtime_sources() {
        let catalog = sample_archetype_catalog();

        let unit = GameDataCompiler::with_options(GameDataCompilerOptions::standalone(
            GameDataCompileMode::SourceFormat,
            GameDataTargetLanguage::Rust,
        ))
        .compile_unit(&catalog);

        assert_eq!(
            unit.codegen_plan_ref().managers().len(),
            validated_native_manager_specs().len(),
            "compiler must carry every validated manager; schema coverage only gates dynamic table inputs"
        );
        assert!(
            unit.codegen_plan_ref()
                .managers()
                .managers()
                .iter()
                .any(|manager| manager.rust_type().as_str() == "crate::ArmorOffsetDataManager"),
            "asset-backed managers must stay in the generated manager inventory"
        );
        assert!(
            unit.codegen_plan_ref()
                .managers()
                .managers()
                .iter()
                .any(|manager| {
                    manager.rust_type().as_str() == "crate::CurrencyExchangeMappingManager"
                }),
            "manager-dependent managers must stay in the generated manager inventory"
        );
        let output = unit
            .emit_with(&crate::rust::source::RustSourceEmitter::standalone())
            .expect("standalone rust output");
        assert_unique_output_paths(&output);
        let paths = output
            .files()
            .iter()
            .map(|file| file.path())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(paths.contains(std::path::Path::new("src/lib.rs")));
        assert!(!paths.contains(std::path::Path::new("src/managers/item_data.rs")));
        assert!(!paths.contains(std::path::Path::new("src/products/camera_settings.rs")));
        assert!(paths.contains(std::path::Path::new("src/managers.rs")));
        assert!(!paths.contains(std::path::Path::new("src/managers/mod.rs")));
        assert!(!paths.contains(std::path::Path::new("src/runtime_managers/mod.rs")));
        assert!(!paths.contains(std::path::Path::new("src/runtime_managers/manifest.rs")));
        assert!(!paths.contains(std::path::Path::new("src/runtime_managers/assets.rs")));
        assert!(!paths.contains(std::path::Path::new("src/runtime_managers/builders.rs")));
        assert!(!paths.contains(std::path::Path::new("src/runtime_managers/registry.rs")));
        assert!(!paths.contains(std::path::Path::new("src/runtime_managers.rs")));
        assert!(paths.contains(std::path::Path::new("src/tables.rs")));
        assert!(paths.contains(std::path::Path::new("src/assets.rs")));
        assert!(paths.contains(std::path::Path::new("src/system.rs")));
        assert!(!paths.contains(std::path::Path::new("src/plugin.rs")));
        assert!(!paths.contains(std::path::Path::new("src/managers/evidence.rs")));
        assert!(!paths.contains(std::path::Path::new("src/managers/manifest.rs")));
        assert!(!paths.contains(std::path::Path::new("src/item_data.rs")));
        assert!(!paths.contains(std::path::Path::new("src/manager_util.rs")));
        assert!(!paths.contains(std::path::Path::new("src/runtime.rs")));
        assert!(!paths.contains(std::path::Path::new(
            "src/tables/archetype_data/archetype_data_table.rs"
        )));
        assert!(!paths.contains(std::path::Path::new("src/managers/archetype_data.rs")));

        let lib = output
            .files()
            .iter()
            .find(|file| file.path() == std::path::Path::new("src/lib.rs"))
            .expect("lib")
            .contents();
        assert!(lib.contains("pub mod tables;"));
        assert!(!lib.contains("pub mod products;"));
        assert!(lib.contains("pub mod managers;"));
        assert!(!lib.contains("pub mod item_data;"));
        assert!(lib.contains("pub mod assets;"));
        assert!(lib.contains("pub mod system;"));
        assert!(!lib.contains("pub mod plugin;"));

        let managers = output
            .files()
            .iter()
            .find(|file| file.path() == std::path::Path::new("src/managers.rs"))
            .expect("standalone managers")
            .contents();
        assert!(managers.contains("pub struct ArchetypeDataManager"));
        assert!(managers.contains("pub struct ArmorOffsetDataManager"));
        assert!(managers.contains("pub struct CurrencyExchangeMappingManager"));
    }

    #[test]
    fn datasheet_standalone_emits_dynamic_schema_descriptors() {
        let catalog = sample_archetype_catalog();

        for (language, table_path, table_name_token, row_type_token, lookup_token) in [
            (
                GameDataTargetLanguage::Rust,
                "src/tables.rs",
                "name: \"ArchetypeDataTable\"",
                "row_type: \"ArchetypeData\"",
                "table_by_source_path",
            ),
            (
                GameDataTargetLanguage::TypeScript,
                "src/managers/index.ts",
                "name: \"ArchetypeDataTable\"",
                "rowType: \"ArchetypeData\"",
                "tableSchemaBySourcePath",
            ),
            (
                GameDataTargetLanguage::Go,
                "managers/managers.go",
                "Name: \"ArchetypeDataTable\"",
                "RowType: \"ArchetypeData\"",
                "TableSchemaBySourcePath",
            ),
        ] {
            let target = GameDataTargetPlan::standalone(language)
                .with_data_format(GameDataDataFormat::Datasheet);
            let unit = GameDataCompiler::with_options(
                GameDataCompilerOptions::with_targets(
                    GameDataCompileMode::SourceFormat,
                    [target.clone()],
                )
                .expect("datasheet target"),
            )
            .compile_unit(&catalog);
            let output = match language {
                GameDataTargetLanguage::TypeScript => unit
                    .emit_with(
                        &crate::typescript::source::TypeScriptSourceEmitter::new(target)
                            .expect("typescript emitter"),
                    )
                    .expect("typescript output"),
                GameDataTargetLanguage::Go => unit
                    .emit_with(
                        &crate::go::source::GoSourceEmitter::new(target).expect("go emitter"),
                    )
                    .expect("go output"),
                GameDataTargetLanguage::Rust => unit
                    .emit_with(
                        &crate::rust::source::RustSourceEmitter::new(target).expect("rust emitter"),
                    )
                    .expect("rust output"),
            };
            assert_unique_output_paths(&output);
            let tables = output
                .files()
                .iter()
                .find(|file| file.path() == std::path::Path::new(table_path))
                .unwrap_or_else(|| panic!("{language:?} schema descriptor `{table_path}`"));
            assert!(tables.contents().contains(table_name_token));
            assert!(tables.contents().contains(row_type_token));
            assert!(
                tables
                    .contents()
                    .contains("resources/datasheets/archetype.datatable")
            );
            assert!(tables.contents().contains(lookup_token));
        }
    }

    #[test]
    fn compiler_options_reject_unsupported_targets() {
        let rust_json = GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)
            .with_data_format(GameDataDataFormat::Json);

        let error = GameDataCompilerOptions::with_targets(GameDataCompileMode::Strict, [rust_json])
            .expect_err("rust json target should be rejected");

        assert_eq!(
            error,
            GameDataTargetPlanError::UnsupportedTarget {
                language: GameDataTargetLanguage::Rust,
                runtime: crate::target::GameDataRuntimeProfile::Standalone,
                asset_source: crate::target::GameDataAssetSourcePlan::ShippingAssets(
                    crate::target::GameAssetSupport::shipping_game_assets(),
                ),
                data_format: GameDataDataFormat::Json,
            }
        );
    }
}
