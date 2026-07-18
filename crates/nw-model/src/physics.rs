//! Export-owned physics scene data.
//!
//! These types are the glTF projection boundary. Legacy `.rnr` parsing stays in
//! `nw-rnr`, while runtime gems own their independent runtime products.

use glam::{Mat4, Quat, Vec3, Vec4};
use serde::Serialize;
use std::sync::Arc;

/// Physics, collision, and hit-volume data attached to one glTF scene.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsScene {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shape_assets: Vec<PhysicsShapeAsset>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hit_volumes: Vec<HitVolume>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rigid_bodies: Vec<RigidBody>,
}

impl PhysicsScene {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shape_assets.is_empty() && self.hit_volumes.is_empty() && self.rigid_bodies.is_empty()
    }

    /// Validate the complete scene without allocating visualization meshes.
    /// Tessellation remains a single build-time operation in the glTF writer.
    pub fn validate(&self) -> Result<(), PhysicsSceneError> {
        for asset in &self.shape_assets {
            for shape in &asset.shapes {
                validate_physical_shape(shape)?;
            }
        }
        for volume in &self.hit_volumes {
            validate_query_shape(&volume.shape)?;
        }
        for body in &self.rigid_bodies {
            if let Some(shape) = &body.shape {
                validate_query_shape(shape)?;
            }
        }
        Ok(())
    }

    pub(crate) fn visuals_with_runner(
        &self,
        runner: &nw_jobs::JobRunner,
    ) -> Result<Vec<PhysicsVisual>, PhysicsSceneError> {
        let mut tasks = Vec::new();
        for (asset_index, asset) in self.shape_assets.iter().enumerate() {
            for (shape_index, shape) in asset.shapes.iter().enumerate() {
                let name = asset.objects.get(shape_index).map_or_else(
                    || format!("shape_{shape_index}"),
                    |object| object.name.clone(),
                );
                tasks.push(PhysicsVisualTask::RockNRoll {
                    shape,
                    // Authored name (or `shape_<index>` fallback); the role/kind
                    // is carried in node extras, so no `__physics__/…` prefix.
                    name,
                    asset: asset_index,
                    shape_index,
                });
            }
        }
        for (index, volume) in self.hit_volumes.iter().enumerate() {
            tasks.push(PhysicsVisualTask::HitVolume { volume, index });
        }
        for (index, body) in self.rigid_bodies.iter().enumerate() {
            if body.shape.is_some() {
                tasks.push(PhysicsVisualTask::RigidBody { body, index });
            }
        }
        let visuals = runner.try_map(&tasks, PhysicsVisualTask::build)?;
        Ok(visuals.into_iter().flatten().collect())
    }
}

enum PhysicsVisualTask<'a> {
    RockNRoll {
        shape: &'a PhysicalShape,
        name: String,
        asset: usize,
        shape_index: usize,
    },
    HitVolume {
        volume: &'a HitVolume,
        index: usize,
    },
    RigidBody {
        body: &'a RigidBody,
        index: usize,
    },
}

impl PhysicsVisualTask<'_> {
    fn build(&self) -> Result<Vec<PhysicsVisual>, PhysicsSceneError> {
        match self {
            Self::RockNRoll {
                shape,
                name,
                asset,
                shape_index,
            } => {
                let mut output = Vec::new();
                collect_shape_visuals(
                    shape,
                    Mat4::IDENTITY,
                    name,
                    PhysicsVisualRole::RockNRoll {
                        asset: *asset,
                        shape: *shape_index,
                    },
                    None,
                    &mut output,
                )?;
                Ok(output)
            }
            Self::HitVolume { volume, index } => {
                // Hit-volume authoring is Cry/Z-up. Skeleton joints and the rest
                // of the glTF scene are Y-up via cry_to_gltf, so convert center +
                // shape here or the capsule axis stays in Cry space while its
                // parent bone is already in glTF space (Head m_axis=Z becomes
                // bone-local up → a vertical snout-crossing capsule).
                let center = crate::math::cry_to_gltf(volume.center);
                let shape = query_shape_to_gltf(&volume.shape);
                Ok(vec![PhysicsVisual {
                    // Authored volume name; role is carried in node extras.
                    name: volume.volume_name.clone(),
                    transform: physics_transform_to_gltf(volume.context.transform)
                        * Mat4::from_translation(center),
                    geometry: query_shape_geometry(&shape)?,
                    role: PhysicsVisualRole::HitVolume { index: *index },
                    target_bone: (!volume.target_bone_name.is_empty()).then(|| PhysicsBoneTarget {
                        name: volume.target_bone_name.clone(),
                        // When the volume rides its bone, the joint provides world
                        // placement; only the bone-local center is baked here (the
                        // shape axis stays in the mesh geometry).
                        local: Mat4::from_translation(center),
                    }),
                }])
            }
            Self::RigidBody { body, index } => {
                let center = crate::math::cry_to_gltf(body.center);
                let shape = body
                    .shape
                    .as_ref()
                    .map(query_shape_to_gltf)
                    .expect("rigid-body task has a query shape");
                Ok(vec![PhysicsVisual {
                    // Authored body name; role is carried in node extras.
                    name: body.name.clone(),
                    transform: physics_transform_to_gltf(body.context.transform)
                        * Mat4::from_translation(center),
                    geometry: query_shape_geometry(&shape)?,
                    role: PhysicsVisualRole::RigidBody { index: *index },
                    target_bone: None,
                }])
            }
        }
    }
}

