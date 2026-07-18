//! Scene physics: scoping character-owned collision, and decoding ObjectStream
//! hit volumes / rigid bodies from consumer slices.
//!
//! Split out of `model_asset` as a pure move; shared helpers stay in the parent.

use super::*;

const ENTITY_ID_FIELD_CRC: u32 = 0xbf39_6750;
const ENTITY_NAME_FIELD_CRC: u32 = 0x5e23_7e06;
const ENTITY_COMPONENTS_FIELD_CRC: u32 = 0xee48_f5fd;

/// Restrict scene-derived physics to the character's own slice context.
///
/// Consumer slices that merely place this model as a prop (loot containers, POI
/// decorations) contribute their own hit volumes and rigid bodies through the
/// same ObjectStream scan (`parse_scene_physics`). Those collision shapes are
/// not the creature's physics and must not be glued onto the exported character.
///
/// Ownership is the set of slices whose hit volumes target this model's
/// skeleton. When nothing targets the skeleton (a static-prop export), the
/// single primary consumer slice is kept instead. Physics from every other
/// context is dropped entirely — those slices are no longer part of the model's
/// story, so they neither become scene nodes nor linger in `extras.physics`. The
/// character's own authored collision (`shape_assets` from the CDF Physics
/// field) is untouched.
pub(super) fn scope_scene_physics(model: &nw_model::Model, physics: &mut nw_model::PhysicsScene) {
    if physics.hit_volumes.is_empty() && physics.rigid_bodies.is_empty() {
        return;
    }
    let skeleton = model.primary_skeleton();
    let mut owning: HashSet<String> = physics
        .hit_volumes
        .iter()
        .filter(|volume| {
            !volume.target_bone_name.is_empty()
                && skeleton
                    .is_some_and(|skeleton| skeleton.bone_index(&volume.target_bone_name).is_some())
        })
        .map(|volume| volume.context.source_path.to_ascii_lowercase())
        .collect();
    if owning.is_empty() {
        owning.extend(primary_physics_source(physics));
    }
    physics
        .hit_volumes
        .retain(|volume| owning.contains(&volume.context.source_path.to_ascii_lowercase()));
    physics
        .rigid_bodies
        .retain(|body| owning.contains(&body.context.source_path.to_ascii_lowercase()));
}

/// The single primary source slice for a static-prop physics export: the slice
/// contributing the most physics components, ties broken by the
/// lexicographically-first normalized path.
fn primary_physics_source(physics: &nw_model::PhysicsScene) -> Option<String> {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let sources = physics
        .hit_volumes
        .iter()
        .map(|volume| &volume.context.source_path)
        .chain(
            physics
                .rigid_bodies
                .iter()
                .map(|body| &body.context.source_path),
        );
    for source in sources {
        *counts.entry(source.to_ascii_lowercase()).or_default() += 1;
    }
    // BTreeMap iterates ascending by key, so `reduce` retains the earlier
    // (lexicographically-first) path on a tie.
    counts
        .into_iter()
        .reduce(|best, current| if current.1 > best.1 { current } else { best })
        .map(|(path, _)| path)
}

