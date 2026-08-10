use serde_json::json;
use uuid::uuid;

use super::*;

#[test]
fn exact_serialize_layout_recovers_collapsed_structured_member_names() {
    let payload_id = uuid!("e8891450-f65c-44a5-91a3-c0f9f8ce6760");
    let u32_id = uuid!("43da906b-7def-4ca8-9790-854106d3f983");
    let string_id = uuid!("3da0c24d-9f75-4e0f-9b72-7b226c640d61");
    let shape = serde_json::from_value(json!({
        "typeId": payload_id,
        "identityProven": true,
        "typeName": "TestPayload",
        "typeNameFull": "TestPayload",
        "memberNameSource": "synthetic-layout-member",
        "memberNamesProven": false,
        "memberCoverageProven": true,
        "wireOrderProven": true,
        "members": [{
            "index": 0,
            "offset": "0x8",
            "nativeOffset": "0x8",
            "name": "field_0",
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "wireShape": "composite<u32,string>",
            "wireOrdinal": 0
        }]
    }))
    .expect("nested type shape");
    let source = NetworkSerializeType {
        type_id: payload_id,
        kind: NetworkSerializeKind::Struct,
        name: "TestPayload".to_owned(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: None,
        emits_source: true,
        factory: None,
        field_count: 2,
        fields: vec![
            crate::network_schema::NetworkSerializeField {
                name: "m_count".to_owned(),
                type_id: u32_id,
                resolved_type: ResolvedType::Scalar(ScalarType::U32),
                offset: Some(8),
                is_base_class: false,
            },
            crate::network_schema::NetworkSerializeField {
                name: "m_label".to_owned(),
                type_id: string_id,
                resolved_type: ResolvedType::Scalar(ScalarType::String),
                offset: Some(16),
                is_base_class: false,
            },
        ],
        variant_count: 0,
        direct_dependency_type_ids: vec![u32_id, string_id],
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    };
    let serialize_types = BTreeMap::from([(payload_id, &source)]);

    let reconciled = try_reconcile_serialize_backed_member_names(shape, &serialize_types)
        .expect("complete source product reconciles");

    assert_eq!(reconciled.member_names_proven, Some(true));
    assert_eq!(reconciled.members.len(), 2);
    assert_eq!(reconciled.members[0].name.as_deref(), Some("m_count"));
    assert_eq!(reconciled.members[0].wire_shape.as_deref(), Some("u32"));
    assert_eq!(reconciled.members[1].name.as_deref(), Some("m_label"));
    assert_eq!(reconciled.members[1].wire_shape.as_deref(), Some("string"));
}

#[test]
fn exact_serialize_layout_recovers_member_name_from_unique_offset() {
    let payload_id = uuid!("aabff5de-d467-4a95-a0c2-4d8896683182");
    let vector_id = uuid!("6b71aef7-b8af-5a7a-b953-5ffa231c845d");
    let element_id = uuid!("2758ac4e-a17b-4990-85d2-6060e89aa18d");
    let shape = serde_json::from_value(json!({
        "typeId": payload_id,
        "identityProven": true,
        "typeName": "ReplicatedEnterHousingData",
        "memberNamesProven": false,
        "memberCoverageProven": true,
        "wireOrderProven": true,
        "members": [{
            "nativeOffset": "0x8",
            "name": "field_0",
            "nameProven": false,
            "wireShape": "vec<composite<string,u32,u64>>",
            "wireOrdinal": 0
        }]
    }))
    .expect("nested type shape");
    let source = NetworkSerializeType {
        type_id: payload_id,
        kind: NetworkSerializeKind::Struct,
        name: "ReplicatedEnterHousingData".to_owned(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: None,
        emits_source: true,
        factory: None,
        field_count: 1,
        fields: vec![crate::network_schema::NetworkSerializeField {
            name: "m_topHouses".to_owned(),
            type_id: vector_id,
            resolved_type: ResolvedType::Sequence {
                kind: crate::types::SequenceKind::Vector,
                element: Box::new(ResolvedType::Named {
                    type_id: element_id,
                    source_name: "TopHouseData".to_owned(),
                }),
                capacity: None,
            },
            offset: Some(8),
            is_base_class: false,
        }],
        variant_count: 0,
        direct_dependency_type_ids: vec![element_id],
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    };
    let serialize_types = BTreeMap::from([(payload_id, &source)]);

    let reconciled = try_reconcile_serialize_backed_member_names(shape, &serialize_types)
        .expect("unique source offset reconciles");

    assert_eq!(reconciled.member_names_proven, Some(true));
    assert_eq!(reconciled.members[0].name.as_deref(), Some("m_topHouses"));
    assert_eq!(
        reconciled.members[0].wire_shape.as_deref(),
        Some("vec<composite<string,u32,u64>>")
    );
    assert_eq!(
        reconciled.members[0].name_evidence.as_deref(),
        Some("exact-type-id+native-offset")
    );
}
