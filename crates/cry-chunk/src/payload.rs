use arrayvec::{ArrayString, ArrayVec};
use thiserror::Error;

use crate::{ChunkFileError, ChunkType};
mod route;
use route::PayloadParse;

/// Number of mesh stream kinds and per-kind stream slots in chunk descriptors.
pub const STREAM_TYPE_COUNT: usize = 16;
/// Number of stream slots per stream kind in `MeshChunk::stream_chunk_ids`.
pub const STREAM_INDEX_COUNT: usize = 8;

/// Canonical stream kind indexes used by `MeshChunk::stream_chunk_ids`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MeshStreamKind {
    Positions = 0,
    Normals = 1,
    TexCoords = 2,
    Colors = 3,
    Indices = 5,
    Tangents = 6,
    Qtangents = 12,
}

impl MeshStreamKind {
    pub const fn as_index(self) -> usize {
        self as usize
    }
}

const MESH_SUBSETS_BONE_IDS: i32 = 0x0002;
const MESH_SUBSETS_DECOMPRESSED_MATERIAL: i32 = 0x0001;
const MESH_SUBSETS_TEXEL_DENSITY: i32 = 0x0004;
const MESH_SUBSETS_NEW_WORLD_METRICS: i32 = 0x0008;
const MAX_MESH_SUBSETS: usize = 256;
const MAX_BONE_NAMES: usize = 65_536;
const MAX_BONE_NAME_LEN: usize = 256;
const MAX_MORPH_TARGET_NAME_LEN: usize = 1024;
const COMPILED_BONE_DESC_SIZE: usize = 584;
const BONE_ENTITY_SIZE: usize = 152;
const LEGACY_MESH_FLAG_BONE_INFO: u8 = 0x01;
const LEGACY_MESH_FLAG_VERTEX_COLOR: u8 = 0x01;
const LEGACY_MESH_FLAG_VERTEX_ALPHA: u8 = 0x02;
const LEGACY_MESH_FLAG_TOPOLOGY_IDS: u8 = 0x04;
const CONTROLLER_PQ_LOG_KEY_SIZE: usize = 28;
const MAX_CONTROLLER_KEYS: usize = 1_000_000;
const MAX_FOLIAGE_SPINES: usize = 1_000_000;
const MAX_FOLIAGE_VERTICES: usize = 10_000_000;
const CGF_NODE_NAME_LENGTH: usize = 64;
const MAX_MATERIAL_SUB_MATERIALS: usize = 128;

