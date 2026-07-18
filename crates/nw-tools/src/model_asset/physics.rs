//! Scene physics: scoping character-owned collision, and decoding ObjectStream
//! hit volumes / rigid bodies from consumer slices.
//!
//! Split out of `model_asset` as a pure move; shared helpers stay in the parent.

use std::collections::{BTreeMap, BTreeSet};
use std::mem;

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
/// field) is untouched. Equivalent complete hit-volume sets from alternate
/// owning slices collapse to their lexicographically-first source, retaining the
/// other source paths as provenance on every kept component context. Rigid bodies
/// follow their source hit-volume variant, so they cannot make alternate character
/// contexts appear simultaneously.
pub(super) fn scope_scene_physics(model: &nw_model::Model, physics: &mut nw_model::PhysicsScene) {
    if physics.hit_volumes.is_empty() && physics.rigid_bodies.is_empty() {
        return;
    }
    let skeleton = model.primary_skeleton();
    let mut owning: BTreeSet<String> = physics
        .hit_volumes
        .iter()
        .filter(|volume| {
            !volume.target_bone_name.is_empty()
                && skeleton
                    .is_some_and(|skeleton| skeleton.bone_index(&volume.target_bone_name).is_some())
        })
        .map(|volume| source_key(&volume.context.source_path))
        .collect();
    if owning.is_empty() {
        owning.extend(primary_physics_source(physics));
    }

    let mut slices = BTreeMap::new();
    for volume in mem::take(&mut physics.hit_volumes) {
        if owning.contains(&source_key(&volume.context.source_path)) {
            slice_for_source(&mut slices, &volume.context.source_path)
                .hit_volumes
                .push(volume);
        }
    }
    for body in mem::take(&mut physics.rigid_bodies) {
        if owning.contains(&source_key(&body.context.source_path)) {
            slice_for_source(&mut slices, &body.context.source_path)
                .rigid_bodies
                .push(body);
        }
    }

    let mut variants = BTreeMap::<SlicePhysicsSignature, Vec<SlicePhysics>>::new();
    for slice in slices.into_values() {
        variants.entry(slice.signature()).or_default().push(slice);
    }

    let mut kept_slices = variants
        .into_values()
        .map(collapse_variant_slices)
        .collect::<Vec<_>>();
    kept_slices.sort_by(|left, right| compare_source_paths(&left.source_path, &right.source_path));

    for slice in &mut kept_slices {
        slice
            .hit_volumes
            .sort_by_key(|volume| HitVolumeOrderKey::from(volume));
        slice
            .rigid_bodies
            .sort_by_key(|body| RigidBodyOrderKey::from(body));
    }
    physics.hit_volumes = kept_slices
        .iter_mut()
        .flat_map(|slice| mem::take(&mut slice.hit_volumes))
        .collect();
    physics.rigid_bodies = kept_slices
        .into_iter()
        .flat_map(|slice| slice.rigid_bodies)
        .collect();
}

/// The single primary source slice for a static-prop physics export: the slice
/// contributing the most physics components, ties broken by the
/// lexicographically-first normalized path.
fn primary_physics_source(physics: &nw_model::PhysicsScene) -> Option<String> {
    let mut counts = BTreeMap::<String, usize>::new();
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
        *counts.entry(source_key(source)).or_default() += 1;
    }
    // BTreeMap iterates ascending by key, so `reduce` retains the earlier
    // (lexicographically-first) path on a tie.
    counts
        .into_iter()
        .reduce(|best, current| if current.1 > best.1 { current } else { best })
        .map(|(path, _)| path)
}

fn source_key(path: &str) -> String {
    normalize_path(path).to_ascii_lowercase()
}

fn compare_source_paths(left: &str, right: &str) -> std::cmp::Ordering {
    source_key(left)
        .cmp(&source_key(right))
        .then_with(|| normalize_path(left).cmp(&normalize_path(right)))
}

#[derive(Default)]
struct SlicePhysics {
    source_path: String,
    hit_volumes: Vec<nw_model::HitVolume>,
    rigid_bodies: Vec<nw_model::RigidBody>,
}

impl SlicePhysics {
    fn signature(&self) -> SlicePhysicsSignature {
        let mut hit_volumes = self
            .hit_volumes
            .iter()
            .map(HitVolumeSignature::from)
            .collect::<Vec<_>>();
        hit_volumes.sort();
        SlicePhysicsSignature { hit_volumes }
    }
}

