//! Borrowed parser for Cry chunk-file containers.
//!
//! Source: `dev/Code/CryEngine/Cry3DEngine/CGF/ChunkFileComponents.h:72`.

use std::fmt;

pub use arrayvec::{ArrayString, ArrayVec};
use thiserror::Error;

mod model;
mod payload;
mod scan;

pub use model::{CgfFile, CgfParseError};
pub use payload::{
    AimPose, BoneAnimChunk, BoneInitialPosChunk, BoneMeshChunk, BoneMeshFace, BoneMeshLink,
    BoneMeshVertex, BoneNameListChunk, BonesBoxesChunk, BreakablePhysicsChunk, ChunkPayload,
    ChunkPayloadError, CompiledBonesChunk, CompiledExt2IntMapChunk, CompiledIntFacesChunk,
    CompiledIntSkinVerticesChunk, CompiledMorphTargetsChunk, CompiledPhysicalBonesChunk,
    CompiledPhysicalProxiesChunk, ControllerChunk, ControllerCompressedChunk, ControllerDbChunk,
    CryBoneDescDataComp,
    ControllerTcbChunk, ControllerTrack, ControllerUncompressedChunk, DataRefChunk,
    DataStreamChunk, ExportFlagsChunk, ExportMetadataChunk, FoliageBoneMapping, FoliageDynamics,
    FoliageInfoChunk, FoliageNodeBoneMapping, FoliageSpine, FoliageVertexBoneMapping,
    GlobalAnimationHeaderAimChunk, GlobalAnimationHeaderCafChunk, HelperChunk, MaterialNameChunk,
    MeshChunk, MeshMorphTargetChunk, MeshPhysicsDataChunk, MeshStreamKind, MeshSubset,
    MeshSubsetBoneIds, MeshSubsetsChunk, MotionParametersChunk, NewWorldChunk, NodeChunk,
    STREAM_INDEX_COUNT, STREAM_TYPE_COUNT, SourceInfoChunk, TimingChunk, VertAnimChunk,
};
pub use scan::{
    CHUNK_FILE_EXTENSIONS, ChunkFileSummary, ChunkFileTotals, is_chunk_file_extension,
    is_chunk_file_name, is_chunk_file_path,
};

const CRY_SIGNATURE: [u8; 4] = *b"CrCh";
const SPEEDTREE_SIGNATURE: [u8; 4] = *b"STCh";
const FILE_VERSION_0X746: u32 = 0x746;
const FILE_HEADER_LEN: usize = 16;
const CHUNK_HEADER_LEN_0X746: usize = 16;
const MAX_CHUNK_COUNT_0X746: u32 = 10_000_000;

/// Cry chunk-file signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkFileSignature {
    Cry,
    SpeedTree,
}

impl ChunkFileSignature {
    pub const fn bytes(self) -> [u8; 4] {
        match self {
            Self::Cry => CRY_SIGNATURE,
            Self::SpeedTree => SPEEDTREE_SIGNATURE,
        }
    }

    const fn from_bytes(bytes: [u8; 4]) -> Option<Self> {
        match bytes {
            CRY_SIGNATURE => Some(Self::Cry),
            SPEEDTREE_SIGNATURE => Some(Self::SpeedTree),
            _ => None,
        }
    }
}

/// Known Cry chunk type IDs from Lumberyard's `CryHeaders.h`.
///
/// New World 3-26 also emits the tail `0x300a`/`0x300b` values. Lumberyard's
/// public headers stop at `ChunkType_BspTreeData`, so those are kept explicit
/// here as New World extensions rather than silently treating them as raw IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum ChunkType {
    Any = 0,
    Mesh = 0x1000,
    Helper = 0x1001,
    VertAnim = 0x1002,
    BoneAnim = 0x1003,
    GeomNameList = 0x1004,
    BoneNameList = 0x1005,
    MtlList = 0x1006,
    Mrm = 0x1007,
    SceneProps = 0x1008,
    Light = 0x1009,
    PatchMesh = 0x100a,
    Node = 0x100b,
    Mtl = 0x100c,
    Controller = 0x100d,
    Timing = 0x100e,
    BoneMesh = 0x100f,
    BoneLightBinding = 0x1010,
    MeshMorphTarget = 0x1011,
    BoneInitialPos = 0x1012,
    SourceInfo = 0x1013,
    MtlName = 0x1014,
    ExportFlags = 0x1015,
    DataStream = 0x1016,
    MeshSubsets = 0x1017,
    MeshPhysicsData = 0x1018,
    ExportMetadata = 0x1019,
    CompiledBones = 0x2000,
    CompiledPhysicalBones = 0x2001,
    CompiledMorphTargets = 0x2002,
    CompiledPhysicalProxies = 0x2003,
    CompiledIntFaces = 0x2004,
    CompiledIntSkinVertices = 0x2005,
    CompiledExt2IntMap = 0x2006,
    BreakablePhysics = 0x3000,
    FaceMap = 0x3001,
    MotionParameters = 0x3002,
    FootPlantInfo = 0x3003,
    BonesBoxes = 0x3004,
    FoliageInfo = 0x3005,
    Timestamp = 0x3006,
    GlobalAnimationHeaderCaf = 0x3007,
    GlobalAnimationHeaderAim = 0x3008,
    BspTreeData = 0x3009,
    NewWorldUnknown = 0x300a,
    DataRef = 0x300b,
}

