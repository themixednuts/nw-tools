use super::*;

#[test]
fn emits_presence_prefixed_composite_state_field() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "8CA7C6C0-2244-4E78-A55E-E7A8752A5984",
            "typeIndex": 7001,
            "typeName": "MB::OptionalBoundsReplicatedState",
            "capabilities": ["replicated-state"],
            "fields": [{
                "index": 0,
                "name": "forbiddenBounds",
                "group": 0,
                "wireShape": "optional<composite<aabb2d,u8,bool>>",
                "wireShapeSource": "complete-presence-prefixed-optional",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [7001])
        .expect("replicated state source");
    let field = &output.report.state_generation_plans[0].fields[0];

    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("::core::option::Option<(::bevy_math::bounding::Aabb2d, u8, bool)>")
    );
    assert_eq!(
        field.rust_field_type.as_deref(),
        Some(
            "::nw_network::serialize::ReplicatedFieldHandler<::core::option::Option<(::bevy_math::bounding::Aabb2d, u8, bool)>>"
        )
    );
    assert!(output.report.state_generation_plans[0].can_generate);
}

#[test]
fn rejects_replicated_state_with_partially_unknown_field_groups() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "8CA7C6C0-2244-4E78-A55E-E7A8752A5984",
            "typeIndex": 7001,
            "typeName": "MB::GroupedReplicatedState",
            "capabilities": ["replicated-state"],
            "fields": [{
                "index": 0,
                "name": "first",
                "group": 0,
                "wireShape": "u64",
                "confidence": "register-field-call"
            }, {
                "index": 1,
                "name": "second",
                "wireShape": "u64",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [7001])
        .expect("replicated state report");
    let plan = &output.report.state_generation_plans[0];

    assert!(!plan.can_generate);
    assert_eq!(plan.blocked_reasons, ["missing-field-group:1"]);
    assert!(!output.source.contains("pub struct GroupedReplicatedState"));
}

#[test]
fn emits_single_generated_state_module_with_registration_allowlist() {
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
        }, {
            "uuid": "F9E72714-96F5-4092-8F90-136DCB98BDB3",
            "typeIndex": 29,
            "typeName": "Javelin::RaidGroupComponentReplicatedState",
            "fields": [{
                "index": 0,
                "name": "groupId",
                "group": 0,
                "handlerVtable": "NewWorld+0x81dad80",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81dad80",
            "fieldCount": 1,
            "wireShape": "u64",
            "wireShapeSource": "marshal-pcode-fixed-width-structure",
            "slots": []
        }]
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states_with_options(
        &schema,
        [28, 29],
        NetworkReplicatedStateEmitOptions::register_only([28]),
    )
    .expect("allowlisted state source");

    assert_eq!(output.report.generatable_state_count, 2);
    assert!(
        output
            .source
            .contains("pub struct RaidDataComponentReplicatedState")
    );
    assert!(
        output
            .source
            .contains("pub struct RaidGroupComponentReplicatedState")
    );
    assert_eq!(output.source.matches("#[type_registry").count(), 1);
    assert!(output.source.contains("#[type_registry(28u32)]"));
    assert!(!output.source.contains("#[type_registry(29u32)]"));
}

#[test]
fn emits_native_fragment_category_attribute_from_schema_evidence() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "39B4C919-3A6D-46B5-92D0-3B4ACB284B1D",
            "typeIndex": 16,
            "typeName": "MB::ProjectileReplicatedState",
            "constructorMatches": [{
                "fragmentMetadata": {
                    "source": "i-fragment-vtable",
                    "isMetadataSlot": 12,
                    "isMetadataFunction": "NewWorld+0x294910",
                    "isMetadata": false,
                    "categorySlot": 13,
                    "categoryFunction": "NewWorld+0x6840000",
                    "categoryValue": 5,
                    "category": "Projectile"
                },
                "fields": []
            }],
            "fields": [{
                "index": 0,
                "name": "projectileId",
                "group": 0,
                "handlerVtable": "NewWorld+0x81dad80",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81dad80",
            "fieldCount": 1,
            "wireShape": "u32",
            "wireShapeSource": "marshal-pcode-fixed-width-structure",
            "slots": []
        }]
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [16]).expect("state source");
    let plan = &output.report.state_generation_plans[0];
    assert_eq!(plan.fragment_category.as_deref(), Some("Projectile"));
    assert_eq!(plan.fragment_category_value, Some(5));
    assert_eq!(plan.is_metadata_fragment, Some(false));
    assert!(
        output
            .source
            .contains("#[replicated_state(category = \"projectile\")]")
    );
}

