use super::*;

impl NetworkSchema {
    pub fn from_ghidra_static_network_report(
        report: &Value,
    ) -> Result<Self, NetworkSchemaImportError> {
        Self::from_ghidra_static_network_report_with_context(report, &CodegenContext::inline())
    }

    pub fn from_ghidra_static_network_report_with_context(
        report: &Value,
        context: &CodegenContext,
    ) -> Result<Self, NetworkSchemaImportError> {
        let root = report
            .as_object()
            .ok_or(NetworkSchemaImportError::ExpectedObjectRoot)?;
        if contains_private_source_evidence(report) {
            return Err(NetworkSchemaImportError::PrivateSourceEvidence);
        }
        let registry_entries = array_values(root, "registryEntries")
            .filter_map(Value::as_object)
            .collect::<Vec<_>>();
        let field_registration_function_entries = array_values(root, "fieldRegistrationFunctions")
            .filter_map(Value::as_object)
            .collect::<Vec<_>>();
        let field_handler_vtable_entries = array_values(root, "fieldHandlerVtables")
            .filter_map(Value::as_object)
            .collect::<Vec<_>>();
        let ((types, field_registration_functions), field_handler_vtables) = context.runner().join(
            || {
                context.runner().join(
                    || {
                        context.runner().map(&registry_entries, |entry| {
                            network_type_from_registry_entry(entry)
                        })
                    },
                    || {
                        context
                            .runner()
                            .map(&field_registration_function_entries, |entry| {
                                network_field_registration_function(entry)
                            })
                    },
                )
            },
            || {
                context
                    .runner()
                    .map(&field_handler_vtable_entries, |entry| {
                        network_field_handler_vtable(entry)
                    })
            },
        );
        let mut schema = Self {
            schema: NETWORK_SCHEMA_VERSION.to_owned(),
            sources: vec![NetworkSchemaSource {
                kind: NetworkSchemaSourceKind::GhidraNetworkStaticReport,
                path: string(root, "input"),
                schema: Some(NETWORK_STATIC_REPORT_SCHEMA_VERSION.to_owned()),
                program: string(root, "program"),
                image_base: string(root, "imageBase"),
            }],
            summary: NetworkSchemaSummary::default(),
            types,
            serialize_types: Vec::new(),
            field_registration_functions,
            field_handler_vtables,
        };
        schema.normalize_derived_shapes();
        Ok(schema)
    }

    pub fn normalize_derived_shapes(&mut self) {
        for vtable in &mut self.field_handler_vtables {
            if vtable.should_suppress_replicated_container_wire_shape() {
                vtable.wire_shape = None;
                vtable.wire_shape_source = None;
                vtable.delta_wire_shape = None;
                vtable.full_wire_shape = None;
            }
        }
        let projections = field_handler_projections(&self.field_handler_vtables);
        for network_type in &mut self.types {
            enrich_fields_from_handler_projections(&mut network_type.fields, &projections);
            for field in &mut network_type.fields {
                collapse_field_alternate_spelling_wire_products(field);
            }
            for field in &mut network_type.marshal_fields {
                collapse_field_alternate_spelling_wire_products(field);
            }
        }
        for function in &mut self.field_registration_functions {
            enrich_fields_from_handler_projections(&mut function.fields, &projections);
        }
        self.suppress_under_shaped_container_wire_shapes();
        promote_replicated_state_capabilities(self);
        self.summary = self.build_summary();
    }

    fn suppress_under_shaped_container_wire_shapes(&mut self) {
        let structured_container_vtables = self
            .field_handler_vtables
            .iter()
            .filter(|vtable| vtable.should_suppress_replicated_container_wire_shape())
            .filter_map(|vtable| vtable.address.as_deref())
            .collect::<BTreeSet<_>>();
        if structured_container_vtables.is_empty() {
            return;
        }

        for network_type in &mut self.types {
            suppress_field_wire_shapes_for_vtables(
                &mut network_type.fields,
                &structured_container_vtables,
            );
        }
        for function in &mut self.field_registration_functions {
            suppress_field_wire_shapes_for_vtables(
                &mut function.fields,
                &structured_container_vtables,
            );
        }
    }

