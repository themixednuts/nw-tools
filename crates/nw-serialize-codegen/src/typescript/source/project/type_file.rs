use std::collections::{BTreeMap, BTreeSet};

use heck::ToUpperCamelCase;

use crate::CodegenContext;
use crate::field_projection::{
    CodegenFieldProjection, CodegenFieldTypeProjection, CodegenTypeReferenceProjection,
    classify_codegen_field, classify_codegen_field_type, codegen_item_referenced_type_ids,
};
use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit};
use crate::layout::source_namespace_segments;
use crate::naming::{rust_type_ident, scoped_rust_type_names_by_id};
use crate::support_usage::{CodegenContainerSupportUsage, CodegenSupportUsage};
use crate::types::{ResolvedType, ScalarType, SequenceKind};
use crate::typescript::layout::{
    TypeScriptTypeFile, items_by_type_id, standalone_type_layout_with_context,
    typescript_type_source_path,
};
use crate::typescript::types::{element_needs_array_parens, parenthesize_array_element};

use super::super::{
    TypeScriptSourceEmitError, emit_typescript_const_enum_like, emit_typescript_rtti_const,
    emit_typescript_rtti_registration, format_typescript_source, reject_unresolved_types,
    relative_typescript_import, type_id_suffix, typescript_collection_alias_names,
    typescript_enum_variant_names, typescript_field_name, typescript_rtti_const_name,
    typescript_string_literal, unique_typescript_field_name,
    unique_typescript_field_name_from_base,
};
use super::TypeScriptStandaloneProjectFile;
use super::index::{append_typescript_index_reexport, typescript_index_reexports_by_dir};

impl crate::typescript::source::TypeScriptSourceEmitter {
    pub(super) fn emit_project_type_files_with_context(
        &self,
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
        context: &CodegenContext,
    ) -> Result<Vec<TypeScriptStandaloneProjectFile>, TypeScriptSourceEmitError> {
        reject_unresolved_types(emitted_unit)?;
        let items_by_type_id = items_by_type_id(context_unit);
        let layout = standalone_type_layout_with_context(emitted_unit, context_unit);
        let files_by_type_id = layout.files_by_type_id;
        let names_by_type_id = codegen_type_names_by_id_in_files(emitted_unit, &files_by_type_id);
        let groups = layout.groups;
        let index_reexports_by_dir =
            typescript_index_reexports_by_dir(emitted_unit, &groups, &names_by_type_id);
        let group_tasks = groups.into_iter().collect::<Vec<_>>();
        let mut files = context.runner().try_map(&group_tasks, |task| {
            let ((type_file, _bucket), items) = task;
            Ok(TypeScriptStandaloneProjectFile {
                path: typescript_type_source_path(type_file),
                source: self.emit_project_type_file(
                    items,
                    &items_by_type_id,
                    &names_by_type_id,
                    &files_by_type_id,
                    type_file,
                )?,
            })
        })?;

        let index_tasks = index_reexports_by_dir.into_iter().collect::<Vec<_>>();
        let index_files = context.runner().try_map(&index_tasks, |(dir, reexports)| {
            let mut source = String::new();
            for reexport in reexports {
                append_typescript_index_reexport(&mut source, &reexport);
            }
            if source.trim().is_empty() {
                source.push_str("export {};\n");
            }
            Ok(TypeScriptStandaloneProjectFile {
                path: format!("src/{dir}/index.ts"),
                source: format_typescript_source(&source)?,
            })
        })?;
        files.extend(index_files);

        if !files.iter().any(|file| file.path == "src/types/index.ts") {
            files.push(TypeScriptStandaloneProjectFile {
                path: "src/types/index.ts".to_owned(),
                source: String::new(),
            });
        }

        Ok(files)
    }

