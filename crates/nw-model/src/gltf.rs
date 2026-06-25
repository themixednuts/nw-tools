//! A minimal glTF 2.0 writer: turns a [`crate::Model`] into a self-contained
//! binary `.glb` (and a `.gltf` + `.bin` pair). Covers the geometry subset now;
//! materials, skins, and animations slot into the same document model later.

use std::collections::HashMap;

use bevy_math::Isometry3d;
use bevy_math::bounding::Aabb3d;
use glam::{Vec2, Vec3, Vec3A, Vec4};
use serde::Serialize;

use crate::geometry::{Model, Skeleton};
use crate::material::{MapSlot, MaterialSet};

const COMPONENT_FLOAT: u32 = 5126;
const COMPONENT_UNSIGNED_INT: u32 = 5125;
const COMPONENT_UNSIGNED_SHORT: u32 = 5123;
const TARGET_ARRAY_BUFFER: u32 = 34962;
const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;
const MODE_TRIANGLES: u32 = 4;

#[derive(Serialize)]
struct Asset {
    version: &'static str,
    generator: &'static str,
}

#[derive(Serialize)]
struct Scene {
    nodes: Vec<usize>,
}

#[derive(Serialize, Default)]
struct Node {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mesh: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skin: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<usize>,
    /// Column-major local transform (16 floats), omitted when identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    matrix: Option<[f32; 16]>,
}

