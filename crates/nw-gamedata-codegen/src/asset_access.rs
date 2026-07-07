//! Plan for self-contained New World game asset access codegen.
//!
//! Standalone targets need enough runtime support to read the shipping game
//! asset packages: pak archives, the asset catalogs inside those archives, and
//! the formats addressed by catalog asset ids. This module models that
//! requirement as compiler IR; language emitters are responsible for turning
//! the plan into idiomatic Rust, TypeScript, or Go source.

use std::path::{Path, PathBuf};

use crate::target::{
    GameAssetSupport, GameDataAssetSourcePlan, GameDataProduct, GameDataTargetLanguage,
    GameDataTargetPlan,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameAssetAccessCodegenPlan {
    runtimes: Vec<GameAssetRuntimePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameAssetRuntimePlan {
    target: GameDataTargetPlan,
    support: GameAssetSupport,
    modules: Vec<GameAssetRuntimeModule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameAssetRuntimeModule {
    kind: GameAssetRuntimeModuleKind,
    path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GameAssetRuntimeModuleKind {
    RustFacade,
    Filesystem,
    PakArchive,
    AssetCatalog,
    DatasheetDecoder,
    Localization,
    ObjectStream,
}

impl GameAssetAccessCodegenPlan {
    #[must_use]
    pub fn from_targets(targets: &[GameDataTargetPlan]) -> Self {
        let runtimes = targets
            .iter()
            .filter(|target| target.supports_product(GameDataProduct::GameAssetAccess))
            .filter_map(GameAssetRuntimePlan::from_target)
            .collect();
        Self { runtimes }
    }

    #[must_use]
    pub fn runtimes(&self) -> &[GameAssetRuntimePlan] {
        &self.runtimes
    }

    #[must_use]
    pub fn runtime_for_target(&self, target: &GameDataTargetPlan) -> Option<&GameAssetRuntimePlan> {
        self.runtimes
            .iter()
            .find(|runtime| runtime.target() == target)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runtimes.is_empty()
    }
}

impl GameAssetRuntimePlan {
    #[must_use]
    pub fn from_target(target: &GameDataTargetPlan) -> Option<Self> {
        let GameDataAssetSourcePlan::ShippingAssets(support) = target.source() else {
            return None;
        };
        Some(Self {
            target: target.clone(),
            support,
            modules: game_asset_runtime_modules(target.language(), support),
        })
    }

    #[must_use]
    pub const fn target(&self) -> &GameDataTargetPlan {
        &self.target
    }

    #[must_use]
    pub const fn support(&self) -> GameAssetSupport {
        self.support
    }

    #[must_use]
    pub fn modules(&self) -> &[GameAssetRuntimeModule] {
        &self.modules
    }

    #[must_use]
    pub fn module(&self, kind: GameAssetRuntimeModuleKind) -> Option<&GameAssetRuntimeModule> {
        self.modules.iter().find(|module| module.kind == kind)
    }
}

impl GameAssetRuntimeModule {
    #[must_use]
    pub fn new(kind: GameAssetRuntimeModuleKind, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            path: path.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> GameAssetRuntimeModuleKind {
        self.kind
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn game_asset_runtime_modules(
    language: GameDataTargetLanguage,
    support: GameAssetSupport,
) -> Vec<GameAssetRuntimeModule> {
    if matches!(language, GameDataTargetLanguage::Rust) {
        return vec![runtime_module(
            language,
            GameAssetRuntimeModuleKind::RustFacade,
        )];
    }

    let mut modules = Vec::with_capacity(6);
    if support.filesystem {
        modules.push(runtime_module(
            language,
            GameAssetRuntimeModuleKind::Filesystem,
        ));
    }
    if support.pak_archives {
        modules.push(runtime_module(
            language,
            GameAssetRuntimeModuleKind::PakArchive,
        ));
    }
    if support.asset_catalog {
        modules.push(runtime_module(
            language,
            GameAssetRuntimeModuleKind::AssetCatalog,
        ));
    }
    modules.push(runtime_module(
        language,
        GameAssetRuntimeModuleKind::DatasheetDecoder,
    ));
    modules.push(runtime_module(
        language,
        GameAssetRuntimeModuleKind::Localization,
    ));
    modules.push(runtime_module(
        language,
        GameAssetRuntimeModuleKind::ObjectStream,
    ));
    modules
}

fn runtime_module(
    language: GameDataTargetLanguage,
    kind: GameAssetRuntimeModuleKind,
) -> GameAssetRuntimeModule {
    GameAssetRuntimeModule::new(kind, runtime_module_path(language, kind))
}

fn runtime_module_path(
    language: GameDataTargetLanguage,
    kind: GameAssetRuntimeModuleKind,
) -> &'static str {
    match language {
        GameDataTargetLanguage::Rust => match kind {
            GameAssetRuntimeModuleKind::RustFacade => "src/assets.rs",
            GameAssetRuntimeModuleKind::Filesystem
            | GameAssetRuntimeModuleKind::PakArchive
            | GameAssetRuntimeModuleKind::AssetCatalog
            | GameAssetRuntimeModuleKind::DatasheetDecoder
            | GameAssetRuntimeModuleKind::Localization
            | GameAssetRuntimeModuleKind::ObjectStream => {
                unreachable!("Rust standalone asset access is provided by nw-tools facade")
            }
        },
        GameDataTargetLanguage::TypeScript => match kind {
            GameAssetRuntimeModuleKind::RustFacade => {
                unreachable!("Rust facade is not a TypeScript module")
            }
            GameAssetRuntimeModuleKind::Filesystem => "src/game-assets/filesystem.ts",
            GameAssetRuntimeModuleKind::PakArchive => "src/game-assets/pak.ts",
            GameAssetRuntimeModuleKind::AssetCatalog => "src/game-assets/catalog.ts",
            GameAssetRuntimeModuleKind::DatasheetDecoder => "src/game-assets/datasheet.ts",
            GameAssetRuntimeModuleKind::Localization => "src/game-assets/localization.ts",
            GameAssetRuntimeModuleKind::ObjectStream => "src/game-assets/object-stream.ts",
        },
        GameDataTargetLanguage::Go => match kind {
            GameAssetRuntimeModuleKind::RustFacade => {
                unreachable!("Rust facade is not a Go module")
            }
            GameAssetRuntimeModuleKind::Filesystem => "gameassets/filesystem.go",
            GameAssetRuntimeModuleKind::PakArchive => "gameassets/pak.go",
            GameAssetRuntimeModuleKind::AssetCatalog => "gameassets/catalog.go",
            GameAssetRuntimeModuleKind::DatasheetDecoder => "gameassets/datasheet.go",
            GameAssetRuntimeModuleKind::Localization => "gameassets/localization.go",
            GameAssetRuntimeModuleKind::ObjectStream => "gameassets/objectstream.go",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::GameDataTargetLanguage;

    #[test]
    fn repo_integrated_rust_has_no_self_contained_game_asset_runtime() {
        let plan = GameAssetAccessCodegenPlan::from_targets(&[GameDataTargetPlan::repo_rust()]);

        assert!(plan.is_empty());
    }

    #[test]
    fn standalone_targets_plan_filesystem_pak_catalog_and_datasheet_modules() {
        let target = GameDataTargetPlan::standalone(GameDataTargetLanguage::TypeScript);
        let plan = GameAssetAccessCodegenPlan::from_targets(&[target]);
        let runtime = plan.runtimes().first().expect("runtime plan");

        assert_eq!(runtime.modules().len(), 6);
        assert_eq!(
            runtime
                .module(GameAssetRuntimeModuleKind::Filesystem)
                .expect("filesystem module")
                .path(),
            Path::new("src/game-assets/filesystem.ts")
        );
        assert_eq!(
            runtime
                .module(GameAssetRuntimeModuleKind::PakArchive)
                .expect("pak module")
                .path(),
            Path::new("src/game-assets/pak.ts")
        );
        assert_eq!(
            runtime
                .module(GameAssetRuntimeModuleKind::AssetCatalog)
                .expect("catalog module")
                .path(),
            Path::new("src/game-assets/catalog.ts")
        );
        assert_eq!(
            runtime
                .module(GameAssetRuntimeModuleKind::DatasheetDecoder)
                .expect("datasheet module")
                .path(),
            Path::new("src/game-assets/datasheet.ts")
        );
        assert_eq!(
            runtime
                .module(GameAssetRuntimeModuleKind::Localization)
                .expect("localization module")
                .path(),
            Path::new("src/game-assets/localization.ts")
        );
        assert_eq!(
            runtime
                .module(GameAssetRuntimeModuleKind::ObjectStream)
                .expect("objectstream module")
                .path(),
            Path::new("src/game-assets/object-stream.ts")
        );
    }

    #[test]
    fn standalone_rust_plans_nw_tools_asset_facade_only() {
        let target = GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust);
        let plan = GameAssetAccessCodegenPlan::from_targets(&[target]);
        let runtime = plan.runtimes().first().expect("runtime plan");

        assert_eq!(runtime.modules().len(), 1);
        assert_eq!(
            runtime
                .module(GameAssetRuntimeModuleKind::RustFacade)
                .expect("rust asset facade")
                .path(),
            Path::new("src/assets.rs")
        );
        assert!(
            runtime
                .module(GameAssetRuntimeModuleKind::Filesystem)
                .is_none()
        );
    }
}