pub(super) fn parse_scene_physics(path: &str, bytes: &[u8]) -> Result<nw_model::PhysicsScene> {
    let stream = nw_objectstream::ObjectStream::from_bytes(bytes, Some(&OBJECTSTREAM_LOOKUP))
        .with_context(|| format!("parse ObjectStream {path}"))?;
    let codec = nw_objectstream::schema::SchemaValueCodec::new(Some(&OBJECTSTREAM_LOOKUP));
    let mut physics = nw_model::PhysicsScene::default();

    for entity in nw_objectstream::query::az_entity_elements_skipping_fields(
        stream.elements(),
        &["m_embeddedShapeEntity"],
    ) {
        let context = scene_entity_context(path, entity)?;
        let Some(components) = nw_objectstream::query::child_by_field_ignore_case_or_crc(
            entity,
            "Components",
            ENTITY_COMPONENTS_FIELD_CRC,
        ) else {
            continue;
        };

        let mut transform = None;
        let mut hit_volumes = Vec::new();
        let mut rigid_bodies = Vec::new();
        for component in components.children() {
            if component.id() == nw_reflected_types::types::TransformComponent::TYPE_ID.as_inner() {
                let fields = codec.field_map(component)?;
                let value: nw_reflected_types::types::TransformComponent =
                    serde_json::from_value(fields.to_serde_value()?)
                        .context("deserialize generated TransformComponent")?;
                transform = Some(nw_model::PhysicsTransform {
                    translation: Vec3::from_array(value.transform.translation.to_array()),
                    rotation: Quat::from_array(value.transform.rotation.to_array()),
                    scale: Vec3::from_array(value.transform.scale.to_array()),
                });
            } else if component.id()
                == nw_reflected_types::types::GameTransformComponent::TYPE_ID.as_inner()
                && transform.is_none()
            {
                let fields = codec.field_map(component)?;
                let value: nw_reflected_types::types::GameTransformComponent =
                    serde_json::from_value(fields.to_serde_value()?)
                        .context("deserialize generated GameTransformComponent")?;
                transform = Some(nw_model::PhysicsTransform {
                    translation: Vec3::from_array(value.world_tm.translation.to_array()),
                    rotation: Quat::from_array(value.world_tm.rotation.to_array()),
                    scale: Vec3::from_array(value.world_tm.scale.to_array()),
                });
            } else if component.id()
                == nw_reflected_types::types::HitVolumeComponent::TYPE_ID.as_inner()
            {
                let fields = codec.field_map(component)?;
                let value: nw_reflected_types::types::HitVolumeComponent =
                    serde_json::from_value(fields.to_serde_value()?)
                        .context("deserialize generated HitVolumeComponent")?;
                hit_volumes.push(nw_model::HitVolume::try_from(&value)?);
            } else if component.id()
                == nw_reflected_types::types::GameRigidBodyComponent::TYPE_ID.as_inner()
            {
                let fields = codec.field_map(component)?;
                let value = decode_game_rigid_body(component, &fields)?;
                rigid_bodies.push(nw_model::RigidBody::try_from(&value)?);
            }
        }

        let mut context = context;
        context.transform = transform.unwrap_or_default();
        for mut volume in hit_volumes {
            volume.context = context.clone();
            physics.hit_volumes.push(volume);
        }
        for mut body in rigid_bodies {
            body.context = context.clone();
            body.name = context.entity_name.clone().unwrap_or_else(|| {
                context.entity_id.map_or_else(
                    || "game_rigid_body".to_owned(),
                    |id| format!("game_rigid_body_{id:016x}"),
                )
            });
            physics.rigid_bodies.push(body);
        }
    }

    physics.validate()?;
    Ok(physics)
}

fn scene_entity_context(
    path: &str,
    entity: &nw_objectstream::Element,
) -> Result<nw_model::PhysicsComponentContext> {
    let entity_id = nw_objectstream::query::child_by_field_ignore_case_or_crc(
        entity,
        "id",
        ENTITY_ID_FIELD_CRC,
    )
    .map(nw_objectstream::value::read_entity_id)
    .transpose()
    .context("decode AZ::Entity id")?;
    let entity_name = nw_objectstream::query::child_by_field_ignore_case_or_crc(
        entity,
        "name",
        ENTITY_NAME_FIELD_CRC,
    )
    .map(nw_objectstream::value::read_trimmed_string_owned)
    .transpose()
    .context("decode AZ::Entity name")?
    .flatten();
    Ok(nw_model::PhysicsComponentContext {
        source_path: normalize_path(path),
        entity_id,
        entity_name,
        ..Default::default()
    })
}

fn decode_game_rigid_body(
    component: &nw_objectstream::Element,
    fields: &nw_objectstream::schema::SchemaFieldMap,
) -> Result<nw_reflected_types::types::GameRigidBodyComponent> {
    let configuration_count = fields.field_count("m_configuration");
    if configuration_count > 1 {
        bail!("GameRigidBodyComponent contains {configuration_count} m_configuration fields");
    }
    let has_configuration = configuration_count == 1;
    let version = component.version().unwrap_or_default();
    match (version, has_configuration) {
        (0, false) | (1, true) => {}
        (0, true) => {
            bail!("GameRigidBodyComponent version 0 unexpectedly contains m_configuration")
        }
        (1, false) => bail!("GameRigidBodyComponent version 1 requires m_configuration"),
        _ => bail!("unsupported GameRigidBodyComponent version {version}"),
    }

    let mut value: nw_reflected_types::types::GameRigidBodyComponent =
        serde_json::from_value(fields.to_serde_value()?)
            .context("deserialize generated GameRigidBodyComponent")?;
    if version == 0 {
        value.configuration = migrate_game_rigid_body_v0(&value);
    }
    Ok(value)
}

