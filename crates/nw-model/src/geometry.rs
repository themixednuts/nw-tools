//! Assemble a [`cry_chunk::CgfFile`] (+ its `.cgfheap` sidecar) into mesh
//! primitives: positions, normals, UVs, and indices per subset, in glTF space.
//!
//! Geometry lives in two places — inline `DataStream` chunks, or `DataRef` chunks
//! pointing into the heap (New World's common interleaved layout). Both are
//! resolved here, following nw-buddy's `convertPrimitive`.

use cry_chunk::{CgfFile, MeshChunk, MeshSubset, MeshSubsetBoneIds};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};
use serde::Serialize;

use crate::math;

// Stream kinds (outer index of `MeshChunk::stream_chunk_ids`).
const KIND_POSITIONS: usize = 0;
const KIND_NORMALS: usize = 1;
const KIND_TEXCOORDS: usize = 2;
const KIND_INDICES: usize = 5;
const KIND_TANGENTS: usize = 6;
const KIND_BONE_MAPPING: usize = 9;
const KIND_QTANGENTS: usize = 12;

/// One drawable primitive: a triangle list with optional vertex attributes.
#[derive(Debug, Clone)]
pub struct Primitive {
    pub positions: Vec<Vec3>,
    pub normals: Option<Vec<Vec3>>,
    pub uvs: Option<Vec<Vec2>>,
    pub indices: Vec<u32>,
    /// Per-vertex joint indices (into the skeleton), for skinned meshes.
    pub joints: Option<Vec<[u16; 4]>>,
    /// Per-vertex joint weights (normalized), for skinned meshes.
    pub weights: Option<Vec<[f32; 4]>>,
    /// Influences 5-8 from a 2× bone-mapping stream (glTF `JOINTS_1`), if present.
    pub joints2: Option<Vec<[u16; 4]>>,
    /// Weights for influences 5-8 (glTF `WEIGHTS_1`), if present.
    pub weights2: Option<Vec<[f32; 4]>>,
    /// Cry subset material slot (index into the material's sub-materials).
    pub material_id: i32,
}

/// A mesh: one or more primitives (one per non-skipped subset).
#[derive(Debug, Clone)]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
    /// Physicalization payloads authored on this same drawable source node.
    pub physics_data: Vec<MeshPhysicsData>,
    /// Semantic use of this geometry in the authored Cry graph.
    pub role: MeshRole,
    /// Skeleton index used by this mesh's JOINTS/WEIGHTS streams.
    pub skin: Option<usize>,
    /// Cry node LOD suffix (`$lodN`), when present.
    pub lod: Option<u32>,
    /// Whether this node is authored as a shadow-proxy mesh.
    pub shadow_proxy: bool,
    /// Optional CDF socket placement for rigid bone/face attachments.
    pub attachment: Option<MeshAttachment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MeshRole {
    #[default]
    Render,
    PhysicsProxy,
    ClothSimulation,
}

#[derive(Debug, Clone)]
pub struct MeshAttachment {
    /// Skeleton containing `bone_name`. `None` is character/root space.
    pub skeleton: Option<usize>,
    /// Bone parent for CA_BONE attachments. `None` is character/root space.
    pub bone_name: Option<String>,
    pub local: Mat4,
}

/// Authored non-render node retained from the Cry chunk graph.
#[derive(Debug, Clone)]
pub struct AuxiliaryNode {
    pub name: String,
    pub role: AuxiliaryNodeRole,
    pub object_chunk_id: i32,
    pub parent_chunk_id: i32,
    pub material_chunk_id: i32,
    pub transform: [f32; 16],
    pub properties: String,
    /// Every physicalization slot referenced by the source mesh. Cry stores
    /// each engine-specific physicalization blob separately from drawable mesh
    /// streams, so the model retains those bytes and their decoded metadata.
    pub physics_data: Vec<MeshPhysicsData>,
}

/// One `MeshPhysicsData` payload referenced by a Cry mesh physicalization slot.
#[derive(Debug, Clone)]
pub struct MeshPhysicsData {
    pub slot: usize,
    pub chunk_id: i32,
    pub flags: i32,
    pub tetrahedra_chunk_id: i32,
    pub physical_data: Vec<u8>,
    pub tetrahedra_data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxiliaryNodeRole {
    PhysicsProxy,
    Physicalized,
}

/// Placement of a complete skeleton graph, used by nested CDF attachments.
#[derive(Debug, Clone)]
pub struct SkeletonPlacement {
    /// Parent skeleton for a bone attachment. `None` places the graph in scene space.
    pub parent_skeleton: Option<usize>,
    /// Parent bone within `parent_skeleton`. `None` places the graph in scene space.
    pub bone_name: Option<String>,
    pub local: Mat4,
}

/// One joint of a [`Skeleton`].
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    /// Cry controller ID generated from the joint name; CAF controllers bind by this ID.
    pub controller_id: u32,
    /// Parent joint index (`None` for a root).
    pub parent: Option<usize>,
    /// Local transform relative to the parent, in glTF space.
    pub local: Mat4,
    /// Inverse bind matrix (world → bone), in glTF space.
    pub inverse_bind: Mat4,
}

/// A character skeleton from a CompiledBones chunk; joint order matches the
/// per-vertex bone indices in the mesh's bone-mapping stream.
#[derive(Debug, Clone)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
    /// Optional placement of this skeleton as a nested character attachment.
    pub placement: Option<SkeletonPlacement>,
}

impl Skeleton {
    #[must_use]
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones
            .iter()
            .position(|bone| bone.name.eq_ignore_ascii_case(name))
    }

    /// Character-space transform for a bone, composed from its local hierarchy.
    #[must_use]
    pub fn bone_world(&self, index: usize) -> Option<Mat4> {
        let mut current = index;
        let mut world = self.bones.get(current)?.local;
        for _ in 0..self.bones.len() {
            let Some(parent) = self.bones.get(current)?.parent else {
                return Some(world);
            };
            world = self.bones.get(parent)?.local * world;
            current = parent;
        }
        None
    }
}

