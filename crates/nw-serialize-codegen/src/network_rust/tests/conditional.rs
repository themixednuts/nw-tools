use super::*;

#[test]
fn emits_boolean_choice_with_default_omitted_false_branch() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "9831F691-7F36-43E4-9B4A-FEBC53337F96",
            "typeIndex": 1843,
            "typeName": "Aoi::PhysicsTrait::QueryWorldAabbMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "Shape",
                "wireShape": "boolean-choice<default-omitted<vec3,vec3>,u32>",
                "confidence": "message-unmarshal-control-flow"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert!(
        output
            .source
            .contains("pub shape: ::nw_network::serialize::BooleanChoice<")
    );
    assert!(output.source.contains("(::glam::Vec3, ::glam::Vec3)"));
    assert!(output.source.contains(
        "::nw_network::serialize::BooleanChoiceCodec<::nw_network::serialize::DefaultOmittedTupleCodec<(::nw_network::serialize::DefaultMarshaler<::glam::Vec3>, ::nw_network::serialize::DefaultMarshaler<::glam::Vec3>)>, ::nw_network::serialize::DefaultMarshaler<u32>>"
    ));
}