#[test]
fn replicated_state_attributes_are_not_emitted_as_normal_fields() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "203DC8C7-0C60-454B-A46F-566114314B84",
            "typeIndex": 10,
            "typeName": "MB::GdeMetadataReplicatedState",
            "fields": [{
                "index": 0,
                "name": "AssetId",
                "group": 0,
                "registrationKind": "field",
                "handlerVtable": "NewWorld+0x8041098",
                "confidence": "fixed-field-table-append"
            }, {
                "index": 1,
                "name": "ReplicationCategory",
                "registrationKind": "attribute",
                "handlerVtable": "NewWorld+0x8041028",
                "confidence": "fixed-attribute-table-append"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8041098",
            "fieldCount": 1,
            "wireShape": "u32",
            "wireShapeSource": "marshal-pcode-fixed-width-structure",
            "slots": []
        }, {
            "address": "NewWorld+0x8041028",
            "fieldCount": 1,
            "wireShape": "u8",
            "wireShapeSource": "marshal-pcode-fixed-width-structure",
            "slots": []
        }]
    }))
    .expect("schema");

    assert_eq!(
        schema.types[0].fields[1].registration_kind.as_deref(),
        Some("attribute")
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [10]).expect("state source");
    let plan = &output.report.state_generation_plans[0];
    assert_eq!(plan.field_count, 1);
    assert_eq!(plan.attribute_count, 1);
    assert!(output.source.contains("pub asset_id:"));
    assert!(!output.source.contains("pub replication_category:"));
}

#[test]
fn disambiguates_repeated_replicated_state_field_labels_by_field_index() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "01B0664B-3AB6-44A6-87E3-8C69D40E0365",
            "typeIndex": 11,
            "typeName": "MB::ALCReplicatedState",
            "capabilities": ["replicated-state"],
            "fields": [{
                "index": 0,
                "name": "Value",
                "group": 0,
                "wireShape": "u8",
                "confidence": "fixed-field-table-append"
            }, {
                "index": 1,
                "name": "Value",
                "group": 0,
                "wireShape": "u8",
                "confidence": "fixed-field-table-append"
            }, {
                "index": 2,
                "name": "Value",
                "group": 0,
                "wireShape": "u8",
                "confidence": "fixed-field-table-append"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [11]).expect("state source");
    let plan = &output.report.state_generation_plans[0];

    assert_eq!(plan.fields[0].field_name.as_deref(), Some("Value"));
    assert_eq!(plan.fields[1].field_name.as_deref(), Some("Value_1"));
    assert_eq!(plan.fields[2].field_name.as_deref(), Some("Value_2"));
    assert!(output.source.contains("pub value:"));
    assert!(output.source.contains("pub value_1:"));
    assert!(output.source.contains("pub value_2:"));
}