/// A fully-resolved model ready to serialize to glTF.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    /// Every independent skeleton graph used by this model. Mesh `skin` indices
    /// and nested placements index this table.
    pub skeletons: Vec<Skeleton>,
    /// Collision/proxy nodes that are part of the source graph but are not
    /// renderable glTF meshes.
    pub auxiliary_nodes: Vec<AuxiliaryNode>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelBuildError {
    #[error("mesh node `{mesh}` references missing MeshSubsets chunk {chunk_id}")]
    MissingSubsets { mesh: String, chunk_id: i32 },
    #[error(
        "mesh node `{mesh}` physicalization slot {slot} references missing MeshPhysicsData chunk {chunk_id}"
    )]
    MissingPhysicsData {
        mesh: String,
        slot: usize,
        chunk_id: i32,
    },
    #[error("mesh node `{mesh}` subset {subset} has no readable position stream")]
    MissingPositions { mesh: String, subset: usize },
    #[error("mesh node `{mesh}` subset {subset} has no readable index stream")]
    MissingIndices { mesh: String, subset: usize },
    #[error("skinned mesh node `{mesh}` subset {subset} has no valid bone mapping")]
    MissingBoneMapping { mesh: String, subset: usize },
    #[error(
        "skinned mesh node `{mesh}` subset {subset} joint {joint} is outside skeleton size {skeleton_len}"
    )]
    JointOutOfRange {
        mesh: String,
        subset: usize,
        joint: u16,
        skeleton_len: usize,
    },
    #[error("CDF attachment targets missing skeleton bone `{bone_name}`")]
    MissingAttachmentBone { bone_name: String },
    #[error("model references missing skeleton index {index}")]
    MissingSkeleton { index: usize },
    #[error("attached skin bone `{bone}` does not exist in target skeleton {target}")]
    MissingSkinBone { bone: String, target: usize },
    #[error("material index {material_id} plus table offset {offset} exceeds Cry's i32 range")]
    MaterialIndexOverflow { material_id: i32, offset: usize },
}

/// Build a model from a parsed chunk file and its heap (`&[]` if none).
///
/// World transforms are baked into vertices (glTF nodes stay at the origin).
impl Model {
    /// Assemble every drawable mesh node from a parsed chunk graph.
    ///
    /// # Errors
    ///
    /// Fails when required geometry or skin streams cannot be resolved. Optional
    /// attributes (normal/UV/tangent) remain optional glTF attributes.
    pub fn try_from_cgf(cgf: &CgfFile<'_>, heap: &[u8]) -> Result<Self, ModelBuildError> {
        let skeletons = cgf
            .compiled_bones()
            .first()
            .map(build_skeleton)
            .into_iter()
            .collect::<Vec<_>>();
        let mut meshes = Vec::new();
        let mut auxiliary_nodes = Vec::new();
        for node in cgf.nodes().values() {
            let name = node.name.as_str();
            let Some(mesh) = cgf.meshes().get(&node.object_id) else {
                continue;
            };
            let is_physics_proxy = name.to_ascii_lowercase().starts_with("$physics_proxy");
            let physics_data = mesh
                .physics_data_chunk_ids
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, chunk_id)| *chunk_id > 0)
                .map(|(slot, chunk_id)| {
                    let payload = cgf.mesh_physics_data().get(&chunk_id).ok_or_else(|| {
                        ModelBuildError::MissingPhysicsData {
                            mesh: name.to_owned(),
                            slot,
                            chunk_id,
                        }
                    })?;
                    Ok(MeshPhysicsData {
                        slot,
                        chunk_id,
                        flags: payload.flags,
                        tetrahedra_chunk_id: payload.tetrahedra_chunk_id,
                        physical_data: payload.physical_data.to_vec(),
                        tetrahedra_data: payload.tetrahedra_data.to_vec(),
                    })
                })
                .collect::<Result<Vec<_>, ModelBuildError>>()?;
            let has_drawable_subsets = cgf.mesh_subsets().contains_key(&mesh.subsets_chunk_id);
            if !has_drawable_subsets && !physics_data.is_empty() {
                auxiliary_nodes.push(AuxiliaryNode {
                    name: name.to_owned(),
                    role: if is_physics_proxy {
                        AuxiliaryNodeRole::PhysicsProxy
                    } else {
                        AuxiliaryNodeRole::Physicalized
                    },
                    object_chunk_id: node.object_id,
                    parent_chunk_id: node.parent_id,
                    material_chunk_id: node.material_chunk_id,
                    transform: node.transform,
                    properties: node.properties.clone(),
                    physics_data,
                });
                continue;
            }
            let skinned = stream_id(mesh, KIND_BONE_MAPPING).is_some();
            // Skinned vertices live in bind/model space — the skin handles placement,
            // so we don't bake the node's world transform into them.
            let world = if skinned {
                Mat4::IDENTITY
            } else {
                node_world_matrix(cgf, node.parent_id, math::node_matrix(node.transform))
            };
            if let Some(mut out) = build_mesh(
                cgf,
                mesh,
                heap,
                &world,
                name,
                skinned,
                skeletons.first().map_or(0, |value| value.bones.len()),
            )? {
                out.physics_data = physics_data;
                if is_physics_proxy {
                    out.role = MeshRole::PhysicsProxy;
                }
                meshes.push(out);
            }
        }
        Ok(Self {
            meshes,
            skeletons,
            auxiliary_nodes,
        })
    }
}