    fn emit_project_type_file(
        &self,
        items: &[&SerializeCodegenItem],
        items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
        names_by_type_id: &BTreeMap<uuid::Uuid, String>,
        files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
        current_file: &TypeScriptTypeFile,
    ) -> Result<String, TypeScriptSourceEmitError> {
        let mut out = String::new();
        let reference_names_by_type_id = typescript_reference_names_for_items(
            items,
            items_by_type_id,
            names_by_type_id,
            files_by_type_id,
            current_file,
        );
        emit_project_type_file_imports(
            items,
            items_by_type_id,
            names_by_type_id,
            &reference_names_by_type_id,
            files_by_type_id,
            current_file,
            &mut out,
        );
        for item in items {
            match item.kind {
                SerializeCodegenItemKind::Struct => self.emit_project_struct_for_file(
                    item,
                    items_by_type_id,
                    names_by_type_id,
                    &reference_names_by_type_id,
                    files_by_type_id,
                    current_file,
                    &mut out,
                ),
                SerializeCodegenItemKind::Enum => {
                    self.emit_project_enum(item, names_by_type_id, &mut out);
                }
            }
            out.push('\n');
        }
        format_typescript_source(&out)
    }

    fn emit_project_struct_for_file(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
        names_by_type_id: &BTreeMap<uuid::Uuid, String>,
        reference_names_by_type_id: &BTreeMap<uuid::Uuid, String>,
        files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
        current_file: &TypeScriptTypeFile,
        out: &mut String,
    ) {
        let type_name = typescript_type_name(item, names_by_type_id);
        emit_typescript_rtti_const(item, &type_name, out);
        out.push_str("export ");
        if item.is_abstract == Some(true) {
            out.push_str("abstract ");
        }
        out.push_str("class ");
        out.push_str(&type_name);
        out.push_str(" extends AzRtti");
        let bases = item
            .fields
            .iter()
            .filter(|field| field.is_base_class)
            .filter(|field| base_class_should_extend(field, items_by_type_id))
            .map(|field| {
                render_project_typescript_type_for_file(
                    &field.resolved_type,
                    names_by_type_id,
                    reference_names_by_type_id,
                    files_by_type_id,
                    current_file,
                )
            })
            .collect::<Vec<_>>();
        if !bases.is_empty() {
            out.push_str(" implements ");
            out.push_str(&bases.join(", "));
        }
        out.push_str(" {\n\toverride readonly azRtti = ");
        out.push_str(&typescript_rtti_const_name(&type_name));
        out.push_str(";\n");
        let mut used_field_names = BTreeMap::new();
        for field in &item.fields {
            if field.is_base_class {
                if !base_class_has_materialized_payload(field, items_by_type_id) {
                    continue;
                }
                out.push('\t');
                out.push_str("declare ");
                out.push_str(&unique_typescript_field_name_from_base(
                    &base_class_field_name(&field.resolved_type),
                    field,
                    &mut used_field_names,
                ));
                out.push_str(": ");
                out.push_str(&render_project_typescript_field_type_for_file(
                    field,
                    names_by_type_id,
                    reference_names_by_type_id,
                    files_by_type_id,
                    current_file,
                ));
                out.push_str(";\n");
                continue;
            }
            out.push('\t');
            out.push_str("declare ");
            out.push_str(&unique_typescript_field_name(field, &mut used_field_names));
            out.push_str(": ");
            out.push_str(&render_project_typescript_field_type_for_file(
                field,
                names_by_type_id,
                reference_names_by_type_id,
                files_by_type_id,
                current_file,
            ));
            out.push_str(";\n");
        }
        out.push_str("}\n");
        emit_typescript_rtti_registration(&type_name, out);
    }

    fn emit_project_enum(
        &self,
        item: &SerializeCodegenItem,
        names_by_type_id: &BTreeMap<uuid::Uuid, String>,
        out: &mut String,
    ) {
        let variant_names = typescript_enum_variant_names(item);
        let type_name = typescript_type_name(item, names_by_type_id);
        emit_typescript_rtti_const(item, &type_name, out);
        emit_typescript_const_enum_like(item, &type_name, &variant_names, out);
        emit_typescript_rtti_registration(&type_name, out);
    }
}

