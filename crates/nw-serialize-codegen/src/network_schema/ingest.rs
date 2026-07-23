use super::*;

pub(super) fn typeindex_evidence(
    type_index: u32,
    confidence: NetworkConfidence,
    detail: Option<String>,
) -> NetworkEvidence {
    NetworkEvidence {
        kind: NetworkEvidenceKind::TypeIndex,
        source: "typeIndex".to_owned(),
        address: None,
        detail: detail.or_else(|| Some(format!("typeIndex={type_index}"))),
        confidence,
    }
}

pub(super) fn network_type_from_registry_entry(entry: &Map<String, Value>) -> NetworkType {
    let type_id = uuid(entry, "uuid");
    let storage_address = stable_address(entry, "storageAddress");
    let base_vtable = stable_address(entry, "baseVtable");
    let vtable = stable_address(entry, "vtable");
    let handler = entry
        .get("handler")
        .and_then(Value::as_object)
        .map(network_handler);
    let instance = entry
        .get("messageUnmarshal")
        .and_then(Value::as_object)
        .map(network_instance_layout);
    let fragment_metadata = network_type_fragment_metadata(entry);
    let replicated_state_abi = network_type_replicated_state_abi(entry);
    let az_rtti = entry
        .get("azRtti")
        .and_then(Value::as_object)
        .map(network_az_rtti);
    let registration_hook = entry
        .get("registrationHook")
        .and_then(Value::as_object)
        .map(network_registration_hook);
    let name = registry_entry_name(entry, az_rtti.as_ref(), registration_hook.as_ref());
    let mut fields = array_values(entry, "fields")
        .filter_map(Value::as_object)
        .filter(|field| is_plausible_network_field(field))
        .map(network_field)
        .collect::<Vec<_>>();
    reindex_message_fields(&mut fields);
    let mut marshal_fields = entry
        .get("messageMarshal")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|plan| array_values(plan, "fields"))
        .filter_map(Value::as_object)
        .filter(|field| is_plausible_network_field(field))
        .map(network_field)
        .collect::<Vec<_>>();
    reindex_directional_message_fields(&mut marshal_fields, NetworkEvidenceKind::MessageMarshal);
    let has_registered_fields = fields.iter().any(|field| {
        field
            .evidence
            .iter()
            .any(|evidence| evidence.kind == NetworkEvidenceKind::RegisterField)
    });
    let capabilities = network_type_capabilities(
        name.as_deref(),
        has_registered_fields,
        replicated_state_abi.is_some(),
    );
    let mut evidence = Vec::new();

    if type_id.is_some() || entry.contains_key("typeIndex") || entry.contains_key("index") {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::TypeRegistry,
            source: "registryEntries".to_owned(),
            address: storage_address.clone(),
            detail: name.clone(),
            confidence: NetworkConfidence::Exact,
        });
    }
    if let Some(hook) = &registration_hook {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::InstallRegistrationHook,
            source: "registrationHook".to_owned(),
            address: hook.hook_function.clone(),
            detail: hook
                .type_name
                .clone()
                .or_else(|| hook.slot_type_name.clone()),
            confidence: NetworkConfidence::High,
        });
    }
    if let Some(rtti) = &az_rtti {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::AzRtti,
            source: rtti.source.clone().unwrap_or_else(|| "azRtti".to_owned()),
            address: rtti.address.clone(),
            detail: rtti.type_name.clone(),
            confidence: NetworkConfidence::High,
        });
    }
    if handler.is_some() {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::HandlerVtable,
            source: "handler".to_owned(),
            address: vtable.clone().or_else(|| base_vtable.clone()),
            detail: None,
            confidence: NetworkConfidence::High,
        });
    }
    if let Some(metadata) = &fragment_metadata {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::FragmentMetadata,
            source: metadata
                .source
                .clone()
                .unwrap_or_else(|| "fragmentMetadata".to_owned()),
            address: metadata.category_function.clone(),
            detail: metadata.category.clone(),
            confidence: NetworkConfidence::High,
        });
    }
    if let Some(abi) = &replicated_state_abi {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::ReplicatedStateAbi,
            source: abi.source.clone(),
            address: abi
                .functions
                .first()
                .map(|function| function.function.clone()),
            detail: Some(format!(
                "vtable slots {}..{} shared by {} registered fragments",
                abi.first_slot,
                abi.first_slot + abi.slot_count - 1,
                abi.cohort_count
            )),
            confidence: NetworkConfidence::Exact,
        });
    }

    NetworkType {
        type_id,
        type_index: u32_value(entry, "typeIndex"),
        registry_index: u32_value(entry, "index"),
        name,
        name_source: string(entry, "typeNameSource"),
        capabilities,
        storage_address,
        base_vtable,
        vtable,
        handler,
        instance,
        fragment_metadata,
        replicated_state_abi,
        serialize: None,
        az_rtti,
        registration_type_name: string(entry, "registrationTypeName"),
        registration_hook,
        fields,
        marshal_fields,
        signature_field_count_conflict: false,
        evidence,
    }
}

