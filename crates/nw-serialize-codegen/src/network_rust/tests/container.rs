use super::*;

fn replicated_container_schema(plan: serde_json::Value) -> NetworkSchema {
    replicated_container_schema_with_shape(plan, None)
}

fn replicated_container_schema_with_shape(
    plan: serde_json::Value,
    value_type_shape: Option<serde_json::Value>,
) -> NetworkSchema {
    replicated_container_schema_with_named_shape("values", plan, value_type_shape)
}

fn replicated_container_schema_with_named_shape(
    field_name: &str,
    plan: serde_json::Value,
    value_type_shape: Option<serde_json::Value>,
) -> NetworkSchema {
    let mut plan = plan;
    let plan = plan.as_object_mut().expect("container plan object");
    plan.insert(
        "unmarshalReconciliation".to_owned(),
        json!("complete-physical-sequence-agreement"),
    );
    plan.insert(
        "unmarshalAnalysisStatus".to_owned(),
        json!("exact-loop-codec-count"),
    );
    let mut vtable = json!({
        "address": "NewWorld+0x8123456",
        "fieldCount": 1,
        "fullContainerPlan": plan,
        "slots": []
    });
    if let Some(value_type_shape) = value_type_shape {
        vtable
            .as_object_mut()
            .expect("vtable object")
            .insert("valueTypeShape".to_owned(), value_type_shape);
    }
    NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "EA233975-66A9-4A2A-A493-2B6A43674868",
            "typeIndex": 4242,
            "typeName": "MB::PlanBackedReplicatedState",
            "fields": [{
                "index": 0,
                "group": 0,
                "name": field_name,
                "handlerVtable": "NewWorld+0x8123456",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [vtable]
    }))
    .expect("plan-backed schema")
}

#[test]
fn anonymous_container_value_layout_emits_support_struct() {
    let schema = replicated_container_schema_with_shape(
        json!({
            "storageKind": "vector",
            "elementStride": "0x58",
            "keyCodecs": [],
            "valueCodecs": [
                { "wireLayout": "fixed-bytes-16" },
                { "wireShape": "u64" },
                { "wireShape": "u64" },
                { "wireLayout": "fixed-bytes-16" },
                { "wireShape": "u64" }
            ]
        }),
        Some(json!({
            "identityProven": false,
            "layoutProven": true,
            "typeName": "Value",
            "typeNameSource": "synthetic-anonymous-composite",
            "memberNameSource": "synthetic-wire-ordinal",
            "memberNamesProven": false,
            "memberCoverageProven": true,
            "wireOrderProven": true,
            "nativeSize": 88,
            "members": [
                { "index": 0, "offset": "0x10", "name": "field_0", "wireLayout": "fixed-bytes-16", "wireOrdinal": 0 },
                { "index": 1, "offset": "0x20", "name": "field_1", "wireShape": "u64", "wireOrdinal": 1 },
                { "index": 2, "offset": "0x28", "name": "field_2", "wireShape": "u64", "wireOrdinal": 2 },
                { "index": 3, "offset": "0x40", "name": "field_3", "wireLayout": "fixed-bytes-16", "wireOrdinal": 3 },
                { "index": 4, "offset": "0x50", "name": "field_4", "wireShape": "u64", "wireOrdinal": 4 }
            ]
        })),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::std::vec::Vec<ValuesValue>")
    );
    assert!(output.source.contains("pub struct ValuesValue"));
    assert!(output.source.contains("pub field_0: [u8; 16]"));
    assert!(output.source.contains("pub field_4: u64"));
}

