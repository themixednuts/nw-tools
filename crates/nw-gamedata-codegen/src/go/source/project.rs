use crate::emit::GameDataCodegenFile;
use crate::project::{GO_TOOLCHAIN, GO_VERSION};
use crate::target::GameDataProduct;

use super::{GoSourceEmitError, format_go_source, is_go_identifier};

mod game_assets;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneProject {
    files: Vec<GoStandaloneProjectFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneProjectOptions {
    module_path: String,
    package_name: String,
    include_product_placeholders: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneProjectFile {
    path: String,
    source: String,
}

impl Default for GoStandaloneProjectOptions {
    fn default() -> Self {
        Self {
            module_path: "example.com/newworld/gamedata".to_owned(),
            package_name: "gamedata".to_owned(),
            include_product_placeholders: true,
        }
    }
}

impl super::GoSourceEmitter {
    pub fn emit_standalone_project(&self) -> Result<GoStandaloneProject, GoSourceEmitError> {
        self.emit_standalone_project_with_options(&GoStandaloneProjectOptions::default())
    }

    pub fn emit_standalone_project_with_options(
        &self,
        options: &GoStandaloneProjectOptions,
    ) -> Result<GoStandaloneProject, GoSourceEmitError> {
        if !is_go_identifier(options.package_name()) {
            return Err(GoSourceEmitError::PackageName {
                package_name: options.package_name().to_owned(),
            });
        }

        let mut files = vec![
            GoStandaloneProjectFile::new("go.mod", go_mod_source(options.module_path())),
            GoStandaloneProjectFile::new("gamedata.go", gamedata_root_source(options)?),
        ];
        if options.include_product_placeholders {
            if self
                .target
                .supports_product(GameDataProduct::SemanticManagers)
            {
                files.push(GoStandaloneProjectFile::new(
                    "managers/managers.go",
                    format_go_source("package managers\n")?,
                ));
            }
            if self.target.supports_product(GameDataProduct::Systems) {
                files.push(GoStandaloneProjectFile::new(
                    "systems/systems.go",
                    format_go_source("package systems\n")?,
                ));
            }
        }
        if self
            .target
            .supports_product(GameDataProduct::GameAssetAccess)
        {
            files.extend([
                GoStandaloneProjectFile::new(
                    "gameassets/catalog.go",
                    game_assets::catalog_go_source()?,
                ),
                GoStandaloneProjectFile::new(
                    "gameassets/datasheet.go",
                    game_assets::datasheet_go_source()?,
                ),
                GoStandaloneProjectFile::new(
                    "gameassets/filesystem.go",
                    game_assets::filesystem_go_source()?,
                ),
                GoStandaloneProjectFile::new(
                    "gameassets/localization.go",
                    game_assets::localization_go_source()?,
                ),
                GoStandaloneProjectFile::new(
                    "gameassets/objectstream.go",
                    game_assets::object_stream_go_source()?,
                ),
                GoStandaloneProjectFile::new(
                    "gameassets/oodle_unsupported.go",
                    game_assets::oodle_unsupported_go_source()?,
                ),
                GoStandaloneProjectFile::new(
                    "gameassets/oodle_windows.go",
                    game_assets::oodle_windows_go_source()?,
                ),
                GoStandaloneProjectFile::new("gameassets/pak.go", game_assets::pak_go_source()?),
            ]);
        }
        Ok(GoStandaloneProject { files })
    }
}

impl GoStandaloneProject {
    #[must_use]
    pub fn files(&self) -> &[GoStandaloneProjectFile] {
        &self.files
    }

    #[must_use]
    pub fn into_files(self) -> Vec<GoStandaloneProjectFile> {
        self.files
    }

    #[must_use]
    pub fn into_codegen_files(self) -> Vec<GameDataCodegenFile> {
        self.files
            .into_iter()
            .map(GoStandaloneProjectFile::into_codegen_file)
            .collect()
    }
}

impl GoStandaloneProjectOptions {
    #[must_use]
    pub fn new(module_path: impl Into<String>, package_name: impl Into<String>) -> Self {
        Self {
            module_path: module_path.into(),
            package_name: package_name.into(),
            include_product_placeholders: true,
        }
    }

    #[must_use]
    pub fn module_path(&self) -> &str {
        &self.module_path
    }

    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    #[must_use]
    pub const fn with_product_placeholders(mut self, include: bool) -> Self {
        self.include_product_placeholders = include;
        self
    }
}

impl GoStandaloneProjectFile {
    #[must_use]
    pub fn new(path: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn into_codegen_file(self) -> GameDataCodegenFile {
        GameDataCodegenFile::new(self.path, self.source)
    }
}

fn go_mod_source(module_path: &str) -> String {
    format!("module {module_path}\n\ngo {GO_VERSION}\n\ntoolchain {GO_TOOLCHAIN}\n")
}

fn gamedata_root_source(options: &GoStandaloneProjectOptions) -> Result<String, GoSourceEmitError> {
    format_go_source(&format!(
        r#"
package {}

import (
	"{}/gameassets"
	"{}/managers"
)

type AssetLoader = gameassets.AssetLoader
type Managers = managers.Managers

func OpenDir(assetRoot string, pakPaths ...string) (*AssetLoader, error) {{
	return gameassets.OpenDir(assetRoot, pakPaths...)
}}

func OpenManagers(loader *AssetLoader) (*Managers, error) {{
	return managers.Open(loader)
}}
"#,
        options.package_name(),
        options.module_path(),
        options.module_path(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_project_uses_current_go_toolchain() {
        let emitter = crate::go::source::GoSourceEmitter::standalone();
        let project = emitter
            .emit_standalone_project_with_options(&GoStandaloneProjectOptions::new(
                "example.com/acme/newworld-gamedata",
                "nwgamedata",
            ))
            .expect("go project");
        let go_mod = project
            .files()
            .iter()
            .find(|file| file.path() == "go.mod")
            .expect("go.mod")
            .source();
        let root = project
            .files()
            .iter()
            .find(|file| file.path() == "gamedata.go")
            .expect("gamedata.go")
            .source();

        assert!(go_mod.contains("module example.com/acme/newworld-gamedata"));
        assert!(go_mod.contains("go 1.26"));
        assert!(go_mod.contains("toolchain go1.26.4"));
        assert!(root.contains("package nwgamedata"));
        assert!(root.contains("\"example.com/acme/newworld-gamedata/gameassets\""));
        assert!(root.contains("\"example.com/acme/newworld-gamedata/managers\""));
        assert!(root.contains("type AssetLoader = gameassets.AssetLoader"));
        assert!(root.contains("type Managers = managers.Managers"));
        assert!(
            root.contains(
                "func OpenDir(assetRoot string, pakPaths ...string) (*AssetLoader, error)"
            )
        );
        assert!(root.contains("func OpenManagers(loader *AssetLoader) (*Managers, error)"));
        assert!(
            project
                .files()
                .iter()
                .all(|file| file.path() != "products/products.go")
        );
        assert!(
            project
                .files()
                .iter()
                .any(|file| file.path() == "managers/managers.go")
        );
        assert!(
            project
                .files()
                .iter()
                .all(|file| file.path() != "tables/tables.go")
        );
        assert!(
            project
                .files()
                .iter()
                .any(|file| file.path() == "systems/systems.go")
        );
        assert!(
            project
                .files()
                .iter()
                .any(|file| file.path() == "gameassets/pak.go")
        );
    }

    #[test]
    fn source_format_standalone_project_emits_pak_oodle_runtime_files() {
        let emitter = crate::go::source::GoSourceEmitter::standalone();
        let project = emitter
            .emit_standalone_project_with_options(&GoStandaloneProjectOptions::new(
                "example.com/acme/newworld-gamedata",
                "nwgamedata",
            ))
            .expect("go project");

        let paths = project
            .files()
            .iter()
            .map(|file| file.path())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"gameassets/pak.go"));
        assert!(paths.contains(&"gameassets/oodle_windows.go"));
        assert!(paths.contains(&"gameassets/oodle_unsupported.go"));
        assert!(
            project
                .files()
                .iter()
                .find(|file| file.path() == "gameassets/oodle_windows.go")
                .expect("windows oodle runtime")
                .source()
                .contains("OodleLZ_Decompress")
        );
    }

    #[test]
    fn standalone_project_rejects_invalid_go_package_name() {
        let emitter = crate::go::source::GoSourceEmitter::standalone();
        let error = emitter
            .emit_standalone_project_with_options(&GoStandaloneProjectOptions::new(
                "example.com/acme/newworld-gamedata",
                "newworld-gamedata",
            ))
            .expect_err("invalid package name");

        assert!(matches!(error, GoSourceEmitError::PackageName { .. }));
    }
}