#[test]
fn emits_fixed_byte_replicated_field_handlers() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "B8B8D08F-3AC4-47E9-8B1A-AD3704D0E001",
            "typeIndex": 702,
            "typeName": "Javelin::GameModeParticipantReplicatedState",
            "fields": [{
                "index": 0,
                "name": "flags",
                "group": 0,
                "handlerVtable": "NewWorld+0x81b6eb8",
                "confidence": "register-field-call"
            }, {
                "index": 1,
                "name": "groupActivityEligibility",
                "group": 0,
                "handlerVtable": "NewWorld+0x80b9830",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81b6eb8",
            "fieldCount": 1,
            "wireShape": "fixed-bytes-6",
            "wireShapeSource": "marshal-raw-write-length",
            "slots": []
        }, {
            "address": "NewWorld+0x80b9830",
            "fieldCount": 1,
            "wireShape": "fixed-bytes-16",
            "wireShapeSource": "marshal-raw-write-length",
            "slots": []
        }]
    }))
    .expect("schema");

    let descriptor_output =
        NetworkRustEmitter::emit_descriptors(&schema).expect("descriptor source");

    assert_eq!(descriptor_output.report.field_wire_shape_count, 2);
    assert!(
        descriptor_output
            .source
            .contains("NetworkWireShape::FixedBytes(6")
    );
    assert!(
        descriptor_output
            .source
            .contains("NetworkWireShape::FixedBytes(16")
    );

    let state_output =
        NetworkRustEmitter::emit_replicated_states(&schema, [702]).expect("state source");

    assert_eq!(state_output.report.generatable_state_count, 1);
    assert!(
        state_output
            .source
            .contains("pub flags: ::nw_network::serialize::ReplicatedFieldHandler<[u8; 6]>")
    );
    assert!(
        state_output
            .source
            .contains("pub group_activity_eligibility:")
    );
    assert!(state_output.source.contains("[u8; 16]"));
}

#[test]
fn replicated_state_rust_type_override_wraps_value_type() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
            "typeIndex": 3362,
            "typeName": "MB::SlayerScriptReplicatedState",
            "fields": [{
                "index": 0,
                "name": "curScriptStateId",
                "group": 0,
                "nativeType": "AZ::s8",
                "rustType": "i8",
                "wireShape": "u8",
                "wireShapeSource": "source:field-override",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(plan.missing_wire_shape_count, 0);
    assert_eq!(plan.fields[0].rust_value_type.as_deref(), Some("i8"));
    assert_eq!(
        plan.fields[0].rust_field_type.as_deref(),
        Some("::nw_network::serialize::ReplicatedFieldHandler<i8>")
    );
    assert!(
        output.source.contains(
            "pub cur_script_state_id: ::nw_network::serialize::ReplicatedFieldHandler<i8>"
        )
    );
}

#[test]
fn replicated_state_rust_type_override_can_be_complete_field_type() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::SlayerScriptReplicatedState",
                "fields": [{
                    "index": 2,
                    "name": "spawnedEntityIdsBySpawnerId",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81bf3d0",
                    "nativeType": "MB::ReplicatedMapFieldHandler<AZ::Crc32, AZ::EntityId>",
                    "rustType": "::nw_network::serialize::ReplicatedContainer<::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>, { ::nw_network::serialize::WIRE_VEC_CAP }, ::nw_network::serialize::DefaultMarshaler<::nw_network::Crc32>, ::nw_network::serialize::DefaultMarshaler<::nw_network::EntityId>>",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81bf3d0",
                "fieldCount": 1,
                "wireShape": "vlq-u32",
                "wireShapeSource": "marshal-call:ambiguous-container-helper",
                "slots": []
            }]
        }))
        .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(plan.shaped_field_count, 1);
    assert_eq!(plan.missing_wire_shape_count, 0);
    assert_eq!(plan.fields[0].wire_shape, None);
    assert_eq!(plan.fields[0].rust_value_type, None);
    assert_eq!(
        plan.fields[0].rust_field_type.as_deref(),
        Some(
            "::nw_network::serialize::ReplicatedContainer<::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>, { ::nw_network::serialize::WIRE_VEC_CAP }, ::nw_network::serialize::DefaultMarshaler<::nw_network::Crc32>, ::nw_network::serialize::DefaultMarshaler<::nw_network::EntityId>>"
        )
    );
    assert!(output.source.contains("ReplicatedContainer"));
    assert!(!output.source.contains("ReplicatedMap<"));
    assert!(output.source.contains("IndexMap"));
    assert!(output.source.contains("::nw_network::Crc32"));
    assert!(output.source.contains("::nw_network::EntityId"));
}

