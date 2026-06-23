use std::path::PathBuf;

use crate::reference::ReferenceKey;
use nw_objectstream::type_uuid::type_ids;
use serde_json::json;
use uuid::uuid;

use super::*;

#[test]
fn lowers_current_top_level_serialize_context_indexes() {
    let document = SerializeContextDocument::from_value_unchecked(json!({
        "$id": 1,
        "uuidMap": {
            "11111111-1111-1111-1111-111111111111": {
                "$id": 10,
                "name": "ExampleComponent",
                "typeId": "11111111-1111-1111-1111-111111111111",
                "version": 3,
                "factory": "NewWorld+0x10",
                "persistentId": "NewWorld+0x11",
                "doSave": "NewWorld+0x12",
                "serializer": "NewWorld+0x20",
                "azRtti": null,
                "container": null,
                "converter": null,
                "dataConverter": null,
                "eventHandler": null,
                "elements": [
                    {
                        "$id": 11,
                        "name": "count",
                        "nameCrc": 5,
                        "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                        "dataSize": "4",
                        "offset": "16",
                        "attributeOwnership": 0,
                        "flags": 0,
                        "is_pointer": false,
                        "is_base_class": false,
                        "no_default_value": false,
                        "is_dynamic_field": false,
                        "is_ui_element": false,
                        "azRtti": null,
                        "genericClassInfo": null,
                        "attributes": [[1, {
                            "$id": 12,
                            "attributeId": 1,
                            "attributeName": "ChangeNotify",
                            "describesChildren": false,
                            "childClassOwned": false,
                            "value": {
                                "kind": "function",
                                "function": null,
                                "memberFunction": "NewWorld+0x2c974f0"
                            }
                        }]]
                    }
                ],
                "attributes": []
            }
        },
        "classNameToUuid": [[5, "11111111-1111-1111-1111-111111111111"]],
        "uuidGenericMap": [],
        "uuidAnyCreationMap": {
            "11111111-1111-1111-1111-111111111111": "NewWorld+0x40"
        },
        "editContext": {
            "$id": 2,
            "classData": [],
            "enumData": []
        },
        "enumTypeIdToUnderlyingTypeIdMap": {}
    }));

    let model = SerializeContextModel::from_document(&document);
    let component = &model.classes[&uuid!("11111111-1111-1111-1111-111111111111")];

    assert_eq!(component.name, "ExampleComponent");
    assert_eq!(component.version, Some(3));
    assert_eq!(component.persistent_id.as_deref(), Some("NewWorld+0x11"));
    assert_eq!(component.do_save.as_deref(), Some("NewWorld+0x12"));
    assert_eq!(component.members[0].name, "count");
    assert_eq!(
        component.members[0].attributes[0]
            .value
            .as_ref()
            .and_then(|value| value.member_function.as_deref()),
        Some("NewWorld+0x2c974f0")
    );
    assert_eq!(model.class_name_index.len(), 1);
    assert_eq!(
        model.any_creators[&uuid!("11111111-1111-1111-1111-111111111111")],
        "NewWorld+0x40"
    );
}

