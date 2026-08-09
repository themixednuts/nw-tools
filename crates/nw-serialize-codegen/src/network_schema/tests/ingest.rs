use super::*;

#[test]
fn imports_fragment_metadata_from_constructor_matches() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "39B4C919-3A6D-46B5-92D0-3B4ACB284B1D",
            "typeIndex": 16,
            "typeName": "MB::ProjectileReplicatedState",
            "constructorMatches": [{
                "address": "NewWorld+0x683fe00",
                "name": "MB::ProjectileReplicatedState::ProjectileReplicatedState",
                "instanceVtable": "NewWorld+0x8549c70",
                "fragmentMetadata": {
                    "source": "i-fragment-vtable",
                    "isMetadataSlot": 12,
                    "isMetadataFunction": "NewWorld+0x294910",
                    "isMetadata": false,
                    "categorySlot": 13,
                    "categoryFunction": "NewWorld+0x294910",
                    "categoryValue": 0,
                    "category": "Uncategorized"
                },
                "fields": []
            }]
        }],
        "fieldRegistrationFunctions": [{
            "address": "NewWorld+0x683fe00",
            "name": "MB::ProjectileReplicatedState::RegisterFields",
            "instanceVtable": "NewWorld+0x8549c70",
            "fragmentMetadata": {
                "source": "i-fragment-vtable",
                "isMetadataSlot": 12,
                "isMetadataFunction": "NewWorld+0x294910",
                "isMetadata": false,
                "categorySlot": 13,
                "categoryFunction": "NewWorld+0x294910",
                "categoryValue": 0,
                "category": "Uncategorized"
            },
            "fields": []
        }],
        "fieldHandlerVtables": []
    }))
    .expect("schema");

    let metadata = schema.types[0]
        .fragment_metadata
        .as_ref()
        .expect("type fragment metadata");
    assert_eq!(metadata.is_metadata, Some(false));
    assert_eq!(metadata.category_value, Some(0));
    assert_eq!(metadata.category.as_deref(), Some("Uncategorized"));
    assert_eq!(
        metadata.category_function.as_deref(),
        Some("NewWorld+0x294910")
    );

    let function_metadata = schema.field_registration_functions[0]
        .fragment_metadata
        .as_ref()
        .expect("function fragment metadata");
    assert_eq!(function_metadata.category.as_deref(), Some("Uncategorized"));
}

