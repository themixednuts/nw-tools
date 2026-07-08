use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GameDataTargetLanguage {
    Rust,
    TypeScript,
    Go,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameDataRuntimeProfile {
    RepoIntegrated,
    Standalone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GameDataProduct {
    CookedTableManifest,
    SemanticManagers,
    TableManifest,
    Systems,
    GameAssetAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameDataDataFormat {
    CookedTable,
    Ron,
    Json,
    Csv,
    Excel,
    Datasheet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameAssetSupport {
    pub filesystem: bool,
    pub pak_archives: bool,
    pub asset_catalog: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameDataAssetSourcePlan {
    SourceFiles,
    EngineCatalog,
    ShippingAssets(GameAssetSupport),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataTargetPlan {
    language: GameDataTargetLanguage,
    runtime: GameDataRuntimeProfile,
    products: Vec<GameDataProduct>,
    source: GameDataAssetSourcePlan,
    data_format: GameDataDataFormat,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameDataTargetPlanError {
    #[error(
        "unsupported GameData target {language:?}/{runtime:?}/{asset_source:?}/{data_format:?}"
    )]
    UnsupportedTarget {
        language: GameDataTargetLanguage,
        runtime: GameDataRuntimeProfile,
        asset_source: GameDataAssetSourcePlan,
        data_format: GameDataDataFormat,
    },
}

impl GameAssetSupport {
    #[must_use]
    pub const fn shipping_game_assets() -> Self {
        Self {
            filesystem: true,
            pak_archives: true,
            asset_catalog: true,
        }
    }

    #[must_use]
    pub const fn engine_asset_catalog() -> Self {
        Self {
            filesystem: false,
            pak_archives: false,
            asset_catalog: true,
        }
    }

    #[must_use]
    pub const fn extracted_filesystem() -> Self {
        Self {
            filesystem: true,
            pak_archives: false,
            asset_catalog: false,
        }
    }

    #[must_use]
    pub const fn is_self_contained(self) -> bool {
        self.filesystem && self.pak_archives && self.asset_catalog
    }
}

impl GameDataTargetLanguage {
    #[must_use]
    pub const fn default_standalone_data_format(self) -> GameDataDataFormat {
        GameDataDataFormat::Datasheet
    }

    #[must_use]
    pub const fn supports_data_format(self, format: GameDataDataFormat) -> bool {
        match self {
            Self::Rust => matches!(
                format,
                GameDataDataFormat::CookedTable
                    | GameDataDataFormat::Ron
                    | GameDataDataFormat::Datasheet
            ),
            Self::TypeScript | Self::Go => matches!(
                format,
                GameDataDataFormat::Json
                    | GameDataDataFormat::Csv
                    | GameDataDataFormat::Excel
                    | GameDataDataFormat::Datasheet
            ),
        }
    }
}

impl GameDataTargetPlan {
    #[must_use]
    pub fn repo_rust() -> Self {
        Self {
            language: GameDataTargetLanguage::Rust,
            runtime: GameDataRuntimeProfile::RepoIntegrated,
            products: default_products_for(
                GameDataTargetLanguage::Rust,
                GameDataRuntimeProfile::RepoIntegrated,
                GameDataDataFormat::CookedTable,
            ),
            source: GameDataAssetSourcePlan::EngineCatalog,
            data_format: GameDataDataFormat::CookedTable,
        }
    }

    #[must_use]
    pub fn standalone(language: GameDataTargetLanguage) -> Self {
        Self {
            language,
            runtime: GameDataRuntimeProfile::Standalone,
            products: default_products_for(
                language,
                GameDataRuntimeProfile::Standalone,
                language.default_standalone_data_format(),
            ),
            source: GameDataAssetSourcePlan::ShippingAssets(
                GameAssetSupport::shipping_game_assets(),
            ),
            data_format: language.default_standalone_data_format(),
        }
    }

    #[must_use]
    pub fn with_products(mut self, products: impl IntoIterator<Item = GameDataProduct>) -> Self {
        self.products = dedup_products(products);
        self
    }

    #[must_use]
    pub const fn language(&self) -> GameDataTargetLanguage {
        self.language
    }

    #[must_use]
    pub const fn runtime(&self) -> GameDataRuntimeProfile {
        self.runtime
    }

    #[must_use]
    pub fn products(&self) -> &[GameDataProduct] {
        &self.products
    }

    #[must_use]
    pub const fn source(&self) -> GameDataAssetSourcePlan {
        self.source
    }

    #[must_use]
    pub const fn data_format(&self) -> GameDataDataFormat {
        self.data_format
    }

    #[must_use]
    pub fn with_data_format(mut self, data_format: GameDataDataFormat) -> Self {
        self.data_format = data_format;
        self.products = default_products_for(self.language, self.runtime, data_format);
        self
    }

    #[must_use]
    pub fn with_source(mut self, source: GameDataAssetSourcePlan) -> Self {
        self.source = source;
        self
    }

    #[must_use]
    pub fn supports_product(&self, product: GameDataProduct) -> bool {
        self.products.contains(&product)
    }

    #[must_use]
    pub const fn is_self_contained(&self) -> bool {
        match self.source {
            GameDataAssetSourcePlan::SourceFiles => false,
            GameDataAssetSourcePlan::EngineCatalog => {
                matches!(self.runtime, GameDataRuntimeProfile::RepoIntegrated)
            }
            GameDataAssetSourcePlan::ShippingAssets(support) => support.is_self_contained(),
        }
    }

    #[must_use]
    pub fn supports_language(&self, language: GameDataTargetLanguage) -> bool {
        self.language == language && self.is_supported()
    }

    pub fn validate(&self) -> Result<(), GameDataTargetPlanError> {
        if self.is_supported() {
            Ok(())
        } else {
            Err(GameDataTargetPlanError::UnsupportedTarget {
                language: self.language,
                runtime: self.runtime,
                asset_source: self.source,
                data_format: self.data_format,
            })
        }
    }

    #[must_use]
    pub fn is_supported(&self) -> bool {
        if !self.language.supports_data_format(self.data_format)
            || !products_match_runtime(self.runtime, self.data_format, &self.products)
        {
            return false;
        }

        match (self.language, self.runtime, self.source) {
            (
                GameDataTargetLanguage::Rust,
                GameDataRuntimeProfile::RepoIntegrated,
                GameDataAssetSourcePlan::EngineCatalog,
            ) => self.data_format == GameDataDataFormat::CookedTable,
            (
                _,
                GameDataRuntimeProfile::Standalone,
                GameDataAssetSourcePlan::ShippingAssets(support),
            ) => support.is_self_contained(),
            _ => false,
        }
    }
}

fn dedup_products(products: impl IntoIterator<Item = GameDataProduct>) -> Vec<GameDataProduct> {
    let mut deduped = Vec::new();
    for product in products {
        if !deduped.contains(&product) {
            deduped.push(product);
        }
    }
    deduped
}

fn default_products_for(
    language: GameDataTargetLanguage,
    runtime: GameDataRuntimeProfile,
    _data_format: GameDataDataFormat,
) -> Vec<GameDataProduct> {
    match runtime {
        GameDataRuntimeProfile::RepoIntegrated => vec![
            GameDataProduct::CookedTableManifest,
            GameDataProduct::SemanticManagers,
        ],
        GameDataRuntimeProfile::Standalone => match language {
            GameDataTargetLanguage::Rust => vec![
                GameDataProduct::TableManifest,
                GameDataProduct::SemanticManagers,
                GameDataProduct::Systems,
                GameDataProduct::GameAssetAccess,
            ],
            GameDataTargetLanguage::TypeScript | GameDataTargetLanguage::Go => vec![
                GameDataProduct::TableManifest,
                GameDataProduct::SemanticManagers,
                GameDataProduct::Systems,
                GameDataProduct::GameAssetAccess,
            ],
        },
    }
}

fn products_match_runtime(
    runtime: GameDataRuntimeProfile,
    data_format: GameDataDataFormat,
    products: &[GameDataProduct],
) -> bool {
    match runtime {
        GameDataRuntimeProfile::RepoIntegrated => products.iter().all(|product| {
            matches!(
                product,
                GameDataProduct::CookedTableManifest | GameDataProduct::SemanticManagers
            )
        }),
        GameDataRuntimeProfile::Standalone => {
            data_format == GameDataDataFormat::Datasheet
                && products
                    .iter()
                    .all(|product| !matches!(product, GameDataProduct::CookedTableManifest))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_language_defaults_use_datasheet_products() {
        assert_eq!(
            GameDataTargetLanguage::Rust.default_standalone_data_format(),
            GameDataDataFormat::Datasheet
        );
        assert_eq!(
            GameDataTargetLanguage::TypeScript.default_standalone_data_format(),
            GameDataDataFormat::Datasheet
        );
        assert_eq!(
            GameDataTargetLanguage::Go.default_standalone_data_format(),
            GameDataDataFormat::Datasheet
        );
    }

    #[test]
    fn standalone_targets_are_datasheet_only() {
        assert!(GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust).is_supported());
        assert!(
            !GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)
                .with_data_format(GameDataDataFormat::CookedTable)
                .is_supported()
        );
        assert!(
            !GameDataTargetPlan::standalone(GameDataTargetLanguage::TypeScript)
                .with_data_format(GameDataDataFormat::Json)
                .is_supported()
        );
        assert!(
            !GameDataTargetPlan::standalone(GameDataTargetLanguage::Go)
                .with_data_format(GameDataDataFormat::Csv)
                .is_supported()
        );
    }

    #[test]
    fn target_support_is_centralized_for_language_emitters() {
        assert!(GameDataTargetPlan::repo_rust().is_supported());
        assert!(
            GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)
                .supports_language(GameDataTargetLanguage::Rust)
        );

        let typescript_csv = GameDataTargetPlan::standalone(GameDataTargetLanguage::TypeScript)
            .with_data_format(GameDataDataFormat::Csv);
        assert!(!typescript_csv.supports_language(GameDataTargetLanguage::TypeScript));
        assert!(!typescript_csv.supports_language(GameDataTargetLanguage::Rust));

        let go_datasheet = GameDataTargetPlan::standalone(GameDataTargetLanguage::Go);
        assert!(go_datasheet.supports_language(GameDataTargetLanguage::Go));
    }

    #[test]
    fn target_support_rejects_crossed_runtime_and_format_combinations() {
        let repo_typescript = GameDataTargetPlan {
            language: GameDataTargetLanguage::TypeScript,
            runtime: GameDataRuntimeProfile::RepoIntegrated,
            products: vec![GameDataProduct::TableManifest],
            source: GameDataAssetSourcePlan::SourceFiles,
            data_format: GameDataDataFormat::Json,
        };
        assert!(!repo_typescript.is_supported());

        let rust_json = GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)
            .with_data_format(GameDataDataFormat::Json);
        assert!(!rust_json.is_supported());

        let extracted_ts = GameDataTargetPlan::standalone(GameDataTargetLanguage::TypeScript)
            .with_source(GameDataAssetSourcePlan::ShippingAssets(
                GameAssetSupport::extracted_filesystem(),
            ));
        assert!(!extracted_ts.is_supported());
    }
}
