use crate::compiler::GameDataCompileUnit;
use crate::emit::{
    GameDataCodegenFile, GameDataCodegenOutput, GameDataEmitter, GameDataEmitterConfigError,
};
use crate::manager::ManagerEmissionContext;
use crate::target::{
    GameDataProduct, GameDataRuntimeProfile, GameDataTargetLanguage, GameDataTargetPlan,
};
use thiserror::Error;

mod managers;
mod project;
mod tables;

pub use managers::RustManagerSourceEmitter;
pub use project::{RustStandaloneProject, RustStandaloneProjectFile, RustStandaloneProjectOptions};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RustSourceEmitError {
    #[error("Rust source parse error: {0}")]
    File(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceEmitter {
    target: GameDataTargetPlan,
}

impl RustSourceEmitter {
    pub fn new(target: GameDataTargetPlan) -> Result<Self, GameDataEmitterConfigError> {
        if Self::target_is_supported(&target) {
            Ok(Self { target })
        } else {
            Err(GameDataEmitterConfigError::unsupported(
                GameDataTargetLanguage::Rust,
                &target,
            ))
        }
    }

    #[must_use]
    pub fn standalone() -> Self {
        Self {
            target: Self::standalone_target(),
        }
    }

    #[must_use]
    pub fn standalone_target() -> GameDataTargetPlan {
        GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)
    }

    #[must_use]
    pub fn target_is_supported(target: &GameDataTargetPlan) -> bool {
        target.supports_language(GameDataTargetLanguage::Rust)
            && matches!(target.runtime(), GameDataRuntimeProfile::Standalone)
    }
}

impl Default for RustSourceEmitter {
    fn default() -> Self {
        Self::standalone()
    }
}

impl GameDataEmitter for RustSourceEmitter {
    fn target(&self) -> GameDataTargetPlan {
        self.target.clone()
    }

    fn emit(&self, unit: &GameDataCompileUnit) -> anyhow::Result<GameDataCodegenOutput> {
        let mut files = if matches!(self.target.runtime(), GameDataRuntimeProfile::Standalone) {
            self.emit_standalone_project_with_options(
                &project::RustStandaloneProjectOptions::default().with_product_placeholders(false),
            )?
            .into_codegen_files()
        } else {
            Vec::new()
        };

        if matches!(self.target.runtime(), GameDataRuntimeProfile::Standalone)
            && self.target.supports_product(GameDataProduct::TableManifest)
        {
            files.extend(tables::emit_schema_files(unit)?);
        }

        if matches!(self.target.runtime(), GameDataRuntimeProfile::Standalone)
            && self
                .target
                .supports_product(GameDataProduct::GameAssetAccess)
        {
            files.extend(crate::oodle_bundle::oodle_runtime_files()?);
        }

        if matches!(self.target.runtime(), GameDataRuntimeProfile::Standalone)
            && self.target.supports_product(GameDataProduct::Systems)
        {
            files.push(GameDataCodegenFile::new(
                "src/system.rs",
                project::standalone_system_source()?,
            ));
        }

        if self
            .target
            .supports_product(GameDataProduct::SemanticManagers)
        {
            if matches!(self.target.runtime(), GameDataRuntimeProfile::Standalone) {
                files.extend(self.emit_manager_files(unit)?);
            }
        }

        Ok(GameDataCodegenOutput::new(self.target(), files))
    }
}

impl RustSourceEmitter {
    fn emit_manager_files(
        &self,
        unit: &GameDataCompileUnit,
    ) -> anyhow::Result<Vec<GameDataCodegenFile>> {
        let output = RustManagerSourceEmitter.emit_managers_with_schema_report(
            ManagerEmissionContext::new(
                unit.codegen_plan_ref().mode(),
                &self.target,
                unit.codegen_plan_ref().managers(),
            ),
            unit.schema_report(),
        )?;
        Ok(output
            .into_files()
            .into_iter()
            .map(|file| {
                let (path, contents) = file.into_parts();
                GameDataCodegenFile::new(path, contents)
            })
            .collect())
    }
}

pub(crate) fn format_rust_source(source: &str) -> Result<String, RustSourceEmitError> {
    let file =
        syn::parse_file(source).map_err(|source| RustSourceEmitError::File(source.to_string()))?;
    Ok(prettyplease::unparse(&file))
}

#[cfg(any())]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use nw_datasheet::game_system::GameSystemDataTables;

    use super::*;
    use crate::compiler::GameDataCompiler;
    use crate::emit::GameDataEmitter;
    use crate::target::{GameDataDataFormat, GameDataTargetLanguage};

    #[test]
    fn strict_repo_output_includes_generated_cooked_runtime_managers() {
        let catalog = GameSystemDataTables::default();
        let unit = GameDataCompiler::strict().compile_unit(&catalog);
        let output = RustSourceEmitter::repo_integrated()
            .emit(&unit)
            .expect("strict rust output");

        let runtime_registry = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/runtime_managers/registry.rs"))
            .expect("runtime manager registry source");
        assert!(
            runtime_registry
                .contents()
                .contains("pub(crate) fn register_table_resource_managers")
        );

        let lib = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/lib.rs"))
            .expect("repo-integrated runtime lib source");
        assert!(!lib.contents().contains("pub mod recipe_data;"));
        assert!(lib.contents().contains("mod bevy_runtime;"));
        assert!(lib.contents().contains("mod runtime_managers;"));
        assert!(lib.contents().contains("mod runtime_projection;"));
        assert!(!lib.contents().contains("declare_gem!"));
        assert!(!lib.contents().contains("mod table_products;"));
        assert!(!lib.contents().contains("pub mod tables;"));
        assert!(
            output
                .files()
                .iter()
                .any(|file| file.path() == Path::new("src/runtime_managers/manifest.rs"))
        );
        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("src/recipe_data.rs"))
        );
        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("src/managers/recipe_data.rs"))
        );
        assert!(output.files().iter().all(|file| {
            !file
                .path()
                .components()
                .any(|component| component.as_os_str() == std::ffi::OsStr::new("tables"))
        }));
        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("src/damage_data.rs"))
        );
    }

    #[test]
    fn standalone_datasheet_output_includes_generated_manager_surface_only() {
        let catalog = GameSystemDataTables::default();
        let target = GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)
            .with_data_format(GameDataDataFormat::Datasheet);
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let output = RustSourceEmitter::new(target)
            .expect("rust source emitter")
            .emit(&unit)
            .expect("source format rust output");

        let emitted_paths = output
            .files()
            .iter()
            .map(|file| file.path().to_path_buf())
            .collect::<BTreeSet<_>>();

        assert!(emitted_paths.contains(Path::new("src/table_manifest.rs")));
        assert!(emitted_paths.contains(Path::new("src/assets.rs")));
        assert!(emitted_paths.contains(Path::new("src/managers.rs")));
        assert!(!emitted_paths.contains(Path::new("src/tables/mod.rs")));
        assert!(!emitted_paths.contains(Path::new("src/assets/mod.rs")));
        assert!(!emitted_paths.contains(Path::new("src/assets/datasheet.rs")));
        assert!(!emitted_paths.contains(Path::new("src/products/mod.rs")));
        assert!(!emitted_paths.contains(Path::new("src/managers/mod.rs")));
        assert!(!emitted_paths.contains(Path::new("src/runtime_managers/mod.rs")));
        assert!(!emitted_paths.contains(Path::new("src/runtime_managers/manifest.rs")));
        assert!(!emitted_paths.contains(Path::new("src/runtime_managers/assets.rs")));
        assert!(!emitted_paths.contains(Path::new("src/runtime_managers/builders.rs")));
        assert!(!emitted_paths.contains(Path::new("src/runtime_managers/registry.rs")));
        assert!(!emitted_paths.contains(Path::new("src/runtime_managers.rs")));
        assert!(!emitted_paths.contains(Path::new("src/managers/evidence.rs")));
        assert!(!emitted_paths.contains(Path::new("src/recipe_data.rs")));
        assert!(!emitted_paths.contains(Path::new("src/item_data.rs")));
        assert!(!emitted_paths.contains(Path::new("src/managers/archetype_data.rs")));
        assert!(!emitted_paths.contains(Path::new("src/managers/game_debug_settings.rs")));
        assert!(!emitted_paths.contains(Path::new("src/managers/item_data.rs")));

        let managers = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers.rs"))
            .expect("standalone managers")
            .contents();
        assert!(managers.contains("pub struct ArmorOffsetDataManager"));
        assert!(managers.contains("sharedassets/genericassets/items/armoroffsets.aoffdb"));
        assert!(managers.contains("sharedassets/genericassets/gatheringactiondatabase.gactdb"));
        assert!(managers.contains("pub struct UiDataManager"));
        assert!(managers.contains("pub struct PlayerDataManager"));
        assert!(managers.contains("pub struct PerkBucketDataManager"));
        assert!(managers.contains("pub struct ReplicationDataManager"));
        assert!(managers.contains("pub struct WeaponRefDataManager"));
        assert!(managers.contains("pub struct DifficultyScalingDataManager"));
        assert!(managers.contains("pub struct PvpRankDataManager"));
        assert!(managers.contains("pub struct LootTagPresetDataManager"));
        assert!(managers.contains("pub struct DiminishingReturnsDataManager"));
        assert!(managers.contains("pub struct DivertedLootDataManager"));
        assert!(managers.contains("pub struct DungeonClusterStaticDataManager"));
        assert!(managers.contains("pub struct LevelDisparityDataManager"));
        assert!(managers.contains("pub struct CostumeChangeDataManager"));
        assert!(managers.contains("pub struct CurseMutationStaticDataManager"));
        assert!(managers.contains("pub struct CutsceneCameraDataManager"));
        assert!(managers.contains("pub struct EconomyTrackerDataManager"));
        assert!(managers.contains("ManagerDependency::Asset"));
        assert!(managers.contains("pub struct CurrencyExchangeMappingManager"));
        assert!(managers.contains("ManagerDependency::Manager"));
        assert!(!managers.contains("pub enum ManagerDependency"));
        assert!(!managers.contains("pub struct ManagerDefinition"));
        assert!(!managers.contains("pub const MANAGERS"));
        assert!(!managers.contains("pub definition:"));
        assert!(!managers.contains("pub fn manager("));
        assert!(!managers.contains("pub fn table("));
        assert!(!managers.contains("pub fn row_by_key"));
        assert!(!managers.contains("pub fn cell_by_key"));

        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("src/managers/recipe_data.rs"))
        );

        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("src/damage_data.rs"))
        );

        let tables = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/table_manifest.rs"))
            .expect("emitted table descriptors")
            .contents();
        assert!(tables.contains("pub struct TableDescriptor"));
        assert!(tables.contains("pub struct ColumnDescriptor"));
        assert!(tables.contains("pub const TABLES: &[TableDescriptor]"));
        assert!(tables.contains("pub fn table_by_source_path"));
        assert!(!tables.contains("impl gamedata::Table for"));

        let lib = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/lib.rs"))
            .expect("emitted lib")
            .contents();
        assert!(lib.contains("pub mod managers;"));
        assert!(!lib.contains("pub mod recipe_data;"));
        assert!(!lib.contains("pub mod item_data;"));

        assert!(output.files().iter().all(|file| {
            std::str::from_utf8(file.contents_bytes()).map_or(true, |contents| {
                !contents.to_ascii_lowercase().contains("cooked")
            })
        }));
    }

    #[test]
    fn standalone_manager_output_includes_every_exported_manager() {
        let catalog = GameSystemDataTables::default();
        let target = GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)
            .with_data_format(GameDataDataFormat::Datasheet);
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let output = RustSourceEmitter::new(target)
            .expect("rust source emitter")
            .emit(&unit)
            .expect("source format rust output");

        let managers_mod = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers.rs"))
            .expect("emitted managers")
            .contents();
        let exported_managers = manager_definition_name_tokens(managers_mod);
        let emitted_manager_names = public_manager_struct_names(managers_mod);

        let missing = exported_managers
            .difference(&emitted_manager_names)
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "emitted standalone manager surface is missing exported manager source token(s): {missing:?}"
        );
    }

    #[test]
    fn standalone_manager_output_includes_every_bevy_resource_manager() {
        let catalog = GameSystemDataTables::default();
        let target = GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)
            .with_data_format(GameDataDataFormat::Datasheet);
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let output = RustSourceEmitter::new(target)
            .expect("rust source emitter")
            .emit(&unit)
            .expect("source format rust output");

        assert!(output.files().iter().all(|file| {
            !file
                .path()
                .components()
                .any(|component| component.as_os_str() == std::ffi::OsStr::new("runtime_managers"))
        }));
        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("src/bevy_runtime.rs"))
        );
    }

    fn manager_definition_name_tokens(source: &str) -> BTreeSet<String> {
        const MANAGERS_PREFIX: &str = "const MANAGERS: &[ManagerDefinition] = &[";
        const PREFIX: &str = "name: \"";
        let Some((_, managers)) = source.split_once(MANAGERS_PREFIX) else {
            return BTreeSet::new();
        };
        let managers = managers
            .split_once("];")
            .map_or(managers, |(block, _)| block);
        managers
            .match_indices(PREFIX)
            .filter_map(|(index, _)| {
                let rest = &managers[index + PREFIX.len()..];
                let end = rest.find('"')?;
                let name = &rest[..end];
                name.ends_with("Manager").then(|| name.to_owned())
            })
            .collect()
    }

    fn public_manager_struct_names(source: &str) -> BTreeSet<String> {
        const PREFIX: &str = "pub struct ";
        source
            .match_indices(PREFIX)
            .filter_map(|(index, _)| {
                let rest = &source[index + PREFIX.len()..];
                let name = rest
                    .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                    .next()?;
                name.ends_with("Manager").then(|| name.to_owned())
            })
            .collect()
    }
}
