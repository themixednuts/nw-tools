use super::*;

pub(super) fn state_generation_plans(
    schema: &NetworkSchema,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    handler_vtables: &BTreeMap<&str, &NetworkFieldHandlerVtable>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
    context: &CodegenContext,
) -> Result<Vec<NetworkStateGenerationPlanReport>, NetworkRustEmitError> {
    let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
    let state_types = schema
        .types
        .iter()
        .filter(|network_type| {
            network_type
                .capabilities
                .contains(&NetworkTypeCapability::ReplicatedState)
        })
        .collect::<Vec<_>>();
    let plans =
        context
            .runner()
            .map_until_cancelled(&state_types, context.cancel(), |network_type| {
                state_generation_plan(
                    network_type,
                    wire_shapes,
                    &wire_shape_sources,
                    handler_vtables,
                    value_type_candidates,
                    serialize_types,
                )
            });
    if plans.was_cancelled() {
        return Err(NetworkRustEmitError::Cancelled);
    }
    Ok(plans.into_completed())
}

pub(super) fn message_generation_plans(
    schema: &NetworkSchema,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    context: &CodegenContext,
) -> Result<Vec<NetworkMessageGenerationPlanReport>, NetworkRustEmitError> {
    let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
    let serialize_types = serialize_types_by_type_id(schema);
    let message_types = schema
        .types
        .iter()
        .filter(|network_type| {
            network_type
                .capabilities
                .contains(&NetworkTypeCapability::DirectMessage)
        })
        .collect::<Vec<_>>();
    let plans =
        context
            .runner()
            .map_until_cancelled(&message_types, context.cancel(), |network_type| {
                message_generation_plan(
                    network_type,
                    wire_shapes,
                    &wire_shape_sources,
                    value_type_candidates,
                    &serialize_types,
                )
            });
    if plans.was_cancelled() {
        return Err(NetworkRustEmitError::Cancelled);
    }
    Ok(plans.into_completed())
}

pub(super) fn state_generation_plan(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    handler_vtables: &BTreeMap<&str, &NetworkFieldHandlerVtable>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> NetworkStateGenerationPlanReport {
    let attribute_count = network_type
        .fields
        .iter()
        .filter(|field| is_replicated_state_attribute_field(field))
        .count();
    let mut fields = network_type
        .fields
        .iter()
        .filter(|field| !is_replicated_state_attribute_field(field))
        .map(|field| {
            state_field_shape_report(
                field,
                wire_shapes,
                wire_shape_sources,
                handler_vtables,
                value_type_candidates,
                serialize_types,
            )
        })
        .collect::<Vec<_>>();
    disambiguate_report_field_names(&mut fields);
    let field_count = fields.len();
    let shaped_field_count = fields
        .iter()
        .filter(|field| state_field_has_complete_shape(field))
        .count();
    let supported_field_count = fields.iter().filter(|field| field.supported).count();
    let missing_wire_shape_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-wire-shape"))
        .count();
    let unsupported_wire_shape_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("unsupported-wire-shape"))
        .count();
    let low_confidence_field_count = fields
        .iter()
        .filter(|field| !field.confidence.is_high_or_exact())
        .count();
    let evidence_issues = state_evidence_issues(network_type);
    let mut blocked_reasons = state_blocked_reasons(network_type, &fields);
    if !evidence_issues.is_empty() {
        blocked_reasons.push(format!("invalid-evidence:{}", evidence_issues.len()));
    }
    NetworkStateGenerationPlanReport {
        type_index: network_type.type_index,
        type_name: network_type.name.clone(),
        fragment_category: network_type
            .fragment_metadata
            .as_ref()
            .and_then(|metadata| metadata.category.clone()),
        fragment_category_value: network_type
            .fragment_metadata
            .as_ref()
            .and_then(|metadata| metadata.category_value),
        is_metadata_fragment: network_type
            .fragment_metadata
            .as_ref()
            .and_then(|metadata| metadata.is_metadata),
        field_count,
        attribute_count,
        shaped_field_count,
        supported_field_count,
        missing_wire_shape_count,
        unsupported_wire_shape_count,
        low_confidence_field_count,
        evidence_issues,
        can_generate: blocked_reasons.is_empty(),
        blocked_reasons,
        fields,
    }
}