// Several chunk payloads embed bounded `ArrayVec`s (e.g. up to 256 mesh subsets,
// each with a 128-entry bone-id array) so parsing never heap-allocates. That makes
// a few variants intrinsically large; the hottest (`Mesh`) is boxed, and boxing
// the rest would only trade those allocations back in for no real gain.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum ChunkPayload<'a> {
    Mesh(Box<MeshChunk>),
    Helper(HelperChunk),
    VertAnim(VertAnimChunk),
    BoneAnim(BoneAnimChunk),
    BoneNameList(BoneNameListChunk),
    Node(NodeChunk),
    Controller(ControllerChunk<'a>),
    BoneMesh(BoneMeshChunk),
    SourceInfo(SourceInfoChunk),
    ExportMetadata(ExportMetadataChunk),
    MaterialName(MaterialNameChunk),
    Timing(TimingChunk),
    BoneInitialPos(BoneInitialPosChunk),
    MeshMorphTarget(MeshMorphTargetChunk),
    ExportFlags(ExportFlagsChunk),
    DataStream(DataStreamChunk<'a>),
    MeshSubsets(MeshSubsetsChunk),
    MeshPhysicsData(MeshPhysicsDataChunk<'a>),
    CompiledBones(CompiledBonesChunk),
    CompiledPhysicalBones(CompiledPhysicalBonesChunk),
    CompiledMorphTargets(CompiledMorphTargetsChunk),
    CompiledPhysicalProxies(CompiledPhysicalProxiesChunk),
    CompiledIntFaces(CompiledIntFacesChunk),
    CompiledIntSkinVertices(CompiledIntSkinVerticesChunk),
    CompiledExt2IntMap(CompiledExt2IntMapChunk),
    BreakablePhysics(BreakablePhysicsChunk),
    MotionParameters(MotionParametersChunk),
    BonesBoxes(BonesBoxesChunk),
    FoliageInfo(FoliageInfoChunk),
    GlobalAnimationHeaderCaf(GlobalAnimationHeaderCafChunk),
    GlobalAnimationHeaderAim(GlobalAnimationHeaderAimChunk),
    NewWorldChunk(NewWorldChunk),
    DataRef(DataRefChunk),
}

#[derive(Debug, Clone, Copy)]
pub struct MeshChunk {
    pub flags: i32,
    pub flags2: i32,
    pub vertex_count: i32,
    pub index_count: i32,
    pub subset_count: i32,
    pub subsets_chunk_id: i32,
    pub vert_anim_id: i32,
    pub stream_chunk_ids: [[i32; STREAM_INDEX_COUNT]; STREAM_TYPE_COUNT],
    pub physics_data_chunk_ids: [i32; 4],
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    pub tex_mapping_density: f32,
    pub geometric_mean_face_area: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct HelperChunk {
    pub helper_type: i32,
    pub size: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct VertAnimChunk {
    pub geometry_chunk_id: i32,
    pub key_count: i32,
    pub vertex_count: i32,
    pub face_count: i32,
}

#[derive(Debug, Clone)]
pub struct BoneAnimChunk {
    pub bone_count: usize,
    pub bones: Vec<BoneEntity>,
}

#[derive(Debug, Clone)]
pub struct BoneNameListChunk {
    pub entity_count: usize,
    pub names: Vec<ArrayString<MAX_BONE_NAME_LEN>>,
}

#[derive(Debug, Clone)]
pub struct NodeChunk {
    pub name: ArrayString<64>,
    pub object_id: i32,
    pub parent_id: i32,
    pub child_count: i32,
    pub material_chunk_id: i32,
    pub transform: [f32; 16],
    pub position_controller_id: i32,
    pub rotation_controller_id: i32,
    pub scale_controller_id: i32,
    pub properties: String,
}

#[derive(Debug, Clone)]
pub struct SourceInfoChunk {
    pub max_path_len: usize,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct ExportMetadataChunk {
    pub max_json_len: usize,
    pub json: String,
}

#[derive(Debug, Clone)]
pub struct MaterialNameChunk {
    pub name: ArrayString<128>,
    pub sub_material_count: i32,
    pub physicalize_types: Vec<i32>,
    pub sub_material_names: Vec<ArrayString<1024>>,
}

#[derive(Debug, Clone)]
pub enum ControllerChunk<'a> {
    Tcb(ControllerTcbChunk<'a>),
    Uncompressed(ControllerUncompressedChunk<'a>),
    Empty0828,
    Compressed(ControllerCompressedChunk<'a>),
    ControllerDb(ControllerDbChunk<'a>),
}

#[derive(Debug, Clone, Copy)]
pub struct ControllerTcbChunk<'a> {
    pub controller_type: i32,
    pub key_count: usize,
    pub flags: u32,
    pub controller_id: u32,
    pub key_size: usize,
    pub key_data: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct ControllerUncompressedChunk<'a> {
    pub key_count: usize,
    pub controller_id: u32,
    pub flags: u32,
    pub keys: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct ControllerCompressedChunk<'a> {
    pub controller_id: u32,
    pub flags: u32,
    pub rotation: Option<ControllerTrack<'a>>,
    pub rotation_times: Option<ControllerTrack<'a>>,
    pub position: Option<ControllerTrack<'a>>,
    pub position_times: Option<ControllerTrack<'a>>,
    pub position_keys_info: u8,
    pub tracks_aligned: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ControllerDbChunk<'a> {
    pub position_key_count: usize,
    pub rotation_key_count: usize,
    pub time_key_count: usize,
    pub animation_count: usize,
    pub data: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct ControllerTrack<'a> {
    pub format: u8,
    pub key_count: usize,
    pub data: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct BoneMeshChunk {
    pub flags1: u8,
    pub flags2: u8,
    pub vertex_count: usize,
    pub texture_vertex_count: usize,
    pub face_count: usize,
    pub vert_anim_id: i32,
    pub vertices: Vec<BoneMeshVertex>,
    pub faces: Vec<BoneMeshFace>,
    pub topology_ids: Vec<i32>,
    pub texture_vertices: Vec<[f32; 2]>,
    pub vertex_links: Vec<Vec<BoneMeshLink>>,
    pub colors: Vec<[u8; 3]>,
    pub alphas: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct BoneMeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct BoneMeshFace {
    pub indices: [i32; 3],
    pub material_id: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct BoneMeshLink {
    pub bone_id: i32,
    pub offset: [f32; 3],
    pub blending: f32,
}

#[derive(Debug, Clone)]
pub struct TimingChunk {
    pub seconds_per_tick: f32,
    pub ticks_per_frame: i32,
    pub range_name: ArrayString<32>,
    pub range_start: i32,
    pub range_end: i32,
    pub sub_range_count: i32,
}

#[derive(Debug, Clone)]
pub struct BoneInitialPosChunk {
    pub mesh_chunk_id: u32,
    pub matrices: Vec<[[f32; 3]; 4]>,
}

#[derive(Debug, Clone)]
pub struct MeshMorphTargetChunk {
    pub mesh_chunk_id: u32,
    pub vertices: Vec<MeshMorphTargetVertex>,
    pub name: ArrayString<MAX_MORPH_TARGET_NAME_LEN>,
}

#[derive(Debug, Clone, Copy)]
pub struct MeshMorphTargetVertex {
    pub vertex_id: u32,
    pub vertex: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct ExportFlagsChunk {
    pub flags: u32,
    pub rc_version: [u32; 4],
    pub rc_version_string: ArrayString<16>,
    pub asset_author_tool: u32,
    pub author_tool_version: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct DataStreamChunk<'a> {
    pub flags: i32,
    pub stream_type: i32,
    pub stream_index: i32,
    pub element_count: usize,
    pub element_size: usize,
    pub data: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct MeshSubsetsChunk {
    pub flags: i32,
    pub subsets: Vec<MeshSubset>,
    pub decompressed_materials: Vec<i32>,
    pub bone_ids: Vec<MeshSubsetBoneIds>,
    pub texel_densities: Vec<f32>,
    pub new_world_metrics: Vec<[f32; 5]>,
}

#[derive(Debug, Clone, Copy)]
pub struct MeshSubset {
    pub first_index: usize,
    pub num_indices: usize,
    pub first_vertex: usize,
    pub num_vertices: usize,
    pub material_id: i32,
    pub radius: f32,
    pub center: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct MeshSubsetBoneIds {
    pub count: usize,
    pub ids: [u16; 128],
}

#[derive(Debug, Clone, Copy)]
pub struct MeshPhysicsDataChunk<'a> {
    pub flags: i32,
    pub tetrahedra_chunk_id: i32,
    pub physical_data: &'a [u8],
    pub tetrahedra_data: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct CompiledBonesChunk {
    pub bones: Vec<CryBoneDescDataComp>,
}

#[derive(Debug, Clone)]
pub struct CompiledPhysicalBonesChunk {
    pub bones: Vec<BoneEntity>,
}

#[derive(Debug, Clone)]
pub struct CompiledMorphTargetsChunk {
    pub rotated: bool,
    pub targets: Vec<CompiledMorphTarget>,
}

#[derive(Debug, Clone)]
pub struct CompiledMorphTarget {
    pub mesh_id: u32,
    pub name: ArrayString<MAX_MORPH_TARGET_NAME_LEN>,
    pub internal_vertices: Vec<MeshMorphTargetVertex>,
    pub external_vertices: Vec<MeshMorphTargetVertex>,
}

#[derive(Debug, Clone)]
pub struct CompiledPhysicalProxiesChunk {
    pub proxies: Vec<PhysicalProxyChunk>,
}

#[derive(Debug, Clone)]
pub struct PhysicalProxyChunk {
    pub chunk_id: u32,
    pub points: Vec<[f32; 3]>,
    pub indices: Vec<u16>,
    pub materials: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CompiledIntFacesChunk {
    pub faces: Vec<[u16; 3]>,
}

#[derive(Debug, Clone)]
pub struct CompiledIntSkinVerticesChunk {
    pub vertices: Vec<IntSkinVertex>,
}

#[derive(Debug, Clone, Copy)]
pub struct IntSkinVertex {
    pub obsolete0: [f32; 3],
    pub position: [f32; 3],
    pub obsolete2: [f32; 3],
    pub bone_ids: [u16; 4],
    pub weights: [f32; 4],
    pub color: [u8; 4],
}

#[derive(Debug, Clone)]
pub struct CompiledExt2IntMapChunk {
    pub indices: Vec<u16>,
}

#[derive(Debug, Clone, Copy)]
pub struct BreakablePhysicsChunk {
    pub granularity: u32,
    pub mode: i32,
    pub retained_vertices: i32,
    pub retained_tetrahedra: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct MotionParametersChunk {
    pub asset_flags: u32,
    pub compression: u32,
    pub ticks_per_frame: i32,
    pub seconds_per_tick: f32,
    pub start: i32,
    pub end: i32,
    pub move_speed: f32,
    pub turn_speed: f32,
    pub asset_turn: f32,
    pub distance: f32,
    pub slope: f32,
    pub start_location: QuatT,
    pub end_location: QuatT,
    pub left_heel_start: f32,
    pub left_heel_end: f32,
    pub left_toe_start: f32,
    pub left_toe_end: f32,
    pub right_heel_start: f32,
    pub right_heel_end: f32,
    pub right_toe_start: f32,
    pub right_toe_end: f32,
}

#[derive(Debug, Clone)]
pub struct BonesBoxesChunk {
    pub bone_id: i32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub indices: Vec<i16>,
}

#[derive(Debug, Clone)]
pub struct FoliageInfoChunk {
    pub spine_count: usize,
    pub spine_vertex_count: usize,
    pub skinned_vertex_count: usize,
    pub bone_id_count: usize,
    pub spines: Vec<FoliageSpine>,
    pub spine_vertices: Vec<[f32; 3]>,
    pub spine_segment_dimensions: Vec<[f32; 4]>,
    pub dynamics: FoliageDynamics,
    pub bone_mapping: FoliageBoneMapping,
}

#[derive(Debug, Clone, Copy)]
pub struct FoliageSpine {
    pub vertex_count: u8,
    pub length: f32,
    pub average_normal: [f32; 3],
    pub attach_spine: u8,
    pub attach_segment: u8,
}

#[derive(Debug, Clone)]
pub enum FoliageDynamics {
    Defaults,
    Explicit {
        stiffness: Vec<f32>,
        damping: Vec<f32>,
        thickness: Vec<f32>,
    },
}

#[derive(Debug, Clone)]
pub enum FoliageBoneMapping {
    PerVertex {
        mappings: Vec<FoliageVertexBoneMapping>,
        bone_ids: Vec<u16>,
    },
    PerNode(Vec<FoliageNodeBoneMapping>),
}

#[derive(Debug, Clone, Copy)]
pub struct FoliageVertexBoneMapping {
    pub bone_ids: [u8; 4],
    pub weights: [u8; 4],
}

#[derive(Debug, Clone)]
pub struct FoliageNodeBoneMapping {
    pub node_name: ArrayString<CGF_NODE_NAME_LENGTH>,
    pub mappings: Vec<FoliageVertexBoneMapping>,
}

#[derive(Debug, Clone)]
pub struct GlobalAnimationHeaderCafChunk {
    pub flags: u32,
    pub file_path: ArrayString<256>,
    pub file_path_crc32: u32,
    pub file_path_dba_crc32: u32,
    pub left_heel_start: f32,
    pub left_heel_end: f32,
    pub left_toe_start: f32,
    pub left_toe_end: f32,
    pub right_heel_start: f32,
    pub right_heel_end: f32,
    pub right_toe_start: f32,
    pub right_toe_end: f32,
    pub start_sec: f32,
    pub end_sec: f32,
    pub total_duration: f32,
    pub controller_count: u32,
    pub start_location: QuatT,
    pub last_locator_key: QuatT,
    pub velocity: [f32; 3],
    pub distance: f32,
    pub speed: f32,
    pub slope: f32,
    pub turn_speed: f32,
    pub asset_turn: f32,
}

#[derive(Debug, Clone)]
pub struct GlobalAnimationHeaderAimChunk {
    pub flags: u32,
    pub file_path: ArrayString<256>,
    pub file_path_crc32: u32,
    pub start_sec: f32,
    pub end_sec: f32,
    pub total_duration: f32,
    pub animation_token_crc32: u32,
    pub exist_mask: u64,
    pub middle_aim_pose_rotation: [f32; 4],
    pub middle_aim_pose: [f32; 4],
    pub polar_grid: Vec<AimVirtualExample>,
    pub aim_pose_count: u32,
    pub aim_poses: Vec<AimPose>,
}

#[derive(Debug, Clone, Copy)]
pub struct AimVirtualExample {
    pub indices: [u8; 4],
    pub values: [i16; 4],
}

#[derive(Debug, Clone)]
pub struct AimPose {
    pub rotations: Vec<[f32; 4]>,
    pub positions: Vec<[f32; 3]>,
}

#[derive(Debug, Clone, Copy)]
pub struct QuatT {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct DataRefChunk {
    pub flags: u32,
    pub index: u32,
    pub offset: usize,
    pub size: usize,
    pub stride: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct NewWorldChunk {
    pub flags: u32,
    pub byte_counts: [u32; 5],
    pub reserved0: u32,
    pub metrics: [f32; 5],
    pub reserved1: u32,
    pub distance_thresholds: [f32; 5],
    pub reserved2: u32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct CryBonePhysicsComp {
    pub physical_geometry_id: i32,
    pub flags: i32,
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub spring_angle: [f32; 3],
    pub spring_tension: [f32; 3],
    pub damping: [f32; 3],
    pub frame_matrix: [[f32; 3]; 3],
}

#[derive(Debug, Clone)]
pub struct BoneEntity {
    pub bone_id: i32,
    pub parent_id: i32,
    pub child_count: i32,
    pub controller_id: u32,
    pub properties: ArrayString<32>,
    pub physics: CryBonePhysicsComp,
}

#[derive(Debug, Clone)]
pub struct CryBoneDescDataComp {
    pub controller_id: u32,
    pub physics: [CryBonePhysicsComp; 2],
    pub mass: f32,
    pub default_world_to_bone: [[f32; 4]; 3],
    pub default_bone_to_world: [[f32; 4]; 3],
    pub bone_name: ArrayString<256>,
    pub limb_id: i32,
    pub parent_offset: i32,
    pub child_count: u32,
    pub children_offset: i32,
}

#[derive(Debug, Error)]
pub enum ChunkPayloadError {
    #[error(transparent)]
    ChunkFile(#[from] ChunkFileError),
    #[error("unknown Cry chunk type {kind:#06x}")]
    UnknownChunkType { kind: u16 },
    #[error("{chunk_type} has no source-backed payload parser: {reason}")]
    UnsupportedChunkPayload {
        chunk_type: ChunkType,
        reason: &'static str,
    },
    #[error("unsupported {chunk_type} version {version:#06x}")]
    UnsupportedVersion { chunk_type: ChunkType, version: u16 },
    #[error("unsupported controller type {controller_type}")]
    UnsupportedControllerType { controller_type: i32 },
    #[error("unsupported controller {field} format {format}")]
    UnsupportedControllerFormat { field: &'static str, format: u8 },
    #[error("invalid {chunk_type} layout: {reason}")]
    InvalidChunkLayout {
        chunk_type: ChunkType,
        reason: &'static str,
    },
    #[error("unexpected end of {chunk_type} payload")]
    UnexpectedEof { chunk_type: ChunkType },
    #[error("{field} is negative: {value}")]
    NegativeCount { field: &'static str, value: i32 },
    #[error("{field} is too large: {value}, maximum {max}")]
    CountTooLarge {
        field: &'static str,
        value: usize,
        max: usize,
    },
    #[error("{field} byte length {value} is not a multiple of {multiple}")]
    InvalidByteMultiple {
        field: &'static str,
        value: usize,
        multiple: usize,
    },
    #[error("invalid UTF-8 string: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("{chunk_type} left {remaining} undecoded payload bytes")]
    TrailingBytes {
        chunk_type: ChunkType,
        remaining: usize,
    },
    #[error("{chunk_type} padding contains non-zero data")]
    NonZeroPadding { chunk_type: ChunkType },
}

pub(crate) fn decode_payload<'a>(
    chunk_type: ChunkType,
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    route::route(chunk_type).parse(chunk_type, version, bytes)
}

fn decode_controller<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0826 => decode_controller_tcb(bytes),
        0x0827 => decode_controller_uncompressed(bytes, false),
        0x0828 => {
            let reader = PayloadReader::new(ChunkType::Controller, bytes);
            reader.finish()?;
            Ok(ChunkPayload::Controller(ControllerChunk::Empty0828))
        }
        0x0829 => decode_controller_compressed(bytes, false),
        0x0830 => decode_controller_uncompressed(bytes, true),
        0x0831 => decode_controller_compressed(bytes, true),
        0x0905 => decode_controller_db(bytes),
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::Controller,
            version,
        }),
    }
}

fn decode_controller_tcb<'a>(bytes: &'a [u8]) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    let mut reader = PayloadReader::new(ChunkType::Controller, bytes);
    let controller_type = reader.read_i32()?;
    let key_count = reader.read_nonnegative_i32("controller key count")?;
    let flags = reader.read_u32()?;
    let controller_id = reader.read_u32()?;
    let key_size = controller_tcb_key_size(controller_type)?;
    let byte_len = checked_count_bytes(key_count, key_size, "controller key bytes")?;
    let key_data = reader.read_bytes(byte_len)?;
    reader.finish()?;
    Ok(ChunkPayload::Controller(ControllerChunk::Tcb(
        ControllerTcbChunk {
            controller_type,
            key_count,
            flags,
            controller_id,
            key_size,
            key_data,
        },
    )))
}

fn decode_controller_uncompressed<'a>(
    bytes: &'a [u8],
    has_flags: bool,
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    let mut reader = PayloadReader::new(ChunkType::Controller, bytes);
    let key_count = reader.read_u32_as_usize("controller key count")?;
    if key_count > MAX_CONTROLLER_KEYS {
        return Err(ChunkPayloadError::CountTooLarge {
            field: "controller key count",
            value: key_count,
            max: MAX_CONTROLLER_KEYS,
        });
    }
    let controller_id = reader.read_u32()?;
    let flags = if has_flags { reader.read_u32()? } else { 0 };
    let byte_len = checked_count_bytes(
        key_count,
        CONTROLLER_PQ_LOG_KEY_SIZE,
        "controller key bytes",
    )?;
    let keys = reader.read_bytes(byte_len)?;
    reader.finish()?;
    Ok(ChunkPayload::Controller(ControllerChunk::Uncompressed(
        ControllerUncompressedChunk {
            key_count,
            controller_id,
            flags,
            keys,
        },
    )))
}

fn decode_controller_compressed<'a>(
    bytes: &'a [u8],
    has_flags: bool,
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    let mut reader = PayloadReader::new(ChunkType::Controller, bytes);
    let controller_id = reader.read_u32()?;
    let flags = if has_flags { reader.read_u32()? } else { 0 };
    let rotation_key_count = reader.read_u16()? as usize;
    let position_key_count = reader.read_u16()? as usize;
    let rotation_format = reader.read_u8()?;
    let rotation_time_format = reader.read_u8()?;
    let position_format = reader.read_u8()?;
    let position_keys_info = reader.read_u8()?;
    let position_time_format = reader.read_u8()?;
    let tracks_aligned = reader.read_u8()? != 0;
    let alignment = if tracks_aligned { 4 } else { 1 };
    let header_len = if has_flags { 18 } else { 14 };
    reader.read_zero_padding(align_len(header_len, 4) - header_len)?;

    let rotation = read_controller_track(
        &mut reader,
        "rotation",
        rotation_format,
        rotation_key_count,
        checked_count_bytes(
            rotation_key_count,
            controller_rotation_format_size(rotation_format)?,
            "controller rotation bytes",
        )?,
        alignment,
    )?;
    let rotation_times = read_controller_track(
        &mut reader,
        "rotation time",
        rotation_time_format,
        rotation_key_count,
        controller_key_time_size(rotation_time_format, rotation_key_count)?,
        alignment,
    )?;
    let position = read_controller_track(
        &mut reader,
        "position",
        position_format,
        position_key_count,
        checked_count_bytes(
            position_key_count,
            controller_position_format_size(position_format)?,
            "controller position bytes",
        )?,
        alignment,
    )?;
    let position_times = match position_keys_info {
        0 => {
            if position_key_count != 0 && rotation_times.is_none() {
                return Err(ChunkPayloadError::InvalidChunkLayout {
                    chunk_type: ChunkType::Controller,
                    reason: "position keys reuse missing rotation times",
                });
            }
            None
        }
        1 | 2 => read_controller_track(
            &mut reader,
            "position time",
            position_time_format,
            position_key_count,
            controller_key_time_size(position_time_format, position_key_count)?,
            alignment,
        )?,
        _ => {
            return Err(ChunkPayloadError::InvalidChunkLayout {
                chunk_type: ChunkType::Controller,
                reason: "unknown position key-time binding",
            });
        }
    };

    reader.finish()?;
    Ok(ChunkPayload::Controller(ControllerChunk::Compressed(
        ControllerCompressedChunk {
            controller_id,
            flags,
            rotation,
            rotation_times,
            position,
            position_times,
            position_keys_info,
            tracks_aligned,
        },
    )))
}

fn decode_controller_db<'a>(bytes: &'a [u8]) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    let mut reader = PayloadReader::new(ChunkType::Controller, bytes);
    let position_key_count = reader.read_u32_as_usize("controller db position key count")?;
    let rotation_key_count = reader.read_u32_as_usize("controller db rotation key count")?;
    let time_key_count = reader.read_u32_as_usize("controller db time key count")?;
    let animation_count = reader.read_u32_as_usize("controller db animation count")?;
    let data = reader.read_remaining_bytes();
    reader.finish()?;
    Ok(ChunkPayload::Controller(ControllerChunk::ControllerDb(
        ControllerDbChunk {
            position_key_count,
            rotation_key_count,
            time_key_count,
            animation_count,
            data,
        },
    )))
}

fn read_controller_track<'a>(
    reader: &mut PayloadReader<'a>,
    field: &'static str,
    format: u8,
    key_count: usize,
    byte_len: usize,
    alignment: usize,
) -> Result<Option<ControllerTrack<'a>>, ChunkPayloadError> {
    if key_count == 0 {
        return Ok(None);
    }
    if key_count > MAX_CONTROLLER_KEYS {
        return Err(ChunkPayloadError::CountTooLarge {
            field,
            value: key_count,
            max: MAX_CONTROLLER_KEYS,
        });
    }
    reader.align_zero_padding(alignment)?;
    let data = reader.read_bytes(byte_len)?;
    Ok(Some(ControllerTrack {
        format,
        key_count,
        data,
    }))
}

fn controller_tcb_key_size(controller_type: i32) -> Result<usize, ChunkPayloadError> {
    match controller_type {
        0 => Ok(0),
        1 => Ok(CONTROLLER_PQ_LOG_KEY_SIZE),
        2 => Ok(8),
        3 => Ok(16),
        4 => Ok(20),
        5 => Ok(16),
        6 => Ok(40),
        7 => Ok(20),
        8 => Ok(28),
        9 => Ok(36),
        10 => Ok(40),
        _ => Err(ChunkPayloadError::UnsupportedControllerType { controller_type }),
    }
}

fn controller_position_format_size(format: u8) -> Result<usize, ChunkPayloadError> {
    match format {
        0 | 2 => Ok(12),
        _ => Err(ChunkPayloadError::UnsupportedControllerFormat {
            field: "position",
            format,
        }),
    }
}

fn controller_rotation_format_size(format: u8) -> Result<usize, ChunkPayloadError> {
    match format {
        0 | 1 => Ok(16),
        5 => Ok(6),
        6 | 8 => Ok(8),
        _ => Err(ChunkPayloadError::UnsupportedControllerFormat {
            field: "rotation",
            format,
        }),
    }
}

fn controller_key_time_size(format: u8, count: usize) -> Result<usize, ChunkPayloadError> {
    let element_size = match format {
        0 => 4,
        1 => 2,
        2 => 1,
        6 => 2,
        _ => {
            return Err(ChunkPayloadError::UnsupportedControllerFormat {
                field: "key time",
                format,
            });
        }
    };
    checked_count_bytes(count, element_size, "controller key-time bytes")
}

fn checked_count_bytes(
    count: usize,
    element_size: usize,
    field: &'static str,
) -> Result<usize, ChunkPayloadError> {
    count
        .checked_mul(element_size)
        .ok_or(ChunkPayloadError::CountTooLarge {
            field,
            value: count,
            max: usize::MAX / element_size.max(1),
        })
}

const fn align_len(value: usize, alignment: usize) -> usize {
    if alignment <= 1 {
        value
    } else {
        (value + alignment - 1) & !(alignment - 1)
    }
}

fn decode_mesh<'a>(version: u16, bytes: &'a [u8]) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 | 0x0801 => {
            let mut reader = PayloadReader::new(ChunkType::Mesh, bytes);
            let flags = reader.read_i32()?;
            let flags2 = reader.read_i32()?;
            let vertex_count = reader.read_i32()?;
            let index_count = reader.read_i32()?;
            let subset_count = reader.read_i32()?;
            let subsets_chunk_id = reader.read_i32()?;
            let vert_anim_id = reader.read_i32()?;
            let mut stream_chunk_ids = [[0; STREAM_INDEX_COUNT]; STREAM_TYPE_COUNT];
            for stream_ids in &mut stream_chunk_ids {
                stream_ids[0] = reader.read_i32()?;
            }
            let physics_data_chunk_ids = [
                reader.read_i32()?,
                reader.read_i32()?,
                reader.read_i32()?,
                reader.read_i32()?,
            ];
            let bbox_min = reader.read_vec3()?;
            let bbox_max = reader.read_vec3()?;
            let tex_mapping_density = reader.read_f32()?;
            let geometric_mean_face_area = reader.read_f32()?;
            reader.skip(31 * 4)?;
            reader.finish()?;
            Ok(ChunkPayload::Mesh(Box::new(MeshChunk {
                flags,
                flags2,
                vertex_count,
                index_count,
                subset_count,
                subsets_chunk_id,
                vert_anim_id,
                stream_chunk_ids,
                physics_data_chunk_ids,
                bbox_min,
                bbox_max,
                tex_mapping_density,
                geometric_mean_face_area,
            })))
        }
        0x0802 => {
            let mut reader = PayloadReader::new(ChunkType::Mesh, bytes);
            let flags = reader.read_i32()?;
            let flags2 = reader.read_i32()?;
            let vertex_count = reader.read_i32()?;
            let index_count = reader.read_i32()?;
            let subset_count = reader.read_i32()?;
            let subsets_chunk_id = reader.read_i32()?;
            let vert_anim_id = reader.read_i32()?;
            let mut stream_chunk_ids = [[0; STREAM_INDEX_COUNT]; STREAM_TYPE_COUNT];
            for stream_ids in &mut stream_chunk_ids {
                for id in stream_ids {
                    *id = reader.read_i32()?;
                }
            }
            let physics_data_chunk_ids = [
                reader.read_i32()?,
                reader.read_i32()?,
                reader.read_i32()?,
                reader.read_i32()?,
            ];
            let bbox_min = reader.read_vec3()?;
            let bbox_max = reader.read_vec3()?;
            let tex_mapping_density = reader.read_f32()?;
            let geometric_mean_face_area = reader.read_f32()?;
            reader.skip(31 * 4)?;
            reader.finish()?;
            Ok(ChunkPayload::Mesh(Box::new(MeshChunk {
                flags,
                flags2,
                vertex_count,
                index_count,
                subset_count,
                subsets_chunk_id,
                vert_anim_id,
                stream_chunk_ids,
                physics_data_chunk_ids,
                bbox_min,
                bbox_max,
                tex_mapping_density,
                geometric_mean_face_area,
            })))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::Mesh,
            version,
        }),
    }
}

fn decode_bone_mesh<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0744 | 0x0745 => {
            let mut reader = PayloadReader::new(ChunkType::BoneMesh, bytes);
            let flags1 = reader.read_u8()?;
            let flags2 = reader.read_u8()?;
            reader.skip(2)?;
            let vertex_count = reader.read_nonnegative_i32("bone mesh vertex count")?;
            let texture_vertex_count =
                reader.read_nonnegative_i32("bone mesh texture vertex count")?;
            let face_count = reader.read_nonnegative_i32("bone mesh face count")?;
            let vert_anim_id = reader.read_i32()?;

            if texture_vertex_count != 0 && texture_vertex_count != vertex_count {
                return Err(ChunkPayloadError::InvalidChunkLayout {
                    chunk_type: ChunkType::BoneMesh,
                    reason: "texture vertex count must be zero or match vertex count",
                });
            }

            let mut vertices = Vec::with_capacity(vertex_count);
            for _ in 0..vertex_count {
                vertices.push(BoneMeshVertex {
                    position: reader.read_vec3()?,
                    normal: reader.read_vec3()?,
                });
            }

            let mut faces = Vec::with_capacity(face_count);
            for _ in 0..face_count {
                faces.push(BoneMeshFace {
                    indices: [reader.read_i32()?, reader.read_i32()?, reader.read_i32()?],
                    material_id: reader.read_i32()?,
                });
            }

            let mut topology_ids = Vec::new();
            if flags2 & LEGACY_MESH_FLAG_TOPOLOGY_IDS != 0 {
                topology_ids.reserve(vertex_count);
                for _ in 0..vertex_count {
                    topology_ids.push(reader.read_i32()?);
                }
            }

            let mut texture_vertices = Vec::new();
            if texture_vertex_count != 0 {
                texture_vertices.reserve(vertex_count);
                for _ in 0..vertex_count {
                    texture_vertices.push(reader.read_vec2()?);
                }
            }

            let mut vertex_links = Vec::new();
            if flags1 & LEGACY_MESH_FLAG_BONE_INFO != 0 {
                vertex_links.reserve(vertex_count);
                for _ in 0..vertex_count {
                    let link_count = reader.read_nonnegative_i32("bone mesh vertex link count")?;
                    let mut links = Vec::with_capacity(link_count);
                    for _ in 0..link_count {
                        links.push(BoneMeshLink {
                            bone_id: reader.read_i32()?,
                            offset: reader.read_vec3()?,
                            blending: reader.read_f32()?,
                        });
                    }
                    vertex_links.push(links);
                }
            }

            let mut colors = Vec::new();
            if flags2 & LEGACY_MESH_FLAG_VERTEX_COLOR != 0 {
                colors.reserve(vertex_count);
                for _ in 0..vertex_count {
                    colors.push([reader.read_u8()?, reader.read_u8()?, reader.read_u8()?]);
                }
            }

            let mut alphas = Vec::new();
            if flags2 & LEGACY_MESH_FLAG_VERTEX_ALPHA != 0 {
                alphas.reserve(vertex_count);
                for _ in 0..vertex_count {
                    alphas.push(reader.read_u8()?);
                }
            }

            reader.finish()?;
            Ok(ChunkPayload::BoneMesh(BoneMeshChunk {
                flags1,
                flags2,
                vertex_count,
                texture_vertex_count,
                face_count,
                vert_anim_id,
                vertices,
                faces,
                topology_ids,
                texture_vertices,
                vertex_links,
                colors,
                alphas,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::BoneMesh,
            version,
        }),
    }
}

fn decode_helper<'a>(version: u16, bytes: &'a [u8]) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0744 => {
            let mut reader = PayloadReader::new(ChunkType::Helper, bytes);
            let helper_type = reader.read_i32()?;
            let size = reader.read_vec3()?;
            reader.finish()?;
            Ok(ChunkPayload::Helper(HelperChunk { helper_type, size }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::Helper,
            version,
        }),
    }
}

fn decode_vert_anim<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0744 => {
            let mut reader = PayloadReader::new(ChunkType::VertAnim, bytes);
            let geometry_chunk_id = reader.read_i32()?;
            let key_count = reader.read_i32()?;
            let vertex_count = reader.read_i32()?;
            let face_count = reader.read_i32()?;
            reader.finish()?;
            Ok(ChunkPayload::VertAnim(VertAnimChunk {
                geometry_chunk_id,
                key_count,
                vertex_count,
                face_count,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::VertAnim,
            version,
        }),
    }
}

fn decode_bone_anim<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0290 => {
            let mut reader = PayloadReader::new(ChunkType::BoneAnim, bytes);
            let bone_count = reader.read_nonnegative_i32("bone animation bone count")?;
            let mut bones = Vec::with_capacity(bone_count);
            for _ in 0..bone_count {
                bones.push(reader.read_bone_entity()?);
            }
            reader.finish()?;
            Ok(ChunkPayload::BoneAnim(BoneAnimChunk { bone_count, bones }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::BoneAnim,
            version,
        }),
    }
}

fn decode_bone_name_list<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0745 => {
            let mut reader = PayloadReader::new(ChunkType::BoneNameList, bytes);
            let entity_count = reader.read_nonnegative_i32("bone name count")?;
            if entity_count > MAX_BONE_NAMES {
                return Err(ChunkPayloadError::CountTooLarge {
                    field: "bone name count",
                    value: entity_count,
                    max: MAX_BONE_NAMES,
                });
            }
            let mut names = Vec::with_capacity(entity_count);
            for _ in 0..entity_count {
                names.push(reader.read_c_string_bounded::<MAX_BONE_NAME_LEN>("bone name")?);
            }
            reader.finish_zero_padding()?;
            Ok(ChunkPayload::BoneNameList(BoneNameListChunk {
                entity_count,
                names,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::BoneNameList,
            version,
        }),
    }
}

fn decode_source_info<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0001 => {
            let mut reader = PayloadReader::new(ChunkType::SourceInfo, bytes);
            let max_path_len = reader.read_u32_as_usize("source info max path length")?;
            let path = reader.read_c_string_dynamic("source info path", max_path_len)?;
            reader.finish()?;
            Ok(ChunkPayload::SourceInfo(SourceInfoChunk {
                max_path_len,
                path,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::SourceInfo,
            version,
        }),
    }
}

fn decode_export_metadata<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0001 => {
            let mut reader = PayloadReader::new(ChunkType::ExportMetadata, bytes);
            let max_json_len = reader.read_u32_as_usize("export metadata max JSON length")?;
            let json = reader.read_c_string_dynamic("export metadata JSON", max_json_len)?;
            reader.finish()?;
            Ok(ChunkPayload::ExportMetadata(ExportMetadataChunk {
                max_json_len,
                json,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::ExportMetadata,
            version,
        }),
    }
}

fn decode_node<'a>(version: u16, bytes: &'a [u8]) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0823 | 0x0824 => {
            let mut reader = PayloadReader::new(ChunkType::Node, bytes);
            let name = reader.read_fixed_string::<64>()?;
            let object_id = reader.read_i32()?;
            let parent_id = reader.read_i32()?;
            let child_count = reader.read_i32()?;
            let material_chunk_id = reader.read_i32()?;
            reader.skip(4)?;
            let mut transform = [0.0; 16];
            for value in &mut transform {
                *value = reader.read_f32()?;
            }
            reader.skip(10 * 4)?;
            let position_controller_id = reader.read_i32()?;
            let rotation_controller_id = reader.read_i32()?;
            let scale_controller_id = reader.read_i32()?;
            let property_len = reader.read_nonnegative_i32("node property length")?;
            let properties = std::str::from_utf8(reader.read_bytes(property_len)?)?.to_owned();
            reader.finish()?;
            Ok(ChunkPayload::Node(NodeChunk {
                name,
                object_id,
                parent_id,
                child_count,
                material_chunk_id,
                transform,
                position_controller_id,
                rotation_controller_id,
                scale_controller_id,
                properties,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::Node,
            version,
        }),
    }
}

