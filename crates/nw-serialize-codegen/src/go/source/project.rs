use std::collections::{BTreeMap, BTreeSet};

use crate::CodegenContext;
use crate::field_projection::{
    CodegenFieldProjection, CodegenFieldTypeProjection, CodegenTypeReferenceProjection,
    base_class_is_abstract, classify_codegen_field, classify_codegen_field_type,
    codegen_item_referenced_type_ids, item_has_materialized_payload,
};
use crate::go::layout::{
    GoTypePackage, go_package_groups_with_context, go_type_file_path,
    go_type_packages_by_id_with_context, items_by_type_id,
};
use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit};
use crate::layout::reflected_base_type_ids;
use crate::naming::{rust_type_ident, scoped_rust_type_names_by_id};
use crate::support_usage::CodegenSupportUsage;
use crate::types::{ResolvedType, ScalarType, SequenceKind};

use super::{
    GoSourceEmitError, format_go_source, go_enum_variant_names, go_string_literal,
    is_go_identifier, reject_unresolved_types, support, unique_go_field_name,
    unique_go_json_field_name_for_field, widen_go_enum_type_if_needed,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneProject {
    pub files: Vec<GoStandaloneProjectFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneProjectFile {
    pub path: String,
    pub source: String,
}

pub(super) fn emit_standalone_project_with_context(
    emitted_unit: &SerializeCodegenUnit,
    context_unit: &SerializeCodegenUnit,
    module_path: &str,
    package_name: &str,
    context: &CodegenContext,
) -> Result<GoStandaloneProject, GoSourceEmitError> {
    reject_unresolved_types(emitted_unit)?;

    if !is_go_identifier(package_name) {
        return Err(GoSourceEmitError::PackageName {
            package_name: package_name.to_owned(),
        });
    }

    let mut files = vec![
        GoStandaloneProjectFile {
            path: "types.go".to_owned(),
            source: format_go_source(&format!("package {package_name}\n"))?,
        },
        GoStandaloneProjectFile {
            path: "az/uuid/uuid.go".to_owned(),
            source: format_go_source(support::uuid_package_source())?,
        },
        GoStandaloneProjectFile {
            path: "az/crc/crc.go".to_owned(),
            source: format_go_source(support::crc_package_source())?,
        },
        GoStandaloneProjectFile {
            path: "az/rtti/rtti.go".to_owned(),
            source: format_go_source(&support::rtti_package_source(module_path))?,
        },
        GoStandaloneProjectFile {
            path: "az/asset/asset.go".to_owned(),
            source: format_go_source(&support::asset_package_source(module_path))?,
        },
        GoStandaloneProjectFile {
            path: "az/math/math.go".to_owned(),
            source: format_go_source(support::math_package_source())?,
        },
    ];
    files.extend(emit_project_type_files_with_context(
        emitted_unit,
        context_unit,
        module_path,
        context,
    )?);
    Ok(GoStandaloneProject { files })
}

fn emit_project_type_files_with_context(
    emitted_unit: &SerializeCodegenUnit,
    context_unit: &SerializeCodegenUnit,
    module_path: &str,
    context: &CodegenContext,
) -> Result<Vec<GoStandaloneProjectFile>, GoSourceEmitError> {
    let items_by_type_id = items_by_type_id(context_unit);
    let base_type_ids = reflected_base_type_ids(context_unit);
    let packages_by_type_id = go_type_packages_by_id_with_context(
        emitted_unit,
        context_unit,
        &items_by_type_id,
        &base_type_ids,
    );
    let names_by_type_id = codegen_type_names_by_id_in_packages(emitted_unit, &packages_by_type_id);
    let groups = go_package_groups_with_context(
        emitted_unit,
        context_unit,
        &items_by_type_id,
        &base_type_ids,
    );
    let tasks = groups.into_iter().collect::<Vec<_>>();
    context.runner().try_map(&tasks, |task| {
        let ((package, file_stem), items) = task;
        Ok(GoStandaloneProjectFile {
            path: go_type_file_path(package, file_stem),
            source: emit_project_type_file(
                items,
                &items_by_type_id,
                &names_by_type_id,
                &packages_by_type_id,
                module_path,
                package,
            )?,
        })
    })
}

fn emit_project_type_file(
    items: &[&SerializeCodegenItem],
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
    module_path: &str,
    package: &GoTypePackage,
) -> Result<String, GoSourceEmitError> {
    let mut out = String::new();
    out.push_str("package ");
    out.push_str(&package.name);
    out.push_str("\n\n");
    let import_context = go_project_import_context(
        items,
        items_by_type_id,
        packages_by_type_id,
        package,
        module_path,
    );
    emit_project_package_imports(&import_context, module_path, &mut out);

    for item in items {
        match item.kind {
            SerializeCodegenItemKind::Struct => emit_project_struct_for_package(
                item,
                items_by_type_id,
                names_by_type_id,
                packages_by_type_id,
                package,
                &import_context,
                &mut out,
            ),
            SerializeCodegenItemKind::Enum => emit_project_enum_for_package(
                item,
                items_by_type_id,
                names_by_type_id,
                packages_by_type_id,
                package,
                &import_context,
                &mut out,
            ),
        }
        out.push('\n');
    }

    format_go_source(&out)
}

fn emit_project_struct_for_package(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
    current_package: &GoTypePackage,
    import_context: &GoProjectImportContext,
    out: &mut String,
) {
    if item.is_abstract == Some(true) && !item_has_materialized_payload(item, items_by_type_id) {
        out.push_str("type ");
        out.push_str(&go_type_name(item, names_by_type_id));
        out.push_str(" interface {\n");
        out.push('\t');
        out.push_str(import_context.rtti_selector());
        out.push_str(".HasRtti\n");
        for field in item.fields.iter().filter(|field| field.is_base_class) {
            if base_class_is_abstract(field, items_by_type_id) {
                out.push('\t');
                out.push_str(&render_project_go_type_for_package(
                    &field.resolved_type,
                    items_by_type_id,
                    names_by_type_id,
                    packages_by_type_id,
                    current_package,
                    import_context,
                ));
                out.push('\n');
            }
        }
        out.push_str("}\n");
        emit_project_go_rtti_registration(
            item,
            &go_type_name(item, names_by_type_id),
            import_context,
            out,
        );
        return;
    }

    let type_name = go_type_name(item, names_by_type_id);
    out.push_str("type ");
    out.push_str(&type_name);
    out.push_str(" struct {\n");
    let mut used_field_names = BTreeMap::new();
    let mut used_json_field_names = BTreeMap::new();
    for field in &item.fields {
        if field.is_base_class {
            match classify_codegen_field(field, items_by_type_id) {
                CodegenFieldProjection::MarkerBaseField => {
                    out.push('\t');
                    out.push_str(&render_project_go_field_type_for_package(
                        field,
                        items_by_type_id,
                        names_by_type_id,
                        packages_by_type_id,
                        current_package,
                        import_context,
                    ));
                    out.push('\n');
                    continue;
                }
                CodegenFieldProjection::MaterializedBaseField => {}
                CodegenFieldProjection::RegularField
                | CodegenFieldProjection::InterfaceBase
                | CodegenFieldProjection::SkippedBase => continue,
            }
            let field_name = unique_go_field_name(field, &mut used_field_names);
            let json_field_name =
                unique_go_json_field_name_for_field(field, &field_name, &mut used_json_field_names);
            out.push('\t');
            out.push_str(&field_name);
            out.push(' ');
            out.push_str(&render_project_go_field_type_for_package(
                field,
                items_by_type_id,
                names_by_type_id,
                packages_by_type_id,
                current_package,
                import_context,
            ));
            out.push_str(" `json:\"");
            out.push_str(&json_field_name);
            out.push_str("\"`\n");
            continue;
        }
        let field_name = unique_go_field_name(field, &mut used_field_names);
        let json_field_name =
            unique_go_json_field_name_for_field(field, &field_name, &mut used_json_field_names);
        out.push('\t');
        out.push_str(&field_name);
        out.push(' ');
        out.push_str(&render_project_go_field_type_for_package(
            field,
            items_by_type_id,
            names_by_type_id,
            packages_by_type_id,
            current_package,
            import_context,
        ));
        out.push_str(" `json:\"");
        out.push_str(&json_field_name);
        out.push_str("\"`\n");
    }
    out.push_str("}\n");
    emit_project_go_rtti_registration_and_method(item, &type_name, import_context, out);
}

fn emit_project_enum_for_package(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
    current_package: &GoTypePackage,
    import_context: &GoProjectImportContext,
    out: &mut String,
) {
    let type_name = go_type_name(item, names_by_type_id);
    let variant_names = go_enum_variant_names(item, &type_name);
    let raw_type = item
        .enum_underlying_type
        .as_ref()
        .map(|resolved| {
            render_project_go_type_for_package(
                resolved,
                items_by_type_id,
                names_by_type_id,
                packages_by_type_id,
                current_package,
                import_context,
            )
        })
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
    emit_project_go_rtti_registration_and_method(item, &type_name, import_context, out);
}

fn emit_project_go_rtti_registration(
    item: &SerializeCodegenItem,
    type_name: &str,
    import_context: &GoProjectImportContext,
    out: &mut String,
) {
    out.push_str("\nvar _ = ");
    out.push_str(import_context.rtti_selector());
    out.push_str(".Register[");
    out.push_str(type_name);
    out.push_str("](");
    out.push_str(&go_string_literal(&item.source_name));
    out.push_str(", ");
    out.push_str(&go_string_literal(
        &item.source_type_id.hyphenated().to_string(),
    ));
    out.push_str(")\n");
}

fn emit_project_go_rtti_registration_and_method(
    item: &SerializeCodegenItem,
    type_name: &str,
    import_context: &GoProjectImportContext,
    out: &mut String,
) {
    emit_project_go_rtti_registration(item, type_name, import_context, out);
    out.push_str("\nfunc (value *");
    out.push_str(type_name);
    out.push_str(") AzRtti() *");
    out.push_str(import_context.rtti_selector());
    out.push_str(".Type {\n\treturn ");
    out.push_str(import_context.rtti_selector());
    out.push_str(".TypeFor[");
    out.push_str(type_name);
    out.push_str("]()\n}\n");
}

fn render_project_go_field_type_for_package(
    field: &crate::ir::SerializeCodegenField,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
    current_package: &GoTypePackage,
    import_context: &GoProjectImportContext,
) -> String {
    match classify_codegen_field_type(field) {
        CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len } => {
            format!("[{byte_len}]byte")
        }
        CodegenFieldTypeProjection::Reflected(resolved_type) => render_project_go_type_for_package(
            resolved_type,
            items_by_type_id,
            names_by_type_id,
            packages_by_type_id,
            current_package,
            import_context,
        ),
    }
}

fn render_project_go_type_for_package(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
    current_package: &GoTypePackage,
    import_context: &GoProjectImportContext,
) -> String {
    match resolved {
        ResolvedType::Scalar(scalar) => render_project_go_scalar(*scalar, import_context),
        ResolvedType::Named {
            type_id,
            source_name,
        } => {
            let type_name = names_by_type_id
                .get(type_id)
                .cloned()
                .unwrap_or_else(|| rust_type_ident(source_name));
            packages_by_type_id
                .get(type_id)
                .filter(|package| *package != current_package)
                .map(|package| {
                    format!(
                        "{}.{}",
                        import_context.type_package_selector(package),
                        type_name
                    )
                })
                .unwrap_or(type_name)
        }
        ResolvedType::Sequence {
            kind,
            element,
            capacity,
        } => render_project_go_sequence_for_package(
            *kind,
            element,
            *capacity,
            &GoProjectTypeRenderContext {
                items_by_type_id,
                names_by_type_id,
                packages_by_type_id,
                current_package,
                import_context,
            },
        ),
        ResolvedType::Map { key, value, .. } => {
            let key_type = render_project_go_type_for_package(
                key,
                items_by_type_id,
                names_by_type_id,
                packages_by_type_id,
                current_package,
                import_context,
            );
            let value_type = render_project_go_type_for_package(
                value,
                items_by_type_id,
                names_by_type_id,
                packages_by_type_id,
                current_package,
                import_context,
            );
            if go_type_is_comparable(key_type.as_str(), key, items_by_type_id) {
                return format!("map[{key_type}]{value_type}");
            }
            format!("[]struct {{ Key {key_type}; Value {value_type} }}")
        }
        ResolvedType::Asset { .. } => format!("{}.Asset", import_context.asset_selector()),
        ResolvedType::Uid { .. } => format!("{}.Uuid", import_context.uuid_selector()),
        ResolvedType::ReplicatedField { value } => {
            format!(
                "*{}",
                render_project_go_type_for_package(
                    value,
                    items_by_type_id,
                    names_by_type_id,
                    packages_by_type_id,
                    current_package,
                    import_context,
                )
            )
        }
        ResolvedType::RangedInteger { value, .. } => render_project_go_type_for_package(
            value,
            items_by_type_id,
            names_by_type_id,
            packages_by_type_id,
            current_package,
            import_context,
        ),
        ResolvedType::ByteStream => "[]byte".to_owned(),
        ResolvedType::Pair { first, second } => {
            format!(
                "struct {{ First {}; Second {} }}",
                render_project_go_type_for_package(
                    first,
                    items_by_type_id,
                    names_by_type_id,
                    packages_by_type_id,
                    current_package,
                    import_context,
                ),
                render_project_go_type_for_package(
                    second,
                    items_by_type_id,
                    names_by_type_id,
                    packages_by_type_id,
                    current_package,
                    import_context,
                )
            )
        }
        ResolvedType::Pointer { target, .. } | ResolvedType::Optional { value: target } => {
            format!(
                "*{}",
                render_project_go_type_for_package(
                    target,
                    items_by_type_id,
                    names_by_type_id,
                    packages_by_type_id,
                    current_package,
                    import_context,
                )
            )
        }
        ResolvedType::Tuple { elements } => render_project_go_tuple_for_package(
            elements,
            items_by_type_id,
            names_by_type_id,
            packages_by_type_id,
            current_package,
            import_context,
        ),
        ResolvedType::Unknown { .. } => {
            unreachable!("unresolved reflected types are rejected before Go rendering")
        }
    }
}

fn render_project_go_scalar(scalar: ScalarType, import_context: &GoProjectImportContext) -> String {
    match scalar {
        ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 => "int8".to_owned(),
        ScalarType::U8 => "uint8".to_owned(),
        ScalarType::I16 => "int16".to_owned(),
        ScalarType::U16 => "uint16".to_owned(),
        ScalarType::I32 => "int32".to_owned(),
        ScalarType::U32 => "uint32".to_owned(),
        ScalarType::I64 => "int64".to_owned(),
        ScalarType::U64 | ScalarType::UnsignedLong => "uint64".to_owned(),
        ScalarType::F32 => "float32".to_owned(),
        ScalarType::F64 => "float64".to_owned(),
        ScalarType::Bool => "bool".to_owned(),
        ScalarType::Uuid => format!("{}.Uuid", import_context.uuid_selector()),
        ScalarType::Crc32 => format!("{}.Crc32", import_context.crc_selector()),
        ScalarType::EntityId => "uint64".to_owned(),
        ScalarType::AssetId => format!("{}.AssetId", import_context.asset_selector()),
        ScalarType::Vector2 => format!("{}.Vector2", import_context.math_selector()),
        ScalarType::Vector3 => format!("{}.Vector3", import_context.math_selector()),
        ScalarType::Vector4 => format!("{}.Vector4", import_context.math_selector()),
        ScalarType::Quaternion => format!("{}.Quaternion", import_context.math_selector()),
        ScalarType::Transform => format!("{}.Transform", import_context.math_selector()),
        ScalarType::Color => format!("{}.Color", import_context.math_selector()),
        ScalarType::ColorF => format!("{}.ColorF", import_context.math_selector()),
        ScalarType::ColorB => format!("{}.ColorB", import_context.math_selector()),
        ScalarType::String => "string".to_owned(),
    }
}

struct GoProjectTypeRenderContext<'a> {
    items_by_type_id: &'a BTreeMap<uuid::Uuid, &'a SerializeCodegenItem>,
    names_by_type_id: &'a BTreeMap<uuid::Uuid, String>,
    packages_by_type_id: &'a BTreeMap<uuid::Uuid, GoTypePackage>,
    current_package: &'a GoTypePackage,
    import_context: &'a GoProjectImportContext,
}

