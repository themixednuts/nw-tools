use super::*;

fn fragment_message_signatures() -> Vec<NetworkMessageSignature> {
    vec![
        NetworkMessageSignature {
            type_id: Some(uuid!("96a58e69-7bd5-45c5-86e4-daf9f5eb1e86")),
            type_index: Some(397),
            name: Some("Replicate::RegisterFragmentAccessMsg".to_owned()),
            rust_name: Some("RegisterFragmentAccessMsg".to_owned()),
            source: None,
            fields: fragment_access_fields(),
        },
        NetworkMessageSignature {
            type_id: Some(uuid!("2b7640e0-4204-4e52-998a-c2db02e0a480")),
            type_index: Some(399),
            name: Some("Replicate::UnregisterFragmentAccessMsg".to_owned()),
            rust_name: Some("UnregisterFragmentAccessMsg".to_owned()),
            source: None,
            fields: fragment_access_fields(),
        },
        NetworkMessageSignature {
            type_id: Some(uuid!("951ef3ed-c9a0-4e3d-a6fd-7fe0673d28d2")),
            type_index: Some(422),
            name: Some("ReplicateClient::FragmentUpdateMsg".to_owned()),
            rust_name: Some("FragmentUpdateMsg".to_owned()),
            source: None,
            fields: vec![
                message_signature_field(0, "TargetRef", "ActorRef", "::nw_network::ActorRef"),
                message_signature_field(1, "Key", "FragmentKey", "::nw_network::hub::FragmentKey"),
                message_signature_field(
                    2,
                    "Fragment",
                    "Amazon::Hub::BaselineableFragment",
                    "::nw_network::hub::BaselineableFragment",
                ),
            ],
        },
    ]
}

fn fragment_access_fields() -> Vec<NetworkMessageFieldSignature> {
    vec![
        message_signature_field(0, "ProxyRef", "ActorRef", "::nw_network::ActorRef"),
        message_signature_field(1, "Key", "FragmentKey", "::nw_network::hub::FragmentKey"),
    ]
}

fn message_signature_field(
    index: u32,
    name: &str,
    native_type: &str,
    rust_type: &str,
) -> NetworkMessageFieldSignature {
    NetworkMessageFieldSignature {
        index: Some(index),
        name: name.to_owned(),
        rust_type: Some(rust_type.to_owned()),
        native_type: Some(native_type.to_owned()),
        wire_shape: None,
    }
}

#[test]
fn reports_message_blocker_summary_with_examples() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "11111111-1111-1111-1111-111111111111",
            "typeIndex": 1,
            "typeName": "Example::EmptyMsg",
            "capabilities": ["direct-message"],
            "fields": []
        }, {
            "uuid": "22222222-2222-2222-2222-222222222222",
            "typeIndex": 2,
            "typeName": "Example::PlaceholderMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "ActorRef",
                "nativeType": "Amazon::Hub::ActorRef",
                "wireShape": "actor-ref",
                "confidence": "message-unmarshal-helper-wrapper"
            }]
        }, {
            "uuid": "33333333-3333-3333-3333-333333333333",
            "typeIndex": 3,
            "typeName": "Example::ReadyMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "Value",
                "nativeType": "u32",
                "wireShape": "u32",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let summary = &output.report.message_blocker_summary;

    assert_eq!(summary.total_plan_count, 3);
    assert_eq!(summary.generatable_count, 3);
    assert_eq!(summary.blocked_count, 0);
    assert!(summary.reason_buckets.is_empty());
    assert!(summary.combination_buckets.is_empty());
    assert!(output.source.contains("pub struct EmptyMsg"));
    let placeholder_plan = output
        .report
        .message_generation_plans
        .iter()
        .find(|plan| plan.type_name.as_deref() == Some("Example::PlaceholderMsg"))
        .expect("placeholder message plan");
    assert_eq!(placeholder_plan.placeholder_field_name_count, 1);
    assert!(placeholder_plan.can_generate);
    assert_eq!(
        placeholder_plan.fields[0].field_name.as_deref(),
        Some("ActorRef")
    );
}

