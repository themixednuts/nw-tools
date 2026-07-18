use crate::compiler::GameDataCompileUnit;
use crate::emit::{GameDataCodegenOutput, GameDataEmitter};
use crate::target::GameDataTargetLanguage;
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TypeScriptSourceEmitter;

impl GameDataEmitter for TypeScriptSourceEmitter {
    fn target_language(&self) -> GameDataTargetLanguage {
        GameDataTargetLanguage::TypeScript
    }

    fn emit(&self, unit: &GameDataCompileUnit) -> anyhow::Result<GameDataCodegenOutput> {
        let mut files = self
            .emit_standalone_project_with_options(
                &project::TypeScriptStandaloneProjectOptions::default(),
            )?
            .into_codegen_files();
        files.extend(managers::emit_dynamic_manager_files(unit)?);
        files.extend(crate::oodle_bundle::oodle_dynamic_runtime_files()?);
        Ok(GameDataCodegenOutput::new(self.target_language(), files))
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

    use super::*;
    use crate::compiler::GameDataCompiler;
    use crate::emit::GameDataEmitter;

    #[test]
    fn standalone_manager_output_emits_available_manager_contracts() {
        let catalog = GameSystemDataTables::default();
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let output = TypeScriptSourceEmitter
            .emit(&unit)
            .expect("typescript output");
        let managers = manager_sources(&output);
        let values = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/values.ts"))
            .expect("shared values")
            .contents();

        let public_managers = public_manager_class_names(&managers);
        assert_eq!(facade_manager_names(&managers), public_managers);
        assert!(!managers.contains("const MANAGERS"));
        assert!(!managers.contains("export const MANAGERS"));
        assert!(!managers.contains("export type ManagerDependencyKind"));
        assert!(!managers.contains("export type ManagerDependency"));
        assert!(!managers.contains("export interface ManagerDefinition"));
        assert!(!managers.contains("export function managerByName"));
        assert!(managers.contains("export class Managers"));
        assert!(managers.contains("export class ManagerLoadError extends Error"));
        assert!(managers.contains("static async open(loader: AssetLoader): Promise<Managers>"));
        assert!(managers.contains("player(): Promise<PlayerDataManager>"));
        assert!(managers.contains("private playerValue?: Promise<PlayerDataManager>"));
        assert!(!managers.contains("playerData():"));
        assert!(!managers.contains("static async load(loader: AssetLoader)"));
        assert!(!managers.contains("openManagers"));
        assert!(managers.contains("export interface Rows<Row> extends Iterable<Row>"));
        assert!(!managers.contains("PakDatasheetSource } from \"../game-assets/loader.js\""));
        assert!(managers.contains("player(): Promise<PlayerDataManager>"));
        assert!(managers.contains("armorOffset(): Promise<ArmorOffsetDataManager>"));
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
        ] {
            assert!(
                managers.contains(&format!("export class {manager}")),
                "{manager} should be emitted as a standalone product-backed manager"
            );
        }
        assert!(managers.contains("cache.resourcesForTables("));
        assert!(!managers.contains("cache.resources("));
        assert!(!managers.contains("ManagerDependency"));
        assert!(!managers.contains("buildManager"));
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
        assert!(
            managers.contains("private readonly playerBaseAttributesValue: PlayerBaseAttributes")
        );
        assert!(managers.contains("return this.playerBaseAttributesValue;"));
        assert!(!managers.contains("private readonly playerBaseAttributes: PlayerBaseAttributes"));
        assert!(managers.contains("maxPerks(rarityLevel: number): number | undefined"));
        assert!(managers.contains("categoricalProgressionId(tradeskill: TradeskillType)"));
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
        assert!(!managers.contains("column.fieldName"));
        assert!(!managers.contains("ManagerAssetFormat"));
        assert!(!managers.contains("format:"));
        assert!(!managers.contains("resource:"));
        assert!(!managers.contains("ManagerCacheShape"));
        assert!(!managers.contains("cacheShape"));
        assert!(!managers.contains("ProjectionTransform"));
        assert!(!managers.contains("transform:"));
        assert!(!managers.contains("duplicateKeyPolicy"));
        assert!(!managers.contains("sourceRowField"));
        assert!(values.contains("fromBytesLowercase(bytes: Uint8Array)"));
        assert!(!values.contains("fromBytes(bytes: Uint8Array, lowercaseAscii"));
    }

