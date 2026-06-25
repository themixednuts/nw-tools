use std::collections::BTreeMap;
use std::path::Path;

use crate::ChunkPayloadError;

/// File extensions that identify Cry chunk-file payload assets.
pub const CHUNK_FILE_EXTENSIONS: [&str; 6] = ["cgf", "cga", "chr", "skin", "caf", "i_caf"];

/// Per-file chunk summary emitted by [`ChunkFile::summary`].
#[derive(Debug, Clone)]
pub struct ChunkFileSummary {
    pub signature: super::ChunkFileSignature,
    pub chunks: usize,
    pub kinds: BTreeMap<u16, usize>,
}

impl ChunkFileSummary {
    pub fn add_kind(&mut self, kind: u16) {
        *self.kinds.entry(kind).or_default() += 1;
    }
}

/// Accumulated totals across multiple inspected Cry chunk files.
#[derive(Debug, Default, Clone)]
pub struct ChunkFileTotals {
    pub files: usize,
    pub chunks: usize,
    pub kinds: BTreeMap<u16, usize>,
}

impl ChunkFileTotals {
    pub fn add_summary(&mut self, summary: &ChunkFileSummary) {
        self.files += 1;
        self.chunks += summary.chunks;
        for (kind, count) in &summary.kinds {
            *self.kinds.entry(*kind).or_default() += count;
        }
    }
}

/// Checks whether a file extension belongs to a Cry chunk asset.
pub fn is_chunk_file_extension(extension: &str) -> bool {
    CHUNK_FILE_EXTENSIONS
        .iter()
        .any(|candidate| extension.eq_ignore_ascii_case(candidate))
}

/// Checks whether a filename (or asset path string) has a Cry chunk extension.
pub fn is_chunk_file_name(path: &str) -> bool {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some(extension) => is_chunk_file_extension(extension),
        None => false,
    }
}

/// Checks whether a filesystem path points at a Cry chunk asset.
pub fn is_chunk_file_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_chunk_file_extension)
}

impl<'a> super::ChunkFile<'a> {
    /// Parse and validate all chunk payloads, returning a normalized summary.
    pub fn summary(&self) -> Result<ChunkFileSummary, ChunkPayloadError> {
        let mut summary = ChunkFileSummary {
            signature: self.signature(),
            chunks: 0,
            kinds: BTreeMap::new(),
        };
        for chunk in self.decoded_chunks() {
            let chunk = chunk?;
            summary.chunks += 1;
            summary.add_kind(chunk.header.kind());
        }
        Ok(summary)
    }
}