#[test]
fn converts_ghidra_report_to_normalized_network_schema() {
    let report = json!({
        "schema": "newworld.network_schema.static.v1",
        "program": "NewWorld.exe",
        "imageBase": "NewWorld+0x0",
        "input": "E:/Projects/new-world/resources/typeregistry.json",
        "registryEntries": [{
            "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
            "index": 1637,
            "typeIndex": 28,
            "storageAddress": "0x1e0e00aa6c0",
            "baseVtable": "NewWorld+0x84cb580",
            "vtable": "0x1e0e00aa6b0",
            "typeName": "Javelin::RaidDataComponentReplicatedState",
            "typeNameSource": "registrationHook",
            "handler": {
                "Destructor": "NewWorld+0x3495230",
                "GetEmptyValue": "NewWorld+0x3495270",
                "CreateInstance": "NewWorld+0x34952b0",
                "CopyValue": "NewWorld+0x34952c0",
                "Marshal": "NewWorld+0x34952d0",
                "Unmarshal": "NewWorld+0x3495310"
            },
            "azRtti": {
                "source": "instance-vtable",
                "address": "NewWorld+0x81e23a8",
                "typeId": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                "providers": [{
                    "kind": "typeId",
                    "slot": 1,
                    "slotOffset": "0x8",
                    "function": "NewWorld+0x34aa660",
                    "provider": "NewWorld+0x34aa660",
                    "typeId": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                    "typeIdSource": "sourceLiteral",
                    "sourceAddress": "NewWorld+0x81ddfb8"
                }]
            },
            "registrationHook": {
                "typeId": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                "typeName": "Javelin::RaidDataComponentReplicatedState",
                "slotTypeName": "Javelin::RaidDataComponentReplicatedState",
                "hookFunction": "NewWorld+0x15ce50",
                "helperTable": "NewWorld+0x81e03b0",
                "registerThunk": "NewWorld+0x34761e0",
                "typeProvider": "NewWorld+0x34aa660",
                "uuidSource": "NewWorld+0x81ddfb8"
            },
            "fields": [{
                "index": 0,
                "callsite": "NewWorld+0x3495762",
                "name": "raidId",
                "nameAddress": "NewWorld+0x81db5f4",
                "group": 0,
                "handlerExpression": "R15",
                "handlerVtable": "NewWorld+0x81dad80",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [{
            "address": "NewWorld+0x3495550",
            "name": "Javelin::RaidDataComponentReplicatedState::RegisterFields",
            "instanceVtable": "NewWorld+0x81e23a8",
            "azRtti": {
                "source": "instance-vtable",
                "address": "NewWorld+0x81e23a8",
                "typeId": "A85DF621-DCE0-409F-8D39-A447EA0807FF"
            },
            "fields": [{
                "index": 0,
                "callsite": "NewWorld+0x3495762",
                "name": "raidId",
                "group": 0,
                "confidence": "register-field-call"
            }]
        }],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81dad80",
            "fieldCount": 1,
            "marshal": "NewWorld+0x344a700",
            "marshalTarget": "NewWorld+0x17266c0",
            "unmarshal": "NewWorld+0x3464830",
            "wireShape": "u64",
            "wireShapeSource": "marshal-call:marshal-function-name",
            "slots": [{
                "slot": 5,
                "slotOffset": "0x28",
                "name": "Marshal",
                "address": "NewWorld+0x344a700",
                "target": "NewWorld+0x17266c0"
            }, {
                "slot": 6,
                "slotOffset": "0x30",
                "name": "Unmarshal",
                "address": "NewWorld+0x3464830"
            }]
        }]
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

    assert_eq!(schema.schema, NETWORK_SCHEMA_VERSION);
    assert_eq!(
        schema.sources[0].path.as_deref(),
        Some("E:/Projects/new-world/resources/typeregistry.json")
    );
    assert_eq!(
        schema.sources[0].schema.as_deref(),
        Some(NETWORK_STATIC_REPORT_SCHEMA_VERSION)
    );
    assert_eq!(schema.summary.type_count, 1);
    assert_eq!(schema.summary.register_field_function_count, 1);
    assert_eq!(schema.summary.register_field_count, 1);
    assert_eq!(schema.summary.high_confidence_field_count, 1);
    assert_eq!(schema.summary.field_handler_vtable_count, 1);

    let network_type = &schema.types[0];
    assert_eq!(
        network_type.type_id,
        Some(uuid!("a85df621-dce0-409f-8d39-a447ea0807ff"))
    );
    assert_eq!(network_type.type_index, Some(28));
    assert_eq!(
        network_type.name.as_deref(),
        Some("Javelin::RaidDataComponentReplicatedState")
    );
    assert_eq!(network_type.storage_address, None);
    assert_eq!(
        network_type.base_vtable.as_deref(),
        Some("NewWorld+0x84cb580")
    );
    assert_eq!(network_type.vtable, None);
    assert_eq!(
        network_type.capabilities,
        vec![
            NetworkTypeCapability::ReplicatedState,
            NetworkTypeCapability::RegisteredFields
        ]
    );
    assert_eq!(
        network_type
            .handler
            .as_ref()
            .and_then(|handler| handler.unmarshal.as_deref()),
        Some("NewWorld+0x3495310")
    );
    assert_eq!(network_type.fields[0].name.as_deref(), Some("raidId"));
    assert_eq!(network_type.fields[0].group, Some(0));
    assert_eq!(
        network_type.fields[0].handler_vtable.as_deref(),
        Some("NewWorld+0x81dad80")
    );
    assert_eq!(network_type.fields[0].confidence, NetworkConfidence::High);

    let function = &schema.field_registration_functions[0];
    assert_eq!(function.owner_type_id, network_type.type_id);
    assert_eq!(
        function.fields[0].callsite.as_deref(),
        Some("NewWorld+0x3495762")
    );

    let handler_vtable = &schema.field_handler_vtables[0];
    assert_eq!(
        handler_vtable.address.as_deref(),
        Some("NewWorld+0x81dad80")
    );
    assert_eq!(handler_vtable.field_count, 1);
    assert_eq!(
        handler_vtable.marshal_target.as_deref(),
        Some("NewWorld+0x17266c0")
    );
    assert_eq!(handler_vtable.wire_shape, Some(NetworkWireShape::U64));
    assert_eq!(
        handler_vtable.wire_shape_source.as_deref(),
        Some("marshal-call:marshal-function-name")
    );
    assert_eq!(handler_vtable.slots[0].name.as_deref(), Some("Marshal"));
    assert_eq!(
        handler_vtable.slots[0].target.as_deref(),
        Some("NewWorld+0x17266c0")
    );
}

#[test]
fn rejects_private_source_derived_ghidra_reports() {
    let report = json!({
        "registryEntries": [],
        "fieldRegistrationFunctions": [{
            "address": "NewWorld+0x3495600",
            "fields": [{
                "index": 0,
                "name": "characterId",
                "wireShape": "entity-ref",
                "wireShapeSource": "source-replicated-field-handler",
                "confidence": "high"
            }]
        }],
        "fieldHandlerVtables": []
    });

    let error =
        NetworkSchema::from_ghidra_static_network_report(&report).expect_err("tainted report");

    assert!(matches!(
        error,
        NetworkSchemaImportError::PrivateSourceEvidence
    ));
}

#[test]
fn preserves_normalized_confidence_labels() {
    let report = json!({
        "registryEntries": [{
            "uuid": "44BA4CBA-AFAD-4EC5-A9DA-500838B28A57",
            "typeIndex": 747,
            "typeName": "ActorMover::CheckMovementStatusMsg",
            "fields": [{
                "index": 0,
                "name": "ActorId",
                "wireShape": "fixed-bytes-16",
                "confidence": "high"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

    assert_eq!(
        schema.types[0].fields[0].confidence,
        NetworkConfidence::High
    );
    assert_eq!(schema.summary.high_confidence_field_count, 1);
}

#[test]
fn parses_fixed_byte_wire_shapes() {
    let report = json!({
        "registryEntries": [],
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
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

    assert_eq!(
        schema.field_handler_vtables[0].wire_shape,
        Some(NetworkWireShape::FixedBytes(6))
    );
    assert_eq!(
        schema.field_handler_vtables[1].wire_shape,
        Some(NetworkWireShape::FixedBytes(16))
    );
}

#[test]
fn parses_container_wire_shapes() {
    let report = json!({
        "registryEntries": [],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81b6eb8",
            "fieldCount": 1,
            "handlerTypeName": "MB::ReplicatedMapFieldHandler<AZ::u32, MB::TimePoint>",
            "handlerTypeSource": "handler-constructor-template",
            "handlerContainerType": {
                "storageKind": "index-map",
                "keyNativeType": "AZ::u32",
                "valueNativeType": "MB::TimePoint",
                "source": "handler-constructor-template"
            },
            "wireShape": "replicated-container<u32,vlq-u64>",
            "wireShapeSource": "replicated-container-marshal-calls",
            "slots": []
        }, {
            "address": "NewWorld+0x81b6ec0",
            "fieldCount": 1,
            "wireShape": "sequence-number",
            "wireShapeSource": "marshal-call:sequence-number",
            "slots": []
        }, {
            "address": "NewWorld+0x81b6ec8",
            "fieldCount": 1,
            "wireShape": "vlq-u64",
            "wireShapeSource": "marshal-call:vlq-u64",
            "slots": []
        }]
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

    assert_eq!(
        schema.field_handler_vtables[0].wire_shape,
        Some(NetworkWireShape::ReplicatedContainer(
            NetworkReplicatedContainerWireShape {
                key: NetworkWireScalarShape::U32,
                value: NetworkWireScalarShape::VlqU64,
            }
        ))
    );
    assert_eq!(
        schema.field_handler_vtables[0].handler_type_name.as_deref(),
        Some("MB::ReplicatedMapFieldHandler<AZ::u32, MB::TimePoint>")
    );
    assert_eq!(
        schema.field_handler_vtables[0]
            .handler_type_source
            .as_deref(),
        Some("handler-constructor-template")
    );
    let container_type = schema.field_handler_vtables[0]
        .handler_container_type
        .as_ref()
        .expect("handler container type");
    assert_eq!(
        container_type.storage_kind,
        NetworkReplicatedContainerStorageKind::Map
    );
    assert_eq!(container_type.key_native_type.as_deref(), Some("AZ::u32"));
    assert_eq!(container_type.value_native_type, "MB::TimePoint");
    assert_eq!(
        schema.field_handler_vtables[1].wire_shape,
        Some(NetworkWireShape::SequenceNumber)
    );
    assert_eq!(
        schema.field_handler_vtables[2].wire_shape,
        Some(NetworkWireShape::VlqU64)
    );

    assert_eq!(
        serde_json::to_value(schema.field_handler_vtables[0].wire_shape.as_ref().unwrap(),)
            .unwrap(),
        json!("replicated-container<u32,vlq-u64>")
    );
}

#[test]
fn ignores_raw_byte_lengths_that_conflict_with_wire_shape() {
    let conflict = json!({
        "index": 0,
        "name": "field_0",
        "rawByteLength": 16,
        "wireShape": "u64",
        "wireShapeSource": "message-unmarshal-helper-nested-call",
        "confidence": "message-unmarshal-helper-argument"
    });
    let field = network_field(conflict.as_object().expect("field object"));

    assert_eq!(field.raw_byte_length, None);
    assert_eq!(field.native_type, None);
    assert_eq!(field.wire_shape, None);

    let fixed = json!({
        "index": 0,
        "name": "field_0",
        "rawByteLength": 16,
        "wireShape": "fixed-bytes-16",
        "wireShapeSource": "message-unmarshal-read-raw",
        "confidence": "message-unmarshal-read-raw"
    });
    let field = network_field(fixed.as_object().expect("field object"));

    assert_eq!(field.raw_byte_length, Some(16));
    assert_eq!(field.wire_shape, Some(NetworkWireShape::FixedBytes(16)));
}

#[test]
fn assigns_direct_message_and_support_data_capabilities() {
    let report = json!({
        "registryEntries": [
            {
                "uuid": "E3578B38-69AD-4C13-A7DD-3FFF752D98AA",
                "typeName": "ClientActorRoutingAuthorizationTrait::ClientAddEntryMsg"
            },
            {
                "uuid": "5566F141-5C23-4BFB-BEFF-372DAF60F713",
                "typeName": "Javelin::ContractActionParamsSellCompletion"
            }
        ],
        "fieldRegistrationFunctions": []
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

    assert_eq!(
        schema.types[0].capabilities,
        vec![NetworkTypeCapability::DirectMessage]
    );
    assert_eq!(
        schema.types[1].capabilities,
        vec![NetworkTypeCapability::SupportData]
    );
}

#[test]
fn replicated_state_capability_requires_state_leaf_name() {
    let report = json!({
        "registryEntries": [
            {
                "uuid": "11111111-1111-4111-9111-111111111111",
                "typeName": "Javelin::GameModeReplicatedState"
            },
            {
                "uuid": "22222222-2222-4222-9222-222222222222",
                "typeName": "Javelin::ClientMessages::ObjectiveInteractorComponentServerFacet_DEBUG_RequestForceUpdateReplicatedState"
            },
            {
                "uuid": "33333333-3333-4333-9333-333333333333",
                "typeName": "MB::ReplicatedState"
            },
            {
                "uuid": "44444444-4444-4444-9444-444444444444",
                "typeName": "Amazon::Hub::ReplicatedStateBundle"
            },
            {
                "uuid": "55555555-5555-4555-9555-555555555555",
                "typeName": "MB::SocialReplicatedState::ChattingStateMessageType"
            }
        ],
        "fieldRegistrationFunctions": []
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

    assert_eq!(
        schema.types[0].capabilities,
        vec![NetworkTypeCapability::ReplicatedState]
    );
    assert_eq!(
        schema.types[1].capabilities,
        vec![NetworkTypeCapability::DirectMessage]
    );
    assert_eq!(
        schema.types[2].capabilities,
        vec![NetworkTypeCapability::SupportData]
    );
    assert_eq!(
        schema.types[3].capabilities,
        vec![NetworkTypeCapability::SupportData]
    );
    assert_eq!(
        schema.types[4].capabilities,
        vec![NetworkTypeCapability::SupportData]
    );
}

#[test]
fn imports_message_unmarshal_fields_without_registered_fields_capability() {
    let report = json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 19,
            "typeName": "RegistrationRequestV3Msg",
            "messageUnmarshal": {
                "wrapper": "NewWorld+0x7ce8e0",
                "helperCallsite": "NewWorld+0x7ce955",
                "helper": "NewWorld+0x7d9620",
                "helperName": "Amazon::REP::REPClient::RegistrationRequestV3Msg::UnmarshalFields<ClientVersionTokenMap,LoginToken,AuthToken,ImpersonatedValues>",
                "createInstance": "NewWorld+0x7ce840",
                "instanceSize": "0x470",
                "instanceSizeSource": "create-instance-operator-new",
                "instanceConstructorCallsite": "NewWorld+0x7ce8fc",
                "instanceConstructor": "NewWorld+0x7e37d0",
                "instanceConstructorName": "Amazon::REP::REPClient::RegistrationRequestV3::RegistrationRequestV3",
                "templateTypes": [
                    "ClientVersionTokenMap",
                    "LoginToken",
                    "AuthToken",
                    "ImpersonatedValues"
                ]
            },
            "fields": [{
                "index": 0,
                "callsite": "NewWorld+0x7ce955",
                "name": "TypeIndexCrc",
                "nameSource": "msvc-rtti-source-signature",
                "nameSourceAddress": "NewWorld+0xa268e80",
                "sourceTypeName": "AZ::Crc32",
                "nativeType": "u32",
                "storageExpression": "(plVar1 + 1)",
                "storageOffset": "0x8",
                "wireShape": "u32",
                "wireShapeSource": "message-unmarshal-native-type",
                "unmarshalEvidence": {
                    "callsite": "NewWorld+0x7ce955",
                    "targetName": "GridMate::Marshaler<AZ::Crc32>::Unmarshal",
                    "targetKind": "field-helper",
                    "evidenceSource": "message-unmarshal-pcode-call"
                },
                "confidence": "message-unmarshal-call"
            }, {
                "index": 2,
                "callsite": "NewWorld+0x7ce955",
                "name": "ConnTicket",
                "nativeType": "AZStd::string",
                "storageExpression": "(plVar1 + 0x14)",
                "storageOffset": "0xa0",
                "wireShape": "string",
                "wireShapeSource": "message-unmarshal-native-type",
                "confidence": "message-unmarshal-call"
            }, {
                "index": 6,
                "callsite": "NewWorld+0x7ce955",
                "name": "UseCapabilities",
                "nameSource": "msvc-rtti-source-signature",
                "nameSourceAddress": "NewWorld+0xa268e80",
                "sourceTypeName": "bool",
                "nativeType": "bool",
                "storageExpression": "plVar1 + 0x8c",
                "storageOffset": "0x460",
                "wireShape": "bool",
                "wireShapeSource": "nested-unmarshal-bool-write",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

    assert_eq!(schema.summary.message_unmarshal_field_count, 3);
    assert_eq!(
        schema.types[0].capabilities,
        vec![NetworkTypeCapability::DirectMessage]
    );
    let instance = schema.types[0].instance.as_ref().expect("instance layout");
    assert_eq!(instance.size, Some(0x470));
    assert_eq!(instance.constructor.as_deref(), Some("NewWorld+0x7e37d0"));
    assert_eq!(
        schema.types[0].fields[0].native_type.as_deref(),
        Some("u32")
    );
    assert_eq!(
        schema.types[0].fields[0].source_type_name.as_deref(),
        Some("AZ::Crc32")
    );
    assert_eq!(
        schema.types[0]
            .fields
            .iter()
            .map(|field| field.index)
            .collect::<Vec<_>>(),
        vec![Some(0), Some(1), Some(2)]
    );
    assert_eq!(
        schema.types[0].fields[0].name.as_deref(),
        Some("TypeIndexCrc")
    );
    assert_eq!(schema.types[0].fields[0].storage_offset, Some(0x8));
    assert_eq!(
        schema.types[0].fields[0].wire_shape,
        Some(NetworkWireShape::U32)
    );
    let unmarshal_evidence = schema.types[0].fields[0]
        .unmarshal_evidence
        .as_ref()
        .expect("unmarshal evidence");
    assert_eq!(
        unmarshal_evidence.target_name.as_deref(),
        Some("GridMate::Marshaler<AZ::Crc32>::Unmarshal")
    );
    assert_eq!(
        unmarshal_evidence.evidence_source.as_deref(),
        Some("message-unmarshal-pcode-call")
    );
    assert_eq!(
        schema.types[0].fields[0].evidence[0].kind,
        NetworkEvidenceKind::MessageUnmarshal
    );
    assert_eq!(
        schema.types[0].fields[0].evidence[1].kind,
        NetworkEvidenceKind::MessageSource
    );
    assert_eq!(
        schema.types[0].fields[0].evidence[1].detail.as_deref(),
        Some("AZ::Crc32")
    );
    assert_eq!(
        schema.types[0].fields[1].wire_shape,
        Some(NetworkWireShape::String)
    );
    assert_eq!(schema.types[0].fields[2].storage_offset, Some(0x460));
    assert_eq!(
        schema.types[0].fields[2].wire_shape,
        Some(NetworkWireShape::Bool)
    );
}

#[test]
fn collapses_synthetic_nested_alternate_spelling_members() {
    let machine = "composite<string,string,fixed-bytes-1,string,fixed-bytes-4,string,string,fixed-bytes-1,fixed-bytes-1,fixed-bytes-1,fixed-bytes-1,fixed-bytes-1,vec<composite<fixed-bytes-4,fixed-bytes-2,fixed-bytes-2,fixed-bytes-2,fixed-bytes-2,fixed-bytes-2,fixed-bytes-2,fixed-bytes-1>>,fixed-bytes-4,string,fixed-bytes-1,fixed-bytes-1,vec<composite<fixed-bytes-4,string>>>";
    let semantic = "composite<string,string,u8,string,u32,string,string,bool,bool,bool,bool,bool,fixed-vector<composite<u32,u16,u16,u16,u16,u16,u16,u8>,20>,u32,string,bool,u8,vec<composite<u32,string>>>";
    let report = json!({
        "registryEntries": [{
            "uuid": "A55A0001-0000-4000-8000-000000001886",
            "typeIndex": 1886,
            "typeName": "LocalizedSystemChatMessage",
            "handler": {
                "Marshal": "NewWorld+0x100",
                "Unmarshal": "NewWorld+0x200"
            },
            "fields": [{
                "index": 0,
                "name": "field_0",
                "storageExpression": "param_3 + 0x20",
                "storageBase": "param_3",
                "storageOffset": 32,
                "wireShape": semantic,
                "wireLayout": format!("composite<{machine},{semantic}>"),
                "wireShapeSource": "cfg-multi-helper-alternate-spelling-collapse",
                "wireLayoutSource": "cfg-ordered-multi-helper-wire-product",
                "nestedTypeShape": {
                    "identityProven": true,
                    "typeName": "LocalizedSystemChatMessage",
                    "typeNameFull": "LocalizedSystemChatMessage",
                    "memberNameSource": "synthetic-offset",
                    "memberNamesProven": false,
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "wireOrderSource": "cfg-ordered-multi-helper-wire-product",
                    "validation": "message-unmarshal-constructor-vptr+az-rtti+typeregistry-type-name",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "name": "_0",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "wireShape": machine,
                        "wireLayout": machine,
                        "wireOrdinal": 0,
                        "wireOrderSource": "cfg-ordered-multi-helper-wire-product",
                        "callsite": "NewWorld+0x300"
                    }, {
                        "index": 1,
                        "offset": "0x1",
                        "name": "_1",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "wireShape": semantic,
                        "wireLayout": semantic,
                        "wireOrdinal": 1,
                        "wireOrderSource": "cfg-ordered-multi-helper-wire-product",
                        "callsite": "NewWorld+0x300"
                    }]
                },
                "confidence": "high"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let field = &schema.types[0].fields[0];
    let nested = field.nested_type_shape.as_ref().expect("nested type shape");

    assert_eq!(
        field.wire_shape.as_ref().map(NetworkWireShape::wire_string),
        Some(semantic.to_owned())
    );
    assert_eq!(nested.members.len(), 18);
    assert_eq!(
        nested.members[12].wire_shape.as_deref(),
        Some("fixed-vector<composite<u32,u16,u16,u16,u16,u16,u16,u8>,20>")
    );
    assert_eq!(
        nested.wire_order_source.as_deref(),
        Some("cfg-multi-helper-alternate-spelling-collapse")
    );
}

#[test]
fn keeps_message_marshal_fields_separate_from_unmarshal_fields() {
    let report = json!({
        "registryEntries": [{
            "uuid": "44BA4CBA-AFAD-4EC5-A9DA-500838B28A57",
            "typeIndex": 747,
            "typeName": "ActorMover::CheckMovementStatusMsg",
            "messageUnmarshal": {
                "wrapper": "NewWorld+0x6a59b30",
                "terminalStatus": "no-success-terminal",
                "supportsUnmarshal": false,
                "fields": [{
                    "index": 0,
                    "callsite": "NewWorld+0x6a59b9e",
                    "storageExpression": "param_3 + 0x8",
                    "storageBase": "param_3",
                    "storageBaseOffset": "0x8",
                    "wireLayout": "fixed-bytes-16",
                    "confidence": "message-unmarshal-pcode-stack-readraw"
                }]
            },
            "messageMarshal": {
                "wrapper": "NewWorld+0x6a59a40",
                "writeBufferParameter": "param_3",
                "rootStorageBase": "param_2",
                "analysisStatus": "complete-cfg-stack-flow",
                "fields": [{
                    "index": 0,
                    "callsite": "NewWorld+0x6a59a4c",
                    "storageExpression": "param_2 + 0x8",
                    "storageBase": "param_2",
                    "storageOffset": "0x8",
                    "wireLayout": "fixed-bytes-16",
                    "confidence": "message-marshal-pcode-stack"
                }, {
                    "index": 1,
                    "callsite": "NewWorld+0x6a59a65",
                    "storageExpression": "param_2 + 0x20",
                    "storageBase": "param_2",
                    "storageOffset": "0x20",
                    "wireShape": "composite<u32,fixed-bytes-16,fixed-bytes-16>",
                    "confidence": "message-marshal-pcode-stack"
                }]
            },
            "fields": [{
                "index": 0,
                "callsite": "NewWorld+0x6a59b9e",
                "storageExpression": "param_3 + 0x8",
                "storageBase": "param_3",
                "storageBaseOffset": "0x8",
                "wireLayout": "fixed-bytes-16",
                "confidence": "message-unmarshal-pcode-stack-readraw"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let network_type = &schema.types[0];

    assert_eq!(network_type.fields.len(), 1);
    assert_eq!(network_type.marshal_fields.len(), 2);
    let instance = network_type.instance.as_ref().expect("instance layout");
    assert_eq!(instance.supports_unmarshal, Some(false));
    assert_eq!(
        instance.terminal_status.as_deref(),
        Some("no-success-terminal")
    );
    assert_eq!(schema.summary.message_unmarshal_field_count, 1);
    assert_eq!(schema.summary.message_marshal_field_count, 2);
    assert_eq!(
        network_type.marshal_fields[0].evidence[0].kind,
        NetworkEvidenceKind::MessageMarshal
    );
    assert_eq!(
        network_type.marshal_fields[1].wire_shape,
        Some(NetworkWireShape::Composite(vec![
            NetworkWireShape::U32,
            NetworkWireShape::FixedBytes(16),
            NetworkWireShape::FixedBytes(16),
        ]))
    );
}

#[test]
fn imports_delegated_fragment_codec_evidence() {
    let report = json!({
        "registryEntries": [{
            "uuid": "6FA0EBFF-94CB-4106-AC73-E0BDA9F2C68B",
            "typeIndex": 573,
            "typeName": "MoveCoordinator",
            "messageUnmarshal": {
                "wrapper": "NewWorld+0x6b05780",
                "readBufferParameter": "param_4",
                "rootEvidenceSource": "direct-call-target-and-ssa-arguments",
                "delegatedCodec": {
                    "kind": "fragment-full-unmarshal",
                    "callsite": "NewWorld+0x6b057d4",
                    "function": "NewWorld+0x6160ae0",
                    "valueStorage": "param_1 + 0x0",
                    "outcomeStorage": "param_2 + 0x0",
                    "readBufferStorage": "param_4 + 0x0",
                    "evidenceSource": "direct-call-target-and-ssa-arguments"
                },
                "fields": []
            },
            "fields": []
        }],
        "fieldRegistrationFunctions": []
    });

    let schema =
        NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
    let instance = schema.types[0].instance.as_ref().expect("instance layout");
    let codec = instance
        .delegated_codec
        .as_ref()
        .expect("delegated fragment codec");

    assert_eq!(codec.kind, "fragment-full-unmarshal");
    assert_eq!(codec.function, "NewWorld+0x6160ae0");
    assert_eq!(codec.callsite, "NewWorld+0x6b057d4");
    assert_eq!(codec.value_storage.as_deref(), Some("param_1 + 0x0"));
    assert_eq!(codec.outcome_storage.as_deref(), Some("param_2 + 0x0"));
    assert_eq!(codec.read_buffer_storage, "param_4 + 0x0");
    assert_eq!(
        codec.evidence_source,
        "direct-call-target-and-ssa-arguments"
    );
}
