use super::*;

#[test]
fn resolves_native_math_types_to_bevy_and_glam() {
    let expected = [
        ("AZ::Matrix3x3", "::glam::Mat3"),
        ("AZ::Transform", "::glam::Affine3A"),
        ("AZ::Bounds", "::bevy_math::bounding::Aabb2d"),
        ("AZ::Aabb", "::bevy_math::bounding::Aabb3d"),
    ];

    for (native, rust) in expected {
        assert_eq!(
            network_native_scalar_type(native).map(|scalar| scalar.rust_type),
            Some(rust)
        );
    }
}

#[test]
fn emits_conversion_marshaler_for_explicit_message_scalar_types() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 19,
            "typeName": "GridSideMsg",
            "fields": [{
                "index": 0,
                "name": "GridSide",
                "nativeType": "u8",
                "rustType": "::nw_network::source::GridSides",
                "wireShape": "u8",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 1);
    assert!(
        output
            .source
            .contains("pub grid_side: ::nw_network::source::GridSides")
    );
    assert!(
        output.source.contains("codec =")
            && output.source.contains(
                "::nw_network::serialize::ConversionMarshaler<u8, ::nw_network::source::GridSides>"
            )
    );
}

#[test]
fn emits_selected_serialize_enum_message_field_from_source_type_id() {
    let grid_sides_type_id = uuid!("ffe86b09-16b9-429e-9cd2-2901adbe8de3");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 19,
            "typeName": "GridSideMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "GridSide",
                "sourceTypeId": grid_sides_type_id.to_string(),
                "sourceTypeIdSource": "pcode-direct-type-provider",
                "sourceTypeIdentityProven": true,
                "wireShape": "u8",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");
    let unit = SerializeCodegenUnit {
        items: vec![grid_sides_enum_item(grid_sides_type_id)],
    };
    schema.merge_serialize_codegen_unit(&unit, Some("selection.json".to_owned()));

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 1);
    let field = &output.report.message_generation_plans[0].fields[0];
    assert_eq!(field.source_type_id, Some(grid_sides_type_id));
    assert_eq!(field.serialize_type_name.as_deref(), Some("GridSides"));
    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("::nw_network::source::GridSides")
    );
    assert!(
        output
            .source
            .contains("pub grid_side: ::nw_network::source::GridSides")
    );
    assert!(output.source.contains(
        "::nw_network::serialize::ConversionMarshaler<u8, ::nw_network::source::GridSides>"
    ));
}

#[test]
fn leaves_explicit_self_marshaling_scalar_types_unwrapped() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 19,
            "typeName": "RegistrationRequestV3Msg",
            "fields": [{
                "index": 0,
                "name": "TypeIndexCrc",
                "nativeType": "AZ::Crc32",
                "rustType": "::nw_network::TypeIndexCrc",
                "wireShape": "u32",
                "confidence": "message-unmarshal-call"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 1);
    assert!(
        output
            .source
            .contains("pub type_index_crc: ::nw_network::TypeIndexCrc")
    );
    assert!(!output.source.contains("ConversionMarshaler"));
    assert!(!output.source.contains("codec ="));
}