/// Build a [`Skeleton`] from a CompiledBones chunk: glTF-space local transforms
/// (relative to parent) and inverse bind matrices.
pub(crate) fn build_skeleton(chunk: &cry_chunk::CompiledBonesChunk) -> Skeleton {
    let count = chunk.bones.len();
    let to_world: Vec<Mat4> = chunk
        .bones
        .iter()
        .map(|bone| math::cry_to_gltf_mat(math::matrix34(&bone.default_bone_to_world)))
        .collect();
    let to_bone: Vec<Mat4> = chunk
        .bones
        .iter()
        .map(|bone| math::cry_to_gltf_mat(math::matrix34(&bone.default_world_to_bone)))
        .collect();

    let bones = chunk
        .bones
        .iter()
        .enumerate()
        .map(|(index, bone)| {
            // `parent_offset == 0` marks a root; otherwise it's a signed delta.
            let parent = (bone.parent_offset != 0)
                .then(|| index as i32 + bone.parent_offset)
                .filter(|&p| p >= 0 && (p as usize) < count)
                .map(|p| p as usize);
            let local = match parent {
                None => to_world[index],
                Some(p) => to_bone[p] * to_world[index],
            };
            Bone {
                name: bone.bone_name.to_string(),
                controller_id: bone.controller_id,
                parent,
                local,
                inverse_bind: to_bone[index],
            }
        })
        .collect();
    Skeleton {
        bones,
        placement: None,
    }
}

impl Model {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.meshes.iter().all(|mesh| mesh.primitives.is_empty())
    }

    #[must_use]
    pub fn has_render_geometry(&self) -> bool {
        self.meshes
            .iter()
            .any(|mesh| mesh.role == MeshRole::Render && !mesh.primitives.is_empty())
    }

    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.meshes
            .iter()
            .flat_map(|mesh| &mesh.primitives)
            .map(|primitive| primitive.positions.len())
            .sum()
    }

    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.meshes
            .iter()
            .flat_map(|mesh| &mesh.primitives)
            .map(|primitive| primitive.indices.len() / 3)
            .sum()
    }

    #[must_use]
    pub fn primary_skeleton(&self) -> Option<&Skeleton> {
        self.skeletons.first()
    }

    pub fn set_primary_skeleton(&mut self, skeleton: Skeleton) {
        if self.skeletons.is_empty() {
            self.skeletons.push(skeleton);
        } else {
            self.skeletons[0] = skeleton;
        }
    }

    /// Merge a `.skin`/cloth binding onto an existing character skeleton. The
    /// attachment's CompiledBones graph may use a different joint order; bindings
    /// are remapped by controller ID/name to the receiving skeleton.
    pub fn append_skinned_geometry(
        &mut self,
        other: Self,
        target: usize,
    ) -> Result<(), ModelBuildError> {
        self.append_skinned_geometry_with_role(other, target, MeshRole::Render)
    }

    /// Merge the simulation `.skin` of a `CA_VCLOTH` attachment. It shares the
    /// character skeleton, but remains explicitly distinguishable from the
    /// attachment's render skin in glTF node metadata.
    pub fn append_cloth_simulation_geometry(
        &mut self,
        other: Self,
        target: usize,
    ) -> Result<(), ModelBuildError> {
        self.append_skinned_geometry_with_role(other, target, MeshRole::ClothSimulation)
    }

    fn append_skinned_geometry_with_role(
        &mut self,
        mut other: Self,
        target: usize,
        role: MeshRole,
    ) -> Result<(), ModelBuildError> {
        let target_skeleton = self
            .skeletons
            .get(target)
            .ok_or(ModelBuildError::MissingSkeleton { index: target })?;
        let joint_remap = other.skeletons.first().map(|source_skeleton| {
            source_skeleton
                .bones
                .iter()
                .map(|source_bone| {
                    target_skeleton
                        .bones
                        .iter()
                        .position(|target_bone| {
                            (source_bone.controller_id != 0
                                && target_bone.controller_id == source_bone.controller_id)
                                || target_bone.name.eq_ignore_ascii_case(&source_bone.name)
                        })
                        .and_then(|index| u16::try_from(index).ok())
                })
                .collect::<Vec<_>>()
        });
        for mesh in &mut other.meshes {
            mesh.role = role;
            if mesh.skin.is_some() {
                mesh.skin = Some(target);
            }
            for primitive in &mut mesh.primitives {
                let (Some(joints), Some(weights)) =
                    (&mut primitive.joints, primitive.weights.as_ref())
                else {
                    continue;
                };
                for (vertex_joints, vertex_weights) in joints.iter_mut().zip(weights) {
                    for influence in 0..4 {
                        if vertex_weights[influence] <= 0.0 {
                            vertex_joints[influence] = 0;
                            continue;
                        }
                        let source_index = usize::from(vertex_joints[influence]);
                        let mapped = if let Some(remap) = &joint_remap {
                            remap.get(source_index).copied().flatten().ok_or_else(|| {
                                ModelBuildError::MissingSkinBone {
                                    bone: other
                                        .skeletons
                                        .first()
                                        .and_then(|skeleton| skeleton.bones.get(source_index))
                                        .map_or_else(
                                            || format!("#{source_index}"),
                                            |bone| bone.name.clone(),
                                        ),
                                    target,
                                }
                            })?
                        } else {
                            if source_index >= target_skeleton.bones.len() {
                                return Err(ModelBuildError::JointOutOfRange {
                                    mesh: mesh.name.clone(),
                                    subset: 0,
                                    joint: vertex_joints[influence],
                                    skeleton_len: target_skeleton.bones.len(),
                                });
                            }
                            vertex_joints[influence]
                        };
                        vertex_joints[influence] = mapped;
                    }
                }
            }
        }
        self.meshes.append(&mut other.meshes);
        self.auxiliary_nodes.append(&mut other.auxiliary_nodes);
        Ok(())
    }

    /// Rebase every non-negative Cry subset material ID before merging asset-local
    /// material tables into one glTF material table.
    pub fn rebase_material_ids(&mut self, offset: usize) -> Result<(), ModelBuildError> {
        for primitive in self.meshes.iter_mut().flat_map(|mesh| &mut mesh.primitives) {
            if primitive.material_id < 0 {
                continue;
            }
            let material_id = primitive.material_id;
            primitive.material_id = usize::try_from(material_id)
                .ok()
                .and_then(|index| index.checked_add(offset))
                .and_then(|index| i32::try_from(index).ok())
                .ok_or(ModelBuildError::MaterialIndexOverflow {
                    material_id,
                    offset,
                })?;
        }
        Ok(())
    }

    /// Merge rigid attachment geometry and parent every emitted mesh node to a
    /// validated character bone.
    pub fn append_attached_geometry(
        &mut self,
        mut other: Self,
        skeleton: usize,
        bone_name: impl Into<String>,
        local: Mat4,
    ) -> Result<(), ModelBuildError> {
        let bone_name = bone_name.into();
        if self
            .skeletons
            .get(skeleton)
            .is_none_or(|skeleton| skeleton.bone_index(&bone_name).is_none())
        {
            return Err(ModelBuildError::MissingAttachmentBone { bone_name });
        }
        for mesh in &mut other.meshes {
            mesh.attachment = Some(MeshAttachment {
                skeleton: Some(skeleton),
                bone_name: Some(bone_name.clone()),
                local,
            });
        }
        self.meshes.append(&mut other.meshes);
        self.auxiliary_nodes.append(&mut other.auxiliary_nodes);
        Ok(())
    }

    /// Merge rigid geometry in character/root space (for example a statically
    /// representable CA_FACE attachment).
    pub fn append_root_geometry(&mut self, mut other: Self, local: Mat4) {
        for mesh in &mut other.meshes {
            mesh.attachment = Some(MeshAttachment {
                skeleton: None,
                bone_name: None,
                local,
            });
        }
        self.meshes.append(&mut other.meshes);
        self.auxiliary_nodes.append(&mut other.auxiliary_nodes);
    }

    pub fn attach_meshes_to_bone(
        &mut self,
        skeleton: usize,
        bone_name: impl Into<String>,
        local: Mat4,
    ) -> Result<(), ModelBuildError> {
        let bone_name = bone_name.into();
        if self
            .skeletons
            .get(skeleton)
            .is_none_or(|skeleton| skeleton.bone_index(&bone_name).is_none())
        {
            return Err(ModelBuildError::MissingAttachmentBone { bone_name });
        }
        for mesh in &mut self.meshes {
            mesh.attachment = Some(MeshAttachment {
                skeleton: Some(skeleton),
                bone_name: Some(bone_name.clone()),
                local,
            });
        }
        Ok(())
    }

    /// Merge a complete model graph and parent its primary skeleton to a bone in
    /// this model. Returns the skeleton-table offset assigned to the child.
    pub fn append_character_attachment(
        &mut self,
        mut other: Self,
        parent_skeleton: usize,
        bone_name: impl Into<String>,
        local: Mat4,
    ) -> Result<usize, ModelBuildError> {
        let bone_name = bone_name.into();
        if self
            .skeletons
            .get(parent_skeleton)
            .is_none_or(|skeleton| skeleton.bone_index(&bone_name).is_none())
        {
            return Err(ModelBuildError::MissingAttachmentBone { bone_name });
        }
        if other.skeletons.is_empty() {
            return Err(ModelBuildError::MissingSkeleton { index: 0 });
        }
        let offset = self.skeletons.len();
        remap_skeleton_indices(&mut other, offset);
        other.skeletons[0].placement = Some(SkeletonPlacement {
            parent_skeleton: Some(parent_skeleton),
            bone_name: Some(bone_name),
            local,
        });
        self.meshes.append(&mut other.meshes);
        self.skeletons.append(&mut other.skeletons);
        self.auxiliary_nodes.append(&mut other.auxiliary_nodes);
        Ok(offset)
    }

    /// Merge a complete model graph in scene/root space. Returns its skeleton offset.
    pub fn append_character_root(
        &mut self,
        mut other: Self,
        local: Mat4,
    ) -> Result<usize, ModelBuildError> {
        if other.skeletons.is_empty() {
            return Err(ModelBuildError::MissingSkeleton { index: 0 });
        }
        let offset = self.skeletons.len();
        remap_skeleton_indices(&mut other, offset);
        other.skeletons[0].placement = Some(SkeletonPlacement {
            parent_skeleton: None,
            bone_name: None,
            local,
        });
        self.meshes.append(&mut other.meshes);
        self.skeletons.append(&mut other.skeletons);
        self.auxiliary_nodes.append(&mut other.auxiliary_nodes);
        Ok(offset)
    }
}