#[test]
fn rtti_container_value_with_nested_sequence_emits_support_struct() {
    let members = json!([
        { "wireLayout": "fixed-bytes-16" },
        { "wireShape": "u64" },
        { "wireShape": "u64" },
        { "wireLayout": "fixed-bytes-16" },
        { "wireShape": "u64" },
        { "wireShape": "f32" },
        { "wireShape": "f32" },
        { "wireShape": "f32" },
        { "wireShape": "u64" },
        { "wireShape": "u64" },
        { "wireShape": "u16" },
        { "wireShape": "bool" },
        {
            "wireShape": "vec<u32>",
            "wireLayout": "vec<u32>",
            "memberSemantics": "counted-sequence",
            "members": [
                { "wireShape": "vlq-u32" },
                { "wireShape": "u32" }
            ]
        },
        { "wireShape": "f32" },
        { "wireShape": "bool" },
        { "wireShape": "bool" },
        { "wireShape": "u64" },
        { "wireShape": "u32" }
    ]);
    let shape_members = [
        "fixed-bytes-16",
        "u64",
        "u64",
        "fixed-bytes-16",
        "u64",
        "vec3",
        "u64",
        "u64",
        "u16",
        "bool",
        "vec<u32>",
        "f32",
        "bool",
        "bool",
        "u64",
        "u32",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, wire_shape)| {
        let mut member = json!({
            "index": index,
            "offset": index,
            "name": format!("field_{index}"),
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "wireShape": wire_shape,
            "wireOrdinal": index
        });
        let scalar_type_id = match wire_shape {
            "u16" => Some(("ECA0B403-C4F8-4B86-95FC-81688D046E40", "az-type-info-fold")),
            "u32" => Some(("43DA906B-7DEF-4CA8-9790-854106D3F983", "az-type-info-fold")),
            "u64" => Some((
                "D6597933-47CD-4FC8-B911-63F3E2B0993A",
                "serialize-unique-leaf-name",
            )),
            "f32" => Some(("EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D", "az-type-info-fold")),
            _ => None,
        };
        if let Some((type_id, type_id_source)) = scalar_type_id {
            member["typeId"] = json!(type_id);
            member["typeIdSource"] = json!(type_id_source);
            member["typeIdentityProven"] = json!(false);
        }
        member
    })
    .collect::<Vec<_>>();
    let schema = replicated_container_schema_with_named_shape(
        "ownedHouses",
        json!({
            "storageKind": "vector",
            "elementStride": 240,
            "keyCodecs": [],
            "valueCodecs": [{
                "memberSemantics": "linear-sequence",
                "members": members
            }]
        }),
        Some(json!({
            "typeId": "CA00FC6D-8593-431B-B4BA-3F235F1BDFC1",
            "typeIdSource": "unmarshal-full-element-vptr",
            "identityProven": true,
            "identitySource": "unmarshal-full-element-vptr+affine-element-stride",
            "typeName": "ReplicatedOwnedHouseData",
            "typeNameFull": "ReplicatedOwnedHouseData",
            "typeNameSource": "az-rtti-vtable-provider",
            "memberNameSource": "synthetic-wire-ordinal",
            "memberNamesProven": false,
            "layoutProven": true,
            "memberCoverageProven": true,
            "wireOrderProven": true,
            "wireOrderSource": "marshal+unmarshal-custom-codec-order",
            "nativeSize": 240,
            "members": shape_members
        })),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::std::vec::Vec<ReplicatedOwnedHouseData>")
    );
    assert!(
        output
            .source
            .contains("pub struct ReplicatedOwnedHouseData")
    );
    assert!(output.source.contains("pub field_5: ::glam::Vec3"));
    assert!(output.source.contains("pub field_10: ::std::vec::Vec<u32>"));
    assert!(
        output
            .source
            .contains("impl ::nw_network::serialize::Marshal for ReplicatedOwnedHouseData")
    );
}

