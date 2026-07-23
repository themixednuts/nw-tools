use super::*;

pub(super) fn message_field_shape_report(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
    message_type_name: Option<&str>,
) -> NetworkStateFieldShapeReport {
    let handler_vtables = BTreeMap::new();
    let mut report = state_field_shape_report(
        field,
        wire_shapes,
        wire_shape_sources,
        &handler_vtables,
        value_type_candidates,
        serialize_types,
    );
    if report.wire_shape.is_none()
        && let Some(shape) = anonymous_message_wire_layout_shape(field)
    {
        report.wire_shape_source = field
            .wire_layout_source
            .clone()
            .or_else(|| Some("message-wire-layout".to_owned()));
        report.wire_shape = Some(shape);
    }
    let source_type = serialize_field_scalar_source_type(field, report.wire_shape.as_ref());
    let rust_type = field
        .rust_type
        .as_deref()
        .map(normalize_generated_rust_type)
        .or_else(|| {
            report.wire_shape.as_ref()?;
            field
                .native_type
                .as_deref()
                .and_then(|native_type| {
                    network_native_type_rust_type(native_type, &BTreeMap::new())
                })
                .map(|rust_type| normalize_generated_rust_type(&rust_type))
        })
        .or_else(|| message_nested_shape_rust_type(field, message_type_name))
        .or_else(|| message_serialize_source_rust_type(field))
        .or(source_type)
        .or_else(|| {
            report
                .wire_shape
                .as_ref()
                .map(rust_field_shape)
                .map(|shape| shape.value_type)
        })
        .or_else(|| report.rust_value_type.clone());
    report.rust_value_type = rust_type.clone();
    report.rust_field_type = rust_type.clone();
    report.blocked_reason =
        message_field_blocked_reason(field, report.wire_shape.as_ref(), rust_type.as_deref());
    report.supported = report.blocked_reason.is_none();
    report
}

fn anonymous_message_wire_layout_shape(field: &NetworkField) -> Option<SchemaWireShape> {
    if field.rust_type.is_some()
        || field.native_type.is_some()
        || field.source_type_name.is_some()
        || field.source_type_id.is_some()
        || field.serialize.is_some()
        || field.nested_type_shape.is_some()
    {
        return None;
    }

    let shape = parse_network_wire_scalar_shape(field.wire_layout.as_deref()?)?;
    let SchemaWireScalarShape::FixedBytes(width) = shape else {
        return None;
    };
    if field
        .raw_byte_length
        .is_some_and(|raw_byte_length| raw_byte_length != u32::from(width))
    {
        return None;
    }
    Some(SchemaWireShape::FixedBytes(width))
}