    pub fn merge_typeindex_root(
        &mut self,
        typeindex: &Value,
        source_path: Option<String>,
    ) -> Result<NetworkTypeIndexMergeReport, NetworkSchemaImportError> {
        let root = typeindex
            .as_object()
            .ok_or(NetworkSchemaImportError::ExpectedTypeIndexArray)?;
        let type_ids = root
            .get("typeIndex")
            .and_then(Value::as_array)
            .ok_or(NetworkSchemaImportError::ExpectedTypeIndexArray)?;
        let type_indices = type_ids
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                value
                    .as_str()
                    .and_then(parse_uuid)
                    .zip(u32::try_from(index).ok())
            })
            .collect::<BTreeMap<_, _>>();

        let mut report = NetworkTypeIndexMergeReport {
            source_type_count: type_indices.len(),
            ..NetworkTypeIndexMergeReport::default()
        };
        for network_type in &mut self.types {
            let Some(type_id) = network_type.type_id else {
                report.unmatched_schema_type_count += 1;
                continue;
            };
            let Some(type_index) = type_indices.get(&type_id).copied() else {
                report.unmatched_schema_type_count += 1;
                continue;
            };

            report.matched_type_count += 1;
            match network_type.type_index {
                Some(existing) if existing == type_index => {
                    report.matching_type_index_count += 1;
                    push_unique(
                        &mut network_type.evidence,
                        typeindex_evidence(type_index, NetworkConfidence::Exact, None),
                    );
                }
                Some(existing) => {
                    report.conflicting_type_index_count += 1;
                    push_unique(
                        &mut network_type.evidence,
                        typeindex_evidence(
                            type_index,
                            NetworkConfidence::Weak,
                            Some(format!("typeindex.json={type_index}, existing={existing}")),
                        ),
                    );
                }
                None => {
                    report.filled_type_index_count += 1;
                    network_type.type_index = Some(type_index);
                    push_unique(
                        &mut network_type.evidence,
                        typeindex_evidence(type_index, NetworkConfidence::Exact, None),
                    );
                }
            }
        }
        push_unique(
            &mut self.sources,
            NetworkSchemaSource {
                kind: NetworkSchemaSourceKind::TypeIndex,
                path: source_path,
                schema: None,
                program: None,
                image_base: None,
            },
        );
        self.summary = self.build_summary();
        Ok(report)
    }

    pub fn merge_serialize_codegen_unit(
        &mut self,
        unit: &SerializeCodegenUnit,
        source_path: Option<String>,
    ) -> NetworkSerializeMergeReport {
        let index = unit.index();
        let name_index = serialize_items_by_name(unit);
        let selected_value_types =
            selected_value_type_info_by_handler_vtable(&self.field_handler_vtables);
        let mut report = NetworkSerializeMergeReport {
            source_type_count: unit.items.len(),
            ..NetworkSerializeMergeReport::default()
        };
        self.merge_serialize_type_index(unit, &index);

        for network_type in &mut self.types {
            let Some((item, confidence, source)) =
                serialize_match(network_type, &index, &name_index, &mut report)
            else {
                merge_field_serialize_types(
                    network_type,
                    &index,
                    &selected_value_types,
                    &mut report,
                );
                continue;
            };
            report.matched_type_count += 1;
            if network_type.name.is_none() {
                network_type.name = Some(item.source_name.clone());
                network_type.name_source = Some(source.clone());
                report.filled_name_count += 1;
            }
            network_type.serialize = Some(network_serialize_type(item, &index));
            push_unique(
                &mut network_type.evidence,
                NetworkEvidence {
                    kind: NetworkEvidenceKind::SerializeContext,
                    source,
                    address: None,
                    detail: Some(item.source_name.clone()),
                    confidence,
                },
            );
            merge_field_serialize_types(network_type, &index, &selected_value_types, &mut report);
        }

        push_unique(
            &mut self.sources,
            NetworkSchemaSource {
                kind: NetworkSchemaSourceKind::SerializeContext,
                path: source_path,
                schema: None,
                program: None,
                image_base: None,
            },
        );
        self.summary = self.build_summary();
        report
    }

    /// Restricts semantic source references to the reflected types that are
    /// actually emitted into the consuming generated crate.
    ///
    /// A SerializeContext catalog describes every type that can be generated;
    /// an explicit root selection emits only a closure of that catalog. Network
    /// Rust generation must use the latter as its source-type availability
    /// boundary or it can create references to types absent from the crate.
    pub fn restrict_serialize_source_availability(&mut self, unit: &SerializeCodegenUnit) {
        let available = unit
            .items
            .iter()
            .map(|item| item.source_type_id)
            .collect::<BTreeSet<_>>();
        for serialize in &mut self.serialize_types {
            serialize.emits_source = available.contains(&serialize.type_id);
        }
        self.summary = self.build_summary();
    }

    pub fn merge_serialize_type_catalog(
        &mut self,
        catalog: &crate::catalog::ReflectedTypeCatalog,
    ) -> NetworkSerializeCatalogMergeReport {
        let required_type_ids = crate::network_selection::required_serialize_type_ids(self);
        let mut merged = self
            .serialize_types
            .iter()
            .cloned()
            .map(|serialize| (serialize.type_id, serialize))
            .collect::<BTreeMap<_, _>>();
        let mut report = NetworkSerializeCatalogMergeReport {
            required_type_count: required_type_ids.len(),
            ..NetworkSerializeCatalogMergeReport::default()
        };

        for type_id in required_type_ids {
            let Some(generic) = catalog.generic_type(type_id) else {
                continue;
            };
            report.matched_generic_type_count += 1;
            merged
                .entry(type_id)
                .or_insert_with(|| network_serialize_generic_type(generic));
        }

        self.serialize_types = merged.into_values().collect();
        self.summary = self.build_summary();
        report
    }

    fn merge_serialize_type_index(
        &mut self,
        unit: &SerializeCodegenUnit,
        index: &SerializeCodegenIndex<'_>,
    ) {
        let mut merged = self
            .serialize_types
            .iter()
            .cloned()
            .map(|serialize| (serialize.type_id, serialize))
            .collect::<BTreeMap<_, _>>();
        for item in &unit.items {
            merged
                .entry(item.source_type_id)
                .or_insert_with(|| network_serialize_type(item, index));
        }
        self.serialize_types = merged.into_values().collect();
    }

    pub fn merge_message_signatures(
        &mut self,
        signatures: &[NetworkMessageSignature],
        source_path: Option<String>,
    ) -> NetworkMessageSignatureMergeReport {
        let mut report = NetworkMessageSignatureMergeReport {
            source_message_count: signatures.len(),
            ..NetworkMessageSignatureMergeReport::default()
        };

        for signature in signatures {
            let candidates = message_signature_candidates(&self.types, signature);
            let [network_type_index] = candidates.as_slice() else {
                if candidates.is_empty() {
                    report.unmatched_message_count += 1;
                } else {
                    report.ambiguous_message_count += 1;
                }
                continue;
            };

            let network_type = &mut self.types[*network_type_index];
            network_type.signature_field_count_conflict = false;
            for field in network_type
                .fields
                .iter_mut()
                .chain(&mut network_type.marshal_fields)
            {
                field.signature_type_conflict = false;
                field.signature_wire_conflict = false;
            }
            let source = signature
                .source
                .clone()
                .or_else(|| source_path.clone())
                .unwrap_or_else(|| "messageSignatures".to_owned());
            report.matched_message_count += 1;
            let supports_unmarshal = network_type
                .instance
                .as_ref()
                .and_then(|instance| instance.supports_unmarshal);
            let mut secondary_report = NetworkMessageSignatureMergeReport::default();
            let (unmarshal_report, marshal_report) = if supports_unmarshal == Some(false) {
                (&mut secondary_report, &mut report)
            } else {
                (&mut report, &mut secondary_report)
            };
            let unmarshal_aligned = merge_message_signature_direction(
                &mut network_type.fields,
                &signature.fields,
                &self.serialize_types,
                &source,
                true,
                unmarshal_report,
            );
            let marshal_aligned = merge_message_signature_direction(
                &mut network_type.marshal_fields,
                &signature.fields,
                &self.serialize_types,
                &source,
                false,
                marshal_report,
            );
            let active_directions_aligned = if supports_unmarshal == Some(false) {
                marshal_aligned
            } else {
                unmarshal_aligned && marshal_aligned
            };
            network_type.signature_field_count_conflict = !active_directions_aligned;
            if !active_directions_aligned {
                report.field_count_mismatch_count += 1;
            }
        }

        push_unique(
            &mut self.sources,
            NetworkSchemaSource {
                kind: NetworkSchemaSourceKind::MessageSignatures,
                path: source_path,
                schema: None,
                program: None,
                image_base: None,
            },
        );
        self.summary = self.build_summary();
        report
    }

    pub fn merge_field_overrides(
        &mut self,
        overrides: &NetworkFieldOverrideFile,
        source_path: Option<String>,
    ) -> NetworkFieldOverrideMergeReport {
        let mut report = NetworkFieldOverrideMergeReport {
            source_field_count: overrides.fields.len(),
            ..NetworkFieldOverrideMergeReport::default()
        };

        for field_override in &overrides.fields {
            let type_candidates = field_override_type_candidates(&self.types, field_override);
            let [network_type_index] = type_candidates.as_slice() else {
                if type_candidates.is_empty() {
                    report.unmatched_type_count += 1;
                } else {
                    report.ambiguous_type_count += 1;
                }
                continue;
            };

            let network_type = &mut self.types[*network_type_index];
            let field_candidates = field_override_field_candidates(network_type, field_override);
            let [field_index] = field_candidates.as_slice() else {
                if field_candidates.is_empty() {
                    report.unmatched_field_count += 1;
                } else {
                    report.ambiguous_field_count += 1;
                }
                continue;
            };

            let source = source_path
                .clone()
                .unwrap_or_else(|| "fieldOverrides".to_owned());
            let field = &mut network_type.fields[*field_index];
            if let Some(name) = field_override.name.as_ref()
                && field.name.as_deref() != Some(name.as_str())
            {
                field.name = Some(name.clone());
                report.field_name_updated_count += 1;
            }
            if let Some(native_type) = field_override.native_type.as_ref()
                && field.native_type.as_deref() != Some(native_type.as_str())
            {
                field.native_type = Some(native_type.clone());
                report.native_type_updated_count += 1;
            }
            if let Some(rust_type) = field_override.rust_type.as_ref()
                && field.rust_type.as_deref() != Some(rust_type.as_str())
            {
                field.rust_type = Some(rust_type.clone());
                report.rust_type_updated_count += 1;
            }
            if let Some(wire_shape) = field_override.wire_shape.as_ref()
                && field.wire_shape.as_ref() != Some(wire_shape)
            {
                field.wire_shape = Some(wire_shape.clone());
                field.wire_shape_source = Some(
                    field_override
                        .wire_shape_source
                        .clone()
                        .unwrap_or_else(|| source.clone()),
                );
                report.wire_shape_updated_count += 1;
            } else if let Some(wire_shape_source) = field_override.wire_shape_source.as_ref()
                && field.wire_shape.is_some()
                && field.wire_shape_source.as_deref() != Some(wire_shape_source.as_str())
            {
                field.wire_shape_source = Some(wire_shape_source.clone());
                report.wire_shape_updated_count += 1;
            }
            if let Some(confidence) = field_override.confidence
                && field.confidence != confidence
            {
                field.confidence = confidence;
                report.confidence_updated_count += 1;
            }
            push_unique(
                &mut field.evidence,
                NetworkEvidence {
                    kind: NetworkEvidenceKind::FieldOverride,
                    source: source.clone(),
                    address: None,
                    detail: Some(field_override_detail(field_override)),
                    confidence: field_override.confidence.unwrap_or(NetworkConfidence::High),
                },
            );
            report.matched_field_count += 1;
        }

        push_unique(
            &mut self.sources,
            NetworkSchemaSource {
                kind: NetworkSchemaSourceKind::FieldOverrides,
                path: source_path,
                schema: None,
                program: None,
                image_base: None,
            },
        );
        self.summary = self.build_summary();
        report
    }

    #[must_use]
    pub fn build_summary(&self) -> NetworkSchemaSummary {
        let register_field_count = self
            .field_registration_functions
            .iter()
            .map(|function| function.fields.len())
            .sum::<usize>();
        NetworkSchemaSummary {
            type_count: self.types.len(),
            type_registry_entry_count: self.types.len(),
            typed_type_count: self
                .types
                .iter()
                .filter(|network_type| network_type.type_id.is_some())
                .count(),
            named_type_count: self
                .types
                .iter()
                .filter(|network_type| network_type.name.is_some())
                .count(),
            register_field_function_count: self.field_registration_functions.len(),
            register_field_count,
            typed_register_field_function_count: self
                .field_registration_functions
                .iter()
                .filter(|function| function.owner_type_id.is_some())
                .count(),
            high_confidence_field_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.fields)
                .filter(|field| field.confidence.is_high_or_exact())
                .count(),
            message_unmarshal_field_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.fields)
                .filter(|field| {
                    field
                        .evidence
                        .iter()
                        .any(|evidence| evidence.kind == NetworkEvidenceKind::MessageUnmarshal)
                })
                .count(),
            message_marshal_field_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.marshal_fields)
                .count(),
            type_index_evidence_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.evidence)
                .filter(|evidence| evidence.kind == NetworkEvidenceKind::TypeIndex)
                .count(),
            serialize_source_type_count: self.serialize_types.len(),
            serialize_type_count: self
                .types
                .iter()
                .filter(|network_type| network_type.serialize.is_some())
                .count(),
            serialize_field_type_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.fields)
                .filter(|field| field.serialize.is_some())
                .count(),
            serialize_dependency_count: self
                .types
                .iter()
                .filter_map(|network_type| network_type.serialize.as_ref())
                .map(|serialize| serialize.direct_dependency_type_ids.len())
                .sum(),
            field_handler_vtable_count: self.field_handler_vtables.len(),
            message_source_field_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.fields)
                .filter(|field| {
                    field
                        .evidence
                        .iter()
                        .any(|evidence| evidence.kind == NetworkEvidenceKind::MessageSource)
                })
                .count(),
        }
    }
}