fn decode_material_name<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::MtlName, bytes);
            reader.skip(8)?;
            let name = reader.read_fixed_string::<128>()?;
            let physicalize_type = reader.read_i32()?;
            let sub_material_count = reader.read_i32()?;
            if sub_material_count < 0 {
                return Err(ChunkPayloadError::NegativeCount {
                    field: "material sub-material count",
                    value: sub_material_count,
                });
            }
            if sub_material_count as usize > 32 {
                return Err(ChunkPayloadError::CountTooLarge {
                    field: "legacy material sub-material count",
                    value: sub_material_count as usize,
                    max: 32,
                });
            }
            reader.skip(32 * 4)?;
            reader.skip(4)?;
            reader.skip(4)?;
            reader.skip(32 * 4)?;
            reader.finish()?;
            Ok(ChunkPayload::MaterialName(MaterialNameChunk {
                name,
                sub_material_count,
                physicalize_types: vec![physicalize_type],
                sub_material_names: Vec::new(),
            }))
        }
        0x0802 => {
            let mut reader = PayloadReader::new(ChunkType::MtlName, bytes);
            let name = reader.read_fixed_string::<128>()?;
            let sub_material_count = reader.read_i32()?;
            decode_material_name_slots(reader, name, sub_material_count)
        }
        0x0804 => {
            // New World layout: 64-byte name, child count, `count` sub-material
            // chunk ids (skipped), then `count` variable-length child names.
            let mut reader = PayloadReader::new(ChunkType::MtlName, bytes);
            let short_name = reader.read_fixed_string::<64>()?;
            let mut name = ArrayString::<128>::new();
            name.push_str(&short_name);
            let sub_material_count = reader.read_i32()?;
            if sub_material_count < 0 {
                return Err(ChunkPayloadError::NegativeCount {
                    field: "material sub-material count",
                    value: sub_material_count,
                });
            }
            let count = sub_material_count as usize;
            if count > MAX_MATERIAL_SUB_MATERIALS {
                return Err(ChunkPayloadError::CountTooLarge {
                    field: "material sub-material count",
                    value: count,
                    max: MAX_MATERIAL_SUB_MATERIALS,
                });
            }
            reader.skip(4 * count)?;
            let mut sub_material_names = Vec::with_capacity(count);
            for _ in 0..count {
                sub_material_names.push(reader.read_c_string()?);
            }
            reader.finish()?;
            Ok(ChunkPayload::MaterialName(MaterialNameChunk {
                name,
                sub_material_count,
                physicalize_types: Vec::new(),
                sub_material_names,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::MtlName,
            version,
        }),
    }
}

