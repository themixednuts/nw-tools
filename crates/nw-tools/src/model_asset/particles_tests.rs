use std::collections::HashMap;

use glam::{Mat4, Quat, Vec3};

use std::collections::BTreeMap;

use super::*;

const LIBRARY_GUID: uuid::Uuid = uuid::uuid!("1E1D1F12-486E-50A5-BD4E-4B1E20076939");

#[test]
fn relative_transform_accumulates_parent_chain() {
    let mut entities = BTreeMap::new();
    entities.insert(1, scene_entity(0, Vec3::ZERO, Vec::new()));
    entities.insert(2, scene_entity(1, Vec3::new(1.0, 0.0, 0.0), Vec::new()));
    entities.insert(3, scene_entity(2, Vec3::new(0.0, 2.0, 0.0), Vec::new()));

    let transform = relative_entity_transform(3, 1, &entities).unwrap();
    assert_eq!(transform.translation, [1.0, 2.0, 0.0]);
}

#[test]
fn model_owner_requires_exact_character_definition_association() {
    let mut entities = BTreeMap::new();
    entities.insert(
        1,
        scene_entity(
            0,
            Vec3::ZERO,
            vec!["objects/characters/isabella.cdf".to_owned()],
        ),
    );
    entities.insert(2, scene_entity(1, Vec3::ZERO, Vec::new()));
    entities.insert(
        3,
        scene_entity(0, Vec3::ZERO, vec!["objects/props/relic.cdf".to_owned()]),
    );

    assert_eq!(
        model_owner_entity(2, "objects/characters/isabella.cdf", &entities),
        Some(1)
    );
    assert_eq!(
        model_owner_entity(3, "objects/characters/isabella.cdf", &entities),
        None
    );
    assert_eq!(
        model_owner_entity(0, "objects/characters/isabella.cdf", &entities),
        None
    );
}

#[test]
fn settings_target_ids_remain_distinct_authored_fields() {
    let mut value = serde_json::json!({
        "Particle": {
            "SelectedEmitter": "cFX.Test",
            "Target Entity": 30,
            "GPU Edge Dissolve Target Entity": 38
        },
        "Load Emitter On Activate": true
    });
    stringify_particle_entity_ids(&mut value);

    assert_eq!(value["Particle"]["Target Entity"], "30");
    assert_eq!(value["Particle"]["GPU Edge Dissolve Target Entity"], "38");
}

#[test]
fn canonical_fingerprint_changes_for_unprojected_settings() {
    let entities = BTreeMap::new();
    let first = serde_json::json!({
        "Particle": { "SelectedEmitter": "cFX.Test", "Time Scale": 1.0 },
        "MeshParticle": [],
        "Load Emitter On Activate": true,
    });
    let second = serde_json::json!({
        "Particle": { "SelectedEmitter": "cFX.Test", "Time Scale": 2.0 },
        "MeshParticle": [],
        "Load Emitter On Activate": true,
    });

    assert_ne!(
        particle_settings_fingerprint(&first, None, &entities).unwrap(),
        particle_settings_fingerprint(&second, None, &entities).unwrap()
    );
}

#[test]
fn canonical_fingerprint_ignores_instance_id_for_resolved_bone_owner() {
    let component = serde_json::json!({
        "Particle": {
            "SelectedEmitter": "cFX.Test",
            "Target Entity": "4294967295",
            "GPU Edge Dissolve Target Entity": "4294967295"
        },
        "MeshParticle": [],
        "Load Emitter On Activate": true,
    });
    let first = serde_json::json!({
        "Target ID": "100",
        "Target Bone Name": "bind_neck_a",
    });
    let second = serde_json::json!({
        "Target ID": "200",
        "Target Bone Name": "bind_neck_a",
    });

    assert_eq!(
        particle_settings_fingerprint(&component, Some(&first), &BTreeMap::new()).unwrap(),
        particle_settings_fingerprint(&component, Some(&second), &BTreeMap::new()).unwrap()
    );
}

#[test]
fn unattached_initial_state_stays_in_particle_entity_frame() {
    let entities = placement_entities();
    let attachment = attachment(2, "missing_bone", false, Vec3::new(2.0, 0.0, 0.0));
    let entity_transform = relative_entity_transform(3, 1, &entities);

    let (placement, issue, binding) = resolve_particle_placement(
        &nw_model::Model::default(),
        1,
        entity_transform,
        Some(&attachment),
        &entities,
    );

    assert_eq!(binding, ParticleBinding::Bound);
    assert_eq!(issue, None);
    assert!(matches!(
        placement,
        nw_model::CryParticlePlacement::Entity { transform }
            if transform.translation == [1.0, 0.0, 0.0]
    ));
}

#[test]
fn boneless_attachment_composes_target_frame_and_offset() {
    let entities = placement_entities();
    let attachment = attachment(2, "", true, Vec3::new(2.0, 0.0, 0.0));

    let (placement, issue, binding) = resolve_particle_placement(
        &nw_model::Model::default(),
        1,
        relative_entity_transform(3, 1, &entities),
        Some(&attachment),
        &entities,
    );

    assert_eq!(binding, ParticleBinding::Bound);
    assert_eq!(issue, None);
    assert!(matches!(
        placement,
        nw_model::CryParticlePlacement::TargetEntity {
            target_entity_id,
            transform,
        } if target_entity_id.get() == 2 && transform.translation == [12.0, 0.0, 0.0]
    ));
}

