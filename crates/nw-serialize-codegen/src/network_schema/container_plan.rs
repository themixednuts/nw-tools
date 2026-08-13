use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::{
    NetworkReplicatedContainerStorageKind, NetworkWireScalarShape, array_values, bool_value,
    parse_network_wire_scalar_shape, parse_network_wire_shape, string, string_ref, u32_value, uuid,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkContainerMemberSemantics {
    LinearSequence,
    CountedSequence,
    FixedSequence,
    StructuredValue,
    OptionalSuffix,
    CfgReachable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkContainerCodecGuard {
    pub branch: Option<String>,
    pub kind: String,
    pub condition: String,
    pub member_on_true: bool,
    pub storage_base: Option<String>,
    pub storage_offset: Option<String>,
    pub storage_address: Option<String>,
    pub mask: Option<String>,
    pub external_condition: Option<NetworkContainerExternalBooleanCondition>,
    pub evidence_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkContainerExternalBooleanCondition {
    pub resolver_object: Option<String>,
    pub resolver_vtable: Option<String>,
    pub resolver_slot: Option<u32>,
    pub resolver: Option<String>,
    pub condition_storage: Option<String>,
    pub condition_offset: Option<String>,
    pub owner: Option<String>,
    pub subobject_offset: Option<String>,
    pub destructor_thunk: Option<String>,
    pub complete_destructor: Option<String>,
    pub initializer: Option<String>,
    pub name_field: Option<String>,
    pub name_offset: Option<String>,
    pub name_begin: Option<String>,
    pub name_end: Option<String>,
    pub name: Option<String>,
    pub default_value: Option<bool>,
    #[serde(skip)]
    pub profile_value: Option<bool>,
    pub default_write: Option<String>,
    pub default_callsite: Option<String>,
    pub default_target: Option<String>,
    pub evidence_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkContainerCodec {
    pub callsite: Option<String>,
    pub target: Option<String>,
    pub target_name: Option<String>,
    pub native_type: Option<String>,
    pub type_id: Option<Uuid>,
    pub type_id_source: Option<String>,
    #[serde(default)]
    pub type_identity_proven: bool,
    #[serde(default)]
    pub source_type_layout_complete: bool,
    pub wire_shape: Option<String>,
    pub wire_shape_source: Option<String>,
    pub wire_layout: Option<String>,
    pub wire_layout_source: Option<String>,
    pub evidence_source: Option<String>,
    pub member_semantics: Option<NetworkContainerMemberSemantics>,
    pub analysis_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_offset: Option<u64>,
    pub members: Vec<Self>,
    #[serde(default)]
    pub optional_members: Vec<Self>,
    #[serde(default)]
    pub guards: Vec<NetworkContainerCodecGuard>,
}

impl NetworkContainerCodec {
    fn apply_external_boolean_profile(&mut self, name: &str, value: bool) -> usize {
        let mut matched = 0;
        for guard in &mut self.guards {
            let Some(condition) = guard.external_condition.as_mut() else {
                continue;
            };
            if condition.name.as_deref() == Some(name) {
                condition.profile_value = Some(value);
                matched += 1;
            }
        }
        matched
            + self
                .members
                .iter_mut()
                .chain(&mut self.optional_members)
                .map(|member| member.apply_external_boolean_profile(name, value))
                .sum::<usize>()
    }

    /// Returns the physical scalar operations performed by this codec.
    ///
    /// Counted sequences retain their logical collection shape on the parent
    /// codec while their children describe the count prefix and one element.
    pub fn exact_wire_shapes(&self) -> Option<Vec<NetworkWireScalarShape>> {
        if !self.guards.is_empty()
            || !self.optional_members.is_empty()
            || self.member_semantics == Some(NetworkContainerMemberSemantics::OptionalSuffix)
        {
            return None;
        }
        if self.members.is_empty() {
            return self
                .wire_shape
                .as_deref()
                .or(self.wire_layout.as_deref())
                .and_then(parse_network_wire_scalar_shape)
                .map(|shape| vec![shape]);
        }
        if !matches!(
            self.member_semantics,
            Some(
                NetworkContainerMemberSemantics::LinearSequence
                    | NetworkContainerMemberSemantics::CountedSequence
                    | NetworkContainerMemberSemantics::FixedSequence
                    | NetworkContainerMemberSemantics::StructuredValue
            )
        ) {
            return None;
        }

        let mut shapes = Vec::new();
        for member in &self.members {
            shapes.extend(member.exact_wire_shapes()?);
        }
        (!shapes.is_empty()).then_some(shapes)
    }

    pub(crate) fn default_profile_wire_shapes(&self) -> Option<Vec<NetworkWireScalarShape>> {
        if self.member_semantics != Some(NetworkContainerMemberSemantics::OptionalSuffix) {
            return self.exact_wire_shapes();
        }
        self.guards.is_empty().then_some(())?;

        let mut shapes = exact_codec_sequence(&self.members)?;
        let mut include_suffix = None;
        let mut condition = None;
        for member in &self.optional_members {
            let [guard] = member.guards.as_slice() else {
                return None;
            };
            let external = guard.external_condition.as_ref()?;
            if let Some(expected) = condition {
                (expected == external).then_some(())?;
            } else {
                condition = Some(external);
            }
            let include = guard.includes_member_for_effective_profile()?;
            if let Some(expected) = include_suffix {
                (expected == include).then_some(())?;
            } else {
                include_suffix = Some(include);
            }
            if include {
                shapes.extend(member.wire_shapes_ignoring_external_guard()?);
            }
        }
        condition?;
        (!shapes.is_empty()).then_some(shapes)
    }

    fn wire_shapes_ignoring_external_guard(&self) -> Option<Vec<NetworkWireScalarShape>> {
        self.guards
            .iter()
            .all(|guard| guard.external_condition.is_some())
            .then_some(())?;
        self.optional_members.is_empty().then_some(())?;
        if self.members.is_empty() {
            return self
                .wire_shape
                .as_deref()
                .or(self.wire_layout.as_deref())
                .and_then(parse_network_wire_scalar_shape)
                .map(|shape| vec![shape]);
        }
        if !matches!(
            self.member_semantics,
            Some(
                NetworkContainerMemberSemantics::LinearSequence
                    | NetworkContainerMemberSemantics::CountedSequence
                    | NetworkContainerMemberSemantics::FixedSequence
                    | NetworkContainerMemberSemantics::StructuredValue
            )
        ) {
            return None;
        }
        exact_codec_sequence(&self.members)
    }

    /// Returns the logical wire members represented by this codec.
    ///
    /// Linear helper boundaries are transparent. A counted sequence is one
    /// logical member even though its physical encoding contains a count and
    /// an element codec.
    pub fn exact_logical_wire_shapes(&self) -> Option<Vec<String>> {
        if !self.guards.is_empty()
            || !self.optional_members.is_empty()
            || self.member_semantics == Some(NetworkContainerMemberSemantics::OptionalSuffix)
        {
            return None;
        }
        if matches!(
            self.member_semantics,
            Some(
                NetworkContainerMemberSemantics::CountedSequence
                    | NetworkContainerMemberSemantics::FixedSequence
            )
        ) {
            return self.wire_shape.clone().map(|shape| vec![shape]);
        }
        if self.members.is_empty() {
            return self.wire_shape.clone().map(|shape| vec![shape]);
        }
        if self.member_semantics == Some(NetworkContainerMemberSemantics::StructuredValue) {
            return self
                .native_type
                .clone()
                .map(|native_type| vec![native_type]);
        }
        if self.member_semantics != Some(NetworkContainerMemberSemantics::LinearSequence) {
            return None;
        }

        let mut shapes = Vec::new();
        for member in &self.members {
            shapes.extend(member.exact_logical_wire_shapes()?);
        }
        (!shapes.is_empty()).then_some(shapes)
    }

    fn decoded_logical_wire_shapes(&self) -> Option<Vec<String>> {
        self.guards
            .iter()
            .all(decode_guard_is_transient)
            .then_some(())?;
        if self.member_semantics == Some(NetworkContainerMemberSemantics::OptionalSuffix) {
            return self.default_profile_wire_shapes().map(|shapes| {
                shapes
                    .into_iter()
                    .map(NetworkWireScalarShape::wire_string)
                    .collect()
            });
        }
        if matches!(
            self.member_semantics,
            Some(
                NetworkContainerMemberSemantics::CountedSequence
                    | NetworkContainerMemberSemantics::FixedSequence
            )
        ) {
            return self.wire_shape.clone().map(|shape| vec![shape]);
        }
        if self.members.is_empty() {
            return self
                .wire_shape
                .clone()
                .or_else(|| self.wire_layout.clone())
                .map(|shape| vec![shape]);
        }
        if !self.optional_members.is_empty()
            || !matches!(
                self.member_semantics,
                Some(
                    NetworkContainerMemberSemantics::LinearSequence
                        | NetworkContainerMemberSemantics::CfgReachable
                        | NetworkContainerMemberSemantics::StructuredValue
                )
            )
        {
            return None;
        }

        let mut shapes = Vec::new();
        for member in &self.members {
            shapes.extend(member.decoded_logical_wire_shapes()?);
        }
        (!shapes.is_empty()).then_some(shapes)
    }

    fn registered_profile_logical_wire_shapes_unguarded(&self) -> Option<Vec<String>> {
        if matches!(
            self.member_semantics,
            Some(
                NetworkContainerMemberSemantics::CountedSequence
                    | NetworkContainerMemberSemantics::FixedSequence
            )
        ) {
            return self.wire_shape.clone().map(|shape| vec![shape]);
        }
        if self.members.is_empty() {
            return self
                .wire_shape
                .clone()
                .or_else(|| self.wire_layout.clone())
                .map(|shape| vec![shape]);
        }
        if !matches!(
            self.member_semantics,
            Some(
                NetworkContainerMemberSemantics::LinearSequence
                    | NetworkContainerMemberSemantics::CfgReachable
                    | NetworkContainerMemberSemantics::StructuredValue
                    | NetworkContainerMemberSemantics::OptionalSuffix
            )
        ) || (!self.optional_members.is_empty()
            && self.member_semantics != Some(NetworkContainerMemberSemantics::OptionalSuffix))
            || (self.member_semantics == Some(NetworkContainerMemberSemantics::CfgReachable)
                && !self.contains_profiled_mask_guard())
        {
            return None;
        }

        let mut shapes = registered_profile_logical_codec_sequence(&self.members)?;
        if !self.optional_members.is_empty() {
            shapes.extend(registered_profile_logical_codec_sequence(
                &self.optional_members,
            )?);
        }
        (!shapes.is_empty()).then_some(shapes)
    }

    fn contains_profiled_mask_guard(&self) -> bool {
        self.guards
            .iter()
            .any(|guard| guard.kind == "storage-bit-mask")
            || self.members.iter().any(Self::contains_profiled_mask_guard)
            || self
                .optional_members
                .iter()
                .any(Self::contains_profiled_mask_guard)
    }

    pub fn direct_type_name(&self) -> Option<&str> {
        (self.guards.is_empty() && self.optional_members.is_empty()).then_some(())?;
        (self.members.is_empty()
            || self.member_semantics == Some(NetworkContainerMemberSemantics::StructuredValue))
        .then_some(())?;
        self.native_type.as_deref()
    }

    #[must_use]
    pub fn contains_non_linear_members(&self) -> bool {
        !self.guards.is_empty()
            || matches!(
                self.member_semantics,
                Some(
                    NetworkContainerMemberSemantics::CfgReachable
                        | NetworkContainerMemberSemantics::OptionalSuffix
                )
            )
            || self.members.iter().any(Self::contains_non_linear_members)
            || self
                .optional_members
                .iter()
                .any(Self::contains_non_linear_members)
    }
}

impl NetworkContainerCodecGuard {
    fn includes_member_for_effective_profile(&self) -> Option<bool> {
        (self.kind == "global-boolean" && self.mask.is_none()).then_some(())?;
        let condition = self.external_condition.as_ref()?;
        condition.has_complete_proof().then_some(())?;
        let value = condition.profile_value.or(condition.default_value)?;
        let branch_on_true = match self.condition.as_str() {
            "equal-zero" => !value,
            "not-equal-zero" => value,
            _ => return None,
        };
        Some(if self.member_on_true {
            branch_on_true
        } else {
            !branch_on_true
        })
    }
}

impl NetworkContainerExternalBooleanCondition {
    fn has_complete_proof(&self) -> bool {
        self.resolver_object.is_some()
            && self.resolver_vtable.is_some()
            && self.resolver_slot.is_some()
            && self.resolver.is_some()
            && self.condition_storage.is_some()
            && self.condition_offset.is_some()
            && self.owner.is_some()
            && self.subobject_offset.is_some()
            && self.destructor_thunk.is_some()
            && self.complete_destructor.is_some()
            && self.initializer.is_some()
            && self.name_field.is_some()
            && self.name_offset.is_some()
            && self.name_begin.is_some()
            && self.name_end.is_some()
            && self.name.as_ref().is_some_and(|name| !name.is_empty())
            && self.default_value.is_some()
            && self.default_write.is_some()
            && self.default_callsite.is_some()
            && self.default_target.is_some()
            && self.evidence_source
                == "static-vtable-dispatch+adjustor-thunk+initializer-writes+resolver-default-flow"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkReplicatedContainerPlan {
    pub marshal_full: Option<String>,
    pub analysis_function: Option<String>,
    pub loop_header: Option<String>,
    pub storage: NetworkReplicatedContainerStorageKind,
    pub element_stride: Option<u64>,
    pub induction_source: Option<String>,
    pub unmarshal_storage_proof: Option<String>,
    pub unmarshal_reconciliation: Option<String>,
    pub unmarshal_analysis_status: Option<String>,
    #[serde(default)]
    pub sequence_marker_proven: bool,
    pub helper_depth: u32,
    pub key_codecs: Vec<NetworkContainerCodec>,
    pub value_codecs: Vec<NetworkContainerCodec>,
    #[serde(default)]
    pub unmarshal_codecs: Vec<NetworkContainerCodec>,
}

impl NetworkReplicatedContainerPlan {
    /// Returns the exact physical value sequence observed in MarshalFull.
    ///
    /// This intentionally does not claim cross-direction agreement. Callers
    /// must supply an independent unmarshal-side identity or layout proof.
    pub fn exact_marshal_value_wire_shapes(&self) -> Option<Vec<NetworkWireScalarShape>> {
        exact_codec_sequence(&self.value_codecs)
    }

    pub fn exact_key_wire_shapes(&self) -> Option<Vec<NetworkWireScalarShape>> {
        self.has_complete_cross_direction_agreement()
            .then_some(())?;
        exact_codec_sequence(&self.key_codecs)
    }

    /// Returns an exact map-key sequence when the value codec is too complex
    /// for whole-container reconciliation but unmarshal still consumes the
    /// independently observed key first.
    pub fn independently_matching_key_wire_shapes(&self) -> Option<Vec<NetworkWireScalarShape>> {
        (self.storage == NetworkReplicatedContainerStorageKind::Map).then_some(())?;
        (self.unmarshal_storage_proof.as_deref()
            == Some("cfg-ordered-buffer-codecs+unique-owner-insertion"))
        .then_some(())?;

        let encoded = exact_codec_sequence(&self.key_codecs)?;
        let mut decoded = Vec::new();
        for codec in &self.unmarshal_codecs {
            decoded.extend(codec.exact_wire_shapes()?);
            if decoded.len() >= encoded.len() {
                return (decoded == encoded).then_some(encoded);
            }
        }
        None
    }

    pub fn exact_value_wire_shapes(&self) -> Option<Vec<NetworkWireScalarShape>> {
        self.has_complete_cross_direction_agreement()
            .then_some(())?;
        exact_codec_sequence(&self.value_codecs)
    }

    /// Resolves externally gated wire members using the selected runtime
    /// profile, falling back to the default registered by the analyzed binary.
    pub fn default_profile_value_wire_shapes(&self) -> Option<Vec<NetworkWireScalarShape>> {
        self.has_complete_cross_direction_agreement()
            .then_some(())?;
        let mut shapes = Vec::new();
        for codec in &self.value_codecs {
            shapes.extend(codec.default_profile_wire_shapes()?);
        }
        (!shapes.is_empty()).then_some(shapes)
    }

    /// Selects one runtime value for every matching external boolean condition
    /// in this plan. The extracted native default remains unchanged.
    pub fn apply_external_boolean_profile(&mut self, name: &str, value: bool) -> usize {
        self.key_codecs
            .iter_mut()
            .chain(&mut self.value_codecs)
            .chain(&mut self.unmarshal_codecs)
            .map(|codec| codec.apply_external_boolean_profile(name, value))
            .sum()
    }

    pub fn exact_logical_key_wire_shapes(&self) -> Option<Vec<String>> {
        self.has_complete_cross_direction_agreement()
            .then_some(())?;
        exact_logical_codec_sequence(&self.key_codecs)
    }

    pub fn exact_logical_value_wire_shapes(&self) -> Option<Vec<String>> {
        self.has_complete_cross_direction_agreement()
            .then_some(())?;
        exact_logical_codec_sequence(&self.value_codecs)
    }

    pub fn decoded_logical_value_wire_shapes(&self) -> Option<Vec<String>> {
        self.has_complete_cross_direction_agreement()
            .then_some(())?;
        self.decoded_logical_value_wire_shapes_unguarded()
    }

    /// Returns the complete unmarshal-side value product when the extractor
    /// proved one owned container loop even though marshal helper recursion
    /// prevented cross-direction reconciliation.
    pub fn complete_unmarshal_logical_value_wire_shapes(&self) -> Option<Vec<String>> {
        (self.unmarshal_analysis_status.as_deref() == Some("single-loop-codec-sequence"))
            .then_some(())?;
        match self.storage {
            NetworkReplicatedContainerStorageKind::Map => (self.unmarshal_storage_proof.as_deref()
                == Some("cfg-ordered-buffer-codecs+unique-owner-insertion"))
            .then_some(())?,
            NetworkReplicatedContainerStorageKind::Vec => (self.unmarshal_storage_proof.as_deref()
                == Some("cfg-natural-loop-affine-pointer-induction"))
            .then_some(())?,
        };
        self.unmarshal_codecs
            .iter()
            .all(|codec| {
                codec.members.is_empty()
                    || codec
                        .analysis_status
                        .as_deref()
                        .is_some_and(|status| status.starts_with("complete"))
            })
            .then_some(())?;
        self.decoded_logical_value_wire_shapes_unguarded()
    }

    fn decoded_logical_value_wire_shapes_unguarded(&self) -> Option<Vec<String>> {
        let mut decoded = decoded_logical_codec_sequence(&self.unmarshal_codecs)?;
        if self.storage == NetworkReplicatedContainerStorageKind::Map {
            let key = exact_logical_codec_sequence(&self.key_codecs)
                .or_else(|| decoded_logical_codec_sequence(&self.key_codecs))?;
            decoded.starts_with(&key).then_some(())?;
            decoded.drain(..key.len());
        }
        (!decoded.is_empty()).then_some(decoded)
    }

    /// Returns a profile-resolved marshal product when guarded CFG recovery is
    /// complete and the unmarshal side independently proves the same element
    /// boundary. The marshal product is retained because it carries mask and
    /// registered-feature semantics that a flat decode trace cannot express.
    pub fn complete_marshal_logical_value_wire_shapes(&self) -> Option<Vec<String>> {
        self.complete_unmarshal_logical_value_wire_shapes()?;
        self.value_codecs
            .iter()
            .all(|codec| {
                codec
                    .analysis_status
                    .as_deref()
                    .is_some_and(|status| status.starts_with("complete"))
            })
            .then_some(())?;
        registered_profile_logical_codec_sequence(&self.value_codecs)
    }

    /// Resolves semantic codec guards against the binary's registered default
    /// profile while retaining mask-driven wire products as one logical shape.
    pub fn registered_profile_logical_value_wire_shapes(&self) -> Option<Vec<String>> {
        self.has_complete_cross_direction_agreement()
            .then_some(())?;
        registered_profile_logical_codec_sequence(&self.value_codecs)
    }

    #[must_use]
    pub fn has_complete_cross_direction_agreement(&self) -> bool {
        self.unmarshal_reconciliation
            .as_deref()
            .is_some_and(|status| status.starts_with("complete-"))
            || self.has_independently_matching_logical_products()
    }

    fn has_independently_matching_logical_products(&self) -> bool {
        if self.unmarshal_codecs.is_empty() {
            return false;
        }

        let Some(mut encoded) = registered_profile_logical_codec_sequence(&self.key_codecs) else {
            return false;
        };
        let Some(values) = registered_profile_logical_codec_sequence(&self.value_codecs) else {
            return false;
        };
        encoded.extend(values);

        decoded_logical_codec_sequence(&self.unmarshal_codecs).is_some_and(|decoded| {
            !encoded.is_empty() && logical_products_machine_compatible(&encoded, &decoded)
        })
    }

    pub fn direct_value_type(&self) -> Option<(&str, Option<Uuid>)> {
        let [codec] = self.value_codecs.as_slice() else {
            return None;
        };
        codec.type_identity_proven.then_some(())?;
        (codec.members.is_empty() || codec.source_type_layout_complete).then_some(())?;
        Some((codec.direct_type_name()?, codec.type_id))
    }

    #[must_use]
    pub fn has_non_linear_key_codec(&self) -> bool {
        self.key_codecs
            .iter()
            .any(NetworkContainerCodec::contains_non_linear_members)
    }

    #[must_use]
    pub fn has_non_linear_value_codec(&self) -> bool {
        self.value_codecs
            .iter()
            .any(NetworkContainerCodec::contains_non_linear_members)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkContainerPlanDiagnostic {
    pub function: Option<String>,
    pub loop_header: Option<String>,
    pub callsite: Option<String>,
    pub target: Option<String>,
    pub target_name: Option<String>,
    pub stage: Option<String>,
    pub reason: Option<String>,
    pub expected_storage: Option<String>,
    pub pcode_buffer_slot: Option<u32>,
    pub abi_buffer_slot: Option<u32>,
    pub codec_count: Option<u32>,
    pub induction: Option<String>,
}

pub(super) fn parse_plan(object: &Map<String, Value>) -> Option<NetworkReplicatedContainerPlan> {
    let storage = match string_ref(object, "storageKind")? {
        "index-map" => NetworkReplicatedContainerStorageKind::Map,
        "vector" => NetworkReplicatedContainerStorageKind::Vec,
        _ => return None,
    };
    Some(NetworkReplicatedContainerPlan {
        marshal_full: string(object, "marshalFull"),
        analysis_function: string(object, "analysisFunction"),
        loop_header: string(object, "loopHeader"),
        storage,
        element_stride: hex_or_decimal_u64(object.get("elementStride")),
        induction_source: string(object, "inductionSource"),
        unmarshal_storage_proof: string(object, "unmarshalStorageProof"),
        unmarshal_reconciliation: string(object, "unmarshalReconciliation"),
        unmarshal_analysis_status: string(object, "unmarshalAnalysisStatus"),
        sequence_marker_proven: bool_value(object, "sequenceMarkerProven").unwrap_or(false),
        helper_depth: u32_value(object, "helperDepth").unwrap_or_default(),
        key_codecs: parse_codecs(object, "keyCodecs"),
        value_codecs: parse_codecs(object, "valueCodecs"),
        unmarshal_codecs: parse_codecs(object, "unmarshalCodecs"),
    })
}

pub(super) fn parse_diagnostics(
    object: &Map<String, Value>,
    key: &str,
) -> Vec<NetworkContainerPlanDiagnostic> {
    array_values(object, key)
        .filter_map(Value::as_object)
        .map(|diagnostic| NetworkContainerPlanDiagnostic {
            function: string(diagnostic, "function"),
            loop_header: string(diagnostic, "loopHeader"),
            callsite: string(diagnostic, "callsite"),
            target: string(diagnostic, "target"),
            target_name: string(diagnostic, "targetName"),
            stage: string(diagnostic, "stage"),
            reason: string(diagnostic, "reason"),
            expected_storage: string(diagnostic, "expectedStorage"),
            pcode_buffer_slot: u32_value(diagnostic, "pcodeBufferSlot"),
            abi_buffer_slot: u32_value(diagnostic, "abiBufferSlot"),
            codec_count: u32_value(diagnostic, "codecCount"),
            induction: string(diagnostic, "induction"),
        })
        .collect()
}

fn parse_codecs(object: &Map<String, Value>, key: &str) -> Vec<NetworkContainerCodec> {
    array_values(object, key)
        .filter_map(Value::as_object)
        .map(parse_codec)
        .collect()
}

fn parse_codec(object: &Map<String, Value>) -> NetworkContainerCodec {
    NetworkContainerCodec {
        callsite: string(object, "callsite"),
        target: string(object, "target"),
        target_name: string(object, "targetName"),
        native_type: string(object, "nativeType"),
        type_id: uuid(object, "typeId"),
        type_id_source: string(object, "typeIdSource"),
        type_identity_proven: object
            .get("typeIdentityProven")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        source_type_layout_complete: object
            .get("sourceTypeLayoutComplete")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        wire_shape: string(object, "wireShape"),
        wire_shape_source: string(object, "wireShapeSource"),
        wire_layout: string(object, "wireLayout"),
        wire_layout_source: string(object, "wireLayoutSource"),
        evidence_source: string(object, "evidenceSource"),
        analysis_status: string(object, "analysisStatus"),
        element_offset: hex_or_decimal_u64(object.get("elementOffset")),
        member_semantics: match string_ref(object, "memberSemantics") {
            Some("linear-sequence") => Some(NetworkContainerMemberSemantics::LinearSequence),
            Some("counted-sequence") => Some(NetworkContainerMemberSemantics::CountedSequence),
            Some("fixed-sequence") => Some(NetworkContainerMemberSemantics::FixedSequence),
            Some("structured-value") => Some(NetworkContainerMemberSemantics::StructuredValue),
            Some("optional-suffix") => Some(NetworkContainerMemberSemantics::OptionalSuffix),
            Some("cfg-reachable") => Some(NetworkContainerMemberSemantics::CfgReachable),
            _ => None,
        },
        members: parse_codecs(object, "members"),
        optional_members: parse_codecs(object, "optionalMembers"),
        guards: array_values(object, "guards")
            .filter_map(Value::as_object)
            .filter_map(parse_codec_guard)
            .collect(),
    }
}

fn parse_codec_guard(object: &Map<String, Value>) -> Option<NetworkContainerCodecGuard> {
    Some(NetworkContainerCodecGuard {
        branch: string(object, "branch"),
        kind: string_ref(object, "kind")?.to_owned(),
        condition: string_ref(object, "condition")?.to_owned(),
        member_on_true: object.get("memberOnTrue")?.as_bool()?,
        storage_base: string(object, "storageBase"),
        storage_offset: string(object, "storageOffset"),
        storage_address: string(object, "storageAddress"),
        mask: string(object, "mask"),
        external_condition: object
            .get("externalCondition")
            .and_then(Value::as_object)
            .and_then(parse_external_boolean_condition),
        evidence_source: string_ref(object, "evidenceSource")?.to_owned(),
    })
}

fn parse_external_boolean_condition(
    object: &Map<String, Value>,
) -> Option<NetworkContainerExternalBooleanCondition> {
    Some(NetworkContainerExternalBooleanCondition {
        resolver_object: string(object, "resolverObject"),
        resolver_vtable: string(object, "resolverVtable"),
        resolver_slot: u32_value(object, "resolverSlot"),
        resolver: string(object, "resolver"),
        condition_storage: string(object, "conditionStorage"),
        condition_offset: string(object, "conditionOffset"),
        owner: string(object, "owner"),
        subobject_offset: string(object, "subobjectOffset"),
        destructor_thunk: string(object, "destructorThunk"),
        complete_destructor: string(object, "completeDestructor"),
        initializer: string(object, "initializer"),
        name_field: string(object, "nameField"),
        name_offset: string(object, "nameOffset"),
        name_begin: string(object, "nameBegin"),
        name_end: string(object, "nameEnd"),
        name: string(object, "name"),
        default_value: object.get("defaultValue").and_then(Value::as_bool),
        profile_value: None,
        default_write: string(object, "defaultWrite"),
        default_callsite: string(object, "defaultCallsite"),
        default_target: string(object, "defaultTarget"),
        evidence_source: string_ref(object, "evidenceSource")?.to_owned(),
    })
}

fn exact_codec_sequence(codecs: &[NetworkContainerCodec]) -> Option<Vec<NetworkWireScalarShape>> {
    let mut shapes = Vec::new();
    for codec in codecs {
        shapes.extend(codec.exact_wire_shapes()?);
    }
    (!shapes.is_empty()).then_some(shapes)
}

fn exact_logical_codec_sequence(codecs: &[NetworkContainerCodec]) -> Option<Vec<String>> {
    let mut shapes = Vec::new();
    for codec in codecs {
        shapes.extend(codec.exact_logical_wire_shapes()?);
    }
    (!shapes.is_empty()).then_some(shapes)
}

fn decoded_logical_codec_sequence(codecs: &[NetworkContainerCodec]) -> Option<Vec<String>> {
    let mut shapes = Vec::new();
    for codec in codecs {
        shapes.extend(codec.decoded_logical_wire_shapes()?);
    }
    (!shapes.is_empty()).then_some(shapes)
}

fn registered_profile_logical_codec_sequence(
    codecs: &[NetworkContainerCodec],
) -> Option<Vec<String>> {
    let active = codecs
        .iter()
        .map(|codec| {
            registered_profile_codec_guards(codec)
                .map(|(included, guards)| included.then_some((codec, guards)))
        })
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if active.is_empty() {
        return Some(Vec::new());
    }

    let semantic_guards = active
        .iter()
        .flat_map(|(_, guards)| guards.iter().copied())
        .collect::<Vec<_>>();
    if semantic_guards.is_empty() {
        let mut shapes = Vec::new();
        for (codec, _) in active {
            shapes.extend(codec.registered_profile_logical_wire_shapes_unguarded()?);
        }
        return (!shapes.is_empty()).then_some(shapes);
    }
    semantic_guards
        .iter()
        .all(|guard| guard.kind == "storage-bit-mask")
        .then_some(())?;

    let storage_base = semantic_guards.first()?.storage_base.as_deref()?;
    let storage_offset = parse_u64_string(semantic_guards.first()?.storage_offset.as_deref()?)?;
    semantic_guards
        .iter()
        .all(|guard| {
            guard.storage_base.as_deref() == Some(storage_base)
                && guard.storage_offset.as_deref().and_then(parse_u64_string)
                    == Some(storage_offset)
        })
        .then_some(())?;

    let producers = active
        .iter()
        .enumerate()
        .filter(|(_, (codec, guards))| {
            guards.is_empty()
                && codec.element_offset == Some(storage_offset)
                && codec
                    .wire_shape
                    .as_deref()
                    .or(codec.wire_layout.as_deref())
                    .is_some_and(is_byte_mask_shape)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [producer_index] = producers.as_slice() else {
        return None;
    };
    active[..*producer_index]
        .iter()
        .all(|(_, guards)| guards.is_empty())
        .then_some(())?;

    let mut result = Vec::new();
    for (codec, _) in &active[..*producer_index] {
        result.extend(codec.registered_profile_logical_wire_shapes_unguarded()?);
    }
    let mask_shape = active[*producer_index]
        .0
        .wire_shape
        .as_deref()
        .or(active[*producer_index].0.wire_layout.as_deref())?;

    let mut groups: Vec<(Option<u8>, Vec<String>)> = Vec::new();
    for (codec, guards) in &active[*producer_index + 1..] {
        let mask = match guards.as_slice() {
            [] => None,
            [guard] => {
                let mask = parse_u64_string(guard.mask.as_deref()?)?;
                (mask <= u8::MAX.into() && mask.is_power_of_two()).then_some(())?;
                let condition_true_when_set = match guard.condition.as_str() {
                    "not-equal-zero" => true,
                    "equal-zero" => false,
                    _ => return None,
                };
                (guard.member_on_true == condition_true_when_set).then_some(())?;
                Some(u8::try_from(mask).ok()?)
            }
            _ => return None,
        };
        let member_shapes = codec.registered_profile_logical_wire_shapes_unguarded()?;
        if groups
            .last()
            .is_some_and(|(group_mask, _)| *group_mask == mask)
        {
            groups.last_mut()?.1.extend(member_shapes);
        } else {
            groups.push((mask, member_shapes));
        }
    }
    (!groups.is_empty() && groups.len() <= 11).then_some(())?;

    let members = groups
        .into_iter()
        .map(|(mask, shapes)| {
            let payload = logical_shape_product(&shapes)?;
            Some(match mask {
                Some(mask) => format!("masked<0x{mask:02x},{payload}>"),
                None => format!("required<{payload}>"),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    result.push(format!(
        "bit-mask-composite<{mask_shape},{}>",
        members.join(",")
    ));
    Some(result)
}

fn registered_profile_codec_guards(
    codec: &NetworkContainerCodec,
) -> Option<(bool, Vec<&NetworkContainerCodecGuard>)> {
    let mut semantic = Vec::new();
    for guard in &codec.guards {
        if decode_guard_is_transient(guard) {
            continue;
        }
        if guard.kind == "global-boolean" && guard.mask.is_none() {
            if !guard.includes_member_for_effective_profile()? {
                return Some((false, Vec::new()));
            }
            continue;
        }
        if guard.kind != "storage-bit-mask" {
            return None;
        }
        semantic.push(guard);
    }
    Some((true, semantic))
}

fn logical_shape_product(shapes: &[String]) -> Option<String> {
    match shapes {
        [] => None,
        [shape] => Some(shape.clone()),
        shapes => Some(format!("composite<{}>", shapes.join(","))),
    }
}

fn logical_products_machine_compatible(left: &[String], right: &[String]) -> bool {
    if left == right {
        return true;
    }
    let Some(left) = logical_shape_product(left).and_then(|shape| parse_network_wire_shape(&shape))
    else {
        return false;
    };
    let Some(right) =
        logical_shape_product(right).and_then(|shape| parse_network_wire_shape(&shape))
    else {
        return false;
    };
    super::merge::wire_shapes_machine_compatible(&left, &right)
}

fn is_byte_mask_shape(value: &str) -> bool {
    matches!(
        parse_network_wire_scalar_shape(value),
        Some(
            NetworkWireScalarShape::Bool
                | NetworkWireScalarShape::U8
                | NetworkWireScalarShape::FixedBytes(1)
        )
    )
}

fn parse_u64_string(value: &str) -> Option<u64> {
    let value = value.trim();
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or_else(
            || value.parse().ok(),
            |hex| u64::from_str_radix(hex, 16).ok(),
        )
}

fn decode_guard_is_transient(guard: &NetworkContainerCodecGuard) -> bool {
    guard.kind == "boolean-storage"
        && guard.storage_base.as_deref() == Some("stack")
        && guard.storage_address.is_none()
        && guard.mask.is_none()
        && guard.external_condition.is_none()
}

fn hex_or_decimal_u64(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(number) => number.as_u64(),
        Value::String(value) => {
            let value = value.trim();
            value
                .strip_prefix("0x")
                .or_else(|| value.strip_prefix("0X"))
                .map_or_else(
                    || value.parse().ok(),
                    |hex| u64::from_str_radix(hex, 16).ok(),
                )
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn preserves_proven_sequence_marker_evidence() {
        let value = json!({
            "storageKind": "index-map",
            "sequenceMarkerProven": true,
            "keyCodecs": [{ "wireShape": "u64" }],
            "valueCodecs": [{ "wireShape": "u8" }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert!(plan.sequence_marker_proven);
    }

    #[test]
    fn linear_helper_members_are_an_exact_wire_sequence() {
        let value = json!({
            "storageKind": "index-map",
            "unmarshalReconciliation": "complete-physical-sequence-agreement",
            "keyCodecs": [{ "wireShape": "u32" }],
            "valueCodecs": [{
                "memberSemantics": "linear-sequence",
                "members": [
                    { "wireShape": "u32" },
                    { "wireShape": "u32" },
                    { "wireShape": "u32" }
                ]
            }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert_eq!(
            plan.exact_key_wire_shapes(),
            Some(vec![NetworkWireScalarShape::U32])
        );
        assert_eq!(
            plan.exact_value_wire_shapes(),
            Some(vec![NetworkWireScalarShape::U32; 3])
        );
    }

    #[test]
    fn reachable_helper_members_are_not_flattened_into_wire_order() {
        let value = json!({
            "storageKind": "index-map",
            "unmarshalReconciliation": "complete-physical-sequence-agreement",
            "keyCodecs": [{ "wireShape": "string" }],
            "valueCodecs": [{
                "memberSemantics": "cfg-reachable",
                "members": [
                    { "wireShape": "u32" },
                    { "wireShape": "u64" }
                ]
            }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert_eq!(
            plan.exact_key_wire_shapes(),
            Some(vec![NetworkWireScalarShape::String])
        );
        assert_eq!(plan.exact_value_wire_shapes(), None);
    }

    #[test]
    fn guarded_member_retains_predicate_without_claiming_linear_wire_order() {
        let value = json!({
            "storageKind": "vector",
            "unmarshalReconciliation": "complete-conditional-physical-agreement",
            "valueCodecs": [{
                "wireShape": "u16",
                "guards": [{
                    "branch": "NewWorld+0x1234",
                    "kind": "storage-bit-mask",
                    "condition": "not-equal-zero",
                    "memberOnTrue": true,
                    "storageBase": "param_2",
                    "storageOffset": "0x4",
                    "mask": "0x1",
                    "evidenceSource": "dominating-cbranch-pcode-storage"
                }]
            }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");
        let codec = &plan.value_codecs[0];

        assert_eq!(codec.guards.len(), 1);
        assert_eq!(codec.guards[0].mask.as_deref(), Some("0x1"));
        assert_eq!(plan.exact_value_wire_shapes(), None);
        assert!(plan.has_non_linear_value_codec());
    }

    #[test]
    fn optional_suffix_preserves_both_physical_paths_without_claiming_one_layout() {
        let value = json!({
            "storageKind": "vector",
            "unmarshalReconciliation": "complete-conditional-physical-agreement",
            "valueCodecs": [{
                "memberSemantics": "optional-suffix",
                "analysisStatus": "optional-suffix",
                "members": [
                    { "wireShape": "u64" },
                    { "wireShape": "u8" }
                ],
                "optionalMembers": [
                    { "wireShape": "u32" },
                    { "wireShape": "u8" }
                ]
            }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");
        let codec = &plan.value_codecs[0];

        assert_eq!(
            codec.member_semantics,
            Some(NetworkContainerMemberSemantics::OptionalSuffix)
        );
        assert_eq!(codec.members.len(), 2);
        assert_eq!(codec.optional_members.len(), 2);
        assert_eq!(plan.exact_value_wire_shapes(), None);
        assert_eq!(plan.exact_logical_value_wire_shapes(), None);
        assert!(plan.has_non_linear_value_codec());
    }

    #[test]
    fn registered_default_resolves_an_external_optional_suffix() {
        let condition = json!({
            "resolverObject": "NewWorld+0x1000",
            "resolverVtable": "NewWorld+0x2000",
            "resolverSlot": 1,
            "resolver": "NewWorld+0x3000",
            "conditionStorage": "NewWorld+0x100c",
            "conditionOffset": "0xc",
            "owner": "NewWorld+0xfb0",
            "subobjectOffset": "0x50",
            "destructorThunk": "NewWorld+0x4000",
            "completeDestructor": "NewWorld+0x5000",
            "initializer": "NewWorld+0x6000",
            "nameField": "NewWorld+0x1010",
            "nameOffset": "0x10",
            "nameBegin": "NewWorld+0x7000",
            "nameEnd": "NewWorld+0x7017",
            "name": "feature.enabled",
            "defaultValue": true,
            "defaultWrite": "NewWorld+0x8000",
            "defaultCallsite": "NewWorld+0x8010",
            "defaultTarget": "NewWorld+0x9000",
            "evidenceSource": "static-vtable-dispatch+adjustor-thunk+initializer-writes+resolver-default-flow"
        });
        let value = json!({
            "storageKind": "vector",
            "unmarshalReconciliation": "complete-conditional-physical-agreement",
            "valueCodecs": [{
                "memberSemantics": "optional-suffix",
                "members": [{ "wireShape": "u64" }],
                "optionalMembers": [{
                    "wireShape": "u32",
                    "guards": [{
                        "branch": "NewWorld+0xa000",
                        "kind": "global-boolean",
                        "condition": "not-equal-zero",
                        "memberOnTrue": true,
                        "storageAddress": "NewWorld+0x100c",
                        "externalCondition": condition,
                        "evidenceSource": "dominating-cbranch-pcode-storage+external-condition-proof"
                    }]
                }]
            }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert_eq!(plan.exact_value_wire_shapes(), None);
        assert_eq!(
            plan.default_profile_value_wire_shapes(),
            Some(vec![
                NetworkWireScalarShape::U64,
                NetworkWireScalarShape::U32
            ])
        );
    }

    #[test]
    fn counted_sequence_preserves_logical_shape_and_physical_codecs() {
        let value = json!({
            "storageKind": "vector",
            "unmarshalReconciliation": "complete-physical-sequence-agreement",
            "valueCodecs": [{
                "wireShape": "fixed-vector<u32,5>",
                "wireLayout": "vec<u32>",
                "memberSemantics": "counted-sequence",
                "analysisStatus": "complete",
                "members": [
                    { "wireShape": "vlq-u32" },
                    { "wireShape": "u32" }
                ]
            }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert_eq!(
            plan.exact_value_wire_shapes(),
            Some(vec![
                NetworkWireScalarShape::VlqU32,
                NetworkWireScalarShape::U32,
            ])
        );
        assert_eq!(
            plan.exact_logical_value_wire_shapes(),
            Some(vec!["fixed-vector<u32,5>".to_owned()])
        );
        assert!(!plan.has_non_linear_value_codec());
    }

    #[test]
    fn fixed_sequence_is_one_logical_member_with_repeated_physical_codecs() {
        let value = json!({
            "storageKind": "vector",
            "unmarshalReconciliation": "complete-physical-sequence-agreement",
            "valueCodecs": [{
                "wireShape": "fixed-array<u16,2>",
                "memberSemantics": "fixed-sequence",
                "analysisStatus": "complete",
                "members": [
                    { "wireShape": "u16" },
                    { "wireShape": "u16" }
                ]
            }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert_eq!(
            plan.exact_value_wire_shapes(),
            Some(vec![NetworkWireScalarShape::U16; 2])
        );
        assert_eq!(
            plan.exact_logical_value_wire_shapes(),
            Some(vec!["fixed-array<u16,2>".to_owned()])
        );
    }

    #[test]
    fn direct_native_codec_preserves_identity_without_scalar_inference() {
        let value = json!({
            "storageKind": "index-map",
            "keyCodecs": [{ "wireLayout": "fixed-bytes-16" }],
            "valueCodecs": [{
                "nativeType": "Javelin::GroupInviteData",
                "typeId": "087fd940-a0ee-4cc1-8b82-08ca2bcaaaea",
                "typeIdentityProven": true
            }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert_eq!(
            plan.direct_value_type(),
            Some((
                "Javelin::GroupInviteData",
                Some(Uuid::parse_str("087fd940-a0ee-4cc1-8b82-08ca2bcaaaea").unwrap())
            ))
        );
        assert_eq!(plan.exact_key_wire_shapes(), None);
        assert_eq!(plan.exact_value_wire_shapes(), None);
    }

    #[test]
    fn independently_matches_map_key_before_non_linear_value_codec() {
        let value = json!({
            "storageKind": "index-map",
            "unmarshalStorageProof": "cfg-ordered-buffer-codecs+unique-owner-insertion",
            "unmarshalReconciliation": "marshal-codec-sequence-not-linear",
            "keyCodecs": [{ "wireLayout": "fixed-bytes-16" }],
            "valueCodecs": [{
                "memberSemantics": "cfg-reachable",
                "members": [{ "wireShape": "u64" }]
            }],
            "unmarshalCodecs": [
                { "wireLayout": "fixed-bytes-16" },
                {
                    "memberSemantics": "cfg-reachable",
                    "members": [{ "wireShape": "u64" }]
                }
            ]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert_eq!(plan.exact_key_wire_shapes(), None);
        assert_eq!(
            plan.independently_matching_key_wire_shapes(),
            Some(vec![NetworkWireScalarShape::FixedBytes(16)])
        );
    }

    #[test]
    fn independent_map_key_requires_storage_and_exact_codec_boundary_proofs() {
        let plan_without_storage_proof = json!({
            "storageKind": "index-map",
            "keyCodecs": [{ "wireLayout": "fixed-bytes-16" }],
            "valueCodecs": [{ "wireShape": "u8" }],
            "unmarshalCodecs": [{ "wireLayout": "fixed-bytes-16" }]
        });
        let plan_without_storage_proof =
            parse_plan(plan_without_storage_proof.as_object().expect("plan object"))
                .expect("valid plan");
        assert_eq!(
            plan_without_storage_proof.independently_matching_key_wire_shapes(),
            None
        );

        let plan_without_codec_boundary = json!({
            "storageKind": "index-map",
            "unmarshalStorageProof": "cfg-ordered-buffer-codecs+unique-owner-insertion",
            "keyCodecs": [{ "wireLayout": "fixed-bytes-16" }],
            "valueCodecs": [{ "wireShape": "u8" }],
            "unmarshalCodecs": [{
                "memberSemantics": "linear-sequence",
                "members": [
                    { "wireLayout": "fixed-bytes-16" },
                    { "wireShape": "u8" }
                ]
            }]
        });
        let plan_without_codec_boundary = parse_plan(
            plan_without_codec_boundary
                .as_object()
                .expect("plan object"),
        )
        .expect("valid plan");
        assert_eq!(
            plan_without_codec_boundary.independently_matching_key_wire_shapes(),
            None
        );
    }

    #[test]
    fn registered_profile_folds_mask_members_and_omits_disabled_global_suffix() {
        let disabled_condition = json!({
            "resolverObject": "NewWorld+0x1000",
            "resolverVtable": "NewWorld+0x2000",
            "resolverSlot": 1,
            "resolver": "NewWorld+0x3000",
            "conditionStorage": "NewWorld+0x100c",
            "conditionOffset": "0xc",
            "owner": "NewWorld+0xfb0",
            "subobjectOffset": "0x50",
            "destructorThunk": "NewWorld+0x4000",
            "completeDestructor": "NewWorld+0x5000",
            "initializer": "NewWorld+0x6000",
            "nameField": "NewWorld+0x1010",
            "nameOffset": "0x10",
            "nameBegin": "NewWorld+0x7000",
            "nameEnd": "NewWorld+0x7017",
            "name": "feature.enabled",
            "defaultValue": false,
            "defaultWrite": "NewWorld+0x8000",
            "defaultCallsite": "NewWorld+0x8010",
            "defaultTarget": "NewWorld+0x9000",
            "evidenceSource": "static-vtable-dispatch+adjustor-thunk+initializer-writes+resolver-default-flow"
        });
        let value = json!({
            "storageKind": "vector",
            "unmarshalReconciliation": "complete-conditional-path-agreement",
            "valueCodecs": [{
                "memberSemantics": "cfg-reachable",
                "members": [
                    { "wireShape": "vlq-u32", "elementOffset": "0x0" },
                    { "wireLayout": "fixed-bytes-1", "elementOffset": "0x4" },
                    {
                        "wireShape": "vlq-u32",
                        "guards": [{
                            "branch": "NewWorld+0xa000",
                            "kind": "storage-bit-mask",
                            "condition": "equal-zero",
                            "memberOnTrue": false,
                            "storageBase": "param_2",
                            "storageOffset": "0x4",
                            "mask": "0x1",
                            "evidenceSource": "dominating-cbranch-pcode-storage"
                        }]
                    },
                    {
                        "wireShape": "u32",
                        "guards": [{
                            "branch": "NewWorld+0xb000",
                            "kind": "global-boolean",
                            "condition": "not-equal-zero",
                            "memberOnTrue": true,
                            "storageAddress": "NewWorld+0x100c",
                            "externalCondition": disabled_condition,
                            "evidenceSource": "dominating-cbranch-pcode-storage+external-condition-proof"
                        }]
                    }
                ]
            }]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert_eq!(
            plan.registered_profile_logical_value_wire_shapes(),
            Some(vec![
                "vlq-u32".to_owned(),
                "bit-mask-composite<fixed-bytes-1,masked<0x01,vlq-u32>>".to_owned(),
            ])
        );
    }

    #[test]
    fn independently_matching_logical_products_override_a_conservative_status() {
        let value = json!({
            "storageKind": "index-map",
            "unmarshalReconciliation": "marshal-unmarshal-codec-count-mismatch",
            "keyCodecs": [{ "wireLayout": "fixed-bytes-16" }],
            "valueCodecs": [{
                "memberSemantics": "linear-sequence",
                "members": [
                    { "wireShape": "u64" },
                    { "wireShape": "vec<composite<u32,u8>>", "memberSemantics": "counted-sequence" }
                ]
            }],
            "unmarshalCodecs": [
                { "wireLayout": "fixed-bytes-16" },
                {
                    "memberSemantics": "linear-sequence",
                    "members": [
                        {
                            "wireShape": "u64",
                            "guards": [{
                                "kind": "boolean-storage",
                                "condition": "not-equal-zero",
                                "memberOnTrue": true,
                                "storageBase": "stack",
                                "storageOffset": "0xffffffffffffffb8",
                                "evidenceSource": "dominating-cbranch-pcode-storage"
                            }]
                        },
                        { "wireShape": "vec<composite<u32,u8>>", "memberSemantics": "counted-sequence" }
                    ]
                }
            ]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert!(plan.has_complete_cross_direction_agreement());
        assert_eq!(
            plan.registered_profile_logical_value_wire_shapes(),
            Some(vec!["u64".to_owned(), "vec<composite<u32,u8>>".to_owned()])
        );
    }

    #[test]
    fn independent_reconciliation_rejects_empty_or_different_decode_products() {
        let mut value = json!({
            "storageKind": "vector",
            "unmarshalReconciliation": "unmarshal-loop-unresolved",
            "valueCodecs": [{ "wireShape": "u32" }],
            "unmarshalCodecs": []
        });
        let unresolved =
            parse_plan(value.as_object().expect("plan object")).expect("valid unresolved plan");
        assert!(!unresolved.has_complete_cross_direction_agreement());

        value["unmarshalCodecs"] = json!([{ "wireShape": "u64" }]);
        let mismatched =
            parse_plan(value.as_object().expect("plan object")).expect("valid mismatched plan");
        assert!(!mismatched.has_complete_cross_direction_agreement());
    }

    #[test]
    fn independent_reconciliation_accepts_equivalent_nested_and_flat_products() {
        let value = json!({
            "storageKind": "vector",
            "unmarshalReconciliation": "marshal-unmarshal-codec-count-mismatch",
            "valueCodecs": [{
                "wireShape": "vec<composite<u32,u8>>",
                "memberSemantics": "counted-sequence"
            }],
            "unmarshalCodecs": [
                { "wireShape": "vlq-u32" },
                { "wireShape": "u32" },
                { "wireLayout": "fixed-bytes-1" }
            ]
        });

        let plan = parse_plan(value.as_object().expect("plan object")).expect("valid plan");

        assert!(plan.has_complete_cross_direction_agreement());
    }
}
