use super::*;

fn replicated_container_schema(plan: serde_json::Value) -> NetworkSchema {
    replicated_container_schema_with_shape(plan, None)
}

fn replicated_container_schema_with_shape(
    plan: serde_json::Value,
    value_type_shape: Option<serde_json::Value>,
) -> NetworkSchema {
    replicated_container_schema_with_named_shape("values", plan, value_type_shape)
}

fn replicated_container_schema_with_named_shape(
    field_name: &str,
    plan: serde_json::Value,
    value_type_shape: Option<serde_json::Value>,
) -> NetworkSchema {
    let mut plan = plan;
    let plan = plan.as_object_mut().expect("container plan object");
    plan.insert(
        "unmarshalReconciliation".to_owned(),
        json!("complete-physical-sequence-agreement"),
    );
    plan.insert(
        "unmarshalAnalysisStatus".to_owned(),
        json!("exact-loop-codec-count"),
    );
    let mut vtable = json!({
        "address": "NewWorld+0x8123456",
        "fieldCount": 1,
        "fullContainerPlan": plan,
        "slots": []
    });
    if let Some(value_type_shape) = value_type_shape {
        vtable
            .as_object_mut()
            .expect("vtable object")
            .insert("valueTypeShape".to_owned(), value_type_shape);
    }
    NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "EA233975-66A9-4A2A-A493-2B6A43674868",
            "typeIndex": 4242,
            "typeName": "MB::PlanBackedReplicatedState",
            "fields": [{
                "index": 0,
                "group": 0,
                "name": field_name,
                "handlerVtable": "NewWorld+0x8123456",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [vtable]
    }))
    .expect("plan-backed schema")
}

#[test]
fn anonymous_container_value_layout_emits_support_struct() {
    let schema = replicated_container_schema_with_shape(
        json!({
            "storageKind": "vector",
            "elementStride": "0x58",
            "keyCodecs": [],
            "valueCodecs": [
                { "wireLayout": "fixed-bytes-16" },
                { "wireShape": "u64" },
                { "wireShape": "u64" },
                { "wireLayout": "fixed-bytes-16" },
                { "wireShape": "u64" }
            ]
        }),
        Some(json!({
            "identityProven": false,
            "layoutProven": true,
            "typeName": "Value",
            "typeNameSource": "synthetic-anonymous-composite",
            "memberNameSource": "synthetic-wire-ordinal",
            "memberNamesProven": false,
            "memberCoverageProven": true,
            "wireOrderProven": true,
            "nativeSize": 88,
            "members": [
                { "index": 0, "offset": "0x10", "name": "field_0", "wireLayout": "fixed-bytes-16", "wireOrdinal": 0 },
                { "index": 1, "offset": "0x20", "name": "field_1", "wireShape": "u64", "wireOrdinal": 1 },
                { "index": 2, "offset": "0x28", "name": "field_2", "wireShape": "u64", "wireOrdinal": 2 },
                { "index": 3, "offset": "0x40", "name": "field_3", "wireLayout": "fixed-bytes-16", "wireOrdinal": 3 },
                { "index": 4, "offset": "0x50", "name": "field_4", "wireShape": "u64", "wireOrdinal": 4 }
            ]
        })),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::std::vec::Vec<ValuesValue>")
    );
    assert!(output.source.contains("pub struct ValuesValue"));
    assert!(output.source.contains("pub field_0: [u8; 16]"));
    assert!(output.source.contains("pub field_4: u64"));
}