#[test]
fn native_type_names_do_not_infer_message_wire_shapes() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 6179,
            "typeName": "Aoi::PhysicsTrait::ResizeAoiObserverMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "Observer",
                "nativeType": "EntityRef",
                "confidence": "message-unmarshal-call"
            }, {
                "index": 1,
                "name": "Elapsed",
                "nativeType": "f32",
                "confidence": "message-unmarshal-call"
            }, {
                "index": 2,
                "name": "Extents",
                "nativeType": "AZ::Vector2",
                "confidence": "message-unmarshal-call"
            }, {
                "index": 3,
                "name": "Bounds",
                "nativeType": "AZ::Bounds",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let descriptor_output =
        NetworkRustEmitter::emit_descriptors(&schema).expect("descriptor source");

    assert_eq!(descriptor_output.report.field_wire_shape_count, 0);

    let message_output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(message_output.report.message_generation_plan_count, 1);
    assert_eq!(message_output.report.generatable_message_count, 0);
    assert_eq!(message_output.report.blocked_message_count, 1);
    let plan = &message_output.report.message_generation_plans[0];
    assert_eq!(plan.missing_wire_shape_count, 4);
    assert!(plan.fields.iter().all(|field| field.wire_shape.is_none()));
}

#[test]
fn time_point_names_do_not_force_u64_wire_shapes() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 6179,
            "typeName": "Example::TimerMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "StartedAt",
                "nativeType": "MB::TimePoint",
                "confidence": "message-unmarshal-call"
            }, {
                "index": 1,
                "name": "WallClock",
                "nativeType": "MB::WallClockTimePoint",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 0);
    assert_eq!(output.report.blocked_message_count, 1);
    let plan = &output.report.message_generation_plans[0];
    assert_eq!(plan.missing_wire_shape_count, 2);
    assert!(plan.fields.iter().all(|field| field.wire_shape.is_none()));
}

#[test]
fn emits_message_structs_with_native_type_field_names() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "77D6477C-F057-4098-A644-58D36C551989",
            "typeIndex": 1444,
            "typeName": "Aoi::PhysicsTrait::ResizeAoiObservableMsg",
            "fields": [{
                "index": 0,
                "name": "f32",
                "nativeType": "f32",
                "wireShape": "f32",
                "confidence": "message-unmarshal-call"
            }]
        }, {
            "uuid": "1E93F466-CD84-4502-BA28-4632F80DD0FA",
            "typeIndex": 780,
            "typeName": "Amazon::Hub::ScaleTestTrait::SetTargetsMsg",
            "fields": [{
                "index": 0,
                "name": "ActorRef",
                "nativeType": "Amazon::Hub::ActorRef",
                "wireShape": "actor-ref",
                "confidence": "message-unmarshal-helper-wrapper"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.message_generation_plan_count, 2);
    assert_eq!(output.report.generatable_message_count, 2);
    assert_eq!(output.report.blocked_message_count, 0);
    for plan in &output.report.message_generation_plans {
        assert_eq!(plan.placeholder_field_name_count, 1);
        assert!(plan.blocked_reasons.is_empty());
    }
    assert!(output.source.contains("pub struct ResizeAoiObservableMsg"));
    assert!(output.source.contains("pub struct SetTargetsMsg"));
}

#[test]
fn emits_message_structs_with_placeholder_field_names() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "6A379FB8-0BDD-43A1-AB3E-9843D7BE8CD3",
            "typeIndex": 349,
            "typeName": "REPClient::PingMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "u64",
                "wireShape": "u64",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.message_generation_plan_count, 1);
    assert_eq!(output.report.generatable_message_count, 1);
    assert_eq!(output.report.blocked_message_count, 0);
    assert_eq!(
        output.report.message_generation_plans[0].placeholder_field_name_count,
        1
    );
    assert!(
        output.report.message_generation_plans[0]
            .blocked_reasons
            .is_empty()
    );
    assert!(output.source.contains("pub struct PingMsg"));
    assert!(output.source.contains("pub field_0: u64"));
}

#[test]
fn emits_message_fields_from_explicit_rust_types_without_wire_shapes() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 19,
            "typeName": "RegistrationRequestV3Msg",
            "fields": [{
                "index": 0,
                "name": "LoginToken",
                "nativeType": "LoginToken",
                "rustType": "::nw_network::LoginToken",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.message_generation_plan_count, 1);
    assert_eq!(output.report.generatable_message_count, 1);
    assert_eq!(output.report.blocked_message_count, 0);
    let plan = &output.report.message_generation_plans[0];
    assert_eq!(plan.missing_wire_shape_count, 1);
    assert_eq!(plan.missing_field_type_count, 0);
    assert_eq!(plan.supported_field_count, 1);
    assert!(
        output
            .source
            .contains("pub login_token: ::nw_network::LoginToken")
    );
}

