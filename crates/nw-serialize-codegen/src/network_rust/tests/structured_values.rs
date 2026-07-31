use super::*;

#[test]
fn exact_scalar_rtti_shape_uses_the_scalar_without_a_support_struct() {
    let u64_id = uuid!("d6597933-47cd-4fc8-b911-63f3e2b0993a");
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "4F70C3BB-8F7D-48C2-A0B6-95431F88F356",
            "typeIndex": 2768,
            "typeName": "MB::PlayerHousingReplicatedState",
            "fields": [{
                "index": 5,
                "name": "m_phasedHousingPlotEntityId",
                "group": 0,
                "sourceTypeId": u64_id.to_string(),
                "sourceTypeIdSource": "ghidra-direct-unmarshal-value-type",
                "sourceTypeIdentityProven": true,
                "handlerVtable": "NewWorld+0x80b8678",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x80b8678",
            "fieldCount": 1,
            "wireShape": "u64",
            "wireShapeSource": "marshal+unmarshal-pcode-agreement",
            "valueTypeShape": {
                "typeId": u64_id.to_string(),
                "typeIdSource": "ghidra-direct-unmarshal-value-type",
                "identityProven": true,
                "identitySource": "ghidra-direct-unmarshal-value-type",
                "typeName": "u64",
                "typeNameFull": "AZ::u64",
                "memberNameSource": "synthetic-layout-member",
                "memberNamesProven": false,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "members": [{
                    "index": 0,
                    "offset": "0x0",
                    "nativeOffset": "0x0",
                    "name": "field_0",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "nativeType": "u64",
                    "wireLayout": "u64",
                    "wireOrdinal": 0
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [2768])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_field_type.as_deref(),
        Some("ReplicatedFieldHandler<u64>")
    );
    assert!(!output.source.contains("pub struct U64"));
    assert!(!output.source.contains("U64Marshaler"));
}

#[test]
fn emits_proven_remote_server_gde_ref_field_codec() {
    let remote_ref_id = uuid!("17207dac-730b-4793-b975-23591a950260");
    let context_id = uuid!("5bedcb9d-f3be-4300-80af-b4e17a7f4646");
    let gde_id = uuid!("07ce17ba-c4b7-4b42-81c1-79af6a61f9a5");
    let uid_id = uuid!("3485f20a-98c0-5315-876b-21bcd23a7bc0");
    let u64_id = uuid!("d6597933-47cd-4fc8-b911-63f3e2b0993a");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
            "typeIndex": 1292,
            "typeName": "MB::CampingComponentReplicatedState",
            "fields": [{
                "index": 0,
                "name": "currentCampRef",
                "group": 0,
                "handlerVtable": "NewWorld+0x818cee8",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x818cee8",
            "fieldCount": 1,
            "wireShape": "remote-server-gde-ref",
            "wireShapeSource": "handler-remote-server-gde-ref-pcode",
            "valueTypeInfo": {
                "address": "NewWorld+0x7fc4ef0",
                "name": "RemoteServerGDERef",
                "typeId": remote_ref_id.to_string(),
                "source": "handler-payload-flow+constructor-vptr+exact-serialize-layout"
            },
            "valueTypeShape": {
                "typeId": remote_ref_id.to_string(),
                "typeIdSource": "handler-constructor-value-vptr",
                "identityProven": true,
                "identitySource": "handler-payload-flow+constructor-vptr+exact-serialize-layout",
                "typeName": "RemoteServerGDERef",
                "typeNameFull": "RemoteServerGDERef",
                "memberNameSource": "serialize-field-for-proven-type",
                "memberNamesProven": true,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "members": [{
                    "index": 0,
                    "offset": "0x10",
                    "nativeOffset": "0x20",
                    "name": "m_remoteServerContext.m_actorId",
                    "nameSource": "serialize-field-for-proven-type",
                    "nameProven": true,
                    "typeId": uid_id.to_string(),
                    "typeIdSource": "serialize-field-for-proven-type",
                    "typeIdentityProven": true,
                    "wireLayout": "fixed-bytes-16",
                    "wireOrdinal": 0
                }, {
                    "index": 1,
                    "offset": "0x20",
                    "nativeOffset": "0x30",
                    "name": "m_targetId.id",
                    "nameSource": "serialize-field-for-proven-type",
                    "nameProven": true,
                    "typeId": u64_id.to_string(),
                    "typeIdSource": "serialize-field-for-proven-type",
                    "typeIdentityProven": true,
                    "wireLayout": "u64",
                    "wireOrdinal": 1
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![
                serialize_struct(
                    remote_ref_id,
                    "RemoteServerGDERef",
                    vec![
                        serialize_field(
                            "m_remoteServerContext",
                            context_id,
                            ResolvedType::Named {
                                type_id: context_id,
                                source_name: "RemoteServerContextRef".to_owned(),
                            },
                            8,
                        ),
                        serialize_field(
                            "m_targetId",
                            gde_id,
                            ResolvedType::Named {
                                type_id: gde_id,
                                source_name: "GDEID".to_owned(),
                            },
                            32,
                        ),
                    ],
                ),
                serialize_struct(
                    context_id,
                    "RemoteServerContextRef",
                    vec![serialize_field(
                        "m_actorId",
                        uid_id,
                        ResolvedType::Uid {
                            type_id: Some(uid_id),
                        },
                        8,
                    )],
                ),
                serialize_struct(
                    gde_id,
                    "GDEID",
                    vec![serialize_field(
                        "id",
                        u64_id,
                        ResolvedType::Scalar(ScalarType::U64),
                        0,
                    )],
                ),
            ],
        },
        Some("selection.json".to_owned()),
    );

    assert_eq!(
        schema.field_handler_vtables[0].wire_shape,
        Some(crate::NetworkWireShape::RemoteServerGdeRef)
    );
    assert_eq!(schema.types[0].fields[0].rust_type, None);

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [1292])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];
    let field = &plan.fields[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("::nw_network::source::RemoteServerGDERef")
    );
    assert_eq!(
        field.rust_field_type.as_deref(),
        Some(
            "ReplicatedFieldHandler<::nw_network::source::RemoteServerGDERef, RemoteServerGdeRefMarshaler>"
        )
    );
    assert!(
        output
            .source
            .contains("::nw_network::serialize::RemoteServerGdeRefMarshaler")
    );
}

#[test]
fn emits_exact_rtti_only_value_from_generated_member_types() {
    let ability_data_id = uuid!("54d2722e-8cc5-43eb-a030-3a9669790e01");
    let time_point_id = uuid!("0989a3e9-37e8-4381-8766-b208f460c1a3");
    let remote_ref_id = uuid!("17207dac-730b-4793-b975-23591a950260");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "A7053315-11BD-4E19-9EE4-63CB93E8B216",
            "typeIndex": 2267,
            "typeName": "MB::AbilityInstanceTrackingComponentReplicatedState",
            "fields": [{
                "index": 0,
                "name": "m_abilityInstances",
                "group": 0,
                "handlerVtable": "NewWorld+0x8091a70",
                "sourceTypeId": ability_data_id.to_string(),
                "sourceTypeIdSource": "handler-constructor-value-rtti",
                "sourceTypeIdentityProven": true,
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8091a70",
            "fieldCount": 55,
            "valueTypeShape": {
                "typeId": ability_data_id.to_string(),
                "typeIdSource": "handler-constructor-value-rtti",
                "identityProven": true,
                "identitySource": "handler-constructor-value-rtti",
                "typeName": "AbilityInstanceReplicatedData",
                "typeNameFull": "AbilityInstanceReplicatedData",
                "memberNameSource": "exact-member-type-name",
                "memberNamesProven": false,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "members": [{
                    "index": 0,
                    "offset": "0x8",
                    "name": "timePoint",
                    "nameSource": "exact-member-type-name",
                    "nameProven": false,
                    "typeId": time_point_id.to_string(),
                    "typeIdSource": "handler-constructor-member-rtti",
                    "typeIdentityProven": true,
                    "wireLayout": "u64",
                    "wireOrdinal": 0
                }, {
                    "index": 1,
                    "offset": "0x18",
                    "name": "remoteServerGDERef",
                    "nameSource": "exact-member-type-name",
                    "nameProven": false,
                    "typeId": remote_ref_id.to_string(),
                    "typeIdSource": "handler-constructor-member-rtti",
                    "typeIdentityProven": true,
                    "wireLayout": "composite<fixed-bytes-16,u64>",
                    "wireOrdinal": 1
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![
                serialize_struct(time_point_id, "TimePoint", Vec::new()),
                serialize_struct(remote_ref_id, "RemoteServerGDERef", Vec::new()),
            ],
        },
        Some("selection.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [2267])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];
    let field = &plan.fields[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("AbilityInstanceReplicatedData")
    );
    assert!(
        output
            .source
            .contains("pub struct AbilityInstanceReplicatedData")
    );
    assert!(
        output
            .source
            .contains("pub time_point: ::nw_network::source::TimePoint")
    );
    assert!(
        output
            .source
            .contains("pub remote_server_gde_ref: ::nw_network::source::RemoteServerGDERef")
    );
}

#[test]
fn emits_anonymous_value_from_proven_subobjects_and_scalar_slots() {
    let player_id = uuid!("6af02f6b-b58c-4f66-bff6-16868eaa5f78");
    let guild_id = uuid!("0252597e-4d49-49d3-a0a3-4169106bbaba");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "89C55D6C-722A-4646-B8F9-FD71184059DD",
            "typeIndex": 1880,
            "typeName": "MB::TerritoryInteractorReplicatedState",
            "fields": [{
                "index": 1,
                "name": "replicatedRosterPlayer",
                "group": 0,
                "handlerVtable": "NewWorld+0x8383278",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8383278",
            "fieldCount": 50,
            "valueTypeShape": {
                "identityProven": false,
                "layoutProven": true,
                "typeName": "Value",
                "typeNameSource": "synthetic-anonymous-composite",
                "memberNameSource": "synthetic-layout-member",
                "memberNamesProven": false,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "nativeSize": 144,
                "members": [{
                    "index": 0,
                    "offset": "0x0",
                    "name": "simple_player_identification",
                    "nameSource": "synthetic-provider-type",
                    "nameProven": false,
                    "typeId": player_id.to_string(),
                    "typeIdSource": "constructor-stored-az-rtti-provider",
                    "typeIdentityProven": true,
                    "wireShape": "composite<string,string,u8>",
                    "wireOrdinal": 0
                }, {
                    "index": 1,
                    "offset": "0x60",
                    "name": "guild_id",
                    "nameSource": "synthetic-provider-type",
                    "nameProven": false,
                    "typeId": guild_id.to_string(),
                    "typeIdSource": "constructor-stored-az-rtti-provider",
                    "typeIdentityProven": true,
                    "wireShape": "fixed-bytes-16",
                    "wireOrdinal": 1
                }, {
                    "index": 2,
                    "offset": "0x80",
                    "name": "field_4",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "u16",
                    "wireOrdinal": 2
                }, {
                    "index": 3,
                    "offset": "0x82",
                    "name": "field_5",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "u8",
                    "wireOrdinal": 3
                }, {
                    "index": 4,
                    "offset": "0x83",
                    "name": "field_6",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "bool",
                    "wireOrdinal": 4
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![
                serialize_struct(player_id, "SimplePlayerIdentification", Vec::new()),
                serialize_struct(guild_id, "GuildId", Vec::new()),
            ],
        },
        Some("selection.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [1880])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];
    let field = &plan.fields[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("ReplicatedRosterPlayerValue")
    );
    assert!(
        output
            .source
            .contains("pub struct ReplicatedRosterPlayerValue")
    );
    assert!(output.source.contains(
        "pub simple_player_identification: ::nw_network::source::SimplePlayerIdentification"
    ));
    assert!(
        output
            .source
            .contains("pub guild_id: ::nw_network::source::GuildId")
    );
    assert!(output.source.contains("pub field_4: u16"));
    assert!(output.source.contains("pub field_5: u8"));
    assert!(output.source.contains("pub field_6: bool"));
}

#[test]
fn state_support_value_constructs_non_default_aabb_members() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "89C55D6C-722A-4646-B8F9-FD71184059DE",
            "typeIndex": 1881,
            "typeName": "MB::ForbiddenBoundsReplicatedState",
            "fields": [{
                "index": 0,
                "name": "forbiddenBounds",
                "group": 0,
                "handlerVtable": "NewWorld+0x8383280",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8383280",
            "fieldCount": 1,
            "valueTypeShape": {
                "identityProven": false,
                "layoutProven": true,
                "typeName": "Value",
                "typeNameSource": "synthetic-anonymous-composite",
                "memberNameSource": "synthetic-layout-member",
                "memberNamesProven": false,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "members": [{
                    "index": 0,
                    "offset": "0x0",
                    "name": "field_0",
                    "wireShape": "bool",
                    "wireOrdinal": 0
                }, {
                    "index": 1,
                    "offset": "0x4",
                    "name": "field_1",
                    "wireShape": "aabb2d",
                    "wireOrdinal": 1
                }, {
                    "index": 2,
                    "offset": "0x14",
                    "name": "field_2",
                    "wireShape": "u8",
                    "wireOrdinal": 2
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [1881])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert!(
        output
            .source
            .contains("impl ::core::default::Default for ForbiddenBoundsValue")
    );
    assert!(output.source.contains("min: ::glam::Vec2::ZERO"));
    syn::parse_file(&output.source).expect("generated state source parses");
}

#[test]
fn emits_anonymous_value_from_proven_scalar_storage_window() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "CA8FAA80-0C5F-4BF9-A1A6-7B4C63E12C72",
            "typeIndex": 2343,
            "typeName": "Javelin::GameModeReplicatedState",
            "fields": [{
                "index": 9,
                "name": "Event1",
                "group": 0,
                "handlerVtable": "NewWorld+0x81b80e0",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81b80e0",
            "fieldCount": 10,
            "valueTypeShape": {
                "identityProven": false,
                "layoutProven": true,
                "typeName": "Value",
                "typeNameSource": "synthetic-anonymous-composite",
                "memberNameSource": "synthetic-layout-member",
                "memberNamesProven": false,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "nativeSize": 24,
                "members": [{
                    "index": 0,
                    "offset": "0x0",
                    "name": "field_0",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "u32",
                    "wireOrdinal": 0
                }, {
                    "index": 1,
                    "offset": "0x8",
                    "name": "field_1",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "u64",
                    "wireOrdinal": 1
                }, {
                    "index": 2,
                    "offset": "0x10",
                    "name": "field_2",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "u32",
                    "wireOrdinal": 2
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [2343])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert!(output.source.contains("pub struct Event1Value"));
    assert!(output.source.contains("pub field_0: u32"));
    assert!(output.source.contains("pub field_1: u64"));
    assert!(output.source.contains("pub field_2: u32"));
}

#[test]
fn scalar_field_ignores_matching_anonymous_handler_storage_shape() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "C9F7B060-0F17-4B8F-BAB5-52D494FD24F7",
            "typeIndex": 6951,
            "typeName": "Javelin::PointsAccumulatorComponentReplicatedState",
            "fields": [{
                "index": 2,
                "name": "timeWhenPointsZeroed0",
                "group": 0,
                "handlerVtable": "NewWorld+0x80b86e8",
                "wireShape": "u64",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x80b86e8",
            "fieldCount": 1,
            "wireShape": "u64",
            "valueTypeShape": {
                "identityProven": false,
                "layoutProven": true,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "typeName": "Value",
                "typeNameSource": "synthetic-anonymous-composite",
                "members": [{
                    "index": 0,
                    "offset": "0x0",
                    "name": "field_0",
                    "nameProven": false,
                    "wireLayout": "u64",
                    "wireOrdinal": 0
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [6951])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(plan.fields[0].rust_value_type.as_deref(), Some("u64"));
    assert_eq!(
        plan.fields[0].rust_field_type.as_deref(),
        Some("ReplicatedFieldHandler<u64>")
    );
    assert!(
        !output
            .source
            .contains("pub struct TimeWhenPointsZeroed0Value")
    );
}

#[test]
fn exact_serialize_value_uses_semantic_type_when_wire_products_match() {
    let time_point_id = uuid!("0989a3e9-37e8-4381-8766-b208f460c1a3");
    let u64_id = uuid!("d6597933-47cd-4fc8-b911-63f3e2b0993a");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "63AD3B5A-3E2E-4923-ACCD-1DA221431EE0",
            "typeIndex": 6951,
            "typeName": "Javelin::PointsAccumulatorComponentReplicatedState",
            "fields": [{
                "index": 2,
                "name": "timeWhenPointsZeroed0",
                "group": 0,
                "handlerVtable": "NewWorld+0x80b86e8",
                "sourceTypeId": time_point_id.to_string(),
                "sourceTypeIdSource": "handler-top-level-storage-window+constructor-vptr+az-rtti",
                "sourceTypeIdentityProven": true,
                "wireShape": "u64",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x80b86e8",
            "fieldCount": 1,
            "wireShape": "u64",
            "valueTypeShape": {
                "typeId": time_point_id.to_string(),
                "typeIdSource": "handler-constructor-top-level-value-vptr",
                "identityProven": true,
                "identitySource": "handler-top-level-storage-window+constructor-vptr+az-rtti",
                "typeName": "TimePoint",
                "typeNameFull": "TimePoint",
                "memberNameSource": "synthetic-layout-member",
                "memberNamesProven": false,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "members": [{
                    "index": 0,
                    "offset": "0x8",
                    "name": "field_0",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "u64",
                    "wireOrdinal": 0
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![serialize_struct(
                time_point_id,
                "TimePoint",
                vec![serialize_field(
                    "m_nanosecondsSinceServerStart",
                    u64_id,
                    ResolvedType::Scalar(ScalarType::U64),
                    8,
                )],
            )],
        },
        Some("selection.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [6951])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::source::TimePoint")
    );
    assert_eq!(
        plan.fields[0].rust_field_type.as_deref(),
        Some("::nw_network::serialize::ReplicatedFieldHandler<::nw_network::source::TimePoint>")
    );
    assert!(!output.source.contains("TimePointMarshaler"));
    syn::parse_file(&output.source).expect("generated state source parses");
}

#[test]
fn emits_nested_collection_products_from_proven_wire_shape() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0AFED170-3F75-4E22-B159-14EC9F3B1357",
            "typeIndex": 1880,
            "typeName": "MB::TerritoryInteractorReplicatedState",
            "fields": [{
                "index": 0,
                "name": "territoryGovernanceData",
                "group": 0,
                "handlerVtable": "NewWorld+0x8383278",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8383278",
            "fieldCount": 1,
            "valueTypeShape": {
                "identityProven": false,
                "layoutProven": true,
                "typeName": "Value",
                "typeNameSource": "synthetic-anonymous-composite",
                "memberNameSource": "synthetic-layout-member",
                "memberNamesProven": false,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "members": [{
                    "index": 0,
                    "offset": "0x0",
                    "name": "homogeneous_entries",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "vec<composite<u32,u32>>",
                    "wireOrdinal": 0
                }, {
                    "index": 1,
                    "offset": "0x18",
                    "name": "nested_entries",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "vec<composite<u64,vec<composite<u8,u64,u64,u64>>,u64>>",
                    "wireOrdinal": 1
                }, {
                    "index": 2,
                    "offset": "0x30",
                    "name": "wide_entries",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "vec<composite<u32,u16,u32,fixed-bytes-1,u32,u32,fixed-bytes-1,fixed-bytes-1,vlq-u32,u32,vlq-u32,u32,fixed-bytes-1,vlq-u32,u32>>",
                    "wireOrdinal": 2
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [1880])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert!(
        output
            .source
            .contains("pub homogeneous_entries: ::std::vec::Vec<[u32; 2]>")
    );
    assert!(
        output
            .source
            .contains("pub nested_entries: ::std::vec::Vec<")
    );
    assert!(
        output
            .source
            .contains("(u64, ::std::vec::Vec<(u8, u64, u64, u64)>, u64)")
    );
    assert!(output.source.contains("pub wide_entries: ::std::vec::Vec<"));
    assert!(output.source.contains("(u32, [u8; 1], u32, u32)"));
    assert!(
        output
            .source
            .contains("::nw_network::serialize::SequenceCodec<")
    );
    assert!(
        output
            .source
            .contains("::nw_network::serialize::TupleCodec<")
    );
    assert!(
        output
            .source
            .contains("::nw_network::serialize::VlqU32Marshaler")
    );
}

#[test]
fn emits_registered_value_identity_without_duplicate_field_metadata() {
    let ping_data_id = uuid!("a578ff31-b135-4104-b798-abea15cbd627");
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "DDB5C4F4-745D-448E-AB12-C5EE86B0F796",
            "typeIndex": 3451,
            "typeName": "Javelin::GroupDataComponentReplicatedState",
            "fields": [{
                "index": 0,
                "name": "groupMemberPingData",
                "group": 0,
                "handlerVtable": "NewWorld+0x81ce180",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81ce180",
            "fieldCount": 5,
            "valueTypeShape": {
                "typeId": ping_data_id.to_string(),
                "typeIdSource": "typeregistry-unmarshal-shared-helper",
                "identityProven": true,
                "identitySource": "shared-unmarshal-helper+registry-create-instance-size+handler-storage-window",
                "layoutProven": true,
                "typeName": "ReplicatedPingData",
                "typeNameFull": "Javelin::ReplicatedPingData",
                "memberNameSource": "synthetic-layout-member",
                "memberNamesProven": false,
                "memberCoverageProven": true,
                "wireOrderProven": true,
                "nativeSize": 80,
                "members": [{
                    "index": 0,
                    "offset": "0x0",
                    "name": "field_0",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "u8",
                    "wireOrdinal": 0
                }, {
                    "index": 1,
                    "offset": "0x10",
                    "name": "field_1",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "vec3",
                    "wireOrdinal": 1
                }, {
                    "index": 2,
                    "offset": "0x20",
                    "name": "field_2",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "bool",
                    "wireOrdinal": 2
                }, {
                    "index": 3,
                    "offset": "0x30",
                    "name": "field_3",
                    "nameSource": "synthetic-wire-ordinal",
                    "nameProven": false,
                    "wireShape": "u8",
                    "wireOrdinal": 3
                }]
            },
            "slots": []
        }]
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3451])
        .expect("generated state source");
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert!(output.source.contains("pub struct ReplicatedPingData"));
    assert!(output.source.contains("pub field_1: ::glam::Vec3"));
}

fn serialize_struct(
    type_id: Uuid,
    name: &str,
    fields: Vec<SerializeCodegenField>,
) -> SerializeCodegenItem {
    SerializeCodegenItem {
        source_type_id: type_id,
        source_name: name.to_owned(),
        role: crate::role::ReflectedTypeRole::SupportType,
        is_reflection_marker: false,
        is_abstract: Some(false),
        factory: None,
        rtti_base_chain: Vec::new(),
        kind: SerializeCodegenItemKind::Struct,
        enum_underlying_type: None,
        fields,
        variants: Vec::new(),
    }
}

fn serialize_field(
    name: &str,
    type_id: Uuid,
    resolved_type: ResolvedType,
    offset: u32,
) -> SerializeCodegenField {
    SerializeCodegenField {
        source_name: name.to_owned(),
        source_type_id: type_id,
        resolved_type,
        data_size: None,
        offset: Some(offset),
        flags: None,
        is_base_class: false,
        is_pointer: false,
        is_dynamic_field: false,
    }
}