pub(super) fn is_plausible_network_field(field: &Map<String, Value>) -> bool {
    let Some(confidence) = string_ref(field, "confidence") else {
        return true;
    };
    if !confidence.starts_with("message-unmarshal") {
        return true;
    }

    let has_known_field_type = string_ref(field, "wireShape").is_some()
        || string_ref(field, "rustType").is_some()
        || string_ref(field, "nativeType").is_some();
    let Some(storage) = string_ref(field, "storageExpression") else {
        return has_known_field_type;
    };
    let storage = storage.trim();
    storage.starts_with("_Dst")
        || ((storage.contains("param_") || storage.contains("plVar") || storage.contains("puVar"))
            && storage.contains('+'))
}

pub(super) fn reindex_message_fields(fields: &mut [NetworkField]) {
    reindex_directional_message_fields(fields, NetworkEvidenceKind::MessageUnmarshal);
}

fn reindex_directional_message_fields(
    fields: &mut [NetworkField],
    evidence_kind: NetworkEvidenceKind,
) {
    if fields.iter().all(|field| {
        field
            .evidence
            .iter()
            .any(|evidence| evidence.kind == evidence_kind)
    }) {
        for (index, field) in fields.iter_mut().enumerate() {
            field.index = Some(index as u32);
        }
    }
}

pub(super) fn network_field_registration_function(
    function: &Map<String, Value>,
) -> NetworkFieldRegistrationFunction {
    let az_rtti = function
        .get("azRtti")
        .and_then(Value::as_object)
        .map(network_az_rtti);
    let fragment_metadata = function
        .get("fragmentMetadata")
        .and_then(Value::as_object)
        .map(network_fragment_metadata);
    let replicated_state_abi = function
        .get("replicatedStateAbi")
        .and_then(Value::as_object)
        .and_then(replicated_state_abi_evidence);
    let fields = array_values(function, "fields")
        .filter_map(Value::as_object)
        .map(network_field)
        .collect::<Vec<_>>();
    let virtual_functions = array_values(function, "virtualFunctions")
        .filter_map(Value::as_object)
        .map(network_virtual_function)
        .collect::<Vec<_>>();
    let mut evidence = vec![NetworkEvidence {
        kind: NetworkEvidenceKind::FieldRegistrationFunction,
        source: "fieldRegistrationFunctions".to_owned(),
        address: string(function, "address"),
        detail: string(function, "name"),
        confidence: NetworkConfidence::High,
    }];
    if let Some(rtti) = &az_rtti {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::AzRtti,
            source: rtti.source.clone().unwrap_or_else(|| "azRtti".to_owned()),
            address: rtti.address.clone(),
            detail: rtti.type_name.clone(),
            confidence: NetworkConfidence::High,
        });
    }
    if let Some(metadata) = &fragment_metadata {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::FragmentMetadata,
            source: metadata
                .source
                .clone()
                .unwrap_or_else(|| "fragmentMetadata".to_owned()),
            address: metadata.category_function.clone(),
            detail: metadata.category.clone(),
            confidence: NetworkConfidence::High,
        });
    }
    if let Some(abi) = &replicated_state_abi {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::ReplicatedStateAbi,
            source: abi.source.clone(),
            address: abi
                .functions
                .first()
                .map(|function| function.function.clone()),
            detail: Some(format!("{} shared vtable slots", abi.slot_count)),
            confidence: NetworkConfidence::Exact,
        });
    }

    NetworkFieldRegistrationFunction {
        address: string(function, "address"),
        name: string(function, "name"),
        constructor_type_name: string(function, "constructorTypeName"),
        owner_type_id: az_rtti.as_ref().and_then(|rtti| rtti.type_id),
        owner_type_name: string(function, "constructorTypeName")
            .or_else(|| az_rtti.as_ref().and_then(|rtti| rtti.type_name.clone())),
        instance_vtable: string(function, "instanceVtable"),
        virtual_functions,
        fragment_metadata,
        replicated_state_abi,
        az_rtti,
        fields,
        evidence,
    }
}