#[test]
fn unmarshal_type_names_do_not_resolve_external_support_types() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "57735773-5773-4773-9773-577357735773",
                "typeIndex": 5773,
                "typeName": "Javelin::ClientMessages::InventoriesComponentServerFacet_UpdateItemBatch",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestId",
                    "nativeType": "ActorRequestId",
                    "sourceTypeName": "ActorRequestId",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x35f48ef",
                        "targetName": "Javelin::ClientMessages::ActorRequestId::Unmarshal",
                        "targetKind": "direct-unmarshal",
                        "evidenceSource": "message-unmarshal-pcode-call"
                    },
                    "nestedTypeShape": {
                        "typeName": "ActorRequestId",
                        "typeNameFull": "Javelin::ClientMessages::ActorRequestId",
                        "typeNameSource": "ghidra-symbol",
                        "function": "NewWorld+0x35f4000",
                        "functionName": "Javelin::ClientMessages::ActorRequestId::Unmarshal",
                        "memberBase": "param_1",
                        "memberNameSource": "synthetic-offset",
                        "memberNamesProven": false,
                        "validation": "layout-consistent-two-u64",
                        "members": [{
                            "index": 0,
                            "offset": "0x0",
                            "name": "_0",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8,
                            "evidenceSource": "pcode-call"
                        }, {
                            "index": 1,
                            "offset": "0x8",
                            "name": "_1",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8,
                            "evidenceSource": "pcode-call"
                        }]
                    },
                    "confidence": "message-unmarshal-pcode-call"
                }]
            }, {
                "uuid": "57745774-5774-4774-9774-577457745774",
                "typeIndex": 5774,
                "typeName": "Javelin::ClientMessages::InventoriesComponentServerFacet_UpdateItemBatchWithoutEvidence",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestId",
                    "nativeType": "ActorRequestId",
                    "sourceTypeName": "ActorRequestId",
                    "confidence": "message-unmarshal-whole-helper-direct-type"
                }]
            }, {
                "uuid": "34773477-3477-4477-9477-347734773477",
                "typeIndex": 3477,
                "typeName": "GroupsComponentClientFacet_OnGroupFinderAddMemberSuccessMsg",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestIdPayload",
                    "nativeType": "composite",
                    "sourceTypeName": "ActorRequestIdPayload,ActorRequestId",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x340e9b1",
                        "targetName": "Javelin::ClientMessages::ActorRequestIdPayload::Unmarshal",
                        "targetKind": "direct-unmarshal",
                        "evidenceSource": "message-unmarshal-pcode-call"
                    },
                    "nestedTypeShape": {
                        "typeName": "ActorRequestId",
                        "typeNameFull": "Javelin::ClientMessages::ActorRequestId",
                        "typeNameSource": "ghidra-symbol",
                        "functionName": "Javelin::ClientMessages::ActorRequestId::Unmarshal",
                        "memberNamesProven": false,
                        "validation": "layout-consistent-two-u64",
                        "members": [{
                            "index": 0,
                            "offset": "0x0",
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8,
                            "nameProven": true
                        }, {
                            "index": 1,
                            "offset": "0x8",
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8,
                            "nameProven": false
                        }]
                    },
                    "confidence": "message-unmarshal-pcode-call"
                }]
            }, {
                "uuid": "34783478-3478-4478-9478-347834783478",
                "typeIndex": 3478,
                "typeName": "GroupsComponentClientFacet_OnGroupFinderClearMemberSuccessMsg",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestIdPayload",
                    "nativeType": "ActorRequestIdPayload",
                    "sourceTypeName": "ActorRequestIdPayload",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x340e9e1",
                        "targetName": "Javelin::ClientMessages::ActorRequestIdPayload::Unmarshal",
                        "targetKind": "direct-unmarshal",
                        "evidenceSource": "message-unmarshal-pcode-call"
                    },
                    "confidence": "message-unmarshal-pcode-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.message_generation_plan_count, 4);
    assert_eq!(output.report.generatable_message_count, 1);
    assert_eq!(output.report.blocked_message_count, 3);

    let resolved_plan = output
        .report
        .message_generation_plans
        .iter()
        .find(|plan| plan.type_index == Some(5773))
        .expect("resolved support type plan");
    assert_eq!(resolved_plan.missing_support_type_count, 0);
    assert!(resolved_plan.blocked_reasons.is_empty());
    assert_eq!(
        resolved_plan.fields[0].rust_value_type.as_deref(),
        Some("ActorRequestId")
    );
    assert_eq!(resolved_plan.fields[0].blocked_reason, None);
    assert!(
        output
            .source
            .contains("pub actor_request_id: ActorRequestId")
    );

    let unresolved_plan = output
        .report
        .message_generation_plans
        .iter()
        .find(|plan| plan.type_index == Some(5774))
        .expect("support type plan");
    assert_eq!(unresolved_plan.missing_support_type_count, 1);
    assert_eq!(unresolved_plan.missing_composite_support_type_count, 0);
    assert_eq!(
        unresolved_plan.blocked_reasons,
        vec!["missing-support-type:1"]
    );
    assert_eq!(
        unresolved_plan.fields[0].source_type_name.as_deref(),
        Some("ActorRequestId")
    );
    assert_eq!(
        unresolved_plan.fields[0].blocked_reason.as_deref(),
        Some("missing-support-type")
    );

    let composite_plan = output
        .report
        .message_generation_plans
        .iter()
        .find(|plan| plan.type_index == Some(3477))
        .expect("composite type plan");
    assert_eq!(composite_plan.missing_support_type_count, 0);
    assert_eq!(composite_plan.missing_composite_support_type_count, 1);
    assert_eq!(
        composite_plan.blocked_reasons,
        vec!["missing-composite-support-type:1"]
    );
    assert_eq!(
        composite_plan.fields[0].source_type_name.as_deref(),
        Some("ActorRequestIdPayload,ActorRequestId")
    );
    assert_eq!(composite_plan.fields[0].rust_value_type, None);
    assert_eq!(
        composite_plan.fields[0].blocked_reason.as_deref(),
        Some("missing-composite-support-type")
    );

    let direct_payload_plan = output
        .report
        .message_generation_plans
        .iter()
        .find(|plan| plan.type_index == Some(3478))
        .expect("direct payload plan");
    assert_eq!(direct_payload_plan.missing_support_type_count, 1);
    assert_eq!(direct_payload_plan.missing_composite_support_type_count, 0);
    assert_eq!(
        direct_payload_plan.blocked_reasons,
        vec!["missing-support-type:1"]
    );
    assert_eq!(direct_payload_plan.fields[0].rust_value_type, None);

    let support_bucket = output
        .report
        .message_blocker_summary
        .reason_buckets
        .iter()
        .find(|bucket| bucket.reason == "missing-support-type")
        .expect("support blocker bucket");
    assert_eq!(
        support_bucket.examples[0].blocked_fields[0]
            .source_type_name
            .as_deref(),
        Some("ActorRequestId")
    );
    assert_eq!(
        support_bucket.examples[0].blocked_fields[0]
            .native_type
            .as_deref(),
        Some("ActorRequestId")
    );
}

