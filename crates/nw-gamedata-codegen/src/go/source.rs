use crate::compiler::GameDataCompileUnit;
use crate::emit::{GameDataCodegenOutput, GameDataEmitter};
use crate::target::GameDataTargetLanguage;
use thiserror::Error;
use treesitter_types_go::FromNode;

mod managers;
mod project;

pub use project::{GoStandaloneProject, GoStandaloneProjectFile, GoStandaloneProjectOptions};

#[derive(Debug, Error)]
pub enum GoSourceEmitError {
    #[error("format Go source: {0}")]
    Format(String),

    #[error("Go source syntax error: {0}")]
    Syntax(String),

    #[error("Go source was not UTF-8 after formatting: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GoSourceEmitter;

impl GameDataEmitter for GoSourceEmitter {
    fn target_language(&self) -> GameDataTargetLanguage {
        GameDataTargetLanguage::Go
    }

    fn emit(&self, unit: &GameDataCompileUnit) -> anyhow::Result<GameDataCodegenOutput> {
        let mut files = self
            .emit_standalone_project_with_options(&project::GoStandaloneProjectOptions::default())?
            .into_codegen_files();
        files.extend(managers::emit_dynamic_manager_files(unit)?);
        files.extend(crate::oodle_bundle::oodle_dynamic_runtime_files()?);
        Ok(GameDataCodegenOutput::new(self.target_language(), files))
    }
}

pub(crate) fn format_go_source(source: &str) -> Result<String, GoSourceEmitError> {
    validate_go_source(source)?;
    let mut child = std::process::Command::new("gofmt")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| GoSourceEmitError::Format(format!("start gofmt: {error}")))?;
    {
        use std::io::Write as _;
        child
            .stdin
            .take()
            .expect("gofmt stdin is piped")
            .write_all(source.as_bytes())
            .map_err(|error| GoSourceEmitError::Format(format!("write gofmt input: {error}")))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| GoSourceEmitError::Format(format!("wait for gofmt: {error}")))?;
    if !output.status.success() {
        return Err(GoSourceEmitError::Format(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let formatted = String::from_utf8(output.stdout)?;
    validate_go_source(&formatted)?;
    Ok(ensure_trailing_newline(&formatted))
}

fn ensure_trailing_newline(source: &str) -> String {
    if source.ends_with('\n') {
        source.to_owned()
    } else {
        format!("{source}\n")
    }
}

fn validate_go_source(source: &str) -> Result<(), GoSourceEmitError> {
    let mut parser = treesitter_types_go::tree_sitter::Parser::new();
    parser
        .set_language(&treesitter_types_go::tree_sitter_go::LANGUAGE.into())
        .map_err(|error| GoSourceEmitError::Syntax(error.to_string()))?;
    let tree = parser
        .parse(source.as_bytes(), None)
        .ok_or_else(|| GoSourceEmitError::Syntax("tree-sitter-go parse failed".to_owned()))?;
    let root = tree.root_node();
    treesitter_types_go::SourceFile::from_node(root, source.as_bytes())
        .map_err(|error| GoSourceEmitError::Syntax(error.to_string()))?;
    if root.has_error() {
        return Err(GoSourceEmitError::Syntax(first_go_parse_error(
            root,
            source.as_bytes(),
        )));
    }
    Ok(())
}

fn first_go_parse_error(node: treesitter_types_go::tree_sitter::Node<'_>, source: &[u8]) -> String {
    if node.is_error() || node.is_missing() {
        return format_go_parse_error(node, source);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_error() || child.is_missing() || child.has_error() {
            return first_go_parse_error(child, source);
        }
    }

    format_go_parse_error(node, source)
}

fn format_go_parse_error(
    node: treesitter_types_go::tree_sitter::Node<'_>,
    source: &[u8],
) -> String {
    let start = node.start_position();
    let end = node.end_position();
    let text = node.utf8_text(source).unwrap_or("");
    let context = std::str::from_utf8(source)
        .ok()
        .map(|source| source_line_context(source, start.row))
        .unwrap_or_default();
    format!(
        "{} at {}:{}..{}:{} `{}`{}",
        node.kind(),
        start.row + 1,
        start.column + 1,
        end.row + 1,
        end.column + 1,
        text,
        context
    )
}

fn source_line_context(source: &str, row: usize) -> String {
    let first = row.saturating_sub(2);
    let mut context = String::new();
    for (index, line) in source.lines().enumerate().skip(first).take(5) {
        context.push_str(&format!("\n{:>6}: {}", index + 1, line));
    }
    context
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
        let output = GoSourceEmitter.emit(&unit).expect("go output");
        let managers = manager_sources(&output);
        let values = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("types/types.go"))
            .expect("shared values")
            .contents();

        let public_managers = public_manager_type_names(&managers);
        assert_eq!(constructed_manager_type_names(&managers), public_managers);
        assert!(!managers.contains("managerDefinitions"));
        assert!(managers.contains("type Managers struct"));
        assert!(managers.contains("type Rows[T any] interface"));
        assert!(managers.contains("Rows() iter.Seq[T]"));
        assert!(managers.contains("type RowRef[TTable ~string, TRow any] struct"));
        assert!(managers.contains("type RowSlot[TTable ~string, TRow any] struct"));
        assert!(managers.contains("type RowSet[TTable ~string, TRow any] struct"));
        assert!(managers.contains("tableIndexes map[string]*rowTableIndex"));
        assert!(managers.contains("type rowTableIndex struct"));
        assert!(managers.contains("byKey"));
        assert!(managers.contains("type ManagerLoadError struct"));
        assert!(!managers.contains("Source *RowEntry["));
        assert!(!managers.contains("Row *RowEntry["));
        assert!(managers.contains("func New(loader *assets.AssetLoader) (*Managers, error)"));
        assert!(
            managers.contains("func (managers *Managers) Player() (*PlayerDataManager, error)")
        );
        assert!(managers.contains("playerOnce"));
        assert!(managers.contains("sync.Once"));
        assert!(!managers.contains("func Load(loader *assets.AssetLoader)"));
        assert!(managers.contains("func newManagerCache(loader *assets.AssetLoader"));
        assert!(!managers.contains("assetSourceFromLoader"));
        assert!(!managers.contains("loader.DatasheetSource()"));
        assert!(
            managers.contains("func (managers *Managers) Player() (*PlayerDataManager, error)")
        );
        assert!(
            managers.contains(
                "func (managers *Managers) ArmorOffset() (*ArmorOffsetDataManager, error)"
            )
        );
        assert!(!managers.contains("var Managers"));
        assert!(!managers.contains("type ManagerDefinition"));
        assert!(!managers.contains("type ManagerDependency "));
        assert!(!managers.contains("type ManagerDependencyKind"));
        assert!(!managers.contains("func ManagerByName"));
        assert!(!managers.contains("type ManagerRuntime struct"));
        assert!(managers.contains("type ArmorOffsetDataManager struct"));
        assert!(managers.contains("ArmorOffsetDataManager"));
        assert!(managers.contains("PlayerDataManager"));
        for manager in [
            "CameraSettingsDataManager",
            "ArmorOffsetDataManager",
            "EquipTypesDataManager",
            "GameDebugSettingsManager",
            "UIDataManager",
            "PlayerDataManager",
        ] {
            assert!(
                managers.contains(&format!("type {manager} struct")),
                "{manager} should be emitted as a standalone product-backed manager"
            );
        }
        assert!(managers.contains("cache.resourcesForTables("));
        assert!(!managers.contains("cache.resources("));
        assert!(!managers.contains("managerDependency"));
        assert!(!managers.contains("buildManager"));
        assert!(!managers.contains("ProductPath"));
        assert!(!managers.contains(".aztbl"));
        assert!(managers.contains("sharedassets/genericassets/items/armoroffsets.aoffdb"));
        assert!(managers.contains("parseArmorOffsetDatabase"));
        assert!(!managers.contains("func ParseArmorOffsetDatabase"));
        assert!(managers.contains("func (manager *ArmorOffsetDataManager) Database"));
        assert!(managers.contains("func (manager *ArmorOffsetDataManager) ArmorOffset"));
        assert!(
            managers.contains("func (manager *ArmorOffsetDataManager) FurthestAttachmentOffset")
        );
        assert!(managers.contains("func (manager *CameraSettingsDataManager) Settings"));
        assert!(managers.contains("func (manager *CameraSettingsDataManager) CameraStates"));
        assert!(managers.contains("func (manager *EquipTypesDataManager) Database"));
        assert!(managers.contains("func (manager *EquipTypesDataManager) EquipTypes"));
        assert!(managers.contains("func (manager *GameDebugSettingsManager) Settings"));
        assert!(
            managers.contains("func (manager *GameDebugSettingsManager) DisabledCombatToggleCount")
        );
        assert!(managers.contains("func (manager *UIDataManager) Database"));
        assert!(managers.contains("func (manager *UIDataManager) InteractOptions"));
        assert!(managers.contains("func (manager *PlayerDataManager) PlayerBaseAttributes"));
        assert!(managers.contains("func (manager *PlayerDataManager) MaxPerks"));
        assert!(managers.contains("func (manager *PlayerDataManager) CategoricalProgressionID"));
        assert!(!managers.contains("type ObjectiveTasksDataManager struct"));
        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("products/products.go"))
        );
        assert!(!managers.contains("type AssetID ="));
        assert!(managers.contains("gametypes.AssetID"));
        assert!(!managers.contains("ProductAssetID"));
        assert!(!managers.contains("ArmorOffsetDatabaseAsset"));
        assert!(!managers.contains("ManagerImplementation"));
        assert!(!managers.contains("Implementation:"));
        assert!(!managers.contains("RuntimeResource"));
        assert!(!managers.contains("func (runtime *managerCache)"));
        assert!(!managers.contains("managerInstance"));
        assert!(managers.contains("type managerResources struct"));
        assert!(values.contains("func CRC32FromBytesLowercase(bytes []byte) CRC32"));
        assert!(!values.contains("func NewCRC32"));
        assert!(!values.contains("func CRC32FromBytes(bytes []byte, lowercaseASCII bool)"));
        assert!(!managers.contains("Generated"));
        assert!(!managers.contains("ManagerInputProduct"));
        assert!(!managers.contains("ManagerInputDatasheet"));
        assert!(!managers.contains("ManagerAssetFormat"));
        assert!(!managers.contains("ProductFormat"));
        assert!(!managers.contains("Format:"));
        assert!(!managers.contains("Inputs:"));
        assert!(!managers.contains("Resource:"));
        assert!(!managers.contains("ManagerCacheShape"));
        assert!(!managers.contains("CacheShape"));
        assert!(!managers.contains("ProjectionTransform"));
        assert!(!managers.contains("Transform:"));
        assert!(!managers.contains("DuplicateKeyPolicy"));
        assert!(!managers.contains("SourceRowField"));
    }

    #[test]
    fn source_format_managers_own_dynamic_schema_runtime() {
        let catalog = GameSystemDataTables::default();
        let unit = GameDataCompiler::source_format().compile_unit(&catalog);
        let output = GoSourceEmitter.emit(&unit).expect("go output");
        let managers = manager_sources(&output);

        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("tables/tables.go"))
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
        assert!(!managers.contains("type DatasheetCellKind string"));
        assert!(managers.contains("type tableSchema struct"));
        assert!(managers.contains("type columnSchema struct"));
        assert!(!managers.contains("type TableSchema struct"));
        assert!(!managers.contains("type ColumnSchema struct"));
        assert!(managers.contains("type Rows[T any] interface"));
        assert!(managers.contains("Rows() iter.Seq[T]"));
        assert!(!managers.contains("var tableSchemas = []tableSchema"));
        assert!(managers.contains("type managerCache struct"));
        assert!(!managers.contains("NewManagerRuntimeFromPakSource"));
        assert!(!managers.contains("type assetSource struct"));
        assert!(managers.contains("loader.Read(path)"));
        assert!(managers.contains("gameassets.ParseDatasheet"));
        assert!(managers.contains("type dynamicTable struct"));
        assert!(!managers.contains("DuplicateKeys"));
        assert!(!managers.contains("map[string][]dynamicTableRow"));
        assert!(!managers.contains("RowsByLookupKey map[string]dynamicTableRow"));
        assert!(!managers.contains("type DynamicTable struct"));
        assert!(!managers.contains("type DynamicTableRow struct"));
        assert!(!managers.contains("type ManagerInstance struct"));
        assert!(!managers.contains("func (instance *managerInstance) Manager"));
        assert!(!managers.contains("func (instance *managerInstance) manager"));
        assert!(!managers.contains("managers map[string]*managerInstance"));
        assert!(managers.contains("cache.resourcesForTables("));
        assert!(!managers.contains("cache.resources("));
        assert!(!managers.contains("buildManager"));
        assert!(!managers.contains("managerByName"));
        assert!(!managers.contains("func (instance *managerInstance) AssetPaths"));
        assert!(!managers.contains("func (manager *ArmorOffsetDataManager) Definition"));
        assert!(!managers.contains("func ManagerByName"));
        assert!(!managers.contains("var Managers"));
        assert!(!managers.contains("type ManagerDefinition"));
        assert!(!managers.contains("type ManagerDependency "));
        assert!(!managers.contains("type ManagerDependencyKind"));
        assert!(!managers.contains("func (instance *managerInstance) Rows"));
        assert!(!managers.contains("func (instance *managerInstance) RowByKey"));
        assert!(!managers.contains("func (instance *managerInstance) CellByKey"));
        assert!(!managers.contains("DatasheetCellKind"));
        assert!(!managers.contains("FieldName string"));
        assert!(!managers.contains("column.FieldName"));
        assert!(!managers.contains("RowCount"));
        assert!(!managers.contains("DeclaredType"));
        assert!(!managers.contains("OptionalBool"));
        assert!(!managers.contains("ProjectionTransform"));
        assert!(!managers.contains("Native"));
        assert!(!managers.contains("RuntimeResource"));
        assert!(managers.contains("values := make([]gametypes.CRC32, 0, len(child.Children))"));
        assert!(managers.contains("return gametypes.CRC32(raw), err"));

        let pak = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("internal/gameassets/pak.go"))
            .expect("pak asset loader")
            .contents();
        assert!(pak.contains("filepath.EvalSymlinks"));
        assert!(pak.contains("pak path %s is outside asset root %s"));
        assert!(pak.contains("errors.Join(err, closeErr)"));
        assert!(!pak.contains("type PakDatasheetSource struct"));
        assert!(!pak.contains("func (loader *AssetLoader) DatasheetSource"));
        assert!(!pak.contains("func LoadPakDatasheetSource"));

        assert!(pak.contains("func Open(assetRoot string)"));
        assert!(!pak.contains("func OpenAssetLoader"));
    }

    fn constructed_manager_type_names(source: &str) -> BTreeSet<String> {
        const PREFIX: &str = "func new";
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

    fn manager_sources(output: &GameDataCodegenOutput) -> String {
        output
            .files()
            .iter()
            .filter(|file| {
                file.path().parent() == Some(Path::new("managers"))
                    && file
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "go")
            })
            .map(|file| file.contents())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn public_manager_type_names(source: &str) -> BTreeSet<String> {
        const PREFIX: &str = "type ";
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