#[test]
fn constructor_grouped_owned_house_data_emits_semantic_members() {
    let typeless_ref_id = uuid!("6328304a-b754-4ad3-bf78-87236958b55b");
    let gde_ref_id = uuid!("17207dac-730b-4793-b975-23591a950260");
    let time_point_id = uuid!("24fbf222-8cf9-4539-b313-34726b8fc675");
    let members = json!([
        { "wireLayout": "fixed-bytes-16" },
        { "wireShape": "u64" },
        { "wireShape": "u64" },
        { "wireLayout": "fixed-bytes-16" },
        { "wireShape": "u64" },
        { "wireShape": "f32" },
        { "wireShape": "f32" },
        { "wireShape": "f32" },
        { "wireShape": "u64" },
        { "wireShape": "u64" },
        { "wireShape": "u16" },
        { "wireShape": "bool" },
        {
            "wireShape": "vec<u32>",
            "wireLayout": "vec<u32>",
            "memberSemantics": "counted-sequence",
            "members": [
                { "wireShape": "vlq-u32" },
                { "wireShape": "u32" }
            ]
        },
        { "wireShape": "f32" },
        { "wireShape": "bool" },
        { "wireShape": "bool" },
        { "wireShape": "u64" },
        { "wireShape": "u32" }
    ]);
    let semantic_members = json!([
        {
            "index": 0,
            "offset": "0x10",
            "nativeOffset": "0x10",
            "name": "remote_typeless_server_facet_ref",
            "nameSource": "synthetic-provider-type",
            "nameProven": false,
            "nativeType": "RemoteTypelessServerFacetRef",
            "typeId": typeless_ref_id.to_string(),
            "typeIdSource": "constructor-stored-az-rtti-provider",
            "typeIdentityProven": true,
            "wireShape": "composite<fixed-bytes-16,u64,u64>",
            "wireLayout": "composite<fixed-bytes-16,u64,u64>",
            "wireOrdinal": 0
        },
        {
            "index": 1,
            "offset": "0x40",
            "nativeOffset": "0x40",
            "name": "remote_server_gde_ref",
            "nameSource": "synthetic-provider-type",
            "nameProven": false,
            "nativeType": "RemoteServerGDERef",
            "typeId": gde_ref_id.to_string(),
            "typeIdSource": "constructor-stored-az-rtti-provider",
            "typeIdentityProven": true,
            "wireShape": "composite<fixed-bytes-16,u64>",
            "wireLayout": "composite<fixed-bytes-16,u64>",
            "wireOrdinal": 1
        },
        {
            "index": 2,
            "offset": "0x70",
            "nativeOffset": "0x70",
            "name": "field_5",
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "nativeType": "AZ::Vector3",
            "wireShape": "vec3",
            "wireLayout": "vec3",
            "wireOrdinal": 2
        },
        {
            "index": 3,
            "offset": "0x80",
            "nativeOffset": "0x80",
            "name": "wall_clock_time_point",
            "nameSource": "synthetic-provider-type",
            "nameProven": false,
            "nativeType": "WallClockTimePoint",
            "typeId": time_point_id.to_string(),
            "typeIdSource": "constructor-stored-az-rtti-provider",
            "typeIdentityProven": true,
            "wireShape": "u64",
            "wireLayout": "u64",
            "wireOrdinal": 3
        },
        {
            "index": 4,
            "offset": "0x90",
            "nativeOffset": "0x90",
            "name": "wall_clock_time_point_2",
            "nameSource": "synthetic-provider-type",
            "nameProven": false,
            "nativeType": "WallClockTimePoint",
            "typeId": time_point_id.to_string(),
            "typeIdSource": "constructor-stored-az-rtti-provider",
            "typeIdentityProven": true,
            "wireShape": "u64",
            "wireLayout": "u64",
            "wireOrdinal": 4
        },
        {
            "index": 5,
            "offset": "0xa0",
            "nativeOffset": "0xa0",
            "name": "field_8",
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "nativeType": "u16",
            "wireShape": "u16",
            "wireLayout": "u16",
            "wireOrdinal": 5
        },
        {
            "index": 6,
            "offset": "0xa2",
            "nativeOffset": "0xa2",
            "name": "field_9",
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "nativeType": "bool",
            "wireShape": "bool",
            "wireLayout": "bool",
            "wireOrdinal": 6
        },
        {
            "index": 7,
            "offset": "0xa8",
            "nativeOffset": "0xa8",
            "name": "field_10",
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "nativeType": "AZStd::vector<AZ::u32>",
            "wireShape": "vec<u32>",
            "wireLayout": "vec<u32>",
            "wireOrdinal": 7
        },
        {
            "index": 8,
            "offset": "0xc8",
            "nativeOffset": "0xc8",
            "name": "field_11",
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "nativeType": "float",
            "wireShape": "f32",
            "wireLayout": "f32",
            "wireOrdinal": 8
        },
        {
            "index": 9,
            "offset": "0xe5",
            "nativeOffset": "0xe5",
            "name": "field_12",
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "nativeType": "bool",
            "wireShape": "bool",
            "wireLayout": "bool",
            "wireOrdinal": 9
        },
        {
            "index": 10,
            "offset": "0xe6",
            "nativeOffset": "0xe6",
            "name": "field_13",
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "nativeType": "bool",
            "wireShape": "bool",
            "wireLayout": "bool",
            "wireOrdinal": 10
        },
        {
            "index": 11,
            "offset": "0xd0",
            "nativeOffset": "0xd0",
            "name": "wall_clock_time_point_3",
            "nameSource": "synthetic-provider-type",
            "nameProven": false,
            "nativeType": "WallClockTimePoint",
            "typeId": time_point_id.to_string(),
            "typeIdSource": "constructor-stored-az-rtti-provider",
            "typeIdentityProven": true,
            "wireShape": "u64",
            "wireLayout": "u64",
            "wireOrdinal": 11
        },
        {
            "index": 12,
            "offset": "0xe0",
            "nativeOffset": "0xe0",
            "name": "field_15",
            "nameSource": "synthetic-wire-ordinal",
            "nameProven": false,
            "nativeType": "u32",
            "wireShape": "u32",
            "wireLayout": "u32",
            "wireOrdinal": 12
        }
    ]);
    let mut schema = replicated_container_schema_with_named_shape(
        "ownedHouses",
        json!({
            "storageKind": "vector",
            "elementStride": 240,
            "keyCodecs": [],
            "valueCodecs": [{
                "memberSemantics": "linear-sequence",
                "members": members
            }]
        }),
        Some(json!({
            "typeId": "CA00FC6D-8593-431B-B4BA-3F235F1BDFC1",
            "typeIdSource": "unmarshal-full-element-vptr",
            "identityProven": true,
            "identitySource": "unmarshal-full-element-vptr+exact-custom-codec-helper",
            "typeName": "ReplicatedOwnedHouseData",
            "typeNameFull": "ReplicatedOwnedHouseData",
            "typeNameSource": "az-rtti-vtable-provider",
            "memberBase": "element",
            "memberNameSource": "synthetic-constructor-type",
            "memberNamesProven": false,
            "layoutProven": true,
            "memberCoverageProven": true,
            "wireOrderProven": true,
            "wireOrderSource": "marshal+unmarshal-custom-codec-order+constructor-layout",
            "validation": "exact-custom-codec-wire-projection+constructor-provider-layout-cover",
            "nativeSize": 240,
            "members": semantic_members
        })),
    );
    schema.merge_serialize_codegen_unit(
        &SerializeCodegenUnit {
            items: vec![
                named_value_item(typeless_ref_id, "RemoteTypelessServerFacetRef", []),
                named_value_item(gde_ref_id, "RemoteServerGDERef", []),
                named_value_item(time_point_id, "WallClockTimePoint", []),
            ],
        },
        Some("serialize.json".to_owned()),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::std::vec::Vec<ReplicatedOwnedHouseData>")
    );
    assert!(output.source.contains(
        "pub remote_typeless_server_facet_ref: ::nw_network::source::RemoteTypelessServerFacetRef"
    ));
    assert!(
        output
            .source
            .contains("pub remote_server_gde_ref: ::nw_network::source::RemoteServerGDERef")
    );
    for field_name in [
        "wall_clock_time_point",
        "wall_clock_time_point_2",
        "wall_clock_time_point_3",
    ] {
        assert!(output.source.contains(&format!(
            "pub {field_name}: ::nw_network::source::WallClockTimePoint"
        )));
    }
    assert!(output.source.contains("pub field_5: ::glam::Vec3"));
    assert!(output.source.contains("pub field_10: ::std::vec::Vec<u32>"));
    for old_scalar_field in ["field_6", "field_7", "field_14"] {
        assert!(
            !output
                .source
                .contains(&format!("pub {old_scalar_field}: u64"))
        );
    }
    assert!(!output.source.contains("pub field_0: [u8; 16]"));
}

