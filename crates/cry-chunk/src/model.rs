use std::collections::BTreeMap;

use thiserror::Error;

use crate::{
    ChunkFile, ChunkFileError, ChunkPayload, ChunkPayloadError, CompiledBonesChunk, DataRefChunk,
    DataStreamChunk, DecodedChunk, MaterialNameChunk, MeshChunk, MeshSubsetsChunk, NodeChunk,
};

/// Parsed representation of the chunk payloads required by Cry static model workflows.
#[derive(Debug, Default)]
pub struct CgfFile<'a> {
    meshes: BTreeMap<i32, MeshChunk>,
    mesh_subsets: BTreeMap<i32, MeshSubsetsChunk>,
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
    /// Parse a full CGF payload graph and keep only the chunks required by the model
    /// transform pipeline.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, CgfParseError> {
        let chunk_file = ChunkFile::parse(bytes)?;
        let mut file = Self::default();

        for chunk in chunk_file.decoded_chunks() {
            let chunk = chunk?;
            if let Some(chunk) = ModelChunk::from_decoded(chunk) {
                chunk.insert_into(&mut file);
            }
        }

        Ok(file)
    }

    pub fn meshes(&self) -> &BTreeMap<i32, MeshChunk> {
        &self.meshes
    }

    pub fn mesh_subsets(&self) -> &BTreeMap<i32, MeshSubsetsChunk> {
        &self.mesh_subsets
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
