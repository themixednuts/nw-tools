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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use nw_datasheet::game_system::GameSystemDataTables;

    use crate::compiler::GameDataCompiler;
    use crate::emit::GameDataEmitter;
    use crate::target::GameDataDataFormat;

    use super::*;

    #[test]
    fn standalone_manager_output_emits_rows_contracts() {
        let catalog = GameSystemDataTables::default();
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let target =
            RustSourceEmitter::standalone_target().with_data_format(GameDataDataFormat::Datasheet);
        let output = RustSourceEmitter::new(target)
            .expect("rust datasheet emitter")
            .emit(&unit)
            .expect("rust output");
        let managers = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers/mod.rs"))
            .expect("manager runtime")
            .contents();
        let surfaces = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers/surfaces.rs"))
            .expect("manager surfaces")
            .contents();

        assert!(managers.contains("pub trait Rows"));
        assert!(managers.contains("fn rows(&self) -> Result<Vec<Self::Row>>"));
        assert!(!surfaces.contains("pub fn damage_data_rows"));
    }
}