#[test]
fn emits_conversion_marshaler_for_explicit_replicated_state_scalar_types() {
    let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
            "typeIndex": 28,
            "typeName": "Javelin::GridSideReplicatedState",
            "capabilities": ["replicated-state"],
            "fields": [{
                "index": 0,
                "name": "GridSide",
                "group": 0,
                "nativeType": "u8",
                "rustType": "::nw_network::source::GridSides",
                "wireShape": "u8",
                "confidence": "exact"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [28]).expect("state source");

    assert_eq!(output.report.generatable_state_count, 1);
    assert!(
        output
            .source
            .contains("pub grid_side: ::nw_network::serialize::ReplicatedFieldHandler<")
    );
    assert!(output.source.contains("::nw_network::source::GridSides"));
    assert!(
        output
            .source
            .contains("::nw_network::serialize::ConversionMarshaler<")
    );
    assert!(output.source.contains("u8,"));
}

#[test]
fn emits_selected_serialize_enum_replicated_state_field_from_source_type_id() {
    let grid_sides_type_id = uuid!("ffe86b09-16b9-429e-9cd2-2901adbe8de3");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
            "typeIndex": 28,
            "typeName": "Javelin::GridSideReplicatedState",
            "capabilities": ["replicated-state"],
            "fields": [{
                "index": 0,
                "name": "GridSide",
                "group": 0,
                "sourceTypeId": grid_sides_type_id.to_string(),
                "sourceTypeIdSource": "pcode-direct-type-provider",
                "sourceTypeIdentityProven": true,
                "wireShape": "u8",
                "confidence": "exact"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");
    let unit = SerializeCodegenUnit {
        items: vec![grid_sides_enum_item(grid_sides_type_id)],
    };
    schema.merge_serialize_codegen_unit(&unit, Some("selection.json".to_owned()));

    let output = NetworkRustEmitter::emit_replicated_states(&schema, [28]).expect("state source");

    assert_eq!(output.report.generatable_state_count, 1);
    let field = &output.report.state_generation_plans[0].fields[0];
    assert_eq!(field.source_type_id, Some(grid_sides_type_id));
    assert_eq!(field.serialize_type_name.as_deref(), Some("GridSides"));
    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("::nw_network::source::GridSides")
    );
    assert!(output.source.contains("ReplicatedFieldHandler<"));
    assert!(output.source.contains("::nw_network::source::GridSides"));
    assert!(output.source.contains("ConversionMarshaler<"));
    assert!(output.source.contains("u8,"));
}

#[test]
fn emits_selected_serialize_struct_message_field_from_source_type_id() {
    let payload_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
    let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
        "registryEntries": [{
            "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
            "typeIndex": 19,
            "typeName": "PayloadMsg",
            "capabilities": ["direct-message"],
            "fields": [{
                "index": 0,
                "name": "Payload",
                "nativeType": "PayloadData",
                "sourceTypeName": "PayloadData",
                "sourceTypeId": payload_type_id.to_string(),
                "sourceTypeIdSource": "pcode-direct-type-provider",
                "sourceTypeIdentityProven": true,
                "confidence": "message-unmarshal-direct-type"
            }]
        }],
        "fieldRegistrationFunctions": []
    }))
    .expect("schema");
    let unit = SerializeCodegenUnit {
        items: vec![SerializeCodegenItem {
            source_type_id: payload_type_id,
            source_name: "PayloadData".to_owned(),
            role: crate::role::ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        }],
    };
    schema.merge_serialize_codegen_unit(&unit, Some("selection.json".to_owned()));

    let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

    assert_eq!(output.report.generatable_message_count, 1);
    let field = &output.report.message_generation_plans[0].fields[0];
    assert_eq!(field.source_type_id, Some(payload_type_id));
    assert_eq!(field.serialize_type_name.as_deref(), Some("PayloadData"));
    assert_eq!(
        field.rust_value_type.as_deref(),
        Some("::nw_network::source::PayloadData")
    );
    assert_eq!(field.blocked_reason, None);
    assert_eq!(
        field.rust_field_type.as_deref(),
        Some("::nw_network::source::PayloadData")
    );
    assert!(output.source.contains("pub struct PayloadMsg"));
    assert!(
        output
            .source
            .contains("pub payload: ::nw_network::source::PayloadData")
    );
}

#[test]
fn emits_marshaler_conversions_for_compact_generated_enums() {
    let item = SerializeCodegenItem {
        source_type_id: Uuid::from_u128(0xffe86b0916b9429e9cd22901adbe8de3),
        source_name: "GridSides".to_owned(),
        role: crate::role::ReflectedTypeRole::SupportType,
        is_reflection_marker: false,
        is_abstract: None,
        factory: None,
        rtti_base_chain: Vec::new(),
        kind: SerializeCodegenItemKind::Enum,
        enum_underlying_type: Some(ResolvedType::Scalar(ScalarType::I32)),
        fields: Vec::new(),
        variants: vec![
            SerializeCodegenVariant {
                source_name: "InvalidSide".to_owned(),
                value_u64: Some(0),
                value_u32: Some(0),
                value_i32: Some(0),
            },
            SerializeCodegenVariant {
                source_name: "Left".to_owned(),
                value_u64: Some(4),
                value_u32: Some(4),
                value_i32: Some(4),
            },
        ],
    };

    let output =
        NetworkRustEmitter::emit_marshaler_conversions([&item]).expect("conversion source");

    assert_eq!(output.report.marshaler_conversion_count, 3);
    assert!(
        output
            .source
            .contains("impl ::nw_network::serialize::MarshalerConversion<u8>")
    );
    assert!(
        output
            .source
            .contains("for ::nw_network::source::GridSides")
    );
    assert!(output.source.contains("let raw = i32::from(self);"));
    assert!(output.source.contains("min: 0u64"));
    assert!(output.source.contains("max: 4u64"));
}