#[test]
fn selected_struct_container_shape_mismatch_stays_blocked() {
    let value_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
            "typeIndex": 3362,
            "typeName": "MB::StructuredMapReplicatedState",
            "fields": [{
                "index": 0,
                "name": "valuesById",
                "group": 0,
                "handlerVtable": "NewWorld+0x81bf3d0",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81bf3d0",
            "fieldCount": 1,
            "deltaMarshalShapes": ["vlq-u32", "u32", "sequence-number", "u8", "u64"],
            "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u8", "u64"],
            "valueTypeInfo": {
                "name": "ExampleValue",
                "typeId": value_type_id.to_string(),
                "source": "unmarshal-full-element-vptr+native-size",
                "nativeSize": "0x20",
                "nativeSizeSource": "serialize-field-data-size"
            },
            "slots": []
        }]
    }))
    .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![example_value_item(
                value_type_id,
                [ScalarType::U64, ScalarType::U8],
            )],
        },
        Some("selection.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
    let plan = &output.report.state_generation_plans[0];
    let field = &plan.fields[0];

    assert!(!plan.can_generate);
    assert_eq!(plan.blocked_reasons, vec!["missing-semantic-type:1"]);
    assert_eq!(field.serialize_type_name.as_deref(), Some("ExampleValue"));
    assert_eq!(field.rust_field_type, None);
    assert_eq!(
        field.blocked_reason.as_deref(),
        Some("missing-semantic-type")
    );
}

#[test]
fn matching_provider_candidate_remains_diagnostic() {
    let recipe_cooldown_id = uuid!("022d0c83-ee04-4e4d-9776-4dfbdaa90923");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
            "typeIndex": 3362,
            "typeName": "MB::CraftingComponentReplicatedState",
            "fields": [{
                "index": 0,
                "name": "recipeCooldowns",
                "group": 0,
                "handlerVtable": "NewWorld+0x81bf3d0",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81bf3d0",
            "fieldCount": 1,
            "deltaMarshalShapes": ["vlq-u32", "u32", "sequence-number", "u8", "u64"],
            "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u8", "u64"],
            "valueTypeInfoCandidates": [{
                "address": "NewWorld+0x8123450",
                "name": "RecipeCooldownData",
                "typeId": recipe_cooldown_id.to_string(),
                "source": "rtti-provider-vtable"
            }],
            "slots": []
        }]
    }))
    .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![named_value_item(
                recipe_cooldown_id,
                "RecipeCooldownData",
                [ScalarType::U8, ScalarType::U64],
            )],
        },
        Some("selection.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
    let plan = &output.report.state_generation_plans[0];
    let field = &plan.fields[0];

    assert!(!plan.can_generate);
    assert_eq!(plan.blocked_reasons, vec!["missing-semantic-type:1"]);
    assert_eq!(field.rust_value_type, None);
    assert_eq!(field.rust_field_type, None);
}