fn render_project_go_sequence_for_package(
    kind: SequenceKind,
    element: &ResolvedType,
    capacity: Option<usize>,
    context: &GoProjectTypeRenderContext<'_>,
) -> String {
    let element_type = render_project_go_type_for_package(
        element,
        context.items_by_type_id,
        context.names_by_type_id,
        context.packages_by_type_id,
        context.current_package,
        context.import_context,
    );
    match (kind, capacity) {
        (SequenceKind::Array | SequenceKind::BitSet, Some(capacity)) => {
            format!("[{capacity}]{element_type}")
        }
        (SequenceKind::Set | SequenceKind::UnorderedSet, _)
            if go_type_is_comparable(element_type.as_str(), element, context.items_by_type_id) =>
        {
            format!("map[{element_type}]struct{{}}")
        }
        (SequenceKind::Set | SequenceKind::UnorderedSet, _) => format!("[]{element_type}"),
        _ => format!("[]{element_type}"),
    }
}

fn render_project_go_tuple_for_package(
    elements: &[ResolvedType],
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
    current_package: &GoTypePackage,
    import_context: &GoProjectImportContext,
) -> String {
    if elements.is_empty() {
        return "struct{}".to_owned();
    }

    format!(
        "struct {{ {} }}",
        elements
            .iter()
            .enumerate()
            .map(|(index, element)| format!(
                "Value{} {}",
                index + 1,
                render_project_go_type_for_package(
                    element,
                    items_by_type_id,
                    names_by_type_id,
                    packages_by_type_id,
                    current_package,
                    import_context,
                )
            ))
            .collect::<Vec<_>>()
            .join("; ")
    )
}

