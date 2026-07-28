use super::*;

use crate::ReflectedTypeCatalog;

#[test]
fn filters_implausible_message_unmarshal_storage_and_reindexes_remaining_fields() {
    let report = json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 19,
            "typeName": "RegistrationRequestV3Msg",
            "fields": [{
                "index": 0,
                "name": "param_4",
                "storageExpression": "param_4",
                "confidence": "message-unmarshal-helper-argument"
            }, {
                "index": 5,
                "name": "UseCapabilities",
                "nativeType": "bool",
                "storageExpression": "param_3 + 0x8c",
                "wireShape": "bool",
                "confidence": "message-unmarshal-helper-argument"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

    assert_eq!(schema.types[0].fields.len(), 1);
    assert_eq!(schema.types[0].fields[0].index, Some(0));
    assert_eq!(
        schema.types[0].fields[0].name.as_deref(),
        Some("UseCapabilities")
    );
}

#[test]
fn merges_required_resolved_generic_types_without_declaring_source_items() {
    let uid_type_id = uuid!("3485f20a-98c0-5315-876b-21bcd23a7bc0");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8123456",
            "fieldCount": 1,
            "fullContainerPlan": {
                "storageKind": "index-map",
                "keyCodecs": [{
                    "nativeType": "Amazon::Pervasives::UID",
                    "typeId": uid_type_id.to_string(),
                    "typeIdentityProven": true,
                    "wireLayout": "fixed-bytes-16"
                }],
                "valueCodecs": [{ "wireShape": "u32" }]
            },
            "slots": []
        }]
    }))
    .expect("network schema");
    let catalog = ReflectedTypeCatalog::from_json_roots(
        &json!({
            "$id": 1,
            "uuidMap": {},
            "classNameToUuid": [],
            "uuidGenericMap": [[
                uid_type_id.to_string(),
                {
                    "$id": 20,
                    "typeId": uid_type_id.to_string(),
                    "registeredTypeIds": [uid_type_id.to_string()],
                    "templatedArgumentCount": 1,
                    "templatedTypeIds": ["ECA0B403-C4F8-4B86-95FC-81688D046E40"],
                    "typeIdFoldTypeIds": null,
                    "specializedTypeId": uid_type_id.to_string(),
                    "genericTypeId": null,
                    "legacySpecializedTypeId": null,
                    "nonTypeTemplateArguments": null,
                    "classData": {
                        "$id": 21,
                        "name": "Amazon::Pervasives::UID",
                        "typeId": uid_type_id.to_string(),
                        "version": 0,
                        "doSave": null,
                        "dataConverter": null,
                        "editData": null,
                        "elements": [],
                        "attributes": []
                    },
                    "elements": []
                }
            ]],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }),
        None,
        None,
        &CodegenContext::inline(),
    );

    let report = schema.merge_serialize_type_catalog(&catalog);
    let [serialize] = schema.serialize_types.as_slice() else {
        panic!("expected one resolved generic type")
    };

    assert_eq!(report.required_type_count, 1);
    assert_eq!(report.matched_generic_type_count, 1);
    assert_eq!(
        serialize.resolved_type,
        Some(ResolvedType::Uid {
            type_id: Some(uid_type_id)
        })
    );
    assert!(!serialize.emits_source);
}

