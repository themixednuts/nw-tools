use crate::emit::GameDataCodegenFile;
use crate::target::{GameDataProduct, GameDataTargetPlan};

use super::{TypeScriptSourceEmitError, format_typescript_source};

mod game_assets;
mod package;

use package::{TYPESCRIPT_VITEPLUS_TSCONFIG, viteplus_config, viteplus_package_json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneProject {
    files: Vec<TypeScriptStandaloneProjectFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneProjectOptions {
    package_name: String,
    pack_entries: Vec<String>,
    include_product_placeholders: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneProjectFile {
    path: String,
    source: String,
}

impl Default for TypeScriptStandaloneProjectOptions {
    fn default() -> Self {
        Self {
            package_name: "newworld-gamedata".to_owned(),
            pack_entries: vec!["src/index.ts".to_owned()],
            include_product_placeholders: true,
        }
    }
}

impl super::TypeScriptSourceEmitter {
    pub fn emit_standalone_project(
        &self,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        self.emit_standalone_project_with_options(&TypeScriptStandaloneProjectOptions::default())
    }

    pub fn emit_standalone_project_with_options(
        &self,
        options: &TypeScriptStandaloneProjectOptions,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        let mut files = vec![
            TypeScriptStandaloneProjectFile::new(
                "package.json",
                viteplus_package_json(options.package_name()),
            ),
            TypeScriptStandaloneProjectFile::new(
                "tsconfig.json",
                TYPESCRIPT_VITEPLUS_TSCONFIG.to_owned(),
            ),
            TypeScriptStandaloneProjectFile::new(
                "vite.config.ts",
                format_typescript_source(&viteplus_config(options.pack_entries()))?,
            ),
            TypeScriptStandaloneProjectFile::new("src/index.ts", index_ts_source(&self.target)?),
        ];
        if options.include_product_placeholders {
            if self
                .target
                .supports_product(GameDataProduct::SemanticManagers)
            {
                files.push(TypeScriptStandaloneProjectFile::new(
                    "src/managers/index.ts",
                    "export {};\n",
                ));
            }
            if self.target.supports_product(GameDataProduct::Systems) {
                files.push(TypeScriptStandaloneProjectFile::new(
                    "src/systems/index.ts",
                    "export {};\n",
                ));
            }
        }
        if self
            .target
            .supports_product(GameDataProduct::GameAssetAccess)
        {
            files.extend([
                TypeScriptStandaloneProjectFile::new(
                    "src/game-assets/catalog.ts",
                    game_assets::catalog_ts_source()?,
                ),
                TypeScriptStandaloneProjectFile::new(
                    "src/game-assets/datasheet.ts",
                    game_assets::datasheet_ts_source()?,
                ),
                TypeScriptStandaloneProjectFile::new(
                    "src/game-assets/filesystem.ts",
                    game_assets::filesystem_ts_source()?,
                ),
                TypeScriptStandaloneProjectFile::new(
                    "src/game-assets/localization.ts",
                    game_assets::localization_ts_source()?,
                ),
                TypeScriptStandaloneProjectFile::new(
                    "src/game-assets/object-stream.ts",
                    game_assets::object_stream_ts_source()?,
                ),
                TypeScriptStandaloneProjectFile::new(
                    "src/game-assets/pak.ts",
                    game_assets::pak_ts_source()?,
                ),
            ]);
        }
        Ok(TypeScriptStandaloneProject { files })
    }
}

impl TypeScriptStandaloneProject {
    #[must_use]
    pub fn files(&self) -> &[TypeScriptStandaloneProjectFile] {
        &self.files
    }

    #[must_use]
    pub fn into_files(self) -> Vec<TypeScriptStandaloneProjectFile> {
        self.files
    }

    #[must_use]
    pub fn into_codegen_files(self) -> Vec<GameDataCodegenFile> {
        self.files
            .into_iter()
            .map(TypeScriptStandaloneProjectFile::into_codegen_file)
            .collect()
    }
}

impl TypeScriptStandaloneProjectOptions {
    #[must_use]
    pub fn new(package_name: impl Into<String>) -> Self {
        Self {
            package_name: package_name.into(),
            pack_entries: vec!["src/index.ts".to_owned()],
            include_product_placeholders: true,
        }
    }

    #[must_use]
    pub fn with_pack_entries(mut self, pack_entries: impl IntoIterator<Item = String>) -> Self {
        self.pack_entries = pack_entries.into_iter().collect();
        self
    }

    #[must_use]
    pub const fn with_product_placeholders(mut self, include: bool) -> Self {
        self.include_product_placeholders = include;
        self
    }

    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    #[must_use]
    pub fn pack_entries(&self) -> &[String] {
        &self.pack_entries
    }
}

impl TypeScriptStandaloneProjectFile {
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

fn index_ts_source(target: &GameDataTargetPlan) -> Result<String, TypeScriptSourceEmitError> {
    let mut source = String::new();
    if target.supports_product(GameDataProduct::SemanticManagers) {
        source.push_str("export * from \"./managers/index.js\";\n");
    }
    if target.supports_product(GameDataProduct::Systems) {
        source.push_str("export * from \"./systems/index.js\";\n");
    }
    if target.supports_product(GameDataProduct::GameAssetAccess) {
        source.push_str(
            r#"export * from "./game-assets/catalog.js";
export * from "./game-assets/datasheet.js";
export * from "./game-assets/filesystem.js";
export * from "./game-assets/localization.js";
export * from "./game-assets/object-stream.js";
export * from "./game-assets/pak.js";
"#,
        );
    }
    if source.is_empty() {
        source.push_str("export {};\n");
    }
    format_typescript_source(&source)
}

#[cfg(test)]
mod tests {
    use crate::project::{
        TYPESCRIPT_NODE_TYPES, TYPESCRIPT_PACKAGE_MANAGER, VITE_PLUS_CORE_OVERRIDE,
        VITE_PLUS_TEST_OVERRIDE, VITE_PLUS_VERSION,
    };
    use crate::target::GameDataRuntimeProfile;

    use super::*;

    #[test]
    fn standalone_project_uses_viteplus_current_package_template() {
        let emitter = crate::typescript::source::TypeScriptSourceEmitter::standalone();
        assert_eq!(emitter.target.runtime(), GameDataRuntimeProfile::Standalone);

        let project = emitter
            .emit_standalone_project_with_options(&TypeScriptStandaloneProjectOptions::new(
                "@azoth/newworld-gamedata",
            ))
            .expect("typescript project");
        let package_json = project
            .files()
            .iter()
            .find(|file| file.path() == "package.json")
            .expect("package json")
            .source();
        let vite_config = project
            .files()
            .iter()
            .find(|file| file.path() == "vite.config.ts")
            .expect("vite config")
            .source();
        let index = project
            .files()
            .iter()
            .find(|file| file.path() == "src/index.ts")
            .expect("index")
            .source();

        assert_eq!(project.files().len(), 12);
        assert!(package_json.contains("\"name\": \"@azoth/newworld-gamedata\""));
        assert!(package_json.contains("\"fast-xml-parser\": \"5.9.3\""));
        assert!(package_json.contains(&format!("\"vite-plus\": \"{VITE_PLUS_VERSION}\"")));
        assert!(package_json.contains(&format!("\"@types/node\": \"{TYPESCRIPT_NODE_TYPES}\"")));
        assert!(!package_json.contains("native"));
        assert!(package_json.contains(&format!(
            "\"packageManager\": \"{TYPESCRIPT_PACKAGE_MANAGER}\""
        )));
        assert!(package_json.contains(&format!("\"vite\": \"{VITE_PLUS_CORE_OVERRIDE}\"")));
        assert!(package_json.contains(&format!("\"vitest\": \"{VITE_PLUS_TEST_OVERRIDE}\"")));
        assert!(!package_json.contains("@latest"));
        assert!(vite_config.contains("from \"vite-plus\""));
        assert!(vite_config.contains("entry: [\"src/index.ts\"]"));
        assert!(index.contains("./managers/index.js"));
        assert!(index.contains("./systems/index.js"));
        assert!(!index.contains("./tables/index.js"));
        assert!(index.contains("./game-assets/pak.js"));
        assert!(!index.contains("./products/index.js"));
        assert!(
            project
                .files()
                .iter()
                .all(|file| file.path() != "src/products/index.ts")
        );
        assert!(
            project
                .files()
                .iter()
                .any(|file| file.path() == "src/managers/index.ts")
        );
        assert!(
            project
                .files()
                .iter()
                .all(|file| file.path() != "src/tables/index.ts")
        );
        assert!(
            project
                .files()
                .iter()
                .any(|file| file.path() == "src/systems/index.ts")
        );
        assert!(
            project
                .files()
                .iter()
                .any(|file| file.path() == "src/game-assets/pak.ts")
        );
    }
}
