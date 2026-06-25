//! Assemble a [`cry_chunk::CgfFile`] (+ its `.cgfheap` sidecar) into mesh
//! primitives: positions, normals, UVs, and indices per subset, in glTF space.
//!
//! Geometry lives in two places — inline `DataStream` chunks, or `DataRef` chunks
//! pointing into the heap (New World's common interleaved layout). Both are
//! resolved here, following nw-buddy's `convertPrimitive`.

use cry_chunk::{CgfFile, MeshChunk, MeshSubset};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};

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
    /// Cry subset material slot (index into the material's sub-materials).
    pub material_id: i32,
}

/// A mesh: one or more primitives (one per non-skipped subset).
#[derive(Debug, Clone)]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
    /// Whether the mesh's primitives carry skin weights (bound to the skeleton).
    pub skinned: bool,
}

/// One joint of a [`Skeleton`].
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
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
}

/// A fully-resolved model ready to serialize to glTF.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    /// The skeleton, if this is a skinned model.
    pub skeleton: Option<Skeleton>,
}

/// Build a model from a parsed chunk file and its heap (`&[]` if none).
///
/// World transforms are baked into vertices (glTF nodes stay at the origin), which
/// is correct and avoids matrix-convention pitfalls; `$lod` and `shadowproxy`
/// nodes are skipped.
impl From<(&CgfFile<'_>, &[u8])> for Model {
    fn from((cgf, heap): (&CgfFile<'_>, &[u8])) -> Self {
        let skeleton = cgf.compiled_bones().first().map(build_skeleton);
        let skinned = skeleton.is_some();
        let mut meshes = Vec::new();
        for node in cgf.nodes().values() {
            let name = node.name.as_str();
            if name.contains("$lod") || name.to_ascii_lowercase().contains("shadowproxy") {
                continue;
            }
            let Some(mesh) = cgf.meshes().get(&node.object_id) else {
                continue;
            };
            // Skinned vertices live in bind/model space — the skin handles placement,
            // so we don't bake the node's world transform into them.
            let world = if skinned {
                Mat4::IDENTITY
            } else {
                node_world_matrix(cgf, node.parent_id, math::node_matrix(node.transform))
            };
            if let Some(out) = build_mesh(cgf, mesh, heap, &world, name, skinned) {
                meshes.push(out);
            }
        }
        Self { meshes, skeleton }
    }
}

/// Build a [`Skeleton`] from a CompiledBones chunk: glTF-space local transforms
/// (relative to parent) and inverse bind matrices.
fn build_skeleton(chunk: &cry_chunk::CompiledBonesChunk) -> Skeleton {
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
                parent,
                local,
                inverse_bind: to_bone[index],
            }
        })
        .collect();
    Skeleton { bones }
}

impl Model {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.meshes.iter().all(|mesh| mesh.primitives.is_empty())
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
) -> Option<Mesh> {
    let subsets = cgf.mesh_subsets().get(&mesh.subsets_chunk_id)?;
    let mut primitives = Vec::new();
    for subset in &subsets.subsets {
        if let Some(primitive) = build_primitive(cgf, mesh, subset, heap, world, skinned) {
            primitives.push(primitive);
        }
    }
    if primitives.is_empty() {
        return None;
    }
    Some(Mesh {
        name: name.to_string(),
        primitives,
        skinned,
    })
}

