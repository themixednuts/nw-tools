use crate::compiler::GameDataCompileUnit;
use crate::emit::{GameDataCodegenFile, GameDataCodegenOutput, GameDataEmitter};
use crate::manager::ManagerEmissionContext;
use crate::target::GameDataTargetLanguage;
use thiserror::Error;

mod managers;
mod project;
pub(crate) mod tables;

pub use managers::RustManagerSourceEmitter;
pub use project::{RustStandaloneProject, RustStandaloneProjectFile, RustStandaloneProjectOptions};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RustSourceEmitError {
    #[error("Rust source parse error: {0}")]
    File(String),
    #[error("Rust source formatting error: {0}")]
    Format(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RustSourceEmitter;

impl GameDataEmitter for RustSourceEmitter {
    fn target_language(&self) -> GameDataTargetLanguage {
        GameDataTargetLanguage::Rust
    }

    fn emit(&self, unit: &GameDataCompileUnit) -> anyhow::Result<GameDataCodegenOutput> {
        let mut files =
            self.emit_standalone_project_with_options(
                &project::RustStandaloneProjectOptions::default(),
            )?
            .into_codegen_files();
        files.extend(tables::emit_schema_files(unit)?);
        files.extend(crate::oodle_bundle::oodle_rust_runtime_files()?);
        files.extend(self.emit_manager_files(unit)?);

        Ok(GameDataCodegenOutput::new(self.target_language(), files))
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
                unit.codegen_plan_ref().managers(),
            ),
            unit.strict_schema_report(),
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
    let source = prettyplease::unparse(&file);
    let mut child = std::process::Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| RustSourceEmitError::Format(format!("start rustfmt: {error}")))?;
    {
        use std::io::Write as _;
        child
            .stdin
            .take()
            .expect("rustfmt stdin is piped")
            .write_all(source.as_bytes())
            .map_err(|error| {
                RustSourceEmitError::Format(format!("write rustfmt input: {error}"))
            })?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| RustSourceEmitError::Format(format!("wait for rustfmt: {error}")))?;
    if !output.status.success() {
        return Err(RustSourceEmitError::Format(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| RustSourceEmitError::Format(format!("decode rustfmt output: {error}")))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use nw_datasheet::game_system::GameSystemDataTables;

    use super::*;
    use crate::compiler::GameDataCompiler;
    use crate::emit::GameDataEmitter;

    #[test]
    fn standalone_manager_output_emits_rows_contracts() {
        let catalog = GameSystemDataTables::default();
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let output = RustSourceEmitter.emit(&unit).expect("rust output");
        let managers = output
            .files()
            .iter()
            .filter(|file| {
                matches!(
                    file.path().to_str(),
                    Some("src/managers/mod.rs" | "src/managers/runtime.rs")
                )
            })
            .map(|file| file.contents())
            .collect::<Vec<_>>()
            .join("\n");
        let surfaces = output
            .files()
            .iter()
            .filter(|file| file.path().starts_with("src/managers/surfaces"))
            .map(|file| file.contents())
            .collect::<Vec<_>>()
            .join("\n");
        let datasheets = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers/datasheets.rs"))
            .expect("Rust datasheet runtime")
            .contents();
        let products = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers/products/mod.rs"))
            .expect("Rust product module")
            .contents();
        let product_decoders = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers/products/decode.rs"))
            .expect("Rust product decoders")
            .contents();
        let camera_manager = output
            .files()
            .iter()
            .find(|file| {
                file.path() == Path::new("src/managers/surfaces/camera_settings_data_manager.rs")
            })
            .expect("Rust camera settings manager")
            .contents();
        let facade = output
            .files()
            .iter()
            .filter(|file| {
                file.path() == Path::new("src/managers/facade.rs")
                    || file.path().starts_with("src/managers/facade")
            })
            .map(|file| file.contents())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(managers.contains("pub trait Rows"));
        assert!(managers.contains("fn rows(&self) -> impl Iterator<Item = &Self::Row>"));
        assert!(managers.contains("pub struct RowCollection<Table, Row>"));
        assert!(managers.contains("table_indexes: Arc<HashMap<String, RowTableIndex>>"));
        assert!(managers.contains("by_key: HashMap<String, usize>"));
        assert!(!managers.contains("impl IntoCrc32Key for u32"));
        assert!(!managers.contains("column.field_name"));
        assert!(managers.contains("pub struct RowRef<Table, Row>"));
        assert!(!surfaces.contains("pub fn damage_data_rows"));
        assert!(!surfaces.contains("pub(super) fn ability_data_manager"));
        assert!(!surfaces.contains("fn from_instance(instance: Arc<ManagerInstance>)"));
        assert!(!surfaces.contains("instance: Arc<ManagerInstance>"));
        assert!(!surfaces.contains("pub fn items(&self)"));
        assert!(managers.contains("pub(super) fn normalize_data_path"));
        assert!(!datasheets.contains("fn normalize_data_path"));
        assert!(products.contains("pub(super) use decode::*"));
        assert!(
            product_decoders.contains("pub(in crate::managers) fn parse_armor_offset_database")
        );
        assert!(!camera_manager.contains("use super::super::runtime::*"));
        assert!(facade.contains("pub struct ManagerLoadError"));
        assert!(facade.contains("pub fn new(loader: &crate::assets::AssetLoader) -> Self"));
        assert!(facade.contains("pub fn player("));
        assert!(facade.contains("ManagerResult<&PlayerDataManager>"));
        assert!(facade.contains("OnceCell<PlayerDataManager>"));
        assert!(!facade.contains("pub fn player_data("));
        assert!(!facade.contains("pub fn load("));
        assert!(
            output
                .files()
                .iter()
                .any(|file| file.path() == Path::new("bin/oo2core_win64.lib"))
        );
        assert!(
            output
                .files()
                .iter()
                .any(|file| file.path() == Path::new("bin/oo2core_win64.dll"))
        );
        assert!(output.files().iter().all(|file| {
            file.path() != Path::new("bin/oo2core_9_win64.dll")
                && file.path() != Path::new("bin/oo2core_8_win64.dll")
        }));
    }
}