#[derive(Serialize)]
struct Attributes {
    #[serde(rename = "POSITION")]
    position: usize,
    #[serde(rename = "NORMAL", skip_serializing_if = "Option::is_none")]
    normal: Option<usize>,
    #[serde(rename = "TEXCOORD_0", skip_serializing_if = "Option::is_none")]
    texcoord_0: Option<usize>,
    #[serde(rename = "JOINTS_0", skip_serializing_if = "Option::is_none")]
    joints_0: Option<usize>,
    #[serde(rename = "WEIGHTS_0", skip_serializing_if = "Option::is_none")]
    weights_0: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Skin {
    joints: Vec<usize>,
    inverse_bind_matrices: usize,
}

#[derive(Serialize)]
struct Primitive {
    attributes: Attributes,
    indices: usize,
    mode: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    material: Option<usize>,
}

#[derive(Serialize)]
struct TextureInfo {
    index: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PbrMetallicRoughness {
    base_color_factor: Vec4,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_color_texture: Option<TextureInfo>,
    metallic_factor: f32,
    roughness_factor: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Material {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    pbr_metallic_roughness: PbrMetallicRoughness,
    #[serde(skip_serializing_if = "Option::is_none")]
    normal_texture: Option<TextureInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    emissive_texture: Option<TextureInfo>,
    emissive_factor: Vec3,
    alpha_mode: &'static str,
    double_sided: bool,
}

/// glTF image (embedded via a buffer view in the GLB binary chunk).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Image {
    mime_type: String,
    buffer_view: usize,
}

#[derive(Serialize)]
struct Texture {
    source: usize,
    sampler: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Sampler {
    mag_filter: u32,
    min_filter: u32,
    wrap_s: u32,
    wrap_t: u32,
}

#[derive(Serialize)]
struct Mesh {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    primitives: Vec<Primitive>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Accessor {
    buffer_view: usize,
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<Vec3>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<Vec3>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BufferView {
    buffer: usize,
    byte_offset: usize,
    byte_length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Buffer {
    byte_length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    uri: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    asset: Asset,
    scene: usize,
    scenes: Vec<Scene>,
    nodes: Vec<Node>,
    meshes: Vec<Mesh>,
    accessors: Vec<Accessor>,
    buffer_views: Vec<BufferView>,
    buffers: Vec<Buffer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skins: Vec<Skin>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    materials: Vec<Material>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    textures: Vec<Texture>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    images: Vec<Image>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    samplers: Vec<Sampler>,
}

/// Loaded texture image bytes (already encoded, e.g. PNG) for embedding.
pub struct TextureData {
    pub bytes: Vec<u8>,
    pub mime: String,
}

/// A texture loader: maps a `.mtl` texture File path to encoded image bytes.
pub type TextureLoader<'a> = dyn FnMut(&str) -> Option<TextureData> + 'a;

/// Accumulates the binary blob, buffer views, accessors, and material/texture
/// tables as primitives are added.
struct Builder {
    bin: Vec<u8>,
    accessors: Vec<Accessor>,
    views: Vec<BufferView>,
    meshes: Vec<Mesh>,
    nodes: Vec<Node>,
    skins: Vec<Skin>,
    materials: Vec<Material>,
    textures: Vec<Texture>,
    images: Vec<Image>,
    samplers: Vec<Sampler>,
    /// sub-material index → glTF material index.
    material_cache: HashMap<usize, Option<usize>>,
    /// texture File path → glTF texture index.
    texture_cache: HashMap<String, Option<usize>>,
}

impl Builder {
    fn new() -> Self {
        Self {
            bin: Vec::new(),
            accessors: Vec::new(),
            views: Vec::new(),
            meshes: Vec::new(),
            nodes: Vec::new(),
            skins: Vec::new(),
            materials: Vec::new(),
            textures: Vec::new(),
            images: Vec::new(),
            samplers: Vec::new(),
            material_cache: HashMap::new(),
            texture_cache: HashMap::new(),
        }
    }

    fn push_view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let byte_offset = self.bin.len();
        self.bin.extend_from_slice(bytes);
        self.views.push(BufferView {
            buffer: 0,
            byte_offset,
            byte_length: bytes.len(),
            target,
        });
        self.views.len() - 1
    }

    /// Get or create a glTF texture for a `.mtl` File path, embedding the image.
    fn texture(&mut self, file: &str, loader: &mut TextureLoader<'_>) -> Option<usize> {
        if let Some(cached) = self.texture_cache.get(file) {
            return *cached;
        }
        let resolved = loader(file).map(|data| {
            let view = self.push_view(&data.bytes, None);
            let image = self.images.len();
            self.images.push(Image {
                mime_type: data.mime,
                buffer_view: view,
            });
            if self.samplers.is_empty() {
                // 9729 = LINEAR, 9987 = LINEAR_MIPMAP_LINEAR, 10497 = REPEAT.
                self.samplers.push(Sampler {
                    mag_filter: 9729,
                    min_filter: 9987,
                    wrap_s: 10497,
                    wrap_t: 10497,
                });
            }
            let texture = self.textures.len();
            self.textures.push(Texture {
                source: image,
                sampler: 0,
            });
            texture
        });
        self.texture_cache.insert(file.to_string(), resolved);
        resolved
    }

    /// Get or create a glTF material for a sub-material, or `None` if it should be
    /// skipped (Nodraw / shadow proxy).
    fn material(
        &mut self,
        materials: &MaterialSet,
        index: usize,
        loader: &mut TextureLoader<'_>,
    ) -> Option<usize> {
        if let Some(cached) = self.material_cache.get(&index) {
            return *cached;
        }
        let resolved = self.build_material(materials, index, loader);
        self.material_cache.insert(index, resolved);
        resolved
    }

    fn build_material(
        &mut self,
        materials: &MaterialSet,
        index: usize,
        loader: &mut TextureLoader<'_>,
    ) -> Option<usize> {
        let sub = materials.sub_materials.get(index)?;
        if sub.is_skippable() {
            return None;
        }
        let base_color_texture = sub
            .texture(MapSlot::Diffuse)
            .and_then(|t| self.texture(&t.file, loader))
            .map(|index| TextureInfo { index });
        let normal_texture = sub
            .texture(MapSlot::Bumpmap)
            .and_then(|t| self.texture(&t.file, loader))
            .map(|index| TextureInfo { index });
        let emissive_texture = sub
            .texture(MapSlot::Emittance)
            .and_then(|t| self.texture(&t.file, loader))
            .map(|index| TextureInfo { index });

        let base_color_factor = Vec4::new(sub.diffuse.x, sub.diffuse.y, sub.diffuse.z, sub.opacity);
        let emissive_factor = sub.emittance.truncate().min(Vec3::ONE);
        let material = Material {
            name: (!sub.name.is_empty()).then(|| sub.name.clone()),
            pbr_metallic_roughness: PbrMetallicRoughness {
                base_color_factor,
                base_color_texture,
                // New World uses a non-metallic flow; roughness is approximated.
                metallic_factor: 0.0,
                roughness_factor: 1.0,
            },
            normal_texture,
            emissive_texture,
            emissive_factor,
            alpha_mode: if sub.is_transparent() { "BLEND" } else { "OPAQUE" },
            double_sided: false,
        };
        let material_index = self.materials.len();
        self.materials.push(material);
        Some(material_index)
    }

    fn accessor_positions(&mut self, data: &[Vec3]) -> usize {
        // glTF requires POSITION accessors to carry the bounding box.
        let aabb = Aabb3d::from_point_cloud(
            Isometry3d::IDENTITY,
            data.iter().map(|v| Vec3A::from(*v)),
        );
        let mut bytes = Vec::with_capacity(data.len() * 12);
        for v in data {
            for value in v.to_array() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let view = self.push_view(&bytes, Some(TARGET_ARRAY_BUFFER));
        self.accessors.push(Accessor {
            buffer_view: view,
            component_type: COMPONENT_FLOAT,
            count: data.len(),
            kind: "VEC3",
            min: Some(aabb.min.into()),
            max: Some(aabb.max.into()),
        });
        self.accessors.len() - 1
    }

    fn accessor_vec3(&mut self, data: &[Vec3]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 12);
        for v in data {
            for value in v.to_array() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let view = self.push_view(&bytes, Some(TARGET_ARRAY_BUFFER));
        self.accessors.push(Accessor {
            buffer_view: view,
            component_type: COMPONENT_FLOAT,
            count: data.len(),
            kind: "VEC3",
            min: None,
            max: None,
        });
        self.accessors.len() - 1
    }

    fn accessor_vec2(&mut self, data: &[Vec2]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 8);
        for v in data {
            for value in v.to_array() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let view = self.push_view(&bytes, Some(TARGET_ARRAY_BUFFER));
        self.accessors.push(Accessor {
            buffer_view: view,
            component_type: COMPONENT_FLOAT,
            count: data.len(),
            kind: "VEC2",
            min: None,
            max: None,
        });
        self.accessors.len() - 1
    }

    fn accessor_indices(&mut self, data: &[u32]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 4);
        for value in data {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let view = self.push_view(&bytes, Some(TARGET_ELEMENT_ARRAY_BUFFER));
        self.accessors.push(Accessor {
            buffer_view: view,
            component_type: COMPONENT_UNSIGNED_INT,
            count: data.len(),
            kind: "SCALAR",
            min: None,
            max: None,
        });
        self.accessors.len() - 1
    }

    fn accessor_joints(&mut self, data: &[[u16; 4]]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 8);
        for joint in data {
            for value in joint {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let view = self.push_view(&bytes, Some(TARGET_ARRAY_BUFFER));
        self.accessors.push(Accessor {
            buffer_view: view,
            component_type: COMPONENT_UNSIGNED_SHORT,
            count: data.len(),
            kind: "VEC4",
            min: None,
            max: None,
        });
        self.accessors.len() - 1
    }

    fn accessor_weights(&mut self, data: &[[f32; 4]]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 16);
        for weight in data {
            for value in weight {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let view = self.push_view(&bytes, Some(TARGET_ARRAY_BUFFER));
        self.accessors.push(Accessor {
            buffer_view: view,
            component_type: COMPONENT_FLOAT,
            count: data.len(),
            kind: "VEC4",
            min: None,
            max: None,
        });
        self.accessors.len() - 1
    }

    /// Inverse bind matrices accessor (column-major `MAT4`, no buffer target).
    fn accessor_matrices(&mut self, data: &[glam::Mat4]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 64);
        for matrix in data {
            for value in matrix.to_cols_array() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let view = self.push_view(&bytes, None);
        self.accessors.push(Accessor {
            buffer_view: view,
            component_type: COMPONENT_FLOAT,
            count: data.len(),
            kind: "MAT4",
            min: None,
            max: None,
        });
        self.accessors.len() - 1
    }

    /// Emit the skeleton's joint nodes (indices `0..bones`, with the hierarchy and
    /// local transforms), an inverse-bind-matrix accessor, and a skin. Returns the
    /// skin index; root joint node indices are appended to `roots`.
    fn add_skeleton(&mut self, skeleton: &Skeleton, roots: &mut Vec<usize>) -> usize {
        let count = skeleton.bones.len();
        for bone in &skeleton.bones {
            self.nodes.push(Node {
                name: (!bone.name.is_empty()).then(|| bone.name.clone()),
                matrix: Some(bone.local.to_cols_array()),
                ..Node::default()
            });
        }
        for (index, bone) in skeleton.bones.iter().enumerate() {
            match bone.parent {
                Some(parent) => self.nodes[parent].children.push(index),
                None => roots.push(index),
            }
        }
        let ibms = skeleton
            .bones
            .iter()
            .map(|bone| bone.inverse_bind)
            .collect::<Vec<_>>();
        let inverse_bind_matrices = self.accessor_matrices(&ibms);
        let skin_index = self.skins.len();
        self.skins.push(Skin {
            joints: (0..count).collect(),
            inverse_bind_matrices,
        });
        skin_index
    }
}

/// Build the in-memory glTF document and its binary blob from a model, optionally
/// resolving materials/textures.
fn build(
    model: &Model,
    materials: Option<&MaterialSet>,
    loader: &mut TextureLoader<'_>,
) -> (Document, Vec<u8>) {
    let mut builder = Builder::new();

    // Emit the skeleton first so joint node indices are `0..bone_count` and line
    // up with the per-vertex bone indices.
    let mut root_nodes = Vec::new();
    let skin = model
        .skeleton
        .as_ref()
        .filter(|skeleton| !skeleton.bones.is_empty())
        .map(|skeleton| builder.add_skeleton(skeleton, &mut root_nodes));

    for mesh in &model.meshes {
        let mut primitives = Vec::new();
        for primitive in &mesh.primitives {
            if primitive.positions.is_empty() || primitive.indices.is_empty() {
                continue;
            }
            // Resolve the material first: a skippable sub-material (Nodraw / shadow
            // proxy) drops the whole primitive.
            let material = match materials {
                Some(set) => {
                    let index = usize::try_from(primitive.material_id).unwrap_or(usize::MAX);
                    if set
                        .sub_materials
                        .get(index)
                        .is_some_and(super::material::SubMaterial::is_skippable)
                    {
                        continue;
                    }
                    builder.material(set, index, loader)
                }
                None => None,
            };

            let count = primitive.positions.len();
            let position = builder.accessor_positions(&primitive.positions);
            let normal = primitive
                .normals
                .as_ref()
                .filter(|n| n.len() == count)
                .map(|n| builder.accessor_vec3(n));
            let texcoord_0 = primitive
                .uvs
                .as_ref()
                .filter(|uv| uv.len() == count)
                .map(|uv| builder.accessor_vec2(uv));
            // JOINTS_0 and WEIGHTS_0 must come as a pair.
            let (joints_0, weights_0) = match (&primitive.joints, &primitive.weights) {
                (Some(j), Some(w)) if j.len() == count && w.len() == count => {
                    (Some(builder.accessor_joints(j)), Some(builder.accessor_weights(w)))
                }
                _ => (None, None),
            };
            let indices = builder.accessor_indices(&primitive.indices);
            primitives.push(Primitive {
                attributes: Attributes {
                    position,
                    normal,
                    texcoord_0,
                    joints_0,
                    weights_0,
                },
                indices,
                mode: MODE_TRIANGLES,
                material,
            });
        }
        if primitives.is_empty() {
            continue;
        }
        let mesh_index = builder.meshes.len();
        builder.meshes.push(Mesh {
            name: Some(mesh.name.clone()),
            primitives,
        });
        let mesh_node = Node {
            name: Some(mesh.name.clone()),
            mesh: Some(mesh_index),
            skin: skin.filter(|_| mesh.skinned),
            ..Node::default()
        };
        root_nodes.push(builder.nodes.len());
        builder.nodes.push(mesh_node);
    }

    let node_indices = root_nodes;
    let document = Document {
        asset: Asset {
            version: "2.0",
            generator: "nw-tools",
        },
        scene: 0,
        scenes: vec![Scene {
            nodes: node_indices,
        }],
        nodes: builder.nodes,
        meshes: builder.meshes,
        accessors: builder.accessors,
        buffer_views: builder.views,
        buffers: vec![Buffer {
            byte_length: builder.bin.len(),
            uri: None,
        }],
        skins: builder.skins,
        materials: builder.materials,
        textures: builder.textures,
        images: builder.images,
        samplers: builder.samplers,
    };
    (document, builder.bin)
}

/// Typestate marker: no material set attached, so textures cannot be supplied.
pub struct NoMaterials;

/// Typestate marker: a material set is attached; texture loaders are now allowed.
pub struct WithMaterials<'a>(&'a MaterialSet);

/// Fluent glTF exporter for a [`Model`]. The type state enforces that a texture
/// loader can only be supplied *after* a material set is attached (textures are
/// referenced through materials), making the meaningless "textures without
/// materials" call unrepresentable.
///
/// ```ignore
/// let glb = Gltf::new(&model).to_glb();                       // geometry only
/// let glb = Gltf::new(&model).materials(&mtl).to_glb(loader); // + materials/textures
/// ```
pub struct Gltf<'a, State> {
    model: &'a Model,
    state: State,
}

impl<'a> Gltf<'a, NoMaterials> {
    #[must_use]
    pub fn new(model: &'a Model) -> Self {
        Self {
            model,
            state: NoMaterials,
        }
    }

    /// Attach a material set, unlocking texture loading.
    #[must_use]
    pub fn materials(self, materials: &'a MaterialSet) -> Gltf<'a, WithMaterials<'a>> {
        Gltf {
            model: self.model,
            state: WithMaterials(materials),
        }
    }

    /// Serialize to a self-contained `.glb` (geometry only).
    #[must_use]
    pub fn to_glb(&self) -> Vec<u8> {
        glb_bytes(build(self.model, None, &mut |_| None))
    }

    /// Serialize to a `.gltf` JSON string + external `.bin` (geometry only).
    #[must_use]
    pub fn to_gltf(&self, bin_uri: &str) -> (String, Vec<u8>) {
        gltf_pair(build(self.model, None, &mut |_| None), bin_uri)
    }
}

impl<'a> Gltf<'a, WithMaterials<'a>> {
    /// Serialize to `.glb`, resolving each material's textures through `loader`.
    #[must_use]
    pub fn to_glb(&self, mut loader: impl FnMut(&str) -> Option<TextureData>) -> Vec<u8> {
        glb_bytes(build(self.model, Some(self.state.0), &mut loader))
    }

    /// Serialize to `.gltf` + `.bin`, resolving textures through `loader`.
    #[must_use]
    pub fn to_gltf(
        &self,
        bin_uri: &str,
        mut loader: impl FnMut(&str) -> Option<TextureData>,
    ) -> (String, Vec<u8>) {
        gltf_pair(build(self.model, Some(self.state.0), &mut loader), bin_uri)
    }
}

/// Pack a built document + blob into a binary `.glb`.
fn glb_bytes((mut document, mut bin): (Document, Vec<u8>)) -> Vec<u8> {
    // The single GLB buffer is the embedded BIN chunk (no uri).
    document.buffers[0].uri = None;
    let mut json = serde_json::to_vec(&document).unwrap_or_default();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }

    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&0x4654_6C67u32.to_le_bytes()); // "glTF"
    out.extend_from_slice(&2u32.to_le_bytes()); // version
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes()); // "JSON"
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes()); // "BIN\0"
    out.extend_from_slice(&bin);
    out
}

/// Pack a built document + blob into a `.gltf` JSON string + external `.bin`.
fn gltf_pair((mut document, bin): (Document, Vec<u8>), bin_uri: &str) -> (String, Vec<u8>) {
    document.buffers[0].uri = Some(bin_uri.to_string());
    let json = serde_json::to_string_pretty(&document).unwrap_or_default();
    (json, bin)
}