#[test]
fn direct_type_labels_do_not_authorize_container_semantics() {
    let task_id = uuid!("e1838273-034d-47fb-b535-95ff1d52d8ee");
    let time_id = uuid!("24fbf222-8cf9-4539-b313-34726b8fc675");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "AEFEDE43-4D48-42ED-81F8-7FF1E8D4D120",
            "typeIndex": 3857,
            "typeName": "Javelin::ObjectivesComponentReplicatedState",
            "fields": [{
                "index": 0,
                "name": "taskStartTimes",
                "group": 0,
                "handlerVtable": "NewWorld+0x8258560",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8258560",
            "fieldCount": 1,
            "keyNativeType": "ObjectiveTaskInstanceId",
            "keyNativeTypeSource": "handler-constructor-template",
            "valueTypeId": time_id.to_string(),
            "valueTypeName": "WallClockTimePoint",
            "valueTypeSource": "full-direct-marshal-template",
            "valueTypeNameSource": "marshal-function-template",
            "deltaMarshalShapes": [
                "vlq-u32",
                "vlq-u32",
                "u8",
                "u64",
                "u8",
                "sequence-number",
                "u8",
                "u64",
                "u8",
                "u64",
                "u8",
                "sequence-number",
                "sequence-number",
                "vlq-u32"
            ],
            "fullMarshalShapes": [
                "sequence-number",
                "vlq-u32",
                "u64",
                "u8",
                "u64",
                "vlq-u32"
            ],
            "valueTypeInfoCandidates": [{
                "address": "NewWorld+0x802f940",
                "name": "WallClockTimePoint",
                "typeId": time_id.to_string(),
                "source": "rtti-provider-vtable"
            }, {
                "address": "NewWorld+0x80cb690",
                "name": "ObjectiveTaskInstanceId",
                "typeId": task_id.to_string(),
                "source": "rtti-provider-vtable"
            }],
            "slots": []
        }]
    }))
    .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![
                named_value_item(
                    task_id,
                    "ObjectiveTaskInstanceId",
                    [ScalarType::U64, ScalarType::U8],
                ),
                named_value_item(time_id, "WallClockTimePoint", [ScalarType::U64]),
            ],
        },
        Some("selection.json".to_owned()),
    );
    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3857]).expect("state source");
    let plan = &output.report.state_generation_plans[0];
    let field = &plan.fields[0];

    assert!(!plan.can_generate);
    assert_eq!(plan.blocked_reasons, vec!["missing-semantic-type:1"]);
    assert_eq!(field.rust_value_type, None);
    assert_eq!(field.rust_field_type, None);
}

#[test]
fn selected_structured_candidate_with_partial_delta_stays_blocked() {
    let value_type_id = uuid!("0dc02dd0-993e-48c0-8b60-5715d4383b0d");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "111AEBB0-4F23-4914-B732-A349CCBD82D4",
            "typeIndex": 3780,
            "typeName": "Javelin::GlobalMapDataManagerComponentReplicatedState",
            "fields": [{
                "index": 0,
                "name": "globalMapData",
                "group": 0,
                "handlerVtable": "NewWorld+0x8223838",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x8223838",
            "fieldCount": 1,
            "deltaMarshalShapes": ["vlq-u32", "u64", "sequence-number", "u8"],
            "fullMarshalShapes": [
                "sequence-number",
                "vlq-u32",
                "u64",
                "vec2",
                "u16",
                "u32"
            ],
            "valueTypeInfo": {
                "name": "GlobalMapData",
                "typeId": value_type_id.to_string(),
                "source": "unmarshal-full-element-vptr+native-size",
                "nativeSize": "0x20",
                "nativeSizeSource": "serialize-field-data-size"
            },
            "slots": []
        }]
    }))
    .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![named_value_item(
                value_type_id,
                "GlobalMapData",
                [ScalarType::Vector2, ScalarType::U16, ScalarType::U32],
            )],
        },
        Some("selection.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3780]).expect("state source");
    let plan = &output.report.state_generation_plans[0];
    let field = &plan.fields[0];

    assert!(!plan.can_generate);
    assert_eq!(plan.blocked_reasons, vec!["missing-semantic-type:1"]);
    assert_eq!(field.rust_value_type, None);
    assert_eq!(field.rust_field_type, None);
}