#[test]
fn schema_document_lowers_class_members_from_generated_schema() {
    let document = SerializeContextDocument::from_slice(
        br#"{
                "$id": 1,
                "uuidMap": {
                    "22222222-2222-2222-2222-222222222222": {
                        "$id": 20,
                        "name": "Example::SchemaComponent",
                        "typeId": "22222222-2222-2222-2222-222222222222",
                        "version": 7,
                        "factory": "NewWorld+0x100",
                        "persistentId": "NewWorld+0x110",
                        "doSave": "NewWorld+0x120",
                        "serializer": "NewWorld+0x130",
                        "eventHandler": "NewWorld+0x140",
                        "container": null,
                        "azRtti": {
                            "address": "NewWorld+0x150",
                            "typeId": "22222222-2222-2222-2222-222222222222",
                            "typeName": "Example::SchemaComponent",
                            "hierarchy": [{
                                "typeId": "22222222-2222-2222-2222-222222222222",
                                "typeName": "Example::SchemaComponent"
                            }],
                            "isAbstract": false
                        },
                        "dataConverter": null,
                        "editData": null,
                        "elements": [{
                            "$id": 21,
                            "name": "m_count",
                            "nameCrc": 42,
                            "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                            "dataSize": "4",
                            "offset": "32",
                            "attributeOwnership": 0,
                            "flags": 1,
                            "is_pointer": false,
                            "is_base_class": false,
                            "no_default_value": true,
                            "is_dynamic_field": false,
                            "is_ui_element": false,
                            "azRtti": {
                                "$id": 30,
                                "address": "NewWorld+0x160",
                                "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                                "typeName": "unsigned int",
                                "hierarchy": [{
                                    "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                                    "typeName": "unsigned int"
                                }],
                                "isAbstract": false
                            },
                            "editData": null,
                            "attributes": []
                        }],
                        "attributes": []
                    }
                },
                "classNameToUuid": [[42, "22222222-2222-2222-2222-222222222222"]],
                "uuidGenericMap": [],
                "uuidAnyCreationMap": {
                    "22222222-2222-2222-2222-222222222222": "NewWorld+0x170"
                },
                "editContext": {
                    "$id": 2,
                    "classData": [],
                    "enumData": []
                },
                "enumTypeIdToUnderlyingTypeIdMap": {}
            }"#,
    )
    .expect("minimal serialize context should match generated schema");

    assert!(document.schema().is_some());

    let model = SerializeContextModel::from_document(&document);
    let component = &model.classes[&uuid!("22222222-2222-2222-2222-222222222222")];
    let member = &component.members[0];

    assert_eq!(component.reference_id, Some(ReferenceKey::Number(20)));
    assert_eq!(component.name, "Example::SchemaComponent");
    assert_eq!(component.version, Some(7));
    assert_eq!(component.factory.as_deref(), Some("NewWorld+0x100"));
    assert_eq!(component.persistent_id.as_deref(), Some("NewWorld+0x110"));
    assert_eq!(component.do_save.as_deref(), Some("NewWorld+0x120"));
    assert_eq!(component.serializer.as_deref(), Some("NewWorld+0x130"));
    assert_eq!(component.event_handler.as_deref(), Some("NewWorld+0x140"));
    let component_rtti = component.az_rtti.as_ref().expect("component rtti");
    assert_eq!(component_rtti.address(), Some("NewWorld+0x150"));
    assert_eq!(
        component_rtti.type_id,
        Some(uuid!("22222222-2222-2222-2222-222222222222"))
    );
    assert_eq!(
        component_rtti.type_name.as_deref(),
        Some("Example::SchemaComponent")
    );
    assert_eq!(component_rtti.hierarchy.len(), 1);
    assert_eq!(component_rtti.is_abstract, Some(false));
    assert_eq!(member.reference_id, Some(ReferenceKey::Number(21)));
    assert_eq!(member.name, "m_count");
    assert_eq!(member.name_crc, Some(42));
    assert_eq!(member.data_size, Some(4));
    assert_eq!(member.offset, Some(32));
    assert_eq!(member.flags, Some(1));
    assert!(member.no_default_value);
    let member_rtti = member.az_rtti.as_ref().expect("member rtti");
    assert_eq!(member_rtti.reference_id, Some(ReferenceKey::Number(30)));
    assert_eq!(member_rtti.address(), Some("NewWorld+0x160"));
    assert_eq!(
        member_rtti.type_id,
        Some(uuid!("43DA906B-7DEF-4CA8-9790-854106D3F983"))
    );
    assert_eq!(model.class_name_index[0].name_crc, Some(42));
    assert_eq!(
        model.any_creators[&uuid!("22222222-2222-2222-2222-222222222222")],
        "NewWorld+0x170"
    );
}

