//! SerializeContext compiler primitives for New World tooling.
//!
//! This crate owns the pipeline from the captured `serialize.json` document to
//! semantic reflected types and code generation inputs. Legacy ObjectStream
//! importers and CLIs consume this crate; they do not own SerializeContext
//! semantics.

#![recursion_limit = "512"]

/// Content fingerprint of the generator crate's source and embedded resources.
///
/// Build-script consumers should include this in generated-output cache keys so
/// a generator implementation change cannot reuse stale emitted source.
#[cfg(feature = "full")]
pub const GENERATOR_SOURCE_FINGERPRINT: &str = env!("NW_SERIALIZE_CODEGEN_SOURCE_FINGERPRINT");

/// Content fingerprint of the SerializeContext compiler and integrated Rust
/// emitter used by New World's generated runtime product.
///
/// Unlike the full-generator fingerprint, this excludes independent network,
/// Go, TypeScript, component-scaffold, and CLI surfaces so their implementation
/// changes do not rewrite an otherwise identical New World generated module.
pub const RUST_SERIALIZE_CONTEXT_EMITTER_COMPILER_FINGERPRINT: &str =
    env!("NW_SERIALIZE_CODEGEN_RUST_SERIALIZE_CONTEXT_EMITTER_COMPILER_FINGERPRINT");

