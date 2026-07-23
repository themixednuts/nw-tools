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
            "messageUnmarshal": {
                "analysisStatus": "proven-empty",
                "emptyWireProven": true,
                "emptyWireEvidenceSource": "vptr-only-instance+empty-marshal-cfg+successful-no-read-unmarshal-cfg",
                "supportsUnmarshal": true,
                "instanceSize": "0x8"
            },
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
        }, {
            "uuid": "44444444-4444-4444-4444-444444444444",
            "typeIndex": 4,
            "typeName": "Example::UnresolvedMsg",
            "capabilities": ["direct-message"],
            "fields": []
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let summary = &output.report.message_blocker_summary;

    assert_eq!(summary.total_plan_count, 4);
    assert_eq!(summary.generatable_count, 3);
    assert_eq!(summary.blocked_count, 1);
    assert_eq!(
        summary.reason_buckets[0].reason,
        "message-layout-unresolved"
    );
    assert!(output.source.contains("pub struct EmptyMsg"));
    assert!(!output.source.contains("pub struct UnresolvedMsg"));
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
fn emits_vlq_counted_ordered_map_messages() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "4CF9D4AE-3BE1-4E9E-A8F1-A313AB114D00",
            "typeIndex": 191,
            "typeName": "Amazon::Hub::HubIdDebugTrait::ReceiveAllHubNamesMsg",
            "fields": [{
                "index": 0,
                "name": "HubNames",
                "storageOffset": "0x8",
                "wireShape": "map<u32,composite<entity-ref,u32,bool>>",
                "wireLayout": "vec<composite<u32,entity-ref,u32,fixed-bytes-1>>",
                "confidence": "message-unmarshal-pcode-interprocedural-collection"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let plan = &output.report.message_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_field_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<u32, (::nw_network::EntityRef, u32, bool)>")
    );
}

#[test]
fn emits_native_marshal_only_messages_without_a_decoder() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "44BA8334-C3AA-476E-855C-27364BF8A964",
            "typeIndex": 747,
            "typeName": "ActorMover::CheckMovementStatusMsg",
            "messageUnmarshal": {
                "terminalStatus": "no-success-terminal",
                "supportsUnmarshal": false
            },
            "fields": [{
                "index": 0,
                "name": "ActorId",
                "storageExpression": "local_20",
                "wireLayout": "fixed-bytes-16",
                "confidence": "message-unmarshal-pcode-stack-readraw"
            }],
            "messageMarshal": {
                "fields": [{
                    "index": 0,
                    "name": "ActorId",
                    "storageOffset": "0x8",
                    "rustType": "::nw_network::ActorId",
                    "wireLayout": "fixed-bytes-16",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 1,
                    "name": "OriginatingMoveCoordinator",
                    "storageOffset": "0x20",
                    "wireShape": "composite<u32,fixed-bytes-16,fixed-bytes-16>",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 2,
                    "name": "MovementInteractionId",
                    "storageOffset": "0x48",
                    "wireShape": "composite<u32,u32,u64,u64>",
                    "confidence": "message-marshal-pcode-stack"
                }]
            }
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let plan = &output.report.message_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(plan.supports_unmarshal, Some(false));
    assert!(
        output
            .source
            .contains("#[derive(Debug, Clone, PartialEq, Marshal)]")
    );
    assert!(
        !plan
            .blocked_reasons
            .iter()
            .any(|reason| reason == "marshal-unmarshal-field-mismatch")
    );
}

#[test]
fn accepts_source_matched_bidirectional_fields_recovered_through_temporaries() {
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0552ED14-3F76-47CC-844C-E1C1150766C0",
            "typeIndex": 1956,
            "typeName": "MB::ServerContext::PulseMsg",
            "fields": [{
                "index": 0,
                "storageExpression": "local_20",
                "wireShape": "u64",
                "confidence": "message-unmarshal-pcode-stack-call"
            }],
            "messageMarshal": {
                "fields": [{
                    "index": 0,
                    "wireShape": "u64",
                    "confidence": "message-marshal-pcode-stack"
                }]
            }
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");
    schema.merge_message_signatures(
        &[NetworkMessageSignature {
            type_id: Some(uuid!("0552ed14-3f76-47cc-844c-e1c1150766c0")),
            type_index: Some(1956),
            name: Some("MB::ServerContext::PulseMsg".to_owned()),
            rust_name: Some("PulseMsg".to_owned()),
            source: Some("source-signature".to_owned()),
            fields: vec![NetworkMessageFieldSignature {
                index: Some(0),
                name: "CurrentTimePoint".to_owned(),
                rust_type: Some("::nw_network::TimePoint".to_owned()),
                native_type: Some("TimePoint".to_owned()),
                wire_shape: Some(crate::NetworkWireShape::U64),
            }],
        }],
        None,
    );

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let plan = &output.report.message_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert!(plan.evidence_issues.is_empty());
    assert!(output.source.contains("pub struct PulseMsg"));
}

