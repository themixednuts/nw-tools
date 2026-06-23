use std::collections::BTreeSet;

use crate::document::SerializeContextDocument;
use crate::field_projection::projected_missing_reflected_types;
use crate::ir::SerializeCodegenUnit;
use crate::layout::{LayoutAnalysisReport, LayoutRootAudit, LayoutRootFindingKind};
use crate::reference::{ReferenceKey, ReferenceReport};
use crate::schema::{SchemaAzRttiInfo, SchemaGenericClassInfo, SerializeContext};
use crate::types::scalar_type;

const MAX_SCHEMA_GENERIC_LINT_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    DuplicateReferenceId,
    MissingReferenceTarget,
    CyclicReference,
    EmptyUuidMap,
    EmptyClassNameIndex,
    InvalidTypeId,
    UuidMapTypeIdMismatch,
    UuidGenericMapTypeIdMismatch,
    MissingTypeDefinition,
    MissingWrapperTarget,
    AmbiguousWrapperTarget,
    AmbiguousSharedSupportRoot,
    UniqueOwnedSupportRoot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
}

#[must_use]
pub fn lint_document(document: &SerializeContextDocument) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let refs = document.references().report();
    lint_reference_report(refs, &mut diagnostics);

    if let Some(schema) = document.schema() {
        lint_schema(schema, &mut diagnostics);
    } else {
        lint_unchecked_root(document, &mut diagnostics);
    }

    diagnostics
}

#[must_use]
pub fn lint_codegen_unit(codegen_unit: &SerializeCodegenUnit) -> Vec<Diagnostic> {
    let mut diagnostics = missing_type_diagnostics(codegen_unit);
    let root_audit = LayoutAnalysisReport::from_codegen_unit(codegen_unit).root_audit();
    diagnostics.extend(layout_root_diagnostics(&root_audit));
    diagnostics
}

fn missing_type_diagnostics(codegen_unit: &SerializeCodegenUnit) -> Vec<Diagnostic> {
    projected_missing_reflected_types(codegen_unit)
        .into_iter()
        .map(|missing| {
            let relation = if missing.is_base_class {
                "base class"
            } else {
                "field"
            };
            Diagnostic {
                severity: Severity::Warning,
                code: DiagnosticCode::MissingTypeDefinition,
                message: format!(
                    "missing reflected type `{}` for {relation} `{}.{}`: {}",
                    missing.type_id, missing.owner_name, missing.field_name, missing.reason
                ),
            }
        })
        .collect()
}

fn layout_root_diagnostics(audit: &LayoutRootAudit) -> Vec<Diagnostic> {
    audit
        .findings
        .iter()
        .map(|finding| match finding.kind {
            LayoutRootFindingKind::AmbiguousSharedSupportRoot => Diagnostic {
                severity: Severity::Warning,
                code: DiagnosticCode::AmbiguousSharedSupportRoot,
                message: format!(
                    "namespace-less support type `{}` emitted as root `{}` and is shared by {} owners without a stronger common layout scope; samples: {}",
                    finding.source_name,
                    finding.root_segment,
                    finding.owner_edges.len(),
                    owner_edge_samples(&finding.owner_edges)
                ),
            },
            LayoutRootFindingKind::UniqueOwnedSupportRoot => Diagnostic {
                severity: Severity::Warning,
                code: DiagnosticCode::UniqueOwnedSupportRoot,
                message: format!(
                    "namespace-less support type `{}` emitted as root `{}` but is only owned by `{}.{}`",
                    finding.source_name,
                    finding.root_segment,
                    finding.owner_edges[0].owner_source_name,
                    finding.owner_edges[0].field_name
                ),
            },
        })
        .collect()
}