pub mod catalog;
pub mod class_registration;
pub mod compiler;
pub mod completion;
#[cfg(feature = "full")]
pub mod component_scaffold;
pub mod context;
pub mod dependency_graph;
pub mod document;
pub mod field_evidence;
pub mod field_projection;
#[cfg(feature = "full")]
pub mod generate;
#[cfg(feature = "full")]
pub mod go;
pub mod graph;
pub mod ir;
pub mod layout;
pub mod lint;
pub mod model;
pub mod module_descriptors;
pub mod naming;
pub mod native;
#[cfg(feature = "full")]
pub mod network_rust;
#[cfg(feature = "full")]
pub mod network_schema;
#[cfg(feature = "full")]
pub mod network_selection;
pub mod reference;
pub mod role;
pub mod rust;
pub mod schema;
pub mod selection;
#[cfg(feature = "full")]
pub mod selection_manifest;
pub mod status;
#[cfg(feature = "full")]
pub mod support_usage;
pub mod symbol_surface;
pub mod types;
#[cfg(feature = "full")]
pub mod typescript;
mod uuid_format;
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
#[cfg(feature = "full")]
pub use compiler::SerializeCodegenView;
pub use compiler::{
    CompileUnit, SerializeContextCompileError, SerializeContextCompileInputs,
    SerializeContextCompiler,
};
pub use completion::{
    CompletedCodegenUnits, MissingReflectedBody, MissingReflectedBodyPlaceholder,
    complete_known_missing_reflected_bodies, missing_reflected_bodies_by_type,
};
#[cfg(feature = "full")]
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
#[cfg(feature = "full")]
pub use generate::{IntegratedRustProject, IntegratedRustProjectRequest};
#[cfg(feature = "full")]
pub use go::layout::{
    GoStandaloneLayoutFileReport, GoStandaloneLayoutItemReport, GoStandaloneLayoutReport,
};
#[cfg(feature = "full")]
pub use go::source::{
    GoSourceEmitError, GoSourceEmitter, GoSourceOptions, GoStandaloneProject,
    GoStandaloneProjectFile,
};
#[cfg(feature = "full")]
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
#[cfg(feature = "full")]
pub use network_rust::{
    NETWORK_RUST_EMITTER_VERSION, NetworkEvidenceIssue, NetworkEvidenceIssueKind,
    NetworkFixedSequenceFieldReport, NetworkReplicatedStateEmitOptions, NetworkRustEmitError,
    NetworkRustEmitter, NetworkRustGenerationReport, NetworkRustOutput,
    NetworkStateFieldShapeReport, NetworkStateGenerationPlanReport,
};
#[cfg(feature = "full")]
pub use network_schema::{
    NETWORK_SCHEMA_VERSION, NetworkAzRtti, NetworkAzRttiProvider, NetworkBooleanChoiceWireShape,
    NetworkConfidence, NetworkContainerCodec, NetworkContainerMemberSemantics,
    NetworkContainerPlanDiagnostic, NetworkEvidence, NetworkEvidenceKind, NetworkField,
    NetworkFieldOverride, NetworkFieldOverrideFile, NetworkFieldOverrideMergeReport,
    NetworkFieldRegistrationFunction, NetworkFixedSequenceShape, NetworkFixedSequenceStorageKind,
    NetworkFixedSequenceWireShape, NetworkGhidraOverlayMergeReport, NetworkHandler,
    NetworkMessageFieldSignature, NetworkMessageSignature, NetworkMessageSignatureMergeReport,
    NetworkNativeTypeInfoEvidence, NetworkRegistrationHook, NetworkReplicatedContainerPlan,
    NetworkReplicatedContainerWireShape, NetworkReplicatedStateAbiEvidence,
    NetworkReplicatedStateAbiFunction, NetworkReplicatedStateAbiKind, NetworkSchema,
    NetworkSchemaImportError, NetworkSchemaSource, NetworkSchemaSourceKind, NetworkSchemaSummary,
    NetworkSerializeField, NetworkSerializeKind, NetworkSerializeMergeReport, NetworkSerializeRole,
    NetworkSerializeType, NetworkType, NetworkTypeCapability, NetworkTypeIndexMergeReport,
    NetworkVirtualFunction, NetworkWireScalarShape, NetworkWireShape,
};
#[cfg(feature = "full")]
pub use network_selection::{NetworkSerializeRootPlan, NetworkSerializeRootPlanner};
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
pub use rust::integrate::wire;
pub use rust::integrate::wire::{WireFile, WireKind, WirePlan, WireSkip, WireSkipReason, WireType};
pub use rust::integrate::{
    FlatRustItemPathResolver, RustIntegrationAction, RustIntegrationError, RustIntegrationItemPlan,
    RustIntegrationPlan, RustIntegrationPlanner, RustItemPathResolver, RustSourceInventory,
    RustSourceInventoryFile, RustSourceInventoryItem,
};
pub use rust::item_plan::{
    RustCodegenUnit, RustFieldPlan, RustIntegerRangePlan, RustItemKind, RustItemPlan,
    RustPrefabPlan, RustUnresolvedTypePlan,
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
#[cfg(feature = "full")]
pub use selection_manifest::{
    SelectionManifestError, SerializeCodegenEngineOwnedTypeEntry, SerializeCodegenRootEntry,
    SerializeCodegenSelectionManifest,
};
pub use status::{
    CodegenStatus, CodegenStatusEvent, CodegenStatusKind, CodegenStatusPhase, CodegenStatusSink,
};
#[cfg(feature = "full")]
pub use support_usage::{CodegenContainerSupportUsage, CodegenSupportUsage};
pub use types::{
    MapKind, PointerKind, ResolvedType, ScalarType, SequenceKind, TypeResolver, scalar_type,
};
#[cfg(feature = "full")]
pub use typescript::layout::{
    TypeScriptStandaloneIndexFileReport, TypeScriptStandaloneLayoutItemReport,
    TypeScriptStandaloneLayoutReport, TypeScriptStandaloneTypeFileReport,
};
#[cfg(feature = "full")]
pub use typescript::source::{
    TypeScriptSourceEmitError, TypeScriptSourceEmitter, TypeScriptSourceOptions,
    TypeScriptStandaloneProject, TypeScriptStandaloneProjectFile,
    TypeScriptStandaloneProjectOptions,
};
#[cfg(feature = "full")]
pub use typescript::types::{TypeScriptTypeOptions, TypeScriptTypeRenderer};
