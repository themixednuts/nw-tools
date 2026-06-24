use std::collections::BTreeMap;

use treesitter_types_go::FromNode;

use crate::CodegenContext;
use crate::field_projection::{CodegenFieldTypeProjection, classify_codegen_field_type};
use crate::go::layout::GoStandaloneLayoutReport;
use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit};
use crate::layout::dependency_ordered_codegen_items;
use crate::naming::{rust_field_ident, rust_type_ident, rust_variant_ident};
use crate::types::ResolvedType;

use super::support;
use super::types::{GoTypeOptions, GoTypeRenderer};

mod error;
mod project;

pub use error::GoSourceEmitError;
pub use project::{GoStandaloneProject, GoStandaloneProjectFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSourceOptions {
    pub package_name: String,
    pub include_support_aliases: bool,
}

impl Default for GoSourceOptions {
    fn default() -> Self {
        Self {
            package_name: "types".to_owned(),
            include_support_aliases: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct GoSourceEmitter;

impl GoSourceEmitter {
    pub fn emit_unit(unit: &SerializeCodegenUnit) -> Result<String, GoSourceEmitError> {
        Self.emit(unit, &GoSourceOptions::default())
    }

    pub fn emit(
        &self,
        unit: &SerializeCodegenUnit,
        options: &GoSourceOptions,
    ) -> Result<String, GoSourceEmitError> {
        reject_unresolved_types(unit)?;

        if !is_go_identifier(&options.package_name) {
            return Err(GoSourceEmitError::PackageName {
                package_name: options.package_name.clone(),
            });
        }

        let mut out = String::new();
        out.push_str("package ");
        out.push_str(&options.package_name);
        out.push_str("\n\n");
        if options.include_support_aliases {
            out.push_str(support::single_file_source());
        }

        let type_renderer = GoTypeRenderer::new(GoTypeOptions {
            use_support_aliases: options.include_support_aliases,
        });

        for item in dependency_ordered_codegen_items(unit)
            .into_iter()
            .filter(|item| !item.is_reflection_marker)
        {
            match item.kind {
                SerializeCodegenItemKind::Struct => {
                    self.emit_struct(
                        item,
                        &type_renderer,
                        options.include_support_aliases,
                        &mut out,
                    );
                }
                SerializeCodegenItemKind::Enum => self.emit_enum(
                    item,
                    &type_renderer,
                    options.include_support_aliases,
                    &mut out,
                ),
            }
            out.push('\n');
        }
        format_go_source(&out)
    }

    pub fn emit_standalone_project(
        &self,
        unit: &SerializeCodegenUnit,
        module_path: &str,
        package_name: &str,
        context: &CodegenContext,
    ) -> Result<GoStandaloneProject, GoSourceEmitError> {
        self.emit_standalone_project_with_context(unit, unit, module_path, package_name, context)
    }

    pub fn emit_standalone_project_with_context(
        &self,
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
        module_path: &str,
        package_name: &str,
        context: &CodegenContext,
    ) -> Result<GoStandaloneProject, GoSourceEmitError> {
        project::emit_standalone_project_with_context(
            emitted_unit,
            context_unit,
            module_path,
            package_name,
            context,
        )
    }

    #[must_use]
    pub fn standalone_layout_report(unit: &SerializeCodegenUnit) -> GoStandaloneLayoutReport {
        GoStandaloneLayoutReport::from_codegen_unit(unit)
    }

    #[must_use]
    pub fn standalone_layout_report_with_context(
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
    ) -> GoStandaloneLayoutReport {
        GoStandaloneLayoutReport::from_codegen_unit_with_context(emitted_unit, context_unit)
    }

    fn emit_struct(
        &self,
        item: &SerializeCodegenItem,
        type_renderer: &GoTypeRenderer,
        include_support_aliases: bool,
        out: &mut String,
    ) {
        let type_name = rust_type_ident(&item.source_name);
        out.push_str("type ");
        out.push_str(&type_name);
        out.push_str(" struct {\n");
        let mut used_field_names = BTreeMap::new();
        let mut used_json_field_names = BTreeMap::new();
        for field in item.fields.iter().filter(|field| !field.is_base_class) {
            let field_name = unique_go_field_name(field, &mut used_field_names);
            let json_field_name =
                unique_go_json_field_name_for_field(field, &field_name, &mut used_json_field_names);
            out.push('\t');
            out.push_str(&field_name);
            out.push(' ');
            out.push_str(&render_go_field_type(field, type_renderer));
            out.push_str(" `json:\"");
            out.push_str(&json_field_name);
            out.push_str("\"`\n");
        }
        out.push_str("}\n");
        if include_support_aliases {
            emit_go_rtti_registration_and_method(item, &type_name, "AzRtti", out);
        }
    }

    fn emit_enum(
        &self,
        item: &SerializeCodegenItem,
        type_renderer: &GoTypeRenderer,
        include_support_aliases: bool,
        out: &mut String,
    ) {
        let type_name = rust_type_ident(&item.source_name);
        let variant_names = go_enum_variant_names(item, &type_name);
        let raw_type = item
            .enum_underlying_type
            .as_ref()
            .map(|resolved| type_renderer.render(resolved))
            .unwrap_or_else(|| "int32".to_owned());
        let raw_type = widen_go_enum_type_if_needed(&raw_type, item);
        out.push_str("type ");
        out.push_str(&type_name);
        out.push(' ');
        out.push_str(&raw_type);
        out.push_str("\n\nconst (\n");
        for (variant, variant_name) in item.variants.iter().zip(variant_names) {
            out.push('\t');
            out.push_str(&variant_name);
            out.push(' ');
            out.push_str(&type_name);
            if let Some(value) = variant.value_i32 {
                out.push_str(" = ");
                out.push_str(&value.to_string());
            }
            out.push('\n');
        }
        out.push_str(")\n");
        if include_support_aliases {
            emit_go_rtti_registration_and_method(item, &type_name, "AzRtti", out);
        }
    }
}

fn emit_go_rtti_registration_and_method(
    item: &SerializeCodegenItem,
    type_name: &str,
    rtti_type: &str,
    out: &mut String,
) {
    out.push_str("\nvar _ = RegisterAzRtti[");
    out.push_str(type_name);
    out.push_str("](");
    out.push_str(&go_string_literal(&item.source_name));
    out.push_str(", ");
    out.push_str(&go_string_literal(
        &item.source_type_id.hyphenated().to_string(),
    ));
    out.push_str(")\n\nfunc (value *");
    out.push_str(type_name);
    out.push_str(") AzRtti() *");
    out.push_str(rtti_type);
    out.push_str(" {\n\treturn AzRttiFor[");
    out.push_str(type_name);
    out.push_str("]()\n}\n");
}

fn render_go_field_type(
    field: &crate::ir::SerializeCodegenField,
    type_renderer: &GoTypeRenderer,
) -> String {
    match classify_codegen_field_type(field) {
        CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len } => {
            format!("[{byte_len}]byte")
        }
        CodegenFieldTypeProjection::Reflected(resolved_type) => type_renderer.render(resolved_type),
    }
}

fn go_enum_variant_names(item: &SerializeCodegenItem, type_name: &str) -> Vec<String> {
    let mut used = BTreeMap::<String, usize>::new();
    item.variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            let base = format!("{type_name}{}", rust_variant_ident(&variant.source_name));
            unique_variant_name(base, variant, index, &mut used)
        })
        .collect()
}

fn unique_variant_name(
    base: String,
    variant: &crate::ir::SerializeCodegenVariant,
    index: usize,
    used: &mut BTreeMap<String, usize>,
) -> String {
    if !used.contains_key(&base) {
        used.insert(base.clone(), 1);
        return base;
    }

    let mut candidate = format!(
        "{base}{}",
        variant_value_suffix(variant).unwrap_or_else(|| format!("Variant{index}"))
    );
    while used.contains_key(&candidate) {
        let next_index = used.get(&base).copied().unwrap_or(1) + 1;
        used.insert(base.clone(), next_index);
        candidate = format!("{base}Variant{next_index}");
    }
    used.insert(candidate.clone(), 1);
    candidate
}

fn variant_value_suffix(variant: &crate::ir::SerializeCodegenVariant) -> Option<String> {
    variant
        .value_i32
        .map(signed_value_suffix)
        .or_else(|| variant.value_u32.map(|value| format!("Value{value}")))
        .or_else(|| variant.value_u64.map(|value| format!("Value{value}")))
}

fn signed_value_suffix(value: i32) -> String {
    if let Some(abs) = value.checked_abs() {
        if value < 0 {
            format!("ValueMinus{abs}")
        } else {
            format!("Value{abs}")
        }
    } else {
        "ValueMin".to_owned()
    }
}

fn widen_go_enum_type_if_needed(raw_type: &str, item: &SerializeCodegenItem) -> String {
    if go_enum_type_fits_values(raw_type, item) {
        return raw_type.to_owned();
    }

    if item
        .variants
        .iter()
        .filter_map(|variant| variant.value_i32)
        .all(|value| value >= 0)
    {
        "uint32".to_owned()
    } else {
        "int32".to_owned()
    }
}

fn go_enum_type_fits_values(raw_type: &str, item: &SerializeCodegenItem) -> bool {
    item.variants
        .iter()
        .filter_map(|variant| variant.value_i32)
        .all(|value| go_enum_type_fits_value(raw_type, value))
}

fn go_enum_type_fits_value(raw_type: &str, value: i32) -> bool {
    match raw_type {
        "uint8" => u8::try_from(value).is_ok(),
        "uint16" => u16::try_from(value).is_ok(),
        "uint32" | "uint64" | "uint" => value >= 0,
        "int8" => i8::try_from(value).is_ok(),
        "int16" => i16::try_from(value).is_ok(),
        "int32" | "int64" | "int" => true,
        _ => true,
    }
}

fn type_id_suffix(type_id: uuid::Uuid) -> String {
    type_id.as_simple().to_string().chars().take(8).collect()
}

fn go_string_literal(value: &str) -> String {
    serde_json::to_string(value).expect("serialize Go string literal")
}

fn format_go_source(source: &str) -> Result<String, GoSourceEmitError> {
    let bytes = gofmt::formatter::format(source).map_err(GoSourceEmitError::Format)?;
    let formatted = String::from_utf8(bytes)?;
    let spaced = space_go_top_level_declarations(&formatted);
    validate_go_source(&spaced)?;
    Ok(spaced)
}

fn space_go_top_level_declarations(source: &str) -> String {
    let mut out = String::new();
    let mut last_line_class = GoTopLevelLine::Other;
    let mut last_output_line_was_blank = true;

    for line in source.lines() {
        let current_line_class = classify_go_top_level_line(line);
        if current_line_class.starts_statement()
            && !last_output_line_was_blank
            && !GoTopLevelLine::same_group(last_line_class, current_line_class)
        {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
        last_output_line_was_blank = line.trim().is_empty();
        if !last_output_line_was_blank {
            last_line_class = current_line_class;
        }
    }

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GoTopLevelLine {
    Package,
    Import,
    Declaration,
    Other,
}

impl GoTopLevelLine {
    const fn starts_statement(self) -> bool {
        matches!(self, Self::Package | Self::Import | Self::Declaration)
    }

    const fn same_group(left: Self, right: Self) -> bool {
        matches!(
            (left, right),
            (Self::Package, Self::Package) | (Self::Import, Self::Import)
        )
    }
}

fn classify_go_top_level_line(line: &str) -> GoTopLevelLine {
    if line.starts_with(' ') || line.starts_with('\t') {
        return GoTopLevelLine::Other;
    }
    if line.starts_with("package ") {
        return GoTopLevelLine::Package;
    }
    if line.starts_with("import ") {
        return GoTopLevelLine::Import;
    }
    if line.starts_with("type ")
        || line.starts_with("const ")
        || line.starts_with("var ")
        || line.starts_with("func ")
    {
        return GoTopLevelLine::Declaration;
    }

    GoTopLevelLine::Other
}

fn validate_go_source(source: &str) -> Result<(), GoSourceEmitError> {
    let source = ensure_trailing_newline(source);
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

fn ensure_trailing_newline(source: &str) -> std::borrow::Cow<'_, str> {
    if source.ends_with('\n') {
        std::borrow::Cow::Borrowed(source)
    } else {
        std::borrow::Cow::Owned(format!("{source}\n"))
    }
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
    format!(
        "{} at {}:{}..{}:{} `{}`",
        node.kind(),
        start.row + 1,
        start.column + 1,
        end.row + 1,
        end.column + 1,
        text
    )
}

fn reject_unresolved_types(unit: &SerializeCodegenUnit) -> Result<(), GoSourceEmitError> {
    for item in &unit.items {
        for field in &item.fields {
            if field.is_base_class {
                continue;
            }
            let CodegenFieldTypeProjection::Reflected(resolved_type) =
                classify_codegen_field_type(field)
            else {
                continue;
            };
            if let Some(unresolved) = resolved_type.unresolved() {
                return Err(GoSourceEmitError::UnresolvedType {
                    item_name: item.source_name.clone(),
                    field_name: field.source_name.clone(),
                    type_id: unresolved.type_id,
                    reason: unresolved.reason.to_owned(),
                });
            }
        }
        if let Some(resolved) = &item.enum_underlying_type
            && let Some(unresolved) = resolved.unresolved()
        {
            return Err(GoSourceEmitError::UnresolvedType {
                item_name: item.source_name.clone(),
                field_name: "<enum_underlying>".to_owned(),
                type_id: unresolved.type_id,
                reason: unresolved.reason.to_owned(),
            });
        }
    }
    Ok(())
}

fn go_field_name(source_name: &str) -> String {
    let rust_name = rust_field_ident(source_name);
    rust_name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(title_case)
        .collect::<String>()
}

fn unique_go_field_name(
    field: &crate::ir::SerializeCodegenField,
    used: &mut BTreeMap<String, usize>,
) -> String {
    let base = if field.is_base_class {
        go_base_class_field_name(field)
    } else {
        go_field_name(&field.source_name)
    };
    if !used.contains_key(&base) {
        used.insert(base.clone(), 1);
        return base;
    }

    let suffix = field
        .offset
        .map(|offset| format!("Offset{offset}"))
        .unwrap_or_else(|| format!("Type{}", type_id_suffix(field.source_type_id)));
    let mut candidate = format!("{base}{suffix}");
    while used.contains_key(&candidate) {
        let next_index = used.get(&base).copied().unwrap_or(1) + 1;
        used.insert(base.clone(), next_index);
        candidate = format!("{base}Field{next_index}");
    }
    used.insert(candidate.clone(), 1);
    candidate
}

fn go_base_class_field_name(field: &crate::ir::SerializeCodegenField) -> String {
    let ResolvedType::Named { source_name, .. } = &field.resolved_type else {
        return "Base".to_owned();
    };
    if source_name.contains("::") {
        go_field_name(&source_name.replace("::", "_"))
    } else {
        go_field_name(&rust_type_ident(source_name))
    }
}

fn go_json_field_name(source_name: &str) -> String {
    let rust_name = rust_field_ident(source_name);
    let mut parts = rust_name.split('_');
    let Some(first) = parts.next() else {
        return "field".to_owned();
    };
    let mut out = first.to_owned();
    for part in parts {
        out.push_str(&title_case(part));
    }
    out
}

fn unique_go_json_field_name(
    source_name: &str,
    go_field_name: &str,
    used: &mut BTreeMap<String, usize>,
) -> String {
    let base = go_json_field_name(source_name);
    let count = used.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        return base;
    }

    let fallback = lower_camel_go_field_name(go_field_name);
    let mut candidate = fallback.clone();
    let mut index = 2;
    while used.contains_key(&candidate) {
        candidate = format!("{fallback}Field{index}");
        index += 1;
    }
    used.insert(candidate.clone(), 1);
    candidate
}

fn unique_go_json_field_name_for_field(
    field: &crate::ir::SerializeCodegenField,
    go_field_name: &str,
    used: &mut BTreeMap<String, usize>,
) -> String {
    let source_name = if field.is_base_class {
        go_field_name
    } else {
        field.source_name.as_str()
    };
    unique_go_json_field_name(source_name, go_field_name, used)
}

fn lower_camel_go_field_name(field_name: &str) -> String {
    let mut chars = field_name.chars();
    let Some(first) = chars.next() else {
        return "field".to_owned();
    };
    let mut out = String::new();
    out.push(first.to_ascii_lowercase());
    out.extend(chars);
    out
}

fn title_case(part: &str) -> String {
    let mut chars = part.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    out.push(first.to_ascii_uppercase());
    out.extend(chars);
    out
}

fn is_go_identifier(value: &str) -> bool {
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
    use uuid::uuid;

    use crate::ir::{
        SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind,
        SerializeCodegenUnit, SerializeCodegenVariant,
    };
    use crate::role::ReflectedTypeRole;
    use crate::types::{MapKind, PointerKind, ResolvedType, ScalarType, SequenceKind};

    use super::*;

    #[test]
    fn emits_go_structs_and_enums_from_shared_codegen_unit() {
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                    source_name: "Example::CounterComponent".to_owned(),
                    role: ReflectedTypeRole::AzComponent,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "m_targetEntity".to_owned(),
                        source_type_id: uuid!("7568B2B9-6C27-4A3D-B334-1D949A3883F7"),
                        resolved_type: ResolvedType::Scalar(ScalarType::EntityId),
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                    source_name: "Mode".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Enum,
                    enum_underlying_type: Some(ResolvedType::Scalar(ScalarType::U8)),
                    fields: Vec::new(),
                    variants: vec![SerializeCodegenVariant {
                        source_name: "Enabled".to_owned(),
                        value_u64: Some(7),
                        value_u32: Some(7),
                        value_i32: Some(7),
                    }],
                },
            ],
        };

        let source = GoSourceEmitter::emit_unit(&unit).expect("Go source");

        assert!(source.contains("package types"));
        assert!(source.contains("\"github.com/google/uuid\""));
        assert!(!source.contains("\"hash/crc32\""));
        assert!(source.contains("type Uuid struct"));
        assert!(source.contains("value uuid.UUID"));
        assert!(source.contains("func CreateUuidData(bytes []byte) Uuid"));
        assert!(source.contains("func CombineUuid(lhs, rhs Uuid) Uuid"));
        assert!(!source.contains("type Uuid = uuid.UUID"));
        assert!(!source.contains("func CreateData(bytes []byte) Uuid"));
        assert!(!source.contains("type EntityId uint64"));
        assert!(!source.contains("type ComponentId uint64"));
        assert!(!source.contains("type ReplicatedField[T any] struct"));
        assert!(source.contains("type Crc32 uint32"));
        assert!(source.contains("func crc32(bytes []byte, forceLowerCase bool) uint32"));
        assert!(source.contains("type AssetId struct"));
        assert!(source.contains("type Asset struct"));
        assert!(source.contains("type CounterComponent struct"));
        assert!(source.contains("TargetEntity uint64 `json:\"targetEntity\"`"));
        assert!(source.contains("type Mode uint8"));
        assert!(source.contains("ModeEnabled Mode = 7"));
    }

    #[test]
    fn formats_top_level_spacing_between_package_imports_and_types() {
        let source = format_go_source(
            "package types\n\n\
             import \"fmt\"\n\
             type First struct{}\n\
             type Second struct{}\n",
        )
        .expect("formatted Go source");

        assert!(source.contains("package types\n\nimport \"fmt\"\n\n"));
        assert!(source.contains("type First struct{}\n\n"));
        assert!(source.contains("\n\ntype Second struct{}"));
        assert!(source.ends_with('\n'));
    }

    #[test]
    fn single_file_emit_orders_dependencies_before_dependents() {
        let leaf_type_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let consumer_type_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let unit = SerializeCodegenUnit {
            items: vec![
                fixture_item(
                    consumer_type_id,
                    "Consumer",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![pointer_field("m_leaf", leaf_type_id, "Leaf")],
                ),
                fixture_item(
                    leaf_type_id,
                    "Leaf",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
            ],
        };

        let source = GoSourceEmitter::emit_unit(&unit).expect("Go source");
        let leaf = source.find("type Leaf struct").expect("Leaf type");
        let consumer = source.find("type Consumer struct").expect("Consumer type");

        assert!(
            leaf < consumer,
            "dependency should be emitted before dependent:\n{source}"
        );
    }

    #[test]
    fn emits_nested_generic_field_types_from_resolved_shape() {
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::RouteComponent".to_owned(),
                role: ReflectedTypeRole::AzComponent,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![SerializeCodegenField {
                    source_name: "m_routes".to_owned(),
                    source_type_id: uuid!("33333333-3333-3333-3333-333333333333"),
                    resolved_type: ResolvedType::Map {
                        kind: MapKind::UnorderedMap,
                        key: Box::new(ResolvedType::Scalar(ScalarType::EntityId)),
                        value: Box::new(ResolvedType::Sequence {
                            kind: SequenceKind::Array,
                            element: Box::new(ResolvedType::Pair {
                                first: Box::new(ResolvedType::Named {
                                    type_id: uuid!("44444444-4444-4444-4444-444444444444"),
                                    source_name: "Example::AddressType".to_owned(),
                                }),
                                second: Box::new(ResolvedType::Pointer {
                                    kind: PointerKind::Shared,
                                    target: Box::new(ResolvedType::Scalar(ScalarType::U8)),
                                }),
                            }),
                            capacity: Some(2),
                        }),
                    },
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                }],
                variants: Vec::new(),
            }],
        };

        let source = GoSourceEmitter::emit_unit(&unit).expect("Go source");

        assert!(source.contains("Routes"));
        assert!(source.contains("map[uint64][2]struct"));
        assert!(source.contains("First"));
        assert!(source.contains("AddressType"));
        assert!(source.contains("Second *uint8"));
        assert!(source.contains("`json:\"routes\"`"));
        assert!(source.contains(
            "var _ = RegisterAzRtti[RouteComponent](\"Example::RouteComponent\", \"11111111-1111-1111-1111-111111111111\")"
        ));
        assert!(source.contains("func (value *RouteComponent) AzRtti() *AzRtti"));
        assert!(source.contains("return AzRttiFor[RouteComponent]()"));
    }

    #[test]
    fn emits_plain_slices_for_non_comparable_single_file_go_shapes() {
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::BlobIndex".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![
                    SerializeCodegenField {
                        source_name: "m_lookup".to_owned(),
                        source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                        resolved_type: ResolvedType::Map {
                            kind: MapKind::UnorderedMap,
                            key: Box::new(ResolvedType::ByteStream),
                            value: Box::new(ResolvedType::Scalar(ScalarType::String)),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                    SerializeCodegenField {
                        source_name: "m_blobs".to_owned(),
                        source_type_id: uuid!("33333333-3333-3333-3333-333333333333"),
                        resolved_type: ResolvedType::Sequence {
                            kind: SequenceKind::UnorderedSet,
                            element: Box::new(ResolvedType::ByteStream),
                            capacity: None,
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                ],
                variants: Vec::new(),
            }],
        };

        let source = GoSourceEmitter::emit_unit(&unit).expect("Go source");

        assert!(source.contains("Lookup"), "{source}");
        assert!(source.contains("[]struct {"), "{source}");
        assert!(
            source.contains("Key") && source.contains("[]byte"),
            "{source}"
        );
        assert!(source.contains("Value string"), "{source}");
        assert!(source.contains("Blobs"), "{source}");
        assert!(source.contains("[][]byte"), "{source}");
        assert!(source.contains("`json:\"blobs\"`"), "{source}");
        assert!(
            !source.contains("AzRttiType AzRtti `json:\"-\"`"),
            "{source}"
        );
        assert!(
            source.contains(
                "var _ = RegisterAzRtti[BlobIndex](\"Example::BlobIndex\", \"11111111-1111-1111-1111-111111111111\")"
            ),
            "{source}"
        );
        assert!(
            source.contains("func (value *BlobIndex) AzRtti() *AzRtti"),
            "{source}"
        );
    }

    #[test]
    fn standalone_project_emits_fixed_opaque_bytes_without_missing_import() {
        let missing_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::OpaquePayload".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![SerializeCodegenField {
                    source_name: "payload".to_owned(),
                    source_type_id: missing_id,
                    resolved_type: ResolvedType::Unknown {
                        type_id: missing_id,
                        reason: "type id is not present in SerializeContext".to_owned(),
                    },
                    data_size: Some(16),
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                }],
                variants: Vec::new(),
            }],
        };

        let unit_source = GoSourceEmitter::emit_unit(&unit).expect("Go source");
        assert!(unit_source.contains("Payload"), "{unit_source}");
        assert!(unit_source.contains("[16]byte"), "{unit_source}");
        assert!(!unit_source.contains("Typeaaaaaaaa"), "{unit_source}");

        let context = crate::CodegenContext::inline();
        let project = GoSourceEmitter
            .emit_standalone_project(&unit, "example.com/aztypes", "aztypes", &context)
            .expect("Go project");
        let source = project
            .files
            .iter()
            .find(|file| file.path.ends_with("opaque_payload.go"))
            .expect("opaque payload file")
            .source
            .as_str();

        assert!(source.contains("Payload"), "{source}");
        assert!(source.contains("[16]byte"), "{source}");
        assert!(
            !source.contains("AzRttiType rtti.Type `json:\"-\"`"),
            "{source}"
        );
        assert!(
            source.contains(
                "var _ = rtti.Register[OpaquePayload](\"Example::OpaquePayload\", \"11111111-1111-1111-1111-111111111111\")"
            ),
            "{source}"
        );
        assert!(
            source.contains("func (value *OpaquePayload) AzRtti() *rtti.Type"),
            "{source}"
        );
        assert!(!source.contains("/types/missing"), "{source}");
        assert!(
            !project
                .files
                .iter()
                .any(|file| file.path == "types/missing/types.go")
        );
    }

    #[test]
    fn emits_unique_json_tags_for_duplicate_reflected_field_names() {
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::ContractItemSimpleData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![
                    SerializeCodegenField {
                        source_name: "rarityLevel".to_owned(),
                        source_type_id: uuid!("72039442-eb38-4d42-a1ad-cb68f7e0eef6"),
                        resolved_type: ResolvedType::Scalar(ScalarType::I32),
                        data_size: None,
                        offset: Some(252),
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                    SerializeCodegenField {
                        source_name: "rarityLevel".to_owned(),
                        source_type_id: uuid!("72039442-eb38-4d42-a1ad-cb68f7e0eef6"),
                        resolved_type: ResolvedType::Scalar(ScalarType::I32),
                        data_size: None,
                        offset: Some(264),
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                ],
                variants: Vec::new(),
            }],
        };

        let source = GoSourceEmitter::emit_unit(&unit).expect("Go source");

        assert!(source.contains("RarityLevel"));
        assert!(source.contains("`json:\"rarityLevel\"`"));
        assert!(source.contains("RarityLevelOffset264"));
        assert!(source.contains("`json:\"rarityLevelOffset264\"`"));
    }

    #[test]
    fn emits_unique_enum_const_names_for_duplicate_reflected_variant_names() {
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Rarity".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Enum,
                enum_underlying_type: Some(ResolvedType::Scalar(ScalarType::U8)),
                fields: Vec::new(),
                variants: vec![
                    SerializeCodegenVariant {
                        source_name: "Level".to_owned(),
                        value_u64: Some(1),
                        value_u32: Some(1),
                        value_i32: Some(1),
                    },
                    SerializeCodegenVariant {
                        source_name: "Level".to_owned(),
                        value_u64: Some(2),
                        value_u32: Some(2),
                        value_i32: Some(2),
                    },
                ],
            }],
        };

        let source = GoSourceEmitter::emit_unit(&unit).expect("Go source");

        assert!(source.contains("RarityLevel"));
        assert!(source.contains("RarityLevelValue2"));
    }

    #[test]
    fn standalone_project_does_not_suffix_unemitted_type_name_collisions() {
        let component_type_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let shared_type_id = uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc");
        let other_shared_type_id = uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd");
        let component = fixture_item(
            component_type_id,
            "CounterComponent",
            ReflectedTypeRole::AzComponent,
            false,
            vec![pointer_field("m_shared", shared_type_id, "Shared")],
        );
        let shared = fixture_item(
            shared_type_id,
            "Shared",
            ReflectedTypeRole::SupportType,
            false,
            Vec::new(),
        );
        let other_shared = fixture_item(
            other_shared_type_id,
            "Other::Shared",
            ReflectedTypeRole::SupportType,
            false,
            Vec::new(),
        );
        let emitted_unit = SerializeCodegenUnit {
            items: vec![component.clone(), shared.clone()],
        };
        let context_unit = SerializeCodegenUnit {
            items: vec![component, shared, other_shared],
        };

        let context = crate::CodegenContext::inline();
        let project = GoSourceEmitter
            .emit_standalone_project_with_context(
                &emitted_unit,
                &context_unit,
                "example.com/aztypes",
                "aztypes",
                &context,
            )
            .expect("standalone Go project");
        let sources = project
            .files
            .iter()
            .map(|file| file.source.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(sources.contains("type Shared struct"));
        assert!(!sources.contains("SharedCCCCCCCC"));
        assert!(!sources.contains("SharedDDDDDDDD"));
    }

    #[test]
    fn standalone_project_keeps_duplicate_leaf_names_in_distinct_packages() {
        let catalog_item_id = uuid!("11111111-1111-1111-1111-111111111111");
        let runtime_item_id = uuid!("22222222-2222-2222-2222-222222222222");
        let holder_id = uuid!("33333333-3333-3333-3333-333333333333");
        let unit = SerializeCodegenUnit {
            items: vec![
                fixture_item(
                    catalog_item_id,
                    "Catalog::Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    runtime_item_id,
                    "Runtime::Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    holder_id,
                    "Runtime::Holder",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![
                        named_field("m_catalogItem", catalog_item_id, "Catalog::Item"),
                        named_field("m_runtimeItem", runtime_item_id, "Runtime::Item"),
                    ],
                ),
            ],
        };

        let context = crate::CodegenContext::inline();
        let project = GoSourceEmitter
            .emit_standalone_project(&unit, "example.com/aztypes", "aztypes", &context)
            .expect("standalone Go project");
        let catalog_item = project
            .files
            .iter()
            .find(|file| file.path == "types/catalog/item.go")
            .expect("catalog item file");
        let runtime_item = project
            .files
            .iter()
            .find(|file| file.path == "types/runtime/item.go")
            .expect("runtime item file");
        let holder = project
            .files
            .iter()
            .find(|file| file.path == "types/runtime/holder.go")
            .expect("holder file");

        assert!(catalog_item.source.contains("type Item struct"));
        assert!(runtime_item.source.contains("type Item struct"));
        assert!(!catalog_item.source.contains("Item11111111"));
        assert!(!runtime_item.source.contains("Item22222222"));
        assert!(
            holder
                .source
                .contains("\"example.com/aztypes/types/catalog\"")
        );
        assert!(holder.source.contains("CatalogItem catalog.Item"));
        assert!(!holder.source.contains("types_catalogtypes"));
        assert!(holder.source.contains("RuntimeItem Item"));
    }

    #[test]
    fn standalone_project_uses_natural_import_selectors_for_reflected_type_packages() {
        let component_type_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let config_type_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let az_component_type_id = uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc");
        let unit = SerializeCodegenUnit {
            items: vec![
                fixture_item(
                    az_component_type_id,
                    "AZ::Component",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![scalar_field(
                        "m_id",
                        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
                        ScalarType::U64,
                    )],
                ),
                fixture_item(
                    config_type_id,
                    "VegetationRandomGradientConfig",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![scalar_field(
                        "m_randomSeed",
                        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
                        ScalarType::I32,
                    )],
                ),
                fixture_item(
                    component_type_id,
                    "VegetationRandomGradientComponent",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![
                        base_field(az_component_type_id, "AZ::Component"),
                        named_field(
                            "m_configuration",
                            config_type_id,
                            "VegetationRandomGradientConfig",
                        ),
                    ],
                ),
            ],
        };

        let context = crate::CodegenContext::inline();
        let project = GoSourceEmitter
            .emit_standalone_project(&unit, "aztypesvalidation", "aztypesvalidation", &context)
            .expect("standalone Go project");
        let component = project
            .files
            .iter()
            .find(|file| {
                file.source
                    .contains("type VegetationRandomGradientComponent struct")
            })
            .expect("vegetation component file");
        let source = component.source.as_str();

        assert!(
            source.contains("\"aztypesvalidation/types/az\""),
            "{source}"
        );
        assert!(source.contains("az.Component"), "{source}");
        assert!(source.contains("Configuration"), "{source}");
        assert!(
            source.contains("VegetationRandomGradientConfig"),
            "{source}"
        );
        assert!(!source.contains("types_az_componentstypes"));
        assert!(!source.contains(
            "types_components_vegetation_random_gradient_component_component_configstypes"
        ));
    }

    #[test]
    fn standalone_project_uses_natural_import_selectors_for_support_packages() {
        let unit = SerializeCodegenUnit {
            items: vec![fixture_item(
                uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
                "Example::SupportRefs",
                ReflectedTypeRole::SupportType,
                false,
                vec![
                    scalar_field(
                        "m_guid",
                        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
                        ScalarType::Uuid,
                    ),
                    scalar_field(
                        "m_crc",
                        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                        ScalarType::Crc32,
                    ),
                    scalar_field(
                        "m_assetId",
                        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
                        ScalarType::AssetId,
                    ),
                    scalar_field(
                        "m_position",
                        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
                        ScalarType::Vector3,
                    ),
                ],
            )],
        };

        let context = crate::CodegenContext::inline();
        let project = GoSourceEmitter
            .emit_standalone_project(&unit, "example.com/aztypes", "aztypes", &context)
            .expect("standalone Go project");
        let source = project
            .files
            .iter()
            .find(|file| file.source.contains("type SupportRefs struct"))
            .expect("support refs file")
            .source
            .as_str();

        assert!(source.contains("\"example.com/aztypes/az/asset\""));
        assert!(source.contains("\"example.com/aztypes/az/crc\""));
        assert!(source.contains("\"example.com/aztypes/az/math\""));
        assert!(source.contains("\"example.com/aztypes/az/uuid\""));
        assert!(source.contains("uuid.Uuid"), "{source}");
        assert!(source.contains("crc.Crc32"), "{source}");
        assert!(source.contains("asset.AssetId"), "{source}");
        assert!(source.contains("math.Vector3"), "{source}");
        assert!(!source.contains("azasset \""));
        assert!(!source.contains("azcrc \""));
        assert!(!source.contains("azmath \""));
        assert!(!source.contains("azuuid \""));
    }

    #[test]
    fn standalone_project_aliases_only_colliding_go_package_names() {
        let left_id = uuid!("11111111-1111-1111-1111-111111111111");
        let right_id = uuid!("22222222-2222-2222-2222-222222222222");
        let holder_id = uuid!("33333333-3333-3333-3333-333333333333");
        let unit = SerializeCodegenUnit {
            items: vec![
                fixture_item(
                    left_id,
                    "Catalog::Shared::Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    right_id,
                    "Runtime::Shared::Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    holder_id,
                    "Holder",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![
                        named_field("m_catalogItem", left_id, "Catalog::Shared::Item"),
                        named_field("m_runtimeItem", right_id, "Runtime::Shared::Item"),
                    ],
                ),
            ],
        };

        let context = crate::CodegenContext::inline();
        let project = GoSourceEmitter
            .emit_standalone_project(&unit, "example.com/aztypes", "aztypes", &context)
            .expect("standalone Go project");
        let source = project
            .files
            .iter()
            .find(|file| file.source.contains("type Holder struct"))
            .expect("holder file")
            .source
            .as_str();

        assert!(source.contains("\"example.com/aztypes/types/catalog/shared\""));
        assert!(source.contains("runtime_shared \"example.com/aztypes/types/runtime/shared\""));
        assert!(source.contains("CatalogItem shared.Item"));
        assert!(source.contains("RuntimeItem runtime_shared.Item"));
        assert!(!source.contains("types_catalog_sharedtypes"));
        assert!(!source.contains("types_runtime_sharedtypes"));
    }

    #[test]
    fn standalone_project_keeps_same_item_name_when_family_package_owns_name() {
        let item_family_id = uuid!("b9f3747d-192b-5eda-606d-737d339a9679");
        let item_record_id = uuid!("a6d8db05-cc68-4fbe-8002-55c0c7b1fd08");
        let ammo_id = uuid!("11111111-1111-1111-1111-111111111111");
        let descriptor_id = uuid!("22222222-2222-2222-2222-222222222222");
        let slot_id = uuid!("33333333-3333-3333-3333-333333333333");
        let other_slot_id = uuid!("44444444-4444-4444-4444-444444444444");
        let version_data_id = uuid!("55555555-5555-5555-5555-555555555555");
        let unit = SerializeCodegenUnit {
            items: vec![
                fixture_item(
                    item_family_id,
                    "Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![named_field("m_descriptor", descriptor_id, "ItemDescriptor")],
                ),
                fixture_item(
                    ammo_id,
                    "Ammo",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![base_field(item_family_id, "Item")],
                ),
                fixture_item(
                    item_record_id,
                    "Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![named_field(
                        "m_currentVersion",
                        version_data_id,
                        "ItemVersionData",
                    )],
                ),
                fixture_item(
                    version_data_id,
                    "ItemVersionData",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![named_field("m_childItem", item_record_id, "Item")],
                ),
                fixture_item(
                    descriptor_id,
                    "ItemDescriptor",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    slot_id,
                    "ItemContainerSlot",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![named_field("m_item", item_family_id, "Item")],
                ),
                fixture_item(
                    other_slot_id,
                    "OtherItemContainerSlot",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![named_field("m_item", item_family_id, "Item")],
                ),
            ],
        };

        let context = crate::CodegenContext::inline();
        let project = GoSourceEmitter
            .emit_standalone_project(&unit, "example.com/aztypes", "aztypes", &context)
            .expect("standalone Go project");
        let paths = project
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let item_family = project
            .files
            .iter()
            .find(|file| file.path == "types/item/item.go")
            .unwrap_or_else(|| panic!("item family file\n{paths}"));
        let item_record = project
            .files
            .iter()
            .find(|file| file.path == "types/item/item/item.go")
            .unwrap_or_else(|| panic!("item record file\n{paths}"));
        let version_data = project
            .files
            .iter()
            .find(|file| file.path == "types/item/item/item_version_data.go")
            .unwrap_or_else(|| panic!("item version data file\n{paths}"));
        let sources = project
            .files
            .iter()
            .map(|file| file.source.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(item_family.source.contains("type Item struct"));
        assert!(item_record.source.contains("type Item struct"));
        assert!(version_data.source.contains("type ItemVersionData struct"));
        assert!(!sources.contains("ItemB9F3747D"));
        assert!(!sources.contains("ItemA6D8DB05"));
    }

    #[test]
    fn standalone_project_places_namespace_before_component_family() {
        let unit = namespace_and_base_family_fixture();

        let context = crate::CodegenContext::inline();
        let project = GoSourceEmitter
            .emit_standalone_project(&unit, "aztypesvalidation", "aztypesvalidation", &context)
            .expect("Go standalone project");
        let paths = project
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(paths.contains("types/az_framework/components/input_system_component.go"));
        assert!(paths.contains("types/az_framework/input_device_id.go"));
        assert!(paths.contains("types/action_conditions/action_condition.go"));
        assert!(paths.contains("types/action_conditions/action_condition_if_input.go"));
        assert!(paths.contains("types/components/faceted_components/facet.go"));
        assert!(paths.contains("types/components/faceted_components/client_facet.go"));
        assert!(paths.contains(
            "types/components/faceted_components/inventory_component/inventory_component.go"
        ));
        assert!(
            paths.contains(
                "types/components/faceted_components/inventory_component/client_facet.go"
            )
        );
        assert!(
            !paths
                .iter()
                .any(|path| path.starts_with("types/components/faceted_components/facets"))
        );
        assert!(
            !paths
                .iter()
                .any(|path| path.starts_with("types/components/az_framework"))
        );
        assert!(!paths.contains("types/facets/client_facets/inventory_client_facet.go"));
        assert!(!paths.contains(
            "types/components/faceted_components/facets/client_facets/inventory_client_facet.go"
        ));
        assert!(!paths.contains("types/global/action_condition.go"));
    }

    #[test]
    fn standalone_layout_report_exposes_package_files_without_emitting_sources() {
        let unit = namespace_and_base_family_fixture();

        let report = GoSourceEmitter::standalone_layout_report(&unit);
        let text = report.to_text();
        let paths = report
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(paths.contains("types/az_framework/components/input_system_component.go"));
        assert!(paths.contains("types/az_framework/input_device_id.go"));
        assert!(paths.contains("types/action_conditions/action_condition.go"));
        assert!(paths.contains("types/action_conditions/action_condition_if_input.go"));
        assert!(paths.contains("types/components/faceted_components/facet.go"));
        assert!(paths.contains("types/components/faceted_components/client_facet.go"));
        assert!(
            paths.contains(
                "types/components/faceted_components/inventory_component/client_facet.go"
            )
        );
        assert!(text.contains(
            "file types/action_conditions/action_condition_if_input.go package action_conditions"
        ));
        assert!(text.contains(
            "file types/components/faceted_components/inventory_component/client_facet.go package inventory_component"
        ));
        assert!(text.contains("  item ActionConditionIfInput "));
        assert!(text.contains("  item InventoryClientFacet "));
        assert!(
            report
                .files
                .iter()
                .flat_map(|file| &file.items)
                .any(|item| {
                    item.go_name == "InputSystemComponent"
                        && item.source_name == "AzFramework::InputSystemComponent"
                })
        );
    }

    #[test]
    fn rejects_invalid_package_names() {
        let unit = SerializeCodegenUnit::default();
        let err = GoSourceEmitter
            .emit(
                &unit,
                &GoSourceOptions {
                    package_name: "type".to_owned(),
                    include_support_aliases: false,
                },
            )
            .expect_err("keyword package name should fail");

        assert!(matches!(err, GoSourceEmitError::PackageName { .. }));
    }

    #[test]
    fn rejects_unresolved_field_types_before_emitting_source() {
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::BrokenComponent".to_owned(),
                role: ReflectedTypeRole::AzComponent,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![SerializeCodegenField {
                    source_name: "m_values".to_owned(),
                    source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                    resolved_type: ResolvedType::Sequence {
                        kind: SequenceKind::Vector,
                        element: Box::new(ResolvedType::Unknown {
                            type_id: uuid!("33333333-3333-3333-3333-333333333333"),
                            reason: "missing fixture type".to_owned(),
                        }),
                        capacity: None,
                    },
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                }],
                variants: Vec::new(),
            }],
        };

        let err = GoSourceEmitter::emit_unit(&unit).expect_err("unresolved type");

        assert!(matches!(
            err,
            GoSourceEmitError::UnresolvedType { field_name, .. } if field_name == "m_values"
        ));
    }

    fn namespace_and_base_family_fixture() -> SerializeCodegenUnit {
        let component_type_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let input_system_type_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let input_device_type_id = uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc");
        let action_condition_type_id = uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd");
        let action_condition_child_type_id = uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee");
        let facet_type_id = uuid!("11111111-1111-1111-1111-111111111111");
        let faceted_component_type_id = uuid!("22222222-2222-2222-2222-222222222222");
        let client_facet_type_id = uuid!("33333333-3333-3333-3333-333333333333");
        let inventory_component_type_id = uuid!("55555555-5555-5555-5555-555555555555");
        let inventory_client_facet_type_id = uuid!("44444444-4444-4444-4444-444444444444");

        SerializeCodegenUnit {
            items: vec![
                abstract_fixture_item(
                    component_type_id,
                    "AZ::Component",
                    ReflectedTypeRole::SupportType,
                    true,
                    Vec::new(),
                ),
                fixture_item(
                    input_system_type_id,
                    "AzFramework::InputSystemComponent",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![base_field(component_type_id, "AZ::Component")],
                ),
                fixture_item(
                    input_device_type_id,
                    "AzFramework::InputDeviceId",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                abstract_fixture_item(
                    action_condition_type_id,
                    "ActionCondition",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    action_condition_child_type_id,
                    "ActionConditionIfInput",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![base_field(action_condition_type_id, "ActionCondition")],
                ),
                abstract_fixture_item(
                    facet_type_id,
                    "Facet",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    faceted_component_type_id,
                    "FacetedComponent",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![
                        base_field(component_type_id, "AZ::Component"),
                        pointer_field("m_clientFacetPtr", client_facet_type_id, "ClientFacet"),
                    ],
                ),
                fixture_item(
                    client_facet_type_id,
                    "ClientFacet",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![base_field(facet_type_id, "Facet")],
                ),
                fixture_item(
                    inventory_component_type_id,
                    "InventoryComponent",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![base_field(faceted_component_type_id, "FacetedComponent")],
                ),
                fixture_item(
                    inventory_client_facet_type_id,
                    "InventoryClientFacet",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![base_field(client_facet_type_id, "ClientFacet")],
                ),
            ],
        }
    }

    fn fixture_item(
        source_type_id: uuid::Uuid,
        source_name: &str,
        role: ReflectedTypeRole,
        is_reflection_marker: bool,
        fields: Vec<SerializeCodegenField>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id,
            source_name: source_name.to_owned(),
            role,
            is_reflection_marker,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::new(),
        }
    }

    fn abstract_fixture_item(
        source_type_id: uuid::Uuid,
        source_name: &str,
        role: ReflectedTypeRole,
        is_reflection_marker: bool,
        fields: Vec<SerializeCodegenField>,
    ) -> SerializeCodegenItem {
        let mut item = fixture_item(
            source_type_id,
            source_name,
            role,
            is_reflection_marker,
            fields,
        );
        item.is_abstract = Some(true);
        item
    }

    fn base_field(source_type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: "BaseClass1".to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: source_type_id,
                source_name: source_name.to_owned(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: true,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }

    fn pointer_field(
        source_name: &str,
        source_type_id: uuid::Uuid,
        type_name: &str,
    ) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: source_name.to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: source_type_id,
                source_name: type_name.to_owned(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: true,
            is_dynamic_field: false,
        }
    }

    fn named_field(
        source_name: &str,
        source_type_id: uuid::Uuid,
        type_name: &str,
    ) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: source_name.to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: source_type_id,
                source_name: type_name.to_owned(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }

    fn scalar_field(
        source_name: &str,
        source_type_id: uuid::Uuid,
        scalar: ScalarType,
    ) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: source_name.to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Scalar(scalar),
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }
}