#[test]
fn emits_message_support_structs_from_proven_nested_shapes() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "49334933-4933-4933-8933-493349334933",
                "typeIndex": 4933,
                "typeName": "Javelin::ClientMessages::RewardTrackComponentServerFacet_DebugRefreshRewards",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestIdBoolPayload",
                    "nativeType": "ActorRequestIdBoolPayload",
                    "sourceTypeName": "ActorRequestIdBoolPayload,ActorRequestId",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x37021b5",
                        "targetName": "Javelin::ClientMessages::ActorRequestIdBoolPayload::Unmarshal",
                        "targetKind": "direct-unmarshal",
                        "evidenceSource": "message-unmarshal-pcode-call"
                    },
                    "nestedTypeShape": {
                        "typeName": "ActorRequestIdBoolPayload",
                        "typeNameFull": "Javelin::ClientMessages::ActorRequestIdBoolPayload",
                        "typeNameSource": "ghidra-symbol",
                        "function": "NewWorld+0x25a2110",
                        "functionName": "Javelin::ClientMessages::ActorRequestIdBoolPayload::Unmarshal",
                        "memberBase": "param_1",
                        "memberNameSource": "synthetic-offset",
                        "memberNamesProven": false,
                        "validation": "layout-consistent-direct-type",
                        "members": [{
                            "index": 0,
                            "offset": "0x0",
                            "name": "_0",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8
                        }, {
                            "index": 1,
                            "offset": "0x8",
                            "name": "_1",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8
                        }, {
                            "index": 2,
                            "offset": "0x20",
                            "name": "value",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "bool",
                            "wireShape": "bool",
                            "byteWidth": 1
                        }]
                    },
                    "confidence": "message-unmarshal-pcode-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 1);
    assert_eq!(output.report.blocked_message_count, 0);
    let plan = &output.report.message_generation_plans[0];
    assert_eq!(plan.fields[0].blocked_reason, None);
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("ActorRequestIdBoolPayload")
    );
    assert!(
        output
            .source
            .contains("pub struct ActorRequestIdBoolPayload")
    );
    assert!(
        output
            .source
            .contains("pub actor_request_id_bool_payload: ActorRequestIdBoolPayload")
    );
    assert!(
        output
            .source
            .contains("impl ::nw_network::serialize::Marshaler for ActorRequestIdBoolPayload")
    );
}

