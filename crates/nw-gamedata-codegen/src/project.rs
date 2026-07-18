//! Standalone project planning for GameData targets.
//!
//! This module keeps target-language package shape and toolchain versions out of
//! table inference. Language emitters can consume this plan to write complete
//! Rust, TypeScript, or Go projects around the shipping datasheet catalog,
//! merged schemas, semantic managers, and language package metadata.

use std::path::{Path, PathBuf};

use crate::target::GameDataTargetLanguage;

pub const RUST_EDITION: &str = "2024";
pub const RUST_VERSION: &str = "1.96";
pub const GO_VERSION: &str = "1.26";
pub const GO_TOOLCHAIN: &str = "go1.26.4";
pub const TYPESCRIPT_PACKAGE_MANAGER: &str = "bun@1.3.14";
pub const TYPESCRIPT_NODE_TYPES: &str = "^26.0.0";
pub const TYPESCRIPT_VERSION: &str = "^6.0.3";
pub const VITE_PLUS_VERSION: &str = "0.2.1";
pub const VITE_PLUS_CORE_OVERRIDE: &str = "npm:@voidzero-dev/vite-plus-core@0.2.1";
pub const VITE_PLUS_TEST_OVERRIDE: &str = "npm:@voidzero-dev/vite-plus-test@0.1.24";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandaloneProjectPlan {
    language: GameDataTargetLanguage,
    toolchain: GameDataToolchainPlan,
    files: Vec<StandaloneProjectFilePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataToolchainPlan {
    language: GameDataTargetLanguage,
    build_tool: GameDataBuildTool,
    dependencies: Vec<GameDataPackageDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataPackageDependency {
    name: &'static str,
    requirement: &'static str,
    role: GameDataPackageDependencyRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandaloneProjectFilePlan {
    kind: StandaloneProjectFileKind,
    path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GameDataBuildTool {
    Cargo,
    VitePlus,
    Go,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GameDataPackageDependencyRole {
    Toolchain,
    Runtime,
    Dev,
    Override,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StandaloneProjectFileKind {
    PackageManifest,
    BuildConfig,
    TypeScriptConfig,
    LibraryRoot,
    DatasheetCatalogModule,
    ManagersModule,
}

impl StandaloneProjectPlan {
    #[must_use]
    pub fn for_language(language: GameDataTargetLanguage) -> Self {
        Self {
            language,
            toolchain: GameDataToolchainPlan::for_language(language),
            files: standalone_project_files(language),
        }
    }

    #[must_use]
    pub const fn language(&self) -> GameDataTargetLanguage {
        self.language
    }

    #[must_use]
    pub const fn toolchain(&self) -> &GameDataToolchainPlan {
        &self.toolchain
    }

    #[must_use]
    pub fn files(&self) -> &[StandaloneProjectFilePlan] {
        &self.files
    }

    #[must_use]
    pub fn file(&self, kind: StandaloneProjectFileKind) -> Option<&StandaloneProjectFilePlan> {
        self.files.iter().find(|file| file.kind == kind)
    }
}

impl GameDataToolchainPlan {
    #[must_use]
    pub fn for_language(language: GameDataTargetLanguage) -> Self {
        match language {
            GameDataTargetLanguage::Rust => Self::rust(),
            GameDataTargetLanguage::TypeScript => Self::typescript_viteplus(),
            GameDataTargetLanguage::Go => Self::go(),
        }
    }

    #[must_use]
    pub fn rust() -> Self {
        let dependencies = vec![
            GameDataPackageDependency::toolchain("rust", RUST_VERSION),
            GameDataPackageDependency::toolchain("edition", RUST_EDITION),
            GameDataPackageDependency::runtime("anyhow", "1"),
            GameDataPackageDependency::runtime("flate2", "1.1.9"),
            GameDataPackageDependency::runtime("glam", "0.32"),
            GameDataPackageDependency::runtime(
                "nw-asset",
                "https://github.com/themixednuts/nw-tools",
            ),
            GameDataPackageDependency::runtime(
                "nw-datasheet",
                "https://github.com/themixednuts/nw-tools",
            ),
            GameDataPackageDependency::runtime(
                "nw-objectstream",
                "https://github.com/themixednuts/nw-tools",
            ),
            GameDataPackageDependency::runtime(
                "nw-pak",
                "https://github.com/themixednuts/nw-tools",
            ),
            GameDataPackageDependency::runtime("serde", "1"),
            GameDataPackageDependency::runtime("serde_json", "1"),
            GameDataPackageDependency::runtime("uuid", "1.23.3"),
        ];
        Self {
            language: GameDataTargetLanguage::Rust,
            build_tool: GameDataBuildTool::Cargo,
            dependencies,
        }
    }

    #[must_use]
    pub fn typescript_viteplus() -> Self {
        Self {
            language: GameDataTargetLanguage::TypeScript,
            build_tool: GameDataBuildTool::VitePlus,
            dependencies: vec![
                GameDataPackageDependency::toolchain("packageManager", TYPESCRIPT_PACKAGE_MANAGER),
                GameDataPackageDependency::dev("@types/node", TYPESCRIPT_NODE_TYPES),
                GameDataPackageDependency::dev("typescript", TYPESCRIPT_VERSION),
                GameDataPackageDependency::dev("vite-plus", VITE_PLUS_VERSION),
                GameDataPackageDependency::override_dependency("vite", VITE_PLUS_CORE_OVERRIDE),
                GameDataPackageDependency::override_dependency("vitest", VITE_PLUS_TEST_OVERRIDE),
            ],
        }
    }

    #[must_use]
    pub fn go() -> Self {
        Self {
            language: GameDataTargetLanguage::Go,
            build_tool: GameDataBuildTool::Go,
            dependencies: vec![
                GameDataPackageDependency::toolchain("go", GO_VERSION),
                GameDataPackageDependency::toolchain("toolchain", GO_TOOLCHAIN),
            ],
        }
    }

    #[must_use]
    pub const fn language(&self) -> GameDataTargetLanguage {
        self.language
    }

    #[must_use]
    pub const fn build_tool(&self) -> GameDataBuildTool {
        self.build_tool
    }

    #[must_use]
    pub fn dependencies(&self) -> &[GameDataPackageDependency] {
        &self.dependencies
    }

    #[must_use]
    pub fn dependency(&self, name: &str) -> Option<&GameDataPackageDependency> {
        self.dependencies
            .iter()
            .find(|dependency| dependency.name == name)
    }
}

impl GameDataPackageDependency {
    #[must_use]
    pub const fn new(
        name: &'static str,
        requirement: &'static str,
        role: GameDataPackageDependencyRole,
    ) -> Self {
        Self {
            name,
            requirement,
            role,
        }
    }

    #[must_use]
    pub const fn toolchain(name: &'static str, requirement: &'static str) -> Self {
        Self::new(name, requirement, GameDataPackageDependencyRole::Toolchain)
    }

    #[must_use]
    pub const fn runtime(name: &'static str, requirement: &'static str) -> Self {
        Self::new(name, requirement, GameDataPackageDependencyRole::Runtime)
    }

    #[must_use]
    pub const fn dev(name: &'static str, requirement: &'static str) -> Self {
        Self::new(name, requirement, GameDataPackageDependencyRole::Dev)
    }

    #[must_use]
    pub const fn override_dependency(name: &'static str, requirement: &'static str) -> Self {
        Self::new(name, requirement, GameDataPackageDependencyRole::Override)
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn requirement(&self) -> &'static str {
        self.requirement
    }

    #[must_use]
    pub const fn role(&self) -> GameDataPackageDependencyRole {
        self.role
    }
}

impl StandaloneProjectFilePlan {
    #[must_use]
    pub fn new(kind: StandaloneProjectFileKind, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            path: path.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> StandaloneProjectFileKind {
        self.kind
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn standalone_project_files(language: GameDataTargetLanguage) -> Vec<StandaloneProjectFilePlan> {
    let mut files = language_project_files(language);
    if matches!(language, GameDataTargetLanguage::Rust) {
        files.push(language_product_module(
            language,
            StandaloneProjectFileKind::DatasheetCatalogModule,
        ));
    }
    files.push(language_product_module(
        language,
        StandaloneProjectFileKind::ManagersModule,
    ));
    files
}

fn language_project_files(language: GameDataTargetLanguage) -> Vec<StandaloneProjectFilePlan> {
    match language {
        GameDataTargetLanguage::Rust => vec![
            StandaloneProjectFilePlan::new(
                StandaloneProjectFileKind::PackageManifest,
                "Cargo.toml",
            ),
            StandaloneProjectFilePlan::new(StandaloneProjectFileKind::LibraryRoot, "src/lib.rs"),
        ],
        GameDataTargetLanguage::TypeScript => vec![
            StandaloneProjectFilePlan::new(
                StandaloneProjectFileKind::PackageManifest,
                "package.json",
            ),
            StandaloneProjectFilePlan::new(
                StandaloneProjectFileKind::TypeScriptConfig,
                "tsconfig.json",
            ),
            StandaloneProjectFilePlan::new(
                StandaloneProjectFileKind::BuildConfig,
                "vite.config.ts",
            ),
            StandaloneProjectFilePlan::new(StandaloneProjectFileKind::LibraryRoot, "src/index.ts"),
        ],
        GameDataTargetLanguage::Go => vec![StandaloneProjectFilePlan::new(
            StandaloneProjectFileKind::PackageManifest,
            "go.mod",
        )],
    }
}

fn language_product_module(
    language: GameDataTargetLanguage,
    kind: StandaloneProjectFileKind,
) -> StandaloneProjectFilePlan {
    StandaloneProjectFilePlan::new(kind, language_product_module_path(language, kind))
}

fn language_product_module_path(
    language: GameDataTargetLanguage,
    kind: StandaloneProjectFileKind,
) -> &'static str {
    match language {
        GameDataTargetLanguage::Rust => match kind {
            StandaloneProjectFileKind::DatasheetCatalogModule => "src/datasheet_catalog.rs",
            StandaloneProjectFileKind::ManagersModule => "src/managers/mod.rs",
            _ => unreachable!("product module kind"),
        },
        GameDataTargetLanguage::TypeScript => match kind {
            StandaloneProjectFileKind::DatasheetCatalogModule => {
                "src/managers/datasheet-catalog.ts"
            }
            StandaloneProjectFileKind::ManagersModule => "src/managers/index.ts",
            _ => unreachable!("product module kind"),
        },
        GameDataTargetLanguage::Go => match kind {
            StandaloneProjectFileKind::DatasheetCatalogModule => "managers/datasheet_catalog.go",
            StandaloneProjectFileKind::ManagersModule => "managers/managers.go",
            _ => unreachable!("product module kind"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typescript_standalone_projects_use_viteplus_toolchain() {
        let project = StandaloneProjectPlan::for_language(GameDataTargetLanguage::TypeScript);

        assert_eq!(
            project.toolchain().build_tool(),
            GameDataBuildTool::VitePlus
        );
        assert_eq!(
            project
                .toolchain()
                .dependency("vite-plus")
                .expect("vite-plus")
                .requirement(),
            VITE_PLUS_VERSION
        );
        assert!(
            project
                .toolchain()
                .dependencies()
                .iter()
                .all(|dependency| !dependency.name().contains("native"))
        );
        assert_eq!(
            project
                .toolchain()
                .dependency("vite")
                .expect("vite override")
                .requirement(),
            VITE_PLUS_CORE_OVERRIDE
        );
        assert_eq!(
            project
                .toolchain()
                .dependency("vitest")
                .expect("vitest override")
                .requirement(),
            VITE_PLUS_TEST_OVERRIDE
        );
        assert_eq!(
            project
                .file(StandaloneProjectFileKind::BuildConfig)
                .expect("vite config")
                .path(),
            Path::new("vite.config.ts")
        );
    }

    #[test]
    fn go_and_rust_standalone_projects_track_current_toolchains() {
        let rust = GameDataToolchainPlan::rust();
        assert_eq!(rust.build_tool(), GameDataBuildTool::Cargo);
        assert_eq!(
            rust.dependency("rust").expect("rust version").requirement(),
            RUST_VERSION
        );
        assert_eq!(
            rust.dependency("edition")
                .expect("rust edition")
                .requirement(),
            RUST_EDITION
        );
        assert!(rust.dependency("ron").is_none());
        assert!(rust.dependency("newworld-datasheet").is_none());
        assert_eq!(
            rust.dependency("anyhow").expect("anyhow").requirement(),
            "1"
        );
        assert_eq!(
            rust.dependency("flate2").expect("flate2").requirement(),
            "1.1.9"
        );
        assert!(rust.dependency("nw-asset").is_some());
        assert!(rust.dependency("nw-datasheet").is_some());
        assert!(rust.dependency("nw-filesystem").is_none());
        assert!(rust.dependency("nw-jobs").is_none());
        assert!(rust.dependency("nw-localization").is_none());
        assert!(rust.dependency("nw-objectstream").is_some());
        assert!(rust.dependency("nw-pak").is_some());
        assert_eq!(rust.dependency("serde").expect("serde").requirement(), "1");
        assert_eq!(
            rust.dependency("serde_json")
                .expect("serde_json")
                .requirement(),
            "1"
        );
        assert_eq!(rust.dependency("glam").expect("glam").requirement(), "0.32");
        assert!(rust.dependency("thiserror").is_none());
        assert_eq!(
            rust.dependency("nw-datasheet")
                .expect("nw-datasheet")
                .requirement(),
            "https://github.com/themixednuts/nw-tools"
        );
        assert!(rust.dependency("az-framework-asset-catalog").is_none());
        assert!(rust.dependency("az-pak").is_none());
        assert!(rust.dependency("bevy").is_none());
        assert!(rust.dependency("gamedata").is_none());
        assert!(rust.dependency("newworld-plugin").is_none());
        assert_eq!(
            rust.dependency("uuid").expect("uuid").requirement(),
            "1.23.3"
        );

        let go = GameDataToolchainPlan::go();
        assert_eq!(go.build_tool(), GameDataBuildTool::Go);
        assert_eq!(
            go.dependency("go").expect("go version").requirement(),
            GO_VERSION
        );
        assert_eq!(
            go.dependency("toolchain")
                .expect("go toolchain")
                .requirement(),
            GO_TOOLCHAIN
        );
    }

    #[test]
    fn go_project_uses_domain_packages_without_an_empty_root_facade() {
        let project = StandaloneProjectPlan::for_language(GameDataTargetLanguage::Go);

        assert!(
            project
                .file(StandaloneProjectFileKind::LibraryRoot)
                .is_none()
        );
    }
}
