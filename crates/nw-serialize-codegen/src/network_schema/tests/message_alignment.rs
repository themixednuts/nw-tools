use super::*;

#[test]
fn source_signatures_reorder_proven_helper_aggregates_without_using_storage_order() {
    let report = json!({
        "registryEntries": [{
            "uuid": "604EE6CA-3B94-4209-9845-0F94F5342B92",
            "typeIndex": 2150,
            "typeName": "MB::ServerContext::AddPortrayalToClientsMsg",
            "fields": [{
                "index": 0,
                "name": "field_0",
                "storageExpression": "param_3 + 0x68",
                "storageOffset": "0x68",
                "wireLayout": "composite<fixed-bytes-16,u64,fixed-bytes-8>",
                "callFrameBoundaryProven": true,
                "nestedTypeShape": {
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "members": [{
                        "index": 0,
                        "wireShape": "fixed-bytes-16",
                        "wireOrdinal": 0
                    }, {
                        "index": 1,
                        "wireShape": "u64",
                        "wireOrdinal": 1
                    }, {
                        "index": 2,
                        "wireShape": "u64",
                        "wireLayout": "fixed-bytes-8",
                        "wireOrdinal": 2
                    }]
                },
                "confidence": "message-unmarshal-call-frame-boundary"
            }, {
                "index": 1,
                "name": "field_1",
                "nativeType": "u64",
                "storageExpression": "param_3 + 0x8",
                "storageOffset": "0x8",
                "wireShape": "u64",
                "confidence": "message-unmarshal-call-frame-output-member"
            }, {
                "index": 2,
                "name": "field_2",
                "nativeType": "ClientRef",
                "sourceTypeName": "ClientRef",
                "sourceTypeId": "C148C555-3264-41F7-A335-E48B65F91728",
                "sourceTypeIdentityProven": true,
                "storageExpression": "param_3 + 0x10",
                "storageOffset": "0x10",
                "wireShape": "actor-ref",
                "confidence": "message-unmarshal-constructor-typed-boundary"
            }, {
                "index": 3,
                "name": "field_3",
                "nativeType": "Amazon::Hub::ActorRef",
                "storageExpression": "param_3 + 0x40",
                "storageOffset": "0x40",
                "wireShape": "composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>",
                "confidence": "message-unmarshal-call-frame-output-member"
            }, {
                "index": 4,
                "name": "field_4",
                "nativeType": "Amazon::Hub::ActorRef",
                "storageExpression": "param_3 + 0x98",
                "storageOffset": "0x98",
                "wireShape": "composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>",
                "confidence": "message-unmarshal-call-frame-output-member"
            }]
        }],
        "fieldRegistrationFunctions": []
    });
    let mut schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("network schema");
    assert_eq!(schema.types[0].fields.len(), 5);
    let signatures = [NetworkMessageSignature {
        type_id: Some(uuid!("604ee6ca-3b94-4209-9845-0f94f5342b92")),
        type_index: Some(2150),
        name: Some("MB::ServerContext::AddPortrayalToClientsMsg".to_owned()),
        rust_name: Some("AddPortrayalToClientsMsg".to_owned()),
        source: None,
        fields: vec![
            signature_field(0, "GdeId", "MB::GDEID", NetworkWireShape::U64),
            signature_field(1, "Client", "MB::ClientRef", NetworkWireShape::ActorRef),
            signature_field(
                2,
                "GhostClient",
                "Amazon::Hub::ActorRef",
                NetworkWireShape::ActorRef,
            ),
            signature_field(
                3,
                "InterestRef",
                "MB::RemoteTypelessServerFacetRef",
                NetworkWireShape::Composite(vec![
                    NetworkWireShape::FixedBytes(16),
                    NetworkWireShape::U64,
                    NetworkWireShape::U64,
                ]),
            ),
            signature_field(
                4,
                "OwningActor",
                "Amazon::Hub::ActorRef",
                NetworkWireShape::ActorRef,
            ),
        ],
    }];

    let merge = schema.merge_message_signatures(&signatures, Some("source".to_owned()));
    let message = &schema.types[0];

    assert_eq!(merge.field_reordered_count, 4);
    assert_eq!(merge.native_type_conflict_count, 0);
    assert_eq!(merge.wire_shape_conflict_count, 0);
    assert!(!message.signature_field_count_conflict);
    assert_eq!(
        message
            .fields
            .iter()
            .map(|field| field.name.as_deref().unwrap())
            .collect::<Vec<_>>(),
        [
            "GdeId",
            "Client",
            "GhostClient",
            "InterestRef",
            "OwningActor"
        ]
    );
    assert_eq!(
        message
            .fields
            .iter()
            .map(|field| field.storage_offset)
            .collect::<Vec<_>>(),
        [Some(8), Some(16), Some(64), Some(104), Some(152)]
    );
    assert_eq!(
        message.fields[3].wire_shape,
        Some(NetworkWireShape::Composite(vec![
            NetworkWireShape::FixedBytes(16),
            NetworkWireShape::U64,
            NetworkWireShape::U64,
        ]))
    );
    assert_eq!(
        message.fields[2].wire_shape,
        Some(NetworkWireShape::ActorRef)
    );
    assert_eq!(
        message.fields[4].wire_shape,
        Some(NetworkWireShape::ActorRef)
    );
}

fn signature_field(
    index: u32,
    name: &str,
    native_type: &str,
    wire_shape: NetworkWireShape,
) -> NetworkMessageFieldSignature {
    NetworkMessageFieldSignature {
        index: Some(index),
        name: name.to_owned(),
        rust_type: None,
        native_type: Some(native_type.to_owned()),
        wire_shape: Some(wire_shape),
    }
}