#[test]
fn layout_only_message_field_uses_anonymous_wire_tuple_without_reusing_source_type() {
    let source_type_id = uuid!("6328304a-b754-4ad3-bf78-87236958b55b");
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "49334933-4933-4933-8933-493349334934",
            "typeIndex": 6179,
            "typeName": "Aoi::PhysicsTrait::ResizeAoiObserverMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "interestRef",
                "storageOffset": "0x8",
                "wireShape": "composite<fixed-bytes-16,u64,u64>",
                "nestedTypeShape": {
                    "typeId": source_type_id.to_string(),
                    "typeIdSource": "layout-only-correlation",
                    "typeName": "RemoteTypelessServerFacetRef",
                    "typeNameFull": "RemoteTypelessServerFacetRef",
                    "typeNameSource": "layout-only-correlation",
                    "nativeSize": 48,
                    "nativeSizeSource": "ghidra-pcode-output-storage-span",
                    "memberNameSource": "layout-only-correlation",
                    "memberNamesProven": true,
                    "validation": "layout-consistent-direct-type+exact-native-wire-layout",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x10",
                        "name": "m_actorId",
                        "wireShape": "fixed-bytes-16",
                        "byteWidth": 16
                    }, {
                        "index": 1,
                        "offset": "0x10",
                        "nativeOffset": "0x20",
                        "name": "id",
                        "wireShape": "u64",
                        "byteWidth": 8
                    }, {
                        "index": 2,
                        "offset": "0x18",
                        "nativeOffset": "0x28",
                        "name": "m_targetId",
                        "wireShape": "u64",
                        "byteWidth": 8
                    }]
                },
                "confidence": "message-unmarshal-helper-argument"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let plan = &output.report.message_generation_plans[0];

    assert!(!plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("([u8; 16], u64, u64)")
    );
    assert_eq!(plan.fields[0].blocked_reason, None);
    assert!(
        plan.blocked_reasons
            .contains(&"invalid-evidence:1".to_owned())
    );
}

#[test]
fn native_fixed_vector_names_do_not_select_arrayvec() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "18141814-1814-4814-9814-181418141814",
                "typeIndex": 1814,
                "typeName": "Javelin::ClientMessages::ObjectivesComponentServerFacet_AddObjectiveFromRecipe",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "field_1",
                    "nativeType": "AZStd::fixed_vector<AZ::u8,64>",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x39e937d",
                        "targetName": "GridMate::Marshaler<AZStd::fixed_vector<AZ::u8,64>>::Unmarshal",
                        "targetKind": "whole-helper-marshaler",
                        "evidenceSource": "message-unmarshal-whole-helper-marshaler"
                    },
                    "confidence": "message-unmarshal-whole-helper-marshaler"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 0);
    assert_eq!(output.report.blocked_message_count, 1);
    let plan = &output.report.message_generation_plans[0];
    assert_eq!(
        plan.fields[0].blocked_reason.as_deref(),
        Some("missing-support-type")
    );
    assert_eq!(plan.fields[0].rust_value_type, None);
}