fn decode_material_name_slots<'a>(
    mut reader: PayloadReader<'a>,
    name: ArrayString<128>,
    sub_material_count: i32,
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    if sub_material_count > MAX_MATERIAL_SUB_MATERIALS as i32 {
        return Err(ChunkPayloadError::CountTooLarge {
            field: "material sub-material count",
            value: sub_material_count as usize,
            max: MAX_MATERIAL_SUB_MATERIALS,
        });
    }

    let physicalize_count = if sub_material_count <= 0 {
        1
    } else {
        sub_material_count as usize
    };
    let mut physicalize_types = Vec::with_capacity(physicalize_count);
    for _ in 0..physicalize_count {
        physicalize_types.push(reader.read_i32()?);
    }

    let name_count = if sub_material_count <= 0 {
        0
    } else {
        sub_material_count as usize
    };
    let mut sub_material_names = Vec::with_capacity(name_count);
    for _ in 0..name_count {
        sub_material_names.push(reader.read_c_string()?);
    }

    reader.finish()?;
    Ok(ChunkPayload::MaterialName(MaterialNameChunk {
        name,
        sub_material_count,
        physicalize_types,
        sub_material_names,
    }))
}

fn decode_export_flags<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0001 => {
            let mut reader = PayloadReader::new(ChunkType::ExportFlags, bytes);
            let flags = reader.read_u32()?;
            let rc_version = [
                reader.read_u32()?,
                reader.read_u32()?,
                reader.read_u32()?,
                reader.read_u32()?,
            ];
            let rc_version_string = reader.read_fixed_string::<16>()?;
            let asset_author_tool = reader.read_u32()?;
            let author_tool_version = reader.read_u32()?;
            reader.skip(30 * 4)?;
            reader.finish()?;
            Ok(ChunkPayload::ExportFlags(ExportFlagsChunk {
                flags,
                rc_version,
                rc_version_string,
                asset_author_tool,
                author_tool_version,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::ExportFlags,
            version,
        }),
    }
}

