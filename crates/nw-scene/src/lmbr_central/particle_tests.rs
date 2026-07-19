use bevy_color::LinearRgba;
use nw_objectstream::{Element, types};
use nw_reflected_types::az::rtti::AzRtti;
use nw_reflected_types::types::Component;
use nw_reflected_types::types::components::particle_component::{
    ParticleComponent, ParticleEmitBoneLayer, ParticleEmitterSettings,
};
use uuid::Uuid;

use super::{LmbrCentralObjectStreamError, read_particle_component};

#[test]
fn missing_fields_use_native_new_world_defaults() {
    let source = read_particle_component(&component([])).unwrap();
    let settings = source.component.particle;

    assert!(settings.visible);
    assert!(settings.enable);
    assert_eq!(settings.color, LinearRgba::WHITE);
    assert_eq!(settings.alpha_scale, 1.0);
    assert_eq!(settings.particle_count_scale, 1.0);
    assert_eq!(settings.particle_size_x, 1.0);
    assert_eq!(settings.strength, -1.0);
    assert!(settings.use_lod);
    assert!(settings.use_vis_area);
    assert!(settings.accept_decals);
    assert!(settings.accept_snow);
    assert!(settings.accept_silhouette);
    assert_eq!(settings.target_entity, u64::from(u32::MAX));
    assert_eq!(
        settings.gpu_edge_dissolve_target_entity,
        u64::from(u32::MAX)
    );
    assert!(source.component.load_emitter_on_activate);
    assert!(source.component.particle_library_asset_id.is_nil());
    assert_eq!(source.component_version, Some(3));
    assert_eq!(source.settings_version, None);
}

#[test]
fn reads_current_settings_mesh_layers_and_verbatim_strings() {
    let settings = versioned(
        Element::new(*ParticleEmitterSettings::TYPE_ID.as_inner())
            .with_field("Particle")
            .with_children([
                leaf(
                    "SelectedEmitter",
                    types::AZSTD_STRING,
                    b"  fx/fire.brazier  ",
                ),
                leaf("Audio RTPC", types::AZSTD_STRING, b"  Play_Fire  "),
                leaf("Color", types::COLOR, floats([0.9, 0.4, 0.1, 0.8])),
                entity_id("Target Entity", 0x1234),
                entity_id("GPU Edge Dissolve Target Entity", 0x5678),
            ]),
        7,
    );
    let mesh = Element::new(types::AZSTD_VECTOR)
        .with_field("MeshParticle")
        .with_children([versioned(
            Element::new(*ParticleEmitBoneLayer::TYPE_ID.as_inner()).with_children([
                leaf("Joint name", types::AZSTD_STRING, b"  Spine2  "),
                leaf("Enable Layer", types::BOOL, [1]),
                Element::new(types::AZSTD_VECTOR)
                    .with_field("AffectedIndices")
                    .with_children([
                        Element::new(types::UNSIGNED_INT).with_data(7_u32.to_be_bytes()),
                        Element::new(types::UNSIGNED_INT).with_data(11_u32.to_be_bytes()),
                    ]),
            ]),
            1,
        )]);
    let source = read_particle_component(&component([
        component_base(0x1122_3344_5566_7788),
        settings,
        mesh,
        Element::new(types::ASSET).with_field("ParticleLibraryAssetId"),
        leaf("Load Emitter On Activate", types::BOOL, [0]),
    ]))
    .unwrap();

    assert_eq!(
        source.component.particle.selected_emitter,
        "  fx/fire.brazier  "
    );
    assert_eq!(source.component.az_component.id, 0x1122_3344_5566_7788);
    assert_eq!(source.component.particle.audio_rtpc, "  Play_Fire  ");
    assert_eq!(
        source.component.particle.color,
        LinearRgba::new(0.9, 0.4, 0.1, 0.8)
    );
    assert_eq!(source.component.particle.target_entity, 0x1234);
    assert_eq!(
        source.component.particle.gpu_edge_dissolve_target_entity,
        0x5678
    );
    assert!(!source.component.load_emitter_on_activate);
    assert_eq!(source.component.mesh_particle.len(), 1);
    assert_eq!(source.component.mesh_particle[0].joint_name, "  Spine2  ");
    assert!(source.component.mesh_particle[0].enable_layer);
    assert_eq!(source.component.mesh_particle[0].affected_indices, [7, 11]);
    assert_eq!(source.settings_version, Some(7));
}