#[test]
fn blocks_messages_with_proven_directional_field_mismatches() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "44BA4CBA-AFAD-4EC5-A9DA-500838B28A58",
            "typeIndex": 748,
            "typeName": "Example::SymmetricMsg",
            "fields": [{
                "index": 0,
                "name": "ActorId",
                "storageOffset": "0x8",
                "wireLayout": "fixed-bytes-16",
                "confidence": "message-unmarshal-pcode-stack-readraw"
            }],
            "messageMarshal": {
                "fields": [{
                    "index": 0,
                    "storageOffset": "0x8",
                    "wireLayout": "fixed-bytes-16",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 1,
                    "storageOffset": "0x18",
                    "wireShape": "u32",
                    "confidence": "message-marshal-pcode-stack"
                }]
            }
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let plan = &output.report.message_generation_plans[0];

    assert_eq!(
        plan.blocked_reasons,
        vec!["marshal-unmarshal-field-mismatch"]
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
fn emits_anonymous_fixed_width_message_fields_as_byte_arrays() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "A5E12A88-7B5C-40EA-A2B0-95A636E90549",
            "typeIndex": 1756,
            "typeName": "Aoi::BaseQueryTrait::QueryAabbMsg",
            "capabilities": ["direct-message"],
            "messageUnmarshal": {
                "supportsUnmarshal": true
            },
            "fields": [{
                "index": 0,
                "name": "field_0",
                "storageExpression": "param_3 + 8",
                "storageBase": "param_3",
                "storageBaseOffset": 8,
                "storageOffset": 8,
                "rawByteLength": 8,
                "wireLayout": "fixed-bytes-8",
                "wireLayoutSource": "message-unmarshal-read-raw",
                "confidence": "message-unmarshal-pcode-stack-readraw"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let plan = output
        .report
        .message_generation_plans
        .first()
        .unwrap_or_else(|| panic!("missing message plan: {:#?}", output.report));

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].wire_shape,
        Some(SchemaWireShape::FixedBytes(8))
    );
    assert_eq!(plan.fields[0].rust_value_type.as_deref(), Some("[u8; 8]"));
    assert!(output.source.contains("pub field_0: [u8; 8]"));
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
fn proven_actor_request_id_shapes_use_the_shared_network_type() {
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
                        "layoutProven": true,
                        "memberCoverageProven": true,
                        "wireOrderProven": true,
                        "wireOrderSource": "cfg-recursive-unmarshal-order+unique-storage-match",
                        "validation": "layout-consistent-two-u64",
                        "members": [{
                            "index": 0,
                            "offset": "0x0",
                            "name": "_0",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "wireLayout": "fixed-bytes-8",
                            "wireOrdinal": 0,
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
                            "wireLayout": "fixed-bytes-8",
                            "wireOrdinal": 1,
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
                        "memberBase": "param_1",
                        "memberNamesProven": false,
                        "layoutProven": true,
                        "memberCoverageProven": true,
                        "wireOrderProven": true,
                        "wireOrderSource": "cfg-dominating-output-storage-order+recursive-unmarshal-order",
                        "validation": "layout-consistent-two-u64+single-direct-unmarshal-delegate",
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
    assert_eq!(output.report.generatable_message_count, 2);
    assert_eq!(output.report.blocked_message_count, 2);

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
        Some("::nw_network::ActorRequestId")
    );
    assert_eq!(resolved_plan.fields[0].blocked_reason, None);
    assert!(
        output
            .source
            .contains("pub actor_request_id: ::nw_network::ActorRequestId")
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
    assert_eq!(composite_plan.missing_composite_support_type_count, 0);
    assert!(composite_plan.blocked_reasons.is_empty());
    assert_eq!(
        composite_plan.fields[0].source_type_name.as_deref(),
        Some("ActorRequestIdPayload,ActorRequestId")
    );
    assert_eq!(
        composite_plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::ActorRequestId")
    );
    assert_eq!(composite_plan.fields[0].blocked_reason, None);
    assert!(!output.source.contains("pub struct ActorRequestId"));

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
                        "layoutProven": true,
                        "memberCoverageProven": true,
                        "wireOrderProven": true,
                        "wireOrderSource": "cfg-recursive-unmarshal-order+unique-storage-match",
                        "validation": "layout-consistent-direct-type",
                        "members": [{
                            "index": 0,
                            "offset": "0x0",
                            "name": "_0",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "wireLayout": "fixed-bytes-8",
                            "wireOrdinal": 0,
                            "byteWidth": 8
                        }, {
                            "index": 1,
                            "offset": "0x8",
                            "name": "_1",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "wireLayout": "fixed-bytes-8",
                            "wireOrdinal": 1,
                            "byteWidth": 8
                        }, {
                            "index": 2,
                            "offset": "0x20",
                            "name": "value",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "bool",
                            "wireShape": "bool",
                            "wireLayout": "fixed-bytes-1",
                            "wireOrdinal": 2,
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
            .contains("impl ::nw_network::serialize::Marshal for ActorRequestIdBoolPayload")
    );
    assert!(
        output
            .source
            .contains("impl ::nw_network::serialize::Unmarshal for ActorRequestIdBoolPayload")
    );
}

#[test]
fn exact_nested_client_ref_uses_the_runtime_wire_type() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "49334933-4933-4933-8933-493349334936",
            "typeIndex": 6180,
            "typeName": "Aoi::ClientRefMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "clientRef",
                "nativeType": "ClientRef",
                "sourceTypeName": "ClientRef",
                "sourceTypeId": "c148c555-3264-41f7-a335-e48b65f91728",
                "sourceTypeIdentityProven": true,
                "wireShape": "actor-ref",
                "nestedTypeShape": {
                    "typeId": "c148c555-3264-41f7-a335-e48b65f91728",
                    "identityProven": true,
                    "typeName": "ClientRef",
                    "typeNameFull": "ClientRef",
                    "memberNamesProven": true,
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "members": [{
                        "index": 0,
                        "offset": "0x8",
                        "name": "m_clientRef",
                        "nameProven": true,
                        "nativeType": "Amazon::Hub::ActorRef",
                        "typeId": "0638e28c-ab7b-4ba4-84ac-0353038e6fdc",
                        "typeIdentityProven": true,
                        "wireShape": "actor-ref",
                        "wireOrdinal": 0
                    }]
                },
                "confidence": "message-unmarshal-constructor-typed-boundary"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let plan = &output.report.message_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::ClientRef")
    );
    assert!(
        output
            .source
            .contains("pub client_ref: ::nw_network::ClientRef")
    );
    assert!(!output.source.contains("::nw_network::source::ClientRef"));
}

#[test]
fn names_constructor_proven_anonymous_message_values_from_their_owner() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "49334933-4933-4933-8933-493349334935",
            "typeIndex": 6178,
            "typeName": "Aoi::AnonymousConstructorMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "field_0",
                "wireShape": "composite<u32,u8>",
                "wireLayout": "composite<fixed-bytes-4,fixed-bytes-1>",
                "nestedTypeShape": {
                    "typeNameSource": "synthetic-constructor-subobject",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "wireOrderSource": "message-cfg-complete-wire-coverage",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "name": "_0",
                        "wireShape": "u32",
                        "wireLayout": "fixed-bytes-4",
                        "wireOrdinal": 0,
                        "byteWidth": 4
                    }, {
                        "index": 1,
                        "offset": "0x4",
                        "name": "_1",
                        "wireShape": "u8",
                        "wireLayout": "fixed-bytes-1",
                        "wireOrdinal": 1,
                        "byteWidth": 1
                    }]
                },
                "confidence": "message-unmarshal-constructor-subobject-boundary"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
    let plan = &output.report.message_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("AnonymousConstructorMsgField0Value")
    );
    assert!(
        output
            .source
            .contains("pub struct AnonymousConstructorMsgField0Value")
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
fn emits_native_unordered_set_with_semantic_element_type() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "C0B051D8-C059-499E-A5E5-597954F2AA5C",
            "typeIndex": 985,
            "typeName": "Amazon::Hub::ResubscribeMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "registryIndex",
                "nativeType": "AZ::u32",
                "wireShape": "u32",
                "confidence": "message-unmarshal-pcode-call"
            }, {
                "index": 1,
                "name": "listeners",
                "nativeType": "AZStd::unordered_set<Amazon::Pervasives::CrcID>",
                "wireShape": "set<fixed-bytes-16>",
                "confidence": "message-unmarshal-cfg-azstd-unordered-set"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 1);
    assert_eq!(output.report.blocked_message_count, 0);
    assert!(
        output
            .source
            .contains("pub listeners: ::nw_network::serialize::IndexSet<::nw_network::CrcId>")
    );
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
