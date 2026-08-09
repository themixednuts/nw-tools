use super::*;

#[test]
fn emits_actor_instantiation_parameters_descriptor_shape() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "F24987B3-0136-4BCF-9D27-08BDE18B0266",
            "typeIndex": 751,
            "typeName": "Amazon::Hub::CreateActorMsg",
            "fields": [{
                "index": 0,
                "name": "Parameters",
                "nativeType": "Amazon::Hub::ActorInstantiationParameters",
                "wireShape": "actor-instantiation-parameters",
                "wireShapeSource": "unmarshal-native-type+u16-counted-bool-class-value-loop",
                "wireLayout": "actor-instantiation-parameters",
                "wireLayoutSource": "unmarshal-native-type+u16-counted-bool-class-value-loop",
                "confidence": "message-unmarshal-pcode-direct-type-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_descriptors(&schema).expect("descriptor source");

    assert_eq!(output.report.field_wire_shape_count, 1);
    assert!(
        output
            .source
            .contains("wire_shape: Some(NetworkWireShape::ActorInstantiationParameters)")
    );
}

#[test]
fn emits_compile_ready_descriptor_module() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
            "typeIndex": 28,
            "typeName": "Javelin::RaidDataComponentReplicatedState",
            "fields": [{
                "index": 0,
                "name": "raidId",
                "group": 0,
                "handlerVtable": "NewWorld+0x81dad80",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81dad80",
            "fieldCount": 1,
            "marshal": "NewWorld+0x344a700",
            "marshalTarget": "NewWorld+0x17266c0",
            "unmarshal": "NewWorld+0x3464830",
            "wireShape": "u64",
            "wireShapeSource": "marshal-pcode-fixed-width-structure",
            "slots": []
        }]
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

    assert_eq!(output.report.descriptor_count, 1);
    assert_eq!(output.report.identity_type_count, 1);
    assert_eq!(output.report.field_descriptor_count, 1);
    assert_eq!(output.report.field_wire_shape_count, 1);
    assert_eq!(output.report.unresolved_field_wire_shape_count, 0);
    assert_eq!(output.report.state_generation_plan_count, 1);
    assert_eq!(output.report.generatable_state_count, 1);
    assert_eq!(output.report.blocked_state_count, 0);
    assert_eq!(output.report.replicated_state_count, 1);
    let state_plan = &output.report.state_generation_plans[0];
    assert!(state_plan.can_generate);
    assert_eq!(
        state_plan.type_name.as_deref(),
        Some("Javelin::RaidDataComponentReplicatedState")
    );
    assert_eq!(state_plan.field_count, 1);
    assert_eq!(state_plan.shaped_field_count, 1);
    assert_eq!(state_plan.supported_field_count, 1);
    assert_eq!(
        state_plan.fields[0].rust_field_type.as_deref(),
        Some("ReplicatedFieldHandler<u64>")
    );
    assert!(output.source.contains("pub trait NetworkTypeIdentity"));
    assert!(output.source.contains("pub mod identity"));
    assert!(output.source.contains("pub enum NetworkWireShape"));
    assert!(output.source.contains("pub fn field_by_index"));
    assert!(output.source.contains("pub fn field_for_type_index"));
    assert!(
        output
            .source
            .contains("pub fn type_indices_missing_field_wire_shapes")
    );
    assert!(
        output
            .source
            .contains("pub struct RaidDataComponentReplicatedState")
    );
    assert!(
        output
            .source
            .contains("pub const NETWORK_TYPES: &[NetworkTypeDescriptor]")
    );
    assert!(output.source.contains("is_replicated_state_type_index"));
    assert!(output.source.contains("non_replicated_state_type_indices"));
    assert!(
        output
            .source
            .contains("Javelin::RaidDataComponentReplicatedState")
    );
    assert!(
        output
            .source
            .contains("name: Some(\"Javelin::RaidDataComponentReplicatedState\")")
    );
    assert!(
        output
            .source
            .contains("0xA85DF621_DCE0_409F_8D39_A447EA0807FF")
    );
    assert!(
        !output
            .source
            .contains("0xA85D_F621_DCE0_409F_8D39_A447_EA08_07FF")
    );
    assert!(output.source.contains("raidId"));
    assert!(
        output
            .source
            .contains("wire_shape: Some(NetworkWireShape::U64)")
    );
    assert!(output.source.contains("unknown_type_indices"));

    let state_output =
        NetworkRustEmitter::emit_replicated_states(&schema, [28]).expect("state source");

    assert_eq!(state_output.report.state_generation_plan_count, 1);
    assert_eq!(state_output.report.generatable_state_count, 1);
    assert_eq!(state_output.report.blocked_state_count, 0);
    assert!(
        state_output
            .source
            .contains("pub mod raid_data_component_replicated_state")
    );
    assert!(
        state_output
            .source
            .contains("pub struct RaidDataComponentReplicatedState")
    );
    assert!(state_output.source.contains("pub raid_id:"));
    assert!(state_output.source.contains("#[replicated_state]"));
    assert!(!state_output.source.contains("Default, ReplicatedState"));
    assert!(
        !state_output
            .source
            .contains("pub hub: ::nw_network::hub::ReplicatedState")
    );
    assert!(
        state_output
            .source
            .contains("#[az_rtti(\"A85DF621-DCE0-409F-8D39-A447EA0807FF\")]")
    );
    assert!(state_output.source.contains("type_registry"));
    assert!(state_output.source.contains("28"));
    assert!(
        state_output
            .source
            .contains("pub use raid_data_component_replicated_state")
    );

    let unregistered_state_output = NetworkRustEmitter::emit_replicated_states_with_options(
        &schema,
        [28],
        NetworkReplicatedStateEmitOptions::unregistered(),
    )
    .expect("unregistered state source");

    assert!(
        unregistered_state_output
            .source
            .contains("pub struct RaidDataComponentReplicatedState")
    );
    assert!(
        unregistered_state_output
            .source
            .contains("impl ::nw_network::types::TypeRegistryEntry")
    );
    assert!(!unregistered_state_output.source.contains("#[type_registry"));
    assert!(
        !unregistered_state_output
            .source
            .contains("AzRtti, ReplicatedState, TypeRegistry")
    );
}
#[test]
fn emits_unnamed_registry_entries_as_descriptors() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "6C735DB3-871C-4762-A02C-1DA6B5DAB7E9",
            "typeIndex": 67
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

    assert_eq!(output.report.descriptor_count, 1);
    assert_eq!(output.report.identity_type_count, 0);
    assert_eq!(output.report.unnamed_descriptor_count, 1);
    assert_eq!(output.report.skipped_missing_name, 0);
    assert!(output.source.contains("type_index: 67"));
    assert!(output.source.contains("name: None"));
}