#[test]
fn unreflected_rtti_map_value_emits_named_support_struct() {
    let schema = replicated_container_schema_with_named_shape(
        "globalMapData",
        json!({
            "storageKind": "index-map",
            "keyCodecs": [{ "nativeType": "u64", "wireShape": "u64" }],
            "valueCodecs": [{
                "memberSemantics": "linear-sequence",
                "elementOffset": "0x18",
                "members": [
                    { "nativeType": "AZ::Vector2", "wireShape": "vec2", "elementOffset": "0x20" },
                    { "nativeType": "AZ::u16", "wireShape": "u16", "elementOffset": "0x28" },
                    { "nativeType": "AZ::u32", "wireShape": "u32", "elementOffset": "0x2c" },
                    { "nativeType": "bool", "wireShape": "bool", "elementOffset": "0x30" }
                ]
            }]
        }),
        Some(json!({
            "typeId": "0DC02DD0-993E-48C0-8B60-5715D4383B0D",
            "typeIdSource": "unmarshal-full-element-vptr+linked-node-allocation+stack-vptr-agreement",
            "identityProven": true,
            "identitySource": "unmarshal-full-element-vptr+linked-node-allocation+stack-vptr-agreement+complete-container-plan-layout",
            "typeName": "GlobalMapData",
            "typeNameFull": "GlobalMapData",
            "typeNameSource": "az-rtti-vtable-provider",
            "memberNameSource": "synthetic-wire-ordinal",
            "memberNamesProven": false,
            "memberCoverageProven": true,
            "wireOrderProven": true,
            "nativeSize": "0x20",
            "nativeSizeSource": "linked-node-allocation-size-minus-value-offset",
            "members": [
                { "index": 0, "offset": "0x8", "name": "field_0", "nativeType": "AZ::Vector2", "wireShape": "vec2", "wireOrdinal": 0 },
                { "index": 1, "offset": "0x10", "name": "field_1", "nativeType": "AZ::u16", "wireShape": "u16", "wireOrdinal": 1 },
                { "index": 2, "offset": "0x14", "name": "field_2", "nativeType": "AZ::u32", "wireShape": "u32", "wireOrdinal": 2 },
                { "index": 3, "offset": "0x18", "name": "field_3", "nativeType": "bool", "wireShape": "bool", "wireOrdinal": 3 }
            ]
        })),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<u64, GlobalMapData>")
    );
    assert!(output.source.contains("pub struct GlobalMapData"));
    assert!(output.source.contains("pub field_0: ::glam::Vec2"));
    assert!(output.source.contains("pub field_1: u16"));
    assert!(output.source.contains("pub field_2: u32"));
    assert!(output.source.contains("pub field_3: bool"));
    assert!(
        output
            .source
            .contains("impl ::nw_network::serialize::Marshal for GlobalMapData")
    );
    assert!(
        output
            .source
            .contains("impl ::nw_network::serialize::Unmarshal for GlobalMapData")
    );
}

#[test]
fn linear_container_plan_emits_ordered_support_struct() {
    let schema = replicated_container_schema(json!({
        "storageKind": "index-map",
        "keyCodecs": [{ "nativeType": "u32", "wireShape": "u32" }],
        "valueCodecs": [{
            "memberSemantics": "linear-sequence",
            "members": [
                { "wireShape": "u32" },
                { "wireShape": "u32" },
                { "wireShape": "u32" }
            ]
        }]
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<u32, ValuesValue>")
    );
    assert!(output.source.contains("pub struct ValuesValue"));
    assert!(output.source.contains("pub field_0: u32"));
    assert!(output.source.contains("pub field_1: u32"));
    assert!(output.source.contains("pub field_2: u32"));
}

#[test]
fn vector_container_plan_preserves_value_marshaler() {
    let schema = replicated_container_schema(json!({
        "storageKind": "vector",
        "elementStride": "0x8",
        "keyCodecs": [],
        "valueCodecs": [{ "nativeType": "u64", "wireShape": "vlq-u64" }]
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let field = &output.report.state_generation_plans[0].fields[0];

    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("::std::vec::Vec<u64>")
    );
    assert!(
        field
            .rust_field_type
            .as_deref()
            .unwrap()
            .contains("VlqU64Marshaler")
    );
}

#[test]
fn handler_container_type_emits_semantic_map_without_loop_plan() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
            "typeIndex": 3362,
            "typeName": "MB::SlayerScriptReplicatedState",
            "fields": [{
                "index": 0,
                "group": 0,
                "name": "spawnedEntityIdsBySpawnerId",
                "handlerVtable": "NewWorld+0x81bf3d0",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81bf3d0",
            "fieldCount": 1,
            "handlerContainerType": {
                "storageKind": "index-map",
                "keyNativeType": "AZ::Crc32",
                "valueNativeType": "AZ::EntityId",
                "storageNativeType": "AZStd::unordered_map<AZ::Crc32, AZ::EntityId>",
                "keyMarshalerType": "Amazon::Pervasives::Marshaller<AZ::Crc32>",
                "valueMarshalerType": "Amazon::Pervasives::Marshaller<AZ::EntityId>",
                "source": "handler-constructor-template"
            },
            "wireShape": "replicated-container<u32,u64>",
            "wireShapeSource": "replicated-container-delta-key-full-key-value",
            "slots": []
        }]
    }))
    .expect("handler-backed container schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3362]).unwrap();
    let field = &output.report.state_generation_plans[0].fields[0];

    assert!(output.report.state_generation_plans[0].can_generate);
    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>")
    );
    assert_eq!(
        field.rust_field_type.as_deref(),
        Some(
            "::nw_network::serialize::ReplicatedContainer<::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>, { ::nw_network::serialize::WIRE_VEC_CAP }, ::nw_network::serialize::DefaultMarshaler<::nw_network::Crc32>, ::nw_network::serialize::DefaultMarshaler<::nw_network::EntityId>>"
        )
    );
}

