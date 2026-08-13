use crate::ir::{
    SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenRttiBase, SerializeCodegenUnit,
};
use crate::role::ReflectedTypeRole;
use serde_json::json;
use uuid::uuid;

use super::*;

mod ingest;
mod merge;
mod message_alignment;

#[test]
fn collapses_alternate_spelling_multi_helper_wire_products() {
    use crate::network_schema::parse::collapse_alternate_spelling_wire_product;

    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,u8>,composite<u64,u64,u8>>",
            None,
        ),
        Some("composite<u64,u64,u8>")
    );
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,actor-ref>",
            None,
        ),
        Some("actor-ref")
    );
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<vec<composite<u32,string>>,fixed-vector<composite<fixed-bytes-4,string>,20>>",
            None,
        ),
        Some("fixed-vector<composite<fixed-bytes-4,string>,20>")
    );
    // Equal-width scalar calls are successive payload members, not duplicate
    // machine/semantic views of one member. ActorRequestId is two u64 limbs.
    assert_eq!(
        collapse_alternate_spelling_wire_product("composite<fixed-bytes-8,fixed-bytes-8>", None,),
        None
    );
    assert_eq!(
        collapse_alternate_spelling_wire_product("composite<fixed-bytes-8,u64>", None),
        None
    );
    // Distinct successive helpers must stay composed (FilterChat GameChatMessage).
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<vec<composite<fixed-bytes-4,fixed-bytes-2>>,composite<fixed-bytes-4,string>>",
            None,
        ),
        None
    );
    // Trailing limb already present as the leading limb of the inner product.
    // Requires nested-shape agreement so unrelated composite<field,same-type>
    // products stay intact.
    let nested = nested_shape_from_layouts(&[
        "fixed-bytes-8",
        "composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>",
        "entity-ref",
        "composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>",
        "string",
    ]);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,entity-ref,composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,string>,fixed-bytes-8>",
            Some(&nested),
        ),
        Some(
            "composite<fixed-bytes-8,composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,entity-ref,composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,string>"
        )
    );
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,string>,fixed-bytes-8>",
            None,
        ),
        None
    );
    let nested = nested_shape_from_layouts(&["u64", "u64", "u32", "bool"]);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,u32,bool>,composite<fixed-bytes-8,fixed-bytes-8>>",
            Some(&nested),
        ),
        Some("composite<fixed-bytes-8,u64,u32,bool>")
    );
    let nested = nested_shape_from_layouts(&[
        "fixed-bytes-8",
        "u64",
        "u8",
        "fixed-bytes-12",
        "fixed-bytes-12",
        "fixed-bytes-4",
        "fixed-bytes-4",
        "fixed-bytes-4",
        "composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>",
        "bool",
        "string",
        "fixed-bytes-4",
        "composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>",
        "bool",
        "string",
    ]);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,u8,fixed-bytes-12,fixed-bytes-12,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string>,composite<fixed-bytes-8,fixed-bytes-8>>",
            Some(&nested),
        ),
        Some(
            "composite<fixed-bytes-8,u64,u8,fixed-bytes-12,fixed-bytes-12,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string>"
        )
    );
    let nested = nested_shape_from_layouts(&[
        "u64",
        "u64",
        "u8",
        "vec3",
        "vec3",
        "f32",
        "f32",
        "f32",
        "composite<f32,f32,f32,f32>",
        "bool",
        "string",
        "f32",
        "composite<f32,f32,f32,f32>",
        "bool",
        "string",
    ]);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,u8,fixed-bytes-12,fixed-bytes-12,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string>,composite<fixed-bytes-8,fixed-bytes-8>>",
            Some(&nested),
        ),
        Some(
            "composite<fixed-bytes-8,u64,u8,fixed-bytes-12,fixed-bytes-12,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string>"
        )
    );
    let mut nested = nested_shape_from_layouts(&["u32", "fixed-bytes-16", "fixed-bytes-16", "u64"]);
    nested.layout_proven = Some(true);
    nested.member_coverage_proven = Some(true);
    nested.wire_order_proven = Some(true);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16,fixed-bytes-8>,fixed-bytes-16>",
            Some(&nested),
        ),
        Some("composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16,fixed-bytes-8>")
    );
    let mut nested = nested_shape_from_layouts(&["u64", "u64", "u32", "aabb3d"]);
    nested.layout_proven = Some(true);
    nested.member_coverage_proven = Some(true);
    nested.wire_order_proven = Some(true);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,fixed-bytes-4,fixed-bytes-24>,composite<fixed-bytes-8,fixed-bytes-8>>",
            Some(&nested),
        ),
        Some("composite<fixed-bytes-8,u64,fixed-bytes-4,fixed-bytes-24>")
    );
}

