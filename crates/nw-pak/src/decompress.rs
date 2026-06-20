//! Per-zip-entry decompression.
//!
//! Handles the three compression methods NW paks use, then peels the
//! optional AZCS inner wrapper if present.

use std::fmt;
use std::io::{self, Cursor, Read};

use flate2::Decompress;
use thiserror::Error;
use zip::CompressionMethod;
use zip::read::ZipFile;

use crate::azcs::{self, AzcsError};
use crate::oodle::Codec as OodleCodec;

/// Compression method used for a pak entry.
///
/// Wraps [`zip::CompressionMethod`] with NW-specific decoding (the
/// non-standard method `15` = Oodle).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Compression {
    /// `Stored` — bytes are not compressed.
    Stored,
    /// `Deflated` — RFC 1951 DEFLATE, optionally wrapped in a zlib
    /// header (`0x78 0xda` sniff fall-through).
    Deflated,
    /// NW-custom Oodle (compression method `15`).
    Oodle,
    /// Any other method we don't currently handle. Surfaces as
    /// [`DecompressError::UnsupportedMethod`] when reading.
    Other(u16),
}

impl Compression {
    #[must_use]
    pub const fn from_method_id(method: u16) -> Self {
        match method {
            0 => Self::Stored,
            8 => Self::Deflated,
            15 => Self::Oodle,
            other => Self::Other(other),
        }
    }

    #[must_use]
    pub const fn method_id(self) -> u16 {
        match self {
            Self::Stored => 0,
            Self::Deflated => 8,
            Self::Oodle => 15,
            Self::Other(method) => method,
        }
    }

    /// Classify a [`zip::CompressionMethod`] without reading bytes.
    #[must_use]
    pub fn from_zip_method(method: CompressionMethod) -> Self {
        #[allow(deprecated)]
        match method {
            CompressionMethod::Stored => Self::from_method_id(0),
            CompressionMethod::Deflated => Self::from_method_id(8),
            CompressionMethod::Unsupported(other) => Self::from_method_id(other),
            _ => Self::Other(u16::MAX),
        }
    }
}

impl fmt::Display for Compression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stored => f.write_str("stored"),
            Self::Deflated => f.write_str("deflated"),
            Self::Oodle => f.write_str("oodle"),
            Self::Other(method) => write!(f, "other({method})"),
        }
    }
}

