//! Error types for Lua chunk parsing and later compiler phases.

use thiserror::Error;

/// Errors returned by `nw-lua`.
#[derive(Debug, Error)]
pub enum LuaError {
    /// The input ended before a field could be fully read.
    #[error("truncated chunk at offset {offset}: needed {needed} bytes, remaining {remaining}")]
    Truncated {
        /// Byte offset where the failed read began.
        offset: usize,
        /// Number of bytes requested.
        needed: usize,
        /// Number of bytes available from `offset`.
        remaining: usize,
    },

    /// The input does not start with Lua's binary chunk signature.
    #[error("bad Lua chunk magic")]
    BadMagic,

    /// The chunk version is recognized as a byte, but not supported by this phase.
    #[error("unsupported Lua chunk version byte 0x{0:02x}")]
    UnsupportedVersion(u8),

    /// The chunk is structurally invalid.
    #[error("malformed Lua chunk: {0}")]
    Malformed(String),

    /// Filesystem I/O failed while loading a chunk from disk.
    #[error("io error")]
    Io(#[from] std::io::Error),

    /// Lua source emission failed.
    #[error("emit error: {0}")]
    Emit(String),

    /// The requested decompilation feature is outside the current phase.
    #[error("unsupported decompile feature: {0}")]
    Unsupported(String),
}

impl LuaError {
    pub(crate) fn truncated(offset: usize, needed: usize, len: usize) -> Self {
        Self::Truncated {
            offset,
            needed,
            remaining: len.saturating_sub(offset),
        }
    }
}
