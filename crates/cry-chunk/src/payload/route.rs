use super::{ChunkPayload, ChunkPayloadError, ChunkType};

pub(super) trait PayloadParse {
    fn parse<'a>(
        &self,
        chunk_type: ChunkType,
        version: u16,
        bytes: &'a [u8],
    ) -> Result<ChunkPayload<'a>, ChunkPayloadError>;
}

#[derive(Clone, Copy)]
pub(super) enum UnsupportedReason {
    WildcardNotFilePayload,
    ObsoleteDescriptor,
    NoLumberyardDescriptor,
    NotImplementedByLumberyard,
}

impl UnsupportedReason {
    pub(super) const fn message(self) -> &'static str {
        match self {
            Self::WildcardNotFilePayload => "wildcard type is not a file payload",
            Self::ObsoleteDescriptor => "obsolete; Lumberyard exposes no 0x746 payload descriptor",
            Self::NoLumberyardDescriptor => "Lumberyard exposes no payload descriptor",
            Self::NotImplementedByLumberyard => "not implemented by Lumberyard",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Decoder {
    Mesh,
    Helper,
    VertAnim,
    BoneAnim,
    BoneNameList,
    Node,
    Controller,
    Timing,
    BoneMesh,
    MeshMorphTarget,
    BoneInitialPos,
    SourceInfo,
    MaterialName,
    ExportFlags,
    DataStream,
    MeshSubsets,
    MeshPhysicsData,
    ExportMetadata,
    CompiledBones,
    CompiledPhysicalBones,
    CompiledMorphTargets,
    CompiledPhysicalProxies,
    CompiledIntFaces,
    CompiledIntSkinVertices,
    CompiledExt2IntMap,
    BreakablePhysics,
    MotionParameters,
    BonesBoxes,
    FoliageInfo,
    GlobalAnimationHeaderCaf,
    GlobalAnimationHeaderAim,
    NewWorldChunk,
    DataRef,
}

impl PayloadParse for Decoder {
    fn parse<'a>(
        &self,
        _: ChunkType,
        version: u16,
        bytes: &'a [u8],
    ) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
        match self {
            Self::Mesh => super::decode_mesh(version, bytes),
            Self::Helper => super::decode_helper(version, bytes),
            Self::VertAnim => super::decode_vert_anim(version, bytes),
            Self::BoneAnim => super::decode_bone_anim(version, bytes),
            Self::BoneNameList => super::decode_bone_name_list(version, bytes),
            Self::Node => super::decode_node(version, bytes),
            Self::Controller => super::decode_controller(version, bytes),
            Self::Timing => super::decode_timing(version, bytes),
            Self::BoneMesh => super::decode_bone_mesh(version, bytes),
            Self::MeshMorphTarget => super::decode_mesh_morph_target(version, bytes),
            Self::BoneInitialPos => super::decode_bone_initial_pos(version, bytes),
            Self::SourceInfo => super::decode_source_info(version, bytes),
            Self::MaterialName => super::decode_material_name(version, bytes),
            Self::ExportFlags => super::decode_export_flags(version, bytes),
            Self::DataStream => super::decode_data_stream(version, bytes),
            Self::MeshSubsets => super::decode_mesh_subsets(version, bytes),
            Self::MeshPhysicsData => super::decode_mesh_physics_data(version, bytes),
            Self::ExportMetadata => super::decode_export_metadata(version, bytes),
            Self::CompiledBones => super::decode_compiled_bones(version, bytes),
            Self::CompiledPhysicalBones => super::decode_compiled_physical_bones(version, bytes),
            Self::CompiledMorphTargets => super::decode_compiled_morph_targets(version, bytes),
            Self::CompiledPhysicalProxies => {
                super::decode_compiled_physical_proxies(version, bytes)
            }
            Self::CompiledIntFaces => super::decode_compiled_int_faces(version, bytes),
            Self::CompiledIntSkinVertices => {
                super::decode_compiled_int_skin_vertices(version, bytes)
            }
            Self::CompiledExt2IntMap => super::decode_compiled_ext2int_map(version, bytes),
            Self::BreakablePhysics => super::decode_breakable_physics(version, bytes),
            Self::MotionParameters => super::decode_motion_parameters(version, bytes),
            Self::BonesBoxes => super::decode_bones_boxes(version, bytes),
            Self::FoliageInfo => super::decode_foliage_info(version, bytes),
            Self::GlobalAnimationHeaderCaf => {
                super::decode_global_animation_header_caf(version, bytes)
            }
            Self::GlobalAnimationHeaderAim => {
                super::decode_global_animation_header_aim(version, bytes)
            }
            Self::NewWorldChunk => super::decode_newworld_chunk(version, bytes),
            Self::DataRef => super::decode_data_ref(version, bytes),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum PayloadRoute {
    Decode(Decoder),
    Unsupported(UnsupportedReason),
}

impl PayloadParse for PayloadRoute {
    fn parse<'a>(
        &self,
        chunk_type: ChunkType,
        version: u16,
        bytes: &'a [u8],
    ) -> Result<ChunkPayload<'a>, ChunkPayloadError> {
        match self {
            Self::Decode(parser) => parser.parse(chunk_type, version, bytes),
            Self::Unsupported(reason) => Err(ChunkPayloadError::UnsupportedChunkPayload {
                chunk_type,
                reason: reason.message(),
            }),
        }
    }
}

pub(super) const fn route(chunk_type: ChunkType) -> PayloadRoute {
    match chunk_type {
        ChunkType::Any => PayloadRoute::Unsupported(UnsupportedReason::WildcardNotFilePayload),
        ChunkType::Mesh => PayloadRoute::Decode(Decoder::Mesh),
        ChunkType::Helper => PayloadRoute::Decode(Decoder::Helper),
        ChunkType::VertAnim => PayloadRoute::Decode(Decoder::VertAnim),
        ChunkType::BoneAnim => PayloadRoute::Decode(Decoder::BoneAnim),
        ChunkType::GeomNameList => PayloadRoute::Unsupported(UnsupportedReason::ObsoleteDescriptor),
        ChunkType::BoneNameList => PayloadRoute::Decode(Decoder::BoneNameList),
        ChunkType::MtlList => PayloadRoute::Unsupported(UnsupportedReason::ObsoleteDescriptor),
        ChunkType::Mrm => PayloadRoute::Unsupported(UnsupportedReason::ObsoleteDescriptor),
        ChunkType::SceneProps => PayloadRoute::Unsupported(UnsupportedReason::ObsoleteDescriptor),
        ChunkType::Light => PayloadRoute::Unsupported(UnsupportedReason::ObsoleteDescriptor),
        ChunkType::PatchMesh => {
            PayloadRoute::Unsupported(UnsupportedReason::NotImplementedByLumberyard)
        }
        ChunkType::Node => PayloadRoute::Decode(Decoder::Node),
        ChunkType::Mtl => PayloadRoute::Unsupported(UnsupportedReason::ObsoleteDescriptor),
        ChunkType::Controller => PayloadRoute::Decode(Decoder::Controller),
        ChunkType::Timing => PayloadRoute::Decode(Decoder::Timing),
        ChunkType::BoneMesh => PayloadRoute::Decode(Decoder::BoneMesh),
        ChunkType::BoneLightBinding => {
            PayloadRoute::Unsupported(UnsupportedReason::ObsoleteDescriptor)
        }
        ChunkType::MeshMorphTarget => PayloadRoute::Decode(Decoder::MeshMorphTarget),
        ChunkType::BoneInitialPos => PayloadRoute::Decode(Decoder::BoneInitialPos),
        ChunkType::SourceInfo => PayloadRoute::Decode(Decoder::SourceInfo),
        ChunkType::MtlName => PayloadRoute::Decode(Decoder::MaterialName),
        ChunkType::ExportFlags => PayloadRoute::Decode(Decoder::ExportFlags),
        ChunkType::DataStream => PayloadRoute::Decode(Decoder::DataStream),
        ChunkType::MeshSubsets => PayloadRoute::Decode(Decoder::MeshSubsets),
        ChunkType::MeshPhysicsData => PayloadRoute::Decode(Decoder::MeshPhysicsData),
        ChunkType::ExportMetadata => PayloadRoute::Decode(Decoder::ExportMetadata),
        ChunkType::CompiledBones => PayloadRoute::Decode(Decoder::CompiledBones),
        ChunkType::CompiledPhysicalBones => PayloadRoute::Decode(Decoder::CompiledPhysicalBones),
        ChunkType::CompiledMorphTargets => PayloadRoute::Decode(Decoder::CompiledMorphTargets),
        ChunkType::CompiledPhysicalProxies => {
            PayloadRoute::Decode(Decoder::CompiledPhysicalProxies)
        }
        ChunkType::CompiledIntFaces => PayloadRoute::Decode(Decoder::CompiledIntFaces),
        ChunkType::CompiledIntSkinVertices => {
            PayloadRoute::Decode(Decoder::CompiledIntSkinVertices)
        }
        ChunkType::CompiledExt2IntMap => PayloadRoute::Decode(Decoder::CompiledExt2IntMap),
        ChunkType::BreakablePhysics => PayloadRoute::Decode(Decoder::BreakablePhysics),
        ChunkType::FaceMap => PayloadRoute::Unsupported(UnsupportedReason::ObsoleteDescriptor),
        ChunkType::MotionParameters => PayloadRoute::Decode(Decoder::MotionParameters),
        ChunkType::FootPlantInfo => {
            PayloadRoute::Unsupported(UnsupportedReason::ObsoleteDescriptor)
        }
        ChunkType::BonesBoxes => PayloadRoute::Decode(Decoder::BonesBoxes),
        ChunkType::FoliageInfo => PayloadRoute::Decode(Decoder::FoliageInfo),
        ChunkType::Timestamp => {
            PayloadRoute::Unsupported(UnsupportedReason::NoLumberyardDescriptor)
        }
        ChunkType::GlobalAnimationHeaderCaf => {
            PayloadRoute::Decode(Decoder::GlobalAnimationHeaderCaf)
        }
        ChunkType::GlobalAnimationHeaderAim => {
            PayloadRoute::Decode(Decoder::GlobalAnimationHeaderAim)
        }
        ChunkType::BspTreeData => {
            PayloadRoute::Unsupported(UnsupportedReason::NoLumberyardDescriptor)
        }
        ChunkType::NewWorldUnknown => PayloadRoute::Decode(Decoder::NewWorldChunk),
        ChunkType::DataRef => PayloadRoute::Decode(Decoder::DataRef),
    }
}