fn owner_edge_samples(owner_edges: &[crate::layout::LayoutFieldOwnerEdge]) -> String {
    owner_edges
        .iter()
        .take(5)
        .map(|edge| format!("{}.{}", edge.owner_source_name, edge.field_name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn lint_schema(schema: &SerializeContext, diagnostics: &mut Vec<Diagnostic>) {
    let known_type_ids = known_reflected_type_ids(schema);
    let mut has_uuid_map = false;
    for (key, class) in schema.uuid_map_entries() {
        has_uuid_map = true;
        let key_uuid = lint_type_id(key, format!("uuidMap key `{key}`"), diagnostics);
        let type_uuid = lint_type_id(
            class.type_id(),
            format!("uuidMap[{key}].typeId `{}`", class.type_id()),
            diagnostics,
        );
        if let (Some(key_uuid), Some(type_uuid)) = (key_uuid, type_uuid)
            && key_uuid != type_uuid
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: DiagnosticCode::UuidMapTypeIdMismatch,
                message: format!(
                    "uuidMap key `{key}` does not match reflected typeId `{}` for `{}`",
                    class.type_id(),
                    class.name()
                ),
            });
        }

        for (index, element) in class.elements().iter().enumerate() {
            lint_referenced_type_id(
                element.type_id(),
                format!(
                    "uuidMap[{key}].elements[{index}].typeId `{}`",
                    element.type_id()
                ),
                &known_type_ids,
                diagnostics,
            );
            if let Some(generic) = element.generic_class_info() {
                lint_generic_class_info_root(
                    &format!("uuidMap[{key}].elements[{index}].genericClassInfo"),
                    generic,
                    &known_type_ids,
                    diagnostics,
                );
            }
            if let Some(rtti) = element.az_rtti() {
                lint_az_rtti(
                    &format!("uuidMap[{key}].elements[{index}].azRtti"),
                    rtti,
                    &known_type_ids,
                    diagnostics,
                );
            }
        }
        if let Some(rtti) = class.az_rtti() {
            lint_az_rtti(
                &format!("uuidMap[{key}].azRtti"),
                rtti,
                &known_type_ids,
                diagnostics,
            );
        }
    }
    if !has_uuid_map {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: DiagnosticCode::EmptyUuidMap,
            message: "SerializeContext uuidMap is missing or empty".to_owned(),
        });
    }

    if schema.class_name_entries().next().is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: DiagnosticCode::EmptyClassNameIndex,
            message: "SerializeContext classNameToUuid is missing or empty".to_owned(),
        });
    }

    for (key, generic) in schema.uuid_generic_class_entries() {
        let key_uuid = lint_type_id(key, format!("uuidGenericMap key `{key}`"), diagnostics);
        let type_uuid = generic.type_id().and_then(|type_id| {
            lint_type_id(
                type_id,
                format!("uuidGenericMap[{key}].typeId `{type_id}`"),
                diagnostics,
            )
        });
        if let (Some(key_uuid), Some(type_uuid)) = (key_uuid, type_uuid)
            && key_uuid != type_uuid
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: DiagnosticCode::UuidGenericMapTypeIdMismatch,
                message: format!(
                    "uuidGenericMap key `{key}` does not match reflected generic typeId `{}`",
                    generic.type_id().unwrap_or("")
                ),
            });
        }
        lint_generic_class_info_root(
            &format!("uuidGenericMap[{key}]"),
            generic,
            &known_type_ids,
            diagnostics,
        );
    }

    for (type_id, _) in schema.uuid_any_creation_entries() {
        lint_type_id(
            type_id,
            format!("uuidAnyCreationMap key `{type_id}`"),
            diagnostics,
        );
    }
}

fn lint_reference_report(report: ReferenceReport, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.extend(report.duplicate_ids.into_iter().map(duplicate_reference_id));
    diagnostics.extend(
        report
            .missing_refs
            .into_iter()
            .map(missing_reference_target),
    );
    diagnostics.extend(report.cyclic_refs.into_iter().map(cyclic_reference));
}

#[derive(Debug, Default)]
struct SchemaGenericLintState {
    depth: usize,
    stack: std::collections::BTreeSet<ReferenceKey>,
}

impl SchemaGenericLintState {
    fn enter(&mut self, generic: SchemaGenericClassInfo<'_>) -> bool {
        if self.depth >= MAX_SCHEMA_GENERIC_LINT_DEPTH {
            return false;
        }
        let reference_id = generic.id().and_then(schema_reference_id);
        if let Some(reference_id) = reference_id.as_ref()
            && !self.stack.insert(reference_id.clone())
        {
            return false;
        }
        self.depth += 1;
        true
    }

    fn exit(&mut self, generic: SchemaGenericClassInfo<'_>) {
        if self.depth > 0 {
            self.depth -= 1;
        }
        if let Some(reference_id) = generic.id().and_then(schema_reference_id) {
            self.stack.remove(&reference_id);
        }
    }
}

fn lint_generic_class_info_root(
    path: &str,
    generic: SchemaGenericClassInfo<'_>,
    known_type_ids: &BTreeSet<uuid::Uuid>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut state = SchemaGenericLintState::default();
    lint_generic_class_info(path, generic, known_type_ids, diagnostics, &mut state);
}

