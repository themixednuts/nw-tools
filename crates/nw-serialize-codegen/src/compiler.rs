use std::{borrow::Cow, path::Path};

use crate::catalog::ReflectedTypeCatalog;
use crate::document::SerializeContextDocument;
use crate::field_evidence::FieldOwnerEvidenceIndex;
use crate::graph::SchemaGraph;
use crate::ir::{SerializeCodegenPlanner, SerializeCodegenUnit};
use crate::lint::{Diagnostic, lint_codegen_unit, lint_document};
use crate::model::SerializeContextModel;
use crate::native::NativeSymbolIndex;

mod compile_unit;
mod diagnostics;
mod error;
mod input;
mod view;

pub use error::SerializeContextCompileError;
pub use input::SerializeContextCompileInputs;

use crate::CodegenContext;
use diagnostics::schema_graph_diagnostics;
use input::{
    read_optional_class_registration_trace, read_optional_json, read_optional_module_descriptors,
};

#[derive(Debug, Clone)]
pub struct CompileUnit {
    pub document: SerializeContextDocument,
    pub model: SerializeContextModel,
    pub schema_graph: SchemaGraph,
    pub field_owner_evidence: FieldOwnerEvidenceIndex,
    pub codegen_unit: SerializeCodegenUnit,
    pub catalog: ReflectedTypeCatalog,
    pub native_symbols: NativeSymbolIndex,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct SerializeCodegenView<'a> {
    emitted_unit: Cow<'a, SerializeCodegenUnit>,
    context_unit: &'a SerializeCodegenUnit,
}

#[derive(Debug, Default)]
pub struct SerializeContextCompiler;

impl SerializeContextCompiler {
    #[must_use]
    pub fn compile(document: SerializeContextDocument, context: &CodegenContext) -> CompileUnit {
        Self::compile_with_inputs(document, SerializeContextCompileInputs::default(), context)
    }

    #[must_use]
    pub fn compile_with_inputs(
        document: SerializeContextDocument,
        inputs: SerializeContextCompileInputs<'_>,
        context: &CodegenContext,
    ) -> CompileUnit {
        let mut diagnostics = lint_document(&document);
        let model = SerializeContextModel::from_document(&document);
        let schema_graph = SchemaGraph::from_model(&model);
        let field_owner_evidence = FieldOwnerEvidenceIndex::from_model(&model);
        let codegen_unit = SerializeCodegenPlanner::plan_model(&model);
        diagnostics.extend(schema_graph_diagnostics(&schema_graph));
        diagnostics.extend(lint_codegen_unit(&codegen_unit));
        let catalog = ReflectedTypeCatalog::from_model_with_inputs(
            &model,
            crate::catalog::ReflectedTypeCatalogInputs {
                module_descriptors_root: inputs.module_descriptors_root,
                serialize_porting_root: inputs.serialize_porting_root,
                class_registration_trace_root: inputs.class_registration_trace_root,
            },
            context,
        );
        let native_symbols = NativeSymbolIndex::from_model(&model);
        CompileUnit {
            document,
            model,
            schema_graph,
            field_owner_evidence,
            codegen_unit,
            catalog,
            native_symbols,
            diagnostics,
        }
    }

    pub fn compile_from_paths(
        serialize_context: impl AsRef<Path>,
        module_descriptors: Option<impl AsRef<Path>>,
        serialize_porting: Option<impl AsRef<Path>>,
        context: &CodegenContext,
    ) -> Result<CompileUnit, SerializeContextCompileError> {
        let serialize_context = serialize_context.as_ref().to_path_buf();
        let module_descriptors = module_descriptors
            .as_ref()
            .map(|path| path.as_ref().to_path_buf());
        let serialize_porting = serialize_porting
            .as_ref()
            .map(|path| path.as_ref().to_path_buf());

        let ((document, module_descriptors_root), serialize_porting_root) = context.runner().join(
            || {
                context.runner().join(
                    || SerializeContextDocument::from_path(&serialize_context),
                    || read_optional_module_descriptors(module_descriptors.as_deref(), context),
                )
            },
            || read_optional_json(serialize_porting.as_deref()),
        );

        let document = document?;
        let module_descriptors_root = module_descriptors_root?;
        let serialize_porting_root = serialize_porting_root?;

        Ok(Self::compile_with_inputs(
            document,
            SerializeContextCompileInputs {
                module_descriptors_root: module_descriptors_root.as_ref(),
                serialize_porting_root: serialize_porting_root.as_ref(),
                class_registration_trace_root: None,
            },
            context,
        ))
    }