#[test]
fn source_vector_candidate_does_not_authorize_container_type() {
    let persistent_item_data_id = uuid!("1be36174-fd4f-4a1c-8e52-7c28d50eec5a");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "393D9FE0-8E0F-41E9-8FE0-A2C33EF9C7C2",
                "typeIndex": 2938,
                "typeName": "MB::GlobalStorageComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "m_globalItemMap",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x813bb88",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x813bb88",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "fixed-bytes-16",
                    "sequence-number",
                    "vlq-u32",
                    "u64"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "fixed-bytes-16",
                    "vlq-u32",
                    "u64"
                ],
                "valueTypeShape": {
                    "typeName": "AZStd::vector<PersistentItemData>",
                    "typeNameFull": "AZStd::vector<PersistentItemData>",
                    "typeNameSource": "marshal-helper-callgraph",
                    "memberNameSource": "container-value-shape",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-serialize-type-sequence-persistent-item-data-vector",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x0",
                        "name": "items",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZStd::vector<PersistentItemData>",
                        "wireShape": "vec<PersistentItemData>",
                        "evidenceSource": "persistent-item-vector-container-slot"
                    }]
                },
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x9f34228",
                    "name": "PersistentItemData",
                    "typeId": persistent_item_data_id.to_string(),
                    "source": "serialize-registration+marshal-helper-callgraph"
                }],
                "slots": []
            }]
        }))
        .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![named_value_item::<0>(
                persistent_item_data_id,
                "PersistentItemData",
                [],
            )],
        },
        Some("selection.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [2938]).expect("state source");
    let plan = &output.report.state_generation_plans[0];
    let field = &plan.fields[0];

    assert!(!plan.can_generate, "{plan:#?}");
    assert_eq!(plan.blocked_reasons, vec!["missing-semantic-type:1"]);
    assert_eq!(field.rust_value_type, None);
    assert_eq!(field.rust_field_type, None);
}

#[test]
fn ambiguous_provider_value_type_shape_matches_stay_blocked() {
    let first_type_id = uuid!("022d0c83-ee04-4e4d-9776-4dfbdaa90923");
    let second_type_id = uuid!("80a9e3d4-2cf6-44b1-b05e-c44a6f36b5db");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
            "typeIndex": 3362,
            "typeName": "MB::CraftingComponentReplicatedState",
            "fields": [{
                "index": 0,
                "name": "recipeCooldowns",
                "group": 0,
                "handlerVtable": "NewWorld+0x81bf3d0",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81bf3d0",
            "fieldCount": 1,
            "deltaMarshalShapes": ["vlq-u32", "u32", "sequence-number", "u8", "u64"],
            "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u8", "u64"],
            "valueTypeInfoCandidates": [{
                "address": "NewWorld+0x8123450",
                "name": "RecipeCooldownData",
                "typeId": first_type_id.to_string(),
                "source": "rtti-provider-vtable"
            }, {
                "address": "NewWorld+0x8123460",
                "name": "OtherCooldownData",
                "typeId": second_type_id.to_string(),
                "source": "rtti-provider-vtable"
            }],
            "slots": []
        }]
    }))
    .expect("schema");
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![
                named_value_item(
                    first_type_id,
                    "RecipeCooldownData",
                    [ScalarType::U8, ScalarType::U64],
                ),
                named_value_item(
                    second_type_id,
                    "OtherCooldownData",
                    [ScalarType::U8, ScalarType::U64],
                ),
            ],
        },
        Some("selection.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
    let plan = &output.report.state_generation_plans[0];
    let field = &plan.fields[0];

    assert!(!plan.can_generate);
    assert_eq!(plan.blocked_reasons, vec!["missing-semantic-type:1"]);
    assert_eq!(field.rust_field_type, None);
}

#[test]
fn reports_selected_replicated_states_that_cannot_be_generated() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
            "typeIndex": 28,
            "typeName": "Javelin::RaidDataComponentReplicatedState",
            "fields": []
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output =
        NetworkRustEmitter::emit_replicated_states(&schema, [28, 29]).expect("state source");

    assert_eq!(output.report.state_generation_plan_count, 2);
    assert_eq!(output.report.generatable_state_count, 0);
    assert_eq!(output.report.blocked_state_count, 2);
    assert_eq!(
        output.report.state_generation_plans[0].blocked_reasons,
        vec!["no-registered-fields"]
    );
    assert_eq!(
        output.report.state_generation_plans[1].blocked_reasons,
        vec!["missing-network-type"]
    );
    assert!(
        !output
            .source
            .contains("pub struct RaidDataComponentReplicatedState")
    );
}