fn go_type_is_comparable(
    rendered_type: &str,
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
) -> bool {
    let mut visiting = BTreeSet::new();
    go_type_is_comparable_inner(rendered_type, resolved, items_by_type_id, &mut visiting)
}

fn go_type_is_comparable_inner(
    rendered_type: &str,
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    visiting: &mut BTreeSet<uuid::Uuid>,
) -> bool {
    match resolved {
        ResolvedType::Scalar(_) | ResolvedType::Asset { .. } | ResolvedType::Uid { .. } => true,
        ResolvedType::Named { type_id, .. } => {
            let Some(item) = items_by_type_id.get(type_id) else {
                return true;
            };
            if !visiting.insert(*type_id) {
                return false;
            }
            let comparable = item.kind == SerializeCodegenItemKind::Enum
                || item.fields.iter().all(|field| {
                    !CodegenTypeReferenceProjection::DataFreeAbstractInterfacesAndMaterializedFields
                        .field_should_reference_type(item, field, items_by_type_id)
                        || go_type_is_comparable_inner(
                            rendered_type,
                            &field.resolved_type,
                            items_by_type_id,
                            visiting,
                        )
                });
            visiting.remove(type_id);
            comparable
        }
        ResolvedType::Sequence {
            kind,
            element,
            capacity,
        } => match (kind, capacity) {
            (SequenceKind::Array | SequenceKind::BitSet, Some(_)) => {
                go_type_is_comparable_inner(rendered_type, element, items_by_type_id, visiting)
            }
            _ => false,
        },
        ResolvedType::Map { .. } | ResolvedType::ByteStream => false,
        ResolvedType::ReplicatedField { .. }
        | ResolvedType::Pointer { .. }
        | ResolvedType::Optional { .. } => true,
        ResolvedType::RangedInteger { value, .. } => {
            go_type_is_comparable_inner(rendered_type, value, items_by_type_id, visiting)
        }
        ResolvedType::Pair { first, second } => {
            go_type_is_comparable_inner(rendered_type, first, items_by_type_id, visiting)
                && go_type_is_comparable_inner(rendered_type, second, items_by_type_id, visiting)
        }
        ResolvedType::Tuple { elements } => elements.iter().all(|element| {
            go_type_is_comparable_inner(rendered_type, element, items_by_type_id, visiting)
        }),
        ResolvedType::Unknown { .. } => rendered_type == "struct{}",
    }
}