#[test]
fn registers_top_level_and_nested_generic_class_infos() {
    let root = json!({
        "$id": 1,
        "uuidMap": {
            "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                "$id": 10,
                "name": "Owner",
                "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                "elements": [{
                    "$id": 11,
                    "name": "names",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "genericClassInfo": { "$ref": "#30" }
                }],
                "attributes": []
            }
        },
        "classNameToUuid": [],
        "uuidGenericMap": [
            ["BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB", { "$ref": "#30" }]
        ],
        "uuidAnyCreationMap": {},
        "editContext": {"$id": 2, "classData": [], "enumData": []},
        "enumTypeIdToUnderlyingTypeIdMap": {},
        "generic": {
            "$id": 30,
            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
            "registeredTypeIds": ["BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"],
            "templatedArgumentCount": 1,
            "templatedTypeIds": ["03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9"],
            "typeIdFoldTypeIds": null,
            "specializedTypeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
            "genericTypeId": "2BADE35A-6F1B-4698-B2BC-3373D010020C",
            "legacySpecializedTypeId": null,
            "nonTypeTemplateArguments": null,
            "classData": {
                "$id": 31,
                "name": "AZStd::vector",
                "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                "elements": [],
                "attributes": []
            },
            "elements": []
        }
    });

    let model = SerializeContextModel::from_root(&root);
    let generic = model
        .generic_class(uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"))
        .expect("generic info should be registered by concrete type id");

    assert_eq!(generic.class_name.as_deref(), Some("AZStd::vector"));
    assert_eq!(
        generic.argument_type_ids(),
        vec![uuid!("03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9")]
    );
    assert!(
        model.classes[&uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA")].members[0]
            .generic_class
            .is_some()
    );
}

#[test]
fn schema_document_follows_nested_generic_refs_from_uuid_generic_map_elements() {
    let document = SerializeContextDocument::from_slice(
        json!({
            "$id": 1,
            "uuidMap": {},
            "classNameToUuid": [],
            "uuidGenericMap": [[
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                {
                    "$id": 30,
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "registeredTypeIds": ["AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"],
                    "templatedArgumentCount": 2,
                    "templatedTypeIds": [
                        type_ids::U32.hyphenated().to_string(),
                        type_ids::AZSTD_STRING.hyphenated().to_string()
                    ],
                    "typeIdFoldTypeIds": null,
                    "specializedTypeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "genericTypeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "legacySpecializedTypeId": null,
                    "nonTypeTemplateArguments": null,
                    "classData": {
                        "$id": 31,
                        "name": "AZStd::unordered_map",
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                        "version": 0,
                        "doSave": null,
                        "dataConverter": null,
                        "editData": null,
                        "elements": [],
                        "attributes": []
                    },
                    "elements": [{
                        "$id": 32,
                        "name": "element",
                        "nameCrc": 0,
                        "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                        "dataSize": "8",
                        "offset": "0",
                        "attributeOwnership": 0,
                        "flags": 0,
                        "is_pointer": false,
                        "is_base_class": false,
                        "no_default_value": false,
                        "is_dynamic_field": false,
                        "is_ui_element": false,
                        "genericClassInfo": {"$ref": "#40"},
                        "editData": null,
                        "attributes": []
                    }]
                }
            ]],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {},
            "genericDefinitions": {
                "$id": 40,
                "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                "registeredTypeIds": ["CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC"],
                "templatedArgumentCount": 2,
                "templatedTypeIds": [
                    type_ids::U32.hyphenated().to_string(),
                    type_ids::AZSTD_STRING.hyphenated().to_string()
                ],
                "typeIdFoldTypeIds": null,
                "specializedTypeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                "genericTypeId": "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD",
                "legacySpecializedTypeId": null,
                "nonTypeTemplateArguments": null,
                "classData": {
                    "$id": 41,
                    "name": "AZStd::pair",
                    "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                    "elements": [],
                    "attributes": []
                },
                "elements": [
                    {
                        "$id": 42,
                        "name": "value1",
                        "typeId": type_ids::U32.hyphenated().to_string()
                    },
                    {
                        "$id": 43,
                        "name": "value2",
                        "typeId": type_ids::AZSTD_STRING.hyphenated().to_string()
                    }
                ]
            }
        })
        .to_string()
        .as_bytes(),
    )
    .expect("schema document");

    let model = SerializeContextModel::from_document(&document);
    let resolved = crate::types::TypeResolver::new(&model)
        .resolve(uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"));

    let crate::types::ResolvedType::Map { kind, key, value } = resolved else {
        panic!("expected resolved map");
    };
    assert_eq!(kind, crate::types::MapKind::UnorderedMap);
    assert_eq!(
        *key,
        crate::types::ResolvedType::Scalar(crate::types::ScalarType::U32)
    );
    assert_eq!(
        *value,
        crate::types::ResolvedType::Scalar(crate::types::ScalarType::String)
    );
}

#[test]
fn model_lowering_stops_cyclic_generic_refs() {
    let model = SerializeContextModel::from_root(&json!({
        "$id": 1,
        "uuidMap": {
            "11111111-1111-1111-1111-111111111111": {
                "$id": 10,
                "name": "Example::ContainerComponent",
                "typeId": "11111111-1111-1111-1111-111111111111",
                "elements": [{
                    "$id": 11,
                    "name": "m_values",
                    "typeId": "22222222-2222-2222-2222-222222222222",
                    "is_base_class": false,
                    "genericClassInfo": {
                        "$id": "generic-cycle",
                        "typeId": "22222222-2222-2222-2222-222222222222",
                        "registeredTypeIds": ["22222222-2222-2222-2222-222222222222"],
                        "templatedArgumentCount": 1,
                        "templatedTypeIds": [type_ids::U32.hyphenated().to_string()],
                        "specializedTypeId": "22222222-2222-2222-2222-222222222222",
                        "genericTypeId": "33333333-3333-3333-3333-333333333333",
                        "classData": {
                            "$id": 12,
                            "name": "AZStd::vector",
                            "typeId": "22222222-2222-2222-2222-222222222222"
                        },
                        "elements": [{
                            "$id": 13,
                            "name": "element",
                            "typeId": type_ids::U32.hyphenated().to_string(),
                            "genericClassInfo": {"$ref": "#generic-cycle"}
                        }]
                    }
                }],
                "attributes": []
            }
        },
        "classNameToUuid": [],
        "uuidGenericMap": [],
        "uuidAnyCreationMap": {},
        "editContext": {"$id": 2, "classData": [], "enumData": []},
        "enumTypeIdToUnderlyingTypeIdMap": {}
    }));

    let class = model
        .classes
        .get(&uuid!("11111111-1111-1111-1111-111111111111"))
        .expect("class");
    let generic = class.members[0]
        .generic_class
        .as_deref()
        .expect("outer generic");

    assert_eq!(generic.members.len(), 1);
    assert!(generic.members[0].generic_class.is_none());
}

#[test]
fn bodyless_rtti_types_lift_abstract_generic_element_rtti() {
    let interface_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    let vector_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    let owner_id = uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc");
    let model = SerializeContextModel::from_root(&json!({
        "$id": 1,
        "uuidMap": {
            "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC": {
                "$id": 10,
                "name": "ExampleOwner",
                "typeId": owner_id.hyphenated().to_string(),
                "elements": [{
                    "$id": 11,
                    "name": "m_interfaces",
                    "typeId": vector_id.hyphenated().to_string(),
                    "is_base_class": false,
                    "genericClassInfo": {
                        "$id": 20,
                        "typeId": vector_id.hyphenated().to_string(),
                        "registeredTypeIds": [vector_id.hyphenated().to_string()],
                        "templatedArgumentCount": 1,
                        "templatedTypeIds": [interface_id.hyphenated().to_string()],
                        "specializedTypeId": vector_id.hyphenated().to_string(),
                        "genericTypeId": "2BADE35A-6F1B-4698-B2BC-3373D010020C",
                        "classData": {
                            "$id": 21,
                            "name": "AZStd::vector",
                            "typeId": vector_id.hyphenated().to_string()
                        },
                        "elements": [{
                            "$id": 22,
                            "name": "element",
                            "typeId": interface_id.hyphenated().to_string(),
                            "is_base_class": false,
                            "azRtti": {
                                "$id": 23,
                                "address": "NewWorld+0x10",
                                "typeId": interface_id.hyphenated().to_string(),
                                "typeName": "ITestInterface",
                                "hierarchy": [{
                                    "typeId": interface_id.hyphenated().to_string(),
                                    "typeName": "ITestInterface"
                                }],
                                "isAbstract": true
                            }
                        }]
                    }
                }],
                "attributes": []
            }
        },
        "classNameToUuid": [],
        "uuidGenericMap": [],
        "uuidAnyCreationMap": {},
        "editContext": {"$id": 2, "classData": [], "enumData": []},
        "enumTypeIdToUnderlyingTypeIdMap": {}
    }));

    let bodyless = model.bodyless_rtti_types();
    let interface = bodyless
        .get(&interface_id)
        .expect("abstract generic element RTTI should become a bodyless support type");

    assert_eq!(interface.name, "ITestInterface");
    assert_eq!(interface.is_abstract, Some(true));
}

#[test]
fn bodyless_rtti_types_do_not_lift_concrete_data_members_without_a_class_body() {
    let client_view_id = uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee");
    let vector_id = uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff");
    let owner_id = uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd");
    let model = SerializeContextModel::from_root(&json!({
        "$id": 1,
        "uuidMap": {
            "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD": {
                "$id": 10,
                "name": "ExampleOwner",
                "typeId": owner_id.hyphenated().to_string(),
                "elements": [{
                    "$id": 11,
                    "name": "m_clientViews",
                    "typeId": vector_id.hyphenated().to_string(),
                    "is_base_class": false,
                    "genericClassInfo": {
                        "$id": 20,
                        "typeId": vector_id.hyphenated().to_string(),
                        "registeredTypeIds": [vector_id.hyphenated().to_string()],
                        "templatedArgumentCount": 1,
                        "templatedTypeIds": [client_view_id.hyphenated().to_string()],
                        "specializedTypeId": vector_id.hyphenated().to_string(),
                        "genericTypeId": "2BADE35A-6F1B-4698-B2BC-3373D010020C",
                        "classData": {
                            "$id": 21,
                            "name": "AZStd::vector",
                            "typeId": vector_id.hyphenated().to_string()
                        },
                        "elements": [{
                            "$id": 22,
                            "name": "element",
                            "typeId": client_view_id.hyphenated().to_string(),
                            "dataSize": "16",
                            "is_base_class": false,
                            "azRtti": {
                                "$id": 23,
                                "address": "NewWorld+0x20",
                                "typeId": client_view_id.hyphenated().to_string(),
                                "typeName": "ClientView",
                                "hierarchy": [{
                                    "typeId": client_view_id.hyphenated().to_string(),
                                    "typeName": "ClientView"
                                }],
                                "isAbstract": false
                            }
                        }]
                    }
                }],
                "attributes": []
            }
        },
        "classNameToUuid": [],
        "uuidGenericMap": [],
        "uuidAnyCreationMap": {},
        "editContext": {"$id": 2, "classData": [], "enumData": []},
        "enumTypeIdToUnderlyingTypeIdMap": {}
    }));

    assert!(
        !model.bodyless_rtti_types().contains_key(&client_view_id),
        "concrete values with data size need a real field body or a first-class Rust RTTI type, not an empty bodyless type"
    );
}

#[test]
fn bodyless_rtti_type_merge_keeps_abstract_evidence() {
    let base_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    let concrete_owner_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    let abstract_owner_id = uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc");
    let model = SerializeContextModel::from_root(&json!({
        "$id": 1,
        "uuidMap": {
            "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                "$id": 10,
                "name": "ConcreteOwner",
                "typeId": concrete_owner_id.hyphenated().to_string(),
                "elements": [{
                    "$id": 11,
                    "name": "BaseClass1",
                    "typeId": base_id.hyphenated().to_string(),
                    "is_base_class": true,
                    "azRtti": {
                        "$id": 12,
                        "typeId": base_id.hyphenated().to_string(),
                        "typeName": "SharedBase",
                        "hierarchy": [{
                            "typeId": base_id.hyphenated().to_string(),
                            "typeName": "SharedBase"
                        }],
                        "isAbstract": false
                    }
                }],
                "attributes": []
            },
            "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC": {
                "$id": 20,
                "name": "AbstractOwner",
                "typeId": abstract_owner_id.hyphenated().to_string(),
                "elements": [{
                    "$id": 21,
                    "name": "BaseClass1",
                    "typeId": base_id.hyphenated().to_string(),
                    "is_base_class": true,
                    "azRtti": {
                        "$id": 22,
                        "typeId": base_id.hyphenated().to_string(),
                        "typeName": "SharedBase",
                        "hierarchy": [{
                            "typeId": base_id.hyphenated().to_string(),
                            "typeName": "SharedBase"
                        }],
                        "isAbstract": true
                    }
                }],
                "attributes": []
            }
        },
        "classNameToUuid": [],
        "uuidGenericMap": [],
        "uuidAnyCreationMap": {},
        "editContext": {"$id": 2, "classData": [], "enumData": []},
        "enumTypeIdToUnderlyingTypeIdMap": {}
    }));

    assert_eq!(
        model.bodyless_rtti_types()[&base_id].is_abstract,
        Some(true)
    );
}

#[test]
fn lowers_edit_context_enum_values() {
    let model = SerializeContextModel::from_root(&json!({
        "$id": 1,
        "uuidMap": {},
        "classNameToUuid": [],
        "uuidGenericMap": [],
        "uuidAnyCreationMap": {},
        "editContext": {
            "$id": 2,
            "classData": [],
            "enumData": [[
                "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                {
                    "$id": 7,
                    "elementId": 3653223581u64,
                    "name": "Platform",
                    "description": "No Description",
                    "deprecatedName": null,
                    "attributes": [[3841142509u64, {
                        "$id": 10,
                        "attributeId": 3841142509u64,
                        "attributeName": "EnumValue",
                        "describesChildren": false,
                        "childClassOwned": false,
                        "value": {
                            "kind": "enumConstant",
                            "valueU64": "0x2",
                            "valueU32": 2,
                            "valueI32": 2,
                            "valueHighU32": 0,
                            "valueF32": 2.8,
                            "valueHighF32": 0,
                            "description": "Server"
                        }
                    }]]
                }
            ]]
        },
        "enumTypeIdToUnderlyingTypeIdMap": {}
    }));

    let enumeration = &model.enums[&uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC")];

    assert_eq!(enumeration.name, "Platform");
    assert_eq!(enumeration.variants[0].name, "Server");
    assert_eq!(enumeration.variants[0].value_u64, Some(2));
}

#[test]
fn lowers_attribute_values_from_document_wide_refs() {
    let model = SerializeContextModel::from_root(&json!({
        "$id": 1,
        "uuidMap": {},
        "classNameToUuid": [],
        "uuidGenericMap": [],
        "uuidAnyCreationMap": {},
        "editContext": {
            "$id": 2,
            "classData": [],
            "enumData": [[
                "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                {
                    "$id": 7,
                    "elementId": 3653223581u64,
                    "name": "Platform",
                    "description": "No Description",
                    "deprecatedName": null,
                    "attributes": [[3841142509u64, {
                        "$id": 10,
                        "attributeId": 3841142509u64,
                        "attributeName": "EnumValue",
                        "describesChildren": false,
                        "childClassOwned": false,
                        "value": {"$ref": "#enum-value-server"}
                    }]]
                }
            ]]
        },
        "enumTypeIdToUnderlyingTypeIdMap": {},
        "sharedAttributeValues": {
            "$id": "enum-value-server",
            "kind": "enumConstant",
            "valueU64": "0x2",
            "valueU32": 2,
            "valueI32": 2,
            "valueHighU32": 0,
            "valueF32": 2.8,
            "valueHighF32": 0,
            "description": "Server"
        }
    }));

    let enumeration = &model.enums[&uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC")];

    assert_eq!(enumeration.variants[0].name, "Server");
    assert_eq!(enumeration.variants[0].value_u64, Some(2));
}

#[test]
fn schema_document_expands_document_wide_refs_before_schema_parse() {
    let document = SerializeContextDocument::from_slice(
        json!({
            "$id": 1,
            "uuidMap": {
                "22222222-2222-2222-2222-222222222222": {"$ref": "#20"}
            },
            "classNameToUuid": [[42, "22222222-2222-2222-2222-222222222222"]],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {
                "22222222-2222-2222-2222-222222222222": "NewWorld+0x170"
            },
            "editContext": {
                "$id": 2,
                "classData": [],
                "enumData": [[
                    "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                    {
                        "$id": 40,
                        "elementId": 3653223581u64,
                        "name": "Platform",
                        "description": "No Description",
                        "deprecatedName": null,
                        "attributes": [[3841142509u64, {
                            "$id": 41,
                            "attributeId": 3841142509u64,
                            "attributeName": "EnumValue",
                            "describesChildren": false,
                            "childClassOwned": false,
                            "value": {"$ref": "#50"}
                        }]]
                    }
                ]]
            },
            "enumTypeIdToUnderlyingTypeIdMap": {},
            "sharedClass": {
                "$id": 20,
                "name": "Example::SchemaComponent",
                "typeId": "22222222-2222-2222-2222-222222222222",
                "version": 7,
                "factory": "NewWorld+0x100",
                "persistentId": "NewWorld+0x110",
                "doSave": "NewWorld+0x120",
                "serializer": "NewWorld+0x130",
                "eventHandler": "NewWorld+0x140",
                "container": null,
                "azRtti": {"$ref": "#60"},
                "dataConverter": null,
                "editData": null,
                "elements": [{
                    "$id": 21,
                    "name": "m_count",
                    "nameCrc": 42,
                    "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                    "dataSize": "4",
                    "offset": "32",
                    "attributeOwnership": 0,
                    "flags": 1,
                    "is_pointer": false,
                    "is_base_class": false,
                    "no_default_value": true,
                    "is_dynamic_field": false,
                    "is_ui_element": false,
                    "azRtti": {"$ref": "#61"},
                    "editData": null,
                    "attributes": []
                }],
                "attributes": []
            },
            "sharedEnumValue": {
                "$id": 50,
                "kind": "enumConstant",
                "valueU64": "0x2",
                "valueU32": 2,
                "valueI32": 2,
                "valueHighU32": 0,
                "valueF32": 2.8,
                "valueHighF32": 0,
                "description": "Server"
            },
            "sharedClassRtti": {
                "$id": 60,
                "address": "NewWorld+0x150",
                "typeId": "22222222-2222-2222-2222-222222222222",
                "typeName": "Example::SchemaComponent",
                "hierarchy": [{
                    "typeId": "22222222-2222-2222-2222-222222222222",
                    "typeName": "Example::SchemaComponent"
                }],
                "isAbstract": false
            },
            "sharedMemberRtti": {
                "$id": 61,
                "address": "NewWorld+0x160",
                "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                "typeName": "unsigned int",
                "hierarchy": [{
                    "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                    "typeName": "unsigned int"
                }],
                "isAbstract": false
            }
        })
        .to_string()
        .as_bytes(),
    )
    .expect("document-wide refs should be expanded before schema parse");

    assert!(document.schema().is_some());

    let model = SerializeContextModel::from_document(&document);
    let component = &model.classes[&uuid!("22222222-2222-2222-2222-222222222222")];
    let enumeration = &model.enums[&uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC")];

    assert_eq!(component.name, "Example::SchemaComponent");
    assert_eq!(component.members[0].name, "m_count");
    assert_eq!(enumeration.variants[0].name, "Server");
    assert_eq!(enumeration.variants[0].value_u64, Some(2));
}

#[test]
fn lowers_project_serialize_context_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("resources")
        .join("serialize.json");
    let document = SerializeContextDocument::from_path(path)
        .expect("project serialize.json should match generated schema");
    let model = SerializeContextModel::from_document(&document);

    assert!(model.classes.len() > 4_000);
    assert!(model.generic_classes.len() > 700);
    assert!(model.enums.len() > 300);
    assert!(model.any_creators.len() > 5_000);
    assert!(
        document.root()["editContext"]["classData"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
}