fn remap_skeleton_indices(model: &mut Model, offset: usize) {
    for mesh in &mut model.meshes {
        if let Some(skin) = &mut mesh.skin {
            *skin += offset;
        }
        if let Some(skeleton) = mesh
            .attachment
            .as_mut()
            .and_then(|attachment| attachment.skeleton.as_mut())
        {
            *skeleton += offset;
        }
    }
    for skeleton in &mut model.skeletons {
        if let Some(parent) = skeleton
            .placement
            .as_mut()
            .and_then(|placement| placement.parent_skeleton.as_mut())
        {
            *parent += offset;
        }
    }
}

/// Compose a node's world matrix by walking up the parent chain. Each node's
/// matrix maps local → parent space, so `world = parentₙ · … · parent₁ · local`.
fn node_world_matrix(cgf: &CgfFile, parent_id: i32, local: Mat4) -> Mat4 {
    let mut world = local;
    let mut current = parent_id;
    // Guard against cycles with a generous bound.
    for _ in 0..256 {
        let Some(parent) = cgf.nodes().get(&current) else {
            break;
        };
        world = math::node_matrix(parent.transform) * world;
        current = parent.parent_id;
    }
    world
}

fn build_mesh(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    heap: &[u8],
    world: &Mat4,
    name: &str,
    skinned: bool,
    skeleton_len: usize,
) -> Result<Option<Mesh>, ModelBuildError> {
    let subsets = cgf
        .mesh_subsets()
        .get(&mesh.subsets_chunk_id)
        .ok_or_else(|| ModelBuildError::MissingSubsets {
            mesh: name.to_owned(),
            chunk_id: mesh.subsets_chunk_id,
        })?;
    let mut primitives = Vec::new();
    let options = PrimitiveBuildOptions {
        world,
        mesh_name: name,
        skinned,
        skeleton_len,
    };
    for (subset_index, subset) in subsets.subsets.iter().enumerate() {
        primitives.push(build_primitive(
            cgf,
            mesh,
            subset,
            subsets.bone_ids.get(subset_index),
            heap,
            subset_index,
            &options,
        )?);
    }
    if primitives.is_empty() {
        return Ok(None);
    }
    Ok(Some(Mesh {
        name: name.to_string(),
        primitives,
        physics_data: Vec::new(),
        role: MeshRole::Render,
        skin: skinned.then_some(0),
        lod: node_lod(name),
        shadow_proxy: name.to_ascii_lowercase().contains("shadowproxy"),
        attachment: None,
    }))
}