fn go_type_name(
    item: &SerializeCodegenItem,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
) -> String {
    names_by_type_id
        .get(&item.source_type_id)
        .cloned()
        .unwrap_or_else(|| rust_type_ident(&item.source_name))
}

fn codegen_type_names_by_id_in_packages(
    unit: &SerializeCodegenUnit,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
) -> BTreeMap<uuid::Uuid, String> {
    scoped_rust_type_names_by_id(
        unit.items
            .iter()
            .filter(|item| !item.is_reflection_marker)
            .map(|item| {
                (
                    item.source_type_id,
                    packages_by_type_id
                        .get(&item.source_type_id)
                        .map(|package| package.dir.split('/').map(str::to_owned).collect())
                        .unwrap_or_default(),
                    item.source_name.as_str(),
                )
            }),
    )
}

fn go_import_alias(package: &GoTypePackage) -> String {
    go_import_alias_from_segments(package.dir.split('/')).unwrap_or_else(|| "types".to_owned())
}

#[derive(Debug, Clone)]
struct GoProjectImportContext {
    specs: Vec<GoProjectImportSpec>,
    selectors_by_type_package_dir: BTreeMap<String, String>,
    asset_selector: String,
    crc_selector: String,
    rtti_selector: String,
    uuid_selector: String,
    math_selector: String,
}