fn lint_generic_class_info(
    path: &str,
    generic: SchemaGenericClassInfo<'_>,
    known_type_ids: &BTreeSet<uuid::Uuid>,
    diagnostics: &mut Vec<Diagnostic>,
    state: &mut SchemaGenericLintState,
) {
    if !state.enter(generic) {
        return;
    }
    if let Some(type_id) = generic.type_id() {
        lint_type_id(type_id, format!("{path}.typeId `{type_id}`"), diagnostics);
    }
    for (index, type_id) in generic.registered_type_ids().iter().enumerate() {
        lint_type_id(
            type_id,
            format!("{path}.registeredTypeIds[{index}] `{type_id}`"),
            diagnostics,
        );
    }
    for (index, type_id) in generic.templated_type_ids().iter().enumerate() {
        lint_referenced_type_id(
            type_id,
            format!("{path}.templatedTypeIds[{index}] `{type_id}`"),
            known_type_ids,
            diagnostics,
        );
    }
    for (index, type_id) in generic.type_id_fold_type_ids().iter().enumerate() {
        lint_type_id(
            type_id,
            format!("{path}.typeIdFoldTypeIds[{index}] `{type_id}`"),
            diagnostics,
        );
    }
    if let Some(type_id) = generic.specialized_type_id() {
        lint_type_id(
            type_id,
            format!("{path}.specializedTypeId `{type_id}`"),
            diagnostics,
        );
    }
    if let Some(type_id) = generic.generic_type_id() {
        lint_type_id(
            type_id,
            format!("{path}.genericTypeId `{type_id}`"),
            diagnostics,
        );
    }
    if let Some(type_id) = generic.legacy_specialized_type_id() {
        lint_type_id(
            type_id.as_ref(),
            format!("{path}.legacySpecializedTypeId `{type_id}`"),
            diagnostics,
        );
    }
    if let Some(class_data) = generic.class_data() {
        lint_type_id(
            class_data.type_id(),
            format!("{path}.classData.typeId `{}`", class_data.type_id()),
            diagnostics,
        );
        if let Some(rtti) = class_data.az_rtti() {
            lint_az_rtti(
                &format!("{path}.classData.azRtti"),
                rtti,
                known_type_ids,
                diagnostics,
            );
        }
    }

    for (index, element) in generic.elements().iter().enumerate() {
        lint_referenced_type_id(
            element.type_id(),
            format!("{path}.elements[{index}].typeId `{}`", element.type_id()),
            known_type_ids,
            diagnostics,
        );
        if let Some(child) = element.generic_class_info() {
            lint_generic_class_info(
                &format!("{path}.elements[{index}].genericClassInfo"),
                child,
                known_type_ids,
                diagnostics,
                state,
            );
        }
        if let Some(rtti) = element.az_rtti() {
            lint_az_rtti(
                &format!("{path}.elements[{index}].azRtti"),
                rtti,
                known_type_ids,
                diagnostics,
            );
        }
    }
    state.exit(generic);
}

fn lint_az_rtti(
    path: &str,
    rtti: SchemaAzRttiInfo<'_>,
    known_type_ids: &BTreeSet<uuid::Uuid>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(type_id) = rtti.type_id() {
        lint_type_id(type_id, format!("{path}.typeId `{type_id}`"), diagnostics);
    }
    for (index, entry) in rtti.hierarchy().iter().enumerate() {
        lint_referenced_type_id(
            entry.type_id,
            format!("{path}.hierarchy[{index}].typeId `{}`", entry.type_id),
            known_type_ids,
            diagnostics,
        );
    }
}

fn lint_type_id(
    value: &str,
    context: String,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<uuid::Uuid> {
    match uuid::Uuid::parse_str(value) {
        Ok(type_id) => Some(type_id),
        Err(_) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: DiagnosticCode::InvalidTypeId,
                message: format!("{context} is not a UUID"),
            });
            None
        }
    }
}

fn lint_referenced_type_id(
    value: &str,
    context: String,
    known_type_ids: &BTreeSet<uuid::Uuid>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<uuid::Uuid> {
    let type_id = lint_type_id(value, context.clone(), diagnostics)?;
    if scalar_type(type_id).is_none() && !known_type_ids.contains(&type_id) {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: DiagnosticCode::MissingTypeDefinition,
            message: format!("{context} does not have a reflected definition in SerializeContext"),
        });
    }
    Some(type_id)
}

