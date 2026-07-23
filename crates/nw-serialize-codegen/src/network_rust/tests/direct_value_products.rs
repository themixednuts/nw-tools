use super::*;

#[test]
fn emits_proven_direct_value_members_with_shared_runtime_types() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "A31A3A11-70D5-4B7E-97F8-FB793A1A2E9D",
            "typeIndex": 5099,
            "typeName": "Example::RequestEntityDebugInfoMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "payload",
                "nativeType": "Example::RequestEntityDebugInfoPayload",
                "confidence": "message-unmarshal-pcode-nested-direct-type-shape",
                "nestedTypeShape": {
                    "typeName": "RequestEntityDebugInfoPayload",
                    "typeNameFull": "Example::RequestEntityDebugInfoPayload",
                    "identityProven": true,
                    "identitySource": "ghidra-direct-unmarshal-value-type+cfg-complete-wire-product",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "members": [{
                        "index": 0,
                        "offset": "0x8",
                        "name": "_0",
                        "nativeType": "Javelin::ClientMessages::ActorRequestId",
                        "typeIdentityProven": true,
                        "typeIdentitySource": "ghidra-direct-unmarshal-value-type+cfg-complete-wire-product",
                        "wireLayout": "composite<u64,u64>",
                        "wireOrdinal": 0
                    }, {
                        "index": 1,
                        "offset": "0x28",
                        "name": "_1",
                        "nativeType": "Amazon::Hub::ActorRef",
                        "typeId": "0638E28C-AB7B-4BA4-84AC-0353038E6FDC",
                        "typeIdentityProven": true,
                        "wireShape": "actor-ref",
                        "wireLayout": "composite<u32,fixed-bytes-16,fixed-bytes-16>",
                        "wireOrdinal": 1
                    }]
                }
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
        Some("RequestEntityDebugInfoPayload")
    );
    assert!(
        output
            .source
            .contains("pub field_0: ::nw_network::ActorRequestId"),
        "{}",
        output.source
    );
    assert!(
        output
            .source
            .contains("pub field_1: ::nw_network::ActorRef"),
        "{}",
        output.source
    );
}
