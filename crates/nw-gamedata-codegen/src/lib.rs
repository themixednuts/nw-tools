//! GameData compiler primitives for New World tooling.
//!
//! This crate owns the pipeline from New World's game-system datasheets
//! to standalone Rust, TypeScript, and Go schema/manager projects.

pub mod asset_access;
pub mod compiler;
pub mod emit;
pub mod game_system_schema;
pub mod go;
pub mod manager;
mod manager_records;
mod naming;
pub mod native;
mod oodle_bundle;
pub mod plan;
pub mod project;
pub mod rust;
pub mod schema;
pub mod source;
pub mod symbols;
pub mod system;
mod table;
pub mod target;
pub mod typescript;

pub use asset_access::{
    GameAssetAccessCodegenPlan, GameAssetRuntimeModule, GameAssetRuntimeModuleKind,
    GameAssetRuntimePlan,
};
pub use compiler::{GameDataCompileUnit, GameDataCompiler, GameDataCompilerOptions};
pub use emit::{
    GameDataCodegenFile, GameDataCodegenOutput, GameDataEmitter, GameDataEmitterConfigError,
};
pub use go::source::GoSourceEmitter;
pub use manager::{
    ManagerCodegenFile, ManagerCodegenOutput, ManagerCodegenPlan, ManagerEmissionContext,
    ManagerEmitter, NativeComposedResourceArgument, NativeComposedResourceManager,
    NativeCostumeAudioSlot, NativeCrcHashPolicy, NativeCrcIndexLookupMethod,
    NativeCrcIndexLookupParameter, NativeCrcIndexLookupParameterKind,
    NativeCrcProjectionDependencyLookupMethod, NativeCrcProjectionFieldLookupMethod,
    NativeCrcProjectionRowFilter, NativeCrcProjectionRowFilterPredicate,
    NativeCrcProjectionSecondaryIndex, NativeCurrencyExchangeMappingManager,
    NativeDuplicateKeyPolicy, NativeEnumLookupMethod, NativeEnumLookupParameterKind,
    NativeManagerInput, NativeManagerProductFormat, NativeManagerProductInput, NativeManagerShape,
    NativeManagerShapeError, NativeManagerSpec, NativeManagerTableInput, NativeNumericKeyType,
    NativeNumericLookupMethod, NativeNumericLookupParameterKind,
    NativeOneTableCostumeChangeManager, NativeOneTableCrcIndexManager,
    NativeOneTableCrcKeyProjectionManager, NativeOneTableCrestPartManager,
    NativeOneTableDarknessManager, NativeOneTableDifficultyScalingManager,
    NativeOneTableDungeonTileManager, NativeOneTableEncumbranceManager,
    NativeOneTableEnumKeyProjectionManager, NativeOneTableLevelDisparityManager,
    NativeOneTableNumericKeyProjectionManager, NativeOneTableOwnedStringCrcIndexManager,
    NativeOneTableParticleDataManager, NativeOneTablePvpBalanceManager,
    NativeOneTableStringKeyProjectionManager, NativePlayerDataManager, NativeProductAssetResource,
    NativeProductAssetResourceManager, NativeProjectionField, NativeProjectionTransform,
    NativeSecondaryIndexKeyType, NativeSecondaryIndexLookupMethod,
    NativeSecondaryIndexLookupParameterKind, NativeSourceHandleMethod,
    NativeStaticTradeskillRankDataMappingManager, NativeStringLookupMethod,
    NativeStringLookupParameterKind, NativeStringLookupTarget, NativeTableFamilyCrcIndexManager,
    NativeTableFamilyCrcKeyProjectionManager, NativeTableFamilyNumericKeyProjectionManager,
    NativeTableFamilyOwnedStringCrcIndexManager, NativeTableFamilyTable,
    validated_native_manager_plan, validated_native_manager_plan_for_schema,
    validated_native_manager_specs,
};
pub use native::{
    NativeClassSpec, NativeClassSpecError, NativeCodegenFile, NativeCodegenOutput,
    NativeCodegenPlan,
};
pub use plan::{GameDataCodegenPlan, TableCodegenPlan};
pub use project::{
    GO_TOOLCHAIN, GO_VERSION, GameDataBuildTool, GameDataPackageDependency,
    GameDataPackageDependencyRole, GameDataToolchainPlan, RUST_EDITION, RUST_VERSION,
    StandaloneProjectCodegenPlan, StandaloneProjectFileKind, StandaloneProjectFilePlan,
    StandaloneProjectPlan, TYPESCRIPT_NODE_TYPES, TYPESCRIPT_PACKAGE_MANAGER, TYPESCRIPT_VERSION,
    VITE_PLUS_CORE_OVERRIDE, VITE_PLUS_TEST_OVERRIDE, VITE_PLUS_VERSION,
};
pub use rust::source::RustSourceEmitter;
pub use schema::{GameDataCompileMode, schema_report_for_mode};
pub use source::{
    FilesystemDatasheetGameDataSource, GameDataSourceProvider, PakDatasheetGameDataSource,
    load_catalog_from_asset_root,
};
pub use symbols::{
    GameAssetPath, GameDataRowTypeName, GameDataTableName, GhidraClassPath, GhidraFunctionPath,
    RustIdentifier, RustPath, RustTypePath, SymbolNameError,
};
pub use system::{
    NativeSystemSpec, SystemCodegenFile, SystemCodegenOutput, SystemCodegenPlan, SystemEmitter,
};
pub use table::GameDataTableSourceFormat;
pub use target::{
    GameAssetSupport, GameDataAssetSourcePlan, GameDataDataFormat, GameDataProduct,
    GameDataRuntimeProfile, GameDataTargetLanguage, GameDataTargetPlan, GameDataTargetPlanError,
};
pub use typescript::source::TypeScriptSourceEmitter;