pub(super) fn state_field_shape_report(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    handler_vtables_by_address: &BTreeMap<&str, &NetworkFieldHandlerVtable>,
    value_type_candidates_by_vtable: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> NetworkStateFieldShapeReport {
    let value_type_candidates = field_value_type_candidates(field, value_type_candidates_by_vtable);
    let normalized_rust_type = field
        .rust_type
        .as_deref()
        .map(normalize_generated_rust_type);
    let rust_type = normalized_rust_type
        .as_deref()
        .filter(|rust_type| syn::parse_str::<syn::Type>(rust_type).is_ok());
    let explicit_field_type =
        rust_type.filter(|rust_type| is_replicated_state_field_type(rust_type));
    let shape = if explicit_field_type.is_some() {
        None
    } else {
        field_wire_shape(field, wire_shapes)
    };
    let source_type = serialize_field_scalar_source_type(field, shape);
    let rust_shape = shape
        .filter(|shape| {
            !shape.is_replicated_container() && !matches!(shape, SchemaWireShape::FixedSequence(_))
        })
        .map(rust_field_shape);
    let handler_vtable = field
        .handler_vtable
        .as_deref()
        .and_then(|address| handler_vtables_by_address.get(address).copied());
    let container_vtable = explicit_field_type
        .is_none()
        .then(|| replicated_container_vtable_for_field(field, handler_vtables_by_address))
        .flatten();
    let is_container_field = explicit_field_type.is_none()
        && (shape.is_some_and(|shape| shape.is_replicated_container())
            || container_vtable.is_some());
    let container_resolution = if is_container_field {
        Some(
            container_vtable.map_or(Err(ContainerPlanError::MissingPlan), |vtable| {
                replicated_container_semantic_field_shape(field, vtable, shape, serialize_types)
            }),
        )
    } else {
        None
    };
    let container_rust_shape = container_resolution
        .as_ref()
        .and_then(|resolution| resolution.as_ref().ok());
    let fixed_sequence_vtable = explicit_field_type
        .is_none()
        .then(|| fixed_sequence_vtable_for_field(field, handler_vtables_by_address))
        .flatten();
    let is_fixed_sequence_field = explicit_field_type.is_none()
        && (matches!(shape, Some(SchemaWireShape::FixedSequence(_)))
            || fixed_sequence_vtable.is_some());
    let fixed_sequence_resolution = if is_fixed_sequence_field {
        Some(fixed_sequence_vtable.map_or(
            Err(fixed_sequence::FixedSequencePlanError::MissingPlan),
            |vtable| fixed_sequence_field_report(vtable, shape, serialize_types),
        ))
    } else {
        None
    };
    let fixed_sequence = fixed_sequence_resolution
        .as_ref()
        .and_then(|resolution| resolution.as_ref().ok());
    let structured_value_resolution = if explicit_field_type.is_none()
        && !is_container_field
        && !is_fixed_sequence_field
        && !matches!(shape, Some(SchemaWireShape::RemoteServerGdeRef))
    {
        handler_vtable.and_then(|vtable| {
            structured_value_field_plan(field, Some(vtable), shape, serialize_types)
        })
    } else {
        None
    };
    let structured_value = structured_value_resolution
        .as_ref()
        .and_then(|resolution| resolution.as_ref().ok());
    let generated_rust_field_type = explicit_field_type
        .map(ToOwned::to_owned)
        .or_else(|| fixed_sequence.map(NetworkFixedSequenceFieldReport::field_type))
        .or_else(|| structured_value.map(|value| value.field_type.clone()))
        .or_else(|| {
            rust_type
                .filter(|_| {
                    shape.is_some_and(|shape| {
                        !shape.is_replicated_container()
                            && !matches!(shape, SchemaWireShape::FixedSequence(_))
                    })
                })
                .map(|rust_type| {
                    replicated_field_handler_type(
                        shape.expect("state value override has a wire shape"),
                        rust_type,
                    )
                })
        })
        .or_else(|| {
            source_type.as_deref().and_then(|source_type| {
                shape
                    .filter(|shape| {
                        !shape.is_replicated_container()
                            && !matches!(shape, SchemaWireShape::FixedSequence(_))
                    })
                    .map(|shape| replicated_field_handler_type(shape, source_type))
            })
        })
        .or_else(|| {
            container_rust_shape
                .as_ref()
                .map(|shape| shape.field_type.clone())
        })
        .or_else(|| {
            shape
                .filter(|shape| {
                    !shape.is_replicated_container()
                        && !matches!(shape, SchemaWireShape::FixedSequence(_))
                })
                .and_then(|_| rust_shape.as_ref().map(|shape| shape.field_type.clone()))
        });
    let blocked_reason = fixed_sequence_resolution
        .as_ref()
        .and_then(|resolution| resolution.as_ref().err())
        .map(|reason| reason.as_str().to_owned())
        .or_else(|| {
            container_resolution
                .as_ref()
                .and_then(|resolution| resolution.as_ref().err())
                .map(|reason| reason.as_str().to_owned())
        })
        .or_else(|| {
            structured_value_resolution
                .as_ref()
                .and_then(|resolution| resolution.as_ref().err())
                .map(|reason| reason.as_str().to_owned())
        })
        .or_else(|| {
            state_field_blocked_reason(
                field,
                shape,
                normalized_rust_type.as_deref(),
                explicit_field_type,
                generated_rust_field_type.is_some(),
                !value_type_candidates.is_empty()
                    || container_vtable.is_some()
                    || fixed_sequence_vtable.is_some(),
            )
        });
    NetworkStateFieldShapeReport {
        field_index: field.index,
        field_name: field.name.clone(),
        group: field.group,
        registration_kind: field.registration_kind.clone(),
        filter_group_attribute: field.filter_group_attribute,
        native_type: field.native_type.clone(),
        source_type_name: field.source_type_name.clone(),
        source_type_id: field
            .source_type_id
            .or_else(|| field.serialize.as_ref().map(|serialize| serialize.type_id)),
        serialize_type_name: field
            .serialize
            .as_ref()
            .map(|serialize| serialize.name.clone()),
        handler_vtable: field.handler_vtable.clone(),
        wire_shape_source: if explicit_field_type.is_some() && shape.is_none() {
            None
        } else {
            field_wire_shape_source(field, wire_shapes, wire_shape_sources)
        },
        wire_shape: shape.cloned(),
        wire_layout: field.wire_layout.clone(),
        wire_layout_source: field.wire_layout_source.clone(),
        value_type_candidates,
        container_key_type_shape: if explicit_field_type.is_some() {
            None
        } else {
            container_rust_shape
                .as_ref()
                .and_then(|shape| shape.container_key_type_shape.clone())
        },
        container_embedded_key_type_shapes: if explicit_field_type.is_some() {
            Vec::new()
        } else {
            container_rust_shape
                .as_ref()
                .map(|shape| shape.container_embedded_key_type_shapes.clone())
                .unwrap_or_default()
        },
        container_value_type_shape: if explicit_field_type.is_some() {
            None
        } else {
            container_rust_shape
                .as_ref()
                .and_then(|shape| shape.container_value_type_shape.clone())
        },
        container_embedded_value_type_shapes: if explicit_field_type.is_some() {
            Vec::new()
        } else {
            container_rust_shape
                .as_ref()
                .map(|shape| shape.container_embedded_value_type_shapes.clone())
                .unwrap_or_default()
        },
        nested_type_shape: structured_value
            .map(|value| value.shape.clone())
            .or_else(|| field.nested_type_shape.clone()),
        nested_embedded_type_shapes: structured_value
            .map(|value| value.embedded_shapes.clone())
            .unwrap_or_default(),
        fixed_sequence: fixed_sequence.cloned(),
        rust_value_type: if explicit_field_type.is_some() {
            None
        } else {
            fixed_sequence
                .map(NetworkFixedSequenceFieldReport::value_type)
                .or_else(|| structured_value.map(|value| value.value_type.clone()))
                .or_else(|| {
                    rust_type
                        .map(ToOwned::to_owned)
                        .or_else(|| source_type.clone())
                        .or_else(|| {
                            container_rust_shape
                                .as_ref()
                                .map(|shape| shape.value_type.clone())
                        })
                        .or_else(|| rust_shape.as_ref().map(|shape| shape.value_type.clone()))
                })
        },
        rust_field_type: generated_rust_field_type,
        constructor_write_count: field.constructor_writes.len(),
        confidence: field.confidence,
        supported: blocked_reason.is_none(),
        blocked_reason,
    }
}

pub(super) fn state_field_has_complete_shape(field: &NetworkStateFieldShapeReport) -> bool {
    field.wire_shape.is_some()
        || field
            .rust_field_type
            .as_deref()
            .is_some_and(is_replicated_state_field_type)
}

pub(super) fn field_wire_shape<'a>(
    field: &'a NetworkField,
    wire_shapes: &'a BTreeMap<&str, &SchemaWireShape>,
) -> Option<&'a SchemaWireShape> {
    field.wire_shape.as_ref().or_else(|| {
        field
            .handler_vtable
            .as_deref()
            .and_then(|handler_vtable| wire_shapes.get(handler_vtable).copied())
    })
}