struct PrimitiveBuildOptions<'a> {
    world: &'a Mat4,
    mesh_name: &'a str,
    skinned: bool,
    skeleton_len: usize,
}

fn build_primitive(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    subset: &MeshSubset,
    bone_palette: Option<&MeshSubsetBoneIds>,
    heap: &[u8],
    subset_index: usize,
    options: &PrimitiveBuildOptions<'_>,
) -> Result<Primitive, ModelBuildError> {
    let positions = read_vec3_stream(
        cgf,
        mesh,
        KIND_POSITIONS,
        subset,
        heap,
        Vec3Transform::Point(options.world),
        // Positions loader accepts elemSize 12 or 8 (half x3).
        true,
    )
    .ok_or_else(|| ModelBuildError::MissingPositions {
        mesh: options.mesh_name.to_owned(),
        subset: subset_index,
    })?;
    let indices =
        read_indices(cgf, mesh, subset, heap).ok_or_else(|| ModelBuildError::MissingIndices {
            mesh: options.mesh_name.to_owned(),
            subset: subset_index,
        })?;
    let uvs = read_uv_stream(cgf, mesh, KIND_TEXCOORDS, subset, heap);
    let normal_transform = Mat3::from_mat4(*options.world).inverse().transpose();
    let normals = read_vec3_stream(
        cgf,
        mesh,
        KIND_NORMALS,
        subset,
        heap,
        Vec3Transform::Direction(normal_transform),
        // Normals loader (FUN_7ff606c3b500 @ 0x7ff606c3b500) accepts elemSize 12 only.
        false,
    )
    .or_else(|| {
        derive_normals_from_tangents(cgf, mesh, subset, heap)
            .map(|normals| transform_gltf_normals(normals, normal_transform))
    })
    .or_else(|| {
        derive_normals_from_qtangents(cgf, mesh, subset, heap)
            .map(|normals| transform_gltf_normals(normals, normal_transform))
    });
    let (joints, weights, joints2, weights2) = if options.skinned {
        let mapping =
            read_bone_mapping(cgf, mesh, subset, bone_palette, heap).ok_or_else(|| {
                ModelBuildError::MissingBoneMapping {
                    mesh: options.mesh_name.to_owned(),
                    subset: subset_index,
                }
            })?;
        let out_of_range = mapping
            .joints
            .iter()
            .chain(mapping.joints2.iter().flatten())
            .flatten()
            .copied()
            .find(|joint| usize::from(*joint) >= options.skeleton_len);
        if let Some(joint) = out_of_range {
            return Err(ModelBuildError::JointOutOfRange {
                mesh: options.mesh_name.to_owned(),
                subset: subset_index,
                joint,
                skeleton_len: options.skeleton_len,
            });
        }
        (
            Some(mapping.joints),
            Some(mapping.weights),
            mapping.joints2,
            mapping.weights2,
        )
    } else {
        (None, None, None, None)
    };

    Ok(Primitive {
        positions,
        normals,
        uvs,
        indices,
        joints,
        weights,
        joints2,
        weights2,
        material_id: subset.material_id,
    })
}

/// Derive per-vertex normals from a packed qtangent stream (stride 8: 4× signed
/// int16 quaternion). The normal is the quaternion's rotated Z axis.
fn derive_normals_from_qtangents(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    subset: &MeshSubset,
    heap: &[u8],
) -> Option<Vec<Vec3>> {
    let id = stream_id(mesh, KIND_QTANGENTS)?;
    let count = subset.num_vertices;
    let (data, stride, base): (&[u8], usize, usize) =
        if let Some(stream) = cgf.data_streams().get(&id) {
            let stride = stream.element_size;
            (
                stream.data,
                stride,
                stride.checked_mul(subset.first_vertex)?,
            )
        } else {
            let data_ref = cgf.data_refs().get(&id)?;
            let stride = data_ref.stride;
            (heap, stride, data_ref.offset + stride * subset.first_vertex)
        };
    if stride < 8 {
        return None;
    }

    let factor = 1.0 / 32767.0;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = base + i * stride;
        let quat = Quat::from_xyzw(
            f32::from(i16_at(data, o)?) * factor,
            f32::from(i16_at(data, o + 2)?) * factor,
            f32::from(i16_at(data, o + 4)?) * factor,
            f32::from(i16_at(data, o + 6)?) * factor,
        )
        .normalize();
        // The qtangent's rotated Z axis is the normal (handedness in w's sign).
        let mut normal = math::cry_to_gltf(Mat3::from_quat(quat).z_axis);
        if quat.w < 0.0 {
            normal = -normal;
        }
        out.push(normal);
    }
    Some(out)
}

/// One influence set for a subset: joint indices (×4) and normalized weights (×4).
type InfluenceSet = (Vec<[u16; 4]>, Vec<[f32; 4]>);

/// Per-vertex skin binding for a subset: primary joint indices/weights (×4)
/// plus an optional second influence set (influences 5-8) surfaced from a 2×
/// bone-mapping stream.
struct BoneMapping {
    joints: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
    joints2: Option<Vec<[u16; 4]>>,
    weights2: Option<Vec<[f32; 4]>>,
}