impl GoProjectImportContext {
    fn type_package_selector(&self, package: &GoTypePackage) -> String {
        self.selectors_by_type_package_dir
            .get(&package.dir)
            .cloned()
            .unwrap_or_else(|| package.name.clone())
    }

    fn asset_selector(&self) -> &str {
        self.asset_selector.as_str()
    }

    fn crc_selector(&self) -> &str {
        self.crc_selector.as_str()
    }

    fn rtti_selector(&self) -> &str {
        self.rtti_selector.as_str()
    }

    fn uuid_selector(&self) -> &str {
        self.uuid_selector.as_str()
    }

    fn math_selector(&self) -> &str {
        self.math_selector.as_str()
    }
}

#[derive(Debug, Clone)]
struct GoProjectImportSpec {
    key: GoProjectImportKey,
    default_name: String,
    import_path: String,
    collision_alias: String,
    selector: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum GoProjectImportKey {
    SupportAsset,
    SupportCrc,
    SupportRtti,
    SupportUuid,
    SupportMath,
    TypePackage(String),
}

fn go_project_import_context(
    items: &[&SerializeCodegenItem],
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
    current_package: &GoTypePackage,
    module_path: &str,
) -> GoProjectImportContext {
    let support_usage = CodegenSupportUsage::for_items(
        items.iter().copied(),
        items_by_type_id,
        CodegenTypeReferenceProjection::DataFreeAbstractInterfacesAndMaterializedFields,
    );
    let type_imports = go_type_imports_for_items(
        items,
        items_by_type_id,
        packages_by_type_id,
        current_package,
    );
    let mut specs = Vec::new();
    if support_usage.asset || support_usage.asset_id {
        specs.push(GoProjectImportSpec::new(
            GoProjectImportKey::SupportAsset,
            "asset",
            format!("{module_path}/az/asset"),
            "azasset",
        ));
    }
    if support_usage.crc32 {
        specs.push(GoProjectImportSpec::new(
            GoProjectImportKey::SupportCrc,
            "crc",
            format!("{module_path}/az/crc"),
            "azcrc",
        ));
    }
    specs.push(GoProjectImportSpec::new(
        GoProjectImportKey::SupportRtti,
        "rtti",
        format!("{module_path}/az/rtti"),
        "azrtti",
    ));
    if support_usage.uuid {
        specs.push(GoProjectImportSpec::new(
            GoProjectImportKey::SupportUuid,
            "uuid",
            format!("{module_path}/az/uuid"),
            "azuuid",
        ));
    }
    if support_usage.has_math() {
        specs.push(GoProjectImportSpec::new(
            GoProjectImportKey::SupportMath,
            "math",
            format!("{module_path}/az/math"),
            "azmath",
        ));
    }
    for package in type_imports {
        specs.push(GoProjectImportSpec::new(
            GoProjectImportKey::TypePackage(package.dir.clone()),
            package.name.clone(),
            format!("{module_path}/{}", package.dir),
            go_import_alias(&package),
        ));
    }

    assign_go_import_selectors(&mut specs);
    let mut selectors_by_type_package_dir = BTreeMap::new();
    let mut asset_selector = "asset".to_owned();
    let mut crc_selector = "crc".to_owned();
    let mut rtti_selector = "rtti".to_owned();
    let mut uuid_selector = "uuid".to_owned();
    let mut math_selector = "math".to_owned();
    for spec in &specs {
        match &spec.key {
            GoProjectImportKey::SupportAsset => asset_selector = spec.selector.clone(),
            GoProjectImportKey::SupportCrc => crc_selector = spec.selector.clone(),
            GoProjectImportKey::SupportRtti => rtti_selector = spec.selector.clone(),
            GoProjectImportKey::SupportUuid => uuid_selector = spec.selector.clone(),
            GoProjectImportKey::SupportMath => math_selector = spec.selector.clone(),
            GoProjectImportKey::TypePackage(dir) => {
                selectors_by_type_package_dir.insert(dir.clone(), spec.selector.clone());
            }
        }
    }

    GoProjectImportContext {
        specs,
        selectors_by_type_package_dir,
        asset_selector,
        crc_selector,
        rtti_selector,
        uuid_selector,
        math_selector,
    }
}

impl GoProjectImportSpec {
    fn new(
        key: GoProjectImportKey,
        default_name: impl Into<String>,
        import_path: impl Into<String>,
        collision_alias: impl Into<String>,
    ) -> Self {
        let default_name = default_name.into();
        Self {
            key,
            selector: default_name.clone(),
            default_name,
            import_path: import_path.into(),
            collision_alias: collision_alias.into(),
        }
    }
}

fn assign_go_import_selectors(specs: &mut [GoProjectImportSpec]) {
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, spec) in specs.iter().enumerate() {
        groups
            .entry(spec.default_name.clone())
            .or_default()
            .push(index);
    }
    for indices in groups.values() {
        if indices.len() <= 1 {
            continue;
        }
        let default_index = preferred_go_import_default_index(specs, indices);
        let mut used_selectors = BTreeSet::from([specs[default_index].default_name.clone()]);
        for &index in indices {
            if index == default_index {
                continue;
            }
            let alias = go_collision_import_alias(&specs[index], &used_selectors);
            used_selectors.insert(alias.clone());
            specs[index].selector = alias;
        }
    }
}