pub(super) fn field_wire_shape_source(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
) -> Option<String> {
    field.wire_shape_source.clone().or_else(|| {
        field
            .handler_vtable
            .as_deref()
            .filter(|handler_vtable| wire_shapes.contains_key(*handler_vtable))
            .and_then(|handler_vtable| wire_shape_sources.get(handler_vtable).copied())
            .map(ToOwned::to_owned)
    })
}

pub(super) fn message_serialize_source_rust_type(field: &NetworkField) -> Option<String> {
    let serialize = field.serialize.as_ref()?;
    exact_type_id_rust_type(serialize.type_id)
        .map(ToOwned::to_owned)
        .or_else(|| serialize_source_rust_type_name(&serialize.name))
}

pub(super) fn message_nested_shape_rust_type(
    field: &NetworkField,
    message_type_name: Option<&str>,
) -> Option<String> {
    let shape = field.nested_type_shape.as_ref()?;
    if !message_nested_shape_matches_field(field, shape) {
        return None;
    }
    if let Some(shared_type) = shared_network_nested_shape_rust_type(shape) {
        return Some(shared_type.to_owned());
    }
    if message_nested_shape_uses_source_type(shape) {
        return shape
            .type_name_full
            .as_deref()
            .or(shape.type_name.as_deref())
            .and_then(serialize_source_rust_type_name);
    }
    if !shape.has_proven_anonymous_layout() {
        return None;
    }
    message_nested_shape_support_type_name(field, shape, message_type_name)
}