#[test]
fn non_linear_container_helper_is_an_explicit_blocker() {
    let schema = replicated_container_schema(json!({
        "storageKind": "index-map",
        "keyCodecs": [{ "wireShape": "u32" }],
        "valueCodecs": [{
            "memberSemantics": "cfg-reachable",
            "members": [{ "wireShape": "u32" }, { "wireShape": "u64" }]
        }]
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(!plan.can_generate);
    assert_eq!(plan.blocked_reasons, ["non-linear-container-codec:1"]);
    assert_eq!(
        plan.fields[0].blocked_reason.as_deref(),
        Some("non-linear-container-codec")
    );
}

#[test]
fn externally_gated_suffix_uses_the_registered_default_profile() {
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
    let schema = replicated_container_schema(json!({
        "storageKind": "vector",
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
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::std::vec::Vec<ValuesValue>")
    );
    assert!(output.source.contains("pub field_0: u64"));
    assert!(output.source.contains("pub field_1: u32"));
}

#[test]
fn raw_container_key_layout_emits_an_opaque_fixed_width_key() {
    let schema = replicated_container_schema(json!({
        "storageKind": "index-map",
        "keyCodecs": [{ "wireLayout": "fixed-bytes-16" }],
        "valueCodecs": [{ "nativeType": "u8", "wireShape": "u8" }]
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<[u8; 16], u8>")
    );
}

#[test]
fn proven_uid_container_key_uses_the_resolved_generic_type() {
    let uid_type_id = uuid!("3485f20a-98c0-5315-876b-21bcd23a7bc0");
    let mut schema = replicated_container_schema(json!({
        "storageKind": "index-map",
        "keyCodecs": [{
            "nativeType": "Amazon::Pervasives::UID",
            "typeId": uid_type_id.to_string(),
            "typeIdSource": "constructor-receiver+owner-rtti+serialize-field-offset",
            "typeIdentityProven": true,
            "wireLayout": "fixed-bytes-16"
        }],
        "valueCodecs": [{ "nativeType": "u32", "wireShape": "u32" }]
    }));
    schema.serialize_types.push(NetworkSerializeType {
        type_id: uid_type_id,
        kind: NetworkSerializeKind::Struct,
        name: "Amazon::Pervasives::UID<AZ::u16>".to_owned(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: Some(ResolvedType::Uid {
            type_id: Some(uid_type_id),
        }),
        emits_source: false,
        factory: None,
        field_count: 0,
        fields: Vec::new(),
        variant_count: 0,
        direct_dependency_type_ids: Vec::new(),
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    });

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<::nw_network::source::AzUuid, u32>")
    );
}

#[test]
fn canonical_actor_ref_identity_uses_the_runtime_wire_type() {
    let actor_ref_type_id = uuid!("0638e28c-ab7b-4ba4-84ac-0353038e6fdc");
    let actor_ref = NetworkSerializeType {
        type_id: actor_ref_type_id,
        kind: NetworkSerializeKind::Struct,
        name: "Amazon::Hub::ActorRef".to_owned(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: None,
        emits_source: true,
        factory: None,
        field_count: 0,
        fields: Vec::new(),
        variant_count: 0,
        direct_dependency_type_ids: Vec::new(),
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    };
    let serialize_types = BTreeMap::from([(actor_ref_type_id, &actor_ref)]);

    assert_eq!(
        network_serialize_type_rust_type(&actor_ref, &serialize_types).as_deref(),
        Some("::nw_network::ActorRef")
    );
}

#[test]
fn reflected_identity_without_struct_fields_uses_the_network_projection() {
    let type_id = uuid!("1be36174-fd4f-4a1c-8e52-7c28d50eec5a");
    let shape = serde_json::from_value::<crate::network_schema::NetworkNestedTypeShape>(json!({
        "typeId": type_id,
        "identityProven": true,
        "typeName": "PersistentItemData",
        "typeNameFull": "PersistentItemData",
        "layoutProven": true,
        "memberCoverageProven": true,
        "wireOrderProven": true,
        "members": [{
            "index": 0,
            "name": "field_0",
            "nameProven": false,
            "wireShape": "u64",
            "wireOrdinal": 0,
            "typeIdentityProven": false,
            "typeConflict": false
        }]
    }))
    .expect("nested type shape");
    let source_type = NetworkSerializeType {
        type_id,
        kind: NetworkSerializeKind::Struct,
        name: "PersistentItemData".to_owned(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: None,
        emits_source: true,
        factory: None,
        field_count: 0,
        fields: Vec::new(),
        variant_count: 0,
        direct_dependency_type_ids: Vec::new(),
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    };
    let source_types = std::collections::BTreeMap::from([(type_id, &source_type)]);

    assert!(!container_value_shape_uses_source_type(
        &shape,
        &source_types
    ));
}