pub(super) fn is_replicated_state_attribute_field(field: &NetworkField) -> bool {
    field.registration_kind.as_deref() == Some("attribute")
}

pub(super) fn message_generation_plan(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> NetworkMessageGenerationPlanReport {
    let supports_unmarshal = network_type
        .instance
        .as_ref()
        .and_then(|instance| instance.supports_unmarshal);
    let native_fields =
        if supports_unmarshal == Some(false) && !network_type.marshal_fields.is_empty() {
            &network_type.marshal_fields
        } else {
            &network_type.fields
        };
    let mut fields = native_fields
        .iter()
        .map(|field| {
            message_field_shape_report(
                field,
                wire_shapes,
                wire_shape_sources,
                value_type_candidates,
                serialize_types,
                network_type.name.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    disambiguate_report_field_names(&mut fields);
    let field_count = fields.len();
    let shaped_field_count = fields
        .iter()
        .filter(|field| field.wire_shape.is_some())
        .count();
    let supported_field_count = fields.iter().filter(|field| field.supported).count();
    let missing_wire_shape_count = fields
        .iter()
        .filter(|field| field.wire_shape.is_none())
        .count();
    let missing_field_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-field-type"))
        .count();
    let missing_support_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-support-type"))
        .count();
    let missing_composite_support_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-composite-support-type"))
        .count();
    let unsupported_wire_shape_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("unsupported-wire-shape"))
        .count();
    let low_confidence_field_count = fields
        .iter()
        .filter(|field| !field.confidence.is_high_or_exact())
        .count();
    let placeholder_field_name_count = fields
        .iter()
        .filter(|field| is_placeholder_report_field_name(field))
        .count();
    let evidence_issues = message_evidence_issues(network_type);
    let mut blocked_reasons = message_blocked_reasons(network_type, &fields);
    if !evidence_issues.is_empty() {
        blocked_reasons.push(format!("invalid-evidence:{}", evidence_issues.len()));
    }

    NetworkMessageGenerationPlanReport {
        type_index: network_type.type_index,
        type_name: network_type.name.clone(),
        analysis_status: network_type
            .instance
            .as_ref()
            .and_then(|instance| instance.analysis_status),
        empty_wire_proven: network_type
            .instance
            .as_ref()
            .is_some_and(|instance| instance.empty_wire_proven),
        supports_unmarshal,
        field_count,
        shaped_field_count,
        supported_field_count,
        missing_wire_shape_count,
        missing_field_type_count,
        missing_support_type_count,
        missing_composite_support_type_count,
        placeholder_field_name_count,
        unsupported_wire_shape_count,
        low_confidence_field_count,
        evidence_issues,
        can_generate: blocked_reasons.is_empty(),
        blocked_reasons,
        fields,
    }
}

pub(super) fn disambiguate_report_field_names(fields: &mut [NetworkStateFieldShapeReport]) {
    let mut seen = BTreeMap::<String, usize>::new();
    for (ordinal, field) in fields.iter_mut().enumerate() {
        let Some(name) = field.field_name.as_deref() else {
            continue;
        };
        let ident = rust_field_ident(name);
        if let std::collections::btree_map::Entry::Vacant(entry) = seen.entry(ident.clone()) {
            entry.insert(1);
            continue;
        }

        let suffix_seed = field.field_index.unwrap_or(ordinal as u32);
        let mut attempt = 0;
        let candidate = loop {
            let suffix = if attempt == 0 {
                suffix_seed.to_string()
            } else {
                format!("{suffix_seed}_{attempt}")
            };
            let candidate = format!("{name}_{suffix}");
            let candidate_ident = rust_field_ident(&candidate);
            if let std::collections::btree_map::Entry::Vacant(entry) = seen.entry(candidate_ident) {
                entry.insert(1);
                break candidate;
            }
            attempt += 1;
        };

        if let Some(count) = seen.get_mut(&ident) {
            *count += 1;
        }
        field.field_name = Some(candidate);
    }
}

pub(super) const BLOCKER_EXAMPLE_LIMIT: usize = 8;
pub(super) const BLOCKED_FIELD_EXAMPLE_LIMIT: usize = 8;

pub(super) fn message_blocker_summary(
    plans: &[NetworkMessageGenerationPlanReport],
) -> NetworkBlockerSummaryReport {
    let mut reason_buckets = BTreeMap::<String, NetworkBlockerReasonBucketReport>::new();
    let mut combination_buckets =
        BTreeMap::<Vec<String>, NetworkBlockerCombinationBucketReport>::new();

    for plan in plans.iter().filter(|plan| !plan.can_generate) {
        let example = blocked_type_example(plan);
        let reason_families = plan
            .blocked_reasons
            .iter()
            .map(|reason| blocker_reason_family(reason).to_owned())
            .collect::<BTreeSet<_>>();
        for reason in reason_families {
            let bucket = reason_buckets.entry(reason.clone()).or_insert_with(|| {
                NetworkBlockerReasonBucketReport {
                    reason,
                    ..NetworkBlockerReasonBucketReport::default()
                }
            });
            bucket.type_count += 1;
            bucket.blocked_field_count += blocked_field_count_for_reason(plan, &bucket.reason);
            if bucket.examples.len() < BLOCKER_EXAMPLE_LIMIT {
                bucket.examples.push(example.clone());
            }
        }

        let mut reasons = plan.blocked_reasons.clone();
        reasons.sort();
        let bucket = combination_buckets
            .entry(reasons.clone())
            .or_insert_with(|| NetworkBlockerCombinationBucketReport {
                reasons,
                ..NetworkBlockerCombinationBucketReport::default()
            });
        bucket.type_count += 1;
        if bucket.examples.len() < BLOCKER_EXAMPLE_LIMIT {
            bucket.examples.push(example);
        }
    }

    let mut reason_buckets = reason_buckets.into_values().collect::<Vec<_>>();
    reason_buckets.sort_by(|left, right| {
        right
            .type_count
            .cmp(&left.type_count)
            .then_with(|| left.reason.cmp(&right.reason))
    });

    let mut combination_buckets = combination_buckets.into_values().collect::<Vec<_>>();
    combination_buckets.sort_by(|left, right| {
        right
            .type_count
            .cmp(&left.type_count)
            .then_with(|| left.reasons.cmp(&right.reasons))
    });

    NetworkBlockerSummaryReport {
        total_plan_count: plans.len(),
        generatable_count: plans.iter().filter(|plan| plan.can_generate).count(),
        blocked_count: plans.iter().filter(|plan| !plan.can_generate).count(),
        reason_buckets,
        combination_buckets,
    }
}

pub(super) fn blocker_reason_family(reason: &str) -> &str {
    reason.split_once(':').map_or(reason, |(family, _)| family)
}

pub(super) fn blocked_field_count_for_reason(
    plan: &NetworkMessageGenerationPlanReport,
    reason: &str,
) -> usize {
    plan.fields
        .iter()
        .filter(|field| {
            field
                .blocked_reason
                .as_deref()
                .is_some_and(|field_reason| blocker_reason_family(field_reason) == reason)
        })
        .count()
}

pub(super) fn blocked_type_example(
    plan: &NetworkMessageGenerationPlanReport,
) -> NetworkBlockedTypeExampleReport {
    NetworkBlockedTypeExampleReport {
        type_index: plan.type_index,
        type_name: plan.type_name.clone(),
        field_count: plan.field_count,
        blocked_reasons: plan.blocked_reasons.clone(),
        blocked_fields: plan
            .fields
            .iter()
            .filter(|field| field.blocked_reason.is_some())
            .take(BLOCKED_FIELD_EXAMPLE_LIMIT)
            .map(blocked_field_example)
            .collect(),
    }
}

pub(super) fn blocked_field_example(
    field: &NetworkStateFieldShapeReport,
) -> NetworkBlockedFieldExampleReport {
    NetworkBlockedFieldExampleReport {
        field_index: field.field_index,
        field_name: field.field_name.clone(),
        native_type: field.native_type.clone(),
        source_type_name: field.source_type_name.clone(),
        source_type_id: field.source_type_id,
        serialize_type_name: field.serialize_type_name.clone(),
        wire_layout: field.wire_layout.clone(),
        wire_layout_source: field.wire_layout_source.clone(),
        value_type_candidates: field.value_type_candidates.clone(),
        rust_value_type: field.rust_value_type.clone(),
        blocked_reason: field.blocked_reason.clone(),
    }
}