#[test]
fn applies_v1_and_v2_rename_and_remove_converter_contract() {
    let v1 = read_particle_component(&component([legacy_settings(1)])).unwrap();
    assert!(v1.component.particle.pre_roll);
    assert_eq!(v1.component.particle.speed_scale, 2.5);
    assert_eq!(v1.component.particle.global_size_scale, 3.0);
    assert_eq!(v1.component.particle.particle_size_x, 4.0);
    assert_eq!(v1.component.particle.particle_size_y, 5.0);
    assert_eq!(v1.component.particle.particle_size_random, 0.25);
    assert_eq!(
        v1.component.particle.color,
        LinearRgba::new(0.1, 0.2, 0.3, 1.0)
    );

    let v2 = read_particle_component(&component([legacy_settings(2)])).unwrap();
    assert_eq!(v2.component.particle.speed_scale, 6.0);
    assert_eq!(v2.settings_version, Some(2));
}

#[test]
fn converts_v4_vector_color_and_keeps_v5_v6_native_defaults() {
    let v4_settings = versioned(
        Element::new(*ParticleEmitterSettings::TYPE_ID.as_inner())
            .with_field("Particle")
            .with_children([leaf("Color", types::VECTOR3, floats([0.2, 0.3, 0.4]))]),
        4,
    );
    let v4 = read_particle_component(&component([v4_settings])).unwrap();
    assert_eq!(
        v4.component.particle.color,
        LinearRgba::new(0.2, 0.3, 0.4, 1.0)
    );

    for version in [5, 6] {
        let settings = versioned(
            Element::new(*ParticleEmitterSettings::TYPE_ID.as_inner()).with_field("Particle"),
            version,
        );
        let source = read_particle_component(&component([settings])).unwrap();
        assert_eq!(source.component.particle.target_entity, u64::from(u32::MAX));
        assert!(source.component.particle.accept_decals);
        assert!(source.component.particle.accept_snow);
        assert!(source.component.particle.accept_silhouette);
    }
}

#[test]
fn rejects_missing_converter_fields_and_unsupported_settings_versions() {
    let mut legacy = legacy_settings(2);
    legacy.elements.retain(|child| {
        child
            .field()
            .is_none_or(|field| field.as_str() != "Geometry")
    });
    assert!(matches!(
        read_particle_component(&component([legacy])).unwrap_err(),
        LmbrCentralObjectStreamError::MissingLegacyField {
            version: 2,
            field: "Geometry",
            ..
        }
    ));

    for version in [0, 8] {
        let settings = versioned(
            Element::new(*ParticleEmitterSettings::TYPE_ID.as_inner()).with_field("Particle"),
            version,
        );
        assert!(matches!(
            read_particle_component(&component([settings])).unwrap_err(),
            LmbrCentralObjectStreamError::UnsupportedVersion { version: actual, .. }
                if actual == version
        ));
    }
}

fn legacy_settings(version: u8) -> Element {
    let attach_type = if version == 1 {
        "Emitter Object Type"
    } else {
        "Attach Type"
    };
    let speed = if version == 1 {
        leaf("Emission Speed", types::FLOAT, 2.5_f32.to_be_bytes())
    } else {
        leaf("Speed Scale", types::FLOAT, 6.0_f32.to_be_bytes())
    };
    let mut children = vec![
        speed,
        leaf("Prime", types::BOOL, [1]),
        leaf("Particle Size Scale", types::FLOAT, 3.0_f32.to_be_bytes()),
        leaf("Size X", types::FLOAT, 4.0_f32.to_be_bytes()),
        leaf("Size Y", types::FLOAT, 5.0_f32.to_be_bytes()),
        leaf("Size Random X", types::FLOAT, 0.25_f32.to_be_bytes()),
        leaf("Color", types::VECTOR3, floats([0.1, 0.2, 0.3])),
    ];
    children.extend([
        removed(attach_type),
        removed("Emitter Shape"),
        removed("Geometry"),
        removed("Count Per Unit"),
        removed("Position Offset"),
        removed("Random Offset"),
        removed("Size Random Y"),
        removed("Init Angles"),
        removed("Rotation Rate X"),
        removed("Rotation Rate Y"),
        removed("Rotation Rate Z"),
        removed("Rotation Rate Random X"),
        removed("Rotation Rate Random Y"),
        removed("Rotation Rate Random Z"),
        removed("Rotation Random Angles"),
    ]);
    versioned(
        Element::new(*ParticleEmitterSettings::TYPE_ID.as_inner())
            .with_field("Particle")
            .with_children(children),
        version,
    )
}

fn component(children: impl Into<Vec<Element>>) -> Element {
    versioned(
        Element::new(*ParticleComponent::TYPE_ID.as_inner()).with_children(children),
        3,
    )
}

fn removed(field: &'static str) -> Element {
    leaf(field, types::BOOL, [0])
}

fn component_base(id: u64) -> Element {
    Element::new(*Component::TYPE_ID.as_inner())
        .with_field("BaseClass1")
        .with_children([leaf("Id", types::AZ_U64, id.to_be_bytes())])
}

fn entity_id(field: &'static str, id: u64) -> Element {
    Element::new(types::ENTITY_ID)
        .with_field(field)
        .with_children([leaf("id", types::AZ_U64, id.to_be_bytes())])
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