/// Read the per-vertex bone mapping (joint indices + weights) for a subset.
/// Stride 8 = 4×u8 joints + 4×u8 weights; stride 12 = 4×u16 joints + 4×u8 weights.
fn read_bone_mapping(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    subset: &MeshSubset,
    bone_palette: Option<&MeshSubsetBoneIds>,
    heap: &[u8],
) -> Option<BoneMapping> {
    let id = stream_id(mesh, KIND_BONE_MAPPING)?;
    let mesh_vertices = usize::try_from(mesh.vertex_count).ok()?;
    let (data, stride, start, entry_count): (&[u8], usize, usize, usize) =
        if let Some(stream) = cgf.data_streams().get(&id) {
            (stream.data, stream.element_size, 0, stream.element_count)
        } else {
            let data_ref = cgf.data_refs().get(&id)?;
            let stride = data_ref.stride;
            (
                heap,
                stride,
                data_ref.offset,
                data_ref.size.checked_div(stride)?,
            )
        };
    decode_bone_mapping(
        data,
        stride,
        start,
        entry_count,
        mesh_vertices,
        subset,
        bone_palette,
    )
}

/// Decode a bone-mapping stream against the engine's entry-count rule.
///
/// The loader @ 0x7ff606c39640 accepts a stream with either exactly
/// vertex-count entries or exactly 2× vertex-count ("wrong # vertices %d
/// (expected %d or %d)"); the second half is the 5th-8th influence set. Any
/// other count is rejected.
fn decode_bone_mapping(
    data: &[u8],
    stride: usize,
    start: usize,
    entry_count: usize,
    mesh_vertices: usize,
    subset: &MeshSubset,
    bone_palette: Option<&MeshSubsetBoneIds>,
) -> Option<BoneMapping> {
    let has_second_set = if entry_count == mesh_vertices {
        false
    } else if entry_count == mesh_vertices.checked_mul(2)? {
        true
    } else {
        return None;
    };

    let read_set = |half: usize| -> Option<InfluenceSet> {
        let mut joints = Vec::with_capacity(subset.num_vertices);
        let mut weights = Vec::with_capacity(subset.num_vertices);
        for i in 0..subset.num_vertices {
            let vertex = half
                .checked_mul(mesh_vertices)?
                .checked_add(subset.first_vertex)?
                .checked_add(i)?;
            let o = start.checked_add(vertex.checked_mul(stride)?)?;
            let (joint, raw_weights) = read_bone_entry(data, o, stride, bone_palette)?;
            joints.push(joint);
            weights.push(normalize_weights(raw_weights));
        }
        Some((joints, weights))
    };

    let (joints, weights) = read_set(0)?;
    let (joints2, weights2) = if has_second_set {
        let (joints2, weights2) = read_set(1)?;
        (Some(joints2), Some(weights2))
    } else {
        (None, None)
    };
    Some(BoneMapping {
        joints,
        weights,
        joints2,
        weights2,
    })
}

/// Read one bone-mapping entry (4 joints + 4 raw u8 weights) at a byte offset.
fn read_bone_entry(
    data: &[u8],
    o: usize,
    stride: usize,
    bone_palette: Option<&MeshSubsetBoneIds>,
) -> Option<([u16; 4], [u8; 4])> {
    match stride {
        8 => Some((
            map_subset_bone_ids(
                [
                    u8_at(data, o)?,
                    u8_at(data, o + 1)?,
                    u8_at(data, o + 2)?,
                    u8_at(data, o + 3)?,
                ],
                bone_palette?,
            )?,
            [
                u8_at(data, o + 4)?,
                u8_at(data, o + 5)?,
                u8_at(data, o + 6)?,
                u8_at(data, o + 7)?,
            ],
        )),
        12 => Some((
            [
                u16_at(data, o)?,
                u16_at(data, o + 2)?,
                u16_at(data, o + 4)?,
                u16_at(data, o + 6)?,
            ],
            [
                u8_at(data, o + 8)?,
                u8_at(data, o + 9)?,
                u8_at(data, o + 10)?,
                u8_at(data, o + 11)?,
            ],
        )),
        _ => None,
    }
}

/// Maps the compact per-subset u8 indices used by New World's stride-8 stream
/// through the parallel MeshSubsets bone-remapping table.
fn map_subset_bone_ids(local: [u8; 4], palette: &MeshSubsetBoneIds) -> Option<[u16; 4]> {
    let ids = palette.ids.get(..palette.count)?;
    Some([
        *ids.get(usize::from(local[0]))?,
        *ids.get(usize::from(local[1]))?,
        *ids.get(usize::from(local[2]))?,
        *ids.get(usize::from(local[3]))?,
    ])
}