fn render_project_typescript_type_for_file(
    resolved: &ResolvedType,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    reference_names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
    current_file: &TypeScriptTypeFile,
) -> String {
    match resolved {
        ResolvedType::Scalar(scalar) => render_project_typescript_scalar(*scalar).to_owned(),
        ResolvedType::Named {
            type_id,
            source_name,
        } => reference_names_by_type_id
            .get(type_id)
            .or_else(|| names_by_type_id.get(type_id))
            .cloned()
            .unwrap_or_else(|| rust_type_ident(source_name)),
        ResolvedType::Sequence {
            kind,
            element,
            capacity,
        } => render_project_typescript_sequence_for_file(
            *kind,
            element,
            *capacity,
            names_by_type_id,
            reference_names_by_type_id,
            files_by_type_id,
            current_file,
        ),
        ResolvedType::Map { key, value, .. } => {
            format!(
                "Map<{}, {}>",
                render_project_typescript_type_for_file(
                    key,
                    names_by_type_id,
                    reference_names_by_type_id,
                    files_by_type_id,
                    current_file,
                ),
                render_project_typescript_type_for_file(
                    value,
                    names_by_type_id,
                    reference_names_by_type_id,
                    files_by_type_id,
                    current_file,
                )
            )
        }
        ResolvedType::Asset { .. } => "Asset".to_owned(),
        ResolvedType::Uid { .. } => "Uuid".to_owned(),
        ResolvedType::ReplicatedField { value } => {
            format!(
                "{} | undefined",
                render_project_typescript_type_for_file(
                    value,
                    names_by_type_id,
                    reference_names_by_type_id,
                    files_by_type_id,
                    current_file,
                )
            )
        }
        ResolvedType::RangedInteger { value, .. } => render_project_typescript_type_for_file(
            value,
            names_by_type_id,
            reference_names_by_type_id,
            files_by_type_id,
            current_file,
        ),
        ResolvedType::ByteStream => "Uint8Array".to_owned(),
        ResolvedType::Pair { first, second } => {
            format!(
                "[{}, {}]",
                render_project_typescript_type_for_file(
                    first,
                    names_by_type_id,
                    reference_names_by_type_id,
                    files_by_type_id,
                    current_file,
                ),
                render_project_typescript_type_for_file(
                    second,
                    names_by_type_id,
                    reference_names_by_type_id,
                    files_by_type_id,
                    current_file,
                )
            )
        }
        ResolvedType::Pointer { target, .. } => render_project_nullable_type_for_file(
            target,
            names_by_type_id,
            reference_names_by_type_id,
            files_by_type_id,
            current_file,
        ),
        ResolvedType::Optional { value } => render_project_nullable_type_for_file(
            value,
            names_by_type_id,
            reference_names_by_type_id,
            files_by_type_id,
            current_file,
        ),
        ResolvedType::Tuple { elements } => format!(
            "[{}]",
            elements
                .iter()
                .map(|element| {
                    render_project_typescript_type_for_file(
                        element,
                        names_by_type_id,
                        reference_names_by_type_id,
                        files_by_type_id,
                        current_file,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ResolvedType::Unknown { .. } => {
            unreachable!("unresolved reflected types are rejected before TypeScript rendering")
        }
    }
}

fn render_project_typescript_field_type_for_file(
    field: &crate::ir::SerializeCodegenField,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    reference_names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
    current_file: &TypeScriptTypeFile,
) -> String {
    if let CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len } =
        classify_codegen_field_type(field)
    {
        return format!("FixedBytes<{byte_len}>");
    }
    render_project_typescript_type_for_file(
        &field.resolved_type,
        names_by_type_id,
        reference_names_by_type_id,
        files_by_type_id,
        current_file,
    )
}

fn render_project_typescript_scalar(scalar: ScalarType) -> &'static str {
    match scalar {
        ScalarType::Char
        | ScalarType::SignedChar
        | ScalarType::I8
        | ScalarType::U8
        | ScalarType::I16
        | ScalarType::U16
        | ScalarType::I32
        | ScalarType::U32
        | ScalarType::F32
        | ScalarType::F64 => "number",
        ScalarType::I64 | ScalarType::U64 | ScalarType::UnsignedLong => "bigint",
        ScalarType::Bool => "boolean",
        ScalarType::Uuid => "Uuid",
        ScalarType::Crc32 => "Crc32",
        ScalarType::EntityId => "bigint",
        ScalarType::AssetId => "AssetId",
        ScalarType::Vector2 => "Vector2",
        ScalarType::Vector3 => "Vector3",
        ScalarType::Vector4 => "Vector4",
        ScalarType::Quaternion => "Quaternion",
        ScalarType::Transform => "Transform",
        ScalarType::Color => "Color",
        ScalarType::ColorF => "ColorF",
        ScalarType::ColorB => "ColorB",
        ScalarType::String => "string",
    }
}

fn render_project_typescript_sequence_for_file(
    kind: SequenceKind,
    element: &ResolvedType,
    capacity: Option<usize>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    reference_names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
    current_file: &TypeScriptTypeFile,
) -> String {
    let element = render_project_typescript_type_for_file(
        element,
        names_by_type_id,
        reference_names_by_type_id,
        files_by_type_id,
        current_file,
    );
    match (kind, capacity) {
        (SequenceKind::Array, Some(capacity)) => format!("FixedArray<{element}, {capacity}>"),
        (SequenceKind::FixedVector, Some(capacity)) => {
            format!("FixedVector<{element}, {capacity}>")
        }
        (SequenceKind::BitSet, Some(capacity)) => format!("BitSet<{capacity}>"),
        (SequenceKind::Set | SequenceKind::UnorderedSet, _) => format!("Set<{element}>"),
        _ => {
            let needs_parens = element_needs_array_parens(&element);
            format!("{}[]", parenthesize_array_element(element, needs_parens))
        }
    }
}

fn render_project_nullable_type_for_file(
    value: &ResolvedType,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    reference_names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
    current_file: &TypeScriptTypeFile,
) -> String {
    let rendered = render_project_typescript_type_for_file(
        value,
        names_by_type_id,
        reference_names_by_type_id,
        files_by_type_id,
        current_file,
    );
    if rendered.ends_with(" | null") {
        rendered
    } else {
        format!("{rendered} | null")
    }
}

pub(super) fn typescript_type_name(
    item: &SerializeCodegenItem,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
) -> String {
    names_by_type_id
        .get(&item.source_type_id)
        .cloned()
        .unwrap_or_else(|| rust_type_ident(&item.source_name))
}

fn codegen_type_names_by_id_in_files(
    unit: &SerializeCodegenUnit,
    files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
) -> BTreeMap<uuid::Uuid, String> {
    scoped_rust_type_names_by_id(
        unit.items
            .iter()
            .filter(|item| !item.is_reflection_marker)
            .map(|item| {
                let scope = files_by_type_id
                    .get(&item.source_type_id)
                    .map(|file| {
                        file.dir
                            .split('/')
                            .filter(|segment| !segment.is_empty())
                            .chain(std::iter::once(file.file_stem.as_str()))
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                (item.source_type_id, scope, item.source_name.as_str())
            }),
    )
}

fn typescript_reference_names_for_items(
    items: &[&SerializeCodegenItem],
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
    current_file: &TypeScriptTypeFile,
) -> BTreeMap<uuid::Uuid, String> {
    let mut used_names = items
        .iter()
        .map(|item| typescript_type_name(item, names_by_type_id))
        .collect::<BTreeSet<_>>();
    let mut reference_names = items
        .iter()
        .map(|item| {
            (
                item.source_type_id,
                typescript_type_name(item, names_by_type_id),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut referenced_type_ids = BTreeSet::new();
    for item in items {
        referenced_type_ids.extend(codegen_item_referenced_type_ids(
            item,
            items_by_type_id,
            CodegenTypeReferenceProjection::MaterializedFieldsAndInterfaceEdges,
        ));
    }

    for type_id in referenced_type_ids {
        let Some(item) = items_by_type_id.get(&type_id) else {
            continue;
        };
        let definition_name = typescript_type_name(item, names_by_type_id);
        if files_by_type_id.get(&type_id) == Some(current_file) {
            reference_names.insert(type_id, definition_name);
            continue;
        }
        let local_name =
            unique_typescript_import_name(definition_name, item, type_id, &mut used_names);
        reference_names.insert(type_id, local_name);
    }

    reference_names
}

fn unique_typescript_import_name(
    definition_name: String,
    item: &SerializeCodegenItem,
    type_id: uuid::Uuid,
    used_names: &mut BTreeSet<String>,
) -> String {
    if used_names.insert(definition_name.clone()) {
        return definition_name;
    }

    if let Some(candidate) = typescript_source_qualified_alias_name(item, &definition_name)
        && used_names.insert(candidate.clone())
    {
        return candidate;
    }

    let base = format!("{}{}", definition_name, type_id_suffix(type_id));
    let mut candidate = base.clone();
    let mut index = 2usize;
    while !used_names.insert(candidate.clone()) {
        candidate = format!("{base}{index}");
        index += 1;
    }
    candidate
}

fn typescript_source_qualified_alias_name(
    item: &SerializeCodegenItem,
    definition_name: &str,
) -> Option<String> {
    let namespace = source_namespace_segments(&item.source_name);
    if namespace.is_empty() {
        return None;
    }
    let mut alias = namespace
        .iter()
        .map(|segment| segment.to_upper_camel_case())
        .collect::<String>();
    alias.push_str(definition_name);
    typescript_alias_candidate(alias, definition_name)
}

fn typescript_alias_candidate(alias: String, definition_name: &str) -> Option<String> {
    (!alias.is_empty() && alias != definition_name).then_some(alias)
}

fn emit_project_type_file_imports(
    items: &[&SerializeCodegenItem],
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    reference_names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
    current_file: &TypeScriptTypeFile,
    out: &mut String,
) {
    let support_usage = CodegenSupportUsage::for_items(
        items.iter().copied(),
        items_by_type_id,
        CodegenTypeReferenceProjection::MaterializedFieldsAndInterfaceEdges,
    );
    let mut import_lines = BTreeSet::<String>::new();
    let has_structs = items
        .iter()
        .any(|item| item.kind == SerializeCodegenItemKind::Struct);
    let rtti_imports = if has_structs {
        "AzRtti, Rtti, registerType"
    } else {
        "Rtti, registerType"
    };
    import_lines.insert(format!(
        "import {{ {rtti_imports} }} from {};",
        typescript_string_literal(&relative_typescript_import(&current_file.dir, "az/rtti"))
    ));
    if support_usage.asset {
        import_lines.insert(format!(
            "import type {{ Asset }} from {};",
            typescript_string_literal(&relative_typescript_import(&current_file.dir, "az/asset"))
        ));
    }
    if support_usage.asset_id {
        import_lines.insert(format!(
            "import type {{ AssetId }} from {};",
            typescript_string_literal(&relative_typescript_import(&current_file.dir, "az/asset"))
        ));
    }
    if support_usage.crc32 {
        import_lines.insert(format!(
            "import type {{ Crc32 }} from {};",
            typescript_string_literal(&relative_typescript_import(&current_file.dir, "az/crc"))
        ));
    }
    if support_usage.uuid {
        import_lines.insert(format!(
            "import type {{ Uuid }} from {};",
            typescript_string_literal(&relative_typescript_import(&current_file.dir, "az/uuid"))
        ));
    }
    let math_names = support_usage
        .math_scalars
        .iter()
        .filter_map(|scalar| typescript_math_alias_name(*scalar))
        .collect::<BTreeSet<_>>();
    if !math_names.is_empty() {
        import_lines.insert(format!(
            "import type {{ {} }} from {};",
            math_names.into_iter().collect::<Vec<_>>().join(", "),
            typescript_string_literal(&relative_typescript_import(&current_file.dir, "az/math"))
        ));
    }
    let collection_names =
        typescript_collection_alias_names(CodegenContainerSupportUsage::for_items(
            items.iter().copied(),
            items_by_type_id,
            CodegenTypeReferenceProjection::MaterializedFieldsAndInterfaceEdges,
        ));
    if !collection_names.is_empty() {
        import_lines.insert(format!(
            "import type {{ {} }} from {};",
            collection_names.into_iter().collect::<Vec<_>>().join(", "),
            typescript_string_literal(&relative_typescript_import(
                &current_file.dir,
                "az/collection"
            ))
        ));
    }

    let type_imports = typescript_type_imports_for_items(
        items,
        items_by_type_id,
        names_by_type_id,
        reference_names_by_type_id,
        files_by_type_id,
        current_file,
    );
    for (target_file, names) in type_imports {
        let module = relative_typescript_import(
            &current_file.dir,
            &format!("{}/{}", target_file.dir, target_file.file_stem),
        );
        import_lines.insert(format!(
            "import type {{ {} }} from {};",
            names.into_iter().collect::<Vec<_>>().join(", "),
            typescript_string_literal(&module)
        ));
    }

    for line in import_lines {
        out.push_str(&line);
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
}

fn typescript_type_imports_for_items(
    items: &[&SerializeCodegenItem],
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    reference_names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
    current_file: &TypeScriptTypeFile,
) -> BTreeMap<TypeScriptTypeFile, BTreeSet<String>> {
    let mut imports = BTreeMap::<TypeScriptTypeFile, BTreeSet<String>>::new();
    for item in items {
        for type_id in codegen_item_referenced_type_ids(
            item,
            items_by_type_id,
            CodegenTypeReferenceProjection::MaterializedFieldsAndInterfaceEdges,
        ) {
            let Some(target_file) = files_by_type_id.get(&type_id) else {
                continue;
            };
            if target_file == current_file {
                continue;
            }
            let Some(item) = items_by_type_id.get(&type_id) else {
                continue;
            };
            let imported_name = names_by_type_id
                .get(&type_id)
                .cloned()
                .unwrap_or_else(|| rust_type_ident(&item.source_name));
            let local_name = reference_names_by_type_id
                .get(&type_id)
                .cloned()
                .unwrap_or_else(|| imported_name.clone());
            let import_spec = if local_name == imported_name {
                imported_name
            } else {
                format!("{imported_name} as {local_name}")
            };
            imports
                .entry(target_file.clone())
                .or_default()
                .insert(import_spec);
        }
    }
    imports
}

fn typescript_math_alias_name(scalar: ScalarType) -> Option<&'static str> {
    match scalar {
        ScalarType::Vector2 => Some("Vector2"),
        ScalarType::Vector3 => Some("Vector3"),
        ScalarType::Vector4 => Some("Vector4"),
        ScalarType::Quaternion => Some("Quaternion"),
        ScalarType::Transform => Some("Transform"),
        ScalarType::Color => Some("Color"),
        ScalarType::ColorF => Some("ColorF"),
        ScalarType::ColorB => Some("ColorB"),
        ScalarType::Char
        | ScalarType::SignedChar
        | ScalarType::I8
        | ScalarType::U8
        | ScalarType::I16
        | ScalarType::U16
        | ScalarType::I32
        | ScalarType::U32
        | ScalarType::I64
        | ScalarType::U64
        | ScalarType::UnsignedLong
        | ScalarType::F32
        | ScalarType::F64
        | ScalarType::Bool
        | ScalarType::Uuid
        | ScalarType::Crc32
        | ScalarType::EntityId
        | ScalarType::AssetId
        | ScalarType::String => None,
    }
}

fn base_class_should_extend(
    field: &crate::ir::SerializeCodegenField,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
) -> bool {
    matches!(
        classify_codegen_field(field, items_by_type_id),
        CodegenFieldProjection::InterfaceBase | CodegenFieldProjection::MarkerBaseField
    )
}

fn base_class_has_materialized_payload(
    field: &crate::ir::SerializeCodegenField,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
) -> bool {
    classify_codegen_field(field, items_by_type_id) == CodegenFieldProjection::MaterializedBaseField
}

fn base_class_field_name(resolved: &ResolvedType) -> String {
    let ResolvedType::Named { source_name, .. } = resolved else {
        return "base".to_owned();
    };
    if source_name.contains("::") {
        typescript_field_name(&source_name.replace("::", "_"))
    } else {
        typescript_field_name(&rust_type_ident(source_name))
    }
}