#[test]
fn bone_attachment_resolves_non_primary_skeleton() {
    let entities = placement_entities();
    let attachment = attachment(2, "wing", true, Vec3::new(2.0, 0.0, 0.0));
    let skeleton = |name: &str| nw_model::Skeleton {
        bones: vec![nw_model::Bone {
            name: name.to_owned(),
            controller_id: 1,
            parent: None,
            local: Mat4::IDENTITY,
            inverse_bind: Mat4::IDENTITY,
        }],
        placement: None,
    };
    let model = nw_model::Model {
        skeletons: vec![skeleton("root"), skeleton("wing")],
        ..Default::default()
    };

    let (placement, issue, binding) = resolve_particle_placement(
        &model,
        1,
        relative_entity_transform(3, 1, &entities),
        Some(&attachment),
        &entities,
    );

    assert_eq!(binding, ParticleBinding::Bound);
    assert_eq!(issue, None);
    assert!(matches!(
        placement,
        nw_model::CryParticlePlacement::Bone {
            skeleton_index: 1,
            ref bone_name,
            ..
        } if bone_name == "wing"
    ));
}

#[test]
fn unresolved_attachment_target_falls_back_with_diagnostic() {
    let entities = placement_entities();
    let attachment = attachment(99, "", true, Vec3::ZERO);

    let (placement, issue, binding) = resolve_particle_placement(
        &nw_model::Model::default(),
        1,
        relative_entity_transform(3, 1, &entities),
        Some(&attachment),
        &entities,
    );

    assert_eq!(binding, ParticleBinding::Bound);
    assert_eq!(
        issue,
        Some(nw_model::CryParticlePlacementIssue::UnresolvedAttachmentTarget)
    );
    assert!(matches!(
        placement,
        nw_model::CryParticlePlacement::Entity { transform }
            if transform.translation == [1.0, 0.0, 0.0]
    ));
}

#[test]
fn unresolved_non_nil_library_is_an_error() {
    let source = TestSource::default();
    let mut extras = nw_model::CryAssetExtras::default();
    let mut emitters = vec![scene_emitter(
        "slices/characters/isabella.dynamicslice",
        "fingerprint",
        ParticleBinding::Bound,
    )];

    let error = resolve_particle_libraries(
        &nw_jobs::JobRunner::inline(),
        &source,
        &mut extras,
        &mut emitters,
        &nw_asset_graph::AssetDependencyGraph::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("resolve particle library"));
}

#[test]
fn selected_effect_packages_only_its_runtime_resources() {
    let library_path = "libs/particles/cfx_npc_isabella_phase2.xml";
    let source = TestSource {
        bytes: HashMap::from([
            (
                library_path.to_owned(),
                br#"<ParticleLibrary Name="cFX_npc_Isabella_Phase2"><Particles Name="Wing_Idle01"><Params Texture="textures/vfx/wing.tif" Material="materials/vfx/wing" Geometry="objects/vfx/wing.cgf"/></Particles><Particles Name="Unused"><Params Texture="textures/vfx/missing.dds"/></Particles></ParticleLibrary>"#.to_vec(),
            ),
            ("textures/vfx/wing.dds".to_owned(), b"dds".to_vec()),
            (
                "textures/vfx/wing.dds.1".to_owned(),
                b"streaming-mip".to_vec(),
            ),
            ("materials/vfx/wing.mtl".to_owned(), b"material".to_vec()),
            ("objects/vfx/wing.cgf".to_owned(), b"geometry".to_vec()),
        ]),
        ids: HashMap::from([(
            nw_asset::AssetId::new(LIBRARY_GUID, 0),
            library_path.to_owned(),
        )]),
    };
    let mut extras = nw_model::CryAssetExtras::default();
    let mut emitters = vec![scene_emitter(
        "slices/characters/isabella.dynamicslice",
        "fingerprint",
        ParticleBinding::Bound,
    )];

    resolve_particle_libraries(
        &nw_jobs::JobRunner::with_workers(2).unwrap(),
        &source,
        &mut extras,
        &mut emitters,
        &nw_asset_graph::AssetDependencyGraph::default(),
    )
    .unwrap();

    assert_eq!(
        emitters[0].emitter.particle_library_path.as_deref(),
        Some(library_path)
    );
    for (path, kind) in [
        (
            "textures/vfx/wing.dds",
            nw_model::CryEmbeddedResourceKind::ParticleTexture,
        ),
        (
            "textures/vfx/wing.dds.1",
            nw_model::CryEmbeddedResourceKind::ParticleTextureSidecar,
        ),
        (
            "materials/vfx/wing.mtl",
            nw_model::CryEmbeddedResourceKind::ParticleMaterial,
        ),
        (
            "objects/vfx/wing.cgf",
            nw_model::CryEmbeddedResourceKind::ParticleGeometry,
        ),
    ] {
        assert!(extras.resource_payloads.iter().any(|resource| {
            resource.kind == kind && resource.source_path.eq_ignore_ascii_case(path)
        }));
    }
    assert!(
        !extras
            .resource_payloads
            .iter()
            .any(|resource| resource.source_path.contains("missing"))
    );
}

