use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkHandler {
    pub destructor: Option<String>,
    pub get_empty_value: Option<String>,
    pub create_instance: Option<String>,
    pub copy_value: Option<String>,
    pub marshal: Option<String>,
    pub unmarshal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInstanceLayout {
    pub create_instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis_status: Option<NetworkMessageAnalysisStatus>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub empty_wire_proven: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_wire_evidence_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_unmarshal: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructor_callsite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructor_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegated_codec: Option<NetworkDelegatedCodec>,
    pub evidence: Vec<NetworkEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkMessageAnalysisStatus {
    RecoveredFields,
    MarshalOnly,
    DelegatedCodec,
    ProvenEmpty,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkDelegatedCodec {
    pub kind: String,
    pub function: String,
    pub callsite: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_storage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome_storage: Option<String>,
    pub read_buffer_storage: String,
    pub evidence_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAzRtti {
    pub source: Option<String>,
    pub address: Option<String>,
    pub type_id: Option<Uuid>,
    pub type_name: Option<String>,
    pub providers: Vec<NetworkAzRttiProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAzRttiProvider {
    pub kind: Option<String>,
    pub slot: Option<u32>,
    pub slot_offset: Option<String>,
    pub function: Option<String>,
    pub provider: Option<String>,
    pub type_id: Option<Uuid>,
    pub type_id_source: Option<String>,
    pub type_name: Option<String>,
    pub source_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRegistrationHook {
    pub type_id: Option<Uuid>,
    pub type_name: Option<String>,
    pub slot_type_name: Option<String>,
    pub hook_function: Option<String>,
    pub helper_table: Option<String>,
    pub register_thunk: Option<String>,
    pub type_provider: Option<String>,
    pub uuid_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkEvidence {
    pub kind: NetworkEvidenceKind,
    pub source: String,
    pub address: Option<String>,
    pub detail: Option<String>,
    pub confidence: NetworkConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkEvidenceKind {
    TypeRegistry,
    TypeIndex,
    SerializeContext,
    HandlerVtable,
    InstallRegistrationHook,
    AzRtti,
    InstanceLayout,
    RegisterField,
    FieldRegistrationFunction,
    MessageUnmarshal,
    MessageMarshal,
    MessageSource,
    FieldOverride,
    FragmentMetadata,
    ReplicatedStateAbi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkConfidence {
    Exact,
    High,
    Inferred,
    Weak,
    Unknown,
}

impl NetworkConfidence {
    #[must_use]
    pub const fn is_high_or_exact(self) -> bool {
        matches!(self, Self::Exact | Self::High)
    }
}