fn node_lod(name: &str) -> Option<u32> {
    let lowercase = name.to_ascii_lowercase();
    let suffix = lowercase.split("$lod").nth(1)?;
    let digits = suffix
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

/// Normalize Cry u8 bone weights (summing to ~255) to floats summing to 1.
fn normalize_weights(raw: [u8; 4]) -> [f32; 4] {
    let sum = f32::from(raw[0]) + f32::from(raw[1]) + f32::from(raw[2]) + f32::from(raw[3]);
    if sum > 0.0 {
        raw.map(|w| f32::from(w) / sum)
    } else {
        [1.0, 0.0, 0.0, 0.0]
    }
}

/// The first non-zero stream/ref chunk id for a kind.
fn stream_id(mesh: &MeshChunk, kind: usize) -> Option<i32> {
    mesh.stream_chunk_ids
        .get(kind)?
        .iter()
        .copied()
        .find(|&id| id != 0)
}

/// Read a per-vertex `[f32;3]` attribute (positions or normals). If `world` is
/// set, points are transformed to world space; all values are converted to glTF
/// axes.
#[derive(Clone, Copy)]
enum Vec3Transform<'a> {
    Point(&'a Mat4),
    Direction(Mat3),
}

fn read_vec3_stream(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    kind: usize,
    subset: &MeshSubset,
    heap: &[u8],
    transform: Vec3Transform<'_>,
    allow_half: bool,
) -> Option<Vec<Vec3>> {
    let id = stream_id(mesh, kind)?;
    let count = subset.num_vertices;
    let mut out = Vec::with_capacity(count);

    let push = |out: &mut Vec<Vec3>, raw: [f32; 3]| {
        let v = Vec3::from_array(raw);
        // A point (position) is placed by the world matrix; a direction (normal)
        // is only rotated. Distinguish by whether a matrix was supplied.
        let v = match transform {
            Vec3Transform::Point(matrix) => matrix.transform_point3(v),
            Vec3Transform::Direction(matrix) => (matrix * v).normalize_or_zero(),
        };
        out.push(math::cry_to_gltf(v));
    };

    if let Some(stream) = cgf.data_streams().get(&id) {
        let size = stream.element_size;
        let base = size.checked_mul(subset.first_vertex)?;
        for i in 0..count {
            let o = base + i * size;
            let raw = decode_stream_vec3(stream.data, o, size, allow_half)?;
            push(&mut out, raw);
        }
    } else {
        let data_ref = cgf.data_refs().get(&id)?;
        let stride = data_ref.stride;
        let base = data_ref.offset + stride * subset.first_vertex;
        for i in 0..count {
            let o = base + i * stride;
            let raw = match stride {
                16 => [
                    half_at(heap, o)?,
                    half_at(heap, o + 2)?,
                    half_at(heap, o + 4)?,
                ],
                24 => [f32_at(heap, o)?, f32_at(heap, o + 4)?, f32_at(heap, o + 8)?],
                _ => return None,
            };
            push(&mut out, raw);
        }
    }
    Some(out)
}

/// Decode one inline `DataStream` vec3 element. `allow_half` gates the 8-byte
/// half x3 layout: the positions loader accepts elemSize 12 or 8, but the
/// normals loader (FUN_7ff606c3b500 @ 0x7ff606c3b500) accepts elemSize 12 only.
fn decode_stream_vec3(
    data: &[u8],
    offset: usize,
    size: usize,
    allow_half: bool,
) -> Option<[f32; 3]> {
    match size {
        12 => Some([
            f32_at(data, offset)?,
            f32_at(data, offset + 4)?,
            f32_at(data, offset + 8)?,
        ]),
        8 if allow_half => Some([
            half_at(data, offset)?,
            half_at(data, offset + 2)?,
            half_at(data, offset + 4)?,
        ]),
        _ => None,
    }
}

fn transform_gltf_normals(normals: Vec<Vec3>, cry_normal_matrix: Mat3) -> Vec<Vec3> {
    // Tangent-derived normals have already had the Cry→glTF axis conversion.
    // Convert back, apply the Cry-space inverse transpose, and convert once more.
    normals
        .into_iter()
        .map(|normal| {
            let cry = math::cry_to_gltf(normal);
            math::cry_to_gltf((cry_normal_matrix * cry).normalize_or_zero())
        })
        .collect()
}

/// Read UVs (the texcoord channel sits after position[+color] in interleaved refs).
fn read_uv_stream(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    kind: usize,
    subset: &MeshSubset,
    heap: &[u8],
) -> Option<Vec<Vec2>> {
    let id = stream_id(mesh, kind)?;
    let count = subset.num_vertices;
    let mut out = Vec::with_capacity(count);

    if let Some(stream) = cgf.data_streams().get(&id) {
        let size = stream.element_size;
        let base = size.checked_mul(subset.first_vertex)?;
        for i in 0..count {
            let o = base + i * size;
            let uv = match size {
                8 => Vec2::new(f32_at(stream.data, o)?, f32_at(stream.data, o + 4)?),
                _ => return None,
            };
            out.push(uv);
        }
    } else {
        let data_ref = cgf.data_refs().get(&id)?;
        let stride = data_ref.stride;
        let base = data_ref.offset + stride * subset.first_vertex;
        for i in 0..count {
            let o = base + i * stride;
            let uv = match stride {
                16 => Vec2::new(half_at(heap, o + 12)?, half_at(heap, o + 14)?),
                24 => Vec2::new(f32_at(heap, o + 16)?, f32_at(heap, o + 20)?),
                _ => return None,
            };
            out.push(uv);
        }
    }
    Some(out)
}

/// Read the subset's index list, localized to its vertex range.
fn read_indices(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    subset: &MeshSubset,
    heap: &[u8],
) -> Option<Vec<u32>> {
    let id = stream_id(mesh, KIND_INDICES)?;
    let count = subset.num_indices;
    let first_vertex = subset.first_vertex as u32;
    let mut out = Vec::with_capacity(count);

    let (data, stride, base): (&[u8], usize, usize) =
        if let Some(stream) = cgf.data_streams().get(&id) {
            let stride = stream.element_size;
            (stream.data, stride, stride.checked_mul(subset.first_index)?)
        } else {
            let data_ref = cgf.data_refs().get(&id)?;
            let stride = data_ref.stride;
            (heap, stride, data_ref.offset + stride * subset.first_index)
        };

    for i in 0..count {
        let o = base + i * stride;
        let value = match stride {
            2 => u32::from(u16_at(data, o)?),
            4 => u32_at(data, o)?,
            _ => return None,
        };
        out.push(value.wrapping_sub(first_vertex));
    }
    Some(out)
}

/// Derive per-vertex normals from a tangent-frame ref stream (stride 16: signed
/// int16 normal + tangent), as nw-buddy does when there is no explicit normal.
fn derive_normals_from_tangents(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    subset: &MeshSubset,
    heap: &[u8],
) -> Option<Vec<Vec3>> {
    let id = stream_id(mesh, KIND_TANGENTS)?;
    let data_ref = cgf.data_refs().get(&id)?;
    if data_ref.stride != 16 {
        return None;
    }
    let count = subset.num_vertices;
    let base = data_ref.offset + data_ref.stride * subset.first_vertex;
    let factor = 1.0 / 32767.0;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = base + i * 16;
        let tw = f32::from(i16_at(heap, o + 6)?) * factor;
        let tangent = math::cry_to_gltf(Vec3::new(
            f32::from(i16_at(heap, o)?) * factor,
            f32::from(i16_at(heap, o + 2)?) * factor,
            f32::from(i16_at(heap, o + 4)?) * factor,
        ));
        let bitangent = math::cry_to_gltf(Vec3::new(
            f32::from(i16_at(heap, o + 8)?) * factor,
            f32::from(i16_at(heap, o + 10)?) * factor,
            f32::from(i16_at(heap, o + 12)?) * factor,
        ));
        let mut normal = tangent.cross(bitangent).normalize_or_zero();
        if tw < 0.0 {
            normal = -normal;
        }
        out.push(normal);
    }
    Some(out)
}

