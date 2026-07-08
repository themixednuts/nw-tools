use crate::compiler::GameDataCompileUnit;
use crate::emit::{
    GameDataCodegenFile, GameDataCodegenOutput, GameDataEmitter, GameDataEmitterConfigError,
};
use crate::target::{
    GameDataProduct, GameDataRuntimeProfile, GameDataTargetLanguage, GameDataTargetPlan,
};
use thiserror::Error;

mod managers;
mod project;

pub use project::{
    TypeScriptStandaloneProject, TypeScriptStandaloneProjectFile,
    TypeScriptStandaloneProjectOptions,
};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TypeScriptSourceEmitError {
    #[error("TypeScript syntax error: {0}")]
    Syntax(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptSourceEmitter {
    target: GameDataTargetPlan,
}

impl TypeScriptSourceEmitter {
    pub fn new(target: GameDataTargetPlan) -> Result<Self, GameDataEmitterConfigError> {
        if Self::target_is_supported(&target) {
            Ok(Self { target })
        } else {
            Err(GameDataEmitterConfigError::unsupported(
                GameDataTargetLanguage::TypeScript,
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
        GameDataTargetPlan::standalone(GameDataTargetLanguage::TypeScript)
    }

    #[must_use]
    pub fn target_is_supported(target: &GameDataTargetPlan) -> bool {
        target.supports_language(GameDataTargetLanguage::TypeScript)
    }
}

impl Default for TypeScriptSourceEmitter {
    fn default() -> Self {
        Self::standalone()
    }
}

impl GameDataEmitter for TypeScriptSourceEmitter {
    fn target(&self) -> GameDataTargetPlan {
        self.target.clone()
    }

    fn emit(&self, unit: &GameDataCompileUnit) -> anyhow::Result<GameDataCodegenOutput> {
        let mut files = if matches!(self.target.runtime(), GameDataRuntimeProfile::Standalone) {
            self.emit_standalone_project_with_options(
                &project::TypeScriptStandaloneProjectOptions::default()
                    .with_product_placeholders(false),
            )?
            .into_codegen_files()
        } else {
            Vec::new()
        };
        if matches!(self.target.runtime(), GameDataRuntimeProfile::Standalone)
            && self
                .target
                .supports_product(GameDataProduct::SemanticManagers)
        {
            if self
                .target
                .supports_product(GameDataProduct::GameAssetAccess)
            {
                files.extend(managers::emit_dynamic_manager_files(unit)?);
            } else {
                files.extend(managers::emit_manager_files(unit)?);
            }
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
                "src/systems/index.ts",
                "export {};\n",
            ));
        }
        Ok(GameDataCodegenOutput::new(self.target(), files))
    }
}

pub(crate) fn format_typescript_source(source: &str) -> Result<String, TypeScriptSourceEmitError> {
    let source = source.trim();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::ts()).parse();
    if !parsed.errors.is_empty() {
        return Err(TypeScriptSourceEmitError::Syntax(
            parsed
                .errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }

    let options = oxc_codegen::CodegenOptions {
        indent_char: oxc_codegen::IndentChar::Space,
        indent_width: 2,
        ..Default::default()
    };
    let code = oxc_codegen::Codegen::new()
        .with_options(options)
        .with_source_text(source)
        .build(&parsed.program)
        .code;

    Ok(ensure_final_newline(code.trim_end()))
}

fn ensure_final_newline(source: &str) -> String {
    if source.ends_with('\n') {
        source.to_owned()
    } else {
        format!("{source}\n")
    }
}

pub(crate) fn typescript_string_literal(value: &str) -> String {
    format!("{value:?}")
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, path::Path};

    use nw_datasheet::game_system::GameSystemDataTables;

    use crate::compiler::GameDataCompiler;
    use crate::emit::GameDataEmitter;
    use crate::target::GameDataDataFormat;

    use super::*;

    #[test]
    fn standalone_manager_output_emits_all_manager_contracts() {
        let catalog = GameSystemDataTables::default();
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let output = TypeScriptSourceEmitter::standalone()
            .emit(&unit)
            .expect("typescript output");
        let managers = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers/index.ts"))
            .expect("manager manifest")
            .contents();

        let manager_definitions = manager_definition_names(managers);
        let public_managers = public_manager_class_names(managers);
        assert_eq!(manager_definitions, public_managers);
        assert!(managers.contains("const MANAGERS"));
        assert!(!managers.contains("export const MANAGERS"));
        assert!(!managers.contains("export type ManagerDependencyKind"));
        assert!(!managers.contains("export type ManagerDependency"));
        assert!(!managers.contains("export interface ManagerDefinition"));
        assert!(!managers.contains("export function managerByName"));
        assert!(managers.contains("export class Managers"));
        assert!(managers.contains("static async open(loader: AssetLoader): Promise<Managers>"));
        assert!(!managers.contains("PakDatasheetSource } from \"../game-assets/pak.js\""));
        assert!(managers.contains("playerData(): PlayerDataManager"));
        assert!(managers.contains("armorOffsetData(): ArmorOffsetDataManager"));
        assert!(!managers.contains("export class ManagerRuntime"));
        assert!(managers.contains("export class ArmorOffsetDataManager"));
        assert!(managers.contains("ArmorOffsetDataManager"));
        assert!(managers.contains("PlayerDataManager"));
        for manager in [
            "CameraSettingsDataManager",
            "ArmorOffsetDataManager",
            "EquipTypesDataManager",
            "GameDebugSettingsManager",
            "UiDataManager",
            "PlayerDataManager",
            "SocialDataManager",
        ] {
            assert!(
                managers.contains(&format!("export class {manager}")),
                "{manager} should be emitted as a standalone product-backed manager"
            );
        }
        assert!(managers.contains("kind: \"asset\""));
        assert!(managers.contains("kind: \"table\""));
        assert!(!managers.contains("productPath"));
        assert!(!managers.contains(".aztbl"));
        assert!(managers.contains("sharedassets/genericassets/items/armoroffsets.aoffdb"));
        assert!(managers.contains("parseArmorOffsetDatabase"));
        assert!(managers.contains("armorOffset(name: string)"));
        assert!(managers.contains("furthestAttachmentOffset("));
        assert!(managers.contains("database(): ArmorOffsetDatabase"));
        assert!(managers.contains("settings(): GameCameraSettings"));
        assert!(managers.contains("cameraStates(): readonly CameraStateSettings[]"));
        assert!(managers.contains("database(): EquipTypesDatabase"));
        assert!(managers.contains("equipTypes(): readonly EquipTypeData[]"));
        assert!(managers.contains("settings(): GameDebugSettings"));
        assert!(managers.contains("disabledCombatToggleCount(): number"));
        assert!(managers.contains("database(): UiDatabase"));
        assert!(managers.contains("interactOptions(): readonly InteractOptionData[]"));
        assert!(managers.contains("playerBaseAttributes(): PlayerBaseAttributes"));
        assert!(managers.contains("maxPerks(rarityLevel: number): number | undefined"));
        assert!(managers.contains("categoricalProgressionId(tradeskill: string | number)"));
        assert!(managers.contains("rankDatabase(): SocialRankDatabase"));
        assert!(managers.contains("ranks(): readonly SocialRankData[]"));
        assert!(!managers.contains("export class ObjectiveTasksDataManager"));
        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("src/products/index.ts"))
        );
        assert!(!managers.contains("assetId"));
        assert!(!managers.contains("productAssetId"));
        assert!(!managers.contains("ManagerImplementation"));
        assert!(!managers.contains("implementation:"));
        assert!(!managers.contains("runtimeResource"));
        assert!(!managers.contains("generated"));
        assert!(!managers.contains("ManagerAssetFormat"));
        assert!(!managers.contains("format:"));
        assert!(!managers.contains("resource:"));
        assert!(!managers.contains("ManagerCacheShape"));
        assert!(!managers.contains("cacheShape"));
        assert!(!managers.contains("ProjectionTransform"));
        assert!(!managers.contains("transform:"));
        assert!(!managers.contains("duplicateKeyPolicy"));
        assert!(!managers.contains("sourceRowField"));
    }

    #[test]
    fn source_format_managers_own_dynamic_schema_runtime() {
        let catalog = GameSystemDataTables::default();
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let target = TypeScriptSourceEmitter::standalone_target()
            .with_data_format(GameDataDataFormat::Datasheet);
        let output = TypeScriptSourceEmitter::new(target)
            .expect("typescript datasheet emitter")
            .emit(&unit)
            .expect("typescript output");
        let managers = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers/index.ts"))
            .expect("manager runtime")
            .contents();

        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("src/tables/index.ts"))
        );
        assert!(
            output
                .files()
                .iter()
                .any(|file| file.path() == Path::new("bin/oo2core_9_win64.dll"))
        );
        assert!(managers.contains("export interface TableSchema"));
        assert!(managers.contains("export interface ColumnSchema"));
        assert!(managers.contains("export const TABLE_SCHEMAS: readonly TableSchema[]"));
        assert!(managers.contains("export class Managers"));
        assert!(managers.contains("class ManagerCache"));
        assert!(managers.contains("interface PakDatasheetSource"));
        assert!(!managers.contains("export interface PakDatasheetSource"));
        assert!(managers.contains("const MANAGER_INSTANCE = Symbol(\"managerInstance\")"));
        assert!(!managers.contains("static fromPakSource"));
        assert!(managers.contains("parseDatasheet"));
        assert!(managers.contains("readonly duplicateKeys"));
        assert!(managers.contains("readonly rowsByLookupKey"));
        assert!(!managers.contains("managerInstance(name: string)"));
        assert!(!managers.contains("manager(name: string)"));
        assert!(!managers.contains("export function managerByName"));
        assert!(!managers.contains("export const MANAGERS"));
        assert!(!managers.contains("export interface ManagerDefinition"));
        assert!(!managers.contains("export type ManagerDependency"));
        assert!(!managers.contains("assetPaths(): readonly string[]"));
        assert!(!managers.contains("definition(): ManagerDefinition"));
        assert!(!managers.contains("rows(tableName?: string)"));
        assert!(!managers.contains("rowByKey("));
        assert!(!managers.contains("cellByKey("));
        assert!(managers.contains("readonly kind: DatasheetCellKind"));
        assert!(!managers.contains("private readonly entries"));
        assert!(!managers.contains("declaredType"));
        assert!(!managers.contains("OptionalBool"));
        assert!(!managers.contains("ProjectionTransform"));
        assert!(!managers.contains("native"));
        assert!(!managers.contains("runtimeResource"));

        let pak = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/game-assets/pak.ts"))
            .expect("pak asset loader")
            .contents();
        assert!(pak.contains("export interface AssetLoaderOptions"));
        assert!(pak.contains("Symbol.for(\"@nw-tools/asset-loader/source\")"));
        assert!(pak.contains("realpath(assetRoot)"));
        assert!(pak.contains("canonicalPakPaths"));
        assert!(pak.contains("pak path ${pakPath} is outside asset root ${assetRoot}"));
        assert!(pak.contains("closeMountedArchives"));
        assert!(!pak.contains("export interface PakDatasheetSource"));
        assert!(!pak.contains("export interface BinaryAsset"));
        assert!(!pak.contains("export async function loadPakDatasheetSource"));

        let filesystem = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/game-assets/filesystem.ts"))
            .expect("filesystem asset loader")
            .contents();
        assert!(filesystem.contains("realpath(root)"));
        assert!(filesystem.contains("datasheet path ${path} is outside root ${root}"));
    }

    fn manager_definition_names(source: &str) -> BTreeSet<String> {
        const MANAGERS_PREFIX: &str = "const MANAGERS: readonly ManagerDefinition[] = [";
        let Some((_, managers)) = source.split_once(MANAGERS_PREFIX) else {
            return BTreeSet::new();
        };
        let managers = managers
            .split_once("];")
            .map_or(managers, |(block, _)| block);
        let mut depth = 0usize;
        let mut names = BTreeSet::new();
        const NAME_PREFIX: &str = "name: \"";
        for line in managers.lines() {
            let trimmed = line.trim();
            depth = depth.saturating_add(trimmed.matches('{').count());
            if depth == 1 {
                if let Some(rest) = trimmed.strip_prefix(NAME_PREFIX) {
                    if let Some(end) = rest.find('"') {
                        let name = &rest[..end];
                        if name.ends_with("Manager") {
                            names.insert(name.to_owned());
                        }
                    }
                }
            }
            depth = depth.saturating_sub(trimmed.matches('}').count());
        }
        names
    }

    fn public_manager_class_names(source: &str) -> BTreeSet<String> {
        const PREFIX: &str = "export class ";
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