fn decode_data_stream<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 | 0x0801 => {
            let mut reader = PayloadReader::new(ChunkType::DataStream, bytes);
            let flags = reader.read_i32()?;
            let stream_type = reader.read_i32()?;
            let stream_index = if version == 0x0801 {
                reader.read_i32()?
            } else {
                0
            };
            let element_count = reader.read_nonnegative_i32("stream element count")?;
            let element_size = reader.read_nonnegative_i32("stream element size")?;
            reader.skip(8)?;
            let byte_len = element_count.checked_mul(element_size).ok_or(
                ChunkPayloadError::CountTooLarge {
                    field: "stream byte length",
                    value: usize::MAX,
                    max: usize::MAX - 1,
                },
            )?;
            let data = reader.read_bytes(byte_len)?;
            reader.finish()?;
            Ok(ChunkPayload::DataStream(DataStreamChunk {
                flags,
                stream_type,
                stream_index,
                element_count,
                element_size,
                data,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::DataStream,
            version,
        }),
    }
}

fn decode_mesh_subsets<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::MeshSubsets, bytes);
            let flags = reader.read_i32()?;
            let count = reader.read_nonnegative_i32("mesh subset count")?;
            if count > MAX_MESH_SUBSETS {
                return Err(ChunkPayloadError::CountTooLarge {
                    field: "mesh subset count",
                    value: count,
                    max: MAX_MESH_SUBSETS,
                });
            }
            reader.skip(8)?;
            let mut subsets = Vec::new();
            for _ in 0..count {
                subsets.push_payload(
                    "mesh subset count",
                    MeshSubset {
                        first_index: reader.read_nonnegative_i32("subset first index")?,
                        num_indices: reader.read_nonnegative_i32("subset index count")?,
                        first_vertex: reader.read_nonnegative_i32("subset first vertex")?,
                        num_vertices: reader.read_nonnegative_i32("subset vertex count")?,
                        material_id: reader.read_i32()?,
                        radius: reader.read_f32()?,
                        center: reader.read_vec3()?,
                    },
                )?;
            }

            let mut decompressed_materials = Vec::new();
            if flags & MESH_SUBSETS_DECOMPRESSED_MATERIAL != 0 {
                for _ in 0..count {
                    decompressed_materials
                        .push_payload("mesh subset decompressed materials", reader.read_i32()?)?;
                }
            }

            let mut bone_ids = Vec::new();
            if flags & MESH_SUBSETS_BONE_IDS != 0 {
                for _ in 0..count {
                    let bone_count = reader.read_nonnegative_i32("subset bone id count")?;
                    if bone_count > 128 {
                        return Err(ChunkPayloadError::CountTooLarge {
                            field: "subset bone id count",
                            value: bone_count,
                            max: 128,
                        });
                    }
                    let mut ids = [0; 128];
                    for id in &mut ids {
                        *id = reader.read_u16()?;
                    }
                    bone_ids.push_payload(
                        "mesh subset bone ids",
                        MeshSubsetBoneIds {
                            count: bone_count,
                            ids,
                        },
                    )?;
                }
            }

            let mut texel_densities = Vec::new();
            if flags & MESH_SUBSETS_TEXEL_DENSITY != 0 {
                for _ in 0..count {
                    texel_densities
                        .push_payload("mesh subset texel densities", reader.read_f32()?)?;
                }
            }

            let mut new_world_metrics = Vec::new();
            if flags & MESH_SUBSETS_NEW_WORLD_METRICS != 0 {
                for _ in 0..count {
                    new_world_metrics.push_payload(
                        "mesh subset New World metrics",
                        [
                            reader.read_f32()?,
                            reader.read_f32()?,
                            reader.read_f32()?,
                            reader.read_f32()?,
                            reader.read_f32()?,
                        ],
                    )?;
                }
            }

            reader.finish()?;
            Ok(ChunkPayload::MeshSubsets(MeshSubsetsChunk {
                flags,
                subsets,
                decompressed_materials,
                bone_ids,
                texel_densities,
                new_world_metrics,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::MeshSubsets,
            version,
        }),
    }
}

fn decode_mesh_physics_data<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::MeshPhysicsData, bytes);
            let data_size = reader.read_nonnegative_i32("mesh physics data size")?;
            let flags = reader.read_i32()?;
            let tetrahedra_data_size =
                reader.read_nonnegative_i32("mesh physics tetrahedra data size")?;
            let tetrahedra_chunk_id = reader.read_i32()?;
            reader.skip(8)?;
            let physical_data = reader.read_bytes(data_size)?;
            let tetrahedra_data = reader.read_bytes(tetrahedra_data_size)?;
            reader.finish()?;
            Ok(ChunkPayload::MeshPhysicsData(MeshPhysicsDataChunk {
                flags,
                tetrahedra_chunk_id,
                physical_data,
                tetrahedra_data,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::MeshPhysicsData,
            version,
        }),
    }
}

fn decode_compiled_bones<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::CompiledBones, bytes);
            reader.skip(32)?;
            let count = reader.remaining_multiple("compiled bone data", COMPILED_BONE_DESC_SIZE)?;
            let mut bones = Vec::with_capacity(count);
            for _ in 0..count {
                bones.push(reader.read_bone_desc_data_comp()?);
            }
            reader.finish()?;
            Ok(ChunkPayload::CompiledBones(CompiledBonesChunk { bones }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::CompiledBones,
            version,
        }),
    }
}

fn decode_compiled_physical_bones<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::CompiledPhysicalBones, bytes);
            reader.skip(32)?;
            let count =
                reader.remaining_multiple("compiled physical bone data", BONE_ENTITY_SIZE)?;
            let mut bones = Vec::with_capacity(count);
            for _ in 0..count {
                bones.push(reader.read_bone_entity()?);
            }
            reader.finish()?;
            Ok(ChunkPayload::CompiledPhysicalBones(
                CompiledPhysicalBonesChunk { bones },
            ))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::CompiledPhysicalBones,
            version,
        }),
    }
}

fn decode_compiled_morph_targets<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 | 0x0801 => {
            let mut reader = PayloadReader::new(ChunkType::CompiledMorphTargets, bytes);
            let count = reader.read_u32_as_usize("compiled morph target count")?;
            let mut targets = Vec::with_capacity(count);
            for _ in 0..count {
                let mesh_id = reader.read_u32()?;
                let name_len = reader.read_u32_as_usize("compiled morph target name length")?;
                if name_len > MAX_MORPH_TARGET_NAME_LEN {
                    return Err(ChunkPayloadError::CountTooLarge {
                        field: "compiled morph target name length",
                        value: name_len,
                        max: MAX_MORPH_TARGET_NAME_LEN,
                    });
                }
                let internal_count =
                    reader.read_u32_as_usize("compiled morph target internal vertex count")?;
                let external_count =
                    reader.read_u32_as_usize("compiled morph target external vertex count")?;
                let name_bytes = reader.read_bytes(name_len)?;
                let name = reader.read_array_string::<MAX_MORPH_TARGET_NAME_LEN>(
                    name_bytes,
                    "compiled morph target name",
                )?;
                let mut internal_vertices = Vec::with_capacity(internal_count);
                for _ in 0..internal_count {
                    internal_vertices.push(reader.read_morph_target_vertex()?);
                }
                let mut external_vertices = Vec::with_capacity(external_count);
                for _ in 0..external_count {
                    external_vertices.push(reader.read_morph_target_vertex()?);
                }
                targets.push(CompiledMorphTarget {
                    mesh_id,
                    name,
                    internal_vertices,
                    external_vertices,
                });
            }
            reader.finish()?;
            Ok(ChunkPayload::CompiledMorphTargets(
                CompiledMorphTargetsChunk {
                    rotated: version == 0x0801,
                    targets,
                },
            ))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::CompiledMorphTargets,
            version,
        }),
    }
}