#[test]
fn emits_struct_marshaler_for_signed_enum_fields() {
    let enum_type_id = uuid!("99ffbb9b-34a3-44a1-a576-1d13d732b0aa");
    let enum_item = SerializeCodegenItem {
        source_type_id: enum_type_id,
        source_name: "SettlementProgressionCategory".to_owned(),
        role: crate::role::ReflectedTypeRole::SupportType,
        is_reflection_marker: false,
        is_abstract: None,
        factory: None,
        rtti_base_chain: Vec::new(),
        kind: SerializeCodegenItemKind::Enum,
        enum_underlying_type: Some(ResolvedType::Scalar(ScalarType::I32)),
        fields: Vec::new(),
        variants: vec![
            SerializeCodegenVariant {
                source_name: "None".to_owned(),
                value_u64: None,
                value_u32: None,
                value_i32: Some(-1),
            },
            SerializeCodegenVariant {
                source_name: "Blacksmithing".to_owned(),
                value_u64: Some(0),
                value_u32: Some(0),
                value_i32: Some(0),
            },
        ],
    };
    let struct_item = SerializeCodegenItem {
        source_type_id: uuid!("27362f56-9317-40ce-8caa-69d5d8f75450"),
        source_name: "TerritoryUpgradeData".to_owned(),
        role: crate::role::ReflectedTypeRole::SupportType,
        is_reflection_marker: false,
        is_abstract: Some(false),
        factory: None,
        rtti_base_chain: Vec::new(),
        kind: SerializeCodegenItemKind::Struct,
        enum_underlying_type: None,
        fields: vec![
            SerializeCodegenField {
                source_name: "m_category".to_owned(),
                source_type_id: enum_type_id,
                resolved_type: ResolvedType::Named {
                    type_id: enum_type_id,
                    source_name: "SettlementProgressionCategory".to_owned(),
                },
                data_size: None,
                offset: None,
                flags: None,
                is_base_class: false,
                is_pointer: false,
                is_dynamic_field: false,
            },
            SerializeCodegenField {
                source_name: "m_level".to_owned(),
                source_type_id: Uuid::nil(),
                resolved_type: ResolvedType::Scalar(ScalarType::U8),
                data_size: None,
                offset: None,
                flags: None,
                is_base_class: false,
                is_pointer: false,
                is_dynamic_field: false,
            },
        ],
        variants: Vec::new(),
    };

    let output = NetworkRustEmitter::emit_marshaler_conversions([&enum_item, &struct_item])
        .expect("conversion source");

    assert!(output.source.contains(
        "impl ::nw_network::serialize::Marshal for ::nw_network::source::TerritoryUpgradeData"
    ));
    assert!(output.source.contains(
        "impl ::nw_network::serialize::Unmarshal for ::nw_network::source::TerritoryUpgradeData"
    ));
    assert!(
        output
            .source
            .contains("let raw = i32::from(self.category);")
    );
    assert!(output.source.contains("min: 0u64"));
    assert!(output.source.contains("max: 0u64"));
}

fn grid_sides_enum_item(type_id: Uuid) -> SerializeCodegenItem {
    SerializeCodegenItem {
        source_type_id: type_id,
        source_name: "GridSides".to_owned(),
        role: crate::role::ReflectedTypeRole::SupportType,
        is_reflection_marker: false,
        is_abstract: Some(false),
        factory: None,
        rtti_base_chain: Vec::new(),
        kind: SerializeCodegenItemKind::Enum,
        enum_underlying_type: Some(ResolvedType::Scalar(ScalarType::I32)),
        fields: Vec::new(),
        variants: vec![
            SerializeCodegenVariant {
                source_name: "InvalidSide".to_owned(),
                value_u64: Some(0),
                value_u32: Some(0),
                value_i32: Some(0),
            },
            SerializeCodegenVariant {
                source_name: "Left".to_owned(),
                value_u64: Some(4),
                value_u32: Some(4),
                value_i32: Some(4),
            },
        ],
    }
}