#[derive(Debug, Error)]
pub enum DecompressError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("zip compression {0:?} is not supported by nw-pak")]
    UnsupportedMethod(Compression),

    #[error("oodle decompression failed (entry size {expected_size} bytes)")]
    Oodle { expected_size: usize },

    #[error("azcs inner-decompression failed: {0}")]
    Azcs(#[from] AzcsError),

    #[error("{field} is too large for this process: {value} bytes")]
    SizeTooLarge { field: &'static str, value: u64 },
}

/// Decompress one zip entry into a freshly allocated `Vec<u8>`.
///
/// Convenience wrapper around [`decompress_zip_entry_into`] that
/// allocates a buffer sized to the entry's `uncompressed_size`. Use
/// the `_into` variant when you can reuse a buffer across many
/// reads.
#[inline]
pub fn decompress_zip_entry<R: Read>(
    entry: &mut ZipFile<'_, R>,
) -> Result<Vec<u8>, DecompressError> {
    let mut out = Vec::with_capacity(usize_from_u64("entry size", entry.size())?);
    decompress_zip_entry_into(entry, &mut out)?;
    Ok(out)
}

/// Decompress one zip entry into a caller-supplied buffer.
///
/// Clears `out` first, then fills it. Handles `Stored`, `Deflated`
/// (with optional `0x78 0xda` zlib header), and the NW-custom
/// `Unsupported(15)` Oodle method. Peels the AZCS inner wrapper if
/// present.
///
/// Returns the final decompressed length (i.e. `out.len()`).
pub fn decompress_zip_entry_into<R: Read>(
    entry: &mut ZipFile<'_, R>,
    out: &mut Vec<u8>,
) -> Result<usize, DecompressError> {
    out.clear();
    out.reserve(usize_from_u64("entry size", entry.size())?);

    if entry.size() == 0 {
        return Ok(0);
    }

    let compression = Compression::from_zip_method(entry.compression());
    match compression {
        Compression::Stored => {
            io::copy(entry, out)?;
        }
        Compression::Deflated => {
            // Sniff for a zlib header (`0x78 0xda`); fall back to raw DEFLATE.
            let mut sig = [0u8; 2];
            entry.read_exact(&mut sig)?;
            if sig == [0x78, 0xda] {
                let mut zlib = flate2::read::ZlibDecoder::new_with_decompress(
                    Cursor::new(sig).chain(entry),
                    Decompress::new(true),
                );
                io::copy(&mut zlib, out)?;
            } else {
                let mut deflate = flate2::read::DeflateDecoder::new(Cursor::new(sig).chain(entry));
                io::copy(&mut deflate, out)?;
            }
        }
        Compression::Oodle => {
            let expected_size = usize_from_u64("entry size", entry.size())?;
            let mut compressed = Vec::with_capacity(usize_from_u64(
                "compressed entry size",
                entry.compressed_size(),
            )?);
            io::copy(entry, &mut compressed)?;
            out.resize(expected_size, 0);
            let written = OodleCodec::default()
                .decompress_into(&compressed, out.as_mut_slice())
                .map_err(|_| DecompressError::Oodle { expected_size })?;
            out.truncate(written);
        }
        other @ Compression::Other(_) => {
            return Err(DecompressError::UnsupportedMethod(other));
        }
    }

    peel_azcs(out)?;
    Ok(out.len())
}

fn usize_from_u64(field: &'static str, value: u64) -> Result<usize, DecompressError> {
    usize::try_from(value).map_err(|_| DecompressError::SizeTooLarge { field, value })
}

/// Decompress raw compressed bytes (no zip framing) into a
/// caller-supplied buffer.
///
/// Same compressors as [`decompress_zip_entry_into`], but works
/// from the raw bytes you'd get by reading the zip entry's
/// payload directly. Used by [`crate::PakFile::extract_parallel`]
/// after raw-`pread`-style entry reads bypass `zip::ZipFile`.
///
/// `expected_uncompressed_size` is used to size the output buffer
/// up-front for `Oodle` (which writes into a pre-sized buffer)
/// and as a hint elsewhere.
pub fn decompress_bytes_into(
    method: Compression,
    compressed: &[u8],
    expected_uncompressed_size: usize,
    out: &mut Vec<u8>,
) -> Result<usize, DecompressError> {
    decompress_bytes_raw_into(method, compressed, expected_uncompressed_size, out)?;
    peel_azcs(out)?;
    Ok(out.len())
}

/// Decompress raw zip payload bytes without peeling an inner AZCS
/// wrapper.
///
/// Use this when callers need to inspect or preserve the wrapper
/// itself. Use [`decompress_bytes_into`] when callers want the final
/// asset bytes.
pub fn decompress_bytes_raw_into(
    method: Compression,
    compressed: &[u8],
    expected_uncompressed_size: usize,
    out: &mut Vec<u8>,
) -> Result<usize, DecompressError> {
    out.clear();
    out.reserve(expected_uncompressed_size);

    if compressed.is_empty() {
        return Ok(0);
    }

    match method {
        Compression::Stored => {
            out.extend_from_slice(compressed);
        }
        Compression::Deflated => {
            if compressed.len() >= 2 && compressed[0] == 0x78 && compressed[1] == 0xda {
                let mut zlib = flate2::read::ZlibDecoder::new_with_decompress(
                    Cursor::new(compressed),
                    Decompress::new(true),
                );
                io::copy(&mut zlib, out)?;
            } else {
                let mut deflate = flate2::read::DeflateDecoder::new(Cursor::new(compressed));
                io::copy(&mut deflate, out)?;
            }
        }
        Compression::Oodle => {
            out.resize(expected_uncompressed_size, 0);
            let written = OodleCodec::default()
                .decompress_into(compressed, out.as_mut_slice())
                .map_err(|_| DecompressError::Oodle {
                    expected_size: expected_uncompressed_size,
                })?;
            out.truncate(written);
        }
        other @ Compression::Other(_) => {
            return Err(DecompressError::UnsupportedMethod(other));
        }
    }

    Ok(out.len())
}

/// Internal: if `out` starts with `b"AZCS"`, peel the inner AZCS
/// wrapper. NW often double-wraps with AZCS inside zip.
fn peel_azcs(out: &mut Vec<u8>) -> Result<(), DecompressError> {
    if azcs::is_azcs(out.as_slice()) {
        let mut inner = Vec::with_capacity(out.len());
        let outer = std::mem::take(out);
        {
            let mut cursor = Cursor::new(outer.as_slice());
            let mut reader = azcs::decompress(&mut cursor)?;
            io::copy(&mut reader, &mut inner)?;
        }
        *out = inner;
    }
    Ok(())
}