fn decode_compiled_physical_proxies<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::CompiledPhysicalProxies, bytes);
            let count = reader.read_u32_as_usize("compiled physical proxy count")?;
            let mut proxies = Vec::with_capacity(count);
            for _ in 0..count {
                let chunk_id = reader.read_u32()?;
                let point_count = reader.read_u32_as_usize("physical proxy point count")?;
                let index_count = reader.read_u32_as_usize("physical proxy index count")?;
                let material_count = reader.read_u32_as_usize("physical proxy material count")?;
                let mut points = Vec::with_capacity(point_count);
                for _ in 0..point_count {
                    points.push(reader.read_vec3()?);
                }
                let mut indices = Vec::with_capacity(index_count);
                for _ in 0..index_count {
                    indices.push(reader.read_u16()?);
                }
                let materials = reader.read_bytes(material_count)?.to_vec();
                proxies.push(PhysicalProxyChunk {
                    chunk_id,
                    points,
                    indices,
                    materials,
                });
            }
            reader.finish()?;
            Ok(ChunkPayload::CompiledPhysicalProxies(
                CompiledPhysicalProxiesChunk { proxies },
            ))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::CompiledPhysicalProxies,
            version,
        }),
    }
}

fn decode_compiled_int_faces<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::CompiledIntFaces, bytes);
            let count = reader.remaining_multiple("compiled int faces", 6)?;
            let mut faces = Vec::with_capacity(count);
            for _ in 0..count {
                faces.push([reader.read_u16()?, reader.read_u16()?, reader.read_u16()?]);
            }
            reader.finish()?;
            Ok(ChunkPayload::CompiledIntFaces(CompiledIntFacesChunk {
                faces,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::CompiledIntFaces,
            version,
        }),
    }
}

fn decode_compiled_int_skin_vertices<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::CompiledIntSkinVertices, bytes);
            reader.skip(32)?;
            let count = reader.remaining_multiple("compiled int skin vertices", 64)?;
            let mut vertices = Vec::with_capacity(count);
            for _ in 0..count {
                vertices.push(IntSkinVertex {
                    obsolete0: reader.read_vec3()?,
                    position: reader.read_vec3()?,
                    obsolete2: reader.read_vec3()?,
                    bone_ids: [
                        reader.read_u16()?,
                        reader.read_u16()?,
                        reader.read_u16()?,
                        reader.read_u16()?,
                    ],
                    weights: [
                        reader.read_f32()?,
                        reader.read_f32()?,
                        reader.read_f32()?,
                        reader.read_f32()?,
                    ],
                    color: [
                        reader.read_u8()?,
                        reader.read_u8()?,
                        reader.read_u8()?,
                        reader.read_u8()?,
                    ],
                });
            }
            reader.finish()?;
            Ok(ChunkPayload::CompiledIntSkinVertices(
                CompiledIntSkinVerticesChunk { vertices },
            ))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::CompiledIntSkinVertices,
            version,
        }),
    }
}

fn decode_compiled_ext2int_map<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::CompiledExt2IntMap, bytes);
            let count = reader.remaining_multiple("compiled ext2int map", 2)?;
            let mut indices = Vec::with_capacity(count);
            for _ in 0..count {
                indices.push(reader.read_u16()?);
            }
            reader.finish()?;
            Ok(ChunkPayload::CompiledExt2IntMap(CompiledExt2IntMapChunk {
                indices,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::CompiledExt2IntMap,
            version,
        }),
    }
}

fn decode_breakable_physics<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0001 => {
            let mut reader = PayloadReader::new(ChunkType::BreakablePhysics, bytes);
            let granularity = reader.read_u32()?;
            let mode = reader.read_i32()?;
            let retained_vertices = reader.read_i32()?;
            let retained_tetrahedra = reader.read_i32()?;
            reader.skip(10 * 4)?;
            reader.finish()?;
            Ok(ChunkPayload::BreakablePhysics(BreakablePhysicsChunk {
                granularity,
                mode,
                retained_vertices,
                retained_tetrahedra,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::BreakablePhysics,
            version,
        }),
    }
}

fn decode_foliage_info<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0001 | 0x0002 => {
            let mut reader = PayloadReader::new(ChunkType::FoliageInfo, bytes);
            let spine_count =
                read_bounded_i32(&mut reader, "foliage spine count", MAX_FOLIAGE_SPINES)?;
            let spine_vertex_count = read_bounded_i32(
                &mut reader,
                "foliage spine vertex count",
                MAX_FOLIAGE_VERTICES,
            )?;
            let skinned_vertex_count = read_bounded_i32(
                &mut reader,
                "foliage skinned vertex count",
                MAX_FOLIAGE_VERTICES,
            )?;
            let bone_id_count =
                read_bounded_i32(&mut reader, "foliage bone id count", MAX_FOLIAGE_VERTICES)?;

            let mut spines = Vec::with_capacity(spine_count);
            for _ in 0..spine_count {
                spines.push(FoliageSpine {
                    vertex_count: reader.read_u8()?,
                    length: {
                        reader.skip(3)?;
                        reader.read_f32()?
                    },
                    average_normal: reader.read_vec3()?,
                    attach_spine: reader.read_u8()?,
                    attach_segment: reader.read_u8()?,
                });
                reader.skip(2)?;
            }

            let mut spine_vertices = Vec::with_capacity(spine_vertex_count);
            for _ in 0..spine_vertex_count {
                spine_vertices.push(reader.read_vec3()?);
            }

            let mut spine_segment_dimensions = Vec::with_capacity(spine_vertex_count);
            for _ in 0..spine_vertex_count {
                spine_segment_dimensions.push(reader.read_vec4()?);
            }

            let dynamics = if version == 0x0001 {
                FoliageDynamics::Defaults
            } else {
                let mut stiffness = Vec::with_capacity(spine_vertex_count);
                let mut damping = Vec::with_capacity(spine_vertex_count);
                let mut thickness = Vec::with_capacity(spine_vertex_count);
                for _ in 0..spine_vertex_count {
                    stiffness.push(reader.read_f32()?);
                }
                for _ in 0..spine_vertex_count {
                    damping.push(reader.read_f32()?);
                }
                for _ in 0..spine_vertex_count {
                    thickness.push(reader.read_f32()?);
                }
                FoliageDynamics::Explicit {
                    stiffness,
                    damping,
                    thickness,
                }
            };

            let bone_mapping =
                decode_foliage_bone_mapping(&mut reader, skinned_vertex_count, bone_id_count)?;

            reader.finish()?;
            Ok(ChunkPayload::FoliageInfo(FoliageInfoChunk {
                spine_count,
                spine_vertex_count,
                skinned_vertex_count,
                bone_id_count,
                spines,
                spine_vertices,
                spine_segment_dimensions,
                dynamics,
                bone_mapping,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::FoliageInfo,
            version,
        }),
    }
}

fn decode_foliage_bone_mapping(
    reader: &mut PayloadReader<'_>,
    skinned_vertex_count: usize,
    bone_id_count: usize,
) -> Result<FoliageBoneMapping, ChunkPayloadError> {
    let per_vertex_len = checked_count_bytes(
        skinned_vertex_count,
        8,
        "foliage per-vertex bone mapping bytes",
    )?;
    if bone_id_count != 0 || reader.remaining() == per_vertex_len {
        let mut mappings = Vec::with_capacity(skinned_vertex_count);
        for _ in 0..skinned_vertex_count {
            mappings.push(reader.read_foliage_vertex_bone_mapping()?);
        }
        let mut bone_ids = Vec::with_capacity(bone_id_count);
        for _ in 0..bone_id_count {
            bone_ids.push(reader.read_u16()?);
        }
        return Ok(FoliageBoneMapping::PerVertex { mappings, bone_ids });
    }

    let mapping_count = reader.read_nonnegative_i32("foliage node bone mapping count")?;
    let mut mappings = Vec::with_capacity(mapping_count);
    for _ in 0..mapping_count {
        let node_name = reader.read_fixed_string::<CGF_NODE_NAME_LENGTH>()?;
        let vertex_count = reader.read_nonnegative_i32("foliage node bone mapping vertex count")?;
        let mut node_mappings = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            node_mappings.push(reader.read_foliage_vertex_bone_mapping()?);
        }
        mappings.push(FoliageNodeBoneMapping {
            node_name,
            mappings: node_mappings,
        });
    }
    Ok(FoliageBoneMapping::PerNode(mappings))
}

fn read_bounded_i32(
    reader: &mut PayloadReader<'_>,
    field: &'static str,
    max: usize,
) -> Result<usize, ChunkPayloadError> {
    let value = reader.read_nonnegative_i32(field)?;
    if value > max {
        return Err(ChunkPayloadError::CountTooLarge { field, value, max });
    }
    Ok(value)
}

fn decode_timing<'a>(version: u16, bytes: &'a [u8]) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0918 => {
            let mut reader = PayloadReader::new(ChunkType::Timing, bytes);
            let seconds_per_tick = reader.read_f32()?;
            let ticks_per_frame = reader.read_i32()?;
            let range_name = reader.read_fixed_string::<32>()?;
            let range_start = reader.read_i32()?;
            let range_end = reader.read_i32()?;
            let sub_range_count = reader.read_i32()?;
            reader.finish()?;
            Ok(ChunkPayload::Timing(TimingChunk {
                seconds_per_tick,
                ticks_per_frame,
                range_name,
                range_start,
                range_end,
                sub_range_count,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::Timing,
            version,
        }),
    }
}

fn decode_bone_initial_pos<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0001 => {
            let mut reader = PayloadReader::new(ChunkType::BoneInitialPos, bytes);
            let mesh_chunk_id = reader.read_u32()?;
            let count = reader.read_u32_as_usize("bone initial pose matrix count")?;
            let mut matrices = Vec::with_capacity(count);
            for _ in 0..count {
                matrices.push([
                    reader.read_vec3()?,
                    reader.read_vec3()?,
                    reader.read_vec3()?,
                    reader.read_vec3()?,
                ]);
            }
            reader.finish()?;
            Ok(ChunkPayload::BoneInitialPos(BoneInitialPosChunk {
                mesh_chunk_id,
                matrices,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::BoneInitialPos,
            version,
        }),
    }
}