#[test]
fn mismatched_unproven_nested_scalar_identity_remains_a_blocker() {
    let schema = replicated_container_schema_with_named_shape(
        "values",
        json!({
            "storageKind": "vector",
            "valueCodecs": [{ "wireShape": "u64" }]
        }),
        Some(json!({
            "typeId": "11111111-1111-4111-8111-111111111111",
            "typeIdSource": "unmarshal-full-element-vptr",
            "identityProven": true,
            "identitySource": "unmarshal-full-element-vptr",
            "typeName": "Value",
            "typeNameFull": "Value",
            "typeNameSource": "az-rtti-vtable-provider",
            "memberNameSource": "synthetic-wire-ordinal",
            "memberNamesProven": false,
            "layoutProven": true,
            "memberCoverageProven": true,
            "wireOrderProven": true,
            "wireOrderSource": "marshal+unmarshal-custom-codec-order",
            "members": [{
                "index": 0,
                "offset": 0,
                "name": "field_0",
                "nameSource": "synthetic-wire-ordinal",
                "nameProven": false,
                "typeId": "6383F1D3-BB27-4E6B-A49A-6409B2059EAA",
                "typeIdSource": "layout-only-correlation",
                "typeIdentityProven": false,
                "wireShape": "u64",
                "wireOrdinal": 0
            }]
        })),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(!plan.can_generate, "{plan:#?}");
    assert_eq!(plan.blocked_reasons, ["invalid-evidence:1"]);
    assert_eq!(
        plan.evidence_issues[0].kind,
        NetworkEvidenceIssueKind::UnprovenNestedMemberTypeIdentity
    );
}

#[test]
fn matching_unproven_nested_string_identity_is_wire_equivalent() {
    let schema = replicated_container_schema_with_named_shape(
        "values",
        json!({
            "storageKind": "vector",
            "valueCodecs": [{ "wireShape": "string" }]
        }),
        Some(json!({
            "typeId": "11111111-1111-4111-8111-111111111111",
            "typeIdSource": "unmarshal-full-element-vptr",
            "identityProven": true,
            "identitySource": "unmarshal-full-element-vptr",
            "typeName": "Value",
            "typeNameFull": "Value",
            "typeNameSource": "az-rtti-vtable-provider",
            "memberNameSource": "synthetic-wire-ordinal",
            "memberNamesProven": false,
            "layoutProven": true,
            "memberCoverageProven": true,
            "wireOrderProven": true,
            "wireOrderSource": "marshal+unmarshal-custom-codec-order",
            "members": [{
                "index": 0,
                "offset": 0,
                "name": "field_0",
                "nameSource": "synthetic-wire-ordinal",
                "nameProven": false,
                "typeId": "03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9",
                "typeIdSource": "az-type-info-fold",
                "typeIdentityProven": false,
                "wireShape": "string",
                "wireOrdinal": 0
            }]
        })),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert!(plan.evidence_issues.is_empty(), "{plan:#?}");
    assert!(output.source.contains("pub field_0: String"));
}

#[test]
fn unreflected_rtti_map_value_emits_named_support_struct() {
    let schema = replicated_container_schema_with_named_shape(
        "globalMapData",
        json!({
            "storageKind": "index-map",
            "keyCodecs": [{ "nativeType": "u64", "wireShape": "u64" }],
            "valueCodecs": [{
                "memberSemantics": "linear-sequence",
                "elementOffset": "0x18",
                "members": [
                    { "nativeType": "AZ::Vector2", "wireShape": "vec2", "elementOffset": "0x20" },
                    { "nativeType": "AZ::u16", "wireShape": "u16", "elementOffset": "0x28" },
                    { "nativeType": "AZ::u32", "wireShape": "u32", "elementOffset": "0x2c" },
                    { "nativeType": "bool", "wireShape": "bool", "elementOffset": "0x30" }
                ]
            }]
        }),
        Some(json!({
            "typeId": "0DC02DD0-993E-48C0-8B60-5715D4383B0D",
            "typeIdSource": "unmarshal-full-element-vptr+linked-node-allocation+stack-vptr-agreement",
            "identityProven": true,
            "identitySource": "unmarshal-full-element-vptr+linked-node-allocation+stack-vptr-agreement+complete-container-plan-layout",
            "typeName": "GlobalMapData",
            "typeNameFull": "GlobalMapData",
            "typeNameSource": "az-rtti-vtable-provider",
            "memberNameSource": "synthetic-wire-ordinal",
            "memberNamesProven": false,
            "memberCoverageProven": true,
            "wireOrderProven": true,
            "nativeSize": "0x20",
            "nativeSizeSource": "linked-node-allocation-size-minus-value-offset",
            "members": [
                { "index": 0, "offset": "0x8", "name": "field_0", "nativeType": "AZ::Vector2", "wireShape": "vec2", "wireOrdinal": 0 },
                { "index": 1, "offset": "0x10", "name": "field_1", "nativeType": "AZ::u16", "wireShape": "u16", "wireOrdinal": 1 },
                { "index": 2, "offset": "0x14", "name": "field_2", "nativeType": "AZ::u32", "wireShape": "u32", "wireOrdinal": 2 },
                { "index": 3, "offset": "0x18", "name": "field_3", "nativeType": "bool", "wireShape": "bool", "wireOrdinal": 3 }
            ]
        })),
    );

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<u64, GlobalMapData>")
    );
    assert!(output.source.contains("pub struct GlobalMapData"));
    assert!(output.source.contains("pub field_0: ::glam::Vec2"));
    assert!(output.source.contains("pub field_1: u16"));
    assert!(output.source.contains("pub field_2: u32"));
    assert!(output.source.contains("pub field_3: bool"));
    assert!(
        output
            .source
            .contains("impl ::nw_network::serialize::Marshal for GlobalMapData")
    );
    assert!(
        output
            .source
            .contains("impl ::nw_network::serialize::Unmarshal for GlobalMapData")
    );
}

#[test]
fn linear_container_plan_emits_ordered_support_struct() {
    let schema = replicated_container_schema(json!({
        "storageKind": "index-map",
        "keyCodecs": [{ "nativeType": "u32", "wireShape": "u32" }],
        "valueCodecs": [{
            "memberSemantics": "linear-sequence",
            "members": [
                { "wireShape": "u32" },
                { "wireShape": "u32" },
                { "wireShape": "u32" }
            ]
        }]
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<u32, ValuesValue>")
    );
    assert!(output.source.contains("pub struct ValuesValue"));
    assert!(output.source.contains("pub field_0: u32"));
    assert!(output.source.contains("pub field_1: u32"));
    assert!(output.source.contains("pub field_2: u32"));
}

#[test]
fn vector_container_plan_preserves_value_marshaler() {
    let schema = replicated_container_schema(json!({
        "storageKind": "vector",
        "elementStride": "0x8",
        "keyCodecs": [],
        "valueCodecs": [{ "nativeType": "u64", "wireShape": "vlq-u64" }]
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let field = &output.report.state_generation_plans[0].fields[0];

    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("::std::vec::Vec<u64>")
    );
    assert!(
        field
            .rust_field_type
            .as_deref()
            .unwrap()
            .contains("VlqU64Marshaler")
    );
}

#[test]
fn handler_container_type_emits_semantic_map_without_loop_plan() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
            "typeIndex": 3362,
            "typeName": "MB::SlayerScriptReplicatedState",
            "fields": [{
                "index": 0,
                "group": 0,
                "name": "spawnedEntityIdsBySpawnerId",
                "handlerVtable": "NewWorld+0x81bf3d0",
                "confidence": "register-field-call"
            }]
        }],
        "fieldRegistrationFunctions": [],
        "fieldHandlerVtables": [{
            "address": "NewWorld+0x81bf3d0",
            "fieldCount": 1,
            "handlerContainerType": {
                "storageKind": "index-map",
                "keyNativeType": "AZ::Crc32",
                "valueNativeType": "AZ::EntityId",
                "storageNativeType": "AZStd::unordered_map<AZ::Crc32, AZ::EntityId>",
                "keyMarshalerType": "Amazon::Pervasives::Marshaller<AZ::Crc32>",
                "valueMarshalerType": "Amazon::Pervasives::Marshaller<AZ::EntityId>",
                "source": "handler-constructor-template"
            },
            "wireShape": "replicated-container<u32,u64>",
            "wireShapeSource": "replicated-container-delta-key-full-key-value",
            "slots": []
        }]
    }))
    .expect("handler-backed container schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [3362]).unwrap();
    let field = &output.report.state_generation_plans[0].fields[0];

    assert!(output.report.state_generation_plans[0].can_generate);
    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>")
    );
    assert_eq!(
        field.rust_field_type.as_deref(),
        Some(
            "::nw_network::serialize::ReplicatedContainer<::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>, { ::nw_network::serialize::WIRE_VEC_CAP }, ::nw_network::serialize::DefaultMarshaler<::nw_network::Crc32>, ::nw_network::serialize::DefaultMarshaler<::nw_network::EntityId>>"
        )
    );
}