fn build_primitive(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    subset: &MeshSubset,
    heap: &[u8],
    world: &Mat4,
    skinned: bool,
) -> Option<Primitive> {
    let positions = read_vec3_stream(cgf, mesh, KIND_POSITIONS, subset, heap, Some(world))?;
    let indices = read_indices(cgf, mesh, subset, heap)?;
    let uvs = read_uv_stream(cgf, mesh, KIND_TEXCOORDS, subset, heap);
    let normals = read_vec3_stream(cgf, mesh, KIND_NORMALS, subset, heap, None)
        .or_else(|| derive_normals_from_tangents(cgf, mesh, subset, heap))
        .or_else(|| derive_normals_from_qtangents(cgf, mesh, subset, heap));
    let (joints, weights) = if skinned {
        read_bone_mapping(cgf, mesh, subset, heap).unzip()
    } else {
        (None, None)
    };

    Some(Primitive {
        positions,
        normals,
        uvs,
        indices,
        joints,
        weights,
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
            (stream.data, stride, stride.checked_mul(subset.first_vertex)?)
        } else if let Some(data_ref) = cgf.data_refs().get(&id) {
            let stride = data_ref.stride;
            (heap, stride, data_ref.offset + stride * subset.first_vertex)
        } else {
            return None;
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

/// Per-vertex skin binding: joint indices (×4) and normalized weights (×4).
type BoneMapping = (Vec<[u16; 4]>, Vec<[f32; 4]>);

/// Read the per-vertex bone mapping (joint indices + weights) for a subset.
/// Stride 8 = 4×u8 joints + 4×u8 weights; stride 12 = 4×u16 joints + 4×u8 weights.
fn read_bone_mapping(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    subset: &MeshSubset,
    heap: &[u8],
) -> Option<BoneMapping> {
    let id = stream_id(mesh, KIND_BONE_MAPPING)?;
    let count = subset.num_vertices;
    let (data, stride, base): (&[u8], usize, usize) =
        if let Some(stream) = cgf.data_streams().get(&id) {
            let stride = stream.element_size;
            (stream.data, stride, stride.checked_mul(subset.first_vertex)?)
        } else if let Some(data_ref) = cgf.data_refs().get(&id) {
            let stride = data_ref.stride;
            (heap, stride, data_ref.offset + stride * subset.first_vertex)
        } else {
            return None;
        };

    let mut joints = Vec::with_capacity(count);
    let mut weights = Vec::with_capacity(count);
    for i in 0..count {
        let o = base + i * stride;
        let (joint, raw_weights) = match stride {
            8 => (
                [
                    u16::from(u8_at(data, o)?),
                    u16::from(u8_at(data, o + 1)?),
                    u16::from(u8_at(data, o + 2)?),
                    u16::from(u8_at(data, o + 3)?),
                ],
                [
                    u8_at(data, o + 4)?,
                    u8_at(data, o + 5)?,
                    u8_at(data, o + 6)?,
                    u8_at(data, o + 7)?,
                ],
            ),
            12 => (
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
            ),
            _ => return None,
        };
        joints.push(joint);
        weights.push(normalize_weights(raw_weights));
    }
    Some((joints, weights))
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
fn read_vec3_stream(
    cgf: &CgfFile,
    mesh: &MeshChunk,
    kind: usize,
    subset: &MeshSubset,
    heap: &[u8],
    world: Option<&Mat4>,
) -> Option<Vec<Vec3>> {
    let id = stream_id(mesh, kind)?;
    let count = subset.num_vertices;
    let mut out = Vec::with_capacity(count);

    let push = |out: &mut Vec<Vec3>, raw: [f32; 3]| {
        let v = Vec3::from_array(raw);
        // A point (position) is placed by the world matrix; a direction (normal)
        // is only rotated. Distinguish by whether a matrix was supplied.
        let v = match world {
            Some(matrix) => matrix.transform_point3(v),
            None => v,
        };
        out.push(math::cry_to_gltf(v));
    };

    if let Some(stream) = cgf.data_streams().get(&id) {
        let size = stream.element_size;
        let base = size.checked_mul(subset.first_vertex)?;
        for i in 0..count {
            let o = base + i * size;
            let raw = match size {
                12 => [f32_at(stream.data, o)?, f32_at(stream.data, o + 4)?, f32_at(stream.data, o + 8)?],
                8 => [half_at(stream.data, o)?, half_at(stream.data, o + 2)?, half_at(stream.data, o + 4)?],
                _ => return None,
            };
            push(&mut out, raw);
        }
    } else if let Some(data_ref) = cgf.data_refs().get(&id) {
        let stride = data_ref.stride;
        let base = data_ref.offset + stride * subset.first_vertex;
        for i in 0..count {
            let o = base + i * stride;
            let raw = match stride {
                16 => [half_at(heap, o)?, half_at(heap, o + 2)?, half_at(heap, o + 4)?],
                24 => [f32_at(heap, o)?, f32_at(heap, o + 4)?, f32_at(heap, o + 8)?],
                _ => return None,
            };
            push(&mut out, raw);
        }
    } else {
        return None;
    }
    Some(out)
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
    } else if let Some(data_ref) = cgf.data_refs().get(&id) {
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
    } else {
        return None;
    }
    Some(out)
}

/// Read the subset's index list, localized to its vertex range.
fn read_indices(cgf: &CgfFile, mesh: &MeshChunk, subset: &MeshSubset, heap: &[u8]) -> Option<Vec<u32>> {
    let id = stream_id(mesh, KIND_INDICES)?;
    let count = subset.num_indices;
    let first_vertex = subset.first_vertex as u32;
    let mut out = Vec::with_capacity(count);

    let (data, stride, base): (&[u8], usize, usize) =
        if let Some(stream) = cgf.data_streams().get(&id) {
            let stride = stream.element_size;
            (stream.data, stride, stride.checked_mul(subset.first_index)?)
        } else if let Some(data_ref) = cgf.data_refs().get(&id) {
            let stride = data_ref.stride;
            (heap, stride, data_ref.offset + stride * subset.first_index)
        } else {
            return None;
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