fn preferred_go_import_default_index(specs: &[GoProjectImportSpec], indices: &[usize]) -> usize {
    indices
        .iter()
        .copied()
        .min_by(|&left, &right| {
            let left_spec = &specs[left];
            let right_spec = &specs[right];
            (
                go_import_default_rank(left_spec),
                go_import_path_segments(left_spec).len(),
                left_spec.import_path.as_str(),
            )
                .cmp(&(
                    go_import_default_rank(right_spec),
                    go_import_path_segments(right_spec).len(),
                    right_spec.import_path.as_str(),
                ))
        })
        .expect("duplicate import group should not be empty")
}

fn go_import_default_rank(spec: &GoProjectImportSpec) -> u8 {
    match spec.key {
        GoProjectImportKey::SupportAsset
        | GoProjectImportKey::SupportCrc
        | GoProjectImportKey::SupportRtti
        | GoProjectImportKey::SupportUuid
        | GoProjectImportKey::SupportMath => 0,
        GoProjectImportKey::TypePackage(_) => 1,
    }
}

fn go_collision_import_alias(
    spec: &GoProjectImportSpec,
    used_selectors: &BTreeSet<String>,
) -> String {
    let segments = go_import_path_segments(spec);
    for suffix_len in 2..=segments.len() {
        if let Some(alias) =
            go_import_alias_from_segments(segments[segments.len() - suffix_len..].iter().copied())
            && alias != spec.default_name
            && !used_selectors.contains(&alias)
        {
            return alias;
        }
    }

    if !used_selectors.contains(&spec.collision_alias) {
        return spec.collision_alias.clone();
    }
    let mut suffix = 2;
    loop {
        let alias = format!("{}{}", spec.collision_alias, suffix);
        if !used_selectors.contains(&alias) {
            return alias;
        }
        suffix += 1;
    }
}

