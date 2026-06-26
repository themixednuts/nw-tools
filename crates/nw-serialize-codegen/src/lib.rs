//! SerializeContext compiler primitives for New World tooling.
//!
//! This crate owns the pipeline from the captured `serialize.json` document to
//! semantic reflected types and code generation inputs. Legacy ObjectStream
//! importers and CLIs consume this crate; they do not own SerializeContext
//! semantics.

#![recursion_limit = "512"]

pub mod catalog;
pub mod class_registration;
pub mod compiler;
pub mod completion;
pub mod component_scaffold;
pub mod context;
pub mod dependency_graph;
pub mod document;
pub mod field_evidence;
pub mod field_projection;
pub mod generate;
pub mod go;
pub mod graph;
pub mod ir;
pub mod layout;
pub mod lint;
pub mod model;
pub mod module_descriptors;
pub mod naming;
pub mod native;
pub mod network_rust;
pub mod network_schema;
pub mod reference;
pub mod role;
pub mod rust;
pub mod schema;
pub mod selection;
pub mod support_usage;
pub mod symbol_surface;
pub mod types;
pub mod typescript;
mod value;

pub use catalog::{
    ComponentDescriptor, ComponentDescriptorVtableSlot, ReflectedField, ReflectedGenericType,
    ReflectedType, ReflectedTypeCatalog, ReflectedTypeCatalogError, ReflectedTypeCatalogInputs,
    ReflectedTypeCatalogSummary, SerializePortingLedger,
};
pub use class_registration::{
    ClassRegistrationTraceIndex, ClassRegistrationTraceRecord, class_registration_trace_index,
    class_registration_trace_root_from_jsonl_str,
};
pub use compiler::{
    CompileUnit, SerializeCodegenView, SerializeContextCompileError, SerializeContextCompileInputs,
    SerializeContextCompiler,
};
pub use completion::{
    CompletedCodegenUnits, MissingReflectedBody, MissingReflectedBodyPlaceholder,
    complete_known_missing_reflected_bodies, missing_reflected_bodies_by_type,
};
pub use component_scaffold::{
    ComponentScaffoldError, ComponentScaffoldReport, ComponentScaffoldRequest,
    ExistingComponentReport, ExistingFieldReport, FacetOwnerEvidence, ModuleScaffoldAction,
    ModuleScaffoldReport, SkippedFieldReport, facet_owner_evidence_from_layout,
    scaffold_components,
};
pub use context::CodegenContext;
pub use dependency_graph::sorted_strongly_connected_components;
pub use document::{SerializeContextDocument, SerializeContextDocumentError};
pub use field_evidence::{
    FieldOwnerEvidence, FieldOwnerEvidenceIndex, FieldOwnerEvidenceSummary, FieldOwnerQuery,
    FieldOwnerResolution, FieldOwnerResolutionKind,
};
pub use field_projection::{
    CodegenFieldProjection, CodegenFieldTypeProjection, CodegenTypeReferenceProjection,
    base_class_has_materialized_payload, base_class_is_abstract, classify_codegen_field,
    classify_codegen_field_type, codegen_item_missing_type_ids,
    codegen_item_references_missing_type, item_has_materialized_payload,
    projected_missing_reflected_type_reasons, projected_missing_reflected_types,
};
pub use generate::{IntegratedRustProject, IntegratedRustProjectRequest};
pub use go::layout::{
    GoStandaloneLayoutFileReport, GoStandaloneLayoutItemReport, GoStandaloneLayoutReport,
};
pub use go::source::{
    GoSourceEmitError, GoSourceEmitter, GoSourceOptions, GoStandaloneProject,
    GoStandaloneProjectFile,
};
pub use go::types::{GoTypeOptions, GoTypeRenderer};
pub use graph::{
    FacetSide, SchemaEdge, SchemaEdgeKind, SchemaEdgeProvenance, SchemaGraph,
    SchemaGraphDiagnostic, SchemaGraphDiagnosticCode, SchemaNode, SchemaNodeKind,
};
pub use ir::{
    MissingReflectedType, SerializeCodegenField, SerializeCodegenIndex, SerializeCodegenItem,
    SerializeCodegenItemKind, SerializeCodegenPlanner, SerializeCodegenRttiBase,
    SerializeCodegenSelection, SerializeCodegenUnit, SerializeCodegenVariant,
    collect_resolved_named_type_ids,
};
pub use layout::{
    LayoutAnalysisItem, LayoutAnalysisReport, LayoutBaseEdge, LayoutConcreteSlotBinding,
    LayoutConcreteSlotCandidate, LayoutConcreteSlotMatchKind, LayoutIndex, LayoutPathSet,
    LayoutRootAudit, LayoutRootFinding, LayoutRootFindingKind, LayoutRootItem, LayoutRootReport,
    LayoutScopeDecision, LayoutScopeReason, LayoutSerializedShape, LayoutSlotAnchor,
    LayoutTypePath, concrete_slot_binding, concrete_slot_file_stem,
    concrete_slot_owner_scope_segments, emitted_scope_segments, has_concrete_slot_children,
    inheritance_family_scope_segments, inheritance_scope_segment, inheritance_scope_segments,
    layout_path_starts_with, reflected_base_type_ids, sanitize_path_segment,
    source_namespace_segments,
};
pub use lint::{Diagnostic, DiagnosticCode, Severity, lint_codegen_unit, lint_document};
pub use model::{
    ClassNameIndexEntry, ReflectedAttribute, ReflectedAttributeValue, ReflectedAzRtti,
    ReflectedAzRttiHierarchyEntry, ReflectedClass, ReflectedEnum, ReflectedEnumVariant,
    ReflectedGenericClass, ReflectedMember, ReflectedNonTypeTemplateArgument,
    SerializeContextModel,
};
pub use module_descriptors::{
    is_module_descriptor_json_name, module_descriptor_capture, module_descriptors_root,
    module_descriptors_root_from_capture, module_name_from_capture_stem, module_name_from_path,
    module_name_from_resource_name,
};
pub use naming::{
    CppCallingConvention, CppGetTypeNameFunction, ParsedSourceName, SourceNameKind,
    missing_reflected_type_name, rust_field_ident, rust_reflected_type_name, rust_type_ident,
    rust_type_name, rust_type_names_by_id,
};
pub use native::{NativeSymbol, NativeSymbolIndex, NativeSymbolUse, NativeSymbolUseKind};
pub use network_rust::{
    NETWORK_RUST_EMITTER_VERSION, NetworkRustEmitError, NetworkRustEmitter,
    NetworkRustGenerationReport, NetworkRustOutput, NetworkStateFieldShapeReport,
    NetworkStateGenerationPlanReport,
};
pub use network_schema::{
    NETWORK_SCHEMA_VERSION, NetworkAzRtti, NetworkAzRttiProvider, NetworkConfidence,
    NetworkEvidence, NetworkEvidenceKind, NetworkField, NetworkFieldOverride,
    NetworkFieldOverrideFile, NetworkFieldOverrideMergeReport, NetworkFieldRegistrationFunction,
    NetworkHandler, NetworkMessageFieldSignature, NetworkMessageSignature,
    NetworkMessageSignatureMergeReport, NetworkRegistrationHook,
    NetworkReplicatedContainerWireShape, NetworkRootKind, NetworkSchema, NetworkSchemaImportError,
    NetworkSchemaSource, NetworkSchemaSourceKind, NetworkSchemaSummary, NetworkSerializeKind,
    NetworkSerializeMergeReport, NetworkSerializeRole, NetworkSerializeType, NetworkType,
    NetworkTypeIndexMergeReport, NetworkVirtualFunction, NetworkWireScalarShape, NetworkWireShape,
};
pub use reference::{
    ReferenceExpansionContext, ReferenceIndex, ReferenceKey, ReferencePathSegment, ReferenceReport,
};
pub use role::{ReflectedTypeRole, RoleRootPolicy, SerializeRoleClassifier};
pub use rust::analyze::{
    RustFieldTypeMismatch, RustIdentityAttr, RustIdentityUpdate, RustItemKindMismatch,
    RustItemStatus, RustItemUpdatePlan, RustSourceAnalyzeError, RustSourceField, RustSourceFile,
    RustSourceItem, RustSourceVariant, RustVariantDiscriminantMismatch,
};
pub use rust::enum_plan::{RustEnumRawConversionPlan, RustVariantPlan};
pub use rust::identity::{RustTypeIdentityKind, RustTypeIdentityPlan};
pub use rust::integrate::source_index::{
    RustDeriveCapabilities, RustSourceTypeIndex, RustSourceTypeLocation,
};
pub use rust::integrate::{
    FlatRustItemPathResolver, RustIntegrationAction, RustIntegrationError, RustIntegrationItemPlan,
    RustIntegrationPlan, RustIntegrationPlanner, RustItemPathResolver, RustSourceInventory,
    RustSourceInventoryFile, RustSourceInventoryItem,
};
pub use rust::item_plan::{
    RustCodegenUnit, RustFieldPlan, RustIntegerRangePlan, RustItemKind, RustItemPlan,
    RustUnresolvedTypePlan,
};
pub use rust::layout::{
    RustStandaloneLayoutFileReport, RustStandaloneLayoutItemReport,
    RustStandaloneLayoutModuleReport, RustStandaloneLayoutReport,
};
pub use rust::options::{RustCodegenMode, RustCodegenOptions};
pub use rust::plan::RustCodegenPlanner;
pub use rust::source::{
    RustSourceEmitError, RustSourceEmitter, RustSourceMode, RustSourceOptions,
    RustStandaloneProject, RustStandaloneProjectFile,
};
pub use rust::types::{RustTypeOptions, RustTypeRenderer};
pub use selection::{
    SerializeCodegenRootMode, SerializeCodegenRootResolveError, SerializeCodegenRootSelection,
    resolve_codegen_root_type_id, resolve_codegen_root_type_ids,
};
pub use support_usage::{CodegenContainerSupportUsage, CodegenSupportUsage};
pub use types::{
    MapKind, PointerKind, ResolvedType, ScalarType, SequenceKind, TypeResolver, scalar_type,
};
pub use typescript::layout::{
    TypeScriptStandaloneIndexFileReport, TypeScriptStandaloneLayoutItemReport,
    TypeScriptStandaloneLayoutReport, TypeScriptStandaloneTypeFileReport,
};
pub use typescript::source::{
    TypeScriptSourceEmitError, TypeScriptSourceEmitter, TypeScriptSourceOptions,
    TypeScriptStandaloneProject, TypeScriptStandaloneProjectFile,
    TypeScriptStandaloneProjectOptions,
};
pub use typescript::types::{TypeScriptTypeOptions, TypeScriptTypeRenderer};