fn validate_physical_shape(shape: &PhysicalShape) -> Result<(), PhysicsSceneError> {
    match &shape.data {
        PhysicsShapeData::Box { half_extents, .. } => {
            validate_positive_vec3(*half_extents)?;
        }
        PhysicsShapeData::Sphere { radius } => validate_positive(*radius)?,
        PhysicsShapeData::ConvexHull {
            vertices, planes, ..
        } => {
            if vertices.iter().any(|value| !value.is_finite())
                || planes.iter().any(|value| !value.is_finite())
            {
                return Err(PhysicsSceneError::InvalidDimension);
            }
        }
        PhysicsShapeData::Cylinder {
            half_height,
            radius,
            ..
        } => {
            validate_positive(*half_height)?;
            validate_positive(*radius)?;
        }
        PhysicsShapeData::CylinderUnaligned {
            endpoint_a,
            endpoint_b,
            radius,
            ..
        }
        | PhysicsShapeData::CapsuleUnaligned {
            endpoint_a,
            endpoint_b,
            radius,
        } => {
            normalized_axis(*endpoint_b - *endpoint_a)?;
            validate_positive(*radius)?;
        }
        PhysicsShapeData::Capsule {
            half_height,
            radius,
        } => {
            if !half_height.is_finite() || *half_height < 0.0 {
                return Err(PhysicsSceneError::InvalidDimension);
            }
            validate_positive(*radius)?;
        }
        PhysicsShapeData::Triangle { a, b, c, .. } => {
            if !a.is_finite() || !b.is_finite() || !c.is_finite() {
                return Err(PhysicsSceneError::InvalidDimension);
            }
        }
        PhysicsShapeData::Mesh {
            vertices, indices, ..
        } => validate_indexed_mesh(vertices, indices)?,
        PhysicsShapeData::Compound { children } => {
            for child in children {
                validate_shape_transform(&child.transform)?;
                validate_physical_shape(&child.shape)?;
            }
        }
        PhysicsShapeData::Transform { transform, shape } => {
            validate_shape_transform(transform)?;
            validate_physical_shape(shape)?;
        }
        PhysicsShapeData::ScaleConvexPolytope { scale, shape, .. }
        | PhysicsShapeData::ScaleMesh { scale, shape, .. } => {
            validate_positive_vec3(scale.abs())?;
            validate_physical_shape(shape)?;
        }
        PhysicsShapeData::SoftBody | PhysicsShapeData::HeightField { data: None, .. } => {}
        PhysicsShapeData::Plane {
            plane,
            aabb_min,
            aabb_max,
        } => {
            normalized_axis(plane.truncate())?;
            if !plane.is_finite() || !aabb_min.is_finite() || !aabb_max.is_finite() {
                return Err(PhysicsSceneError::InvalidDimension);
            }
        }
        PhysicsShapeData::HeightField {
            data: Some(data), ..
        } => validate_height_field(data)?,
    }
    Ok(())
}

fn validate_query_shape(shape: &QueryShape) -> Result<(), PhysicsSceneError> {
    match shape {
        QueryShape::Aabb { half_extents } => validate_positive_vec3(half_extents.abs()),
        QueryShape::Box { size } => validate_positive_vec3(size.abs()),
        QueryShape::Capsule {
            height,
            radius,
            axis,
        } => {
            validate_positive(*height)?;
            validate_positive(*radius)?;
            normalized_axis(*axis)?;
            Ok(())
        }
        QueryShape::Cylinder { height, radius } => {
            validate_positive(*height)?;
            validate_positive(*radius)
        }
        QueryShape::Point => Ok(()),
        QueryShape::Sphere { radius } => validate_positive(*radius),
    }
}