fn nested_shape_from_layouts(layouts: &[&str]) -> crate::network_schema::NetworkNestedTypeShape {
    crate::network_schema::NetworkNestedTypeShape {
        type_id: None,
        type_id_source: None,
        identity_proven: None,
        identity_source: None,
        type_name: None,
        type_name_full: None,
        type_name_source: None,
        function: None,
        function_name: None,
        factory: None,
        az_rtti_address: None,
        constructor: None,
        vtable: None,
        member_base: None,
        member_name_source: None,
        member_names_proven: None,
        layout_proven: None,
        member_coverage_proven: None,
        wire_order_proven: None,
        wire_order_source: None,
        datatype_path: None,
        validation: None,
        native_size: None,
        native_size_source: None,
        members: layouts
            .iter()
            .enumerate()
            .map(
                |(index, layout)| crate::network_schema::NetworkNestedTypeMember {
                    index: Some(index as u32),
                    offset: None,
                    native_offset: None,
                    name: None,
                    name_source: None,
                    name_proven: None,
                    name_evidence: None,
                    native_type: None,
                    type_id: None,
                    type_id_source: None,
                    type_identity_proven: false,
                    type_identity_source: None,
                    wire_shape: None,
                    wire_shape_source: None,
                    wire_layout: Some((*layout).to_owned()),
                    wire_layout_source: None,
                    byte_width: None,
                    wire_ordinal: None,
                    wire_order_source: None,
                    callsite: None,
                    target: None,
                    target_name: None,
                    type_conflict: false,
                },
            )
            .collect(),
    }
}

fn proven_synthetic_nested_shape(
    layouts: &[&str],
) -> crate::network_schema::NetworkNestedTypeShape {
    let mut shape = nested_shape_from_layouts(layouts);
    shape.validation =
        Some("message-unmarshal-constructor-vptr+az-rtti+typeregistry-type-name".to_owned());
    shape.member_name_source = Some("synthetic-offset".to_owned());
    shape.wire_order_source = Some("cfg-ordered-multi-helper-wire-product".to_owned());
    shape.layout_proven = Some(true);
    shape.member_coverage_proven = Some(true);
    shape.wire_order_proven = Some(true);
    for member in &mut shape.members {
        member.callsite = Some("NewWorld+0x1234".to_owned());
    }
    shape
}

#[test]
fn collapses_stale_duplicate_nested_wire_products() {
    use crate::network_schema::parse::collapse_synthetic_nested_duplicate_wire_product;

    let preferred = "composite<u64,u64,u32>";
    let mut nested = proven_synthetic_nested_shape(&[
        "composite<fixed-bytes-8,fixed-bytes-8,fixed-bytes-4>",
        preferred,
    ]);

    assert!(collapse_synthetic_nested_duplicate_wire_product(
        &mut nested,
        preferred,
    ));
    assert_eq!(
        nested
            .members
            .iter()
            .filter_map(|member| member.wire_layout.as_deref())
            .collect::<Vec<_>>(),
        ["u64", "u64", "u32"]
    );
    assert!(
        nested
            .members
            .iter()
            .all(|member| member.callsite.as_deref() == Some("NewWorld+0x1234"))
    );
}

#[test]
fn preserves_genuine_nested_members_and_distinct_callsites() {
    use crate::network_schema::parse::collapse_synthetic_nested_duplicate_wire_product;

    let preferred = "composite<u64,u64>";
    let mut genuine = proven_synthetic_nested_shape(&["u64", "u64"]);
    assert!(!collapse_synthetic_nested_duplicate_wire_product(
        &mut genuine,
        preferred,
    ));
    assert_eq!(genuine.members.len(), 2);

    let mut distinct_callsites =
        proven_synthetic_nested_shape(&["composite<u64,u64>", "composite<u64,u64>"]);
    distinct_callsites.members[1].callsite = Some("NewWorld+0x5678".to_owned());
    assert!(!collapse_synthetic_nested_duplicate_wire_product(
        &mut distinct_callsites,
        preferred,
    ));
    assert_eq!(distinct_callsites.members.len(), 2);
}