fn decode_mesh_morph_target<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0001 => {
            let mut reader = PayloadReader::new(ChunkType::MeshMorphTarget, bytes);
            let mesh_chunk_id = reader.read_u32()?;
            let count = reader.read_u32_as_usize("mesh morph target vertex count")?;
            let mut vertices = Vec::with_capacity(count);
            for _ in 0..count {
                vertices.push(reader.read_morph_target_vertex()?);
            }
            let name = reader
                .read_c_string_bounded::<MAX_MORPH_TARGET_NAME_LEN>("mesh morph target name")?;
            reader.finish_zero_padding()?;
            Ok(ChunkPayload::MeshMorphTarget(MeshMorphTargetChunk {
                mesh_chunk_id,
                vertices,
                name,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::MeshMorphTarget,
            version,
        }),
    }
}

fn decode_motion_parameters<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0925 => {
            let mut reader = PayloadReader::new(ChunkType::MotionParameters, bytes);
            let chunk = MotionParametersChunk {
                asset_flags: reader.read_u32()?,
                compression: reader.read_u32()?,
                ticks_per_frame: reader.read_i32()?,
                seconds_per_tick: reader.read_f32()?,
                start: reader.read_i32()?,
                end: reader.read_i32()?,
                move_speed: reader.read_f32()?,
                turn_speed: reader.read_f32()?,
                asset_turn: reader.read_f32()?,
                distance: reader.read_f32()?,
                slope: reader.read_f32()?,
                start_location: reader.read_quat_t()?,
                end_location: reader.read_quat_t()?,
                left_heel_start: reader.read_f32()?,
                left_heel_end: reader.read_f32()?,
                left_toe_start: reader.read_f32()?,
                left_toe_end: reader.read_f32()?,
                right_heel_start: reader.read_f32()?,
                right_heel_end: reader.read_f32()?,
                right_toe_start: reader.read_f32()?,
                right_toe_end: reader.read_f32()?,
            };
            reader.finish()?;
            Ok(ChunkPayload::MotionParameters(chunk))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::MotionParameters,
            version,
        }),
    }
}

fn decode_bones_boxes<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 | 0x0801 => {
            let mut reader = PayloadReader::new(ChunkType::BonesBoxes, bytes);
            let bone_id = reader.read_i32()?;
            let bounds_min = reader.read_vec3()?;
            let bounds_max = reader.read_vec3()?;
            let index_count = reader.read_nonnegative_i32("bone box index count")?;
            let mut indices = Vec::with_capacity(index_count);
            for _ in 0..index_count {
                indices.push(reader.read_i16()?);
            }
            reader.finish()?;
            Ok(ChunkPayload::BonesBoxes(BonesBoxesChunk {
                bone_id,
                bounds_min,
                bounds_max,
                indices,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::BonesBoxes,
            version,
        }),
    }
}

fn decode_global_animation_header_caf<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0971 => {
            let mut reader = PayloadReader::new(ChunkType::GlobalAnimationHeaderCaf, bytes);
            let chunk = GlobalAnimationHeaderCafChunk {
                flags: reader.read_u32()?,
                file_path: reader.read_fixed_string::<256>()?,
                file_path_crc32: reader.read_u32()?,
                file_path_dba_crc32: reader.read_u32()?,
                left_heel_start: reader.read_f32()?,
                left_heel_end: reader.read_f32()?,
                left_toe_start: reader.read_f32()?,
                left_toe_end: reader.read_f32()?,
                right_heel_start: reader.read_f32()?,
                right_heel_end: reader.read_f32()?,
                right_toe_start: reader.read_f32()?,
                right_toe_end: reader.read_f32()?,
                start_sec: reader.read_f32()?,
                end_sec: reader.read_f32()?,
                total_duration: reader.read_f32()?,
                controller_count: reader.read_u32()?,
                start_location: reader.read_quat_t()?,
                last_locator_key: reader.read_quat_t()?,
                velocity: reader.read_vec3()?,
                distance: reader.read_f32()?,
                speed: reader.read_f32()?,
                slope: reader.read_f32()?,
                turn_speed: reader.read_f32()?,
                asset_turn: reader.read_f32()?,
            };
            reader.finish()?;
            Ok(ChunkPayload::GlobalAnimationHeaderCaf(chunk))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::GlobalAnimationHeaderCaf,
            version,
        }),
    }
}

fn decode_global_animation_header_aim<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0970 => {
            let mut reader = PayloadReader::new(ChunkType::GlobalAnimationHeaderAim, bytes);
            let flags = reader.read_u32()?;
            let file_path = reader.read_fixed_string::<256>()?;
            let file_path_crc32 = reader.read_u32()?;
            let start_sec = reader.read_f32()?;
            let end_sec = reader.read_f32()?;
            let total_duration = reader.read_f32()?;
            let animation_token_crc32 = reader.read_u32()?;
            let exist_mask = reader.read_u64()?;
            let middle_aim_pose_rotation = reader.read_quat()?;
            let middle_aim_pose = reader.read_quat()?;
            let mut polar_grid = Vec::with_capacity(17 * 9);
            for _ in 0..(17 * 9) {
                polar_grid.push(AimVirtualExample {
                    indices: [
                        reader.read_u8()?,
                        reader.read_u8()?,
                        reader.read_u8()?,
                        reader.read_u8()?,
                    ],
                    values: [
                        reader.read_i16()?,
                        reader.read_i16()?,
                        reader.read_i16()?,
                        reader.read_i16()?,
                    ],
                });
            }
            let aim_pose_count = reader.read_u32()?;
            let aim_pose_count_usize =
                usize::try_from(aim_pose_count).map_err(|_| ChunkPayloadError::CountTooLarge {
                    field: "aim pose count",
                    value: usize::MAX,
                    max: usize::MAX - 1,
                })?;
            if !matches!(aim_pose_count_usize, 9 | 15 | 27) {
                return Err(ChunkPayloadError::InvalidChunkLayout {
                    chunk_type: ChunkType::GlobalAnimationHeaderAim,
                    reason: "AIM pose count must be 9, 15, or 27",
                });
            }
            let mut aim_poses = Vec::with_capacity(aim_pose_count_usize);
            for _ in 0..aim_pose_count_usize {
                let rotation_count = reader.read_u32_as_usize("aim pose rotation count")?;
                let max_rotation_count = reader.remaining().saturating_sub(4) / 16;
                if rotation_count > max_rotation_count {
                    return Err(ChunkPayloadError::CountTooLarge {
                        field: "aim pose rotation count",
                        value: rotation_count,
                        max: max_rotation_count,
                    });
                }
                let mut rotations = Vec::with_capacity(rotation_count);
                for _ in 0..rotation_count {
                    rotations.push(reader.read_quat()?);
                }

                let position_count = reader.read_u32_as_usize("aim pose position count")?;
                let max_position_count = reader.remaining() / 12;
                if position_count > max_position_count {
                    return Err(ChunkPayloadError::CountTooLarge {
                        field: "aim pose position count",
                        value: position_count,
                        max: max_position_count,
                    });
                }
                let mut positions = Vec::with_capacity(position_count);
                for _ in 0..position_count {
                    positions.push(reader.read_vec3()?);
                }

                aim_poses.push(AimPose {
                    rotations,
                    positions,
                });
            }
            reader.finish()?;
            Ok(ChunkPayload::GlobalAnimationHeaderAim(
                GlobalAnimationHeaderAimChunk {
                    flags,
                    file_path,
                    file_path_crc32,
                    start_sec,
                    end_sec,
                    total_duration,
                    animation_token_crc32,
                    exist_mask,
                    middle_aim_pose_rotation,
                    middle_aim_pose,
                    polar_grid,
                    aim_pose_count,
                    aim_poses,
                },
            ))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::GlobalAnimationHeaderAim,
            version,
        }),
    }
}

fn decode_data_ref<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0800 => {
            let mut reader = PayloadReader::new(ChunkType::DataRef, bytes);
            let flags = reader.read_u32()?;
            let index = reader.read_u32()?;
            let offset = reader.read_u32()? as usize;
            let size = reader.read_u32()? as usize;
            let stride = reader.read_u32()? as usize;
            // New World DataRef chunks carry a 12-byte reserved tail (three zero
            // u32s) after the five fields, so the payload is 32 bytes. Consume
            // whatever remains rather than rejecting it.
            let reserved = reader.remaining();
            reader.skip(reserved)?;
            reader.finish()?;
            Ok(ChunkPayload::DataRef(DataRefChunk {
                flags,
                index,
                offset,
                size,
                stride,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::DataRef,
            version,
        }),
    }
}

trait BoundedArrayVecPush<T> {
    fn push_payload(&mut self, field: &'static str, value: T) -> Result<(), ChunkPayloadError>;
}

impl<T, const N: usize> BoundedArrayVecPush<T> for ArrayVec<T, N> {
    fn push_payload(&mut self, field: &'static str, value: T) -> Result<(), ChunkPayloadError> {
        let next_len = self.len() + 1;
        self.try_push(value)
            .map_err(|_| ChunkPayloadError::CountTooLarge {
                field,
                value: next_len,
                max: N,
            })
    }
}

// Heap-backed payloads (e.g. mesh subsets) keep the chunk structs small; their
// upper bound is enforced explicitly by the decoder before any push, so this push
// is infallible.
impl<T> BoundedArrayVecPush<T> for Vec<T> {
    fn push_payload(&mut self, _field: &'static str, value: T) -> Result<(), ChunkPayloadError> {
        self.push(value);
        Ok(())
    }
}

fn decode_newworld_chunk<'a>(
    version: u16,
    bytes: &'a [u8],
) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
    match version {
        0x0001 => {
            let mut reader = PayloadReader::new(ChunkType::NewWorldUnknown, bytes);
            let flags = reader.read_u32()?;
            let byte_counts = [
                reader.read_u32()?,
                reader.read_u32()?,
                reader.read_u32()?,
                reader.read_u32()?,
                reader.read_u32()?,
            ];
            let reserved0 = reader.read_u32()?;
            let metrics = [
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
            ];
            let reserved1 = reader.read_u32()?;
            let distance_thresholds = [
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
            ];
            let reserved2 = reader.read_u32()?;
            let bounds_min = reader.read_vec3()?;
            let bounds_max = reader.read_vec3()?;
            reader.finish()?;
            Ok(ChunkPayload::NewWorldChunk(NewWorldChunk {
                flags,
                byte_counts,
                reserved0,
                metrics,
                reserved1,
                distance_thresholds,
                reserved2,
                bounds_min,
                bounds_max,
            }))
        }
        _ => Err(ChunkPayloadError::UnsupportedVersion {
            chunk_type: ChunkType::NewWorldUnknown,
            version,
        }),
    }
}