impl ChunkType {
    pub const fn from_u16(value: u16) -> Option<Self> {
        match value {
            0 => Some(Self::Any),
            0x1000 => Some(Self::Mesh),
            0x1001 => Some(Self::Helper),
            0x1002 => Some(Self::VertAnim),
            0x1003 => Some(Self::BoneAnim),
            0x1004 => Some(Self::GeomNameList),
            0x1005 => Some(Self::BoneNameList),
            0x1006 => Some(Self::MtlList),
            0x1007 => Some(Self::Mrm),
            0x1008 => Some(Self::SceneProps),
            0x1009 => Some(Self::Light),
            0x100a => Some(Self::PatchMesh),
            0x100b => Some(Self::Node),
            0x100c => Some(Self::Mtl),
            0x100d => Some(Self::Controller),
            0x100e => Some(Self::Timing),
            0x100f => Some(Self::BoneMesh),
            0x1010 => Some(Self::BoneLightBinding),
            0x1011 => Some(Self::MeshMorphTarget),
            0x1012 => Some(Self::BoneInitialPos),
            0x1013 => Some(Self::SourceInfo),
            0x1014 => Some(Self::MtlName),
            0x1015 => Some(Self::ExportFlags),
            0x1016 => Some(Self::DataStream),
            0x1017 => Some(Self::MeshSubsets),
            0x1018 => Some(Self::MeshPhysicsData),
            0x1019 => Some(Self::ExportMetadata),
            0x2000 => Some(Self::CompiledBones),
            0x2001 => Some(Self::CompiledPhysicalBones),
            0x2002 => Some(Self::CompiledMorphTargets),
            0x2003 => Some(Self::CompiledPhysicalProxies),
            0x2004 => Some(Self::CompiledIntFaces),
            0x2005 => Some(Self::CompiledIntSkinVertices),
            0x2006 => Some(Self::CompiledExt2IntMap),
            0x3000 => Some(Self::BreakablePhysics),
            0x3001 => Some(Self::FaceMap),
            0x3002 => Some(Self::MotionParameters),
            0x3003 => Some(Self::FootPlantInfo),
            0x3004 => Some(Self::BonesBoxes),
            0x3005 => Some(Self::FoliageInfo),
            0x3006 => Some(Self::Timestamp),
            0x3007 => Some(Self::GlobalAnimationHeaderCaf),
            0x3008 => Some(Self::GlobalAnimationHeaderAim),
            0x3009 => Some(Self::BspTreeData),
            0x300a => Some(Self::NewWorldUnknown),
            0x300b => Some(Self::DataRef),
            _ => None,
        }
    }

    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Any => "ChunkType_ANY",
            Self::Mesh => "ChunkType_Mesh",
            Self::Helper => "ChunkType_Helper",
            Self::VertAnim => "ChunkType_VertAnim",
            Self::BoneAnim => "ChunkType_BoneAnim",
            Self::GeomNameList => "ChunkType_GeomNameList",
            Self::BoneNameList => "ChunkType_BoneNameList",
            Self::MtlList => "ChunkType_MtlList",
            Self::Mrm => "ChunkType_MRM",
            Self::SceneProps => "ChunkType_SceneProps",
            Self::Light => "ChunkType_Light",
            Self::PatchMesh => "ChunkType_PatchMesh",
            Self::Node => "ChunkType_Node",
            Self::Mtl => "ChunkType_Mtl",
            Self::Controller => "ChunkType_Controller",
            Self::Timing => "ChunkType_Timing",
            Self::BoneMesh => "ChunkType_BoneMesh",
            Self::BoneLightBinding => "ChunkType_BoneLightBinding",
            Self::MeshMorphTarget => "ChunkType_MeshMorphTarget",
            Self::BoneInitialPos => "ChunkType_BoneInitialPos",
            Self::SourceInfo => "ChunkType_SourceInfo",
            Self::MtlName => "ChunkType_MtlName",
            Self::ExportFlags => "ChunkType_ExportFlags",
            Self::DataStream => "ChunkType_DataStream",
            Self::MeshSubsets => "ChunkType_MeshSubsets",
            Self::MeshPhysicsData => "ChunkType_MeshPhysicsData",
            Self::ExportMetadata => "ChunkType_ExportMetadata",
            Self::CompiledBones => "ChunkType_CompiledBones",
            Self::CompiledPhysicalBones => "ChunkType_CompiledPhysicalBones",
            Self::CompiledMorphTargets => "ChunkType_CompiledMorphTargets",
            Self::CompiledPhysicalProxies => "ChunkType_CompiledPhysicalProxies",
            Self::CompiledIntFaces => "ChunkType_CompiledIntFaces",
            Self::CompiledIntSkinVertices => "ChunkType_CompiledIntSkinVertices",
            Self::CompiledExt2IntMap => "ChunkType_CompiledExt2IntMap",
            Self::BreakablePhysics => "ChunkType_BreakablePhysics",
            Self::FaceMap => "ChunkType_FaceMap",
            Self::MotionParameters => "ChunkType_MotionParameters",
            Self::FootPlantInfo => "ChunkType_FootPlantInfo",
            Self::BonesBoxes => "ChunkType_BonesBoxes",
            Self::FoliageInfo => "ChunkType_FoliageInfo",
            Self::Timestamp => "ChunkType_Timestamp",
            Self::GlobalAnimationHeaderCaf => "ChunkType_GlobalAnimationHeaderCAF",
            Self::GlobalAnimationHeaderAim => "ChunkType_GlobalAnimationHeaderAIM",
            Self::BspTreeData => "ChunkType_BspTreeData",
            Self::NewWorldUnknown => "ChunkType_UNKNOWN",
            Self::DataRef => "ChunkType_DataRef",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Any => "wildcard chunk type used by tooling",
            Self::Mesh => "mesh descriptor and references to stream, subset, and physics chunks",
            Self::Helper => "helper object data",
            Self::VertAnim => "legacy vertex animation data",
            Self::BoneAnim => "legacy bone animation data",
            Self::GeomNameList => "obsolete geometry name list",
            Self::BoneNameList => "bone name list",
            Self::MtlList => "obsolete material list",
            Self::Mrm => "obsolete multi-resolution mesh data",
            Self::SceneProps => "obsolete scene properties",
            Self::Light => "obsolete light data",
            Self::PatchMesh => "patch mesh data, not implemented by Lumberyard",
            Self::Node => "scene node with object, parent, material, and transform references",
            Self::Mtl => "obsolete material data",
            Self::Controller => "animation controller data",
            Self::Timing => "timing data",
            Self::BoneMesh => "bone mesh data",
            Self::BoneLightBinding => "obsolete lights bound to bones",
            Self::MeshMorphTarget => "morph target for a mesh chunk",
            Self::BoneInitialPos => "initial 4x3 bone matrices",
            Self::SourceInfo => "export source file path metadata",
            Self::MtlName => "material name and sub-material references",
            Self::ExportFlags => "special export flags",
            Self::DataStream => "inline mesh stream data",
            Self::MeshSubsets => "array of mesh subsets",
            Self::MeshPhysicsData => "physicalized mesh data",
            Self::ExportMetadata => "New World DCC/export JSON metadata",
            Self::CompiledBones => "compiled character bones",
            Self::CompiledPhysicalBones => "compiled character physical bones",
            Self::CompiledMorphTargets => "compiled character morph targets",
            Self::CompiledPhysicalProxies => "compiled character physical proxies",
            Self::CompiledIntFaces => "compiled internal character faces",
            Self::CompiledIntSkinVertices => "compiled internal skin vertices",
            Self::CompiledExt2IntMap => "compiled external-to-internal map",
            Self::BreakablePhysics => "breakable physics data",
            Self::FaceMap => "obsolete face map data",
            Self::MotionParameters => "motion parameter data",
            Self::FootPlantInfo => "obsolete foot-plant data",
            Self::BonesBoxes => "bone bounding boxes",
            Self::FoliageInfo => "foliage info data",
            Self::Timestamp => "timestamp data",
            Self::GlobalAnimationHeaderCaf => "global CAF animation header",
            Self::GlobalAnimationHeaderAim => "global AIM animation header",
            Self::BspTreeData => "BSP tree data",
            Self::NewWorldUnknown => "New World extension chunk with unresolved payload semantics",
            Self::DataRef => "New World extension referencing stream bytes in a .cgfheap sidecar",
        }
    }
}

