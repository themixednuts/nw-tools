//! A minimal glTF 2.0 writer: turns a [`crate::Model`] into a self-contained
//! binary `.glb` (and a `.gltf` + `.bin` pair). Covers the geometry subset now;
//! materials, skins, and animations slot into the same document model later.

use std::collections::{BTreeMap, HashMap};

use bevy_math::Isometry3d;
use bevy_math::bounding::Aabb3d;
use cry_animation::{AnimationClip, AnimationEvent, CafRootMotion};
use glam::{Quat, Vec2, Vec3, Vec3A, Vec4};
use serde::Serialize;

use crate::geometry::{AuxiliaryNode, AuxiliaryNodeRole, Model, Skeleton};
use crate::material::{MapSlot, MaterialSet, SubMaterial};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    translation: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scale: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Option<NodeExtensions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extras: Option<CryNodeExtras>,
}

#[derive(Serialize)]
struct NodeExtensions {
    #[serde(rename = "MSFT_lod")]
    msft_lod: MsftLod,
}

#[derive(Serialize)]
struct MsftLod {
    ids: Vec<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CryNodeExtras {
    #[serde(skip_serializing_if = "Option::is_none")]
    cry_lod: Option<u32>,
    shadow_proxy: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    skeleton: Option<usize>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    occlusion_texture: Option<TextureInfo>,
    emissive_factor: Vec3,
    alpha_mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    alpha_cutoff: Option<f32>,
    double_sided: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Option<MaterialExtensions>,
    extras: CryMaterialExtras,
}

#[derive(Serialize)]
struct MaterialExtensions {
    #[serde(
        rename = "KHR_materials_specular",
        skip_serializing_if = "Option::is_none"
    )]
    specular: Option<KhrMaterialsSpecular>,
    #[serde(
        rename = "KHR_materials_emissive_strength",
        skip_serializing_if = "Option::is_none"
    )]
    emissive_strength: Option<KhrMaterialsEmissiveStrength>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct KhrMaterialsSpecular {
    specular_factor: f32,
    specular_color_factor: Vec3,
    #[serde(skip_serializing_if = "Option::is_none")]
    specular_color_texture: Option<TextureInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct KhrMaterialsEmissiveStrength {
    emissive_strength: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CryMaterialExtras {
    cry_shader: String,
    cry_surface_type: String,
    cry_flags: i64,
    cry_opacity: f32,
    cry_shininess: f32,
    cry_alpha_test: f32,
    cry_diffuse: Vec4,
    cry_specular: Vec4,
    cry_emittance: Vec4,
    #[serde(skip_serializing_if = "Option::is_none")]
    cry_shader_generation_mask: Option<String>,
    /// Lossless `.mtl` subtree, including every texture slot/TexMod/public param.
    cry_source: serde_json::Value,
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
    min: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<Vec<f32>>,
}

#[derive(Serialize)]
struct Animation {
    name: String,
    samplers: Vec<AnimationSampler>,
    channels: Vec<AnimationChannel>,
    extras: CryAnimationExtras,
}

#[derive(Serialize)]
struct AnimationSampler {
    input: usize,
    output: usize,
    interpolation: &'static str,
}

#[derive(Serialize)]
struct AnimationChannel {
    sampler: usize,
    target: AnimationTarget,
}

#[derive(Serialize)]
struct AnimationTarget {
    node: usize,
    path: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CryAnimationExtras {
    cry_source_path: String,
    cry_duration: f32,
    cry_sample_rate: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    cry_root_motion: Option<CryRootMotion>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cry_events: Vec<CryAnimationEvent>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CryRootMotion {
    start_translation: [f32; 3],
    start_rotation: [f32; 4],
    end_translation: [f32; 3],
    end_rotation: [f32; 4],
    velocity: [f32; 3],
    distance: f32,
    speed: f32,
    turn_speed: f32,
    turn_angle: f32,
    slope: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CryAnimationEvent {
    name: String,
    name_lowercase_crc32: u32,
    normalized_time: f32,
    normalized_end_time: f32,
    parameter: String,
    bone: String,
    second_bone: String,
    offset: [f32; 3],
    direction: [f32; 3],
    model: String,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    animations: Vec<Animation>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extensions_used: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extras: Option<CryAssetExtras>,
}

/// Typed envelope for Cry data that glTF has no native representation for.
/// Geometry, skins, materials, and CAF channels still use native glTF fields;
/// CDF/CHRPARAMS/Mannequin/audio control documents live here losslessly.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CryAssetExtras {
    pub source_path: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub source_assets: Vec<CrySourceAsset>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub non_render_nodes: Vec<CryNonRenderNode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unbound_animations: Vec<CryUnboundAnimation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CryUnboundAnimation {
    pub source_path: String,
    pub skeleton: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CryNonRenderNode {
    pub name: String,
    pub role: CryNonRenderNodeRole,
    pub object_chunk_id: i32,
    pub parent_chunk_id: i32,
    pub material_chunk_id: i32,
    pub transform: [f32; 16],
    pub properties: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CryNonRenderNodeRole {
    PhysicsProxy,
}

/// A Cry animation clip bound to one skeleton graph in a multi-character model.
#[derive(Debug, Clone)]
pub struct ModelAnimation {
    pub skeleton: usize,
    pub clip: AnimationClip,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrySourceAsset {
    pub path: String,
    pub kind: CrySourceAssetKind,
    pub document: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CrySourceAssetKind {
    CharacterDefinition,
    CharacterParameters,
    AnimationEvents,
    MannequinAnimationDatabase,
    MannequinTagDefinition,
    MannequinControllerDefinition,
    BlendSpace,
    CombinedBlendSpace,
    AudioControls,
    WwiseSoundBank,
    WwiseMedia,
    WwiseTriggerBankMap,
}

/// Loaded texture image bytes (already encoded, e.g. PNG) for embedding.
#[derive(Debug, Clone)]
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
    animations: Vec<Animation>,
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
    uses_khr_materials_specular: bool,
    uses_khr_materials_emissive_strength: bool,
}

struct EmittedSkeleton {
    skin: usize,
    joints: Vec<usize>,
    roots: Vec<usize>,
}

struct BuiltAnimation {
    bin: Vec<u8>,
    accessors: Vec<Accessor>,
    views: Vec<BufferView>,
    animation: Animation,
}

impl Builder {
    fn new() -> Self {
        Self {
            bin: Vec::new(),
            accessors: Vec::new(),
            views: Vec::new(),
            meshes: Vec::new(),
            animations: Vec::new(),
            nodes: Vec::new(),
            skins: Vec::new(),
            materials: Vec::new(),
            textures: Vec::new(),
            images: Vec::new(),
            samplers: Vec::new(),
            material_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            uses_khr_materials_specular: false,
            uses_khr_materials_emissive_strength: false,
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

    fn append_animation(&mut self, mut built: BuiltAnimation) {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let byte_offset = self.bin.len();
        let view_offset = self.views.len();
        let accessor_offset = self.accessors.len();
        for view in &mut built.views {
            view.byte_offset += byte_offset;
        }
        for accessor in &mut built.accessors {
            accessor.buffer_view += view_offset;
        }
        for sampler in &mut built.animation.samplers {
            sampler.input += accessor_offset;
            sampler.output += accessor_offset;
        }
        self.bin.append(&mut built.bin);
        self.views.append(&mut built.views);
        self.accessors.append(&mut built.accessors);
        self.animations.push(built.animation);
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
            .texture(MapSlot::Normals)
            .and_then(|t| self.texture(&t.file, loader))
            .map(|index| TextureInfo { index });
        let emissive_texture = sub
            .texture(MapSlot::Emittance)
            .and_then(|t| self.texture(&t.file, loader))
            .map(|index| TextureInfo { index });
        let occlusion_texture = sub
            .texture(MapSlot::Occlusion)
            .and_then(|t| self.texture(&t.file, loader))
            .map(|index| TextureInfo { index });
        let specular_color_texture = sub
            .texture(MapSlot::Specular)
            .and_then(|t| self.texture(&t.file, loader))
            .map(|index| TextureInfo { index });

        let base_color_factor = Vec4::new(sub.diffuse.x, sub.diffuse.y, sub.diffuse.z, sub.opacity);
        let emissive_factor = sub.emittance.truncate().min(Vec3::ONE);
        let emissive_strength = sub.emittance.w.max(1.0);
        let uses_emissive_strength = emissive_strength > 1.0;
        let uses_specular = specular_color_texture.is_some() || sub.specular != Vec4::ONE;
        self.uses_khr_materials_emissive_strength |= uses_emissive_strength;
        self.uses_khr_materials_specular |= uses_specular;
        let roughness_factor = 1.0 - (sub.shininess / 255.0).clamp(0.0, 1.0);
        let alpha_mode = if sub.is_alpha_masked() {
            "MASK"
        } else if sub.is_transparent() {
            "BLEND"
        } else {
            "OPAQUE"
        };
        let material = Material {
            name: (!sub.name.is_empty()).then(|| sub.name.clone()),
            pbr_metallic_roughness: PbrMetallicRoughness {
                base_color_factor,
                base_color_texture,
                // Cry's legacy shader flow is non-metallic. `Shininess` stores
                // smoothness in the 0..255 engine range.
                metallic_factor: 0.0,
                roughness_factor,
            },
            normal_texture,
            emissive_texture,
            occlusion_texture,
            emissive_factor,
            alpha_mode,
            alpha_cutoff: sub.is_alpha_masked().then_some(sub.alpha_test),
            double_sided: sub.is_double_sided(),
            extensions: (uses_specular || uses_emissive_strength).then_some(MaterialExtensions {
                specular: uses_specular.then_some(KhrMaterialsSpecular {
                    specular_factor: sub.specular.w.clamp(0.0, 1.0),
                    specular_color_factor: sub.specular.truncate().min(Vec3::ONE),
                    specular_color_texture,
                }),
                emissive_strength: uses_emissive_strength
                    .then_some(KhrMaterialsEmissiveStrength { emissive_strength }),
            }),
            extras: CryMaterialExtras::from(sub),
        };
        let material_index = self.materials.len();
        self.materials.push(material);
        Some(material_index)
    }

    fn accessor_positions(&mut self, data: &[Vec3]) -> usize {
        // glTF requires POSITION accessors to carry the bounding box.
        let aabb =
            Aabb3d::from_point_cloud(Isometry3d::IDENTITY, data.iter().map(|v| Vec3A::from(*v)));
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
            min: Some(Vec3::from(aabb.min).to_array().to_vec()),
            max: Some(Vec3::from(aabb.max).to_array().to_vec()),
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

    fn accessor_scalar_f32(&mut self, data: &[f32]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 4);
        for value in data {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let view = self.push_view(&bytes, None);
        let min = data
            .iter()
            .copied()
            .reduce(f32::min)
            .map(|value| vec![value]);
        let max = data
            .iter()
            .copied()
            .reduce(f32::max)
            .map(|value| vec![value]);
        self.accessors.push(Accessor {
            buffer_view: view,
            component_type: COMPONENT_FLOAT,
            count: data.len(),
            kind: "SCALAR",
            min,
            max,
        });
        self.accessors.len() - 1
    }

    fn accessor_vec4_f32(&mut self, data: &[[f32; 4]]) -> usize {
        let mut bytes = Vec::with_capacity(data.len() * 16);
        for value in data {
            for component in value {
                bytes.extend_from_slice(&component.to_le_bytes());
            }
        }
        let view = self.push_view(&bytes, None);
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

    /// Emit one independent skeleton graph and its glTF skin.
    fn add_skeleton(&mut self, skeleton: &Skeleton) -> EmittedSkeleton {
        let count = skeleton.bones.len();
        let node_offset = self.nodes.len();
        for bone in &skeleton.bones {
            let (scale, rotation, translation) = bone.local.to_scale_rotation_translation();
            self.nodes.push(Node {
                name: (!bone.name.is_empty()).then(|| bone.name.clone()),
                translation: (!translation.abs_diff_eq(Vec3::ZERO, 0.000_001))
                    .then_some(translation.to_array()),
                rotation: (!rotation.abs_diff_eq(Quat::IDENTITY, 0.000_001))
                    .then_some(rotation.normalize().to_array()),
                scale: (!scale.abs_diff_eq(Vec3::ONE, 0.000_001)).then_some(scale.to_array()),
                ..Node::default()
            });
        }
        let mut roots = Vec::new();
        for (index, bone) in skeleton.bones.iter().enumerate() {
            match bone.parent {
                Some(parent) => self.nodes[node_offset + parent]
                    .children
                    .push(node_offset + index),
                None => roots.push(node_offset + index),
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
            joints: (node_offset..node_offset + count).collect(),
            inverse_bind_matrices,
            skeleton: None,
        });
        EmittedSkeleton {
            skin: skin_index,
            joints: (node_offset..node_offset + count).collect(),
            roots,
        }
    }

    fn add_animation(&mut self, clip: &AnimationClip, skeleton: &Skeleton, joint_nodes: &[usize]) {
        let bone_by_controller = skeleton
            .bones
            .iter()
            .enumerate()
            .filter_map(|(index, bone)| {
                joint_nodes
                    .get(index)
                    .copied()
                    .map(|node| (bone.controller_id, node))
            })
            .collect::<HashMap<_, _>>();
        let mut samplers = Vec::new();
        let mut channels = Vec::new();

        for controller in &clip.caf.controllers {
            let Some(&node) = bone_by_controller.get(&controller.controller_id) else {
                continue;
            };
            if !controller.positions.is_empty() {
                let times = controller
                    .positions
                    .iter()
                    .map(|key| key.time)
                    .collect::<Vec<_>>();
                let values = controller
                    .positions
                    .iter()
                    .map(|key| crate::math::cry_to_gltf(Vec3::from_array(key.value)))
                    .collect::<Vec<_>>();
                self.push_animation_channel(
                    node,
                    "translation",
                    &times,
                    AnimationValues::Vec3(&values),
                    &mut samplers,
                    &mut channels,
                );
            }
            if !controller.rotations.is_empty() {
                let times = controller
                    .rotations
                    .iter()
                    .map(|key| key.time)
                    .collect::<Vec<_>>();
                let values = controller
                    .rotations
                    .iter()
                    .map(|key| {
                        let value = key.value;
                        crate::math::cry_to_gltf_quat(Quat::from_xyzw(
                            value[0], value[1], value[2], value[3],
                        ))
                        .normalize()
                        .to_array()
                    })
                    .collect::<Vec<_>>();
                self.push_animation_channel(
                    node,
                    "rotation",
                    &times,
                    AnimationValues::Vec4(&values),
                    &mut samplers,
                    &mut channels,
                );
            }
            if !controller.scales.is_empty() {
                let times = controller
                    .scales
                    .iter()
                    .map(|key| key.time)
                    .collect::<Vec<_>>();
                let values = controller
                    .scales
                    .iter()
                    .map(|key| crate::math::cry_to_gltf_scale(Vec3::from_array(key.value)))
                    .collect::<Vec<_>>();
                self.push_animation_channel(
                    node,
                    "scale",
                    &times,
                    AnimationValues::Vec3(&values),
                    &mut samplers,
                    &mut channels,
                );
            }
        }

        self.animations.push(Animation {
            name: clip.name.clone(),
            samplers,
            channels,
            extras: CryAnimationExtras {
                cry_source_path: clip.source_path.clone(),
                cry_duration: clip.caf.header.total_duration,
                cry_sample_rate: clip.caf.sample_rate,
                cry_root_motion: clip.caf.root_motion.map(CryRootMotion::from),
                cry_events: clip.events.iter().map(CryAnimationEvent::from).collect(),
            },
        });
    }

    fn push_animation_channel(
        &mut self,
        node: usize,
        path: &'static str,
        times: &[f32],
        values: AnimationValues<'_>,
        samplers: &mut Vec<AnimationSampler>,
        channels: &mut Vec<AnimationChannel>,
    ) {
        let input = self.accessor_scalar_f32(times);
        let output = match values {
            AnimationValues::Vec3(values) => self.accessor_vec3(values),
            AnimationValues::Vec4(values) => self.accessor_vec4_f32(values),
        };
        let sampler = samplers.len();
        samplers.push(AnimationSampler {
            input,
            output,
            interpolation: "LINEAR",
        });
        channels.push(AnimationChannel {
            sampler,
            target: AnimationTarget { node, path },
        });
    }
}

impl From<&SubMaterial> for CryMaterialExtras {
    fn from(material: &SubMaterial) -> Self {
        Self {
            cry_shader: material.shader.clone(),
            cry_surface_type: material.surface_type.clone(),
            cry_flags: material.flags,
            cry_opacity: material.opacity,
            cry_shininess: material.shininess,
            cry_alpha_test: material.alpha_test,
            cry_diffuse: material.diffuse,
            cry_specular: material.specular,
            cry_emittance: material.emittance,
            cry_shader_generation_mask: material.shader_generation_mask.clone(),
            cry_source: xml_element_json(&material.source),
        }
    }
}

impl From<&AuxiliaryNode> for CryNonRenderNode {
    fn from(node: &AuxiliaryNode) -> Self {
        Self {
            name: node.name.clone(),
            role: match node.role {
                AuxiliaryNodeRole::PhysicsProxy => CryNonRenderNodeRole::PhysicsProxy,
            },
            object_chunk_id: node.object_chunk_id,
            parent_chunk_id: node.parent_chunk_id,
            material_chunk_id: node.material_chunk_id,
            transform: node.transform,
            properties: node.properties.clone(),
        }
    }
}

fn xml_element_json(element: &cry_xml::XmlElement) -> serde_json::Value {
    serde_json::json!({
        "name": element.name,
        "attributes": element.attributes,
        "text": element.text,
        "children": element.children.iter().map(xml_element_json).collect::<Vec<_>>(),
    })
}

enum AnimationValues<'a> {
    Vec3(&'a [Vec3]),
    Vec4(&'a [[f32; 4]]),
}

impl From<CafRootMotion> for CryRootMotion {
    fn from(root: CafRootMotion) -> Self {
        let start_rotation = root.start.rotation;
        let end_rotation = root.end.rotation;
        Self {
            start_translation: crate::math::cry_to_gltf(Vec3::from_array(root.start.translation))
                .to_array(),
            start_rotation: crate::math::cry_to_gltf_quat(Quat::from_xyzw(
                start_rotation[0],
                start_rotation[1],
                start_rotation[2],
                start_rotation[3],
            ))
            .to_array(),
            end_translation: crate::math::cry_to_gltf(Vec3::from_array(root.end.translation))
                .to_array(),
            end_rotation: crate::math::cry_to_gltf_quat(Quat::from_xyzw(
                end_rotation[0],
                end_rotation[1],
                end_rotation[2],
                end_rotation[3],
            ))
            .to_array(),
            velocity: crate::math::cry_to_gltf(Vec3::from_array(root.velocity)).to_array(),
            distance: root.distance,
            speed: root.speed,
            turn_speed: root.turn_speed,
            turn_angle: root.turn_angle,
            slope: root.slope,
        }
    }
}

impl From<&AnimationEvent> for CryAnimationEvent {
    fn from(event: &AnimationEvent) -> Self {
        Self {
            name: event.name.clone(),
            name_lowercase_crc32: event.name_lowercase_crc32,
            normalized_time: event.normalized_time,
            normalized_end_time: event.normalized_end_time,
            parameter: event.parameter.clone(),
            bone: event.bone.clone(),
            second_bone: event.second_bone.clone(),
            offset: crate::math::cry_to_gltf(Vec3::from_array(event.offset)).to_array(),
            direction: crate::math::cry_to_gltf(Vec3::from_array(event.direction)).to_array(),
            model: event.model.clone(),
        }
    }
}

/// Build the in-memory glTF document and its binary blob from a model, optionally
/// resolving materials/textures.
fn build(
    model: &Model,
    animations: &[ModelAnimation],
    materials: Option<&MaterialSet>,
    extras: Option<&CryAssetExtras>,
    loader: &mut TextureLoader<'_>,
    runner: &nw_jobs::JobRunner,
) -> (Document, Vec<u8>) {
    let mut builder = Builder::new();

    let mut root_nodes = Vec::new();
    let emitted_skeletons = model
        .skeletons
        .iter()
        .map(|skeleton| builder.add_skeleton(skeleton))
        .collect::<Vec<_>>();

    // Place every skeleton graph. A wrapper preserves the CDF attachment
    // transform once even when the character has multiple root joints.
    for (index, skeleton) in model.skeletons.iter().enumerate() {
        let emitted = &emitted_skeletons[index];
        let anchor = if let Some(placement) = &skeleton.placement {
            let (scale, rotation, translation) = placement.local.to_scale_rotation_translation();
            let anchor = builder.nodes.len();
            builder.nodes.push(Node {
                name: Some(format!("skeleton_{index}")),
                children: emitted.roots.clone(),
                translation: (!translation.abs_diff_eq(Vec3::ZERO, 0.000_001))
                    .then_some(translation.to_array()),
                rotation: (!rotation.abs_diff_eq(Quat::IDENTITY, 0.000_001))
                    .then_some(rotation.normalize().to_array()),
                scale: (!scale.abs_diff_eq(Vec3::ONE, 0.000_001)).then_some(scale.to_array()),
                ..Node::default()
            });
            if let (Some(parent_skeleton), Some(bone_name)) =
                (placement.parent_skeleton, placement.bone_name.as_deref())
                && let Some(parent) = model.skeletons.get(parent_skeleton)
                && let Some(bone) = parent.bone_index(bone_name)
                && let Some(parent_node) = emitted_skeletons
                    .get(parent_skeleton)
                    .and_then(|value| value.joints.get(bone))
                    .copied()
            {
                builder.nodes[parent_node].children.push(anchor);
            } else {
                root_nodes.push(anchor);
            }
            anchor
        } else if emitted.roots.len() == 1 {
            root_nodes.push(emitted.roots[0]);
            emitted.roots[0]
        } else {
            let anchor = builder.nodes.len();
            builder.nodes.push(Node {
                name: Some(format!("skeleton_{index}")),
                children: emitted.roots.clone(),
                ..Node::default()
            });
            root_nodes.push(anchor);
            anchor
        };
        builder.skins[emitted.skin].skeleton = Some(anchor);
    }

    let batch_size = runner.parallelism().saturating_mul(2).max(1);
    for batch in animations.chunks(batch_size) {
        let built = runner.map(batch, |animation| {
            let skeleton = model.skeletons.get(animation.skeleton)?;
            let emitted = emitted_skeletons.get(animation.skeleton)?;
            let mut local = Builder::new();
            local.add_animation(&animation.clip, skeleton, &emitted.joints);
            Some(BuiltAnimation {
                bin: local.bin,
                accessors: local.accessors,
                views: local.views,
                animation: local.animations.pop().expect("one animation was emitted"),
            })
        });
        for animation in built.into_iter().flatten() {
            builder.append_animation(animation);
        }
    }

    let mut lod_groups = BTreeMap::<String, Vec<(u32, usize)>>::new();
    let mut attachment_parents = HashMap::<usize, usize>::new();
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
                (Some(j), Some(w)) if j.len() == count && w.len() == count => (
                    Some(builder.accessor_joints(j)),
                    Some(builder.accessor_weights(w)),
                ),
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
        let (translation, rotation, scale) =
            mesh.attachment
                .as_ref()
                .map_or((None, None, None), |attachment| {
                    let (scale, rotation, translation) =
                        attachment.local.to_scale_rotation_translation();
                    (
                        (!translation.abs_diff_eq(Vec3::ZERO, 0.000_001))
                            .then_some(translation.to_array()),
                        (!rotation.abs_diff_eq(Quat::IDENTITY, 0.000_001))
                            .then_some(rotation.normalize().to_array()),
                        (!scale.abs_diff_eq(Vec3::ONE, 0.000_001)).then_some(scale.to_array()),
                    )
                });
        let mesh_node = Node {
            name: Some(mesh.name.clone()),
            mesh: Some(mesh_index),
            skin: mesh
                .skin
                .and_then(|index| emitted_skeletons.get(index))
                .map(|value| value.skin),
            translation,
            rotation,
            scale,
            extras: (mesh.lod.is_some() || mesh.shadow_proxy).then_some(CryNodeExtras {
                cry_lod: mesh.lod,
                shadow_proxy: mesh.shadow_proxy,
            }),
            ..Node::default()
        };
        let node_index = builder.nodes.len();
        builder.nodes.push(mesh_node);
        if let Some(attachment) = &mesh.attachment
            && let Some(skeleton_index) = attachment.skeleton
            && let Some(skeleton) = model.skeletons.get(skeleton_index)
            && let Some(bone_name) = attachment.bone_name.as_deref()
            && let Some(bone) = skeleton.bone_index(bone_name)
            && let Some(parent) = emitted_skeletons
                .get(skeleton_index)
                .and_then(|value| value.joints.get(bone))
                .copied()
        {
            attachment_parents.insert(node_index, parent);
        }
        lod_groups
            .entry(lod_group_name(&mesh.name))
            .or_default()
            .push((mesh.lod.unwrap_or(0), node_index));
    }

    let mut uses_msft_lod = false;
    for group in lod_groups.values_mut() {
        group.sort_unstable_by_key(|(lod, _)| *lod);
        let Some((_, root)) = group.first().copied() else {
            continue;
        };
        if let Some(parent) = attachment_parents.get(&root).copied() {
            builder.nodes[parent].children.push(root);
        } else {
            root_nodes.push(root);
        }
        if group.len() > 1 {
            uses_msft_lod = true;
            builder.nodes[root].extensions = Some(NodeExtensions {
                msft_lod: MsftLod {
                    ids: group.iter().skip(1).map(|(_, node)| *node).collect(),
                },
            });
        }
    }

    let node_indices = root_nodes;
    let mut extensions_used = Vec::new();
    if uses_msft_lod {
        extensions_used.push("MSFT_lod");
    }
    if builder.uses_khr_materials_specular {
        extensions_used.push("KHR_materials_specular");
    }
    if builder.uses_khr_materials_emissive_strength {
        extensions_used.push("KHR_materials_emissive_strength");
    }
    let document_extras = if extras.is_some() || !model.auxiliary_nodes.is_empty() {
        let mut value = extras.cloned().unwrap_or_default();
        value
            .non_render_nodes
            .extend(model.auxiliary_nodes.iter().map(CryNonRenderNode::from));
        Some(value)
    } else {
        None
    };
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
        animations: builder.animations,
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
        extensions_used,
        extras: document_extras,
    };
    (document, builder.bin)
}

fn lod_group_name(name: &str) -> String {
    let lowercase = name.to_ascii_lowercase();
    let Some(marker) = lowercase.find("$lod") else {
        return lowercase;
    };
    let suffix = &lowercase[marker + 4..];
    let digit_count = suffix.chars().take_while(char::is_ascii_digit).count();
    let mut grouped = lowercase;
    grouped.replace_range(marker..marker + 4 + digit_count, "");
    grouped
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
    animations: &'a [ModelAnimation],
    extras: Option<&'a CryAssetExtras>,
    state: State,
}

impl<'a> Gltf<'a, NoMaterials> {
    #[must_use]
    pub fn new(model: &'a Model) -> Self {
        Self {
            model,
            animations: &[],
            extras: None,
            state: NoMaterials,
        }
    }

    /// Attach a material set, unlocking texture loading.
    #[must_use]
    pub fn materials(self, materials: &'a MaterialSet) -> Gltf<'a, WithMaterials<'a>> {
        Gltf {
            model: self.model,
            animations: self.animations,
            extras: self.extras,
            state: WithMaterials(materials),
        }
    }

    /// Serialize to a self-contained `.glb` (geometry only).
    #[must_use]
    pub fn to_glb(&self) -> Vec<u8> {
        self.to_glb_with_runner(&nw_jobs::JobRunner::automatic())
    }

    /// Serialize to GLB using the caller's worker policy for animation channels.
    #[must_use]
    pub fn to_glb_with_runner(&self, runner: &nw_jobs::JobRunner) -> Vec<u8> {
        glb_bytes(build(
            self.model,
            self.animations,
            None,
            self.extras,
            &mut |_| None,
            runner,
        ))
    }

    /// Serialize to a `.gltf` JSON string + external `.bin` (geometry only).
    #[must_use]
    pub fn to_gltf(&self, bin_uri: &str) -> (String, Vec<u8>) {
        self.to_gltf_with_runner(bin_uri, &nw_jobs::JobRunner::automatic())
    }

    /// Serialize to glTF using the caller's worker policy for animation channels.
    #[must_use]
    pub fn to_gltf_with_runner(
        &self,
        bin_uri: &str,
        runner: &nw_jobs::JobRunner,
    ) -> (String, Vec<u8>) {
        gltf_pair(
            build(
                self.model,
                self.animations,
                None,
                self.extras,
                &mut |_| None,
                runner,
            ),
            bin_uri,
        )
    }
}

impl<'a> Gltf<'a, WithMaterials<'a>> {
    /// Serialize to `.glb`, resolving each material's textures through `loader`.
    #[must_use]
    pub fn to_glb(&self, mut loader: impl FnMut(&str) -> Option<TextureData>) -> Vec<u8> {
        self.to_glb_with_runner(&nw_jobs::JobRunner::automatic(), &mut loader)
    }

    /// Serialize to GLB using the caller's worker policy for animation channels.
    #[must_use]
    pub fn to_glb_with_runner(
        &self,
        runner: &nw_jobs::JobRunner,
        mut loader: impl FnMut(&str) -> Option<TextureData>,
    ) -> Vec<u8> {
        glb_bytes(build(
            self.model,
            self.animations,
            Some(self.state.0),
            self.extras,
            &mut loader,
            runner,
        ))
    }

    /// Serialize to `.gltf` + `.bin`, resolving textures through `loader`.
    #[must_use]
    pub fn to_gltf(
        &self,
        bin_uri: &str,
        mut loader: impl FnMut(&str) -> Option<TextureData>,
    ) -> (String, Vec<u8>) {
        self.to_gltf_with_runner(bin_uri, &nw_jobs::JobRunner::automatic(), &mut loader)
    }

    /// Serialize to glTF using the caller's worker policy for animation channels.
    #[must_use]
    pub fn to_gltf_with_runner(
        &self,
        bin_uri: &str,
        runner: &nw_jobs::JobRunner,
        mut loader: impl FnMut(&str) -> Option<TextureData>,
    ) -> (String, Vec<u8>) {
        gltf_pair(
            build(
                self.model,
                self.animations,
                Some(self.state.0),
                self.extras,
                &mut loader,
                runner,
            ),
            bin_uri,
        )
    }
}

impl<'a, State> Gltf<'a, State> {
    /// Attach lossless Cry source documents that have no core glTF equivalent.
    #[must_use]
    pub fn extras(mut self, extras: &'a CryAssetExtras) -> Self {
        self.extras = Some(extras);
        self
    }

    /// Attach CAF clips after validating each explicit skeleton binding.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed error for a missing skeleton or a clip whose
    /// controller IDs do not exist in that skeleton.
    pub fn animations(
        mut self,
        animations: &'a [ModelAnimation],
    ) -> Result<Self, GltfAnimationError> {
        if animations.is_empty() {
            self.animations = animations;
            return Ok(self);
        }
        for animation in animations {
            let skeleton = self.model.skeletons.get(animation.skeleton).ok_or(
                GltfAnimationError::MissingSkeleton {
                    index: animation.skeleton,
                },
            )?;
            let mapped = animation.clip.caf.controllers.iter().any(|controller| {
                skeleton
                    .bones
                    .iter()
                    .any(|bone| bone.controller_id == controller.controller_id)
            });
            if !mapped {
                return Err(GltfAnimationError::NoSkeletonChannels {
                    source_path: animation.clip.source_path.clone(),
                    skeleton: animation.skeleton,
                });
            }
        }
        self.animations = animations;
        Ok(self)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GltfAnimationError {
    #[error("animation targets missing model skeleton {index}")]
    MissingSkeleton { index: usize },
    #[error("CAF `{source_path}` has no controllers targeting model skeleton {skeleton}")]
    NoSkeletonChannels {
        source_path: String,
        skeleton: usize,
    },
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
