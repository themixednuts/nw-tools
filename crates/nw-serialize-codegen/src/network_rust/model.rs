use super::*;

pub const NETWORK_RUST_EMITTER_VERSION: &str = "network-rust-v57";

#[derive(Debug, Error)]
pub enum NetworkRustEmitError {
    #[error("generated network Rust source did not parse")]
    Parse(#[from] syn::Error),
    #[error("network Rust source emission was cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkRustOutput {
    pub source: String,
    pub report: NetworkRustGenerationReport,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRustGenerationReport {
    pub descriptor_count: usize,
    pub identity_type_count: usize,
    pub identity_name_collision_count: usize,
    pub field_descriptor_count: usize,
    pub unnamed_descriptor_count: usize,
    pub skipped_missing_type_id: usize,
    pub skipped_missing_type_index: usize,
    pub skipped_missing_name: usize,
    pub replicated_state_count: usize,
    pub message_count: usize,
    pub field_registered_count: usize,
    pub support_type_count: usize,
    pub low_confidence_field_count: usize,
    pub field_wire_shape_count: usize,
    pub unresolved_field_wire_shape_count: usize,
    pub state_generation_plan_count: usize,
    pub generatable_state_count: usize,
    pub blocked_state_count: usize,
    pub state_generation_plans: Vec<NetworkStateGenerationPlanReport>,
    pub message_generation_plan_count: usize,
    pub generatable_message_count: usize,
    pub blocked_message_count: usize,
    pub message_generation_plans: Vec<NetworkMessageGenerationPlanReport>,
    #[serde(default)]
    pub message_blocker_summary: NetworkBlockerSummaryReport,
    #[serde(default)]
    pub marshaler_conversion_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStateGenerationPlanReport {
    pub type_index: Option<u32>,
    pub type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_category_value: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_metadata_fragment: Option<bool>,
    pub field_count: usize,
    #[serde(default)]
    pub attribute_count: usize,
    pub shaped_field_count: usize,
    pub supported_field_count: usize,
    pub missing_wire_shape_count: usize,
    pub unsupported_wire_shape_count: usize,
    pub low_confidence_field_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_issues: Vec<NetworkEvidenceIssue>,
    pub can_generate: bool,
    pub blocked_reasons: Vec<String>,
    pub fields: Vec<NetworkStateFieldShapeReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStateFieldShapeReport {
    pub field_index: Option<u32>,
    pub field_name: Option<String>,
    pub group: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_group_attribute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialize_type_name: Option<String>,
    pub handler_vtable: Option<String>,
    pub wire_shape: Option<SchemaWireShape>,
    pub wire_shape_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_layout_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_type_candidates: Vec<NetworkNativeTypeInfoEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_key_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub container_embedded_key_type_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_value_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub container_embedded_value_type_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nested_embedded_type_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_sequence: Option<NetworkFixedSequenceFieldReport>,
    pub rust_value_type: Option<String>,
    pub rust_field_type: Option<String>,
    #[serde(default)]
    pub constructor_write_count: usize,
    pub confidence: NetworkConfidence,
    pub supported: bool,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMessageGenerationPlanReport {
    pub type_index: Option<u32>,
    pub type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis_status: Option<crate::network_schema::NetworkMessageAnalysisStatus>,
    #[serde(default)]
    pub empty_wire_proven: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_unmarshal: Option<bool>,
    pub field_count: usize,
    pub shaped_field_count: usize,
    pub supported_field_count: usize,
    pub missing_wire_shape_count: usize,
    #[serde(default)]
    pub missing_field_type_count: usize,
    #[serde(default)]
    pub missing_support_type_count: usize,
    #[serde(default)]
    pub missing_composite_support_type_count: usize,
    #[serde(default)]
    pub placeholder_field_name_count: usize,
    pub unsupported_wire_shape_count: usize,
    pub low_confidence_field_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_issues: Vec<NetworkEvidenceIssue>,
    pub can_generate: bool,
    pub blocked_reasons: Vec<String>,
    pub fields: Vec<NetworkStateFieldShapeReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockerSummaryReport {
    pub total_plan_count: usize,
    pub generatable_count: usize,
    pub blocked_count: usize,
    pub reason_buckets: Vec<NetworkBlockerReasonBucketReport>,
    pub combination_buckets: Vec<NetworkBlockerCombinationBucketReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockerReasonBucketReport {
    pub reason: String,
    pub type_count: usize,
    pub blocked_field_count: usize,
    pub examples: Vec<NetworkBlockedTypeExampleReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockerCombinationBucketReport {
    pub reasons: Vec<String>,
    pub type_count: usize,
    pub examples: Vec<NetworkBlockedTypeExampleReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockedTypeExampleReport {
    pub type_index: Option<u32>,
    pub type_name: Option<String>,
    pub field_count: usize,
    pub blocked_reasons: Vec<String>,
    pub blocked_fields: Vec<NetworkBlockedFieldExampleReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockedFieldExampleReport {
    pub field_index: Option<u32>,
    pub field_name: Option<String>,
    pub native_type: Option<String>,
    pub source_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialize_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_layout_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_type_candidates: Vec<NetworkNativeTypeInfoEvidence>,
    pub rust_value_type: Option<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Default)]
pub struct NetworkRustEmitter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkReplicatedStateEmitOptions {
    pub register_fragments: bool,
    pub registered_type_indices: Option<BTreeSet<u32>>,
}

impl Default for NetworkReplicatedStateEmitOptions {
    fn default() -> Self {
        Self {
            register_fragments: true,
            registered_type_indices: None,
        }
    }
}

impl NetworkReplicatedStateEmitOptions {
    pub fn unregistered() -> Self {
        Self {
            register_fragments: false,
            registered_type_indices: None,
        }
    }

    pub fn register_only(type_indices: impl IntoIterator<Item = u32>) -> Self {
        Self {
            register_fragments: true,
            registered_type_indices: Some(type_indices.into_iter().collect()),
        }
    }

    pub(super) fn registers_type_index(&self, type_index: u32) -> bool {
        self.register_fragments
            && self
                .registered_type_indices
                .as_ref()
                .is_none_or(|type_indices| type_indices.contains(&type_index))
    }
}