fn slice_at(data: &[u8], offset: usize, len: usize) -> Option<&[u8]> {
    data.get(offset..offset + len)
}

fn u8_at(data: &[u8], offset: usize) -> Option<u8> {
    data.get(offset).copied()
}

fn u16_at(data: &[u8], offset: usize) -> Option<u16> {
    let b = slice_at(data, offset, 2)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

fn i16_at(data: &[u8], offset: usize) -> Option<i16> {
    Some(u16_at(data, offset)? as i16)
}

fn u32_at(data: &[u8], offset: usize) -> Option<u32> {
    let b = slice_at(data, offset, 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn f32_at(data: &[u8], offset: usize) -> Option<f32> {
    Some(f32::from_bits(u32_at(data, offset)?))
}

fn half_at(data: &[u8], offset: usize) -> Option<f32> {
    Some(math::half_to_f32(u16_at(data, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stride_eight_joint_indices_use_subset_palette() {
        let mut ids = [0_u16; 128];
        ids[..4].copy_from_slice(&[21, 7, 42, 3]);
        let palette = MeshSubsetBoneIds { count: 4, ids };

        assert_eq!(
            map_subset_bone_ids([2, 0, 3, 1], &palette),
            Some([42, 21, 3, 7])
        );
        assert_eq!(map_subset_bone_ids([4, 0, 0, 0], &palette), None);
    }

    #[test]
    fn extracts_cry_lod_suffix() {
        assert_eq!(node_lod("body$lod2"), Some(2));
        assert_eq!(node_lod("body$LOD12_extra"), Some(12));
        assert_eq!(node_lod("body"), None);
    }

    fn subset(first_vertex: usize, num_vertices: usize) -> MeshSubset {
        MeshSubset {
            first_index: 0,
            num_indices: 0,
            first_vertex,
            num_vertices,
            material_id: 0,
            radius: 0.0,
            center: [0.0; 3],
        }
    }

    fn palette_1234() -> MeshSubsetBoneIds {
        let mut ids = [0_u16; 128];
        ids[..4].copy_from_slice(&[10, 11, 12, 13]);
        MeshSubsetBoneIds { count: 4, ids }
    }

    #[test]
    fn double_bone_mapping_stream_surfaces_second_influence_set() {
        // Loader @ 0x7ff606c39640 accepts 2× vertex-count entries; the second
        // half is the 5th-8th influence set (stride-8, palette-remapped).
        let palette = palette_1234();
        // mesh_vertices = 2, entry_count = 4 (= 2×): influences 1-4 then 5-8.
        let data: Vec<u8> = vec![
            0, 1, 2, 3, 255, 0, 0, 0, // v0 influences 1-4
            1, 0, 0, 0, 128, 127, 0, 0, // v1 influences 1-4
            2, 3, 0, 1, 255, 0, 0, 0, // v0 influences 5-8
            3, 2, 1, 0, 0, 0, 0, 0, // v1 influences 5-8 (zero weights)
        ];

        let mapping =
            decode_bone_mapping(&data, 8, 0, 4, 2, &subset(0, 2), Some(&palette)).unwrap();

        assert_eq!(mapping.joints, [[10, 11, 12, 13], [11, 10, 10, 10]]);
        assert_eq!(
            mapping.joints2,
            Some(vec![[12, 13, 10, 11], [13, 12, 11, 10]])
        );
        assert_eq!(mapping.weights[0], [1.0, 0.0, 0.0, 0.0]);
        // All-zero weights normalize to bone-0, same as the primary set.
        assert_eq!(mapping.weights2.unwrap()[1], [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn single_bone_mapping_stream_has_no_second_set() {
        let palette = palette_1234();
        let data: Vec<u8> = vec![
            0, 1, 2, 3, 255, 0, 0, 0, // v0
            1, 0, 0, 0, 255, 0, 0, 0, // v1
        ];

        let mapping =
            decode_bone_mapping(&data, 8, 0, 2, 2, &subset(0, 2), Some(&palette)).unwrap();

        assert_eq!(mapping.joints.len(), 2);
        assert!(mapping.joints2.is_none());
        assert!(mapping.weights2.is_none());
    }

    #[test]
    fn bone_mapping_stream_rejects_other_multiples() {
        // 3× vertex-count is neither the single nor the double form.
        let palette = palette_1234();
        let data = vec![0_u8; 8 * 6];

        assert!(decode_bone_mapping(&data, 8, 0, 6, 2, &subset(0, 2), Some(&palette)).is_none());
    }

    #[test]
    fn normals_stream_rejects_half_precision_element() {
        // Six bytes: enough for a half x3 read; the decoded value is irrelevant
        // to the accept/reject decision.
        let data = [0x00, 0x3c, 0x00, 0x00, 0x00, 0x00];
        // Positions accept the 8-byte half x3 layout; normals do not.
        assert!(decode_stream_vec3(&data, 0, 8, true).is_some());
        assert!(decode_stream_vec3(&data, 0, 8, false).is_none());
    }

    #[test]
    fn vec3_stream_accepts_full_float_element_for_both() {
        let mut data = Vec::new();
        for value in [1.0_f32, 2.0, 3.0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(
            decode_stream_vec3(&data, 0, 12, true),
            Some([1.0, 2.0, 3.0])
        );
        assert_eq!(
            decode_stream_vec3(&data, 0, 12, false),
            Some([1.0, 2.0, 3.0])
        );
    }
}
