use crate::compiler::GameDataCompileUnit;
use crate::emit::{
    GameDataCodegenFile, GameDataCodegenOutput, GameDataEmitter, GameDataEmitterConfigError,
};
use crate::target::{
    GameDataProduct, GameDataRuntimeProfile, GameDataTargetLanguage, GameDataTargetPlan,
};
use thiserror::Error;
use treesitter_types_go::FromNode;

mod managers;
mod project;

pub use project::{GoStandaloneProject, GoStandaloneProjectFile, GoStandaloneProjectOptions};

#[derive(Debug, Error)]
pub enum GoSourceEmitError {
    #[error("invalid Go package name `{package_name}`")]
    PackageName { package_name: String },

    #[error("format Go source: {0}")]
    Format(String),

    #[error("Go source syntax error: {0}")]
    Syntax(String),

    #[error("Go source was not UTF-8 after formatting: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSourceEmitter {
    target: GameDataTargetPlan,
}

impl GoSourceEmitter {
    pub fn new(target: GameDataTargetPlan) -> Result<Self, GameDataEmitterConfigError> {
        if Self::target_is_supported(&target) {
            Ok(Self { target })
        } else {
            Err(GameDataEmitterConfigError::unsupported(
                GameDataTargetLanguage::Go,
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
        GameDataTargetPlan::standalone(GameDataTargetLanguage::Go)
    }

    #[must_use]
    pub fn target_is_supported(target: &GameDataTargetPlan) -> bool {
        target.supports_language(GameDataTargetLanguage::Go)
    }
}

impl Default for GoSourceEmitter {
    fn default() -> Self {
        Self::standalone()
    }
}

impl GameDataEmitter for GoSourceEmitter {
    fn target(&self) -> GameDataTargetPlan {
        self.target.clone()
    }

    fn emit(&self, unit: &GameDataCompileUnit) -> anyhow::Result<GameDataCodegenOutput> {
        let mut files = if matches!(self.target.runtime(), GameDataRuntimeProfile::Standalone) {
            self.emit_standalone_project_with_options(
                &project::GoStandaloneProjectOptions::default().with_product_placeholders(false),
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
                "systems/systems.go",
                format_go_source("package systems\n")?,
            ));
        }
        Ok(GameDataCodegenOutput::new(self.target(), files))
    }
}

pub(crate) fn format_go_source(source: &str) -> Result<String, GoSourceEmitError> {
    const GOFMT_SOURCE_LIMIT: usize = 128 * 1024;

    if source.len() > GOFMT_SOURCE_LIMIT {
        validate_go_source(source)?;
        return Ok(source.to_owned());
    }

    let formatted = match std::panic::catch_unwind(|| gofmt::formatter::format(source)) {
        Ok(Ok(bytes)) => String::from_utf8(bytes)?,
        Ok(Err(error)) => return Err(GoSourceEmitError::Format(error.to_string())),
        Err(_) => source.to_owned(),
    };
    validate_go_source(&formatted)?;
    Ok(formatted)
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

pub(crate) fn is_go_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && !is_go_keyword(value)
}

fn is_go_keyword(value: &str) -> bool {
    matches!(
        value,
        "break"
            | "default"
            | "func"
            | "interface"
            | "select"
            | "case"
            | "defer"
            | "go"
            | "map"
            | "struct"
            | "chan"
            | "else"
            | "goto"
            | "package"
            | "switch"
            | "const"
            | "fallthrough"
            | "if"
            | "range"
            | "type"
            | "continue"
            | "for"
            | "import"
            | "return"
            | "var"
    )
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
        let output = GoSourceEmitter::standalone()
            .emit(&unit)
            .expect("go output");
        let managers = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("managers/managers.go"))
            .expect("manager manifest")
            .contents();

        let manager_definitions = manager_definition_names(managers);
        let public_managers = public_manager_type_names(managers);
        assert_eq!(manager_definitions, public_managers);
        assert!(managers.contains("var managers = []managerDefinition"));
        assert!(!managers.contains("var Managers"));
        assert!(!managers.contains("type ManagerDefinition"));
        assert!(!managers.contains("type ManagerDependency "));
        assert!(!managers.contains("type ManagerDependencyKind"));
        assert!(!managers.contains("func ManagerByName"));
        assert!(managers.contains("type ManagerRuntime struct"));
        assert!(managers.contains("type ArmorOffsetDataManager struct"));
        assert!(managers.contains("ArmorOffsetDataManager"));
        assert!(managers.contains("PlayerDataManager"));
        assert!(managers.contains("Kind: managerDependencyAsset"));
        assert!(managers.contains("managerDependencyTable"));
        assert!(!managers.contains("ProductPath"));
        assert!(!managers.contains(".aztbl"));
        assert!(managers.contains("sharedassets/genericassets/items/armoroffsets.aoffdb"));
        assert!(managers.contains("ParseArmorOffsetDatabase"));
        assert!(managers.contains("func (manager *ArmorOffsetDataManager) Database"));
        assert!(managers.contains("func (manager *ArmorOffsetDataManager) ArmorOffset"));
        assert!(
            managers.contains("func (manager *ArmorOffsetDataManager) FurthestAttachmentOffset")
        );
        assert!(!managers.contains("type ObjectiveTasksDataManager struct"));
        assert!(
            output
                .files()
                .iter()
                .all(|file| file.path() != Path::new("products/products.go"))
        );
        assert!(!managers.contains("AssetID"));
        assert!(!managers.contains("ProductAssetID"));
        assert!(!managers.contains("ArmorOffsetDatabaseAsset"));
        assert!(!managers.contains("ManagerImplementation"));
        assert!(!managers.contains("Implementation:"));
        assert!(!managers.contains("RuntimeResource"));
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
        let target =
            GoSourceEmitter::standalone_target().with_data_format(GameDataDataFormat::Datasheet);
        let output = GoSourceEmitter::new(target)
            .expect("go datasheet emitter")
            .emit(&unit)
            .expect("go output");
        let managers = output
            .files()
            .iter()
            .find(|file| file.path() == Path::new("managers/managers.go"))
            .expect("manager runtime")
            .contents();

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
        assert!(managers.contains("type DatasheetCellKind string"));
        assert!(managers.contains("type TableSchema struct"));
        assert!(managers.contains("type ColumnSchema struct"));
        assert!(managers.contains("var TableSchemas = []TableSchema"));
        assert!(managers.contains("type ManagerRuntime struct"));
        assert!(managers.contains("NewManagerRuntimeFromPakSource"));
        assert!(managers.contains("gameassets.ParseDatasheet"));
        assert!(managers.contains("type dynamicTable struct"));
        assert!(managers.contains("DuplicateKeys"));
        assert!(managers.contains("map[string][]dynamicTableRow"));
        assert!(managers.contains("RowsByLookupKey map[string]dynamicTableRow"));
        assert!(!managers.contains("type DynamicTable struct"));
        assert!(!managers.contains("type DynamicTableRow struct"));
        assert!(!managers.contains("type ManagerInstance struct"));
        assert!(!managers.contains("func (instance *managerInstance) Manager"));
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
        assert!(managers.contains("Kind      DatasheetCellKind"));
        assert!(!managers.contains("DeclaredType"));
        assert!(!managers.contains("OptionalBool"));
        assert!(!managers.contains("ProjectionTransform"));
        assert!(!managers.contains("Native"));
        assert!(!managers.contains("RuntimeResource"));
    }

    fn manager_definition_names(source: &str) -> BTreeSet<String> {
        const MANAGERS_PREFIX: &str = "var managers = []managerDefinition{";
        let Some((_, managers)) = source.split_once(MANAGERS_PREFIX) else {
            return BTreeSet::new();
        };
        let managers = managers
            .split_once("\n}\n")
            .map_or(managers, |(block, _)| block);
        let mut depth = 0usize;
        let mut names = BTreeSet::new();
        const NAME_PREFIX: &str = "Name: \"";
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
