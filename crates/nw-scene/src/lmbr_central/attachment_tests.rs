use bevy_math::Vec3;
use nw_objectstream::{Element, types};
use nw_reflected_types::az::rtti::AzRtti;
use nw_reflected_types::types::Component;
use nw_reflected_types::types::components::attachment_component::{
    AttachmentComponent, AttachmentConfiguration,
};
use uuid::Uuid;

use super::{LmbrCentralObjectStreamError, read_attachment_component};

#[test]
fn missing_configuration_uses_native_attachment_defaults() {
    let source = read_attachment_component(&component()).unwrap();
    let configuration = source.component.configuration;

    assert_eq!(configuration.target_id, u64::from(u32::MAX));
    assert!(configuration.attached_initially);
    assert_eq!(configuration.target_offset.translation, Vec3::ZERO);
    assert_eq!(configuration.target_offset.scale, Vec3::ONE);
    assert_eq!(source.component_version, Some(1));
    assert_eq!(source.configuration_version, None);
}

#[test]
fn reads_configuration_without_trimming_authored_bone_name() {
    let configuration = versioned(
        Element::new(*AttachmentConfiguration::TYPE_ID.as_inner())
            .with_field("Configuration")
            .with_children([
                entity_id("Target ID", 0x1234),
                leaf("Target Bone Name", types::AZSTD_STRING, b"  Head  "),
                leaf(
                    "Target Offset",
                    types::TRANSFORM,
                    floats([1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 4.0, 5.0, 6.0]),
                ),
                leaf("Attached Initially", types::BOOL, [0]),
                leaf("Scale Source", types::UNSIGNED_CHAR, [2]),
                leaf("Update Tolerance", types::FLOAT, 0.25_f32.to_be_bytes()),
            ]),
        1,
    );
    let source = read_attachment_component(
        &component().with_children([component_base(0x1122_3344_5566_7788), configuration]),
    )
    .unwrap();
    let configuration = source.component.configuration;

    assert_eq!(configuration.target_id, 0x1234);
    assert_eq!(configuration.target_bone_name, "  Head  ");
    assert_eq!(
        configuration.target_offset.translation,
        Vec3::new(4.0, 5.0, 6.0)
    );
    assert_eq!(configuration.target_offset.scale, Vec3::new(1.0, 2.0, 3.0));
    assert!(!configuration.attached_initially);
    assert_eq!(configuration.scale_source, 2);
    assert_eq!(configuration.update_tolerance, 0.25);
    assert_eq!(source.component.az_component.id, 0x1122_3344_5566_7788);
    assert_eq!(source.configuration_version, Some(1));
}

#[test]
fn rejects_unknown_scale_source_and_future_versions() {
    let bad_scale = Element::new(*AttachmentConfiguration::TYPE_ID.as_inner())
        .with_field("Configuration")
        .with_children([leaf("Scale Source", types::UNSIGNED_CHAR, [3])]);
    assert!(matches!(
        read_attachment_component(&component().with_children([bad_scale])).unwrap_err(),
        LmbrCentralObjectStreamError::InvalidEnum {
            field: "Scale Source",
            value: 3,
            ..
        }
    ));

    let future = versioned(
        Element::new(*AttachmentConfiguration::TYPE_ID.as_inner()).with_field("Configuration"),
        2,
    );
    assert!(matches!(
        read_attachment_component(&component().with_children([future])).unwrap_err(),
        LmbrCentralObjectStreamError::UnsupportedVersion { version: 2, .. }
    ));
}

fn component() -> Element {
    versioned(Element::new(*AttachmentComponent::TYPE_ID.as_inner()), 1)
}

fn entity_id(field: &'static str, id: u64) -> Element {
    Element::new(types::ENTITY_ID)
        .with_field(field)
        .with_children([leaf("id", types::AZ_U64, id.to_be_bytes())])
}

fn component_base(id: u64) -> Element {
    Element::new(*Component::TYPE_ID.as_inner())
        .with_field("BaseClass1")
        .with_children([leaf("Id", types::AZ_U64, id.to_be_bytes())])
}

fn versioned(mut element: Element, version: u8) -> Element {
    element.version = Some(version);
    element
}

fn leaf(field: &'static str, id: Uuid, data: impl Into<Vec<u8>>) -> Element {
    Element::new(id).with_field(field).with_data(data)
}

fn floats<const N: usize>(values: [f32; N]) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_be_bytes).collect()
}