pub(super) fn network_type_fragment_metadata(
    entry: &Map<String, Value>,
) -> Option<NetworkFragmentMetadata> {
    entry
        .get("fragmentMetadata")
        .and_then(Value::as_object)
        .map(network_fragment_metadata)
        .or_else(|| {
            array_values(entry, "constructorMatches")
                .filter_map(Value::as_object)
                .find_map(|constructor| {
                    constructor
                        .get("fragmentMetadata")
                        .and_then(Value::as_object)
                        .map(network_fragment_metadata)
                })
        })
}

pub(super) fn network_fragment_metadata(metadata: &Map<String, Value>) -> NetworkFragmentMetadata {
    NetworkFragmentMetadata {
        source: string(metadata, "source"),
        is_metadata_slot: u32_value(metadata, "isMetadataSlot"),
        is_metadata_function: stable_address(metadata, "isMetadataFunction"),
        is_metadata: bool_value(metadata, "isMetadata"),
        category_slot: u32_value(metadata, "categorySlot"),
        category_function: stable_address(metadata, "categoryFunction"),
        category_value: u32_value(metadata, "categoryValue"),
        category: string(metadata, "category"),
    }
}

pub(super) fn network_field(field: &Map<String, Value>) -> NetworkField {
    let raw_confidence = string(field, "confidence");
    let confidence = confidence_from_raw(string_ref(field, "confidence"));
    let evidence_kind = match raw_confidence.as_deref() {
        Some(value) if value.starts_with("message-unmarshal") => {
            NetworkEvidenceKind::MessageUnmarshal
        }
        Some(value) if value.starts_with("message-marshal") => NetworkEvidenceKind::MessageMarshal,
        Some(value) if value.starts_with("message-signature") => NetworkEvidenceKind::MessageSource,
        _ => NetworkEvidenceKind::RegisterField,
    };
    let mut evidence = vec![NetworkEvidence {
        kind: evidence_kind,
        source: raw_confidence.unwrap_or_else(|| "field".to_owned()),
        address: string(field, "callsite"),
        detail: string(field, "name").or_else(|| string(field, "nativeType")),
        confidence,
    }];
    if let Some(name_source) = string(field, "nameSource") {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::MessageSource,
            source: name_source,
            address: string(field, "nameSourceAddress"),
            detail: string(field, "sourceTypeName").or_else(|| string(field, "name")),
            confidence: NetworkConfidence::High,
        });
    }
    let mut native_type = string(field, "nativeType");
    let wire_shape_raw = string(field, "wireShape");
    let mut wire_shape = wire_shape_raw.as_deref().and_then(parse_network_wire_shape);
    let mut wire_shape_source = string(field, "wireShapeSource");
    let raw_byte_length = u32_value(field, "rawByteLength");
    let helper_internal_conflict =
        raw_byte_length_conflicts_with_wire_shape(raw_byte_length, wire_shape.as_ref())
            && wire_shape_source
                .as_deref()
                .is_some_and(|source| source.starts_with("message-unmarshal-helper-"));
    let raw_byte_length = consistent_raw_byte_length(raw_byte_length, wire_shape.as_ref());
    if helper_internal_conflict {
        native_type = None;
        wire_shape = None;
        wire_shape_source = None;
    }
    NetworkField {
        index: u32_value(field, "index"),
        name: string(field, "name"),
        name_address: string(field, "nameAddress"),
        group: u32_value(field, "group"),
        registration_kind: string(field, "registrationKind"),
        filter_group_attribute: bool_value(field, "filterGroupAttribute"),
        handler_offset: string(field, "handlerOffset"),
        handler_expression: string(field, "handlerExpression"),
        handler_vtable: string(field, "handlerVtable"),
        handler_kind: string(field, "handlerKind"),
        handler_kind_source: string(field, "handlerKindSource"),
        handler_vtable_slots: u32_value(field, "handlerVtableSlots"),
        physical_field_count: u32_value(field, "physicalFieldCount"),
        native_type,
        source_type_name: string(field, "sourceTypeName"),
        source_type_id: uuid(field, "sourceTypeId"),
        source_type_id_source: string(field, "sourceTypeIdSource"),
        source_type_identity_proven: bool_value(field, "sourceTypeIdentityProven").unwrap_or(false),
        rust_type: string(field, "rustType"),
        storage_expression: string(field, "storageExpression"),
        storage_base: string(field, "storageBase"),
        storage_base_offset: hex_or_decimal_u32(field, "storageBaseOffset"),
        storage_offset: hex_or_decimal_u32(field, "storageOffset"),
        raw_byte_length,
        wire_shape,
        wire_shape_raw,
        wire_layout: string(field, "wireLayout"),
        wire_layout_source: string(field, "wireLayoutSource"),
        type_conflict: bool_value(field, "typeConflict").unwrap_or(false),
        signature_type_conflict: false,
        signature_wire_conflict: false,
        wire_shape_source,
        constructor_writes: network_field_constructor_writes(field),
        unmarshal_evidence: network_field_unmarshal_evidence(field),
        nested_type_shape: network_field_nested_type_shape(field),
        serialize: None,
        callsite: string(field, "callsite"),
        confidence,
        evidence,
    }
}