impl fmt::Display for ChunkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.source_name())
    }
}

#[derive(Debug, Clone)]
pub struct DecodedChunk<'a> {
    pub header: ChunkHeader,
    pub payload: ChunkPayload<'a>,
}

/// Borrowed Cry chunk-file container.
#[derive(Debug, Clone, Copy)]
pub struct ChunkFile<'a> {
    bytes: &'a [u8],
    signature: ChunkFileSignature,
    chunk_count: u32,
    chunk_table_offset: u32,
}

impl<'a> ChunkFile<'a> {
    /// Parse a Cry chunk-file header and validate its chunk table.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ChunkFileError> {
        let header = bytes
            .get(..FILE_HEADER_LEN)
            .ok_or(ChunkFileError::UnexpectedEof {
                context: "chunk file header",
            })?;
        let signature_bytes = read_array::<4>(header, 0, "chunk file signature")?;
        let Some(signature) = ChunkFileSignature::from_bytes(signature_bytes) else {
            return Err(ChunkFileError::InvalidSignature {
                found: signature_bytes,
            });
        };

        let version = read_u32(header, 4, "chunk file version")?;
        if version != FILE_VERSION_0X746 {
            return Err(ChunkFileError::UnsupportedVersion { version });
        }

        let chunk_count = read_u32(header, 8, "chunk count")?;
        if chunk_count > MAX_CHUNK_COUNT_0X746 {
            return Err(ChunkFileError::ChunkCountTooLarge {
                count: chunk_count,
                max: MAX_CHUNK_COUNT_0X746,
            });
        }