fn known_reflected_type_ids(schema: &SerializeContext) -> BTreeSet<uuid::Uuid> {
    let mut type_ids = BTreeSet::new();
    for (key, class) in schema.uuid_map_entries() {
        insert_uuid(&mut type_ids, key);
        insert_uuid(&mut type_ids, class.type_id());
        for element in class.elements() {
            if let Some(generic) = element.generic_class_info() {
                collect_known_generic_type_ids(&mut type_ids, generic);
            }
        }
    }
    for (key, generic) in schema.uuid_generic_class_entries() {
        insert_uuid(&mut type_ids, key);
        collect_known_generic_type_ids(&mut type_ids, generic);
    }
    type_ids
}

fn collect_known_generic_type_ids(
    type_ids: &mut BTreeSet<uuid::Uuid>,
    generic: SchemaGenericClassInfo<'_>,
) {
    if let Some(type_id) = generic.type_id() {
        insert_uuid(type_ids, type_id);
    }
    for type_id in generic.registered_type_ids() {
        insert_uuid(type_ids, type_id);
    }
    if let Some(type_id) = generic.specialized_type_id() {
        insert_uuid(type_ids, type_id);
    }
    if let Some(type_id) = generic.legacy_specialized_type_id() {
        insert_uuid(type_ids, type_id.as_ref());
    }
    if let Some(class_data) = generic.class_data() {
        insert_uuid(type_ids, class_data.type_id());
    }
    for element in generic.elements() {
        if let Some(child) = element.generic_class_info() {
            collect_known_generic_type_ids(type_ids, child);
        }
    }
}

fn insert_uuid(type_ids: &mut BTreeSet<uuid::Uuid>, value: &str) {
    if let Ok(type_id) = uuid::Uuid::parse_str(value) {
        type_ids.insert(type_id);
    }
}

fn lint_unchecked_root(document: &SerializeContextDocument, diagnostics: &mut Vec<Diagnostic>) {
    let root = document.root();
    if root
        .get("uuidMap")
        .and_then(serde_json::Value::as_object)
        .is_none_or(serde_json::Map::is_empty)
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: DiagnosticCode::EmptyUuidMap,
            message: "SerializeContext uuidMap is missing or empty".to_owned(),
        });
    }
    if root
        .get("classNameToUuid")
        .and_then(serde_json::Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: DiagnosticCode::EmptyClassNameIndex,
            message: "SerializeContext classNameToUuid is missing or empty".to_owned(),
        });
    }
}

fn duplicate_reference_id(key: ReferenceKey) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: DiagnosticCode::DuplicateReferenceId,
        message: format!("duplicate $id {}", key.display()),
    }
}

fn missing_reference_target(key: ReferenceKey) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: DiagnosticCode::MissingReferenceTarget,
        message: format!("missing $ref target {}", key.display()),
    }
}

fn cyclic_reference(key: ReferenceKey) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: DiagnosticCode::CyclicReference,
        message: format!("cyclic $ref chain reaches {}", key.display()),
    }
}