pub(super) fn suppress_field_wire_shapes_for_vtables(
    fields: &mut [NetworkField],
    vtables: &BTreeSet<&str>,
) {
    for field in fields {
        let Some(handler_vtable) = field.handler_vtable.as_deref() else {
            continue;
        };
        if !vtables.contains(handler_vtable) {
            continue;
        }
        if field
            .wire_shape
            .as_ref()
            .is_some_and(NetworkWireShape::is_replicated_container)
        {
            field.wire_shape = None;
            field.wire_shape_source = None;
        }
    }
}

pub(super) fn network_field_unmarshal_evidence(
    field: &Map<String, Value>,
) -> Option<NetworkFieldUnmarshalEvidence> {
    let evidence = field.get("unmarshalEvidence")?.as_object()?;
    Some(NetworkFieldUnmarshalEvidence {
        callsite: string(evidence, "callsite"),
        target_name: string(evidence, "targetName"),
        target_kind: string(evidence, "targetKind"),
        evidence_source: string(evidence, "evidenceSource"),
    })
}

pub(super) fn network_field_nested_type_shape(
    field: &Map<String, Value>,
) -> Option<NetworkNestedTypeShape> {
    let shape = field.get("nestedTypeShape")?.as_object()?;
    Some(network_nested_type_shape(shape))
}