#[test]
fn collapses_redundant_call_frame_aggregate_fields() {
    let report = json!({
        "registryEntries": [{
            "uuid": "A8C4C56B-33F1-4A0A-A518-6D03A89EA3C4",
            "typeIndex": 9001,
            "typeName": "Test::AggregateMessage",
            "fields": [{
                "index": 0,
                "name": "aggregate_0",
                "storageBase": "param_3",
                "storageOffset": 16,
                "wireLayout": "composite<u32,bool>",
                "nestedTypeShape": {
                    "function": "NewWorld+0x1000",
                    "memberBase": "param_3",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "call-frame-output-parameter",
                    "members": [{
                        "offset": "0x0",
                        "wireLayout": "u32",
                        "wireOrdinal": 0,
                        "callsite": "NewWorld+0x1010"
                    }, {
                        "offset": "0x4",
                        "wireLayout": "bool",
                        "wireOrdinal": 1,
                        "callsite": "NewWorld+0x1020"
                    }]
                },
                "callsite": "NewWorld+0x1000",
                "confidence": "message-unmarshal-call-frame-boundary"
            }, {
                "index": 1,
                "name": "duplicate_bool",
                "storageBase": "param_3",
                "storageOffset": 20,
                "wireLayout": "fixed-bytes-1",
                "callsite": "NewWorld+0x1020",
                "confidence": "message-unmarshal-call-frame-output-member"
            }, {
                "index": 2,
                "name": "aggregate_1",
                "storageBase": "param_3",
                "storageOffset": 64,
                "wireLayout": "composite<u32,string>",
                "nestedTypeShape": {
                    "function": "NewWorld+0x2000",
                    "memberBase": "param_3",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "call-frame-output-parameter",
                    "members": [{
                        "offset": "0x0",
                        "wireLayout": "u32",
                        "wireOrdinal": 0,
                        "callsite": "NewWorld+0x2010"
                    }, {
                        "offset": "0x8",
                        "wireLayout": "string",
                        "wireOrdinal": 1,
                        "callsite": "NewWorld+0x2010"
                    }]
                },
                "callsite": "NewWorld+0x2000",
                "confidence": "message-unmarshal-call-frame-boundary"
            }, {
                "index": 3,
                "name": "status_plus_duplicate_product",
                "storageBase": "param_3",
                "storageOffset": 64,
                "wireLayout": "composite<fixed-bytes-4,fixed-bytes-4,string>",
                "callsite": "NewWorld+0x2010",
                "confidence": "message-unmarshal-pcode-stack"
            }, {
                "index": 4,
                "name": "real_field",
                "storageBase": "param_3",
                "storageOffset": 128,
                "wireLayout": "u64",
                "callsite": "NewWorld+0x3000",
                "confidence": "message-unmarshal-pcode-stack"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema = NetworkSchema::from_ghidra_static_network_report(&report)
        .expect("normalized network schema");
    let fields = &schema.types[0].fields;
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| field.name.as_deref())
            .collect::<Vec<_>>(),
        ["aggregate_0", "aggregate_1", "real_field"]
    );
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| field.index)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn collapses_contained_nested_helper_aggregate_with_distinct_callsite() {
    let report = json!({
        "registryEntries": [{
            "uuid": "8A03CB14-81B4-45F9-9BE8-6240E52C4B91",
            "typeIndex": 9006,
            "typeName": "Test::ContainedHelperMessage",
            "fields": [{
                "index": 0,
                "name": "header",
                "storageBase": "param_3",
                "storageOffset": 0,
                "wireLayout": "composite<u64,u64>"
            }, {
                "index": 1,
                "name": "nested_tail_helper",
                "storageBase": "param_3",
                "storageOffset": 80,
                "wireLayout": "composite<vec3,u32>",
                "nestedTypeShape": {
                    "function": "NewWorld+0x2000",
                    "memberBase": "param_3",
                    "layoutProven": true,
                    "nativeSize": 92,
                    "members": [{
                        "offset": "0x0",
                        "wireLayout": "fixed-bytes-12",
                        "callsite": "NewWorld+0x2010"
                    }, {
                        "offset": "0x10",
                        "wireLayout": "u32",
                        "callsite": "NewWorld+0x2020"
                    }]
                },
                "callsite": "NewWorld+0x1100"
            }, {
                "index": 2,
                "name": "enclosing_payload",
                "storageBase": "param_3",
                "storageOffset": 48,
                "wireLayout": "composite<vec3,u32>",
                "nestedTypeShape": {
                    "function": "NewWorld+0x3000",
                    "memberBase": "param_3",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "call-frame-output-parameter+symbolic-direct-type-identity",
                    "nativeSize": 124,
                    "members": [{
                        "offset": "0x20",
                        "wireLayout": "fixed-bytes-12",
                        "wireOrdinal": 0,
                        "callsite": "NewWorld+0x3010"
                    }, {
                        "offset": "0x30",
                        "wireLayout": "u32",
                        "wireOrdinal": 1,
                        "callsite": "NewWorld+0x3020"
                    }]
                },
                "callsite": "NewWorld+0x1200"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema = NetworkSchema::from_ghidra_static_network_report(&report)
        .expect("normalized network schema");
    assert_eq!(
        schema.types[0]
            .fields
            .iter()
            .filter_map(|field| field.name.as_deref())
            .collect::<Vec<_>>(),
        ["header", "enclosing_payload"]
    );
}

#[test]
fn collapses_repeated_helper_fields_already_covered_by_a_leading_product() {
    let report = json!({
        "registryEntries": [{
            "uuid": "34F86809-B6D0-46F5-821C-F3E14C15E995",
            "typeIndex": 9004,
            "typeName": "Test::RepeatedHelperMessage",
            "fields": [{
                "index": 0,
                "name": "entity_refs",
                "nativeType": "EntityRef",
                "sourceTypeName": "EntityRef",
                "storageBase": "param_3",
                "storageOffset": 16,
                "wireShape": "composite<entity-ref,entity-ref,entity-ref>",
                "wireShapeSource": "cfg-partial-call-frame-typed-prefix",
                "wireLayout": "composite<entity-ref,entity-ref,entity-ref>",
                "callsite": "NewWorld+0x1000",
                "confidence": "message-unmarshal-partial-call-frame-prefix"
            }, {
                "index": 1,
                "name": "duplicate_entity_ref_1",
                "nativeType": "EntityRef",
                "storageBase": "param_3",
                "storageOffset": 80,
                "wireShape": "entity-ref",
                "wireLayout": "entity-ref",
                "callsite": "NewWorld+0x1010",
                "confidence": "message-unmarshal-pcode-stack"
            }, {
                "index": 2,
                "name": "duplicate_entity_ref_2",
                "nativeType": "EntityRef",
                "storageBase": "param_3",
                "storageOffset": 144,
                "wireShape": "entity-ref",
                "wireLayout": "entity-ref",
                "callsite": "NewWorld+0x1020",
                "confidence": "message-unmarshal-pcode-stack"
            }, {
                "index": 3,
                "name": "real_field",
                "nativeType": "AZStd::string",
                "storageBase": "param_3",
                "storageOffset": 208,
                "wireShape": "string",
                "wireLayout": "string",
                "callsite": "NewWorld+0x1030",
                "confidence": "message-unmarshal-pcode-stack"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema = NetworkSchema::from_ghidra_static_network_report(&report)
        .expect("normalized network schema");
    let fields = &schema.types[0].fields;
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| field.name.as_deref())
            .collect::<Vec<_>>(),
        ["entity_refs", "real_field"]
    );
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| field.index)
            .collect::<Vec<_>>(),
        [0, 1]
    );
}

#[test]
fn preserves_uncovered_call_frame_products() {
    let report = json!({
        "registryEntries": [{
            "uuid": "A1804D9A-F7F6-47ED-82B9-B37E3929B050",
            "typeIndex": 9002,
            "typeName": "Test::AggregateMessage",
            "fields": [{
                "index": 0,
                "name": "aggregate",
                "storageBase": "param_3",
                "storageOffset": 16,
                "wireLayout": "composite<u32,string>",
                "nestedTypeShape": {
                    "function": "NewWorld+0x4000",
                    "memberBase": "param_3",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "call-frame-output-parameter",
                    "members": [{
                        "offset": "0x0",
                        "wireLayout": "u32",
                        "wireOrdinal": 0,
                        "callsite": "NewWorld+0x4010"
                    }, {
                        "offset": "0x8",
                        "wireLayout": "string",
                        "wireOrdinal": 1,
                        "callsite": "NewWorld+0x4010"
                    }]
                },
                "callsite": "NewWorld+0x4000",
                "confidence": "message-unmarshal-call-frame-boundary"
            }, {
                "index": 1,
                "name": "not_just_status",
                "storageBase": "param_3",
                "storageOffset": 16,
                "wireLayout": "composite<u64,u8,u32,string>",
                "callsite": "NewWorld+0x4010",
                "confidence": "message-unmarshal-pcode-stack"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema = NetworkSchema::from_ghidra_static_network_report(&report)
        .expect("normalized network schema");
    assert_eq!(schema.types[0].fields.len(), 2);
}

#[test]
fn normalizes_anonymous_call_frame_boundary_without_erasing_real_identity() {
    let report = json!({
        "registryEntries": [{
            "uuid": "790E00CC-474A-4128-90D9-321280ED762F",
            "typeIndex": 9003,
            "typeName": "Test::AggregateMessage",
            "fields": [{
                "index": 0,
                "name": "anonymous_boundary",
                "nativeType": "EntityRef",
                "storageBase": "param_3",
                "storageOffset": 16,
                "wireShape": "entity-ref",
                "nestedTypeShape": {
                    "function": "NewWorld+0x5000",
                    "memberBase": "param_3",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "call-frame-output-parameter",
                    "members": [{
                        "offset": "0x0",
                        "nativeType": "EntityRef",
                        "wireShape": "entity-ref",
                        "wireOrdinal": 0,
                        "callsite": "NewWorld+0x5010"
                    }, {
                        "offset": "0x40",
                        "nativeType": "EntityRef",
                        "wireShape": "entity-ref",
                        "wireOrdinal": 1,
                        "callsite": "NewWorld+0x5020"
                    }]
                },
                "callsite": "NewWorld+0x5000",
                "confidence": "message-unmarshal-call-frame-boundary"
            }, {
                "index": 1,
                "name": "semantic_boundary",
                "nativeType": "ActorRequestId",
                "storageBase": "param_3",
                "storageOffset": 160,
                "wireShape": "actor-ref",
                "nestedTypeShape": {
                    "typeName": "ActorRequestId",
                    "identityProven": true,
                    "function": "NewWorld+0x5100",
                    "memberBase": "param_3",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "call-frame-output-parameter",
                    "members": [{
                        "offset": "0x0",
                        "nativeType": "u32",
                        "wireShape": "u32",
                        "wireOrdinal": 0,
                        "callsite": "NewWorld+0x5110"
                    }, {
                        "offset": "0x4",
                        "wireLayout": "fixed-bytes-16",
                        "wireOrdinal": 1,
                        "callsite": "NewWorld+0x5120"
                    }, {
                        "offset": "0x14",
                        "wireLayout": "fixed-bytes-16",
                        "wireOrdinal": 2,
                        "callsite": "NewWorld+0x5130"
                    }]
                },
                "callsite": "NewWorld+0x5100",
                "confidence": "message-unmarshal-call-frame-boundary"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema = NetworkSchema::from_ghidra_static_network_report(&report)
        .expect("normalized network schema");
    let anonymous = &schema.types[0].fields[0];
    assert_eq!(anonymous.native_type, None);
    assert_eq!(
        anonymous
            .wire_shape
            .as_ref()
            .map(NetworkWireShape::wire_string),
        Some("composite<entity-ref,entity-ref>".to_owned())
    );
    assert_eq!(
        anonymous.wire_shape_source.as_deref(),
        Some("normalized-proven-call-frame-output-product")
    );

    let semantic = &schema.types[0].fields[1];
    assert_eq!(semantic.native_type.as_deref(), Some("ActorRequestId"));
    assert_eq!(
        semantic
            .wire_shape
            .as_ref()
            .map(NetworkWireShape::wire_string),
        Some("actor-ref".to_owned())
    );
}

#[test]
fn reconciles_stale_semantic_products_from_complete_exact_nested_layouts() {
    let report = json!({
        "registryEntries": [{
            "uuid": "FC70C4CE-F60C-43A1-8D44-F3257B7DE18A",
            "typeIndex": 9005,
            "typeName": "Test::ExactNestedMessage",
            "fields": [{
                "index": 0,
                "name": "ability_limit",
                "nativeType": "composite",
                "sourceTypeName": "AbilityInstanceLimit",
                "wireShape": "composite<composite<u32,bool>,bool>",
                "wireShapeSource": "cfg-ordered-multi-helper-wire-product",
                "wireLayout": "composite<u32,bool>",
                "wireLayoutSource": "message-unmarshal-final-nested-shape",
                "nestedTypeShape": {
                    "typeId": "16B90EF8-DC71-4B3D-B113-E1B2E231A535",
                    "identityProven": true,
                    "typeName": "AbilityInstanceLimit",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "call-frame-output-parameter+known-serialize-identity-layout",
                    "members": [{
                        "index": 0,
                        "wireShape": "u32",
                        "wireLayout": "u32",
                        "wireOrdinal": 0
                    }, {
                        "index": 1,
                        "wireShape": "bool",
                        "wireLayout": "bool",
                        "wireOrdinal": 1
                    }]
                },
                "confidence": "high"
            }, {
                "index": 1,
                "name": "symbolic_value",
                "nativeType": "composite",
                "wireShape": "composite<string,composite<string,fixed-bytes-2>>",
                "wireShapeSource": "cfg-ordered-multi-helper-wire-product",
                "wireLayout": "composite<string,fixed-bytes-2>",
                "wireLayoutSource": "message-unmarshal-final-nested-shape",
                "nestedTypeShape": {
                    "identityProven": true,
                    "typeName": "SymbolicValue",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "pcode-read-buffer-call-stack-wire-sequence+symbolic-direct-type-identity",
                    "members": [{
                        "index": 0,
                        "wireShape": "string",
                        "wireLayout": "string",
                        "wireOrdinal": 0
                    }, {
                        "index": 1,
                        "wireLayout": "fixed-bytes-2",
                        "wireOrdinal": 1
                    }]
                },
                "confidence": "high"
            }, {
                "index": 2,
                "name": "actor_ref",
                "nativeType": "ActorRef",
                "wireShape": "actor-ref",
                "wireLayout": "composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>",
                "nestedTypeShape": {
                    "identityProven": true,
                    "typeName": "ActorRef",
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "call-frame-output-parameter+known-serialize-identity-layout",
                    "members": [{
                        "index": 0,
                        "wireShape": "u32",
                        "wireLayout": "fixed-bytes-4",
                        "wireOrdinal": 0
                    }, {
                        "index": 1,
                        "wireLayout": "fixed-bytes-16",
                        "wireOrdinal": 1
                    }, {
                        "index": 2,
                        "wireLayout": "fixed-bytes-16",
                        "wireOrdinal": 2
                    }]
                },
                "confidence": "high"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema = NetworkSchema::from_ghidra_static_network_report(&report)
        .expect("normalized network schema");
    let fields = &schema.types[0].fields;
    assert_eq!(
        fields[0].wire_shape_raw.as_deref(),
        Some("composite<u32,bool>")
    );
    assert_eq!(
        fields[1].wire_shape_raw.as_deref(),
        Some("composite<string,fixed-bytes-2>")
    );
    assert_eq!(
        fields[2]
            .wire_shape
            .as_ref()
            .map(NetworkWireShape::wire_string),
        Some("actor-ref".to_owned())
    );
}

#[test]
fn parses_nested_collection_member_wire_shapes() {
    use crate::network_schema::parse::nested_member_wire_shapes;

    let shapes = nested_member_wire_shapes(
        "vec<composite<u64,vec<composite<u8,u64,u64,u64>>,u64>>",
        &[],
    )
    .expect("nested collection wire shape");

    assert_eq!(
        shapes,
        vec![
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::U64,
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::U8,
            NetworkWireScalarShape::U64,
            NetworkWireScalarShape::U64,
            NetworkWireScalarShape::U64,
            NetworkWireScalarShape::U64,
        ]
    );
}

#[test]
fn parses_conditional_wire_shapes_without_flattening_branch_structure() {
    use crate::network_schema::parse::{nested_member_wire_shapes, parse_network_wire_shape};

    let text = "boolean-choice<default-omitted<vec3,vec3>,u32>";
    let shape = parse_network_wire_shape(text).expect("conditional wire shape");

    assert_eq!(shape.wire_string(), text);
    assert_eq!(nested_member_wire_shapes(text, &[]), None);
}

#[test]
fn parses_counted_set_wire_shapes() {
    use crate::network_schema::parse::{nested_member_wire_shapes, parse_network_wire_shape};

    let text = "set<fixed-bytes-16>";
    let shape = parse_network_wire_shape(text).expect("counted set wire shape");

    assert_eq!(shape.wire_string(), text);
    assert_eq!(
        nested_member_wire_shapes(text, &[]),
        Some(vec![
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::FixedBytes(16),
        ])
    );
}

#[test]
fn parses_counted_map_wire_shapes() {
    use crate::network_schema::parse::{nested_member_wire_shapes, parse_network_wire_shape};

    let text = "map<u32,composite<entity-ref,u32,bool>>";
    let shape = parse_network_wire_shape(text).expect("counted map wire shape");

    assert_eq!(shape.wire_string(), text);
    assert_eq!(
        nested_member_wire_shapes(text, &[]),
        Some(vec![
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::U32,
            NetworkWireScalarShape::EntityRef,
            NetworkWireScalarShape::U32,
            NetworkWireScalarShape::Bool,
        ])
    );
}

#[test]
fn nested_type_wire_product_respects_proven_wire_ordinals() {
    use crate::network_schema::parse::nested_type_shape_wire_shapes;

    let mut nested = nested_shape_from_layouts(&[
        "fixed-bytes-8",
        "u64",
        "fixed-bytes-1",
        "fixed-bytes-1",
        "vec<composite<fixed-bytes-1,fixed-bytes-4,fixed-bytes-8,fixed-bytes-4>>",
    ]);
    nested.wire_order_proven = Some(true);
    for (member, ordinal) in nested.members.iter_mut().zip([0, 1, 4, 3, 2]) {
        member.wire_ordinal = Some(ordinal);
    }

    assert_eq!(
        nested_type_shape_wire_shapes(&nested, &[]),
        Some(vec![
            NetworkWireScalarShape::FixedBytes(8),
            NetworkWireScalarShape::U64,
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::FixedBytes(1),
            NetworkWireScalarShape::FixedBytes(4),
            NetworkWireScalarShape::FixedBytes(8),
            NetworkWireScalarShape::FixedBytes(4),
            NetworkWireScalarShape::FixedBytes(1),
            NetworkWireScalarShape::FixedBytes(1),
        ])
    );
}

#[test]
fn normalizes_call_frame_aggregate_from_proven_wire_ordinals() {
    let report = json!({
        "registryEntries": [{
            "uuid": "A9A48C2D-4A51-4E31-B1E8-2E85F58451D5",
            "typeIndex": 9003,
            "typeName": "Test::ReadyOwnerData",
            "fields": [{
                "index": 0,
                "wireShape": "composite<vec<string>,fixed-bytes-8,fixed-bytes-16>",
                "wireLayout": "composite<vec<string>,fixed-bytes-8,fixed-bytes-16>",
                "nestedTypeShape": {
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "call-frame-output-parameter",
                    "members": [{
                        "index": 0,
                        "wireShape": "vec<string>",
                        "wireLayout": "vec<string>",
                        "wireOrdinal": 2
                    }, {
                        "index": 1,
                        "wireShape": "u64",
                        "wireLayout": "fixed-bytes-8",
                        "wireOrdinal": 0
                    }, {
                        "index": 2,
                        "wireShape": "fixed-bytes-16",
                        "wireLayout": "fixed-bytes-16",
                        "wireOrdinal": 1
                    }]
                },
                "confidence": "message-unmarshal-call-frame-boundary"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema = NetworkSchema::from_ghidra_static_network_report(&report)
        .expect("normalized network schema");
    let field = &schema.types[0].fields[0];
    assert_eq!(
        field.wire_shape_raw.as_deref(),
        Some("composite<u64,fixed-bytes-16,vec<string>>")
    );
    assert_eq!(
        field.wire_layout.as_deref(),
        Some("composite<fixed-bytes-8,fixed-bytes-16,vec<string>>")
    );
    assert_eq!(
        field.wire_shape_source.as_deref(),
        Some("normalized-proven-call-frame-output-product")
    );
}

#[test]
fn normalizes_structured_members_without_flattening_semantics() {
    let report = json!({
        "registryEntries": [{
            "uuid": "40DE90B4-7932-44E6-92E2-7415EB03234A",
            "typeIndex": 9004,
            "typeName": "Test::UpdatePhasing",
            "fields": [{
                "index": 0,
                "wireShape": "composite<u64,u64,u8,u8,map<composite<u8,u32>,composite<u64,u32>>>",
                "wireLayout": "composite<fixed-bytes-8,u64,fixed-bytes-1,fixed-bytes-1,vec<composite<fixed-bytes-1,fixed-bytes-4,fixed-bytes-8,fixed-bytes-4>>>",
                "nestedTypeShape": {
                    "layoutProven": true,
                    "memberCoverageProven": true,
                    "wireOrderProven": true,
                    "validation": "layout-consistent-direct-type",
                    "members": [{
                        "index": 0,
                        "wireShape": "u64",
                        "wireLayout": "fixed-bytes-8",
                        "wireOrdinal": 0
                    }, {
                        "index": 1,
                        "wireShape": "u64",
                        "wireLayout": "u64",
                        "wireOrdinal": 1
                    }, {
                        "index": 2,
                        "wireShape": "u8",
                        "wireLayout": "fixed-bytes-1",
                        "wireOrdinal": 4
                    }, {
                        "index": 3,
                        "wireShape": "u8",
                        "wireLayout": "fixed-bytes-1",
                        "wireOrdinal": 3
                    }, {
                        "index": 4,
                        "wireShape": "map<composite<u8,u32>,composite<u64,u32>>",
                        "wireLayout": "vec<composite<fixed-bytes-1,fixed-bytes-4,fixed-bytes-8,fixed-bytes-4>>",
                        "wireOrdinal": 2
                    }]
                },
                "confidence": "message-unmarshal-call-frame-boundary"
            }]
        }],
        "fieldRegistrationFunctions": []
    });

    let schema = NetworkSchema::from_ghidra_static_network_report(&report)
        .expect("normalized network schema");
    let field = &schema.types[0].fields[0];
    assert_eq!(
        field.wire_shape_raw.as_deref(),
        Some("composite<u64,u64,map<composite<u8,u32>,composite<u64,u32>>,u8,u8>")
    );
    assert_eq!(
        field.wire_layout.as_deref(),
        Some(
            "composite<fixed-bytes-8,u64,vec<composite<fixed-bytes-1,fixed-bytes-4,fixed-bytes-8,fixed-bytes-4>>,fixed-bytes-1,fixed-bytes-1>"
        )
    );
}

#[test]
fn parses_actor_instantiation_parameters_wire_shape() {
    use crate::network_schema::parse::parse_network_wire_shape;

    let text = "actor-instantiation-parameters";
    let shape = parse_network_wire_shape(text).expect("actor parameter wire shape");

    assert_eq!(shape, NetworkWireShape::ActorInstantiationParameters);
    assert_eq!(shape.wire_string(), text);
}

fn fragment_access_message_signatures() -> Vec<NetworkMessageSignature> {
    vec![
        NetworkMessageSignature {
            type_id: Some(uuid!("96a58e69-7bd5-45c5-86e4-daf9f5eb1e86")),
            type_index: Some(397),
            name: Some("Replicate::RegisterFragmentAccessMsg".to_owned()),
            rust_name: Some("RegisterFragmentAccessMsg".to_owned()),
            source: None,
            fields: fragment_access_fields(),
        },
        NetworkMessageSignature {
            type_id: Some(uuid!("2b7640e0-4204-4e52-998a-c2db02e0a480")),
            type_index: Some(399),
            name: Some("Replicate::UnregisterFragmentAccessMsg".to_owned()),
            rust_name: Some("UnregisterFragmentAccessMsg".to_owned()),
            source: None,
            fields: fragment_access_fields(),
        },
        NetworkMessageSignature {
            type_id: Some(uuid!("951ef3ed-c9a0-4e3d-a6fd-7fe0673d28d2")),
            type_index: Some(422),
            name: Some("ReplicateClient::FragmentUpdateMsg".to_owned()),
            rust_name: Some("FragmentUpdateMsg".to_owned()),
            source: None,
            fields: vec![
                message_field_signature(0, "TargetRef", "ActorRef"),
                message_field_signature(1, "Key", "FragmentKey"),
                message_field_signature(2, "Fragment", "BaselineableFragment"),
            ],
        },
    ]
}

fn fragment_access_fields() -> Vec<NetworkMessageFieldSignature> {
    vec![
        message_field_signature(0, "ProxyRef", "ActorRef"),
        message_field_signature(1, "Key", "FragmentKey"),
    ]
}

fn message_field_signature(
    index: u32,
    name: &str,
    native_type: &str,
) -> NetworkMessageFieldSignature {
    NetworkMessageFieldSignature {
        index: Some(index),
        name: name.to_owned(),
        rust_type: None,
        native_type: Some(native_type.to_owned()),
        wire_shape: None,
    }
}