fn slice_for_source<'a>(
    slices: &'a mut BTreeMap<String, SlicePhysics>,
    source_path: &str,
) -> &'a mut SlicePhysics {
    let source_path = normalize_path(source_path);
    let key = source_key(&source_path);
    let slice = slices.entry(key).or_insert_with(|| SlicePhysics {
        source_path: source_path.clone(),
        ..Default::default()
    });
    if compare_source_paths(&source_path, &slice.source_path).is_lt() {
        slice.source_path = source_path;
    }
    slice
}

fn collapse_variant_slices(mut variants: Vec<SlicePhysics>) -> SlicePhysics {
    variants.sort_by(|left, right| compare_source_paths(&left.source_path, &right.source_path));
    let mut kept = variants.remove(0);
    let alternate_source_paths = variants
        .into_iter()
        .map(|variant| variant.source_path)
        .collect::<Vec<_>>();
    for volume in &mut kept.hit_volumes {
        volume.context.source_path.clone_from(&kept.source_path);
        volume
            .context
            .alternate_source_paths
            .clone_from(&alternate_source_paths);
    }
    for body in &mut kept.rigid_bodies {
        body.context.source_path.clone_from(&kept.source_path);
        body.context
            .alternate_source_paths
            .clone_from(&alternate_source_paths);
    }
    kept
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SlicePhysicsSignature {
    hit_volumes: Vec<HitVolumeSignature>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct HitVolumeSignature {
    volume_name: String,
    target_bone_name: String,
    center: Vec3Signature,
    shape: QueryShapeSignature,
    damage_multiplier: u32,
    is_headshot: bool,
    is_legshot: bool,
}

impl From<&nw_model::HitVolume> for HitVolumeSignature {
    fn from(volume: &nw_model::HitVolume) -> Self {
        Self {
            volume_name: volume.volume_name.clone(),
            target_bone_name: volume.target_bone_name.clone(),
            center: volume.center.into(),
            shape: (&volume.shape).into(),
            damage_multiplier: volume.damage_multiplier.to_bits(),
            is_headshot: volume.is_headshot,
            is_legshot: volume.is_legshot,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct RigidBodySignature {
    center: Vec3Signature,
    shape: Option<QueryShapeSignature>,
    shape_asset_path: Option<String>,
    source: String,
    transform: PhysicsTransformSignature,
}

impl From<&nw_model::RigidBody> for RigidBodySignature {
    fn from(body: &nw_model::RigidBody) -> Self {
        Self {
            center: body.center.into(),
            shape: body.shape.as_ref().map(QueryShapeSignature::from),
            shape_asset_path: body.shape_asset_path.as_deref().map(source_key),
            // The lossless payload keeps ordering stable when the projected
            // shape fields alone do not distinguish two rigid bodies.
            source: body.source.to_string(),
            transform: body.context.transform.into(),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum QueryShapeSignature {
    Aabb(Vec3Signature),
    Box(Vec3Signature),
    Capsule {
        height: u32,
        radius: u32,
        axis: Vec3Signature,
    },
    Cylinder {
        height: u32,
        radius: u32,
    },
    Point,
    Sphere(u32),
}

impl From<&nw_model::QueryShape> for QueryShapeSignature {
    fn from(shape: &nw_model::QueryShape) -> Self {
        match shape {
            nw_model::QueryShape::Aabb { half_extents } => Self::Aabb((*half_extents).into()),
            nw_model::QueryShape::Box { size } => Self::Box((*size).into()),
            nw_model::QueryShape::Capsule {
                height,
                radius,
                axis,
            } => Self::Capsule {
                height: height.to_bits(),
                radius: radius.to_bits(),
                axis: (*axis).into(),
            },
            nw_model::QueryShape::Cylinder { height, radius } => Self::Cylinder {
                height: height.to_bits(),
                radius: radius.to_bits(),
            },
            nw_model::QueryShape::Point => Self::Point,
            nw_model::QueryShape::Sphere { radius } => Self::Sphere(radius.to_bits()),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Vec3Signature([u32; 3]);

impl From<Vec3> for Vec3Signature {
    fn from(value: Vec3) -> Self {
        Self(value.to_array().map(f32::to_bits))
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct PhysicsTransformSignature {
    translation: Vec3Signature,
    rotation: [u32; 4],
    scale: Vec3Signature,
}

impl From<nw_model::PhysicsTransform> for PhysicsTransformSignature {
    fn from(transform: nw_model::PhysicsTransform) -> Self {
        Self {
            translation: transform.translation.into(),
            rotation: transform.rotation.to_array().map(f32::to_bits),
            scale: transform.scale.into(),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct HitVolumeOrderKey {
    signature: HitVolumeSignature,
    transform: PhysicsTransformSignature,
    entity_id: Option<u64>,
    entity_name: Option<String>,
    filter: String,
    hit_category: String,
    lightweight_character_entity_id: u64,
    source: String,
}

impl From<&nw_model::HitVolume> for HitVolumeOrderKey {
    fn from(volume: &nw_model::HitVolume) -> Self {
        Self {
            signature: volume.into(),
            transform: volume.context.transform.into(),
            entity_id: volume.context.entity_id,
            entity_name: volume.context.entity_name.clone(),
            filter: volume.filter.clone(),
            hit_category: volume.hit_category.clone(),
            lightweight_character_entity_id: volume.lightweight_character_entity_id,
            source: volume.source.to_string(),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct RigidBodyOrderKey {
    signature: RigidBodySignature,
    name: String,
    entity_id: Option<u64>,
    entity_name: Option<String>,
}

impl From<&nw_model::RigidBody> for RigidBodyOrderKey {
    fn from(body: &nw_model::RigidBody) -> Self {
        Self {
            signature: body.into(),
            name: body.name.clone(),
            entity_id: body.context.entity_id,
            entity_name: body.context.entity_name.clone(),
        }
    }
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

    fn model_with_hit_volume_bone() -> nw_model::Model {
        nw_model::Model {
            meshes: Vec::new(),
            skeletons: vec![nw_model::Skeleton {
                bones: vec![nw_model::Bone {
                    name: "bind_spine_1_jnt".to_owned(),
                    controller_id: 1,
                    parent: None,
                    local: Mat4::IDENTITY,
                    inverse_bind: Mat4::IDENTITY,
                }],
                placement: None,
            }],
            auxiliary_nodes: Vec::new(),
        }
    }

    fn component_context(
        path: &str,
        entity_id: u64,
        entity_name: &str,
    ) -> nw_model::PhysicsComponentContext {
        nw_model::PhysicsComponentContext {
            source_path: path.to_owned(),
            entity_id: Some(entity_id),
            entity_name: Some(entity_name.to_owned()),
            ..Default::default()
        }
    }

    fn hit_volume(
        path: &str,
        entity_id: u64,
        volume_name: &str,
        shape: nw_model::QueryShape,
    ) -> nw_model::HitVolume {
        nw_model::HitVolume {
            context: component_context(path, entity_id, volume_name),
            center: Vec3::new(0.0, 0.0, 0.5),
            shape,
            damage_multiplier: 1.0,
            is_headshot: volume_name == "Head",
            is_legshot: false,
            volume_name: volume_name.to_owned(),
            filter: String::new(),
            target_bone_name: "bind_spine_1_jnt".to_owned(),
            hit_category: String::new(),
            lightweight_character_entity_id: 0,
            source: serde_json::Value::Null,
        }
    }

    fn rigid_body(
        path: &str,
        entity_id: u64,
        name: &str,
        shape: nw_model::QueryShape,
    ) -> nw_model::RigidBody {
        nw_model::RigidBody {
            context: component_context(path, entity_id, name),
            name: name.to_owned(),
            center: Vec3::ZERO,
            shape: Some(shape),
            shape_asset_path: None,
            source: serde_json::json!({ "configuration": { "isDynamic": false } }),
        }
    }

    #[test]
    fn scene_physics_collapses_identical_variant_sets_with_provenance() {
        let primary = "slices/characters/wolf_elemental_earth.dynamicslice";
        let alternate = "slices/characters/wolf_elemental_earth_fl_noblight.dynamicslice";
        let hit_shape = nw_model::QueryShape::Capsule {
            height: 0.9,
            radius: 0.26,
            axis: Vec3::Z,
        };
        let body_shape = nw_model::QueryShape::Box { size: Vec3::ONE };
        let mut physics = nw_model::PhysicsScene {
            hit_volumes: vec![
                hit_volume(alternate, 20, "Body", hit_shape.clone()),
                hit_volume(primary, 10, "Body", hit_shape),
            ],
            rigid_bodies: vec![
                rigid_body(alternate, 21, "alternate_body", body_shape.clone()),
                rigid_body(primary, 11, "primary_body", body_shape),
            ],
            ..Default::default()
        };

        scope_scene_physics(&model_with_hit_volume_bone(), &mut physics);

        assert_eq!(physics.hit_volumes.len(), 1);
        assert_eq!(physics.rigid_bodies.len(), 1);
        for context in [
            &physics.hit_volumes[0].context,
            &physics.rigid_bodies[0].context,
        ] {
            assert_eq!(context.source_path, primary);
            assert_eq!(context.alternate_source_paths, [alternate]);
        }
        let context = serde_json::to_value(&physics).unwrap();
        assert_eq!(
            context["hitVolumes"][0]["context"]["alternateSourcePaths"],
            serde_json::json!([alternate])
        );
    }

    #[test]
    fn scene_physics_preserves_distinct_body_capsules_within_one_slice() {
        let source = "slices/characters/wolf_elemental.dynamicslice";
        let mut physics = nw_model::PhysicsScene {
            hit_volumes: vec![hit_volume(
                source,
                1,
                "Head",
                nw_model::QueryShape::Sphere { radius: 0.26 },
            )],
            rigid_bodies: vec![
                rigid_body(
                    source,
                    2,
                    "Body",
                    nw_model::QueryShape::Capsule {
                        height: 0.9,
                        radius: 0.26,
                        axis: Vec3::Z,
                    },
                ),
                rigid_body(
                    source,
                    3,
                    "Body",
                    nw_model::QueryShape::Capsule {
                        height: 1.2,
                        radius: 0.26,
                        axis: Vec3::Z,
                    },
                ),
            ],
            ..Default::default()
        };

        scope_scene_physics(&model_with_hit_volume_bone(), &mut physics);

        let heights = physics
            .rigid_bodies
            .iter()
            .map(|body| match &body.shape {
                Some(nw_model::QueryShape::Capsule { height, .. }) => *height,
                _ => panic!("expected Body capsule"),
            })
            .collect::<Vec<_>>();
        assert_eq!(heights, [0.9, 1.2]);
        assert!(
            physics.hit_volumes[0]
                .context
                .alternate_source_paths
                .is_empty()
        );
        assert!(
            physics
                .rigid_bodies
                .iter()
                .all(|body| body.context.alternate_source_paths.is_empty())
        );
        let context = serde_json::to_value(&physics).unwrap();
        assert!(
            context["hitVolumes"][0]["context"]
                .get("alternateSourcePaths")
                .is_none()
        );
    }

    #[test]
    fn scene_physics_keeps_differing_variant_sets() {
        let first = "slices/characters/wolf_elemental.dynamicslice";
        let second = "slices/characters/wolf_elemental_armored.dynamicslice";
        let mut physics = nw_model::PhysicsScene {
            hit_volumes: vec![
                hit_volume(
                    second,
                    2,
                    "Body",
                    nw_model::QueryShape::Capsule {
                        height: 1.2,
                        radius: 0.26,
                        axis: Vec3::Z,
                    },
                ),
                hit_volume(
                    first,
                    1,
                    "Body",
                    nw_model::QueryShape::Capsule {
                        height: 0.9,
                        radius: 0.26,
                        axis: Vec3::Z,
                    },
                ),
            ],
            ..Default::default()
        };

        scope_scene_physics(&model_with_hit_volume_bone(), &mut physics);

        assert_eq!(physics.hit_volumes.len(), 2);
        assert_eq!(
            physics
                .hit_volumes
                .iter()
                .map(|volume| volume.context.source_path.as_str())
                .collect::<Vec<_>>(),
            [first, second],
            "distinct sets keep their original source contexts in deterministic path order"
        );
        assert!(
            physics
                .hit_volumes
                .iter()
                .all(|volume| volume.context.alternate_source_paths.is_empty())
        );
    }

    #[test]
    fn scene_physics_is_deterministic_when_input_order_changes() {
        let primary = "slices/characters/wolf_elemental_earth.dynamicslice";
        let alternate = "slices/characters/wolf_elemental_earth_fl_noblight.dynamicslice";
        let different = "slices/characters/wolf_elemental_fire.dynamicslice";
        let body_shape = nw_model::QueryShape::Capsule {
            height: 0.9,
            radius: 0.26,
            axis: Vec3::Z,
        };
        let mut first = nw_model::PhysicsScene {
            hit_volumes: vec![
                hit_volume(alternate, 20, "Body", body_shape.clone()),
                hit_volume(
                    different,
                    30,
                    "Body",
                    nw_model::QueryShape::Sphere { radius: 0.26 },
                ),
                hit_volume(primary, 10, "Body", body_shape.clone()),
            ],
            rigid_bodies: vec![
                rigid_body(alternate, 21, "alternate_body", body_shape.clone()),
                rigid_body(primary, 11, "primary_body", body_shape),
            ],
            ..Default::default()
        };
        let mut permuted = first.clone();
        first.hit_volumes.reverse();
        first.rigid_bodies.reverse();

        scope_scene_physics(&model_with_hit_volume_bone(), &mut first);
        scope_scene_physics(&model_with_hit_volume_bone(), &mut permuted);

        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&permuted).unwrap()
        );
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