pub(super) fn network_nested_type_shape(shape: &Map<String, Value>) -> NetworkNestedTypeShape {
    NetworkNestedTypeShape {
        type_id: uuid(shape, "typeId"),
        type_id_source: string(shape, "typeIdSource"),
        identity_proven: bool_value(shape, "identityProven"),
        identity_source: string(shape, "identitySource"),
        type_name: string(shape, "typeName"),
        type_name_full: string(shape, "typeNameFull"),
        type_name_source: string(shape, "typeNameSource"),
        function: string(shape, "function"),
        function_name: string(shape, "functionName"),
        factory: string(shape, "factory"),
        az_rtti_address: string(shape, "azRttiAddress"),
        constructor: string(shape, "constructor"),
        vtable: string(shape, "vtable"),
        member_base: string(shape, "memberBase"),
        member_name_source: string(shape, "memberNameSource"),
        member_names_proven: bool_value(shape, "memberNamesProven"),
        layout_proven: bool_value(shape, "layoutProven"),
        member_coverage_proven: bool_value(shape, "memberCoverageProven"),
        wire_order_proven: bool_value(shape, "wireOrderProven"),
        wire_order_source: string(shape, "wireOrderSource"),
        datatype_path: string(shape, "datatypePath"),
        validation: string(shape, "validation"),
        native_size: u64_value(shape, "nativeSize"),
        native_size_source: string(shape, "nativeSizeSource"),
        members: shape
            .get("members")
            .and_then(Value::as_array)
            .map(|members| {
                members
                    .iter()
                    .filter_map(Value::as_object)
                    .map(network_nested_type_member)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub(super) fn network_nested_type_member(member: &Map<String, Value>) -> NetworkNestedTypeMember {
    NetworkNestedTypeMember {
        index: u32_value(member, "index"),
        offset: string(member, "offset"),
        native_offset: string(member, "nativeOffset"),
        name: string(member, "name"),
        name_source: string(member, "nameSource"),
        name_proven: bool_value(member, "nameProven"),
        name_evidence: string(member, "nameEvidence"),
        native_type: string(member, "nativeType"),
        type_id: string(member, "typeId").as_deref().and_then(parse_uuid),
        type_id_source: string(member, "typeIdSource"),
        type_identity_proven: bool_value(member, "typeIdentityProven").unwrap_or(false),
        type_identity_source: string(member, "typeIdentitySource"),
        wire_shape: string(member, "wireShape"),
        wire_shape_source: string(member, "wireShapeSource"),
        wire_layout: string(member, "wireLayout"),
        wire_layout_source: string(member, "wireLayoutSource"),
        byte_width: u32_value(member, "byteWidth"),
        wire_ordinal: u32_value(member, "wireOrdinal"),
        wire_order_source: string(member, "wireOrderSource"),
        callsite: string(member, "callsite"),
        target: string(member, "target"),
        target_name: string(member, "targetName"),
        type_conflict: bool_value(member, "typeConflict").unwrap_or(false),
    }
}

pub(super) fn network_field_constructor_writes(
    field: &Map<String, Value>,
) -> Vec<NetworkFieldConstructorWrite> {
    array_values(field, "constructorWrites")
        .filter_map(Value::as_object)
        .map(|write| NetworkFieldConstructorWrite {
            write: string(write, "write"),
            handler_offset: string(write, "handlerOffset"),
            relative_offset: string(write, "relativeOffset"),
            width_bits: u32_value(write, "widthBits"),
            byte_length: u32_value(write, "byteLength"),
            value_kind: string(write, "valueKind"),
            value: string(write, "value"),
            value_hex: string(write, "valueHex"),
            source_operand: string(write, "sourceOperand"),
            source: string(write, "source"),
        })
        .collect()
}

pub(super) fn contains_private_source_evidence(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            key.starts_with("sourceReplicated")
                || contains_private_source_marker(key)
                || contains_private_source_evidence(value)
        }),
        Value::Array(values) => values.iter().any(contains_private_source_evidence),
        Value::String(value) => contains_private_source_marker(value),
        _ => false,
    }
}

pub(super) fn contains_private_source_marker(value: &str) -> bool {
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    normalized == "source-replicated-field-handler"
        || normalized.contains("resources/newworld/src")
        || normalized.contains("new-world/gems/newworld/src")
        || normalized.contains("newworld/src")
}

pub(super) fn consistent_raw_byte_length(
    raw_byte_length: Option<u32>,
    wire_shape: Option<&NetworkWireShape>,
) -> Option<u32> {
    let byte_length = raw_byte_length?;
    if raw_byte_length_conflicts_with_wire_shape(raw_byte_length, wire_shape) {
        None
    } else {
        Some(byte_length)
    }
}

pub(super) fn raw_byte_length_conflicts_with_wire_shape(
    raw_byte_length: Option<u32>,
    wire_shape: Option<&NetworkWireShape>,
) -> bool {
    let Some(byte_length) = raw_byte_length else {
        return false;
    };
    match wire_shape {
        Some(NetworkWireShape::U8) => byte_length != 1,
        Some(NetworkWireShape::FixedBytes(width)) => byte_length != u32::from(*width),
        Some(_) => true,
        None => false,
    }
}

pub(super) fn network_virtual_function(function: &Map<String, Value>) -> NetworkVirtualFunction {
    NetworkVirtualFunction {
        slot: u32_value(function, "slot"),
        slot_offset: string(function, "slotOffset"),
        name: string(function, "name"),
        address: string(function, "address"),
        target: string(function, "target"),
        function: string(function, "function"),
    }
}

pub(super) fn network_field_handler_vtable(
    vtable: &Map<String, Value>,
) -> NetworkFieldHandlerVtable {
    let confidence = NetworkConfidence::High;
    let mut result = NetworkFieldHandlerVtable {
        address: string(vtable, "address"),
        field_count: usize_value(vtable, "fieldCount").unwrap_or_default(),
        handler_kind: string(vtable, "handlerKind"),
        handler_kind_source: string(vtable, "handlerKindSource"),
        handler_type_name: string(vtable, "handlerTypeName"),
        handler_type_source: string(vtable, "handlerTypeSource"),
        handler_container_type: vtable
            .get("handlerContainerType")
            .and_then(Value::as_object)
            .and_then(network_handler_container_type),
        vtable_slots: u32_value(vtable, "vtableSlots"),
        physical_field_count: u32_value(vtable, "physicalFieldCount"),
        marshal: string(vtable, "marshal"),
        marshal_target: string(vtable, "marshalTarget"),
        unmarshal: string(vtable, "unmarshal"),
        unmarshal_target: string(vtable, "unmarshalTarget"),
        wire_shape: wire_shape(vtable, "wireShape"),
        wire_shape_source: string(vtable, "wireShapeSource"),
        wire_layout: string(vtable, "wireLayout"),
        wire_layout_source: string(vtable, "wireLayoutSource"),
        delta_wire_shape: wire_shape(vtable, "deltaWireShape"),
        full_wire_shape: wire_shape(vtable, "fullWireShape"),
        delta_wire_layout: string(vtable, "deltaWireLayout"),
        full_wire_layout: string(vtable, "fullWireLayout"),
        key_native_type: string(vtable, "keyNativeType"),
        key_native_type_source: string(vtable, "keyNativeTypeSource"),
        delta_marshal_shapes: string_array(vtable, "deltaMarshalShapes"),
        full_marshal_shapes: string_array(vtable, "fullMarshalShapes"),
        delta_marshal_layouts: string_array(vtable, "deltaMarshalLayouts"),
        full_marshal_layouts: string_array(vtable, "fullMarshalLayouts"),
        value_type_info: vtable
            .get("valueTypeInfo")
            .and_then(Value::as_object)
            .map(native_type_info_evidence),
        value_type_candidates: native_type_info_candidates(vtable, "valueTypeInfoCandidates"),
        value_type_shape: vtable
            .get("valueTypeShape")
            .and_then(Value::as_object)
            .map(network_nested_type_shape),
        embedded_value_type_shapes: array_values(vtable, "embeddedValueTypeShapes")
            .filter_map(Value::as_object)
            .map(network_nested_type_shape)
            .collect(),
        full_container_plan: vtable
            .get("fullContainerPlan")
            .and_then(Value::as_object)
            .and_then(container_plan::parse_plan),
        full_container_plan_diagnostics: container_plan::parse_diagnostics(
            vtable,
            "fullContainerPlanDiagnostics",
        ),
        fixed_sequence_shape: vtable
            .get("fixedSequenceShape")
            .and_then(Value::as_object)
            .and_then(parse_fixed_sequence_shape),
        slots: array_values(vtable, "slots")
            .filter_map(Value::as_object)
            .map(network_virtual_function)
            .collect(),
        evidence: vec![NetworkEvidence {
            kind: NetworkEvidenceKind::HandlerVtable,
            source: "fieldHandlerVtables".to_owned(),
            address: string(vtable, "address"),
            detail: None,
            confidence,
        }],
    };
    if result.should_suppress_replicated_container_wire_shape() {
        result.wire_shape = None;
        result.wire_shape_source = None;
        result.delta_wire_shape = None;
        result.full_wire_shape = None;
    }
    result
}

fn network_handler_container_type(
    value: &Map<String, Value>,
) -> Option<NetworkHandlerContainerType> {
    let storage_kind = match string_ref(value, "storageKind")? {
        "index-map" => NetworkReplicatedContainerStorageKind::Map,
        "vector" => NetworkReplicatedContainerStorageKind::Vec,
        _ => return None,
    };
    let key_native_type = string(value, "keyNativeType");
    if storage_kind == NetworkReplicatedContainerStorageKind::Map && key_native_type.is_none() {
        return None;
    }
    Some(NetworkHandlerContainerType {
        storage_kind,
        key_native_type,
        value_native_type: string_ref(value, "valueNativeType")?.to_owned(),
        storage_native_type: string(value, "storageNativeType"),
        key_marshaler_type: string(value, "keyMarshalerType"),
        value_marshaler_type: string(value, "valueMarshalerType"),
        source: string(value, "source"),
    })
}

pub(super) fn native_type_info_candidates(
    object: &Map<String, Value>,
    key: &str,
) -> Vec<NetworkNativeTypeInfoEvidence> {
    array_values(object, key)
        .filter_map(Value::as_object)
        .map(native_type_info_evidence)
        .collect()
}

pub(super) fn native_type_info_evidence(
    object: &Map<String, Value>,
) -> NetworkNativeTypeInfoEvidence {
    NetworkNativeTypeInfoEvidence {
        address: string(object, "address"),
        name: string(object, "name"),
        type_id: uuid(object, "typeId"),
        source: string(object, "source"),
        name_source: string(object, "nameSource"),
        native_size: hex_or_decimal_u64(object, "nativeSize"),
        native_size_source: string(object, "nativeSizeSource"),
    }
}

pub(super) fn network_handler(handler: &Map<String, Value>) -> NetworkHandler {
    NetworkHandler {
        destructor: string(handler, "Destructor"),
        get_empty_value: string(handler, "GetEmptyValue"),
        create_instance: string(handler, "CreateInstance"),
        copy_value: string(handler, "CopyValue"),
        marshal: string(handler, "Marshal"),
        unmarshal: string(handler, "Unmarshal"),
    }
}

pub(super) fn network_instance_layout(
    message_unmarshal: &Map<String, Value>,
) -> NetworkInstanceLayout {
    let delegated_codec = message_unmarshal
        .get("delegatedCodec")
        .and_then(Value::as_object)
        .and_then(network_delegated_codec);
    let confidence = if message_unmarshal.contains_key("instanceSize") {
        NetworkConfidence::High
    } else {
        NetworkConfidence::Inferred
    };
    NetworkInstanceLayout {
        create_instance: string(message_unmarshal, "createInstance"),
        analysis_status: string(message_unmarshal, "analysisStatus").and_then(
            |status| match status.as_str() {
                "recovered-fields" => Some(NetworkMessageAnalysisStatus::RecoveredFields),
                "marshal-only" => Some(NetworkMessageAnalysisStatus::MarshalOnly),
                "delegated-codec" => Some(NetworkMessageAnalysisStatus::DelegatedCodec),
                "proven-empty" => Some(NetworkMessageAnalysisStatus::ProvenEmpty),
                "unresolved" => Some(NetworkMessageAnalysisStatus::Unresolved),
                _ => None,
            },
        ),
        empty_wire_proven: message_unmarshal
            .get("emptyWireProven")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        empty_wire_evidence_source: string(message_unmarshal, "emptyWireEvidenceSource"),
        supports_unmarshal: message_unmarshal
            .get("supportsUnmarshal")
            .and_then(Value::as_bool),
        terminal_status: string(message_unmarshal, "terminalStatus"),
        size: hex_or_decimal_u32(message_unmarshal, "instanceSize"),
        size_source: string(message_unmarshal, "instanceSizeSource"),
        constructor: string(message_unmarshal, "instanceConstructor"),
        constructor_callsite: string(message_unmarshal, "instanceConstructorCallsite"),
        constructor_name: string(message_unmarshal, "instanceConstructorName"),
        delegated_codec,
        evidence: vec![NetworkEvidence {
            kind: NetworkEvidenceKind::InstanceLayout,
            source: string(message_unmarshal, "instanceSizeSource")
                .unwrap_or_else(|| "messageUnmarshal".to_owned()),
            address: string(message_unmarshal, "createInstance"),
            detail: string(message_unmarshal, "instanceConstructorName"),
            confidence,
        }],
    }
}

pub(super) fn network_delegated_codec(codec: &Map<String, Value>) -> Option<NetworkDelegatedCodec> {
    Some(NetworkDelegatedCodec {
        kind: string(codec, "kind")?,
        function: string(codec, "function")?,
        callsite: string(codec, "callsite")?,
        value_storage: string(codec, "valueStorage"),
        outcome_storage: string(codec, "outcomeStorage"),
        read_buffer_storage: string(codec, "readBufferStorage")?,
        evidence_source: string(codec, "evidenceSource")?,
    })
}

pub(super) fn network_az_rtti(rtti: &Map<String, Value>) -> NetworkAzRtti {
    NetworkAzRtti {
        source: string(rtti, "source"),
        address: string(rtti, "address"),
        type_id: uuid(rtti, "typeId"),
        type_name: string(rtti, "typeName"),
        providers: array_values(rtti, "providers")
            .filter_map(Value::as_object)
            .map(network_az_rtti_provider)
            .collect(),
    }
}

pub(super) fn network_az_rtti_provider(provider: &Map<String, Value>) -> NetworkAzRttiProvider {
    NetworkAzRttiProvider {
        kind: string(provider, "kind"),
        slot: u32_value(provider, "slot"),
        slot_offset: string(provider, "slotOffset"),
        function: string(provider, "function"),
        provider: string(provider, "provider"),
        type_id: uuid(provider, "typeId"),
        type_id_source: string(provider, "typeIdSource"),
        type_name: string(provider, "typeName"),
        source_address: string(provider, "sourceAddress"),
    }
}

pub(super) fn network_registration_hook(hook: &Map<String, Value>) -> NetworkRegistrationHook {
    NetworkRegistrationHook {
        type_id: uuid(hook, "typeId"),
        type_name: string(hook, "typeName"),
        slot_type_name: string(hook, "slotTypeName"),
        hook_function: string(hook, "hookFunction"),
        helper_table: string(hook, "helperTable"),
        register_thunk: string(hook, "registerThunk"),
        type_provider: string(hook, "typeProvider"),
        uuid_source: string(hook, "uuidSource"),
    }
}

pub(super) fn registry_entry_name(
    entry: &Map<String, Value>,
    az_rtti: Option<&NetworkAzRtti>,
    registration_hook: Option<&NetworkRegistrationHook>,
) -> Option<String> {
    string(entry, "typeName")
        .or_else(|| string(entry, "name"))
        .or_else(|| string(entry, "registrationTypeName"))
        .or_else(|| registration_hook.and_then(|hook| hook.type_name.clone()))
        .or_else(|| az_rtti.and_then(|rtti| rtti.type_name.clone()))
}

pub(super) fn network_type_capabilities(
    name: Option<&str>,
    has_registered_fields: bool,
    has_replicated_state_abi: bool,
) -> Vec<NetworkTypeCapability> {
    let mut capabilities = Vec::new();
    let is_direct_message = name.is_some_and(is_direct_message_name);
    if (has_replicated_state_abi || name.is_some_and(is_replicated_state_name))
        && !is_direct_message
    {
        capabilities.push(NetworkTypeCapability::ReplicatedState);
    }
    if is_direct_message {
        capabilities.push(NetworkTypeCapability::DirectMessage);
    }
    if has_registered_fields {
        capabilities.push(NetworkTypeCapability::RegisteredFields);
    }
    if capabilities.is_empty() {
        capabilities.push(NetworkTypeCapability::SupportData);
    }
    capabilities
}

pub(super) fn is_replicated_state_name(name: &str) -> bool {
    let leaf = name.rsplit("::").next().unwrap_or(name);
    leaf != "ReplicatedState"
        && (leaf.ends_with("ReplicatedState") || leaf.contains("ReplicatedState<"))
}

pub(super) fn is_direct_message_name(name: &str) -> bool {
    name.contains("ClientMessages::")
        || name.contains("ServerMessages::")
        || name.ends_with("Msg")
        || name.contains("Msg<")
}