#[derive(Debug)]
struct PayloadReader<'a> {
    chunk_type: ChunkType,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> PayloadReader<'a> {
    const fn new(chunk_type: ChunkType, bytes: &'a [u8]) -> Self {
        Self {
            chunk_type,
            bytes,
            pos: 0,
        }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    fn remaining_multiple(
        &self,
        field: &'static str,
        size: usize,
    ) -> Result<usize, ChunkPayloadError> {
        let remaining = self.remaining();
        if !remaining.is_multiple_of(size) {
            return Err(ChunkPayloadError::InvalidByteMultiple {
                field,
                value: remaining,
                multiple: size,
            });
        }
        Ok(remaining / size)
    }

    fn finish(&self) -> Result<(), ChunkPayloadError> {
        let remaining = self.remaining();
        if remaining == 0 {
            Ok(())
        } else {
            Err(ChunkPayloadError::TrailingBytes {
                chunk_type: self.chunk_type,
                remaining,
            })
        }
    }

    fn finish_zero_padding(&mut self) -> Result<(), ChunkPayloadError> {
        if self.bytes[self.pos..].iter().all(|byte| *byte == 0) {
            self.pos = self.bytes.len();
            Ok(())
        } else {
            Err(ChunkPayloadError::NonZeroPadding {
                chunk_type: self.chunk_type,
            })
        }
    }

    fn read_zero_padding(&mut self, len: usize) -> Result<(), ChunkPayloadError> {
        if self.read_bytes(len)?.iter().all(|byte| *byte == 0) {
            Ok(())
        } else {
            Err(ChunkPayloadError::NonZeroPadding {
                chunk_type: self.chunk_type,
            })
        }
    }

    fn align_zero_padding(&mut self, alignment: usize) -> Result<(), ChunkPayloadError> {
        let aligned = align_len(self.pos, alignment);
        self.read_zero_padding(aligned - self.pos)
    }

    fn skip(&mut self, len: usize) -> Result<(), ChunkPayloadError> {
        self.read_bytes(len).map(|_| ())
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ChunkPayloadError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(ChunkPayloadError::UnexpectedEof {
                chunk_type: self.chunk_type,
            })?;
        if end > self.bytes.len() {
            return Err(ChunkPayloadError::UnexpectedEof {
                chunk_type: self.chunk_type,
            });
        }
        let bytes = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn read_remaining_bytes(&mut self) -> &'a [u8] {
        let bytes = &self.bytes[self.pos..];
        self.pos = self.bytes.len();
        bytes
    }

    fn read_u8(&mut self) -> Result<u8, ChunkPayloadError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, ChunkPayloadError> {
        let bytes: [u8; 2] =
            self.read_bytes(2)?
                .try_into()
                .map_err(|_| ChunkPayloadError::UnexpectedEof {
                    chunk_type: self.chunk_type,
                })?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn read_i16(&mut self) -> Result<i16, ChunkPayloadError> {
        let bytes: [u8; 2] =
            self.read_bytes(2)?
                .try_into()
                .map_err(|_| ChunkPayloadError::UnexpectedEof {
                    chunk_type: self.chunk_type,
                })?;
        Ok(i16::from_le_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32, ChunkPayloadError> {
        let bytes: [u8; 4] =
            self.read_bytes(4)?
                .try_into()
                .map_err(|_| ChunkPayloadError::UnexpectedEof {
                    chunk_type: self.chunk_type,
                })?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64, ChunkPayloadError> {
        let bytes: [u8; 8] =
            self.read_bytes(8)?
                .try_into()
                .map_err(|_| ChunkPayloadError::UnexpectedEof {
                    chunk_type: self.chunk_type,
                })?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_i32(&mut self) -> Result<i32, ChunkPayloadError> {
        let bytes: [u8; 4] =
            self.read_bytes(4)?
                .try_into()
                .map_err(|_| ChunkPayloadError::UnexpectedEof {
                    chunk_type: self.chunk_type,
                })?;
        Ok(i32::from_le_bytes(bytes))
    }

    fn read_f32(&mut self) -> Result<f32, ChunkPayloadError> {
        let bytes: [u8; 4] =
            self.read_bytes(4)?
                .try_into()
                .map_err(|_| ChunkPayloadError::UnexpectedEof {
                    chunk_type: self.chunk_type,
                })?;
        Ok(f32::from_le_bytes(bytes))
    }

    fn read_vec2(&mut self) -> Result<[f32; 2], ChunkPayloadError> {
        Ok([self.read_f32()?, self.read_f32()?])
    }

    fn read_vec3(&mut self) -> Result<[f32; 3], ChunkPayloadError> {
        Ok([self.read_f32()?, self.read_f32()?, self.read_f32()?])
    }

    fn read_vec4(&mut self) -> Result<[f32; 4], ChunkPayloadError> {
        Ok([
            self.read_f32()?,
            self.read_f32()?,
            self.read_f32()?,
            self.read_f32()?,
        ])
    }

    fn read_quat(&mut self) -> Result<[f32; 4], ChunkPayloadError> {
        self.read_vec4()
    }

    fn read_quat_t(&mut self) -> Result<QuatT, ChunkPayloadError> {
        Ok(QuatT {
            rotation: self.read_quat()?,
            translation: self.read_vec3()?,
        })
    }

    fn read_matrix_3x4(&mut self) -> Result<[[f32; 4]; 3], ChunkPayloadError> {
        Ok([self.read_vec4()?, self.read_vec4()?, self.read_vec4()?])
    }

    fn read_nonnegative_i32(&mut self, field: &'static str) -> Result<usize, ChunkPayloadError> {
        let value = self.read_i32()?;
        if value < 0 {
            return Err(ChunkPayloadError::NegativeCount { field, value });
        }
        Ok(value as usize)
    }

    fn read_array_string<const N: usize>(
        &mut self,
        bytes: &[u8],
        field: &'static str,
    ) -> Result<ArrayString<N>, ChunkPayloadError> {
        let len = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        if len > N {
            return Err(ChunkPayloadError::CountTooLarge {
                field,
                value: len,
                max: N,
            });
        }
        let text = std::str::from_utf8(&bytes[..len])?;
        let mut out = ArrayString::<N>::new();
        out.try_push_str(text)
            .map_err(|_| ChunkPayloadError::CountTooLarge {
                field,
                value: len,
                max: N,
            })?;
        Ok(out)
    }

    fn read_u32_as_usize(&mut self, field: &'static str) -> Result<usize, ChunkPayloadError> {
        usize::try_from(self.read_u32()?).map_err(|_| ChunkPayloadError::CountTooLarge {
            field,
            value: usize::MAX,
            max: usize::MAX - 1,
        })
    }

    fn read_fixed_string<const N: usize>(&mut self) -> Result<ArrayString<N>, ChunkPayloadError> {
        let bytes = self.read_bytes(N)?;
        self.read_array_string::<N>(bytes, "fixed string length")
    }

    fn read_c_string(&mut self) -> Result<ArrayString<1024>, ChunkPayloadError> {
        self.read_c_string_bounded::<1024>("cstring")
    }

    fn read_c_string_bounded<const N: usize>(
        &mut self,
        field: &'static str,
    ) -> Result<ArrayString<N>, ChunkPayloadError> {
        let remaining = &self.bytes[self.pos..];
        let len = remaining.iter().position(|byte| *byte == 0).ok_or(
            ChunkPayloadError::UnexpectedEof {
                chunk_type: self.chunk_type,
            },
        )?;
        if len > N {
            return Err(ChunkPayloadError::CountTooLarge {
                field,
                value: len,
                max: N,
            });
        }
        let bytes = self.read_bytes(len)?;
        self.skip(1)?;
        self.read_array_string(bytes, field)
    }

    fn read_c_string_dynamic(
        &mut self,
        field: &'static str,
        max_len: usize,
    ) -> Result<String, ChunkPayloadError> {
        let remaining = &self.bytes[self.pos..];
        let len = remaining.iter().position(|byte| *byte == 0).ok_or(
            ChunkPayloadError::UnexpectedEof {
                chunk_type: self.chunk_type,
            },
        )?;
        if len > max_len {
            return Err(ChunkPayloadError::CountTooLarge {
                field,
                value: len,
                max: max_len,
            });
        }
        let bytes = self.read_bytes(len)?;
        self.skip(1)?;
        Ok(std::str::from_utf8(bytes)?.to_owned())
    }

    fn read_morph_target_vertex(&mut self) -> Result<MeshMorphTargetVertex, ChunkPayloadError> {
        Ok(MeshMorphTargetVertex {
            vertex_id: self.read_u32()?,
            vertex: self.read_vec3()?,
        })
    }

    fn read_foliage_vertex_bone_mapping(
        &mut self,
    ) -> Result<FoliageVertexBoneMapping, ChunkPayloadError> {
        Ok(FoliageVertexBoneMapping {
            bone_ids: [
                self.read_u8()?,
                self.read_u8()?,
                self.read_u8()?,
                self.read_u8()?,
            ],
            weights: [
                self.read_u8()?,
                self.read_u8()?,
                self.read_u8()?,
                self.read_u8()?,
            ],
        })
    }

    fn read_bone_physics_comp(&mut self) -> Result<CryBonePhysicsComp, ChunkPayloadError> {
        Ok(CryBonePhysicsComp {
            physical_geometry_id: self.read_i32()?,
            flags: self.read_i32()?,
            min: self.read_vec3()?,
            max: self.read_vec3()?,
            spring_angle: self.read_vec3()?,
            spring_tension: self.read_vec3()?,
            damping: self.read_vec3()?,
            frame_matrix: [self.read_vec3()?, self.read_vec3()?, self.read_vec3()?],
        })
    }

    fn read_bone_entity(&mut self) -> Result<BoneEntity, ChunkPayloadError> {
        Ok(BoneEntity {
            bone_id: self.read_i32()?,
            parent_id: self.read_i32()?,
            child_count: self.read_i32()?,
            controller_id: self.read_u32()?,
            properties: self.read_fixed_string::<32>()?,
            physics: self.read_bone_physics_comp()?,
        })
    }

    fn read_bone_desc_data_comp(&mut self) -> Result<CryBoneDescDataComp, ChunkPayloadError> {
        Ok(CryBoneDescDataComp {
            controller_id: self.read_u32()?,
            physics: [
                self.read_bone_physics_comp()?,
                self.read_bone_physics_comp()?,
            ],
            mass: self.read_f32()?,
            default_world_to_bone: self.read_matrix_3x4()?,
            default_bone_to_world: self.read_matrix_3x4()?,
            bone_name: self.read_fixed_string::<256>()?,
            limb_id: self.read_i32()?,
            parent_offset: self.read_i32()?,
            child_count: self.read_u32()?,
            children_offset: self.read_i32()?,
        })
    }
}