#[test]
fn emits_actor_ref_for_proven_actor_ref_message_fields() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "96A58E69-7BD5-45C5-86E4-DAF9F5EB1E86",
            "typeIndex": 397,
            "typeName": "Replicate::RegisterFragmentAccessMsg",
            "fields": [{
                "index": 0,
                "name": "ProxyRef",
                "nativeType": "Amazon::Hub::ActorRef",
                "wireShape": "actor-ref",
                "confidence": "message-unmarshal-helper-direct-type-call"
            }, {
                "index": 1,
                "name": "Key",
                "nativeType": "FragmentKey",
                "rustType": "::nw_network::hub::FragmentKey",
                "confidence": "message-signature-source"
            }]
        }, {
            "uuid": "17117117-1711-4711-9711-171171171171",
            "typeIndex": 171,
            "typeName": "ConfigOverridesDebugTrait::SendConfigOverridesMsg",
            "fields": [{
                "index": 0,
                "name": "ProxyAddress",
                "nativeType": "Amazon::Hub::ActorRef",
                "wireShape": "actor-ref",
                "confidence": "message-unmarshal-pcode-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 2);
    assert_eq!(output.report.blocked_message_count, 0);
    assert!(
        output
            .source
            .contains("pub proxy_ref: ::nw_network::ActorRef")
    );
    assert!(
        output
            .source
            .contains("pub field_0: ::nw_network::ActorRef")
    );
    assert!(
        output
            .source
            .contains("pub key: ::nw_network::hub::FragmentKey")
    );
}

#[test]
fn baselineable_fragment_names_do_not_select_protocol_types() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "951EF3ED-C9A0-4E3D-A6FD-7FE0673D28D2",
            "typeIndex": 422,
            "typeName": "ReplicateClient::FragmentUpdateMsg",
            "fields": [{
                "index": 0,
                "name": "Fragment",
                "nativeType": "Amazon::Hub::BaselineableFragment",
                "confidence": "message-unmarshal-helper-direct-type-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 0);
    assert_eq!(output.report.blocked_message_count, 1);
    assert_eq!(
        output.report.message_generation_plans[0].fields[0]
            .blocked_reason
            .as_deref(),
        Some("missing-support-type")
    );
}

#[test]
fn emits_fragment_messages_from_source_signature_merge() {
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "96A58E69-7BD5-45C5-86E4-DAF9F5EB1E86",
            "typeIndex": 397,
            "typeName": "Replicate::RegisterFragmentAccessMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "ActorRef",
                "storageExpression": "param_3 + 1",
                "confidence": "message-unmarshal-helper-wrapper"
            }, {
                "index": 1,
                "name": "field_1",
                "nativeType": "FragmentKey",
                "storageExpression": "param_3 + 0x19",
                "confidence": "message-unmarshal-helper-wrapper"
            }]
        }, {
            "uuid": "2B7640E0-4204-4E52-998A-C2DB02E0A480",
            "typeIndex": 399,
            "typeName": "Replicate::UnregisterFragmentAccessMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "ActorRef",
                "storageExpression": "param_3 + 1",
                "confidence": "message-unmarshal-helper-wrapper"
            }, {
                "index": 1,
                "name": "field_1",
                "nativeType": "FragmentKey",
                "storageExpression": "param_3 + 0x19",
                "confidence": "message-unmarshal-helper-wrapper"
            }]
        }, {
            "uuid": "951EF3ED-C9A0-4E3D-A6FD-7FE0673D28D2",
            "typeIndex": 422,
            "typeName": "ReplicateClient::FragmentUpdateMsg",
            "fields": [{
                "index": 0,
                "name": "ProxyAddress",
                "nameSource": "message-native-type-name",
                "nativeType": "ActorRef",
                "confidence": "message-unmarshal-inline-direct-type-call"
            }, {
                "index": 1,
                "name": "field_1",
                "nativeType": "FragmentKey",
                "confidence": "message-unmarshal-inline-call"
            }, {
                "index": 2,
                "name": "field_2",
                "nativeType": "Amazon::Hub::BaselineableFragment",
                "confidence": "message-unmarshal-inline-direct-type-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");
    schema.merge_message_signatures(
        &fragment_message_signatures(),
        Some("message-signatures.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 3);
    assert_eq!(output.report.blocked_message_count, 0);
    assert!(
        output
            .source
            .contains("pub struct RegisterFragmentAccessMsg")
    );
    assert!(
        output
            .source
            .contains("pub struct UnregisterFragmentAccessMsg")
    );
    assert!(output.source.contains("pub struct FragmentUpdateMsg"));
    assert!(
        output
            .source
            .contains("pub proxy_ref: ::nw_network::ActorRef")
    );
    assert!(
        output
            .source
            .contains("pub target_ref: ::nw_network::ActorRef")
    );
    assert!(
        output
            .source
            .contains("pub key: ::nw_network::hub::FragmentKey")
    );
    assert!(
        output
            .source
            .contains("pub fragment: ::nw_network::hub::BaselineableFragment")
    );
}