fn merge_message_signature_direction(
    fields: &mut Vec<NetworkField>,
    signatures: &[NetworkMessageFieldSignature],
    serialize_types: &[NetworkSerializeType],
    source: &str,
    fill_empty: bool,
    report: &mut NetworkMessageSignatureMergeReport,
) -> bool {
    if fields.is_empty() {
        if !fill_empty || signatures.is_empty() {
            return true;
        }
        *fields = network_fields_from_message_signature(signatures, source.to_owned());
        report.field_name_filled_count += signatures.len();
        report.native_type_filled_count += signatures
            .iter()
            .filter(|field| field.native_type.is_some())
            .count();
        report.wire_shape_filled_count += signatures
            .iter()
            .filter(|field| field.wire_shape.is_some())
            .count();
        return true;
    }

    report.field_reordered_count +=
        reorder_message_fields_by_signature(fields, signatures, serialize_types);
    if let Some((grouped, grouped_count)) =
        group_message_fields_by_signature(fields, signatures, serialize_types)
    {
        *fields = grouped;
        report.field_grouped_count += grouped_count;
    } else if fields.len() != signatures.len() {
        if !is_complete_native_listing_signature(signatures, source) {
            return false;
        }
        *fields = network_fields_from_message_signature(signatures, source.to_owned());
        report.field_name_filled_count += signatures.len();
        report.native_type_filled_count += signatures
            .iter()
            .filter(|field| field.native_type.is_some())
            .count();
        report.wire_shape_filled_count += signatures.len();
        return true;
    }

    for (field, signature) in fields.iter_mut().zip(signatures) {
        if let (Some(existing), Some(expected)) = (field.index, signature.index)
            && existing != expected
        {
            report.field_index_mismatch_count += 1;
            continue;
        }

        if field.name.as_deref().is_none_or(is_placeholder_field_name)
            || field_has_native_type_name(field)
        {
            field.name = Some(signature.name.clone());
            report.field_name_filled_count += 1;
        } else if field.name.as_deref() != Some(signature.name.as_str()) {
            report.field_name_conflict_count += 1;
        }

        merge_message_field_native_type(field, signature, report);
        if field.rust_type.is_none() {
            field.rust_type = signature.rust_type.clone();
        }

        if field.wire_shape.is_none()
            && let Some(wire_shape) = signature.wire_shape.as_ref()
        {
            field.wire_shape = Some(wire_shape.clone());
            field.wire_shape_source = Some(source.to_owned());
            report.wire_shape_filled_count += 1;
        } else if let (Some(existing), Some(expected)) =
            (field.wire_shape.as_ref(), signature.wire_shape.as_ref())
            && existing != expected
        {
            if wire_shapes_machine_compatible(existing, expected) {
                if field.wire_layout.is_none() {
                    field.wire_layout = Some(existing.wire_string());
                    field.wire_layout_source = field.wire_shape_source.clone();
                }
                field.wire_shape = Some(expected.clone());
                field.wire_shape_source =
                    Some("message-signature+machine-wire-equivalence".to_owned());
            } else {
                field.signature_wire_conflict = true;
                report.wire_shape_conflict_count += 1;
            }
        }

        if !field.signature_type_conflict
            && !field.signature_wire_conflict
            && !field.confidence.is_high_or_exact()
        {
            field.confidence = NetworkConfidence::High;
        }
        push_unique(
            &mut field.evidence,
            NetworkEvidence {
                kind: NetworkEvidenceKind::MessageSource,
                source: source.to_owned(),
                address: None,
                detail: Some(signature.name.clone()),
                confidence: NetworkConfidence::High,
            },
        );
    }
    true
}

fn is_complete_native_listing_signature(
    fields: &[NetworkMessageFieldSignature],
    source: &str,
) -> bool {
    source == "native-unmarshal-and-marshal-listing"
        && !fields.is_empty()
        && fields.iter().enumerate().all(|(index, field)| {
            field.wire_shape.is_some()
                && field
                    .index
                    .is_none_or(|field_index| u32::try_from(index).ok() == Some(field_index))
        })
}
