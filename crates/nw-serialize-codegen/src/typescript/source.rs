use std::collections::{BTreeMap, BTreeSet};

use crate::field_projection::{CodegenFieldTypeProjection, classify_codegen_field_type};
use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit};
use crate::layout::dependency_ordered_codegen_items;
use crate::naming::{rust_field_ident, rust_type_ident, rust_variant_ident};
use crate::support_usage::CodegenContainerSupportUsage;
use crate::typescript::layout::TypeScriptStandaloneLayoutReport;

use super::support;
use super::types::{TypeScriptTypeOptions, TypeScriptTypeRenderer};

mod error;
mod project;

pub use error::TypeScriptSourceEmitError;
pub use project::{
    TypeScriptStandaloneProject, TypeScriptStandaloneProjectFile,
    TypeScriptStandaloneProjectOptions,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptSourceOptions {
    pub include_support_aliases: bool,
}

impl Default for TypeScriptSourceOptions {
    fn default() -> Self {
        Self {
            include_support_aliases: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct TypeScriptSourceEmitter;

impl TypeScriptSourceEmitter {
    pub fn emit_unit(unit: &SerializeCodegenUnit) -> Result<String, TypeScriptSourceEmitError> {
        Self::default().emit(unit)
    }

    pub fn emit(&self, unit: &SerializeCodegenUnit) -> Result<String, TypeScriptSourceEmitError> {
        self.emit_with_options(unit, &TypeScriptSourceOptions::default())
    }

    pub fn emit_with_options(
        &self,
        unit: &SerializeCodegenUnit,
        options: &TypeScriptSourceOptions,
    ) -> Result<String, TypeScriptSourceEmitError> {
        reject_unresolved_types(unit)?;

        let mut out = String::new();
        if needs_container_aliases(unit) {
            out.push_str(
                "export class FixedBytes<Length extends number> {\n\
                    constructor(readonly bytes: Uint8Array, readonly length: Length) {\n\
                        if (bytes.length !== length) {\n\
                            throw new Error(`expected ${length} bytes, got ${bytes.length}`);\n\
                        }\n\
                    }\n\
                 }\n\
                 export type FixedArray<T, Length extends number> = T[] & { readonly length: Length };\n\
                 export type FixedVector<T, _Capacity extends number> = T[];\n\
                 export type BitSet<Size extends number> = FixedArray<boolean, Size>;\n\n",
            );
        }
        if options.include_support_aliases {
            out.push_str(&support::single_file_source());
        }

        let type_renderer = TypeScriptTypeRenderer::new(TypeScriptTypeOptions {
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
                SerializeCodegenItemKind::Enum => {
                    self.emit_enum(item, options.include_support_aliases, &mut out);
                }
            }
            out.push('\n');
        }
        format_typescript_source(&out)
    }

    #[must_use]
    pub fn standalone_layout_report(
        unit: &SerializeCodegenUnit,
    ) -> TypeScriptStandaloneLayoutReport {
        TypeScriptStandaloneLayoutReport::from_codegen_unit(unit)
    }

    #[must_use]
    pub fn standalone_layout_report_with_context(
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
    ) -> TypeScriptStandaloneLayoutReport {
        TypeScriptStandaloneLayoutReport::from_codegen_unit_with_context(emitted_unit, context_unit)
    }

    fn emit_struct(
        &self,
        item: &SerializeCodegenItem,
        type_renderer: &TypeScriptTypeRenderer,
        include_support_aliases: bool,
        out: &mut String,
    ) {
        let type_name = rust_type_ident(&item.source_name);
        if include_support_aliases {
            emit_typescript_rtti_const(item, &type_name, out);
        }
        if include_support_aliases {
            out.push_str("export ");
            if item.is_abstract == Some(true) {
                out.push_str("abstract ");
            }
            out.push_str("class ");
            out.push_str(&type_name);
            out.push_str(" extends AzRtti {\n\toverride readonly azRtti = ");
            out.push_str(&typescript_rtti_const_name(&type_name));
            out.push_str(";\n");
        } else {
            out.push_str("export interface ");
            out.push_str(&type_name);
            out.push_str(" {\n");
        }
        let mut used_field_names = BTreeMap::new();
        for field in item.fields.iter().filter(|field| !field.is_base_class) {
            out.push('\t');
            if include_support_aliases {
                out.push_str("declare ");
            }
            out.push_str(&unique_typescript_field_name(field, &mut used_field_names));
            out.push_str(": ");
            out.push_str(&render_typescript_field_type(field, type_renderer));
            out.push_str(";\n");
        }
        out.push_str("}\n");
        if include_support_aliases {
            emit_typescript_rtti_registration(&type_name, out);
        }
    }

    fn emit_enum(
        &self,
        item: &SerializeCodegenItem,
        include_support_aliases: bool,
        out: &mut String,
    ) {
        let variant_names = typescript_enum_variant_names(item);
        let type_name = rust_type_ident(&item.source_name);
        if include_support_aliases {
            emit_typescript_rtti_const(item, &type_name, out);
        }
        emit_typescript_const_enum_like(item, &type_name, &variant_names, out);
        if include_support_aliases {
            emit_typescript_rtti_registration(&type_name, out);
        }
    }
}

fn emit_typescript_rtti_const(item: &SerializeCodegenItem, type_name: &str, out: &mut String) {
    out.push_str("export const ");
    out.push_str(&typescript_rtti_const_name(type_name));
    out.push_str(": Rtti = Rtti.fromTypeId(");
    out.push_str(&typescript_string_literal(&item.source_name));
    out.push_str(", ");
    out.push_str(&typescript_string_literal(
        &item.source_type_id.hyphenated().to_string(),
    ));
    out.push_str(");\n");
}

fn emit_typescript_rtti_registration(type_name: &str, out: &mut String) {
    out.push_str("registerType(");
    out.push_str(type_name);
    out.push_str(", ");
    out.push_str(&typescript_rtti_const_name(type_name));
    out.push_str(");\n");
}

fn typescript_rtti_const_name(type_name: &str) -> String {
    format!("{type_name}Rtti")
}

fn emit_typescript_const_enum_like(
    item: &SerializeCodegenItem,
    type_name: &str,
    variant_names: &[String],
    out: &mut String,
) {
    out.push_str("export const ");
    out.push_str(type_name);
    out.push_str(" = {\n");
    for (variant, variant_name) in item.variants.iter().zip(variant_names) {
        out.push('\t');
        out.push_str(variant_name);
        out.push_str(": ");
        if let Some(value) = variant.value_i32 {
            out.push_str(&value.to_string());
        } else {
            out.push_str(&typescript_string_literal(&variant.source_name));
        }
        out.push_str(",\n");
    }
    out.push_str("} as const;\n");
    out.push_str("export type ");
    out.push_str(type_name);
    out.push_str(" = (typeof ");
    out.push_str(type_name);
    out.push_str(")[keyof typeof ");
    out.push_str(type_name);
    out.push_str("];\n");
}

fn render_typescript_field_type(
    field: &crate::ir::SerializeCodegenField,
    type_renderer: &TypeScriptTypeRenderer,
) -> String {
    match classify_codegen_field_type(field) {
        CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len } => {
            format!("FixedBytes<{byte_len}>")
        }
        CodegenFieldTypeProjection::Reflected(resolved_type) => type_renderer.render(resolved_type),
    }
}

fn typescript_enum_variant_names(item: &SerializeCodegenItem) -> Vec<String> {
    let mut used = BTreeMap::<String, usize>::new();
    item.variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            let base = rust_variant_ident(&variant.source_name);
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

fn type_id_suffix(type_id: uuid::Uuid) -> String {
    type_id.as_simple().to_string().chars().take(8).collect()
}

fn relative_typescript_import(from_dir: &str, target: &str) -> String {
    let from_parts = from_dir
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let target_parts = target
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let common_len = from_parts
        .iter()
        .zip(&target_parts)
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts = vec![".."; from_parts.len().saturating_sub(common_len)];
    parts.extend(target_parts[common_len..].iter().copied());
    let mut path = if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    };
    if !path.starts_with('.') {
        path.insert_str(0, "./");
    }
    path.push_str(".js");
    path
}

fn format_typescript_source(source: &str) -> Result<String, TypeScriptSourceEmitError> {
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
        ..oxc_codegen::CodegenOptions::default()
    };
    let formatted = oxc_codegen::Codegen::new()
        .with_options(options)
        .build(&parsed.program)
        .code;
    Ok(viteplus_style_typescript_source(
        &space_typescript_top_level_declarations(&formatted),
    ))
}

fn space_typescript_top_level_declarations(source: &str) -> String {
    let mut out = String::new();
    let mut last_line_class = TypeScriptTopLevelLine::Other;
    let mut last_output_line_was_blank = true;

    for line in source.lines() {
        let current_line_class = classify_typescript_top_level_line(line);
        if current_line_class.starts_statement()
            && !last_output_line_was_blank
            && !TypeScriptTopLevelLine::same_group(last_line_class, current_line_class)
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

const TYPESCRIPT_LINE_WIDTH: usize = 100;

fn viteplus_style_typescript_source(source: &str) -> String {
    let source = support_viteplus_style_fixes(source);
    let source = wrap_long_typescript_rtti_consts(&source);
    let source = wrap_long_typescript_registrations(&source);
    let source = wrap_long_typescript_class_declarations(&source);
    wrap_long_typescript_reexports(&source)
}

fn wrap_long_typescript_rtti_consts(source: &str) -> String {
    source
        .lines()
        .map(|line| wrap_long_typescript_rtti_const_line(line).unwrap_or_else(|| line.to_owned()))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn wrap_long_typescript_rtti_const_line(line: &str) -> Option<String> {
    if line.len() <= TYPESCRIPT_LINE_WIDTH
        || !line.starts_with("export const ")
        || !line.contains(": Rtti = Rtti.fromTypeId(")
        || !line.ends_with(");")
    {
        return None;
    }

    let (prefix, args) = line.split_once("Rtti.fromTypeId(")?;
    let args = args.strip_suffix(");")?;
    let (name, type_id) = args.split_once(", ")?;
    Some(format!(
        "{prefix}Rtti.fromTypeId(\n  {name},\n  {type_id},\n);"
    ))
}

fn wrap_long_typescript_reexports(source: &str) -> String {
    source
        .lines()
        .map(|line| wrap_long_typescript_reexport_line(line).unwrap_or_else(|| line.to_owned()))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn wrap_long_typescript_registrations(source: &str) -> String {
    source
        .lines()
        .map(|line| wrap_long_typescript_registration_line(line).unwrap_or_else(|| line.to_owned()))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn wrap_long_typescript_class_declarations(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            wrap_long_typescript_class_declaration_line(line).unwrap_or_else(|| line.to_owned())
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn wrap_long_typescript_class_declaration_line(line: &str) -> Option<String> {
    if line.len() <= TYPESCRIPT_LINE_WIDTH || !line.starts_with("export ") || !line.ends_with(" {")
    {
        return None;
    }

    let line = line.strip_suffix(" {")?;
    let (class_head, implements) = line.split_once(" extends AzRtti implements ")?;
    Some(format!(
        "{class_head}\n  extends AzRtti\n  implements {implements}\n{{"
    ))
}

fn wrap_long_typescript_registration_line(line: &str) -> Option<String> {
    if line.len() <= TYPESCRIPT_LINE_WIDTH
        || !line.starts_with("registerType(")
        || !line.ends_with(");")
    {
        return None;
    }

    let args = line.strip_prefix("registerType(")?.strip_suffix(");")?;
    let (type_name, rtti_name) = args.split_once(", ")?;
    Some(format!("registerType(\n  {type_name},\n  {rtti_name},\n);"))
}

fn wrap_long_typescript_reexport_line(line: &str) -> Option<String> {
    if line.len() <= TYPESCRIPT_LINE_WIDTH {
        return None;
    }

    let (keyword, rest) = if let Some(rest) = line.strip_prefix("export type { ") {
        ("export type", rest)
    } else if let Some(rest) = line.strip_prefix("export { ") {
        ("export", rest)
    } else {
        return None;
    };
    let (names, module) = rest.split_once(" } from ")?;
    let module = module.strip_suffix(';')?;
    let names = names
        .split(", ")
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if names.is_empty() {
        return None;
    }

    let mut out = String::new();
    out.push_str(keyword);
    out.push_str(" {\n");
    for name in names {
        out.push_str("  ");
        out.push_str(name);
        out.push_str(",\n");
    }
    out.push_str("} from ");
    out.push_str(module);
    out.push(';');
    Some(out)
}

fn support_viteplus_style_fixes(source: &str) -> String {
    source
        .replace(
            "constructor(readonly name: string, readonly typeId: Uuid) {}",
            "constructor(\n    readonly name: string,\n    readonly typeId: Uuid,\n  ) {}",
        )
        .replace(
            "constructor(readonly guid: Uuid, readonly subId: number) {}",
            "constructor(\n    readonly guid: Uuid,\n    readonly subId: number,\n  ) {}",
        )
        .replace(
            "constructor(readonly assetId: AssetId, readonly assetType: Uuid, readonly hint?: string) {}",
            "constructor(\n    readonly assetId: AssetId,\n    readonly assetType: Uuid,\n    readonly hint?: string,\n  ) {}",
        )
        .replace(
            "constructor(readonly bytes: Uint8Array, readonly length: Length) {",
            "constructor(\n    readonly bytes: Uint8Array,\n    readonly length: Length,\n  ) {",
        )
        .replace(
            "return new Uuid(`${hex.slice(0, 4).join(\"\")}-${hex.slice(4, 6).join(\"\")}-${hex.slice(6, 8).join(\"\")}-${hex.slice(8, 10).join(\"\")}-${hex.slice(10, 16).join(\"\")}`);",
            "return new Uuid(\n      `${hex.slice(0, 4).join(\"\")}-${hex.slice(4, 6).join(\"\")}-${hex.slice(6, 8).join(\"\")}-${hex.slice(8, 10).join(\"\")}-${hex.slice(10, 16).join(\"\")}`,\n    );",
        )
        .replace(
            "data[8] = data[8] & 191 | 128;",
            "data[8] = (data[8] & 191) | 128;",
        )
        .replace(
            "data[6] = data[6] & 95 | 80;",
            "data[6] = (data[6] & 95) | 80;",
        )
        .replace(
            "static async specializedTemplatePrefix(templateBase: Uuid, args: readonly Uuid[]): Promise<Uuid | undefined> {",
            "static async specializedTemplatePrefix(\n    templateBase: Uuid,\n    args: readonly Uuid[],\n  ): Promise<Uuid | undefined> {",
        )
        .replace(
            "static async specializedTemplatePostfix(templateBase: Uuid, args: readonly Uuid[]): Promise<Uuid | undefined> {",
            "static async specializedTemplatePostfix(\n    templateBase: Uuid,\n    args: readonly Uuid[],\n  ): Promise<Uuid | undefined> {",
        )
        .replace(
            "tableValue = tableValue & 1 ? 3988292384 ^ tableValue >>> 1 : tableValue >>> 1;",
            "tableValue = tableValue & 1 ? 3988292384 ^ (tableValue >>> 1) : tableValue >>> 1;",
        )
        .replace(
            "return (currentCrc >>> 8 ^ tableValue) >>> 0;",
            "return ((currentCrc >>> 8) ^ tableValue) >>> 0;",
        )
        .replace(
            "export type FixedArray<\n  T,\n  Length extends number\n> = T[] & {",
            "export type FixedArray<T, Length extends number> = T[] & {",
        )
        .replace(
            "export type FixedVector<\n  T,\n  _Capacity extends number\n> = T[];",
            "export type FixedVector<T, _Capacity extends number> = T[];",
        )
        .replace(
            "throw new Error(`AZ RTTI type id ${typeId} already registered for ${existingById.rtti.name}`);",
            "throw new Error(\n          `AZ RTTI type id ${typeId} already registered for ${existingById.rtti.name}`,\n        );",
        )
        .replace(
            "if (!existingByTarget.rtti.typeId.equals(rtti.typeId) || existingByTarget.rtti.name !== rtti.name) {",
            "if (\n        !existingByTarget.rtti.typeId.equals(rtti.typeId) ||\n        existingByTarget.rtti.name !== rtti.name\n      ) {",
        )
        .replace(
            "throw new Error(`target already registered as ${existingByTarget.rtti.name} (${existingByTarget.rtti.typeId.toString()})`);",
            "throw new Error(\n          `target already registered as ${existingByTarget.rtti.name} (${existingByTarget.rtti.typeId.toString()})`,\n        );",
        )
        .replace(
            "const registration = {\n      target,\n      rtti\n    } satisfies RttiRegistration<T>;",
            "const registration = {\n      target,\n      rtti,\n    } satisfies RttiRegistration<T>;",
        )
        .replace(
            "u8: Uuid.parse(\"72b9409a-7d1a-4831-9cfe-fcb3fadd3426\")",
            "u8: Uuid.parse(\"72b9409a-7d1a-4831-9cfe-fcb3fadd3426\"),",
        )
        .replace("entry: [\"src/index.ts\"]\n", "entry: [\"src/index.ts\"],\n")
        .replace("typeCheck: true\n    }", "typeCheck: true,\n    },")
        .replace(
            "fmt: { ignorePatterns: [\"dist/**\", \"node_modules/**\"] }\n",
            "fmt: { ignorePatterns: [\"dist/**\", \"node_modules/**\"] },\n",
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeScriptTopLevelLine {
    Import,
    Reexport,
    Declaration,
    Other,
}

impl TypeScriptTopLevelLine {
    const fn starts_statement(self) -> bool {
        matches!(self, Self::Import | Self::Reexport | Self::Declaration)
    }

    const fn same_group(left: Self, right: Self) -> bool {
        matches!(
            (left, right),
            (Self::Import, Self::Import) | (Self::Reexport, Self::Reexport)
        )
    }
}

fn classify_typescript_top_level_line(line: &str) -> TypeScriptTopLevelLine {
    if line.starts_with(' ') || line.starts_with('\t') {
        return TypeScriptTopLevelLine::Other;
    }

    if line.starts_with("import ") {
        return TypeScriptTopLevelLine::Import;
    }
    if line.starts_with("export {") || line.starts_with("export type {") {
        return TypeScriptTopLevelLine::Reexport;
    }
    if line.starts_with("export ")
        || line.starts_with("type ")
        || line.starts_with("interface ")
        || line.starts_with("class ")
        || line.starts_with("const ")
        || line.starts_with("function ")
    {
        return TypeScriptTopLevelLine::Declaration;
    }

    TypeScriptTopLevelLine::Other
}

fn typescript_field_name(source_name: &str) -> String {
    let rust_name = rust_field_ident(source_name);
    let mut parts = rust_name.split('_');
    let Some(first) = parts.next() else {
        return "field".to_owned();
    };
    let mut out = first.to_owned();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    if is_typescript_keyword(&out) {
        out.push('_');
    }
    out
}

fn unique_typescript_field_name(
    field: &crate::ir::SerializeCodegenField,
    used: &mut BTreeMap<String, usize>,
) -> String {
    let base = typescript_field_name(&field.source_name);
    unique_typescript_field_name_from_base(&base, field, used)
}

fn unique_typescript_field_name_from_base(
    base: &str,
    field: &crate::ir::SerializeCodegenField,
    used: &mut BTreeMap<String, usize>,
) -> String {
    if !used.contains_key(base) {
        used.insert(base.to_owned(), 1);
        return base.to_owned();
    }

    let suffix = field
        .offset
        .map(|offset| format!("Offset{offset}"))
        .unwrap_or_else(|| format!("Type{}", type_id_suffix(field.source_type_id)));
    let mut candidate = format!("{base}{suffix}");
    while used.contains_key(&candidate) {
        let next_index = used.get(base).copied().unwrap_or(1) + 1;
        used.insert(base.to_owned(), next_index);
        candidate = format!("{base}Field{next_index}");
    }
    used.insert(candidate.clone(), 1);
    candidate
}

fn typescript_string_literal(value: &str) -> String {
    format!("{value:?}")
}

fn is_typescript_keyword(value: &str) -> bool {
    matches!(
        value,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
    )
}

fn reject_unresolved_types(unit: &SerializeCodegenUnit) -> Result<(), TypeScriptSourceEmitError> {
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
                return Err(TypeScriptSourceEmitError::UnresolvedType {
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
            return Err(TypeScriptSourceEmitError::UnresolvedType {
                item_name: item.source_name.clone(),
                field_name: "<enum_underlying>".to_owned(),
                type_id: unresolved.type_id,
                reason: unresolved.reason.to_owned(),
            });
        }
    }
    Ok(())
}

fn needs_container_aliases(unit: &SerializeCodegenUnit) -> bool {
    CodegenContainerSupportUsage::for_unit_fields(unit).any()
}

fn typescript_collection_alias_names(
    usage: CodegenContainerSupportUsage,
) -> BTreeSet<&'static str> {
    let mut names = BTreeSet::new();
    if usage.bit_set {
        names.insert("BitSet");
    }
    if usage.fixed_array {
        names.insert("FixedArray");
    }
    if usage.fixed_bytes {
        names.insert("FixedBytes");
    }
    if usage.fixed_vector {
        names.insert("FixedVector");
    }
    names
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
    fn relative_import_uses_common_directory_prefix() {
        assert_eq!(
            relative_typescript_import(
                "types/navigation_profile",
                "types/navigation_profile/nav_configuration",
            ),
            "./nav_configuration.js"
        );
        assert_eq!(
            relative_typescript_import(
                "types/navigation_profile",
                "types/navigation_profile/nav_configurations/parameters/parameters",
            ),
            "./nav_configurations/parameters/parameters.js"
        );
        assert_eq!(
            relative_typescript_import("types/navigation_profile", "az/uuid"),
            "../../az/uuid.js"
        );
        assert_eq!(
            relative_typescript_import("types", "types/navigation_profile/navigation_profile"),
            "./navigation_profile/navigation_profile.js"
        );
    }

    #[test]
    fn formats_top_level_spacing_between_imports_and_declarations() {
        let source = format_typescript_source(
            "import type { A } from './a.js';\n\
             import { B } from './b.js';\n\
             export interface First { value: A; }\n\
             export const Mode = { Enabled: 7 } as const;\n\
             export type Mode = (typeof Mode)[keyof typeof Mode];\n\
             export interface Second { mode: Mode; }\n",
        )
        .expect("formatted TypeScript source");

        assert!(source.contains("import type { A } from"));
        assert!(source.contains(";\nimport { B } from"));
        assert!(
            !source.contains(";\n\nimport { B } from"),
            "import group should stay compact:\n{source}"
        );
        assert!(
            source.contains("\n\nexport interface First"),
            "imports should be separated from declarations:\n{source}"
        );
        assert!(
            source.contains("}\n\nexport const Mode"),
            "declarations should be separated:\n{source}"
        );
        assert!(
            source.contains("} as const;\n\nexport type Mode"),
            "const enum values and type aliases should be separated:\n{source}"
        );
        assert!(
            source.contains(";\n\nexport interface Second"),
            "type aliases and later declarations should be separated:\n{source}"
        );
        assert!(source.ends_with('\n'));
    }

    #[test]
    fn emits_interfaces_and_enums_from_shared_codegen_unit() {
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

        let source = TypeScriptSourceEmitter::emit_unit(&unit).expect("TypeScript source");

        assert!(source.contains("export class Uuid"));
        assert!(source.contains("static async createData(bytes: Uint8Array): Promise<Uuid>"));
        assert!(source.contains("export class Crc32"));
        assert!(source.contains("static fromStringLower(value: string): Crc32"));
        assert!(!source.contains("export class EntityId"));
        assert!(source.contains("export class CounterComponent extends AzRtti"));
        assert!(source.contains("override readonly azRtti = CounterComponentRtti;"));
        assert!(source.contains("declare targetEntity: bigint;"));
        assert!(source.contains("registerType(CounterComponent, CounterComponentRtti);"));
        assert!(source.contains("export const Mode = {"));
        assert!(source.contains("export type Mode = (typeof Mode)[keyof typeof Mode];"));
        assert!(source.contains("registerType(Mode, ModeRtti);"));
        assert!(source.contains("Enabled: 7"));
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

        let source = TypeScriptSourceEmitter::emit_unit(&unit).expect("TypeScript source");
        let leaf = source.find("class Leaf").expect("Leaf type");
        let consumer = source.find("class Consumer").expect("Consumer type");

        assert!(
            leaf < consumer,
            "dependency should be emitted before dependent:\n{source}"
        );
    }

    #[test]
    fn can_emit_without_support_aliases_for_standalone_wire_shapes() {
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
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
            }],
        };

        let source = TypeScriptSourceEmitter::default()
            .emit_with_options(
                &unit,
                &TypeScriptSourceOptions {
                    include_support_aliases: false,
                },
            )
            .expect("TypeScript source");

        assert!(!source.contains("export class EntityId"));
        assert!(!source.contains("export type EntityId"));
        assert!(source.contains("targetEntity: bigint;"));
    }

    #[test]
    fn fixed_opaque_bytes_remain_self_contained_without_support_aliases() {
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

        let source = TypeScriptSourceEmitter::default()
            .emit_with_options(
                &unit,
                &TypeScriptSourceOptions {
                    include_support_aliases: false,
                },
            )
            .expect("TypeScript source");

        assert!(source.contains("export class FixedBytes"), "{source}");
        assert!(source.contains("payload: FixedBytes<16>;"), "{source}");
        assert!(!source.contains("export class Uuid"), "{source}");
        assert!(!source.contains("Typeaaaaaaaa"), "{source}");
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

        let source = TypeScriptSourceEmitter::emit_unit(&unit).expect("TypeScript source");

        assert!(source.contains("export type FixedArray<"), "{source}");
        assert!(source.contains("readonly length: Length;"), "{source}");
        assert!(source.contains("export type FixedVector<"), "{source}");
        assert!(
            source.contains("export type BitSet<Size extends number>"),
            "{source}"
        );
        assert!(
            source.contains("routes: Map<bigint, FixedArray<[AddressType, number | null], 2>>;")
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

        let unit_source = TypeScriptSourceEmitter::emit_unit(&unit).expect("TypeScript source");
        assert!(
            unit_source.contains("export class FixedBytes"),
            "{unit_source}"
        );
        assert!(
            unit_source.contains("payload: FixedBytes<16>;"),
            "{unit_source}"
        );
        assert!(!unit_source.contains("Typeaaaaaaaa"), "{unit_source}");

        let context = crate::CodegenContext::inline();
        let project = TypeScriptSourceEmitter
            .emit_standalone_project(&unit, &context)
            .expect("TypeScript project");
        let source = project
            .files
            .iter()
            .find(|file| file.path.ends_with("opaque_payload.ts"))
            .expect("opaque payload file")
            .source
            .as_str();

        assert!(source.contains("import type { FixedBytes }"), "{source}");
        assert!(source.contains("payload: FixedBytes<16>;"), "{source}");
        assert!(!source.contains("types/missing/types"), "{source}");
        assert!(
            !project
                .files
                .iter()
                .any(|file| file.path == "src/types/missing/types.ts")
        );
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
        let project = TypeScriptSourceEmitter
            .emit_standalone_project_with_context(&emitted_unit, &context_unit, &context)
            .expect("standalone TypeScript project");
        let sources = project
            .files
            .iter()
            .map(|file| file.source.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(sources.contains("export class Shared extends AzRtti"));
        assert!(!sources.contains("SharedCCCCCCCC"));
        assert!(!sources.contains("SharedDDDDDDDD"));
    }

    #[test]
    fn standalone_project_keeps_duplicate_leaf_names_in_distinct_files() {
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
        let project = TypeScriptSourceEmitter
            .emit_standalone_project(&unit, &context)
            .expect("standalone TypeScript project");
        let catalog_item = project
            .files
            .iter()
            .find(|file| file.path == "src/types/catalog/item.ts")
            .expect("catalog item file");
        let runtime_item = project
            .files
            .iter()
            .find(|file| file.path == "src/types/runtime/item.ts")
            .expect("runtime item file");
        let holder = project
            .files
            .iter()
            .find(|file| file.path == "src/types/runtime/holder.ts")
            .expect("holder file");
        let package_index = project
            .files
            .iter()
            .find(|file| file.path == "src/index.ts")
            .expect("package index");
        let root_types_index = project
            .files
            .iter()
            .find(|file| file.path == "src/types/index.ts")
            .expect("root types index");
        let catalog_index = project
            .files
            .iter()
            .find(|file| file.path == "src/types/catalog/index.ts")
            .expect("catalog index");
        let runtime_index = project
            .files
            .iter()
            .find(|file| file.path == "src/types/runtime/index.ts")
            .expect("runtime index");

        assert!(
            catalog_item
                .source
                .contains("export class Item extends AzRtti")
        );
        assert!(
            runtime_item
                .source
                .contains("export class Item extends AzRtti")
        );
        assert!(!catalog_item.source.contains("Item11111111"));
        assert!(!runtime_item.source.contains("Item22222222"));
        assert!(
            holder
                .source
                .contains("import type { Item } from \"../catalog/item.js\";")
        );
        assert!(
            holder
                .source
                .contains("import type { Item as RuntimeItem } from \"./item.js\";")
        );
        assert!(holder.source.contains("declare catalogItem: Item;"));
        assert!(holder.source.contains("declare runtimeItem: RuntimeItem;"));
        assert!(
            package_index
                .source
                .contains("export * as az from \"./az/index.js\";")
        );
        assert!(
            package_index
                .source
                .contains("export { Asset, AssetId } from \"./az/asset.js\";")
        );
        assert!(
            package_index
                .source
                .contains("export { Crc32 } from \"./az/crc.js\";")
        );
        assert!(
            package_index
                .source
                .contains("export { Uuid, typeIds } from \"./az/uuid.js\";")
        );
        assert!(catalog_index.source.contains("export { Item"));
        assert!(runtime_index.source.contains("export { Holder"));
        assert!(runtime_index.source.contains("export { Item"));
        assert!(root_types_index.source.contains("Holder"));
        assert!(!root_types_index.source.contains("Item"));
        assert!(!root_types_index.source.contains("export * from"));
    }

    #[test]
    fn standalone_project_imports_only_used_math_aliases() {
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Plane".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![SerializeCodegenField {
                    source_name: "m_normal".to_owned(),
                    source_type_id: uuid!("00000000-0000-0000-0000-000000000002"),
                    resolved_type: ResolvedType::Scalar(ScalarType::Vector2),
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

        let context = crate::CodegenContext::inline();
        let project = TypeScriptSourceEmitter
            .emit_standalone_project(&unit, &context)
            .expect("TypeScript standalone project");
        let source = project
            .files
            .iter()
            .find(|file| file.path.ends_with("plane.ts"))
            .expect("plane type file")
            .source
            .as_str();

        assert!(source.contains("import type { Vector2 } from"), "{source}");
        assert!(source.contains("normal: Vector2;"), "{source}");
        assert!(!source.contains("Color"), "{source}");
        assert!(!source.contains("Transform"), "{source}");
        assert!(!source.contains("Vector3"), "{source}");
    }

    #[test]
    fn emits_unique_enum_member_names_for_duplicate_reflected_variant_names() {
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

        let source = TypeScriptSourceEmitter::emit_unit(&unit).expect("TypeScript source");

        assert!(source.contains("Level: 1"));
        assert!(source.contains("LevelValue2: 2"));
    }

    #[test]
    fn standalone_project_places_namespace_before_component_family() {
        let unit = namespace_and_base_family_fixture();

        let context = crate::CodegenContext::inline();
        let project = TypeScriptSourceEmitter
            .emit_standalone_project(&unit, &context)
            .expect("TypeScript standalone project");
        let paths = project
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(paths.contains("src/types/az_framework/components/input_system_component.ts"));
        assert!(paths.contains("src/types/az_framework/input_device_id.ts"));
        assert!(paths.contains("src/types/action_conditions/action_condition.ts"));
        assert!(paths.contains("src/types/action_conditions/action_condition_if_input.ts"));
        assert!(paths.contains("src/types/components/faceted_components/facet.ts"));
        assert!(paths.contains("src/types/components/faceted_components/client_facet.ts"));
        assert!(paths.contains(
            "src/types/components/faceted_components/inventory_component/inventory_component.ts"
        ));
        assert!(paths.contains(
            "src/types/components/faceted_components/inventory_component/client_facet.ts"
        ));
        assert!(
            !paths
                .iter()
                .any(|path| path.starts_with("src/types/components/az_framework"))
        );
        assert!(!paths.contains("src/types/facets/client_facets/inventory_client_facet.ts"));
        assert!(
            !paths
                .iter()
                .any(|path| path.starts_with("src/types/components/faceted_components/facets"))
        );
        assert!(!paths.contains(
            "src/types/components/faceted_components/facets/client_facets/inventory_client_facet.ts"
        ));
        assert!(!paths.contains("src/types/global/action_condition.ts"));
    }

    #[test]
    fn standalone_layout_report_exposes_type_files_and_indexes_without_emitting_sources() {
        let unit = namespace_and_base_family_fixture();

        let report = TypeScriptSourceEmitter::standalone_layout_report(&unit);
        let text = report.to_text();
        let type_paths = report
            .type_files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let index_paths = report
            .index_files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(type_paths.contains("src/types/az_framework/components/input_system_component.ts"));
        assert!(type_paths.contains("src/types/az_framework/input_device_id.ts"));
        assert!(type_paths.contains("src/types/action_conditions/action_condition.ts"));
        assert!(type_paths.contains("src/types/action_conditions/action_condition_if_input.ts"));
        assert!(type_paths.contains("src/types/components/faceted_components/facet.ts"));
        assert!(type_paths.contains("src/types/components/faceted_components/client_facet.ts"));
        assert!(type_paths.contains(
            "src/types/components/faceted_components/inventory_component/client_facet.ts"
        ));
        assert!(index_paths.contains("src/types/index.ts"));
        assert!(index_paths.contains("src/types/action_conditions/index.ts"));
        assert!(
            index_paths
                .contains("src/types/components/faceted_components/inventory_component/index.ts")
        );
        assert!(text.contains("file src/types/action_conditions/action_condition_if_input.ts"));
        assert!(text.contains(
            "file src/types/components/faceted_components/inventory_component/client_facet.ts"
        ));
        assert!(text.contains("index src/types/action_conditions/index.ts"));
        assert!(text.contains("  export ./action_condition_if_input.js"));
        assert!(text.contains("  export ./client_facet.js"));
        assert!(text.contains("  item ActionConditionIfInput "));
        assert!(text.contains("  item InventoryClientFacet "));
        assert!(
            report
                .type_files
                .iter()
                .flat_map(|file| &file.items)
                .any(|item| {
                    item.type_name == "InputSystemComponent"
                        && item.source_name == "AzFramework::InputSystemComponent"
                })
        );
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

        let err = TypeScriptSourceEmitter::emit_unit(&unit).expect_err("unresolved type");

        assert!(matches!(
            err,
            TypeScriptSourceEmitError::UnresolvedType { field_name, .. }
                if field_name == "m_values"
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
}