        let chunk_table_offset = read_u32(header, 12, "chunk table offset")?;
        let table_len = usize::try_from(chunk_count)
            .ok()
            .and_then(|count| count.checked_mul(CHUNK_HEADER_LEN_0X746))
            .ok_or(ChunkFileError::ChunkTableOutOfBounds {
                offset: chunk_table_offset,
                count: chunk_count,
            })?;
        let table_start = usize::try_from(chunk_table_offset).map_err(|_| {
            ChunkFileError::ChunkTableOutOfBounds {
                offset: chunk_table_offset,
                count: chunk_count,
            }
        })?;
        let table_end =
            table_start
                .checked_add(table_len)
                .ok_or(ChunkFileError::ChunkTableOutOfBounds {
                    offset: chunk_table_offset,
                    count: chunk_count,
                })?;
        if table_end > bytes.len() {
            return Err(ChunkFileError::ChunkTableOutOfBounds {
                offset: chunk_table_offset,
                count: chunk_count,
            });
        }

        Ok(Self {
            bytes,
            signature,
            chunk_count,
            chunk_table_offset,
        })
    }

    pub const fn signature(&self) -> ChunkFileSignature {
        self.signature
    }

    pub const fn version(&self) -> u32 {
        FILE_VERSION_0X746
    }

    pub const fn chunk_count(&self) -> u32 {
        self.chunk_count
    }

    pub const fn chunk_table_offset(&self) -> u32 {
        self.chunk_table_offset
    }

    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn chunks(&self) -> ChunkHeaders<'a> {
        let offset = self.chunk_table_offset as usize;
        let len = self.chunk_count as usize * CHUNK_HEADER_LEN_0X746;
        ChunkHeaders {
            table: &self.bytes[offset..offset + len],
            file_len: self.bytes.len(),
        }
    }

    pub fn decoded_chunks(&self) -> DecodedChunks<'a> {
        DecodedChunks {
            bytes: self.bytes,
            chunks: self.chunks(),
        }
    }
}

/// Iterator over validated and decoded chunk payloads.
#[derive(Debug, Clone)]
pub struct DecodedChunks<'a> {
    bytes: &'a [u8],
    chunks: ChunkHeaders<'a>,
}

impl<'a> Iterator for DecodedChunks<'a> {
    type Item = Result<DecodedChunk<'a>, ChunkPayloadError>;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk = match self.chunks.next()? {
            Ok(chunk) => chunk,
            Err(err) => return Some(Err(err.into())),
        };
        Some(chunk.decode_from(self.bytes))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}

