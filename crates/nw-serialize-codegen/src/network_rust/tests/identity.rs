use super::*;

#[test]
fn emits_identity_for_nil_uuid_descriptor() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "00000000-0000-0000-0000-000000000000",
            "typeIndex": 0,
            "typeName": "NullType",
            "fields": []
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

    assert_eq!(output.report.descriptor_count, 1);
    assert_eq!(output.report.identity_type_count, 1);
    assert!(output.source.contains("pub struct NullType"));
}

#[test]
fn qualifies_identity_leaf_name_collisions_with_namespace() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [
            {
                "uuid": "11111111-1111-1111-1111-111111111111",
                "typeIndex": 10,
                "typeName": "First::SharedName",
                "fields": []
            },
            {
                "uuid": "22222222-2222-2222-2222-222222222222",
                "typeIndex": 11,
                "typeName": "Second::SharedName",
                "fields": []
            }
        ],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

    assert_eq!(output.report.identity_name_collision_count, 1);
    assert_eq!(output.report.identity_type_count, 2);
    assert!(output.source.contains("pub struct FirstSharedName"));
    assert!(output.source.contains("pub struct SecondSharedName"));
}