#[test]
fn merges_message_signature_field_names_without_overwriting_real_names() {
    let report = json!({
        "registryEntries": [{
            "uuid": "6A379FB8-8E18-4D62-89A1-9A891DC98CAD",
            "typeIndex": 349,
            "typeName": "REPClient::PingMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "storageExpression": "param_3 + 1",
                "wireShape": "u64",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");

    let merge = schema.merge_message_signatures(
        &[NetworkMessageSignature {
            type_id: Some(uuid!("6a379fb8-8e18-4d62-89a1-9a891dc98cad")),
            type_index: Some(349),
            name: Some("REPClient::PingMsg".to_owned()),
            rust_name: Some("PingMsg".to_owned()),
            source: None,
            fields: vec![NetworkMessageFieldSignature {
                index: Some(0),
                name: "epoch_time_send".to_owned(),
                rust_type: Some("u64".to_owned()),
                native_type: Some("u64".to_owned()),
                wire_shape: Some(NetworkWireShape::U64),
            }],
        }],
        Some("rust-source".to_owned()),
    );

    assert_eq!(merge.matched_message_count, 1);
    assert_eq!(merge.field_name_filled_count, 1);
    assert_eq!(merge.field_name_conflict_count, 0);
    assert_eq!(schema.summary.message_source_field_count, 1);
    let field = &schema.types[0].fields[0];
    assert_eq!(field.name.as_deref(), Some("epoch_time_send"));
    assert_eq!(field.native_type.as_deref(), Some("u64"));
    assert_eq!(field.wire_shape, Some(NetworkWireShape::U64));
}

#[test]
fn message_signatures_report_native_type_conflicts_without_overwriting_evidence() {
    let report = json!({
        "registryEntries": [{
            "uuid": "96A58E69-7BD5-45C5-86E4-DAF9F5EB1E86",
            "typeIndex": 397,
            "typeName": "Replicate::RegisterFragmentAccessMsg",
            "fields": [{
                "index": 0,
                "name": "ProxyAddress",
                "nameSource": "message-native-type-name",
                "nativeType": "ProxyAddress",
                "confidence": "message-unmarshal-helper-direct-type-call"
            }, {
                "index": 1,
                "name": "field_1",
                "nativeType": "u32",
                "wireShape": "u32",
                "confidence": "message-unmarshal-helper-nested-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");

    let merge = schema.merge_message_signatures(
        &[NetworkMessageSignature {
            type_id: Some(uuid!("96a58e69-7bd5-45c5-86e4-daf9f5eb1e86")),
            type_index: Some(397),
            name: Some("Replicate::RegisterFragmentAccessMsg".to_owned()),
            rust_name: Some("RegisterFragmentAccessMsg".to_owned()),
            source: None,
            fields: vec![
                NetworkMessageFieldSignature {
                    index: Some(0),
                    name: "ProxyRef".to_owned(),
                    rust_type: None,
                    native_type: Some("ActorRef".to_owned()),
                    wire_shape: None,
                },
                NetworkMessageFieldSignature {
                    index: Some(1),
                    name: "Key".to_owned(),
                    rust_type: None,
                    native_type: Some("FragmentKey".to_owned()),
                    wire_shape: None,
                },
            ],
        }],
        Some("message-signatures.json".to_owned()),
    );

    assert_eq!(merge.matched_message_count, 1);
    assert_eq!(merge.field_name_filled_count, 2);
    assert_eq!(merge.field_name_conflict_count, 0);
    assert_eq!(merge.native_type_conflict_count, 2);
    assert_eq!(schema.types[0].fields[0].name.as_deref(), Some("ProxyRef"));
    assert_eq!(schema.types[0].fields[1].name.as_deref(), Some("Key"));
    assert_eq!(
        schema.types[0].fields[0].native_type.as_deref(),
        Some("ProxyAddress")
    );
    assert_eq!(
        schema.types[0].fields[1].native_type.as_deref(),
        Some("u32")
    );
    assert!(
        schema.types[0]
            .fields
            .iter()
            .all(|field| field.signature_type_conflict)
    );
}

#[test]
fn message_signature_conflicts_are_recomputed_after_schema_round_trip() {
    let report = json!({
        "registryEntries": [{
            "uuid": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
            "typeIndex": 77,
            "typeName": "ExampleMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "u32",
                "wireShape": "u32",
                "confidence": "message-unmarshal-pcode-stack"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");
    let conflicting = NetworkMessageSignature {
        type_id: Some(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")),
        type_index: Some(77),
        name: Some("ExampleMsg".to_owned()),
        rust_name: Some("ExampleMsg".to_owned()),
        source: None,
        fields: vec![NetworkMessageFieldSignature {
            index: Some(0),
            name: "value".to_owned(),
            rust_type: Some("String".to_owned()),
            native_type: Some("AZStd::string".to_owned()),
            wire_shape: Some(NetworkWireShape::String),
        }],
    };
    schema.merge_message_signatures(&[conflicting], Some("first-pass".to_owned()));
    assert!(schema.types[0].fields[0].signature_type_conflict);
    assert!(schema.types[0].fields[0].signature_wire_conflict);

    let encoded = serde_json::to_value(&schema).expect("serialize schema");
    let mut schema: NetworkSchema = serde_json::from_value(encoded).expect("deserialize schema");
    let corrected = NetworkMessageSignature {
        type_id: Some(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")),
        type_index: Some(77),
        name: Some("ExampleMsg".to_owned()),
        rust_name: Some("ExampleMsg".to_owned()),
        source: None,
        fields: vec![NetworkMessageFieldSignature {
            index: Some(0),
            name: "value".to_owned(),
            rust_type: Some("u32".to_owned()),
            native_type: Some("AZ::u32".to_owned()),
            wire_shape: Some(NetworkWireShape::U32),
        }],
    };
    let merge = schema.merge_message_signatures(&[corrected], Some("second-pass".to_owned()));

    assert_eq!(merge.native_type_conflict_count, 0);
    assert_eq!(merge.wire_shape_conflict_count, 0);
    assert!(!schema.types[0].fields[0].signature_type_conflict);
    assert!(!schema.types[0].fields[0].signature_wire_conflict);
}

#[test]
fn message_signature_merge_preserves_extractor_type_conflicts() {
    let report = json!({
        "registryEntries": [{
            "uuid": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
            "typeIndex": 77,
            "typeName": "ExampleMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "u32",
                "wireShape": "u32",
                "typeConflict": true,
                "confidence": "message-unmarshal-pcode-stack"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");
    schema.merge_message_signatures(
        &[NetworkMessageSignature {
            type_id: Some(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")),
            type_index: Some(77),
            name: Some("ExampleMsg".to_owned()),
            rust_name: Some("ExampleMsg".to_owned()),
            source: None,
            fields: vec![NetworkMessageFieldSignature {
                index: Some(0),
                name: "value".to_owned(),
                rust_type: Some("u32".to_owned()),
                native_type: Some("AZ::u32".to_owned()),
                wire_shape: Some(NetworkWireShape::U32),
            }],
        }],
        Some("message-signatures".to_owned()),
    );

    assert!(schema.types[0].fields[0].type_conflict);
    assert!(!schema.types[0].fields[0].signature_type_conflict);
}

#[test]
fn message_signatures_refine_wire_projections_into_semantic_native_types() {
    let report = json!({
        "registryEntries": [{
            "uuid": "059B0DE2-4789-4DC1-945C-3728873D68F2",
            "typeIndex": 5044,
            "typeName": "ActorMover::CrashMoveActorMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "u32",
                "wireShape": "u32",
                "wireShapeSource": "message-unmarshal-pcode-stack-store",
                "confidence": "message-unmarshal-pcode-stack-store"
            }, {
                "index": 1,
                "name": "field_1",
                "nativeType": "u32",
                "wireShape": "u32",
                "wireShapeSource": "unmarshal-codec-specialization+fixed-width-pcode",
                "confidence": "message-unmarshal-pcode-stack"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");

    let merge = schema.merge_message_signatures(
        &[NetworkMessageSignature {
            type_id: Some(uuid!("059b0de2-4789-4dc1-945c-3728873d68f2")),
            type_index: Some(5044),
            name: Some("ActorMover::CrashMoveActorMsg".to_owned()),
            rust_name: Some("CrashMoveActorMsg".to_owned()),
            source: None,
            fields: vec![
                NetworkMessageFieldSignature {
                    index: Some(0),
                    name: "Target".to_owned(),
                    rust_type: Some("::nw_network::CrashTarget".to_owned()),
                    native_type: Some("CrashTarget".to_owned()),
                    wire_shape: Some(NetworkWireShape::U32),
                },
                NetworkMessageFieldSignature {
                    index: Some(1),
                    name: "DurationSecs".to_owned(),
                    rust_type: Some("u32".to_owned()),
                    native_type: Some("AZ::u32".to_owned()),
                    wire_shape: Some(NetworkWireShape::U32),
                },
            ],
        }],
        Some("message-signatures.json".to_owned()),
    );

    assert_eq!(merge.native_type_conflict_count, 0);
    assert_eq!(merge.native_type_filled_count, 1);
    assert_eq!(
        schema.types[0].fields[0].native_type.as_deref(),
        Some("CrashTarget")
    );
    assert_eq!(
        schema.types[0].fields[1].native_type.as_deref(),
        Some("AZ::u32")
    );
    assert!(
        schema.types[0]
            .fields
            .iter()
            .all(|field| !field.signature_type_conflict)
    );
}

#[test]
fn message_signatures_do_not_replace_partial_ghidra_fields() {
    let report = json!({
        "registryEntries": [{
            "uuid": "96A58E69-7BD5-45C5-86E4-DAF9F5EB1E86",
            "typeIndex": 397,
            "typeName": "Replicate::RegisterFragmentAccessMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "u32",
                "storageExpression": "param_3 + 1",
                "wireShape": "u32",
                "confidence": "message-unmarshal-helper-wrapper"
            }]
        }, {
            "uuid": "2B7640E0-4204-4E52-998A-C2DB02E0A480",
            "typeIndex": 399,
            "typeName": "Replicate::UnregisterFragmentAccessMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "u32",
                "storageExpression": "param_3 + 1",
                "wireShape": "u32",
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
                "nativeType": "ProxyAddress",
                "confidence": "message-unmarshal-inline-direct-type-call"
            }, {
                "index": 1,
                "name": "field_1",
                "nativeType": "u32",
                "wireShape": "u32",
                "confidence": "message-unmarshal-inline-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");

    let merge = schema.merge_message_signatures(
        &fragment_access_message_signatures(),
        Some("message-signatures.json".to_owned()),
    );

    assert_eq!(merge.matched_message_count, 3);
    assert_eq!(merge.field_count_mismatch_count, 3);
    assert_eq!(schema.types[0].fields.len(), 1);
    assert_eq!(schema.types[1].fields.len(), 1);
    assert_eq!(schema.types[2].fields.len(), 2);
    assert!(
        schema
            .types
            .iter()
            .all(|network_type| network_type.signature_field_count_conflict)
    );
    assert_eq!(schema.types[0].fields[0].name.as_deref(), Some("field_0"));
    assert_eq!(schema.types[1].fields[0].name.as_deref(), Some("field_0"));
    assert_eq!(
        schema.types[2].fields[0].name.as_deref(),
        Some("ProxyAddress")
    );
}

#[test]
fn message_signatures_group_exact_serialized_field_sequences() {
    let report = json!({
        "registryEntries": [{
            "uuid": "6E73E3A8-450B-4292-9582-9A75424EAC96",
            "typeIndex": 1873,
            "typeName": "MB::ServerContext::InitializeMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "u64",
                "wireShape": "u64",
                "wireShapeSource": "message-unmarshal-pcode-call",
                "confidence": "message-unmarshal-pcode-call"
            }, {
                "index": 1,
                "name": "field_1",
                "wireShape": "fixed-bytes-16",
                "confidence": "message-unmarshal-pcode-readraw"
            }, {
                "index": 2,
                "name": "field_2",
                "nativeType": "u32",
                "wireShape": "u32",
                "confidence": "message-unmarshal-pcode-call"
            }, {
                "index": 3,
                "name": "ActorInstantiationParameters",
                "nativeType": "Amazon::Hub::ActorInstantiationParameters",
                "confidence": "message-unmarshal-pcode-stack-direct-type"
            }, {
                "index": 4,
                "name": "field_4",
                "nativeType": "u32",
                "wireShape": "u32",
                "wireShapeSource": "message-unmarshal-pcode-store",
                "confidence": "message-unmarshal-pcode-store"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    schema.serialize_types = serde_json::from_value(json!([{
        "typeId": "652ED536-3402-439B-AEBE-4A5DBC554085",
        "kind": "struct",
        "name": "AssetId",
        "role": "support-type",
        "fieldCount": 2,
        "variantCount": 0,
        "directDependencyTypeIds": [],
        "wireShapes": ["fixed-bytes-16", "u32"],
        "isAbstract": null,
        "isReflectionMarker": true
    }]))
    .expect("serialize types");

    let merge = schema.merge_message_signatures(
        &[NetworkMessageSignature {
            type_id: Some(uuid!("6e73e3a8-450b-4292-9582-9a75424eac96")),
            type_index: Some(1873),
            name: Some("MB::ServerContext::InitializeMsg".to_owned()),
            rust_name: Some("InitializeMsg".to_owned()),
            source: None,
            fields: vec![
                NetworkMessageFieldSignature {
                    index: Some(0),
                    name: "GdeId".to_owned(),
                    rust_type: Some("::nw_network::GdeId".to_owned()),
                    native_type: Some("MB::GDEID".to_owned()),
                    wire_shape: Some(NetworkWireShape::U64),
                },
                NetworkMessageFieldSignature {
                    index: Some(1),
                    name: "AssetId".to_owned(),
                    rust_type: Some("::nw_network::AssetId".to_owned()),
                    native_type: Some("AZ::Data::AssetId".to_owned()),
                    wire_shape: None,
                },
                NetworkMessageFieldSignature {
                    index: Some(2),
                    name: "Parameters".to_owned(),
                    rust_type: None,
                    native_type: Some("Amazon::Hub::ActorInstantiationParameters".to_owned()),
                    wire_shape: None,
                },
                NetworkMessageFieldSignature {
                    index: Some(3),
                    name: "PostInitPriority".to_owned(),
                    rust_type: None,
                    native_type: Some("Amazon::Hub::SchedulerPriority".to_owned()),
                    wire_shape: Some(NetworkWireShape::U32),
                },
            ],
        }],
        Some("message-signatures.json".to_owned()),
    );

    assert_eq!(merge.field_count_mismatch_count, 0);
    assert_eq!(merge.field_grouped_count, 1);
    assert_eq!(merge.native_type_conflict_count, 0);
    assert_eq!(schema.types[0].fields.len(), 4);
    assert!(!schema.types[0].signature_field_count_conflict);
    let asset_id = &schema.types[0].fields[1];
    assert_eq!(asset_id.name.as_deref(), Some("AssetId"));
    assert_eq!(asset_id.native_type.as_deref(), Some("AZ::Data::AssetId"));
    assert_eq!(
        asset_id.wire_shape,
        Some(NetworkWireShape::Composite(vec![
            NetworkWireShape::FixedBytes(16),
            NetworkWireShape::U32,
        ]))
    );
    assert_eq!(schema.types[0].fields[2].index, Some(2));
    assert_eq!(schema.types[0].fields[3].index, Some(3));
}

#[test]
fn message_signatures_group_unknown_scalar_lanes_inside_exact_products() {
    let report = json!({
        "registryEntries": [{
            "uuid": "4604A3CC-2CAC-4A93-A0E2-D330041777E6",
            "typeIndex": 736,
            "typeName": "ActorMover::MovementTimeoutMsg",
            "messageUnmarshal": {
                "terminalStatus": "no-success-terminal",
                "supportsUnmarshal": false,
                "fields": []
            },
            "messageMarshal": {
                "fields": [{
                    "index": 0,
                    "storageExpression": "param_2 + 0x8",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 1,
                    "storageExpression": "param_2 + 0x18",
                    "wireShape": "u32",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 2,
                    "storageExpression": "param_2 + 0x1c",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 3,
                    "storageExpression": "param_2 + 0x2c",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 4,
                    "storageExpression": "param_2 + 0x40",
                    "wireShape": "u32",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 5,
                    "storageExpression": "param_2 + 0x44",
                    "wireShape": "u32",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 6,
                    "storageExpression": "param_2 + 0x48",
                    "wireShape": "u64",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 7,
                    "storageExpression": "param_2 + 0x50",
                    "wireShape": "u64",
                    "confidence": "message-marshal-pcode-stack"
                }]
            }
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    schema.serialize_types = serde_json::from_value(json!([{
        "typeId": "0638E28C-AB7B-4BA4-84AC-0353038E6FDC",
        "kind": "struct",
        "name": "Amazon::Hub::ActorRef",
        "role": "support-type",
        "fieldCount": 0,
        "variantCount": 0,
        "directDependencyTypeIds": [],
        "wireShapes": ["u32", "fixed-bytes-16", "fixed-bytes-16"],
        "isAbstract": null,
        "isReflectionMarker": true
    }, {
        "typeId": "AABF0B66-00C9-478D-BF17-25BF39F9D894",
        "kind": "struct",
        "name": "MovementInteractionId",
        "role": "support-type",
        "fieldCount": 4,
        "variantCount": 0,
        "directDependencyTypeIds": [],
        "wireShapes": ["u32", "u32", "u64", "u64"],
        "isAbstract": null,
        "isReflectionMarker": true
    }]))
    .expect("serialize types");

    let merge = schema.merge_message_signatures(
        &[NetworkMessageSignature {
            type_id: Some(uuid!("4604a3cc-2cac-4a93-a0e2-d330041777e6")),
            type_index: Some(736),
            name: Some("ActorMover::MovementTimeoutMsg".to_owned()),
            rust_name: Some("MovementTimeoutMsg".to_owned()),
            source: None,
            fields: vec![
                NetworkMessageFieldSignature {
                    index: Some(0),
                    name: "ActorId".to_owned(),
                    rust_type: Some("::nw_network::ActorId".to_owned()),
                    native_type: Some("Amazon::Hub::ActorID".to_owned()),
                    wire_shape: Some(NetworkWireShape::FixedBytes(16)),
                },
                NetworkMessageFieldSignature {
                    index: Some(1),
                    name: "RemoteMoveCoordinator".to_owned(),
                    rust_type: Some("::nw_network::ActorRef".to_owned()),
                    native_type: Some("Amazon::Hub::ActorRef".to_owned()),
                    wire_shape: None,
                },
                NetworkMessageFieldSignature {
                    index: Some(2),
                    name: "MovementInteractionId".to_owned(),
                    rust_type: Some("::nw_network::MovementInteractionId".to_owned()),
                    native_type: Some("MovementInteractionId".to_owned()),
                    wire_shape: None,
                },
            ],
        }],
        Some("message-signatures.json".to_owned()),
    );

    assert_eq!(merge.field_count_mismatch_count, 0);
    assert_eq!(merge.field_grouped_count, 2);
    assert_eq!(schema.types[0].marshal_fields.len(), 3);
    assert!(!schema.types[0].signature_field_count_conflict);
    assert_eq!(
        schema.types[0].marshal_fields[0].wire_shape,
        Some(NetworkWireShape::FixedBytes(16))
    );
    assert_eq!(
        schema.types[0].marshal_fields[1].name.as_deref(),
        Some("RemoteMoveCoordinator")
    );
    assert_eq!(
        schema.types[0].marshal_fields[2].name.as_deref(),
        Some("MovementInteractionId")
    );
    assert!(
        schema.types[0]
            .marshal_fields
            .iter()
            .all(|field| field.confidence == NetworkConfidence::High)
    );
}

#[test]
fn message_signatures_project_proven_nested_storage_into_semantic_fields() {
    let report = json!({
        "registryEntries": [{
            "uuid": "96A58E69-7BD5-45C5-86E4-DAF9F5EB1E86",
            "typeIndex": 397,
            "typeName": "Replicate::RegisterFragmentAccessMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "Amazon::Hub::Replicate::FragmentAccessMsg",
                "storageExpression": "param_3 + 0x8",
                "nestedTypeShape": {
                    "typeName": "FragmentAccessMsg",
                    "typeNameFull": "Amazon::Hub::Replicate::FragmentAccessMsg",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "wireOrderSource": "cfg-success-flow",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "wireShape": "u32",
                        "wireOrdinal": 0
                    }, {
                        "index": 1,
                        "offset": "0x4",
                        "wireShape": "fixed-bytes-16",
                        "wireOrdinal": 1
                    }, {
                        "index": 2,
                        "offset": "0x14",
                        "wireShape": "fixed-bytes-16",
                        "wireOrdinal": 2
                    }, {
                        "index": 3,
                        "offset": "0x24",
                        "wireShape": "u32",
                        "wireOrdinal": 3
                    }]
                },
                "confidence": "message-unmarshal-pcode-stack-direct-type"
            }]
        }, {
            "uuid": "951EF3ED-C9A0-4E3D-A6FD-7FE0673D28D2",
            "typeIndex": 422,
            "typeName": "ReplicateClient::FragmentUpdateMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "nativeType": "Amazon::Hub::ProxyAddress",
                "nestedTypeShape": {
                    "typeName": "ProxyAddress",
                    "typeNameFull": "Amazon::Hub::ProxyAddress",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "wireOrderSource": "cfg-success-flow",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "wireShape": "u32",
                        "wireOrdinal": 0
                    }, {
                        "index": 1,
                        "offset": "0x4",
                        "wireShape": "fixed-bytes-16",
                        "wireOrdinal": 1
                    }, {
                        "index": 2,
                        "offset": "0x14",
                        "wireShape": "fixed-bytes-16",
                        "wireOrdinal": 2
                    }]
                },
                "confidence": "message-unmarshal-pcode-stack-direct-type"
            }, {
                "index": 1,
                "name": "field_1",
                "nativeType": "u32",
                "wireShape": "u32",
                "confidence": "message-unmarshal-pcode-call"
            }, {
                "index": 2,
                "name": "field_2",
                "nativeType": "Amazon::Hub::BaselineableFragment",
                "confidence": "message-unmarshal-pcode-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

    let merge = schema.merge_message_signatures(
        &[
            NetworkMessageSignature {
                type_id: Some(uuid!("96a58e69-7bd5-45c5-86e4-daf9f5eb1e86")),
                type_index: Some(397),
                name: Some("Replicate::RegisterFragmentAccessMsg".to_owned()),
                rust_name: Some("RegisterFragmentAccessMsg".to_owned()),
                source: None,
                fields: vec![
                    NetworkMessageFieldSignature {
                        index: Some(0),
                        name: "ProxyRef".to_owned(),
                        rust_type: Some("::nw_network::ActorRef".to_owned()),
                        native_type: Some("ActorRef".to_owned()),
                        wire_shape: Some(NetworkWireShape::ActorRef),
                    },
                    NetworkMessageFieldSignature {
                        index: Some(1),
                        name: "Key".to_owned(),
                        rust_type: None,
                        native_type: Some("FragmentKey".to_owned()),
                        wire_shape: Some(NetworkWireShape::U32),
                    },
                ],
            },
            NetworkMessageSignature {
                type_id: Some(uuid!("951ef3ed-c9a0-4e3d-a6fd-7fe0673d28d2")),
                type_index: Some(422),
                name: Some("ReplicateClient::FragmentUpdateMsg".to_owned()),
                rust_name: Some("FragmentUpdateMsg".to_owned()),
                source: None,
                fields: vec![
                    NetworkMessageFieldSignature {
                        index: Some(0),
                        name: "TargetRef".to_owned(),
                        rust_type: Some("::nw_network::ActorRef".to_owned()),
                        native_type: Some("ActorRef".to_owned()),
                        wire_shape: Some(NetworkWireShape::ActorRef),
                    },
                    NetworkMessageFieldSignature {
                        index: Some(1),
                        name: "Key".to_owned(),
                        rust_type: None,
                        native_type: Some("FragmentKey".to_owned()),
                        wire_shape: Some(NetworkWireShape::U32),
                    },
                    NetworkMessageFieldSignature {
                        index: Some(2),
                        name: "Fragment".to_owned(),
                        rust_type: None,
                        native_type: Some("Amazon::Hub::BaselineableFragment".to_owned()),
                        wire_shape: None,
                    },
                ],
            },
        ],
        Some("message-signatures.json".to_owned()),
    );

    assert_eq!(merge.field_count_mismatch_count, 0);
    assert_eq!(merge.native_type_conflict_count, 0);
    let access = &schema.types[0];
    assert_eq!(access.fields.len(), 2);
    assert_eq!(access.fields[0].name.as_deref(), Some("ProxyRef"));
    assert_eq!(access.fields[0].native_type.as_deref(), Some("ActorRef"));
    assert_eq!(
        access.fields[0].wire_shape,
        Some(NetworkWireShape::ActorRef)
    );
    assert_eq!(access.fields[1].name.as_deref(), Some("Key"));
    assert_eq!(access.fields[1].native_type.as_deref(), Some("FragmentKey"));
    assert_eq!(access.fields[1].wire_shape, Some(NetworkWireShape::U32));
    assert!(
        access
            .fields
            .iter()
            .all(|field| field.storage_expression.is_none())
    );
    assert!(
        access
            .fields
            .iter()
            .all(|field| field.nested_type_shape.is_none())
    );

    let update = &schema.types[1];
    assert_eq!(update.fields.len(), 3);
    assert_eq!(update.fields[0].name.as_deref(), Some("TargetRef"));
    assert_eq!(update.fields[0].native_type.as_deref(), Some("ActorRef"));
    assert_eq!(
        update.fields[0].wire_shape,
        Some(NetworkWireShape::ActorRef)
    );
    assert_eq!(update.fields[1].name.as_deref(), Some("Key"));
    assert_eq!(update.fields[1].native_type.as_deref(), Some("FragmentKey"));
    assert_eq!(update.fields[2].name.as_deref(), Some("Fragment"));
}

#[test]
fn message_signatures_reject_nonmatching_serialized_field_sequences() {
    let report = json!({
        "registryEntries": [{
            "uuid": "6E73E3A8-450B-4292-9582-9A75424EAC96",
            "typeIndex": 1873,
            "typeName": "MB::ServerContext::InitializeMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "wireShape": "u32",
                "confidence": "message-unmarshal-pcode-call"
            }, {
                "index": 1,
                "name": "field_1",
                "wireShape": "fixed-bytes-16",
                "confidence": "message-unmarshal-pcode-readraw"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    schema.serialize_types = serde_json::from_value(json!([{
        "typeId": "652ED536-3402-439B-AEBE-4A5DBC554085",
        "kind": "struct",
        "name": "AssetId",
        "role": "support-type",
        "fieldCount": 2,
        "variantCount": 0,
        "directDependencyTypeIds": [],
        "wireShapes": ["fixed-bytes-16", "u32"],
        "isAbstract": null,
        "isReflectionMarker": true
    }]))
    .expect("serialize types");

    let merge = schema.merge_message_signatures(
        &[NetworkMessageSignature {
            type_id: Some(uuid!("6e73e3a8-450b-4292-9582-9a75424eac96")),
            type_index: Some(1873),
            name: None,
            rust_name: None,
            source: None,
            fields: vec![NetworkMessageFieldSignature {
                index: Some(0),
                name: "AssetId".to_owned(),
                rust_type: Some("::nw_network::AssetId".to_owned()),
                native_type: Some("AZ::Data::AssetId".to_owned()),
                wire_shape: None,
            }],
        }],
        Some("message-signatures.json".to_owned()),
    );

    assert_eq!(merge.field_grouped_count, 0);
    assert_eq!(merge.field_count_mismatch_count, 1);
    assert!(schema.types[0].signature_field_count_conflict);
    assert_eq!(schema.types[0].fields.len(), 2);
}

#[test]
fn merges_message_signature_fields_when_static_report_has_none() {
    let report = json!({
        "registryEntries": [{
            "uuid": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
            "typeIndex": 77,
            "typeName": "ExampleMsg",
            "fields": []
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");

    let merge = schema.merge_message_signatures(
        &[NetworkMessageSignature {
            type_id: Some(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")),
            type_index: Some(77),
            name: Some("ExampleMsg".to_owned()),
            rust_name: Some("ExampleMsg".to_owned()),
            source: None,
            fields: vec![NetworkMessageFieldSignature {
                index: Some(0),
                name: "Payload".to_owned(),
                rust_type: Some("::nw_network::Payload".to_owned()),
                native_type: Some("Payload".to_owned()),
                wire_shape: None,
            }],
        }],
        Some("message-signatures.json".to_owned()),
    );

    assert_eq!(merge.matched_message_count, 1);
    assert_eq!(merge.field_name_filled_count, 1);
    assert_eq!(merge.native_type_filled_count, 1);
    assert_eq!(merge.wire_shape_filled_count, 0);
    assert_eq!(schema.types[0].fields.len(), 1);
    let field = &schema.types[0].fields[0];
    assert_eq!(field.name.as_deref(), Some("Payload"));
    assert_eq!(field.rust_type.as_deref(), Some("::nw_network::Payload"));
    assert_eq!(field.confidence, NetworkConfidence::High);
    assert_eq!(field.evidence[0].kind, NetworkEvidenceKind::MessageSource);
}

#[test]
fn merges_field_overrides_with_source_style_container_types() {
    let report = json!({
        "registryEntries": [{
            "uuid": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
            "typeIndex": 3362,
            "typeName": "Javelin::SlayerScriptReplicatedState",
            "fields": [{
                "index": 3,
                "name": "spawnedEntityIdsBySpawnerId",
                "nativeType": "MB::ReplicatedMapFieldHandler<AZ::Crc32, AZ::EntityId>",
                "wireShape": "replicated-container<u32,u64>",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");
    let overrides = NetworkFieldOverrideFile {
            fields: vec![NetworkFieldOverride {
                type_id: None,
                type_index: Some(3362),
                type_name: None,
                field_index: Some(3),
                field: Some("spawnedEntityIdsBySpawnerId".to_owned()),
                name: None,
                native_type: Some("MB::ReplicatedMapFieldHandler<AZ::Crc32, AZ::EntityId>".to_owned()),
                rust_type: Some("::nw_network::serialize::ReplicatedContainer<::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>, { ::nw_network::serialize::WIRE_VEC_CAP }, ::nw_network::serialize::DefaultMarshaler<::nw_network::Crc32>, ::nw_network::serialize::DefaultMarshaler<::nw_network::EntityId>>".to_owned()),
                wire_shape: Some(NetworkWireShape::ReplicatedContainer(
                    NetworkReplicatedContainerWireShape {
                        key: NetworkWireScalarShape::U32,
                        value: NetworkWireScalarShape::U64,
                    },
                )),
                wire_shape_source: Some("field-overrides".to_owned()),
                confidence: Some(NetworkConfidence::High),
            }],
        };

    let merge =
        schema.merge_field_overrides(&overrides, Some("network-field-overrides.json".to_owned()));

    assert_eq!(merge.source_field_count, 1);
    assert_eq!(merge.matched_field_count, 1);
    assert_eq!(merge.unmatched_type_count, 0);
    assert_eq!(merge.unmatched_field_count, 0);
    assert_eq!(merge.rust_type_updated_count, 1);
    assert_eq!(merge.wire_shape_updated_count, 1);
    assert!(schema.sources.iter().any(|source| {
        source.kind == NetworkSchemaSourceKind::FieldOverrides
            && source.path.as_deref() == Some("network-field-overrides.json")
    }));
    let field = &schema.types[0].fields[0];
    assert!(
        field
            .rust_type
            .as_deref()
            .is_some_and(|rust_type| rust_type.contains("ReplicatedContainer<"))
    );
    assert!(
        field
            .rust_type
            .as_deref()
            .is_some_and(|rust_type| rust_type.contains("IndexMap<"))
    );
    assert_eq!(
        field.evidence.last().map(|evidence| evidence.kind),
        Some(NetworkEvidenceKind::FieldOverride)
    );
}

#[test]
fn renames_a_field_override_selected_by_index() {
    let report = json!({
        "registryEntries": [{
            "uuid": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
            "typeIndex": 226,
            "typeName": "ReceiveConfigOverridesKeyValuePairsMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "wireShape": "vec<composite<string,vec<composite<string,string>>>>",
                "confidence": "message-unmarshal-pcode-interprocedural-collection"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");
    let overrides = NetworkFieldOverrideFile {
        fields: vec![NetworkFieldOverride {
            type_id: None,
            type_index: Some(226),
            type_name: None,
            field_index: Some(0),
            field: None,
            name: Some("sections".to_owned()),
            native_type: None,
            rust_type: None,
            wire_shape: None,
            wire_shape_source: None,
            confidence: Some(NetworkConfidence::High),
        }],
    };

    let merge =
        schema.merge_field_overrides(&overrides, Some("network-field-overrides.json".to_owned()));

    assert_eq!(merge.matched_field_count, 1);
    assert_eq!(merge.unmatched_field_count, 0);
    assert_eq!(merge.field_name_updated_count, 1);
    assert_eq!(schema.types[0].fields[0].name.as_deref(), Some("sections"));
}

#[test]
fn merges_typeindex_without_overwriting_conflicts() {
    let report = json!({
        "registryEntries": [
            {
                "uuid": "8673A3CC-2848-4C87-AA72-CC860589D1B5",
                "typeName": "ExampleFilled"
            },
            {
                "uuid": "DA4E5889-A65C-4480-8642-0278160125A7",
                "typeName": "ExampleConflict",
                "typeIndex": 9
            }
        ],
        "fieldRegistrationFunctions": []
    });
    let typeindex = json!({
        "typeIndex": [
            "00000000000000000000000000000000",
            "8673A3CC28484C87AA72CC860589D1B5",
            "DA4E5889A65C448086420278160125A7"
        ]
    });

    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let merge = schema
        .merge_typeindex_root(&typeindex, Some("typeindex.json".to_owned()))
        .expect("typeindex merge");

    assert_eq!(merge.source_type_count, 3);
    assert_eq!(merge.matched_type_count, 2);
    assert_eq!(merge.filled_type_index_count, 1);
    assert_eq!(merge.conflicting_type_index_count, 1);
    assert_eq!(schema.types[0].type_index, Some(1));
    assert_eq!(schema.types[1].type_index, Some(9));
    assert_eq!(schema.summary.type_index_evidence_count, 2);
    assert!(schema.sources.iter().any(|source| {
        source.kind == NetworkSchemaSourceKind::TypeIndex
            && source.path.as_deref() == Some("typeindex.json")
    }));
    assert_eq!(
        schema.types[1]
            .evidence
            .last()
            .map(|evidence| evidence.confidence),
        Some(NetworkConfidence::Weak)
    );
}

#[test]
fn merges_serialize_codegen_evidence_and_dependencies() {
    let root_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
    let dependency_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
    let report = json!({
        "registryEntries": [{
            "uuid": root_type_id.to_string(),
            "typeName": "NetworkName"
        }],
        "fieldRegistrationFunctions": []
    });
    let unit = SerializeCodegenUnit {
        items: vec![SerializeCodegenItem {
            source_type_id: root_type_id,
            source_name: "SerializeName".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: vec![SerializeCodegenRttiBase {
                type_id: dependency_type_id,
                source_name: "Dependency".to_owned(),
            }],
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        }],
    };

    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

    assert_eq!(merge.source_type_count, 1);
    assert_eq!(merge.matched_type_count, 1);
    assert_eq!(merge.type_id_matched_count, 1);
    assert_eq!(merge.name_matched_count, 0);
    assert_eq!(merge.filled_name_count, 0);
    assert_eq!(schema.summary.serialize_type_count, 1);
    assert_eq!(schema.summary.serialize_dependency_count, 1);
    let serialize = schema.types[0].serialize.as_ref().expect("serialize merge");
    assert_eq!(serialize.name, "SerializeName");
    assert_eq!(serialize.kind, NetworkSerializeKind::Struct);
    assert_eq!(serialize.role, NetworkSerializeRole::SupportType);
    assert_eq!(
        serialize.direct_dependency_type_ids,
        vec![dependency_type_id]
    );
    assert!(
        schema.types[0]
            .evidence
            .iter()
            .any(|evidence| evidence.kind == NetworkEvidenceKind::SerializeContext)
    );
}

#[test]
fn merges_serialize_codegen_by_unique_source_name_with_inferred_confidence() {
    let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
    let serialize_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
    let report = json!({
        "registryEntries": [{
            "uuid": network_type_id.to_string(),
            "typeName": "Example::SharedName"
        }],
        "fieldRegistrationFunctions": []
    });
    let unit = SerializeCodegenUnit {
        items: vec![SerializeCodegenItem {
            source_type_id: serialize_type_id,
            source_name: "Example::SharedName".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        }],
    };

    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

    assert_eq!(merge.matched_type_count, 1);
    assert_eq!(merge.type_id_matched_count, 0);
    assert_eq!(merge.name_matched_count, 1);
    assert_eq!(merge.ambiguous_name_match_count, 0);
    assert_eq!(schema.summary.serialize_type_count, 1);
    let evidence = schema.types[0]
        .evidence
        .iter()
        .find(|evidence| evidence.kind == NetworkEvidenceKind::SerializeContext)
        .expect("serialize evidence");
    assert_eq!(evidence.source, "serializeContext:name");
    assert_eq!(evidence.confidence, NetworkConfidence::Inferred);
}

#[test]
fn merges_field_serialize_type_by_nested_type_id() {
    let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
    let payload_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
    let report = json!({
        "registryEntries": [{
            "uuid": network_type_id.to_string(),
            "typeName": "Example::PayloadMessage",
            "fields": [{
                "index": 0,
                "name": "payload",
                "nativeType": "PayloadData",
                "sourceTypeId": payload_type_id.to_string(),
                "confidence": "message-unmarshal-direct-type",
                "storageExpression": "param_3 + 0x8",
                "nestedTypeShape": {
                    "typeId": payload_type_id.to_string(),
                    "typeIdSource": "serialize-context-name",
                    "identityProven": true,
                    "identitySource": "pcode-direct-type-provider",
                    "typeName": "PayloadData",
                    "typeNameFull": "Example::PayloadData",
                    "factory": "NewWorld+0x1234",
                    "members": []
                }
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let unit = SerializeCodegenUnit {
        items: vec![SerializeCodegenItem {
            source_type_id: payload_type_id,
            source_name: "Example::PayloadData".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: Some("NewWorld+0x1234".to_owned()),
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        }],
    };

    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

    assert_eq!(merge.matched_type_count, 0);
    assert_eq!(merge.matched_field_type_count, 1);
    assert_eq!(merge.field_type_id_matched_count, 1);
    assert_eq!(schema.summary.serialize_type_count, 0);
    assert_eq!(schema.summary.serialize_field_type_count, 1);
    let field = &schema.types[0].fields[0];
    let serialize = field.serialize.as_ref().expect("field serialize type");
    assert_eq!(serialize.type_id, payload_type_id);
    assert_eq!(serialize.name, "Example::PayloadData");
    assert_eq!(serialize.source, "serializeContext:field-type-id");
    assert_eq!(serialize.confidence, NetworkConfidence::Exact);
}

#[test]
fn merges_field_serialize_type_by_selected_handler_value_type_id() {
    let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
    let payload_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
    let report = json!({
        "registryEntries": [{
            "uuid": network_type_id.to_string(),
            "typeName": "Example::ReplicatedState",
            "fields": [{
                "index": 0,
                "name": "payloads",
                "handlerVtable": "NewWorld+0x8123450",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8123450",
            "fieldCount": 1,
            "valueTypeInfo": {
                "address": "NewWorld+0x8234560",
                "name": "PayloadData",
                "typeId": payload_type_id.to_string(),
                "source": "unmarshal-full-element-vptr+native-size",
                "nameSource": "az-rtti-vtable-provider",
                "nativeSize": "0x20",
                "nativeSizeSource": "serialize-field-data-size"
            },
            "valueTypeInfoCandidates": [{
                "address": "NewWorld+0x8345670",
                "name": "NestedMember",
                "typeId": "11111111-1111-1111-1111-111111111111",
                "source": "rtti-provider-vtable",
                "nameSource": "rtti-helper-function-name"
            }],
            "valueTypeShape": {
                "typeId": payload_type_id.to_string(),
                "typeIdSource": "rtti-provider-vtable",
                "identityProven": true,
                "identitySource": "pcode-direct-type-provider",
                "typeName": "PayloadData",
                "typeNameFull": "PayloadData",
                "typeNameSource": "rtti-helper-function-name",
                "azRttiAddress": "NewWorld+0x8234560",
                "members": []
            }
        }]
    });
    let unit = SerializeCodegenUnit {
        items: vec![SerializeCodegenItem {
            source_type_id: payload_type_id,
            source_name: "PayloadData".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        }],
    };

    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

    assert_eq!(merge.matched_type_count, 0);
    assert_eq!(merge.matched_field_type_count, 1);
    assert_eq!(merge.field_type_id_matched_count, 1);
    assert_eq!(schema.summary.serialize_type_count, 0);
    assert_eq!(schema.summary.serialize_field_type_count, 1);
    let field = &schema.types[0].fields[0];
    let serialize = field.serialize.as_ref().expect("field serialize type");
    assert_eq!(serialize.type_id, payload_type_id);
    assert_eq!(serialize.name, "PayloadData");
    assert_eq!(serialize.source, "serializeContext:handler-value-type-id");
    assert_eq!(serialize.confidence, NetworkConfidence::High);
}

#[test]
fn does_not_merge_field_serialize_type_from_provider_candidate_only() {
    let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
    let payload_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
    let report = json!({
        "registryEntries": [{
            "uuid": network_type_id.to_string(),
            "typeName": "Example::ReplicatedState",
            "fields": [{
                "index": 0,
                "name": "payloads",
                "handlerVtable": "NewWorld+0x8123450",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8123450",
            "fieldCount": 1,
            "valueTypeInfoCandidates": [{
                "address": "NewWorld+0x8234560",
                "name": "PayloadData",
                "typeId": payload_type_id.to_string(),
                "source": "rtti-provider-vtable",
                "nameSource": "rtti-helper-function-name"
            }]
        }]
    });
    let unit = SerializeCodegenUnit {
        items: vec![SerializeCodegenItem {
            source_type_id: payload_type_id,
            source_name: "PayloadData".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        }],
    };

    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

    assert_eq!(merge.matched_field_type_count, 0);
    assert_eq!(merge.field_type_id_matched_count, 0);
    assert_eq!(schema.summary.serialize_field_type_count, 0);
    assert!(schema.types[0].fields[0].serialize.is_none());
}

#[test]
fn skips_ambiguous_serialize_codegen_name_matches() {
    let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
    let report = json!({
        "registryEntries": [{
            "uuid": network_type_id.to_string(),
            "typeName": "Example::SharedName"
        }],
        "fieldRegistrationFunctions": []
    });
    let unit = SerializeCodegenUnit {
        items: vec![
            SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::SharedName".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: Vec::new(),
                variants: Vec::new(),
            },
            SerializeCodegenItem {
                source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                source_name: "Example::SharedName".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: Vec::new(),
                variants: Vec::new(),
            },
        ],
    };

    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

    assert_eq!(merge.matched_type_count, 0);
    assert_eq!(merge.name_matched_count, 0);
    assert_eq!(merge.ambiguous_name_match_count, 1);
    assert_eq!(merge.unmatched_schema_type_count, 1);
    assert_eq!(schema.summary.serialize_type_count, 0);
}

#[test]
fn does_not_merge_serialize_codegen_by_nil_type_id() {
    let report = json!({
        "registryEntries": [{
            "uuid": "00000000-0000-0000-0000-000000000000",
            "typeName": "NullType"
        }],
        "fieldRegistrationFunctions": []
    });
    let unit = SerializeCodegenUnit {
        items: vec![SerializeCodegenItem {
            source_type_id: Uuid::nil(),
            source_name: "WaterDepth".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        }],
    };

    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

    assert_eq!(merge.matched_type_count, 0);
    assert_eq!(merge.type_id_matched_count, 0);
    assert_eq!(merge.name_matched_count, 0);
    assert_eq!(merge.unmatched_schema_type_count, 1);
    assert_eq!(schema.summary.serialize_type_count, 0);
}