impl ExactSizeIterator for DecodedChunks<'_> {}

/// Iterator over validated 0x746 chunk-table entries.
#[derive(Debug, Clone)]
pub struct ChunkHeaders<'a> {
    table: &'a [u8],
    file_len: usize,
}

impl Iterator for ChunkHeaders<'_> {
    type Item = Result<ChunkHeader, ChunkFileError>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.table.get(..CHUNK_HEADER_LEN_0X746)?;
        self.table = &self.table[CHUNK_HEADER_LEN_0X746..];
        Some(ChunkHeader::parse(entry).and_then(|header| {
            header.validate_payload(self.file_len)?;
            Ok(header)
        }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.table.len() / CHUNK_HEADER_LEN_0X746;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ChunkHeaders<'_> {}

/// Cry chunk-file 0x746 table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkHeader {
    kind: u16,
    version: u16,
    id: i32,
    size: u32,
    offset: u32,
    big_endian: bool,
}

impl ChunkHeader {
    fn parse(entry: &[u8]) -> Result<Self, ChunkFileError> {
        let raw_version = read_u16(entry, 2, "chunk version")?;
        Ok(Self {
            kind: read_u16(entry, 0, "chunk kind")?,
            version: raw_version & 0x7fff,
            id: read_i32(entry, 4, "chunk id")?,
            size: read_u32(entry, 8, "chunk size")?,
            offset: read_u32(entry, 12, "chunk offset")?,
            big_endian: raw_version & 0x8000 != 0,
        })
    }

    pub const fn kind(&self) -> u16 {
        self.kind
    }

    pub const fn chunk_type(&self) -> Option<ChunkType> {
        ChunkType::from_u16(self.kind)
    }

    pub const fn version(&self) -> u16 {
        self.version
    }

    pub const fn id(&self) -> i32 {
        self.id
    }

    pub const fn size(&self) -> u32 {
        self.size
    }

    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub const fn is_big_endian(&self) -> bool {
        self.big_endian
    }

    pub fn decode_from<'a>(&self, bytes: &'a [u8]) -> Result<DecodedChunk<'a>, ChunkPayloadError> {
        let chunk_type = self
            .chunk_type()
            .ok_or(ChunkPayloadError::UnknownChunkType { kind: self.kind })?;
        let payload = payload::decode_payload(chunk_type, self.version, self.payload_from(bytes)?)?;
        Ok(DecodedChunk {
            header: *self,
            payload,
        })
    }

    pub fn payload_from<'a>(&self, bytes: &'a [u8]) -> Result<&'a [u8], ChunkFileError> {
        self.validate_payload(bytes.len())?;
        let start = self.offset as usize;
        let end = start + self.size as usize;
        Ok(&bytes[start..end])
    }

    fn validate_payload(&self, file_len: usize) -> Result<(), ChunkFileError> {
        let start = usize::try_from(self.offset).map_err(|_| ChunkFileError::ChunkOutOfBounds {
            id: self.id,
            offset: self.offset,
            size: self.size,
        })?;
        let size = usize::try_from(self.size).map_err(|_| ChunkFileError::ChunkOutOfBounds {
            id: self.id,
            offset: self.offset,
            size: self.size,
        })?;
        let end = start
            .checked_add(size)
            .ok_or(ChunkFileError::ChunkOutOfBounds {
                id: self.id,
                offset: self.offset,
                size: self.size,
            })?;
        if end > file_len {
            return Err(ChunkFileError::ChunkOutOfBounds {
                id: self.id,
                offset: self.offset,
                size: self.size,
            });
        }
        Ok(())
    }
}

/// Error for Cry chunk-file parsing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChunkFileError {
    #[error("unexpected end of file while reading {context}")]
    UnexpectedEof { context: &'static str },
    #[error("invalid Cry chunk-file signature {found:?}")]
    InvalidSignature { found: [u8; 4] },
    #[error("unsupported Cry chunk-file version {version:#x}")]
    UnsupportedVersion { version: u32 },
    #[error("chunk count {count} exceeds maximum {max}")]
    ChunkCountTooLarge { count: u32, max: u32 },
    #[error("chunk table at {offset:#x} with {count} chunks points outside the file")]
    ChunkTableOutOfBounds { offset: u32, count: u32 },
    #[error("chunk {id} at {offset:#x} with {size} bytes points outside the file")]
    ChunkOutOfBounds { id: i32, offset: u32, size: u32 },
}