fn shared_network_nested_shape_rust_type(
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> Option<&'static str> {
    if !shape.has_proven_layout() {
        return None;
    }
    let wire_product = crate::network_schema::parse::nested_type_shape_wire_shapes(shape, &[])?;
    match shape.type_name_full.as_deref()? {
        "Javelin::ClientMessages::ActorRequestId"
            if wire_product == [SchemaWireScalarShape::U64, SchemaWireScalarShape::U64] =>
        {
            Some("::nw_network::ActorRequestId")
        }
        _ => None,
    }
}

pub(super) fn message_nested_shape_matches_field(
    field: &NetworkField,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> bool {
    if shape.has_exact_identity() {
        return true;
    }
    if shape.has_proven_anonymous_layout()
        && shape
            .type_name
            .as_deref()
            .or(shape.type_name_full.as_deref())
            .is_none()
    {
        return true;
    }
    let Some(shape_name) = shape
        .type_name
        .as_deref()
        .or_else(|| shape.type_name_full.as_deref().map(type_name_leaf))
    else {
        return false;
    };
    field
        .source_type_name
        .as_deref()
        .or(field.native_type.as_deref())
        .is_some_and(|source_names| source_type_contains_leaf(source_names, shape_name))
}

fn source_type_contains_leaf(value: &str, expected: &str) -> bool {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(type_name_leaf)
        .any(|part| part == expected)
}

pub(super) fn message_nested_shape_uses_source_type(
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> bool {
    shape.has_exact_identity()
}

pub(super) fn message_nested_shape_support_type_name(
    field: &NetworkField,
    shape: &crate::network_schema::NetworkNestedTypeShape,
    message_type_name: Option<&str>,
) -> Option<String> {
    let mut name = shape
        .type_name
        .as_deref()
        .or(shape.type_name_full.as_deref())
        .map(rust_type_ident)
        .or_else(|| {
            let message = rust_type_ident(message_type_name?);
            let field = rust_type_ident(&rust_field_ident(field.name.as_deref()?));
            Some(format!("{message}{field}Value"))
        })?;
    if message_type_name
        .map(rust_type_ident)
        .is_some_and(|message_name| message_name == name)
    {
        name.push_str("Body");
    }
    syn::parse_str::<syn::Type>(&name).ok()?;
    Some(name)
}

pub(super) fn resolved_field_descriptor_rust_type(field: &NetworkField) -> Option<String> {
    field
        .rust_type
        .as_deref()
        .map(normalize_generated_rust_type)
        .or_else(|| message_serialize_source_rust_type(field))
}

pub(super) fn normalize_generated_rust_type(rust_type: &str) -> String {
    rust_type
        .replace("::std::string::String", "String")
        .replace("::std::vec::Vec<", "Vec<")
        .replace(
            "::std::collections::HashMap<",
            "::nw_network::serialize::IndexMap<",
        )
        .replace(
            "std::collections::HashMap<",
            "::nw_network::serialize::IndexMap<",
        )
}

pub(super) fn wire_shape_sources_by_handler_vtable(schema: &NetworkSchema) -> BTreeMap<&str, &str> {
    schema
        .field_handler_vtables
        .iter()
        .filter_map(|vtable| {
            Some((
                vtable.address.as_deref()?,
                vtable.wire_shape_source.as_deref()?,
            ))
        })
        .collect()
}

pub(super) fn value_type_candidates_by_handler_vtable(
    schema: &NetworkSchema,
) -> BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>> {
    schema
        .field_handler_vtables
        .iter()
        .filter_map(|vtable| {
            let address = vtable.address.as_deref()?;
            let mut candidates = Vec::new();
            if let Some(value_type) = &vtable.value_type_info {
                candidates.push(value_type.clone());
            }
            for candidate in &vtable.value_type_candidates {
                push_unique_value_type_candidate(&mut candidates, candidate.clone());
            }
            (!candidates.is_empty()).then_some((address, candidates))
        })
        .collect()
}

pub(super) fn field_value_type_candidates(
    field: &NetworkField,
    candidates_by_vtable: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
) -> Vec<NetworkNativeTypeInfoEvidence> {
    field
        .handler_vtable
        .as_deref()
        .and_then(|handler_vtable| candidates_by_vtable.get(handler_vtable))
        .cloned()
        .unwrap_or_default()
}

pub(super) fn push_unique_value_type_candidate(
    candidates: &mut Vec<NetworkNativeTypeInfoEvidence>,
    candidate: NetworkNativeTypeInfoEvidence,
) {
    let duplicate = candidates.iter().any(|existing| {
        existing
            .type_id
            .as_ref()
            .zip(candidate.type_id.as_ref())
            .is_some_and(|(lhs, rhs)| lhs == rhs)
            || (existing.type_id.is_none()
                && candidate.type_id.is_none()
                && existing.address == candidate.address
                && existing.name == candidate.name)
    });
    if !duplicate {
        candidates.push(candidate);
    }
}

pub(super) fn state_blocked_reasons(
    network_type: &NetworkType,
    fields: &[NetworkStateFieldShapeReport],
) -> Vec<String> {
    let mut reasons = Vec::new();
    if network_type.type_index.is_none() {
        reasons.push("missing-type-index".to_owned());
    }
    if network_type.name.is_none() {
        reasons.push("missing-type-name".to_owned());
    }
    if fields.is_empty() {
        reasons.push("no-registered-fields".to_owned());
    }
    reasons.extend(counted_field_blocked_reasons(fields));
    reasons
}

pub(super) fn message_blocked_reasons(
    network_type: &NetworkType,
    fields: &[NetworkStateFieldShapeReport],
) -> Vec<String> {
    let mut reasons = Vec::new();
    if network_type.type_id.is_none() {
        reasons.push("missing-type-id".to_owned());
    }
    if network_type.type_index.is_none() {
        reasons.push("missing-type-index".to_owned());
    }
    if network_type.name.is_none() {
        reasons.push("missing-type-name".to_owned());
    }
    if network_type.fields.is_empty()
        && network_type.marshal_fields.is_empty()
        && !network_type.instance.as_ref().is_some_and(|instance| {
            instance.empty_wire_proven
                && instance.analysis_status
                    == Some(crate::network_schema::NetworkMessageAnalysisStatus::ProvenEmpty)
        })
    {
        reasons.push("message-layout-unresolved".to_owned());
    }
    let supports_unmarshal = network_type
        .instance
        .as_ref()
        .and_then(|instance| instance.supports_unmarshal);
    if supports_unmarshal != Some(false) && !message_directional_fields_agree(network_type) {
        reasons.push("marshal-unmarshal-field-mismatch".to_owned());
    }
    reasons.extend(counted_field_blocked_reasons(fields));
    reasons
}

fn message_directional_fields_agree(network_type: &NetworkType) -> bool {
    network_type.marshal_fields.is_empty()
        || (network_type.fields.len() == network_type.marshal_fields.len()
            && network_type
                .fields
                .iter()
                .zip(&network_type.marshal_fields)
                .all(|(unmarshal, marshal)| {
                    directional_field_offset(unmarshal) == directional_field_offset(marshal)
                        && directional_wire_evidence_agrees(unmarshal, marshal)
                }))
}

fn directional_field_offset(field: &NetworkField) -> Option<u32> {
    field.storage_offset.or(field.storage_base_offset)
}

fn directional_wire_evidence_agrees(left: &NetworkField, right: &NetworkField) -> bool {
    let shape_agrees = left
        .wire_shape
        .as_ref()
        .zip(right.wire_shape.as_ref())
        .is_none_or(|(left, right)| left == right);
    let layout_agrees = left
        .wire_layout
        .as_ref()
        .zip(right.wire_layout.as_ref())
        .is_none_or(|(left, right)| left == right);
    shape_agrees
        && layout_agrees
        && (left.wire_shape.is_some() && right.wire_shape.is_some()
            || left.wire_layout.is_some() && right.wire_layout.is_some())
}

pub(super) fn counted_field_blocked_reasons(
    fields: &[NetworkStateFieldShapeReport],
) -> Vec<String> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for reason in fields
        .iter()
        .filter_map(|field| field.blocked_reason.as_deref())
    {
        *counts.entry(reason).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(reason, count)| format!("{reason}:{count}"))
        .collect()
}

pub(super) fn state_field_blocked_reason(
    field: &NetworkField,
    shape: Option<&SchemaWireShape>,
    rust_type: Option<&str>,
    explicit_field_type: Option<&str>,
    has_generated_field_type: bool,
    has_value_type_evidence: bool,
) -> Option<String> {
    if field.index.is_none() {
        return Some("missing-field-index".to_owned());
    }
    if field.name.is_none() {
        return Some("missing-field-name".to_owned());
    }
    if !field.confidence.is_high_or_exact() {
        return Some("low-confidence-field".to_owned());
    }
    if let Some(rust_type) = rust_type
        && syn::parse_str::<syn::Type>(rust_type).is_err()
    {
        return Some("invalid-rust-field-type".to_owned());
    }
    if shape.is_none() && explicit_field_type.is_none() && !has_generated_field_type {
        if field.wire_layout.is_some() {
            return Some("known-layout-missing-semantic-type".to_owned());
        }
        if has_value_type_evidence {
            return Some("missing-semantic-type".to_owned());
        }
        return Some("missing-wire-shape".to_owned());
    }
    None
}

pub(super) fn message_field_blocked_reason(
    field: &NetworkField,
    shape: Option<&SchemaWireShape>,
    rust_type: Option<&str>,
) -> Option<String> {
    if field.index.is_none() {
        return Some("missing-field-index".to_owned());
    }
    if field.name.is_none() {
        return Some("missing-field-name".to_owned());
    }
    if !field.confidence.is_high_or_exact() {
        return Some("low-confidence-field".to_owned());
    }
    if let Some(rust_type) = rust_type
        && syn::parse_str::<syn::Type>(rust_type).is_ok()
    {
        return None;
    }
    if rust_type.is_some() {
        return Some("invalid-rust-field-type".to_owned());
    }
    if shape.is_none() {
        if field.wire_layout.is_some() {
            return Some("known-layout-missing-semantic-type".to_owned());
        }
        if has_composite_support_type_evidence(field) {
            return Some("missing-composite-support-type".to_owned());
        }
        if has_support_type_evidence(field) {
            return Some("missing-support-type".to_owned());
        }
        return Some("missing-field-type".to_owned());
    }
    if shape.is_some_and(SchemaWireShape::is_replicated_container) {
        return Some("missing-semantic-type".to_owned());
    }
    if matches!(shape, Some(SchemaWireShape::FixedSequence(_))) {
        return Some("missing-semantic-type".to_owned());
    }
    None
}

pub(super) fn has_composite_support_type_evidence(field: &NetworkField) -> bool {
    field.native_type.as_deref() == Some("composite")
        || field
            .source_type_name
            .as_deref()
            .is_some_and(|source_type| source_type.contains(','))
}

pub(super) fn has_support_type_evidence(field: &NetworkField) -> bool {
    field.serialize.is_some()
        || field
            .source_type_name
            .as_deref()
            .is_some_and(is_named_support_type_evidence)
        || field
            .native_type
            .as_deref()
            .is_some_and(is_named_support_type_evidence)
}

pub(super) fn is_named_support_type_evidence(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value != "unknown" && value != "composite"
}

pub(super) fn is_placeholder_field_name(value: &str) -> bool {
    value
        .strip_prefix("field_")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

pub(super) fn is_placeholder_report_field_name(field: &NetworkStateFieldShapeReport) -> bool {
    field
        .field_name
        .as_deref()
        .is_some_and(|name| is_placeholder_field_name(name) || is_native_type_field_name(name))
}

pub(super) fn is_native_type_field_name(name: &str) -> bool {
    matches!(
        name.trim(),
        "bool"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "f32"
            | "f64"
            | "float"
            | "double"
            | "String"
            | "Vector2"
            | "Vector3"
            | "Vector4"
            | "Quaternion"
            | "Matrix3x3"
            | "Aabb"
            | "EntityRef"
            | "ActorRef"
            | "HubAddress"
            | "ProxyAddress"
            | "FragmentKey"
            | "BaselineableFragment"
            | "Amazon::Hub::ActorRef"
            | "Amazon::Hub::FragmentKey"
            | "Amazon::Hub::BaselineableFragment"
            | "composite"
    )
}