#[test]
fn non_linear_container_helper_is_an_explicit_blocker() {
    let schema = replicated_container_schema(json!({
        "storageKind": "index-map",
        "keyCodecs": [{ "wireShape": "u32" }],
        "valueCodecs": [{
            "memberSemantics": "cfg-reachable",
            "members": [{ "wireShape": "u32" }, { "wireShape": "u64" }]
        }]
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(!plan.can_generate);
    assert_eq!(plan.blocked_reasons, ["non-linear-container-codec:1"]);
    assert_eq!(
        plan.fields[0].blocked_reason.as_deref(),
        Some("non-linear-container-codec")
    );
}

#[test]
fn externally_gated_suffix_uses_the_registered_default_profile() {
    let condition = json!({
        "resolverObject": "NewWorld+0x1000",
        "resolverVtable": "NewWorld+0x2000",
        "resolverSlot": 1,
        "resolver": "NewWorld+0x3000",
        "conditionStorage": "NewWorld+0x100c",
        "conditionOffset": "0xc",
        "owner": "NewWorld+0xfb0",
        "subobjectOffset": "0x50",
        "destructorThunk": "NewWorld+0x4000",
        "completeDestructor": "NewWorld+0x5000",
        "initializer": "NewWorld+0x6000",
        "nameField": "NewWorld+0x1010",
        "nameOffset": "0x10",
        "nameBegin": "NewWorld+0x7000",
        "nameEnd": "NewWorld+0x7017",
        "name": "feature.enabled",
        "defaultValue": true,
        "defaultWrite": "NewWorld+0x8000",
        "defaultCallsite": "NewWorld+0x8010",
        "defaultTarget": "NewWorld+0x9000",
        "evidenceSource": "static-vtable-dispatch+adjustor-thunk+initializer-writes+resolver-default-flow"
    });
    let schema = replicated_container_schema(json!({
        "storageKind": "vector",
        "valueCodecs": [{
            "memberSemantics": "optional-suffix",
            "members": [{ "wireShape": "u64" }],
            "optionalMembers": [{
                "wireShape": "u32",
                "guards": [{
                    "branch": "NewWorld+0xa000",
                    "kind": "global-boolean",
                    "condition": "not-equal-zero",
                    "memberOnTrue": true,
                    "storageAddress": "NewWorld+0x100c",
                    "externalCondition": condition,
                    "evidenceSource": "dominating-cbranch-pcode-storage+external-condition-proof"
                }]
            }]
        }]
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::std::vec::Vec<ValuesValue>")
    );
    assert!(output.source.contains("pub field_0: u64"));
    assert!(output.source.contains("pub field_1: u32"));
}

#[test]
fn raw_container_key_layout_emits_an_opaque_fixed_width_key() {
    let schema = replicated_container_schema(json!({
        "storageKind": "index-map",
        "keyCodecs": [{ "wireLayout": "fixed-bytes-16" }],
        "valueCodecs": [{ "nativeType": "u8", "wireShape": "u8" }]
    }));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<[u8; 16], u8>")
    );
}

#[test]
fn proven_uid_container_key_uses_the_resolved_generic_type() {
    let uid_type_id = uuid!("3485f20a-98c0-5315-876b-21bcd23a7bc0");
    let mut schema = replicated_container_schema(json!({
        "storageKind": "index-map",
        "keyCodecs": [{
            "nativeType": "Amazon::Pervasives::UID",
            "typeId": uid_type_id.to_string(),
            "typeIdSource": "constructor-receiver+owner-rtti+serialize-field-offset",
            "typeIdentityProven": true,
            "wireLayout": "fixed-bytes-16"
        }],
        "valueCodecs": [{ "nativeType": "u32", "wireShape": "u32" }]
    }));
    schema.serialize_types.push(NetworkSerializeType {
        type_id: uid_type_id,
        kind: NetworkSerializeKind::Struct,
        name: "Amazon::Pervasives::UID<AZ::u16>".to_owned(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: Some(ResolvedType::Uid {
            type_id: Some(uid_type_id),
        }),
        emits_source: false,
        factory: None,
        field_count: 0,
        fields: Vec::new(),
        variant_count: 0,
        direct_dependency_type_ids: Vec::new(),
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    });

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [4242]).unwrap();
    let plan = &output.report.state_generation_plans[0];

    assert!(plan.can_generate, "{plan:#?}");
    assert_eq!(
        plan.fields[0].rust_value_type.as_deref(),
        Some("::nw_network::serialize::IndexMap<::nw_network::source::AzUuid, u32>")
    );
}

#[test]
fn canonical_reference_identities_use_the_runtime_wire_types() {
    let actor_ref_type_id = uuid!("0638e28c-ab7b-4ba4-84ac-0353038e6fdc");
    let client_ref_type_id = uuid!("c148c555-3264-41f7-a335-e48b65f91728");
    let actor_ref = NetworkSerializeType {
        type_id: actor_ref_type_id,
        kind: NetworkSerializeKind::Struct,
        name: "Amazon::Hub::ActorRef".to_owned(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: None,
        emits_source: true,
        factory: None,
        field_count: 0,
        fields: Vec::new(),
        variant_count: 0,
        direct_dependency_type_ids: Vec::new(),
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    };
    let client_ref = NetworkSerializeType {
        type_id: client_ref_type_id,
        kind: NetworkSerializeKind::Struct,
        name: "ClientRef".to_owned(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: None,
        emits_source: true,
        factory: None,
        field_count: 1,
        fields: Vec::new(),
        variant_count: 0,
        direct_dependency_type_ids: vec![actor_ref_type_id],
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    };
    let serialize_types = BTreeMap::from([
        (actor_ref_type_id, &actor_ref),
        (client_ref_type_id, &client_ref),
    ]);

    assert_eq!(
        network_serialize_type_rust_type(&actor_ref, &serialize_types).as_deref(),
        Some("::nw_network::ActorRef")
    );
    assert_eq!(
        network_serialize_type_rust_type(&client_ref, &serialize_types).as_deref(),
        Some("::nw_network::ClientRef")
    );
}

#[test]
fn reflected_identity_without_struct_fields_uses_the_network_projection() {
    let type_id = uuid!("1be36174-fd4f-4a1c-8e52-7c28d50eec5a");
    let shape = serde_json::from_value::<crate::network_schema::NetworkNestedTypeShape>(json!({
        "typeId": type_id,
        "identityProven": true,
        "typeName": "PersistentItemData",
        "typeNameFull": "PersistentItemData",
        "layoutProven": true,
        "memberCoverageProven": true,
        "wireOrderProven": true,
        "members": [{
            "index": 0,
            "name": "field_0",
            "nameProven": false,
            "wireShape": "u64",
            "wireOrdinal": 0,
            "typeIdentityProven": false,
            "typeConflict": false
        }]
    }))
    .expect("nested type shape");
    let source_type = NetworkSerializeType {
        type_id,
        kind: NetworkSerializeKind::Struct,
        name: "PersistentItemData".to_owned(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: None,
        emits_source: true,
        factory: None,
        field_count: 0,
        fields: Vec::new(),
        variant_count: 0,
        direct_dependency_type_ids: Vec::new(),
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    };
    let source_types = std::collections::BTreeMap::from([(type_id, &source_type)]);

    assert!(!container_value_shape_uses_source_type(
        &shape,
        &source_types
    ));
}