fn schema_reference_id(value: i64) -> Option<ReferenceKey> {
    u64::try_from(value).ok().map(ReferenceKey::Number)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::uuid;

    use crate::document::SerializeContextDocument;
    use crate::layout::{
        LayoutFieldOwnerEdge, LayoutRootAudit, LayoutRootFinding, LayoutRootFindingKind,
    };

    use super::*;

    #[test]
    fn lints_reference_integrity_and_required_indexes() {
        let root = json!({
            "$id": 1,
            "uuidMap": {},
            "classNameToUuid": [],
            "dangling": { "$ref": "#404" }
        });
        let document = SerializeContextDocument::from_value_unchecked(root);

        let diagnostics = lint_document(&document);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MissingReferenceTarget
                && diagnostic.severity == Severity::Error
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::EmptyUuidMap
                && diagnostic.severity == Severity::Error
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::EmptyClassNameIndex
                && diagnostic.severity == Severity::Warning
        }));
    }

    #[test]
    fn lints_cyclic_references_before_model_lowering_can_recurse() {
        let root = json!({
            "$id": 1,
            "uuidMap": {
                "11111111-1111-1111-1111-111111111111": {
                    "$id": "class-a",
                    "$ref": "#class-b"
                },
                "22222222-2222-2222-2222-222222222222": {
                    "$id": "class-b",
                    "$ref": "#class-a"
                }
            },
            "classNameToUuid": [[1, "11111111-1111-1111-1111-111111111111"]]
        });
        let document = SerializeContextDocument::from_value_unchecked(root);

        let diagnostics = lint_document(&document);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::CyclicReference
                && diagnostic.severity == Severity::Error
        }));
    }

    #[test]
    fn lints_schema_type_id_integrity_without_raw_json_shape_checks() {
        let document = SerializeContextDocument::from_slice(
            br#"{
                "$id": 1,
                "uuidMap": {
                    "11111111-1111-1111-1111-111111111111": {
                        "$id": 10,
                        "name": "Example::CounterComponent",
                        "typeId": "22222222-2222-2222-2222-222222222222",
                        "version": 0,
                        "doSave": null,
                        "dataConverter": null,
                        "editData": null,
                        "elements": [{
                            "$id": 11,
                            "name": "m_count",
                            "nameCrc": 1,
                            "typeId": "not-a-type-id",
                            "dataSize": "4",
                            "offset": "0",
                            "attributeOwnership": 0,
                            "flags": 0,
                            "is_pointer": false,
                            "is_base_class": false,
                            "no_default_value": false,
                            "is_dynamic_field": false,
                            "is_ui_element": false,
                            "editData": null,
                            "attributes": []
                        }],
                        "attributes": []
                    }
                },
                "classNameToUuid": [[123, "11111111-1111-1111-1111-111111111111"]],
                "uuidGenericMap": [[
                    "33333333-3333-3333-3333-333333333333",
                    {
                        "$id": 20,
                        "typeId": "44444444-4444-4444-4444-444444444444",
                        "registeredTypeIds": ["33333333-3333-3333-3333-333333333333"],
                        "templatedArgumentCount": 1,
                        "templatedTypeIds": ["also-not-a-type-id"],
                        "typeIdFoldTypeIds": null,
                        "specializedTypeId": "33333333-3333-3333-3333-333333333333",
                        "genericTypeId": "55555555-5555-5555-5555-555555555555",
                        "legacySpecializedTypeId": null,
                        "nonTypeTemplateArguments": null,
                        "classData": {
                            "$id": 21,
                            "name": "AZStd::vector",
                            "typeId": "33333333-3333-3333-3333-333333333333",
                            "version": 0,
                            "doSave": null,
                            "dataConverter": null,
                            "editData": null,
                            "elements": [],
                            "attributes": []
                        },
                        "elements": []
                    }
                ]],
                "uuidAnyCreationMap": {
                    "66666666-6666-6666-6666-666666666666": "0x1234"
                },
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            }"#,
        )
        .expect("schema document");

        let diagnostics = lint_document(&document);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::UuidMapTypeIdMismatch
                && diagnostic.severity == Severity::Error
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::UuidGenericMapTypeIdMismatch
                && diagnostic.severity == Severity::Error
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::InvalidTypeId
                && diagnostic.message.contains("not-a-type-id")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::InvalidTypeId
                && diagnostic.message.contains("also-not-a-type-id")
        }));
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::EmptyUuidMap)
        );
    }

    #[test]
    fn lints_missing_referenced_type_definitions_as_warnings() {
        let document = SerializeContextDocument::from_slice(
            br#"{
                "$id": 1,
                "uuidMap": {
                    "11111111-1111-1111-1111-111111111111": {
                        "$id": 10,
                        "name": "Example::CounterComponent",
                        "typeId": "11111111-1111-1111-1111-111111111111",
                        "version": 0,
                        "doSave": null,
                        "dataConverter": null,
                        "editData": null,
                        "elements": [{
                            "$id": 11,
                            "name": "m_missing",
                            "nameCrc": 1,
                            "typeId": "22222222-2222-2222-2222-222222222222",
                            "dataSize": "4",
                            "offset": "0",
                            "attributeOwnership": 0,
                            "flags": 0,
                            "is_pointer": false,
                            "is_base_class": false,
                            "no_default_value": false,
                            "is_dynamic_field": false,
                            "is_ui_element": false,
                            "editData": null,
                            "attributes": []
                        }],
                        "attributes": []
                    }
                },
                "classNameToUuid": [[123, "11111111-1111-1111-1111-111111111111"]],
                "uuidGenericMap": [],
                "uuidAnyCreationMap": {},
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            }"#,
        )
        .expect("schema document");

        let diagnostics = lint_document(&document);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MissingTypeDefinition
                && diagnostic.severity == Severity::Warning
                && diagnostic
                    .message
                    .contains("22222222-2222-2222-2222-222222222222")
        }));
    }

    #[test]
    fn lints_missing_az_rtti_hierarchy_types_as_warnings() {
        let document = SerializeContextDocument::from_slice(
            br#"{
                "$id": 1,
                "uuidMap": {
                    "11111111-1111-1111-1111-111111111111": {
                        "$id": 10,
                        "name": "Example::RuntimeComponent",
                        "typeId": "11111111-1111-1111-1111-111111111111",
                        "version": 0,
                        "doSave": null,
                        "dataConverter": null,
                        "editData": null,
                        "azRtti": {
                            "address": "NewWorld+0x10",
                            "typeId": "11111111-1111-1111-1111-111111111111",
                            "typeName": "Example::RuntimeComponent",
                            "hierarchy": [{
                                "typeId": "11111111-1111-1111-1111-111111111111",
                                "typeName": "Example::RuntimeComponent"
                            }, {
                                "typeId": "22222222-2222-2222-2222-222222222222",
                                "typeName": "Example::MissingRuntimeBase"
                            }],
                            "isAbstract": false
                        },
                        "elements": [],
                        "attributes": []
                    }
                },
                "classNameToUuid": [[123, "11111111-1111-1111-1111-111111111111"]],
                "uuidGenericMap": [],
                "uuidAnyCreationMap": {},
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            }"#,
        )
        .expect("schema document");

        let diagnostics = lint_document(&document);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MissingTypeDefinition
                && diagnostic.severity == Severity::Warning
                && diagnostic
                    .message
                    .contains("uuidMap[11111111-1111-1111-1111-111111111111].azRtti.hierarchy[1]")
                && diagnostic
                    .message
                    .contains("22222222-2222-2222-2222-222222222222")
        }));
    }

    #[test]
    fn lints_layout_owned_support_roots_as_warnings() {
        let audit = LayoutRootAudit {
            findings: vec![LayoutRootFinding {
                kind: LayoutRootFindingKind::UniqueOwnedSupportRoot,
                source_type_id: uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
                source_name: "MeshRenderOptions".to_owned(),
                root_segment: "mesh_render_options".to_owned(),
                owner_edges: vec![LayoutFieldOwnerEdge {
                    owner_type_id: uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
                    owner_source_name: "MeshComponent".to_owned(),
                    field_name: "m_options".to_owned(),
                }],
            }],
        };

        let diagnostics = layout_root_diagnostics(&audit);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Warning);
        assert_eq!(diagnostics[0].code, DiagnosticCode::UniqueOwnedSupportRoot);
        assert!(diagnostics[0].message.contains("MeshComponent.m_options"));
    }

    #[test]
    fn lints_layout_ambiguous_shared_support_roots_as_warnings() {
        let audit = LayoutRootAudit {
            findings: vec![LayoutRootFinding {
                kind: LayoutRootFindingKind::AmbiguousSharedSupportRoot,
                source_type_id: uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
                source_name: "LocalEntityRef".to_owned(),
                root_segment: "local_entity_ref".to_owned(),
                owner_edges: vec![
                    LayoutFieldOwnerEdge {
                        owner_type_id: uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
                        owner_source_name: "DoorComponentServerFacet".to_owned(),
                        field_name: "m_target".to_owned(),
                    },
                    LayoutFieldOwnerEdge {
                        owner_type_id: uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                        owner_source_name: "ContainerComponentServerFacet".to_owned(),
                        field_name: "m_owner".to_owned(),
                    },
                ],
            }],
        };

        let diagnostics = layout_root_diagnostics(&audit);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Warning);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::AmbiguousSharedSupportRoot
        );
        assert!(diagnostics[0].message.contains("shared by 2 owners"));
        assert!(
            diagnostics[0]
                .message
                .contains("DoorComponentServerFacet.m_target")
        );
    }

    #[test]
    fn lints_project_serialize_fixture_without_structural_errors() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("resources")
            .join("serialize.json");
        let document = SerializeContextDocument::from_path(path)
            .expect("project serialize.json should match generated schema");

        let diagnostics = lint_document(&document);

        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "{diagnostics:#?}"
        );
    }
}