fn go_import_path_segments(spec: &GoProjectImportSpec) -> Vec<&str> {
    spec.import_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn go_import_alias_from_segments<'a>(
    segments: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let mut alias = segments
        .into_iter()
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if alias.is_empty() {
        return None;
    }
    if alias.starts_with(|ch: char| ch.is_ascii_digit()) {
        alias.insert_str(0, "pkg_");
    }
    is_go_identifier(&alias).then_some(alias)
}

fn emit_project_package_imports(
    import_context: &GoProjectImportContext,
    _module_path: &str,
    out: &mut String,
) {
    if import_context.specs.is_empty() {
        return;
    }

    out.push_str("import (\n");
    for spec in &import_context.specs {
        if spec.selector == spec.default_name {
            out.push_str(&format!("\t\"{}\"\n", spec.import_path));
        } else {
            out.push_str(&format!("\t{} \"{}\"\n", spec.selector, spec.import_path));
        }
    }
    out.push_str(")\n\n");
}

fn go_type_imports_for_items(
    items: &[&SerializeCodegenItem],
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
    current_package: &GoTypePackage,
) -> Vec<GoTypePackage> {
    let mut imports = BTreeMap::<String, GoTypePackage>::new();
    for item in items {
        collect_go_type_imports_for_item(
            item,
            items_by_type_id,
            packages_by_type_id,
            current_package,
            &mut imports,
        );
    }
    imports.into_values().collect()
}