fn read_u16(bytes: &[u8], offset: usize, context: &'static str) -> Result<u16, ChunkFileError> {
    Ok(u16::from_le_bytes(read_array(bytes, offset, context)?))
}

fn read_u32(bytes: &[u8], offset: usize, context: &'static str) -> Result<u32, ChunkFileError> {
    Ok(u32::from_le_bytes(read_array(bytes, offset, context)?))
}

fn read_i32(bytes: &[u8], offset: usize, context: &'static str) -> Result<i32, ChunkFileError> {
    Ok(i32::from_le_bytes(read_array(bytes, offset, context)?))
}

fn read_array<const N: usize>(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<[u8; N], ChunkFileError> {
    bytes
        .get(offset..offset + N)
        .ok_or(ChunkFileError::UnexpectedEof { context })?
        .try_into()
        .map_err(|_| ChunkFileError::UnexpectedEof { context })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_crch_0x746_chunk_table() {
        let bytes = chunk_file(CRY_SIGNATURE, &[(0x1000, 0x0802, 7, vec![1, 2, 3, 4])]);
        let file = ChunkFile::parse(&bytes).unwrap();

        assert_eq!(file.signature(), ChunkFileSignature::Cry);
        assert_eq!(file.version(), 0x746);
        assert_eq!(file.chunk_count(), 1);

        let chunk = file.chunks().next().unwrap().unwrap();
        assert_eq!(chunk.kind(), 0x1000);
        assert_eq!(chunk.version(), 0x0802);
        assert_eq!(chunk.id(), 7);
        assert_eq!(chunk.payload_from(file.bytes()).unwrap(), &[1, 2, 3, 4]);
    }

    #[test]
    fn parses_speedtree_signature() {
        let bytes = chunk_file(SPEEDTREE_SIGNATURE, &[]);
        let file = ChunkFile::parse(&bytes).unwrap();

        assert_eq!(file.signature(), ChunkFileSignature::SpeedTree);
        assert_eq!(file.chunk_count(), 0);
    }

    #[test]
    fn maps_lumberyard_and_newworld_chunk_type_names() {
        assert_eq!(ChunkType::from_u16(0x1000), Some(ChunkType::Mesh));
        assert_eq!(ChunkType::from_u16(0x3009), Some(ChunkType::BspTreeData));
        assert_eq!(
            ChunkType::from_u16(0x300a),
            Some(ChunkType::NewWorldUnknown)
        );
        assert_eq!(ChunkType::from_u16(0x300b), Some(ChunkType::DataRef));
        assert_eq!(ChunkType::from_u16(0xffff), None);
        assert_eq!(ChunkType::DataRef.as_u16(), 0x300b);
        assert_eq!(ChunkType::DataRef.source_name(), "ChunkType_DataRef");
        assert!(ChunkType::Mesh.description().contains("stream"));
    }

    #[test]
    fn rejects_unknown_signature() {
        let bytes = chunk_file(*b"bad!", &[]);

        assert!(matches!(
            ChunkFile::parse(&bytes),
            Err(ChunkFileError::InvalidSignature { found }) if found == *b"bad!"
        ));
    }

    #[test]
    fn rejects_out_of_bounds_chunk_payload() {
        let mut bytes = chunk_file(CRY_SIGNATURE, &[(0x1000, 0x0802, 1, vec![1])]);
        let len = bytes.len();
        bytes[len - 8..len - 4].copy_from_slice(&999u32.to_le_bytes());

        let file = ChunkFile::parse(&bytes).unwrap();
        assert!(matches!(
            file.chunks().next().unwrap(),
            Err(ChunkFileError::ChunkOutOfBounds { id: 1, .. })
        ));
    }

    fn chunk_file(signature: [u8; 4], chunks: &[(u16, u16, i32, Vec<u8>)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&signature);
        bytes.extend_from_slice(&FILE_VERSION_0X746.to_le_bytes());
        bytes.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(FILE_HEADER_LEN as u32).to_le_bytes());

        let mut payload_offset = FILE_HEADER_LEN + chunks.len() * CHUNK_HEADER_LEN_0X746;
        for (kind, version, id, payload) in chunks {
            bytes.extend_from_slice(&kind.to_le_bytes());
            bytes.extend_from_slice(&version.to_le_bytes());
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&(payload_offset as u32).to_le_bytes());
            payload_offset += payload.len();
        }
        for (_, _, _, payload) in chunks {
            bytes.extend_from_slice(payload);
        }
        bytes
    }
}