    #[test]
    fn source_format_managers_own_dynamic_schema_runtime() {
        let catalog = GameSystemDataTables::default();
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let output = TypeScriptSourceEmitter
            .emit(&unit)
            .expect("typescript output");
        let managers = manager_sources(&output);
        let datasheet_catalog = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers/datasheet-catalog.ts"))
            .expect("datasheet catalog")
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
        assert!(output.files().iter().all(|file| {
            file.path() != Path::new("bin/oo2core_win64.lib")
                && file.path() != Path::new("bin/oo2core_win64.dll")
        }));
        assert!(managers.contains("TableSchema"));
        assert!(!managers.contains("type ColumnSchema"));
        assert!(managers.contains("export interface Rows<Row> extends Iterable<Row>"));
        assert!(!managers.contains("const TABLE_SCHEMAS"));
        assert!(managers.contains("export class Managers"));
        assert!(managers.contains("class ManagerCache"));
        assert!(!managers.contains("PakDatasheetSource"));
        assert!(managers.contains("static async open("));
        assert!(managers.contains("this.loader.read(path).then("));
        assert!(managers.contains("const CREATE_MANAGER = Symbol(\"createManager\")"));
        assert!(managers.contains("static [CREATE_MANAGER](cache: ManagerCache)"));
        assert!(!managers.contains("static fromCache"));
        assert!(!managers.contains("(managers: Managers):"));
        assert!(managers.contains("rows(): IterableIterator<Row>"));
        assert!(managers.contains("export interface RowCollection<"));
        assert!(!managers.contains("readonly source: RowEntry<"));
        assert!(managers.contains("RowEntry<Table, Row>"));
        assert!(managers.contains("class RowCollectionImpl<"));
        assert!(managers.contains("RowCollection<Row, Table>"));
        assert!(managers.contains("private readonly entriesByTable"));
        let public_index = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/managers/index.ts"))
            .expect("manager public barrel")
            .contents();
        assert!(!public_index.contains("RowCollectionImpl"));
        assert!(!public_index.contains("TableRowsImpl"));
        assert!(!managers.contains("readonly rows: readonly Row[]"));
        assert!(!managers.contains("static fromPakSource"));
        assert!(managers.contains("parseDatasheet"));
        assert!(!managers.contains("readonly duplicateKeys"));
        assert!(!managers.contains("readonly rowsByLookupKey"));
        assert!(!managers.contains("managerInstance(name: string)"));
        assert!(!managers.contains("manager(name: string): ManagerInstance | undefined"));
        assert!(!managers.contains("private readonly managers"));
        assert!(managers.contains("cache.resourcesForTables("));
        assert!(!managers.contains("cache.resources("));
        assert!(!managers.contains("buildManager"));
        assert!(!managers.contains("managerByName"));
        assert!(!managers.contains("export function managerByName"));
        assert!(!managers.contains("export const MANAGERS"));
        assert!(!managers.contains("export interface ManagerDefinition"));
        assert!(!managers.contains("export type ManagerDependency"));
        assert!(!managers.contains("assetPaths(): readonly string[]"));
        assert!(!managers.contains("definition(): ManagerDefinition"));
        assert!(!managers.contains("rows(tableName?: string)"));
        assert!(!managers.contains("rowByKey("));
        assert!(!managers.contains("cellByKey("));
        assert!(datasheet_catalog.contains("readonly rowKey: boolean"));
        assert!(!datasheet_catalog.contains("DatasheetCellKind"));
        assert!(!datasheet_catalog.contains("readonly fieldName"));
        assert!(!datasheet_catalog.contains("readonly rowCount"));
        assert!(!managers.contains("declaredType"));
        assert!(!managers.contains("OptionalBool"));
        assert!(!managers.contains("ProjectionTransform"));
        assert!(!managers.contains("native"));
        assert!(!managers.contains("runtimeResource"));

        let pak = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("src/game-assets/loader.ts"))
            .expect("asset loader")
            .contents();
        assert!(pak.contains("export interface AssetLoaderOptions"));
        assert!(!pak.contains("@nw-tools/asset-loader/source"));
        assert!(pak.contains("realpath(assetRoot)"));
        assert!(pak.contains("canonicalPakPaths"));
        assert!(pak.contains("pak path ${pakPath} is outside asset root ${assetRoot}"));
        assert!(pak.contains("closeMountedArchives"));
        assert!(!pak.contains("export interface PakDatasheetSource"));
        assert!(!pak.contains("export interface BinaryAsset"));
        assert!(!pak.contains("export async function loadPakDatasheetSource"));
        assert!(pak.contains("static async open(assetRoot: string"));
        assert!(!pak.contains("static async fromDir"));
        assert!(!pak.contains("export function isManagerAssetPath"));
    }

    fn manager_sources(output: &GameDataCodegenOutput) -> String {
        output
            .files()
            .iter()
            .filter(|file| {
                file.path().parent() == Some(Path::new("src/managers"))
                    && file
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "ts")
            })
            .map(|file| file.contents())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn facade_manager_names(source: &str) -> BTreeSet<String> {
        const MARKER: &str = "[CREATE_MANAGER](";
        source
            .match_indices(MARKER)
            .filter_map(|(index, _)| {
                let name = source[..index]
                    .trim_end()
                    .rsplit(|character: char| {
                        !character.is_ascii_alphanumeric() && character != '_'
                    })
                    .next()?;
                name.ends_with("Manager").then(|| name.to_owned())
            })
            .collect()
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
