use std::collections::BTreeMap;

use thiserror::Error;

use crate::{
    ChunkFile, ChunkFileError, ChunkPayload, ChunkPayloadError, CompiledBonesChunk, DataRefChunk,
    DataStreamChunk, DecodedChunk, MaterialNameChunk, MeshChunk, MeshPhysicsDataChunk,
    MeshSubsetsChunk, NodeChunk,
};

/// Parsed Cry model/character chunk graph with convenience indexes for the
/// geometry hot path. `chunks` retains every decoded payload in file order, so
/// morph targets, physical data, animation headers, and New World extensions
/// remain available even when they do not have a dedicated index yet.
#[derive(Debug, Default)]
pub struct CgfFile<'a> {
    chunks: Vec<DecodedChunk<'a>>,
    meshes: BTreeMap<i32, MeshChunk>,
    mesh_subsets: BTreeMap<i32, MeshSubsetsChunk>,
    mesh_physics_data: BTreeMap<i32, MeshPhysicsDataChunk<'a>>,
    data_streams: BTreeMap<i32, DataStreamChunk<'a>>,
    data_refs: BTreeMap<i32, DataRefChunk>,
    nodes: BTreeMap<i32, NodeChunk>,
    materials: BTreeMap<i32, MaterialNameChunk>,
    compiled_bones: Vec<CompiledBonesChunk>,
}

/// Error while building a parsed CGF model view.
#[derive(Debug, Error)]
pub enum CgfParseError {
    #[error(transparent)]
    ChunkFile(#[from] ChunkFileError),
    #[error(transparent)]
    ChunkPayload(#[from] ChunkPayloadError),
}

impl<'a> CgfFile<'a> {
    /// Parse and retain the full decoded CGF/CHR/SKIN chunk graph.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, CgfParseError> {
        let chunk_file = ChunkFile::parse(bytes)?;
        let mut file = Self::default();

        for chunk in chunk_file.decoded_chunks() {
            let chunk = chunk?;
            file.chunks.push(chunk.clone());
            if let Some(chunk) = ModelChunk::from_decoded(chunk) {
                chunk.insert_into(&mut file);
            }
        }

        Ok(file)
    }

    /// Every decoded chunk in original file order.
    pub fn chunks(&self) -> &[DecodedChunk<'a>] {
        &self.chunks
    }

    pub fn meshes(&self) -> &BTreeMap<i32, MeshChunk> {
        &self.meshes
    }

    pub fn mesh_subsets(&self) -> &BTreeMap<i32, MeshSubsetsChunk> {
        &self.mesh_subsets
    }

    pub fn mesh_physics_data(&self) -> &BTreeMap<i32, MeshPhysicsDataChunk<'a>> {
        &self.mesh_physics_data
    }

    pub fn data_streams(&self) -> &BTreeMap<i32, DataStreamChunk<'a>> {
        &self.data_streams
    }

    pub fn data_refs(&self) -> &BTreeMap<i32, DataRefChunk> {
        &self.data_refs
    }

    pub fn nodes(&self) -> &BTreeMap<i32, NodeChunk> {
        &self.nodes
    }

    pub fn materials(&self) -> &BTreeMap<i32, MaterialNameChunk> {
        &self.materials
    }

    /// Compiled bone chunks (character skeletons), in file order.
    pub fn compiled_bones(&self) -> &[CompiledBonesChunk] {
        &self.compiled_bones
    }
}

// Mirrors the bounded-`ArrayVec` payloads (see `ChunkPayload`); large by design.
#[allow(clippy::large_enum_variant)]
enum ModelChunk<'a> {
    Mesh(i32, Box<MeshChunk>),
    MeshSubsets(i32, MeshSubsetsChunk),
    MeshPhysicsData(i32, MeshPhysicsDataChunk<'a>),
    DataStream(i32, DataStreamChunk<'a>),
    DataRef(i32, DataRefChunk),
    Node(i32, NodeChunk),
    MaterialName(i32, MaterialNameChunk),
    CompiledBones(CompiledBonesChunk),
}

impl<'a> ModelChunk<'a> {
    fn from_decoded(chunk: DecodedChunk<'a>) -> Option<Self> {
        let id = chunk.header.id();
        match chunk.payload {
            ChunkPayload::Mesh(payload) => Some(Self::Mesh(id, payload)),
            ChunkPayload::MeshSubsets(payload) => Some(Self::MeshSubsets(id, payload)),
            ChunkPayload::MeshPhysicsData(payload) => Some(Self::MeshPhysicsData(id, payload)),
            ChunkPayload::DataStream(payload) => Some(Self::DataStream(id, payload)),
            ChunkPayload::DataRef(payload) => Some(Self::DataRef(id, payload)),
            ChunkPayload::Node(payload) => Some(Self::Node(id, payload)),
            ChunkPayload::MaterialName(payload) => Some(Self::MaterialName(id, payload)),
            ChunkPayload::CompiledBones(payload) => Some(Self::CompiledBones(payload)),
            _ => None,
        }
    }

    fn insert_into(self, file: &mut CgfFile<'a>) {
        match self {
            Self::CompiledBones(payload) => file.compiled_bones.push(payload),
            Self::Mesh(id, payload) => {
                file.meshes.insert(id, *payload);
            }
            Self::MeshSubsets(id, payload) => {
                file.mesh_subsets.insert(id, payload);
            }
            Self::MeshPhysicsData(id, payload) => {
                file.mesh_physics_data.insert(id, payload);
            }
            Self::DataStream(id, payload) => {
                file.data_streams.insert(id, payload);
            }
            Self::DataRef(id, payload) => {
                file.data_refs.insert(id, payload);
            }
            Self::Node(id, payload) => {
                file.nodes.insert(id, payload);
            }
            Self::MaterialName(id, payload) => {
                file.materials.insert(id, payload);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CHUNK_HEADER_LEN_0X746, CRY_SIGNATURE, ChunkType, FILE_HEADER_LEN, FILE_VERSION_0X746,
    };

    #[test]
    fn indexes_mesh_physics_data_by_chunk_id() {
        let physical_data = [1, 2, 3, 4];
        let tetrahedra_data = [5, 6];
        let mut payload = Vec::new();
        payload.extend_from_slice(&(physical_data.len() as i32).to_le_bytes());
        payload.extend_from_slice(&7_i32.to_le_bytes());
        payload.extend_from_slice(&(tetrahedra_data.len() as i32).to_le_bytes());
        payload.extend_from_slice(&91_i32.to_le_bytes());
        payload.extend_from_slice(&[0; 8]);
        payload.extend_from_slice(&physical_data);
        payload.extend_from_slice(&tetrahedra_data);

        let bytes = chunk_file(ChunkType::MeshPhysicsData.as_u16(), 0x0800, 72, &payload);
        let file = CgfFile::parse(&bytes).unwrap();
        let physics = file.mesh_physics_data().get(&72).unwrap();

        assert_eq!(physics.flags, 7);
        assert_eq!(physics.tetrahedra_chunk_id, 91);
        assert_eq!(physics.physical_data, physical_data);
        assert_eq!(physics.tetrahedra_data, tetrahedra_data);
    }

    fn chunk_file(kind: u16, version: u16, id: i32, payload: &[u8]) -> Vec<u8> {
        let payload_offset = FILE_HEADER_LEN + CHUNK_HEADER_LEN_0X746;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&CRY_SIGNATURE);
        bytes.extend_from_slice(&FILE_VERSION_0X746.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&(FILE_HEADER_LEN as u32).to_le_bytes());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(payload_offset as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }
}
