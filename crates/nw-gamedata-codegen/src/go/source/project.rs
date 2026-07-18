use super::{GoSourceEmitError, format_go_source};
use crate::emit::GameDataCodegenFile;
use crate::project::{GO_TOOLCHAIN, GO_VERSION};

mod game_assets;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneProject {
    files: Vec<GoStandaloneProjectFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneProjectOptions {
    module_path: String,
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
        let mut files = vec![
            GoStandaloneProjectFile::new("go.mod", go_mod_source(options.module_path())),
            GoStandaloneProjectFile::new("go.sum", go_sum_source()),
            GoStandaloneProjectFile::new("assets/loader.go", assets_source(options)?),
            GoStandaloneProjectFile::new("types/types.go", game_assets::types_go_source()?),
        ];
        files.extend([
            GoStandaloneProjectFile::new(
                "internal/gameassets/catalog.go",
                game_assets::catalog_go_source()?,
            ),
            GoStandaloneProjectFile::new(
                "internal/gameassets/datasheet.go",
                game_assets::datasheet_go_source()?,
            ),
            GoStandaloneProjectFile::new(
                "internal/gameassets/localization.go",
                game_assets::localization_go_source()?,
            ),
            GoStandaloneProjectFile::new(
                "internal/gameassets/objectstream.go",
                game_assets::object_stream_go_source()?,
            ),
            GoStandaloneProjectFile::new(
                "internal/gameassets/oodle_unsupported.go",
                game_assets::oodle_unsupported_go_source()?,
            ),
            GoStandaloneProjectFile::new(
                "internal/gameassets/oodle_windows.go",
                game_assets::oodle_windows_go_source()?,
            ),
            GoStandaloneProjectFile::new(
                "internal/gameassets/pak.go",
                game_assets::pak_go_source()?,
            ),
        ]);
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
    pub fn new(module_path: impl Into<String>) -> Self {
        Self {
            module_path: module_path.into(),
        }
    }

    #[must_use]
    pub fn module_path(&self) -> &str {
        &self.module_path
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
    format!(
        "module {module_path}\n\ngo {GO_VERSION}\n\ntoolchain {GO_TOOLCHAIN}\n\nrequire github.com/google/uuid v1.6.0\n"
    )
}

fn go_sum_source() -> String {
    concat!(
        "github.com/google/uuid v1.6.0 h1:NIvaJDMOsjHA8n1jAhLSgzrAzy1Hgr+hNrb57e+94F0=\n",
        "github.com/google/uuid v1.6.0/go.mod h1:TIyPZe4MgqvfeYDBFedMoGGpEw/LqOeaOT+nhxU+yHo=\n",
    )
    .to_owned()
}

fn assets_source(options: &GoStandaloneProjectOptions) -> Result<String, GoSourceEmitError> {
    format_go_source(&format!(
        r#"
package assets

import (
	gameassets "{}/internal/gameassets"
)

type AssetLoader = gameassets.AssetLoader
type Catalog = gameassets.AssetCatalog
type CatalogEntry = gameassets.AssetCatalogEntry

func Open(assetRoot string) (*AssetLoader, error) {{
	return gameassets.Open(assetRoot)
}}
"#,
        options.module_path(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_project_uses_current_go_toolchain() {
        let emitter = crate::go::source::GoSourceEmitter;
        let project = emitter
            .emit_standalone_project_with_options(&GoStandaloneProjectOptions::new(
                "example.com/acme/newworld-gamedata",
            ))
            .expect("go project");
        let go_mod = project
            .files()
            .iter()
            .find(|file| file.path() == "go.mod")
            .expect("go.mod")
            .source();
        let assets = project
            .files()
            .iter()
            .find(|file| file.path() == "assets/loader.go")
            .expect("assets loader")
            .source();
        let go_sum = project
            .files()
            .iter()
            .find(|file| file.path() == "go.sum")
            .expect("go.sum")
            .source();

        assert!(go_mod.contains("module example.com/acme/newworld-gamedata"));
        assert!(go_mod.contains("go 1.26"));
        assert!(go_mod.contains("toolchain go1.26.4"));
        assert!(go_sum.contains("github.com/google/uuid v1.6.0"));
        assert!(
            project
                .files()
                .iter()
                .all(|file| file.path() != "gamedata.go")
        );
        assert!(assets.contains("type AssetLoader = gameassets.AssetLoader"));
        assert!(assets.contains("func Open(assetRoot string) (*AssetLoader, error)"));
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
                .all(|file| file.path() != "managers/managers.go")
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
                .all(|file| file.path() != "systems/systems.go")
        );
        assert!(
            project
                .files()
                .iter()
                .any(|file| file.path() == "internal/gameassets/pak.go")
        );
    }

    #[test]
    fn source_format_standalone_project_emits_pak_oodle_runtime_files() {
        let emitter = crate::go::source::GoSourceEmitter;
        let project = emitter
            .emit_standalone_project_with_options(&GoStandaloneProjectOptions::new(
                "example.com/acme/newworld-gamedata",
            ))
            .expect("go project");

        let paths = project
            .files()
            .iter()
            .map(|file| file.path())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"internal/gameassets/pak.go"));
        assert!(paths.contains(&"internal/gameassets/oodle_windows.go"));
        assert!(paths.contains(&"internal/gameassets/oodle_unsupported.go"));
        assert!(
            project
                .files()
                .iter()
                .find(|file| file.path() == "internal/gameassets/oodle_windows.go")
                .expect("windows oodle runtime")
                .source()
                .contains("OodleLZ_Decompress")
        );
    }
}