#[test]
fn different_fingerprints_do_not_collapse() {
    let path = "slices/characters/isabella.dynamicslice";
    let mut emitters = vec![
        scene_emitter(path, "first", ParticleBinding::Bound),
        scene_emitter(path, "second", ParticleBinding::Bound),
    ];

    collapse_scene_particle_variants(&mut emitters);

    assert_eq!(emitters.len(), 2);
}

fn scene_entity(
    parent_id: u64,
    translation: Vec3,
    character_definitions: Vec<String>,
) -> SceneEntity {
    let transform = nw_model::CryParticleTransform {
        translation: translation.to_array(),
        rotation: Quat::IDENTITY.to_array(),
        scale: Vec3::ONE.to_array(),
    };
    SceneEntity {
        name: None,
        parent_id,
        local_transform: transform,
        character_definitions,
    }
}

fn placement_entities() -> BTreeMap<u64, SceneEntity> {
    BTreeMap::from([
        (
            1,
            scene_entity(
                0,
                Vec3::ZERO,
                vec!["objects/characters/test.cdf".to_owned()],
            ),
        ),
        (2, scene_entity(1, Vec3::new(10.0, 0.0, 0.0), Vec::new())),
        (3, scene_entity(1, Vec3::new(1.0, 0.0, 0.0), Vec::new())),
    ])
}

fn attachment(
    target_id: u64,
    target_bone_name: &str,
    attached_initially: bool,
    offset: Vec3,
) -> DecodedAttachment {
    let mut configuration = nw_reflected_types::types::AttachmentConfiguration {
        target_id,
        target_bone_name: target_bone_name.to_owned(),
        attached_initially,
        ..Default::default()
    };
    configuration.target_offset.translation = offset;
    DecodedAttachment {
        configuration,
        component_version: Some(1),
        configuration_version: Some(1),
        component_source: serde_json::Value::Null,
        source: serde_json::Value::Null,
    }
}

fn scene_emitter(path: &str, fingerprint: &str, binding: ParticleBinding) -> SceneParticleEmitter {
    let asset_id = nw_asset::AssetId::new(LIBRARY_GUID, 0);
    SceneParticleEmitter {
        emitter: nw_model::CryParticleEmitter {
            selected_emitter: "cFX_npc_Isabella_Phase2.Wing_Idle01".to_owned(),
            particle_library_asset_id: Some(asset_id.to_string()),
            particle_library_path: None,
            visible: true,
            enabled: true,
            attach_to_mesh: false,
            load_emitter_on_activate: true,
            color: [1.0; 4],
            particle_target_entity_id: u64::from(u32::MAX).into(),
            gpu_edge_dissolve_target_entity_id: u64::from(u32::MAX).into(),
            entity_transform: Default::default(),
            entity_parent_id: 1.into(),
            placement: nw_model::CryParticlePlacement::Bone {
                target_entity_id: 1.into(),
                skeleton_index: 0,
                bone_name: "bind_right_wingC_05".to_owned(),
                transform: Default::default(),
            },
            placement_issue: None,
            attachment: Some(nw_model::CryParticleAttachment {
                target_entity_id: 1.into(),
                target_bone_name: "bind_right_wingC_05".to_owned(),
                target_offset: Default::default(),
                attached_initially: true,
                scale_source: 0,
                update_tolerance: 0.0,
            }),
            authored_payload: nw_model::CryParticleAuthoredPayload::default(),
            authored_settings_fingerprint: fingerprint.to_owned(),
            context: nw_model::CryParticleEmitterContext {
                source_path: path.to_owned(),
                entity_id: Some(7.into()),
                entity_name: Some("VFX_Wing_Right03".to_owned()),
                ..Default::default()
            },
        },
        particle_library_asset_id: Some(asset_id),
        binding,
    }
}

#[derive(Default)]
struct TestSource {
    bytes: HashMap<String, Vec<u8>>,
    ids: HashMap<nw_asset::AssetId, String>,
}

impl nw_asset_graph::AssetSource for TestSource {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.bytes.get(path).cloned()
    }

    fn contains(&self, path: &str) -> bool {
        self.bytes.contains_key(path)
    }

    fn matching_paths(&self, pattern: &str) -> Result<Vec<String>> {
        let Some(prefix) = pattern.strip_suffix('*') else {
            return Ok(Vec::new());
        };
        Ok(self
            .bytes
            .keys()
            .filter(|path| path.starts_with(prefix))
            .cloned()
            .collect())
    }

    fn path_by_id(&self, asset_id: nw_asset::AssetId) -> Option<String> {
        self.ids.get(&asset_id).cloned()
    }
}

impl AssetSource for TestSource {
    fn materials(&self, _cgf: &[u8], _mesh: &MeshRef) -> Option<nw_model::MaterialSet> {
        None
    }
}