#[test]
fn emits_message_unmarshal_fields_as_descriptors() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 19,
            "typeName": "RegistrationRequestV3Msg",
            "messageUnmarshal": {
                "createInstance": "NewWorld+0x7ce840",
                "instanceSize": "0x470",
                "instanceSizeSource": "create-instance-operator-new"
            },
            "fields": [{
                "index": 0,
                "name": "StatusCode",
                "nativeType": "u32",
                "storageOffset": "0x8",
                "wireShape": "u32",
                "wireShapeSource": "message-unmarshal-pcode-output-storage",
                "confidence": "message-unmarshal-call"
            }, {
                "index": 2,
                "name": "ServerVersion",
                "nativeType": "AZStd::string",
                "storageOffset": "0xa0",
                "wireShape": "string",
                "wireShapeSource": "message-unmarshal-pcode-output-storage",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

    assert_eq!(output.report.descriptor_count, 1);
    assert_eq!(output.report.message_count, 1);
    assert_eq!(output.report.field_registered_count, 0);
    assert_eq!(output.report.field_descriptor_count, 2);
    assert_eq!(output.report.field_wire_shape_count, 2);
    assert!(
        output
            .source
            .contains("pub struct RegistrationRequestV3Msg")
    );
    assert!(output.source.contains("native_type: Some(\"u32\")"));
    assert!(output.source.contains("storage_offset: Some(8u32)"));
    assert!(output.source.contains("instance_size: Some(1136u32)"));
    assert!(
        output
            .source
            .contains("native_type: Some(\"AZStd::string\")")
    );
    assert!(
        output
            .source
            .contains("wire_shape: Some(NetworkWireShape::String)")
    );

    let message_output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(message_output.report.message_generation_plan_count, 1);
    assert_eq!(message_output.report.generatable_message_count, 1);
    assert_eq!(message_output.report.blocked_message_count, 0);
    assert!(
        message_output
            .source
            .contains("pub mod registration_request_v3_msg")
    );
    assert!(
        message_output
            .source
            .contains("pub struct RegistrationRequestV3Msg")
    );
    assert!(message_output.source.contains("pub status_code: u32"));
    assert!(message_output.source.contains("pub server_version: String"));
    assert!(message_output.source.contains("Marshaler"));
    assert!(message_output.source.contains("az_rtti"));
    assert!(message_output.source.contains("type_registry"));
}