fn validate_shape_transform(transform: &ShapeTransform) -> Result<(), PhysicsSceneError> {
    if transform.iter().any(|column| !column.is_finite()) {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    Ok(())
}

fn validate_positive_vec3(value: Vec3) -> Result<(), PhysicsSceneError> {
    if !value.is_finite() || value.min_element() <= 0.0 {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    Ok(())
}

fn validate_positive(value: f32) -> Result<(), PhysicsSceneError> {
    if !finite_positive(value) {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    Ok(())
}

fn validate_indexed_mesh(vertices: &[Vec3], indices: &[u16]) -> Result<(), PhysicsSceneError> {
    if vertices.iter().any(|vertex| !vertex.is_finite()) {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    if !indices.len().is_multiple_of(3) {
        return Err(PhysicsSceneError::InvalidMeshIndexCount(indices.len()));
    }
    if let Some(&index) = indices
        .iter()
        .find(|&&index| usize::from(index) >= vertices.len())
    {
        return Err(PhysicsSceneError::MeshIndexOutOfRange {
            index: u32::from(index),
            vertex_count: vertices.len(),
        });
    }
    Ok(())
}

/// One source `.rnr` asset plus the complete typed shape tree.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsShapeAsset {
    pub source_path: String,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_guid: Option<[u8; 16]>,
    pub objects: Vec<PhysicsShapeObject>,
    pub material_filter: PhysicsMaterialFilter,
    pub shapes: Vec<PhysicalShape>,
    /// Original stream bytes are embedded through a glTF buffer view. Keeping
    /// them out of JSON avoids duplicating BVH and opaque payloads.
    #[serde(skip)]
    pub source_bytes: Arc<[u8]>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsShapeObject {
    pub name: String,
    pub material_indices: Vec<u16>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsMaterialFilter {
    pub enabled: bool,
    pub secondary_geometry: bool,
    pub indices: Vec<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalShape {
    pub data: PhysicsShapeData,
    pub extra_byte_length: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PhysicsShapeData {
    Box {
        half_extents: Vec3,
        convex_radius: f32,
    },
    Sphere {
        radius: f32,
    },
    ConvexHull {
        vertices: Vec<Vec3>,
        planes: Vec<Vec4>,
        convex_radius: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        extra: Option<ConvexHullExtra>,
    },
    Cylinder {
        half_height: f32,
        radius: f32,
        convex_radius: f32,
    },
    CylinderUnaligned {
        endpoint_a: Vec3,
        endpoint_b: Vec3,
        radius: f32,
        convex_radius: f32,
    },
    Capsule {
        half_height: f32,
        radius: f32,
    },
    CapsuleUnaligned {
        endpoint_a: Vec3,
        endpoint_b: Vec3,
        radius: f32,
    },
    Triangle {
        a: Vec3,
        b: Vec3,
        c: Vec3,
        convex_radius: f32,
    },
    Mesh {
        stream_header: u32,
        vertices: Vec<Vec3>,
        indices: Vec<u16>,
        #[serde(skip_serializing_if = "Option::is_none")]
        adjacent_triangles: Option<Vec<u16>>,
        bvh: BvhTree,
    },
    Compound {
        children: Vec<CompoundChild>,
    },
    Transform {
        transform: ShapeTransform,
        shape: Box<PhysicalShape>,
    },
    SoftBody,
    Plane {
        plane: Vec4,
        aabb_min: Vec3,
        aabb_max: Vec3,
    },
    ScaleConvexPolytope {
        stream_header: u32,
        scale: Vec3,
        shape: Box<PhysicalShape>,
    },
    ScaleMesh {
        stream_header: u32,
        scale: Vec3,
        shape: Box<PhysicalShape>,
    },
    HeightField {
        layout: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<HeightFieldData>,
    },
}

pub type ShapeTransform = [Vec4; 3];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvexHullExtra {
    pub data_a: Vec<u16>,
    pub data_b: Vec<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompoundChild {
    pub transform: ShapeTransform,
    pub shape: Box<PhysicalShape>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightFieldData {
    pub version: u32,
    pub width: u32,
    pub length: u32,
    pub height_scale: f32,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub sample_byte_length: usize,
    #[serde(skip)]
    pub samples: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BvhTree {
    pub version: u32,
    pub quantized_nodes_offset: u32,
    pub subtree_infos_offset: u32,
    pub triangle_index_map_offset: u32,
    pub quantized_node_count: u32,
    pub subtree_info_count: u16,
    pub triangle_index_count: u32,
    pub flags: u16,
    pub payload_byte_length: usize,
}

/// Generated `HitVolumeComponent` projected into the export scene.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HitVolume {
    pub context: PhysicsComponentContext,
    pub center: Vec3,
    pub shape: QueryShape,
    pub damage_multiplier: f32,
    pub is_headshot: bool,
    pub is_legshot: bool,
    pub volume_name: String,
    pub filter: String,
    pub target_bone_name: String,
    pub hit_category: String,
    pub lightweight_character_entity_id: u64,
    pub source: serde_json::Value,
}

/// Generated rigid-body configuration projected into the export scene.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RigidBody {
    pub context: PhysicsComponentContext,
    pub name: String,
    pub center: Vec3,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<QueryShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape_asset_path: Option<String>,
    pub source: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum QueryShape {
    Aabb {
        half_extents: Vec3,
    },
    Box {
        size: Vec3,
    },
    Capsule {
        height: f32,
        radius: f32,
        axis: Vec3,
    },
    Cylinder {
        height: f32,
        radius: f32,
    },
    Point,
    Sphere {
        radius: f32,
    },
}

/// Source entity and placement retained at the legacy-to-export boundary.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsComponentContext {
    pub source_path: String,
    /// Equivalent alternate scene-slice contexts omitted from this component.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alternate_source_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_name: Option<String>,
    pub transform: PhysicsTransform,
}

/// Serializable transform independent of Bevy/runtime component types.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl PhysicsTransform {
    #[must_use]
    pub fn matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Default for PhysicsTransform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl TryFrom<&nw_reflected_types::types::QueryShape> for QueryShape {
    type Error = PhysicsSceneError;

    fn try_from(value: &nw_reflected_types::types::QueryShape) -> Result<Self, Self::Error> {
        use nw_reflected_types::types::QueryShape as Generated;
        Ok(match value {
            Generated::Aabb(shape) => Self::Aabb {
                half_extents: Vec3::from_array(shape.aabb.to_array()),
            },
            Generated::Box(shape) => Self::Box {
                size: Vec3::from_array(shape.box_.to_array()),
            },
            Generated::Capsule(shape) => Self::Capsule {
                height: shape.height,
                radius: shape.radius,
                axis: query_capsule_axis(Vec3::from_array(shape.axis.to_array()))?,
            },
            Generated::Cylinder(shape) => Self::Cylinder {
                height: shape.height,
                radius: shape.radius,
            },
            Generated::Point(_) => Self::Point,
            Generated::Sphere(shape) => Self::Sphere {
                radius: shape.radius,
            },
        })
    }
}

impl TryFrom<&nw_reflected_types::types::HitVolumeComponent> for HitVolume {
    type Error = PhysicsSceneError;

    fn try_from(
        value: &nw_reflected_types::types::HitVolumeComponent,
    ) -> Result<Self, Self::Error> {
        let shape = value
            .shape
            .as_ref()
            .ok_or(PhysicsSceneError::MissingQueryShape)?
            .try_into()?;
        Ok(Self {
            context: PhysicsComponentContext::default(),
            center: Vec3::from_array(value.center.to_array()),
            shape,
            damage_multiplier: value.damage_mult,
            is_headshot: value.is_headshot,
            is_legshot: value.is_legshot,
            volume_name: value.volume_name.clone(),
            filter: value.str_filter.clone(),
            target_bone_name: value.target_bone_name.clone(),
            hit_category: value.hit_category.clone(),
            lightweight_character_entity_id: value.lightweight_character_entity_id,
            source: serde_json::to_value(value).map_err(PhysicsSceneError::SerializeSource)?,
        })
    }
}

impl TryFrom<&nw_reflected_types::types::GameRigidBodyComponent> for RigidBody {
    type Error = PhysicsSceneError;

    fn try_from(
        value: &nw_reflected_types::types::GameRigidBodyComponent,
    ) -> Result<Self, Self::Error> {
        let config = &value.configuration;
        Ok(Self {
            context: PhysicsComponentContext::default(),
            name: "game_rigid_body".to_owned(),
            center: Vec3::from_array(config.center.to_array()),
            shape: config
                .collision_shape
                .as_ref()
                .map(QueryShape::try_from)
                .transpose()?,
            shape_asset_path: config.rnr_asset.hint().map(str::to_owned),
            source: serde_json::to_value(value).map_err(PhysicsSceneError::SerializeSource)?,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PhysicsSceneError {
    #[error("hit volume has no query shape")]
    MissingQueryShape,
    #[error("physics shape contains a non-finite or non-positive dimension")]
    InvalidDimension,
    #[error("physics mesh index count {0} is not divisible by three")]
    InvalidMeshIndexCount(usize),
    #[error("physics mesh index {index} is outside {vertex_count} vertices")]
    MeshIndexOutOfRange { index: u32, vertex_count: usize },
    #[error("height field dimensions overflow this platform")]
    HeightFieldSizeOverflow,
    #[error("height field version {version} expects {expected} sample bytes, got {actual}")]
    InvalidHeightFieldSamples {
        version: u32,
        expected: usize,
        actual: usize,
    },
    #[error("unsupported height-field sample version {0}")]
    UnsupportedHeightFieldVersion(u32),
    #[error("serialize generated physics component")]
    SerializeSource(#[source] serde_json::Error),
}

pub(crate) struct PhysicsVisual {
    pub name: String,
    pub transform: Mat4,
    pub geometry: Option<PhysicsGeometry>,
    pub role: PhysicsVisualRole,
    /// When set, the visual attaches to a named skeleton joint: its node parents
    /// under that joint with `local` as the bone-local transform, so it follows
    /// the bone in rest pose and during animation.
    pub target_bone: Option<PhysicsBoneTarget>,
}

/// A skeleton-joint attachment for a hit volume that rides a bone at runtime.
#[derive(Clone)]
pub(crate) struct PhysicsBoneTarget {
    pub name: String,
    pub local: Mat4,
}

pub(crate) struct PhysicsGeometry {
    pub positions: Vec<Vec3>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum PhysicsVisualRole {
    RockNRoll { asset: usize, shape: usize },
    HitVolume { index: usize },
    RigidBody { index: usize },
}

impl PhysicsVisualRole {
    /// Hit volumes and rigid bodies come from `Javelin::QueryShapeBase` shapes,
    /// the Ghidra-confirmed debug-draw cluster that shares one hardcoded color.
    /// RockNRoll `.rnr` visuals use a different, unverified debug system and are
    /// deliberately excluded.
    pub(crate) fn is_query_shape(self) -> bool {
        matches!(self, Self::HitVolume { .. } | Self::RigidBody { .. })
    }
}

fn collect_shape_visuals(
    shape: &PhysicalShape,
    transform: Mat4,
    name: &str,
    role: PhysicsVisualRole,
    target_bone: Option<PhysicsBoneTarget>,
    output: &mut Vec<PhysicsVisual>,
) -> Result<(), PhysicsSceneError> {
    match &shape.data {
        PhysicsShapeData::Compound { children } => {
            for (index, child) in children.iter().enumerate() {
                collect_shape_visuals(
                    &child.shape,
                    transform * shape_transform_matrix(&child.transform),
                    &format!("{name}/child_{index}"),
                    role,
                    target_bone.clone(),
                    output,
                )?;
            }
        }
        PhysicsShapeData::Transform {
            transform: local,
            shape,
        } => collect_shape_visuals(
            shape,
            transform * shape_transform_matrix(local),
            name,
            role,
            target_bone,
            output,
        )?,
        PhysicsShapeData::ScaleConvexPolytope { scale, shape, .. }
        | PhysicsShapeData::ScaleMesh { scale, shape, .. } => collect_shape_visuals(
            shape,
            transform * Mat4::from_scale(*scale),
            name,
            role,
            target_bone,
            output,
        )?,
        data => output.push(PhysicsVisual {
            name: name.to_owned(),
            transform,
            geometry: shape_geometry(data)?,
            role,
            target_bone,
        }),
    }
    Ok(())
}

fn shape_geometry(data: &PhysicsShapeData) -> Result<Option<PhysicsGeometry>, PhysicsSceneError> {
    let geometry = match data {
        PhysicsShapeData::Box { half_extents, .. } => cuboid(*half_extents * 2.0)?,
        PhysicsShapeData::Sphere { radius } => sphere(*radius)?,
        PhysicsShapeData::ConvexHull {
            vertices, planes, ..
        } => convex_hull(vertices, planes)?,
        PhysicsShapeData::Cylinder {
            half_height,
            radius,
            ..
        } => cylinder(*radius, *half_height * 2.0, Vec3::Y)?,
        PhysicsShapeData::CylinderUnaligned {
            endpoint_a,
            endpoint_b,
            radius,
            ..
        } => segment_shape(*endpoint_a, *endpoint_b, *radius, false)?,
        PhysicsShapeData::Capsule {
            half_height,
            radius,
        } => capsule(*radius, *half_height * 2.0, Vec3::Y)?,
        PhysicsShapeData::CapsuleUnaligned {
            endpoint_a,
            endpoint_b,
            radius,
        } => segment_shape(*endpoint_a, *endpoint_b, *radius, true)?,
        PhysicsShapeData::Triangle { a, b, c, .. } => PhysicsGeometry {
            positions: vec![*a, *b, *c],
            indices: vec![0, 1, 2],
        },
        PhysicsShapeData::Mesh {
            vertices, indices, ..
        } => indexed_mesh(vertices, indices)?,
        PhysicsShapeData::SoftBody => return Ok(None),
        PhysicsShapeData::Plane {
            plane,
            aabb_min,
            aabb_max,
        } => plane_mesh(*plane, *aabb_min, *aabb_max)?,
        PhysicsShapeData::HeightField {
            data: Some(data), ..
        } => height_field(data)?,
        PhysicsShapeData::HeightField { data: None, .. } => return Ok(None),
        PhysicsShapeData::Compound { .. }
        | PhysicsShapeData::Transform { .. }
        | PhysicsShapeData::ScaleConvexPolytope { .. }
        | PhysicsShapeData::ScaleMesh { .. } => unreachable!("wrappers are flattened first"),
    };
    Ok(Some(geometry))
}

/// Convert a Cry-space query shape into the glTF Y-up basis used by skeleton joints.
fn query_shape_to_gltf(shape: &QueryShape) -> QueryShape {
    match shape {
        QueryShape::Aabb { half_extents } => QueryShape::Aabb {
            half_extents: crate::math::cry_to_gltf_scale(*half_extents),
        },
        QueryShape::Box { size } => QueryShape::Box {
            size: crate::math::cry_to_gltf_scale(*size),
        },
        QueryShape::Capsule {
            height,
            radius,
            axis,
        } => QueryShape::Capsule {
            height: *height,
            radius: *radius,
            axis: crate::math::cry_to_gltf(*axis),
        },
        QueryShape::Cylinder { height, radius } => QueryShape::Cylinder {
            height: *height,
            radius: *radius,
        },
        QueryShape::Point => QueryShape::Point,
        QueryShape::Sphere { radius } => QueryShape::Sphere { radius: *radius },
    }
}

fn physics_transform_to_gltf(transform: PhysicsTransform) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        crate::math::cry_to_gltf_scale(transform.scale),
        crate::math::cry_to_gltf_quat(transform.rotation),
        crate::math::cry_to_gltf(transform.translation),
    )
}

fn query_shape_geometry(shape: &QueryShape) -> Result<Option<PhysicsGeometry>, PhysicsSceneError> {
    Ok(Some(match shape {
        QueryShape::Aabb { half_extents } => cuboid(half_extents.abs() * 2.0)?,
        QueryShape::Box { size } => cuboid(size.abs())?,
        QueryShape::Sphere { radius } => sphere(*radius)?,
        QueryShape::Cylinder { height, radius } => cylinder(*radius, *height, Vec3::Z)?,
        QueryShape::Capsule {
            height,
            radius,
            axis,
        } => {
            // QueryShapeCapsule.m_height is the cylinder-segment length, not the
            // total end-to-end length. NewWorld 3-26 FUN_7ff60169e260 builds
            // endpoints as ±normalize(m_axis) * (scale * m_height * 0.5) and
            // passes scale * m_radius separately (DAT_7ff607efa328 = 0.5).
            // Treating height as total (height - 2r) collapses authored legs
            // (h=0.8, r=0.4) into spheres.
            let axis = normalized_axis(*axis)?;
            capsule(*radius, *height, axis)?
        }
        QueryShape::Point => return Ok(None),
    }))
}

fn cuboid(size: Vec3) -> Result<PhysicsGeometry, PhysicsSceneError> {
    if !size.is_finite() || size.min_element() <= 0.0 {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    let h = size * 0.5;
    let positions = vec![
        Vec3::new(-h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, -h.z),
        Vec3::new(h.x, h.y, -h.z),
        Vec3::new(-h.x, h.y, -h.z),
        Vec3::new(-h.x, -h.y, h.z),
        Vec3::new(h.x, -h.y, h.z),
        Vec3::new(h.x, h.y, h.z),
        Vec3::new(-h.x, h.y, h.z),
    ];
    let indices = vec![
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7, 3,
        1, 2, 6, 1, 6, 5,
    ];
    Ok(PhysicsGeometry { positions, indices })
}

fn sphere(radius: f32) -> Result<PhysicsGeometry, PhysicsSceneError> {
    if !finite_positive(radius) {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    let rings = 12_u32;
    let segments = 24_u32;
    let mut positions = Vec::with_capacity(((rings + 1) * (segments + 1)) as usize);
    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as f32 / rings as f32;
        let y = phi.cos() * radius;
        let r = phi.sin() * radius;
        for segment in 0..=segments {
            let theta = std::f32::consts::TAU * segment as f32 / segments as f32;
            positions.push(Vec3::new(theta.cos() * r, y, theta.sin() * r));
        }
    }
    let mut indices = Vec::with_capacity((rings * segments * 6) as usize);
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * (segments + 1) + segment;
            let b = a + segments + 1;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    Ok(PhysicsGeometry { positions, indices })
}

fn cylinder(radius: f32, height: f32, axis: Vec3) -> Result<PhysicsGeometry, PhysicsSceneError> {
    if !finite_positive(radius) || !finite_positive(height) {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    let segments = 24_u32;
    let half = height * 0.5;
    let mut positions = Vec::with_capacity((segments * 2 + 2) as usize);
    for &y in &[-half, half] {
        for segment in 0..segments {
            let angle = std::f32::consts::TAU * segment as f32 / segments as f32;
            positions.push(Vec3::new(angle.cos() * radius, y, angle.sin() * radius));
        }
    }
    positions.extend_from_slice(&[Vec3::new(0.0, -half, 0.0), Vec3::new(0.0, half, 0.0)]);
    let bottom = segments * 2;
    let top = bottom + 1;
    let mut indices = Vec::with_capacity((segments * 12) as usize);
    for i in 0..segments {
        let next = (i + 1) % segments;
        indices.extend_from_slice(&[i, segments + i, next, next, segments + i, segments + next]);
        indices.extend_from_slice(&[bottom, next, i, top, segments + i, segments + next]);
    }
    let rotation = Quat::from_rotation_arc(Vec3::Y, normalized_axis(axis)?);
    for position in &mut positions {
        *position = rotation * *position;
    }
    Ok(PhysicsGeometry { positions, indices })
}

fn capsule(
    radius: f32,
    segment_length: f32,
    axis: Vec3,
) -> Result<PhysicsGeometry, PhysicsSceneError> {
    if !finite_positive(radius) || !segment_length.is_finite() || segment_length < 0.0 {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    let mut geometry = sphere(radius)?;
    let half = segment_length * 0.5;
    for position in &mut geometry.positions {
        position.y += position.y.signum() * half;
    }
    let rotation = Quat::from_rotation_arc(Vec3::Y, normalized_axis(axis)?);
    for position in &mut geometry.positions {
        *position = rotation * *position;
    }
    Ok(geometry)
}

fn segment_shape(
    a: Vec3,
    b: Vec3,
    radius: f32,
    capsule_shape: bool,
) -> Result<PhysicsGeometry, PhysicsSceneError> {
    let axis = b - a;
    let length = axis.length();
    let mut geometry = if capsule_shape {
        capsule(radius, length, axis)?
    } else {
        cylinder(radius, length, axis)?
    };
    let center = (a + b) * 0.5;
    for position in &mut geometry.positions {
        *position += center;
    }
    Ok(geometry)
}

fn indexed_mesh(vertices: &[Vec3], indices: &[u16]) -> Result<PhysicsGeometry, PhysicsSceneError> {
    validate_indexed_mesh(vertices, indices)?;
    let indices = indices
        .iter()
        .map(|&index| u32::from(index))
        .collect::<Vec<_>>();
    Ok(PhysicsGeometry {
        positions: vertices.to_vec(),
        indices,
    })
}

fn convex_hull(vertices: &[Vec3], planes: &[Vec4]) -> Result<PhysicsGeometry, PhysicsSceneError> {
    if vertices.iter().any(|vertex| !vertex.is_finite())
        || planes.iter().any(|plane| !plane.is_finite())
    {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    let mut indices = Vec::new();
    let epsilon = 0.001_f32;
    for plane in planes {
        let normal = plane.truncate().normalize_or_zero();
        if normal == Vec3::ZERO {
            continue;
        }
        let mut face = vertices
            .iter()
            .enumerate()
            .filter_map(|(index, vertex)| {
                ((normal.dot(*vertex) - plane.w).abs() <= epsilon).then_some(index)
            })
            .collect::<Vec<_>>();
        if face.len() < 3 {
            continue;
        }
        let center = face.iter().map(|&index| vertices[index]).sum::<Vec3>() / face.len() as f32;
        let tangent = normal.any_orthonormal_vector();
        let bitangent = normal.cross(tangent);
        face.sort_by(|&left, &right| {
            let l = vertices[left] - center;
            let r = vertices[right] - center;
            l.dot(bitangent)
                .atan2(l.dot(tangent))
                .total_cmp(&r.dot(bitangent).atan2(r.dot(tangent)))
        });
        for index in 1..face.len() - 1 {
            indices.extend_from_slice(&[
                face[0] as u32,
                face[index] as u32,
                face[index + 1] as u32,
            ]);
        }
    }
    if indices.is_empty() && vertices.len() >= 3 {
        for index in 1..vertices.len() - 1 {
            indices.extend_from_slice(&[0, index as u32, index as u32 + 1]);
        }
    }
    Ok(PhysicsGeometry {
        positions: vertices.to_vec(),
        indices,
    })
}

fn plane_mesh(plane: Vec4, min: Vec3, max: Vec3) -> Result<PhysicsGeometry, PhysicsSceneError> {
    let normal = normalized_axis(plane.truncate())?;
    if !min.is_finite() || !max.is_finite() {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    let bounds_center = (min + max) * 0.5;
    let center = bounds_center - normal * (normal.dot(bounds_center) - plane.w);
    let a = normal.any_orthonormal_vector();
    let b = normal.cross(a);
    let extent = ((max - min).length() * 0.5).max(0.001);
    Ok(PhysicsGeometry {
        positions: vec![
            center - a * extent - b * extent,
            center + a * extent - b * extent,
            center + a * extent + b * extent,
            center - a * extent + b * extent,
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    })
}

fn height_field(data: &HeightFieldData) -> Result<PhysicsGeometry, PhysicsSceneError> {
    validate_height_field(data)?;
    let width =
        usize::try_from(data.width).map_err(|_| PhysicsSceneError::HeightFieldSizeOverflow)?;
    let length =
        usize::try_from(data.length).map_err(|_| PhysicsSceneError::HeightFieldSizeOverflow)?;
    let count = width
        .checked_mul(length)
        .ok_or(PhysicsSceneError::HeightFieldSizeOverflow)?;
    let sample_size = match data.version {
        0 => 1,
        1 => 2,
        2 => 4,
        _ => unreachable!("height field version was validated"),
    };
    let base = data.aabb_min.y;
    let heights = (0..count)
        .map(|index| {
            let offset = index * sample_size;
            let value = match data.version {
                0 => f32::from(data.samples[offset]),
                1 => f32::from(u16::from_le_bytes([
                    data.samples[offset],
                    data.samples[offset + 1],
                ])),
                2 => f32::from_le_bytes(
                    data.samples[offset..offset + 4]
                        .try_into()
                        .expect("four sample bytes"),
                ),
                _ => unreachable!(),
            };
            value * data.height_scale + base
        })
        .collect::<Vec<_>>();
    let mut positions = Vec::with_capacity(count);
    for z in 0..length {
        let tz = if length > 1 {
            z as f32 / (length - 1) as f32
        } else {
            0.0
        };
        for x in 0..width {
            let tx = if width > 1 {
                x as f32 / (width - 1) as f32
            } else {
                0.0
            };
            positions.push(Vec3::new(
                data.aabb_min.x + (data.aabb_max.x - data.aabb_min.x) * tx,
                heights[z * width + x],
                data.aabb_min.z + (data.aabb_max.z - data.aabb_min.z) * tz,
            ));
        }
    }
    let mut indices = Vec::new();
    if width > 1 && length > 1 {
        indices.reserve((width - 1) * (length - 1) * 6);
        for z in 0..length - 1 {
            for x in 0..width - 1 {
                let a = (z * width + x) as u32;
                let b = a + 1;
                let c = a + width as u32;
                let d = c + 1;
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }
    Ok(PhysicsGeometry { positions, indices })
}

fn validate_height_field(data: &HeightFieldData) -> Result<(), PhysicsSceneError> {
    if !data.height_scale.is_finite() || !data.aabb_min.is_finite() || !data.aabb_max.is_finite() {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    let width =
        usize::try_from(data.width).map_err(|_| PhysicsSceneError::HeightFieldSizeOverflow)?;
    let length =
        usize::try_from(data.length).map_err(|_| PhysicsSceneError::HeightFieldSizeOverflow)?;
    let count = width
        .checked_mul(length)
        .ok_or(PhysicsSceneError::HeightFieldSizeOverflow)?;
    let sample_size = match data.version {
        0 => 1,
        1 => 2,
        2 => 4,
        value => return Err(PhysicsSceneError::UnsupportedHeightFieldVersion(value)),
    };
    let expected = count
        .checked_mul(sample_size)
        .ok_or(PhysicsSceneError::HeightFieldSizeOverflow)?;
    if data.samples.len() != expected {
        return Err(PhysicsSceneError::InvalidHeightFieldSamples {
            version: data.version,
            expected,
            actual: data.samples.len(),
        });
    }
    Ok(())
}

fn shape_transform_matrix(transform: &ShapeTransform) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(transform[0].x, transform[1].x, transform[2].x, 0.0),
        Vec4::new(transform[0].y, transform[1].y, transform[2].y, 0.0),
        Vec4::new(transform[0].z, transform[1].z, transform[2].z, 0.0),
        Vec4::new(transform[0].w, transform[1].w, transform[2].w, 1.0),
    )
}

fn normalized_axis(axis: Vec3) -> Result<Vec3, PhysicsSceneError> {
    if !axis.is_finite() || axis.length_squared() <= f32::EPSILON {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    Ok(axis.normalize())
}

/// New World treats an all-zero query-capsule axis as the Z axis. The native
/// query methods use the same near-zero test before normalizing the stored
/// vector; non-finite axes remain invalid.
fn query_capsule_axis(axis: Vec3) -> Result<Vec3, PhysicsSceneError> {
    if !axis.is_finite() {
        return Err(PhysicsSceneError::InvalidDimension);
    }
    if axis.length_squared() <= f32::EPSILON {
        return Ok(Vec3::Z);
    }
    Ok(axis)
}

const fn finite_positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use nw_reflected_types::types::{
        HitVolumeComponent, QueryShape as GeneratedQueryShape, QueryShapeCapsule,
    };

    #[test]
    fn zero_query_capsule_axis_uses_engine_z_axis_sentinel() {
        let generated = GeneratedQueryShape::Capsule(QueryShapeCapsule {
            height: 0.8,
            radius: 0.5,
            axis: Default::default(),
        });

        let projected = QueryShape::try_from(&generated).expect("project query capsule");

        assert!(matches!(
            projected,
            QueryShape::Capsule { axis, .. } if axis == Vec3::Z
        ));
    }

    #[test]
    fn hit_volume_source_preserves_zero_query_capsule_axis() {
        let generated = HitVolumeComponent {
            shape: Some(GeneratedQueryShape::Capsule(QueryShapeCapsule {
                height: 0.8,
                radius: 0.5,
                axis: Default::default(),
            })),
            ..Default::default()
        };

        let projected = HitVolume::try_from(&generated).expect("project hit volume");

        assert_eq!(
            projected.source["m_shape"]["m_axis"],
            serde_json::json!([0.0, 0.0, 0.0])
        );
        assert!(matches!(
            projected.shape,
            QueryShape::Capsule { axis, .. } if axis == Vec3::Z
        ));
    }

    #[test]
    fn query_shape_capsule_height_is_cylinder_segment_length() {
        // Leg-shaped authored values: height == diameter used to collapse to a
        // sphere under the incorrect total-length reading.
        let geometry = query_shape_geometry(&QueryShape::Capsule {
            height: 0.8,
            radius: 0.4,
            axis: Vec3::X,
        })
        .expect("capsule geometry")
        .expect("visual");

        let min_x = geometry
            .positions
            .iter()
            .map(|p| p.x)
            .fold(f32::INFINITY, f32::min);
        let max_x = geometry
            .positions
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max);
        // Endpoints at ±height/2 plus spherical caps of radius → total 0.8+0.8=1.6.
        assert!(
            (max_x - min_x - 1.6).abs() < 1e-3,
            "extent along axis {max_x}-{min_x}"
        );
    }

    #[test]
    fn query_shape_capsule_axis_converts_cry_z_to_gltf_y() {
        // Head hit volume is authored m_axis = Cry +Z. After cry_to_gltf that
        // becomes glTF +Y so a bone-parented capsule aligns with the joint frame.
        let converted = query_shape_to_gltf(&QueryShape::Capsule {
            height: 1.2,
            radius: 0.4,
            axis: Vec3::Z,
        });
        assert!(matches!(
            converted,
            QueryShape::Capsule { axis, .. } if axis.abs_diff_eq(Vec3::Y, 1e-5)
        ));
        assert!(
            crate::math::cry_to_gltf(Vec3::new(0.0, 0.0, 0.05))
                .abs_diff_eq(Vec3::new(0.0, 0.05, 0.0), 1e-5)
        );
    }
}