fn migrate_game_rigid_body_v0(
    component: &nw_reflected_types::types::GameRigidBodyComponent,
) -> nw_reflected_types::types::GameRigidBodyConfig {
    use nw_reflected_types::types::GameRigidBodyConfig;

    GameRigidBodyConfig {
        center: component.center,
        collision_type: component.collision_type,
        rnr_asset: component.rnr_asset.clone(),
        material_override_asset: component.material_override_asset.clone(),
        collision_shape: component.collision_shape.clone(),
        override_collision_shape_material: false,
        override_collision_shape_material_name: String::new(),
        shape_entity: component.shape_entity,
        is_dynamic: component.is_dynamic,
        mass: component.mass,
        linear_damping: component.linear_damping,
        angular_damping: component.angular_damping,
        sleep_min_energy: component.sleep_min_energy,
        interact_with_triggers: component.interact_with_triggers,
        str_filter: component.str_filter.clone(),
        gameplay_flags: component.gameplay_flags.clone(),
        scale_shapes: component.scale_shapes,
        apply_alignment_details: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_physics_keeps_only_the_character_owning_slice() {
        use nw_model::{HitVolume, PhysicsComponentContext, QueryShape, RigidBody};

        // A character with one skeleton bone the hit volume targets.
        let model = nw_model::Model {
            meshes: Vec::new(),
            skeletons: vec![nw_model::Skeleton {
                bones: vec![nw_model::Bone {
                    name: "bind_mouth_web".to_owned(),
                    controller_id: 1,
                    parent: None,
                    local: Mat4::IDENTITY,
                    inverse_bind: Mat4::IDENTITY,
                }],
                placement: None,
            }],
            auxiliary_nodes: Vec::new(),
        };

        let context = |path: &str| PhysicsComponentContext {
            source_path: path.to_owned(),
            ..Default::default()
        };
        let mut physics = nw_model::PhysicsScene {
            hit_volumes: vec![HitVolume {
                context: context("slices/characters/alligator.dynamicslice"),
                center: Vec3::ZERO,
                shape: QueryShape::Sphere { radius: 0.5 },
                damage_multiplier: 1.0,
                is_headshot: false,
                is_legshot: false,
                volume_name: "head".to_owned(),
                filter: String::new(),
                target_bone_name: "bind_mouth_web".to_owned(),
                hit_category: String::new(),
                lightweight_character_entity_id: 0,
                source: serde_json::Value::Null,
            }],
            rigid_bodies: vec![
                RigidBody {
                    context: context("slices/instanced_loot_container.dynamicslice"),
                    name: "raycast_box".to_owned(),
                    center: Vec3::ZERO,
                    shape: Some(QueryShape::Box { size: Vec3::ONE }),
                    shape_asset_path: None,
                    source: serde_json::Value::Null,
                },
                RigidBody {
                    context: context("slices/pois/crafting/settlement_alligator_mesh.dynamicslice"),
                    name: "prop_cylinder".to_owned(),
                    center: Vec3::ZERO,
                    shape: Some(QueryShape::Cylinder {
                        height: 1.0,
                        radius: 0.5,
                    }),
                    shape_asset_path: None,
                    source: serde_json::Value::Null,
                },
            ],
            ..Default::default()
        };

        scope_scene_physics(&model, &mut physics);

        // The skeleton-targeting hit volume's slice owns the character physics;
        // the loot-container and POI rigid bodies are foreign and dropped.
        assert_eq!(physics.hit_volumes.len(), 1);
        assert!(physics.rigid_bodies.is_empty());
    }

    #[test]
    fn scene_physics_decodes_every_generated_query_shape_variant() {
        use nw_objectstream::types;
        use nw_reflected_types::types::{HitVolumeComponent, TransformComponent};

        let shapes = [
            (
                "QueryShapeAabb",
                "27462017-FE0F-4B81-96E9-8875B750EC69",
                format!(
                    r#"<Class name="Vector3" field="m_aabb" value="1 2 3" type="{{{}}}"/>"#,
                    types::VECTOR3
                ),
            ),
            (
                "QueryShapeBox",
                "C6651A66-23D4-4508-B4AD-180C516655A8",
                format!(
                    r#"<Class name="Vector3" field="m_box" value="2 3 4" type="{{{}}}"/>"#,
                    types::VECTOR3
                ),
            ),
            (
                "QueryShapeCapsule",
                "7495C65C-9193-4193-BBB2-DE3343B9EB03",
                format!(
                    r#"<Class name="float" field="m_height" value="4" type="{{{float}}}"/><Class name="float" field="m_radius" value="1" type="{{{float}}}"/><Class name="Vector3" field="m_axis" value="0 0 1" type="{{{vector}}}"/>"#,
                    float = types::FLOAT,
                    vector = types::VECTOR3,
                ),
            ),
            (
                "QueryShapeCylinder",
                "709B11EA-FD56-4FEF-B841-7CEA549368E6",
                format!(
                    r#"<Class name="float" field="m_height" value="3" type="{{{0}}}"/><Class name="float" field="m_radius" value="1" type="{{{0}}}"/>"#,
                    types::FLOAT
                ),
            ),
            (
                "QueryShapePoint",
                "44B34B6C-63B0-443C-BEEE-272EA4106EDC",
                String::new(),
            ),
            (
                "QueryShapeSphere",
                "7F2EF312-4089-4582-89C5-5D4156DAA7FB",
                format!(
                    r#"<Class name="float" field="m_radius" value="1" type="{{{}}}"/>"#,
                    types::FLOAT
                ),
            ),
        ];
        let mut entities = String::new();
        for (index, (shape_name, shape_id, shape_fields)) in shapes.iter().enumerate() {
            let id = index + 1;
            entities.push_str(&format!(
                r#"<Class name="AZ::Entity" type="{{{entity}}}"><Class name="AZ::EntityId" field="Id" type="{{{entity_id}}}"><Class name="AZ::u64" field="id" value="{id}" type="{{{u64_type}}}"/></Class><Class name="AZStd::string" field="Name" value="shape_{id}" type="{{{string}}}"/><Class name="AZStd::vector" field="Components" type="{{{vector_type}}}"><Class name="TransformComponent" version="0" type="{{{transform_component}}}"><Class name="Transform" field="Transform" value="1 0 0 0 1 0 0 0 1 10 20 30" type="{{{transform}}}"/></Class><Class name="HitVolumeComponent" version="0" type="{{{hit_volume}}}"><Class name="{shape_name}" field="m_shape" type="{{{shape_id}}}">{shape_fields}</Class><Class name="AZStd::string" field="m_volumeName" value="shape_{id}" type="{{{string}}}"/></Class></Class></Class>"#,
                entity = types::AZ_ENTITY,
                entity_id = types::ENTITY_ID,
                u64_type = types::AZ_U64,
                string = types::AZSTD_STRING,
                vector_type = types::AZSTD_VECTOR,
                transform_component = TransformComponent::TYPE_ID,
                transform = types::TRANSFORM,
                hit_volume = HitVolumeComponent::TYPE_ID,
            ));
        }
        let xml = format!(r#"<ObjectStream version="3">{entities}</ObjectStream>"#);
        let physics = parse_scene_physics("slices/query_shapes.slice", xml.as_bytes()).unwrap();

        assert_eq!(physics.hit_volumes.len(), 6);
        assert!(matches!(
            physics.hit_volumes[0].shape,
            nw_model::QueryShape::Aabb { .. }
        ));
        assert!(matches!(
            physics.hit_volumes[1].shape,
            nw_model::QueryShape::Box { .. }
        ));
        assert!(matches!(
            physics.hit_volumes[2].shape,
            nw_model::QueryShape::Capsule { .. }
        ));
        assert!(matches!(
            physics.hit_volumes[3].shape,
            nw_model::QueryShape::Cylinder { .. }
        ));
        assert!(matches!(
            physics.hit_volumes[4].shape,
            nw_model::QueryShape::Point
        ));
        assert!(matches!(
            physics.hit_volumes[5].shape,
            nw_model::QueryShape::Sphere { .. }
        ));
        assert_eq!(physics.hit_volumes[0].context.entity_id, Some(1));
        assert_eq!(
            physics.hit_volumes[0].context.entity_name.as_deref(),
            Some("shape_1")
        );
        assert_eq!(
            physics.hit_volumes[0].context.transform.translation,
            Vec3::new(10.0, 20.0, 30.0)
        );
    }

    #[test]
    fn game_rigid_body_versions_are_fail_closed() {
        let mut component = nw_objectstream::Element::new(
            *nw_reflected_types::types::GameRigidBodyComponent::TYPE_ID.as_inner(),
        );
        component.version = Some(1);
        let fields = nw_objectstream::schema::SchemaFieldMap::default();
        let error = decode_game_rigid_body(&component, &fields).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("version 1 requires m_configuration")
        );
    }
}