    pub fn compile_from_paths_with_class_registration_trace(
        serialize_context: impl AsRef<Path>,
        module_descriptors: Option<impl AsRef<Path>>,
        serialize_porting: Option<impl AsRef<Path>>,
        class_registration_trace: Option<impl AsRef<Path>>,
        context: &CodegenContext,
    ) -> Result<CompileUnit, SerializeContextCompileError> {
        let serialize_context = serialize_context.as_ref().to_path_buf();
        let module_descriptors = module_descriptors
            .as_ref()
            .map(|path| path.as_ref().to_path_buf());
        let serialize_porting = serialize_porting
            .as_ref()
            .map(|path| path.as_ref().to_path_buf());
        let class_registration_trace = class_registration_trace
            .as_ref()
            .map(|path| path.as_ref().to_path_buf());

        let ((document, module_descriptors_root), (serialize_porting_root, trace_root)) =
            context.runner().join(
                || {
                    context.runner().join(
                        || SerializeContextDocument::from_path(&serialize_context),
                        || read_optional_module_descriptors(module_descriptors.as_deref(), context),
                    )
                },
                || {
                    context.runner().join(
                        || read_optional_json(serialize_porting.as_deref()),
                        || {
                            read_optional_class_registration_trace(
                                class_registration_trace.as_deref(),
                            )
                        },
                    )
                },
            );

        let document = document?;
        let module_descriptors_root = module_descriptors_root?;
        let serialize_porting_root = serialize_porting_root?;
        let trace_root = trace_root?;

        Ok(Self::compile_with_inputs(
            document,
            SerializeContextCompileInputs {
                module_descriptors_root: module_descriptors_root.as_ref(),
                serialize_porting_root: serialize_porting_root.as_ref(),
                class_registration_trace_root: trace_root.as_ref(),
            },
            context,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::go::source::GoSourceOptions;
    use crate::typescript::source::TypeScriptSourceOptions;
    use serde_json::json;

    use crate::document::SerializeContextDocument;
    use crate::field_projection::projected_missing_reflected_types;
    use crate::ir::SerializeCodegenSelection;
    use crate::lint::{DiagnosticCode, Severity};
    use crate::rust::integrate::{
        FlatRustItemPathResolver, RustIntegrationAction, RustSourceInventory,
    };
    use crate::typescript::source::TypeScriptStandaloneProjectOptions;

    use super::*;

    #[test]
    fn compile_unit_lowers_model_and_keeps_lints_together() {
        let document = SerializeContextDocument::from_value_unchecked(json!({
            "$id": 1,
            "uuidMap": {},
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let unit = SerializeContextCompiler::compile(document, &CodegenContext::inline());

        assert!(unit.model.classes.is_empty());
        assert!(unit.codegen_unit.items.is_empty());
        assert_eq!(unit.catalog.summary().reflected_types, 0);
        assert!(unit.native_symbols.is_empty());
        assert!(unit.has_errors());
        assert!(!unit.diagnostics.is_empty());
    }

    #[test]
    fn compile_unit_surfaces_schema_graph_diagnostics() {
        let document = SerializeContextDocument::from_value_unchecked(json!({
            "$id": 1,
            "uuidMap": {
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 10,
                    "name": "RemoteServerFacetRef<MissingFacet >",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let unit = SerializeContextCompiler::compile(document, &CodegenContext::inline());

        assert!(unit.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MissingWrapperTarget
                && diagnostic.severity == Severity::Warning
                && diagnostic.message.contains("MissingFacet")
        }));
    }

    #[test]
    fn compile_unit_does_not_report_fixed_opaque_fields_as_codegen_missing_types() {
        let fixed_id = "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA";
        let document = SerializeContextDocument::from_value_unchecked(json!({
            "$id": 1,
            "uuidMap": {
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 10,
                    "name": "Example::OpaquePayload",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [{
                        "$id": 11,
                        "name": "payload",
                        "typeId": fixed_id,
                        "dataSize": "16",
                        "is_base_class": false
                    }],
                    "attributes": []
                }
            },
            "classNameToUuid": [[123, "11111111-1111-1111-1111-111111111111"]],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let unit = SerializeContextCompiler::compile(document, &CodegenContext::inline());

        assert!(projected_missing_reflected_types(&unit.codegen_unit).is_empty());
        assert!(
            !unit.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::MissingTypeDefinition
                    && diagnostic
                        .message
                        .contains("missing reflected type `aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa`")
            }),
            "fixed opaque fields should not produce codegen missing-type diagnostics: {:#?}",
            unit.diagnostics
        );
    }

    #[test]
    fn compile_unit_plans_rust_integration_from_serialize_model() {
        let document = SerializeContextDocument::from_value_unchecked(json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "AZ::Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "Example::CounterComponent",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "elements": [
                        {
                            "$id": 21,
                            "name": "AZ::Component",
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "is_base_class": true
                        },
                        {
                            "$id": 22,
                            "name": "m_count",
                            "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                            "is_base_class": false
                        }
                    ],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let compile_unit = SerializeContextCompiler::compile(document, &CodegenContext::inline());
        let inventory = RustSourceInventory::from_sources([]).expect("empty source inventory");

        let plan = compile_unit
            .plan_rust_integration(
                &inventory,
                &FlatRustItemPathResolver::new("components"),
                &CodegenContext::inline(),
            )
            .expect("rust integration plan");

        let counter = plan
            .items
            .iter()
            .find(|item| item.item.rust_name == "CounterComponent")
            .expect("counter component plan");
        assert!(matches!(
            &counter.action,
            RustIntegrationAction::Create { target_path, source }
                if target_path.ends_with("counter_component.rs")
                    && source.contains("AzRtti")
                    && source.contains("Component)]")
                    && source.contains("pub count: u32")
        ));
    }

    #[test]
    fn compile_unit_rust_integration_reuses_existing_source_type_ids() {
        let external_type_id = "11111111-2222-3333-4444-555555555555";
        let document = SerializeContextDocument::from_value_unchecked(json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "AZ::Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "Example::OwnerComponent",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "elements": [
                        {
                            "$id": 21,
                            "name": "AZ::Component",
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "is_base_class": true
                        },
                        {
                            "$id": 22,
                            "name": "m_payload",
                            "typeId": external_type_id,
                            "is_base_class": false
                        }
                    ],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let compile_unit = SerializeContextCompiler::compile(document, &CodegenContext::inline());
        let temp = tempfile::tempdir().expect("tempdir");
        let components_root = temp.path().join("components");
        let shared_root = components_root.join("shared");
        fs::create_dir_all(&shared_root).expect("create source module root");
        fs::write(
            shared_root.join("external.rs"),
            r#"
use az_derive::AzRtti;

#[derive(AzRtti)]
#[az_rtti("11111111-2222-3333-4444-555555555555")]
pub struct ExternalPayload;
"#,
        )
        .expect("write existing support type");
        let inventory = RustSourceInventory::from_root(&components_root, &CodegenContext::inline())
            .expect("inventory");

        let plan = compile_unit
            .plan_rust_integration(
                &inventory,
                &FlatRustItemPathResolver::new("components"),
                &CodegenContext::inline(),
            )
            .expect("rust integration plan");

        let owner = plan
            .items
            .iter()
            .find(|item| item.item.rust_name == "OwnerComponent")
            .expect("owner component plan");
        assert!(matches!(
            &owner.action,
            RustIntegrationAction::Create { source, .. }
                if source.contains(
                    "pub payload: crate::shared::external::ExternalPayload"
                )
                    && !source.contains("unresolved reflected type")
        ));
    }

    #[test]
    fn compile_from_paths_keeps_descriptor_and_porting_inputs_in_catalog() {
        let temp = tempfile::tempdir().expect("tempdir");
        let serialize_context = temp.path().join("serialize.json");
        let module_descriptors = temp.path().join("example-module.json");
        let serialize_porting = temp.path().join("serialize-porting.json");
        let class_registration_trace = temp.path().join("serialize-class-registration.jsonl");

        fs::write(
            &serialize_context,
            json!({
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
                        "elements": [],
                        "attributes": []
                    }
                },
                "classNameToUuid": [[123, "11111111-1111-1111-1111-111111111111"]],
                "uuidGenericMap": [],
                "uuidAnyCreationMap": {
                    "11111111-1111-1111-1111-111111111111": "NewWorld+0x100"
                },
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            })
            .to_string(),
        )
        .expect("write serialize context");
        fs::write(
            &module_descriptors,
            json!({
                "descriptors": [{
                    "componentName": "Example::CounterComponent",
                    "componentUuid": "11111111-1111-1111-1111-111111111111",
                    "vftable": "NewWorld+0x200",
                    "addr": "NewWorld+0x300"
                }]
            })
            .to_string(),
        )
        .expect("write module descriptors");
        fs::write(
            &serialize_porting,
            json!({
                "components": [{
                    "component": "Example::CounterComponent",
                    "module": "components/counter.rs"
                }],
                "support_owners": []
            })
            .to_string(),
        )
        .expect("write serialize porting");
        fs::write(
            &class_registration_trace,
            r#"{"sequence":1,"typeName":"Example::CounterComponent","typeId":"11111111-1111-1111-1111-111111111111","returnAddress":"NewWorld+0x440","classDataFactory":"NewWorld+0x550","classDataAzRtti":"NewWorld+0x660","anyCreator":"NewWorld+0x770"}
"#,
        )
        .expect("write class registration trace");

        let unit = SerializeContextCompiler::compile_from_paths_with_class_registration_trace(
            &serialize_context,
            Some(&module_descriptors),
            Some(&serialize_porting),
            Some(&class_registration_trace),
            &CodegenContext::inline(),
        )
        .expect("compile from paths");

        let summary = unit.catalog.summary();
        assert!(!unit.has_errors(), "{:#?}", unit.diagnostics);
        assert_eq!(summary.reflected_types, 1);
        assert_eq!(summary.component_descriptors, 1);
        assert_eq!(summary.component_descriptor_name_collisions, 0);
        assert_eq!(summary.class_registration_records, 1);
        assert_eq!(summary.class_registration_type_ids, 1);
        assert_eq!(summary.porting_components, 1);
        assert_eq!(
            unit.catalog
                .descriptor_type_id_by_name("Example::CounterComponent"),
            Some(uuid::uuid!("11111111-1111-1111-1111-111111111111"))
        );
        assert_eq!(
            unit.catalog
                .descriptor_by_type_id(uuid::uuid!("11111111-1111-1111-1111-111111111111"))
                .and_then(|descriptor| descriptor.module_name.as_deref()),
            Some("Example")
        );
        assert_eq!(
            unit.catalog
                .class_registration_by_type_id(uuid::uuid!("11111111-1111-1111-1111-111111111111"))
                .and_then(|record| record.return_address.as_deref()),
            Some("NewWorld+0x440")
        );
    }

    #[test]
    fn compile_from_paths_reads_module_descriptor_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let serialize_context = temp.path().join("serialize.json");
        let modules = temp.path().join("modules");
        fs::create_dir_all(&modules).expect("create modules directory");

        fs::write(
            &serialize_context,
            json!({
                "$id": 1,
                "uuidMap": {},
                "classNameToUuid": [],
                "uuidGenericMap": [],
                "uuidAnyCreationMap": {},
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            })
            .to_string(),
        )
        .expect("write serialize context");
        fs::write(
            modules.join("alpha-module.json"),
            json!({
                "descriptors": [{
                    "componentName": "SharedComponent",
                    "componentUuid": "aaaaaaaa-1111-2222-3333-444444444444",
                    "vftable": "NewWorld+0x200",
                    "addr": "NewWorld+0x300",
                    "vtableSlots": [{
                        "slot": 3,
                        "expected": "Reflect",
                        "address": "NewWorld+0x400"
                    }]
                }]
            })
            .to_string(),
        )
        .expect("write alpha module");
        fs::write(
            modules.join("beta-module.json"),
            json!({
                "descriptors": [{
                    "componentName": "SharedComponent",
                    "componentUuid": "bbbbbbbb-1111-2222-3333-444444444444"
                }]
            })
            .to_string(),
        )
        .expect("write beta module");
        fs::write(
            modules.join("ignored.debug.json"),
            json!({
                "descriptors": [{
                    "componentName": "IgnoredComponent",
                    "componentUuid": "cccccccc-1111-2222-3333-444444444444"
                }]
            })
            .to_string(),
        )
        .expect("write ignored debug capture");

        let unit = SerializeContextCompiler::compile_from_paths(
            &serialize_context,
            Some(&modules),
            None::<&std::path::Path>,
            &CodegenContext::inline(),
        )
        .expect("compile from module directory");

        let summary = unit.catalog.summary();
        assert_eq!(summary.component_descriptors, 2);
        assert_eq!(summary.component_descriptor_name_collisions, 1);
        let alpha = unit
            .catalog
            .descriptor_by_type_id(uuid::uuid!("aaaaaaaa-1111-2222-3333-444444444444"))
            .expect("alpha descriptor");
        assert_eq!(alpha.module_name.as_deref(), Some("Alpha"));
        assert_eq!(alpha.vtable_slots.len(), 1);
        assert_eq!(alpha.vtable_slots[0].expected.as_deref(), Some("Reflect"));
        assert_eq!(
            unit.catalog.descriptor_type_id_by_name("SharedComponent"),
            None
        );
        assert_eq!(
            unit.catalog.descriptor_type_ids_by_name("SharedComponent"),
            &[
                uuid::uuid!("aaaaaaaa-1111-2222-3333-444444444444"),
                uuid::uuid!("bbbbbbbb-1111-2222-3333-444444444444"),
            ]
        );
        assert_eq!(
            unit.catalog.descriptor_type_id_by_name("IgnoredComponent"),
            None
        );
    }

    #[test]
    fn compile_unit_emits_supported_language_sources_from_shared_ir() {
        let document = SerializeContextDocument::from_value_unchecked(json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "AZ::Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "Example::CounterComponent",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "elements": [
                        {
                            "$id": 21,
                            "name": "AZ::Component",
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "is_base_class": true
                        },
                        {
                            "$id": 22,
                            "name": "m_count",
                            "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                            "is_base_class": false
                        }
                    ],
                    "attributes": []
                }
            },
            "classNameToUuid": [[1, "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"]],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let unit = SerializeContextCompiler::compile(document, &CodegenContext::inline());
        let selected_view = unit.codegen_view(SerializeCodegenSelection::RuntimeRoots);

        let context = crate::CodegenContext::inline();
        let analysis = unit.layout_analysis_report();
        let layout = unit.standalone_rust_layout_report(&context);
        let selected_layout = unit.selected_standalone_rust_layout_report(
            SerializeCodegenSelection::RuntimeRoots,
            &context,
        );
        let go_layout = unit.standalone_go_layout_report();
        let selected_go_layout =
            unit.selected_standalone_go_layout_report(SerializeCodegenSelection::RuntimeRoots);
        let typescript_layout = unit.standalone_typescript_layout_report();
        let selected_typescript_layout = unit
            .selected_standalone_typescript_layout_report(SerializeCodegenSelection::RuntimeRoots);
        let rust = unit.emit_rust_source(&context).expect("Rust source");
        let typescript = unit
            .emit_typescript_source_with_options(&TypeScriptSourceOptions {
                include_support_aliases: false,
            })
            .expect("TypeScript source");
        let go = unit
            .emit_go_source_with_options(&GoSourceOptions {
                package_name: "nwtypes".to_owned(),
                include_support_aliases: false,
            })
            .expect("Go source");
        let rust_project = unit
            .emit_standalone_rust_project(&context)
            .expect("standalone Rust project");
        let selected_rust_project = unit
            .emit_selected_standalone_rust_project(
                SerializeCodegenSelection::RuntimeRoots,
                &context,
            )
            .expect("selected standalone Rust project");
        let go_project = unit
            .emit_standalone_go_project("aztypesvalidation", "aztypesvalidation", &context)
            .expect("standalone Go project");
        let selected_go_project = unit
            .emit_selected_standalone_go_project(
                SerializeCodegenSelection::RuntimeRoots,
                "aztypesvalidation",
                "aztypesvalidation",
                &context,
            )
            .expect("selected standalone Go project");
        let typescript_project = unit
            .emit_standalone_typescript_project_with_options(
                &TypeScriptStandaloneProjectOptions {
                    package_name: "aztypes-typescript-validation".to_owned(),
                    pack_entries: vec!["src/index.ts".to_owned()],
                },
                &context,
            )
            .expect("standalone TypeScript project");
        let selected_typescript_project = unit
            .emit_selected_standalone_typescript_project(
                SerializeCodegenSelection::RuntimeRoots,
                &context,
            )
            .expect("selected standalone TypeScript project");

        assert_eq!(selected_view.emitted_unit().items.len(), 2);
        assert_eq!(
            selected_view.context_unit().items.len(),
            unit.codegen_unit.items.len()
        );
        assert_eq!(
            selected_view
                .standalone_rust_layout_report(&context)
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            selected_layout
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            selected_view
                .standalone_go_layout_report()
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            selected_go_layout
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            selected_view
                .standalone_typescript_layout_report()
                .type_files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            selected_typescript_layout
                .type_files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>()
        );

        let counter_analysis = analysis
            .item_by_source_name("Example::CounterComponent")
            .expect("CounterComponent layout analysis");
        assert_eq!(
            counter_analysis.emitted_scope_segments,
            vec!["example", "components"]
        );
        assert!(
            counter_analysis
                .primary_base_chain
                .iter()
                .any(|edge| { edge.source_name == "AZ::Component" && edge.matches_reflected_type })
        );
        assert!(
            layout
                .files
                .iter()
                .any(|file| file.path == "src/types/example/components/counter_component.rs")
        );
        assert!(
            selected_layout
                .files
                .iter()
                .any(|file| file.path == "src/types/example/components/counter_component.rs")
        );
        assert!(
            go_layout
                .files
                .iter()
                .any(|file| file.path == "types/example/components/counter_component.go")
        );
        assert!(
            selected_go_layout
                .files
                .iter()
                .any(|file| file.path == "types/example/components/counter_component.go")
        );
        assert!(
            typescript_layout
                .type_files
                .iter()
                .any(|file| { file.path == "src/types/example/components/counter_component.ts" })
        );
        assert!(
            selected_typescript_layout
                .type_files
                .iter()
                .any(|file| file.path == "src/types/example/components/counter_component.ts")
        );
        assert!(
            rust_project
                .files
                .iter()
                .any(|file| { file.path == "src/types/example/components/counter_component.rs" })
        );
        assert!(
            selected_rust_project
                .files
                .iter()
                .any(|file| { file.path == "src/types/example/components/counter_component.rs" })
        );
        assert!(
            go_project
                .files
                .iter()
                .any(|file| file.path == "types/example/components/counter_component.go")
        );
        assert!(
            selected_go_project
                .files
                .iter()
                .any(|file| file.path == "types/example/components/counter_component.go")
        );
        assert!(
            typescript_project
                .files
                .iter()
                .any(|file| { file.path == "src/types/example/components/counter_component.ts" })
        );
        assert!(typescript_project.files.iter().any(|file| {
            file.path == "package.json"
                && file
                    .source
                    .contains("\"name\": \"aztypes-typescript-validation\"")
        }));
        assert!(
            selected_typescript_project
                .files
                .iter()
                .any(|file| { file.path == "src/types/example/components/counter_component.ts" })
        );
        assert!(rust.contains("AzRtti"));
        assert!(rust.contains("Component)]"));
        assert!(rust.contains("pub count: u32"));
        assert!(typescript.contains("export interface CounterComponent"));
        assert!(typescript.contains("count: number;"));
        assert!(go.contains("package nwtypes"));
        assert!(go.contains("type CounterComponent struct"));
        assert!(go.contains("Count uint32 `json:\"count\"`"));
    }
}