fn collect_go_type_imports_for_item(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
    current_package: &GoTypePackage,
    imports: &mut BTreeMap<String, GoTypePackage>,
) {
    for type_id in codegen_item_referenced_type_ids(
        item,
        items_by_type_id,
        CodegenTypeReferenceProjection::DataFreeAbstractInterfacesAndMaterializedFields,
    ) {
        if let Some(package) = packages_by_type_id.get(&type_id)
            && package != current_package
        {
            imports.insert(package.dir.clone(), package.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_selectors_alias_only_duplicate_package_names_with_path_suffixes() {
        let mut specs = vec![
            GoProjectImportSpec::new(
                GoProjectImportKey::TypePackage("types/az/components".to_owned()),
                "components",
                "aztypesvalidation/types/az/components",
                "types_az_componentstypes",
            ),
            GoProjectImportSpec::new(
                GoProjectImportKey::TypePackage("types/components".to_owned()),
                "components",
                "aztypesvalidation/types/components",
                "types_componentstypes",
            ),
            GoProjectImportSpec::new(
                GoProjectImportKey::SupportCrc,
                "crc",
                "aztypesvalidation/az/crc",
                "azcrc",
            ),
            GoProjectImportSpec::new(
                GoProjectImportKey::TypePackage("types/az/crc".to_owned()),
                "crc",
                "aztypesvalidation/types/az/crc",
                "types_az_crctypes",
            ),
        ];

        assign_go_import_selectors(&mut specs);

        let selectors = specs
            .iter()
            .map(|spec| (spec.import_path.as_str(), spec.selector.as_str()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            selectors["aztypesvalidation/types/az/components"],
            "az_components"
        );
        assert_eq!(
            selectors["aztypesvalidation/types/components"],
            "components"
        );
        assert_eq!(selectors["aztypesvalidation/az/crc"], "crc");
        assert_eq!(selectors["aztypesvalidation/types/az/crc"], "az_crc");
    }
}
