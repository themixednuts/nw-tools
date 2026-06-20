//! AzCore compressed-stream envelope.
//!
//! After a zip entry has been decompressed, the resulting bytes may be
//! wrapped in an `AZ::IO::CompressorStream` envelope:
//!
//! ```text
//! +0x00  signature       [u8; 4] = b"AZCS"
//! +0x04  compressor_id   u32 (big-endian)  -- ZLIB | ZSTD
//! +0x08  uncompressed_sz u64 (big-endian)
//! +0x10  payload         (compressor-specific)
//! ```
//!
use std::fmt;
use std::io::{self, Read, Write};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use thiserror::Error;

/// `b"AZCS"` — the AzCore compressed-stream signature.
pub const AZCS_SIGNATURE: &[u8; 4] = b"AZCS";

/// Header size in bytes (signature + id + uncompressed_size).
pub const HEADER_LEN: usize = 16;
const ZLIB_HINT_LEN: usize = 4;

#[derive(Debug, Error)]
pub enum AzcsError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("invalid AZCS signature {actual:02x?} (expected {AZCS_SIGNATURE:02x?})")]
    InvalidSignature { actual: [u8; 4] },

    #[error("unsupported AZCS compressor id: {0:#010x}")]
    UnsupportedCompressor(u32),

    #[error("AZCS ZSTD compressor not yet implemented")]
    Zstd,

    #[error("AZCS input is too large: {len} bytes")]
    InputTooLarge { len: usize },
}

/// AZCS compressor identifier.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AzcsId {
    Zlib = 0x7388_7D3A,
    Zstd = 0x72FD_505E,
}

impl AzcsId {
    /// `const`-friendly conversion from a raw u32.
    #[inline]
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0x7388_7D3A => Some(Self::Zlib),
            0x72FD_505E => Some(Self::Zstd),
            _ => None,
        }
    }

    /// The raw u32 value as it appears on disk.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

impl TryFrom<u32> for AzcsId {
    type Error = AzcsError;

    #[inline]
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::from_u32(value).ok_or(AzcsError::UnsupportedCompressor(value))
    }
}

impl fmt::Display for AzcsId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AzcsId::Zlib => "ZLIB",
            AzcsId::Zstd => "ZSTD",
        })
    }
}

/// Parsed AZCS compressed-stream header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AzcsHeader {
    pub compressor_id: AzcsId,
    pub uncompressed_size: u64,
}

impl AzcsHeader {
    /// Read + validate the AZCS header from `reader`.
    pub fn from_reader<R: Read>(reader: &mut R) -> Result<Self, AzcsError> {
        let mut sig = [0u8; 4];
        reader.read_exact(&mut sig)?;
        if &sig != AZCS_SIGNATURE {
            return Err(AzcsError::InvalidSignature { actual: sig });
        }

        let mut buf4 = [0u8; 4];
        reader.read_exact(&mut buf4)?;
        let compressor_id = AzcsId::try_from(u32::from_be_bytes(buf4))?;

        let mut buf8 = [0u8; 8];
        reader.read_exact(&mut buf8)?;
        let uncompressed_size = u64::from_be_bytes(buf8);

        Ok(Self {
            compressor_id,
            uncompressed_size,
        })
    }

    /// Try to decode the header from a byte slice without consuming
    /// the buffer. Returns `None` if the slice is too short or the
    /// signature doesn't match.
    #[must_use]
    pub fn peek(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < HEADER_LEN || &bytes[0..4] != AZCS_SIGNATURE {
            return None;
        }
        let compressor_id = AzcsId::from_u32(u32::from_be_bytes(bytes[4..8].try_into().ok()?))?;
        let uncompressed_size = u64::from_be_bytes(bytes[8..16].try_into().ok()?);
        Some(Self {
            compressor_id,
            uncompressed_size,
        })
    }
}

/// Cheap signature-only check: does `bytes` start with `b"AZCS"`?
#[inline]
#[must_use]
pub const fn is_azcs(bytes: &[u8]) -> bool {
    bytes.len() >= AZCS_SIGNATURE.len()
        && bytes[0] == AZCS_SIGNATURE[0]
        && bytes[1] == AZCS_SIGNATURE[1]
        && bytes[2] == AZCS_SIGNATURE[2]
        && bytes[3] == AZCS_SIGNATURE[3]
}

/// Wrap `reader` in an AZCS decompressor. Reads and validates the
/// header, then returns a streaming reader over the decompressed
/// payload.
///
/// Currently only [`AzcsId::Zlib`] is implemented; ZSTD returns
/// [`AzcsError::Zstd`].
pub fn decompress<R: Read>(reader: &mut R) -> Result<impl Read + use<'_, R>, AzcsError> {
    let header = AzcsHeader::from_reader(reader)?;
    match header.compressor_id {
        AzcsId::Zlib => {
            // Existing files carry four compressor-specific bytes before the
            // zlib stream. They appear to be a block-size hint; decoding does
            // not need the value.
            let mut skip = [0u8; ZLIB_HINT_LEN];
            reader.read_exact(&mut skip)?;
            Ok(ZlibDecoder::new(reader))
        }
        AzcsId::Zstd => Err(AzcsError::Zstd),
    }
}

/// Compress `bytes` into an AZCS ZLIB envelope.
///
/// # Errors
///
/// Returns an error if the payload length cannot be represented in the
/// envelope header or if zlib compression fails.
pub fn compress_zlib(bytes: &[u8]) -> Result<Vec<u8>, AzcsError> {
    let mut out = Vec::with_capacity(HEADER_LEN + ZLIB_HINT_LEN + bytes.len());
    compress_zlib_into(bytes, &mut out)?;
    Ok(out)
}

/// Compress `bytes` into an AZCS ZLIB envelope, appending to `out`.
///
/// # Errors
///
/// Returns an error if the payload length cannot be represented in the
/// envelope header or if zlib compression fails.
pub fn compress_zlib_into(bytes: &[u8], out: &mut Vec<u8>) -> Result<(), AzcsError> {
    let uncompressed_size =
        u64::try_from(bytes.len()).map_err(|_| AzcsError::InputTooLarge { len: bytes.len() })?;
    out.extend_from_slice(AZCS_SIGNATURE);
    out.extend_from_slice(&AzcsId::Zlib.as_u32().to_be_bytes());
    out.extend_from_slice(&uncompressed_size.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    out.extend_from_slice(&encoder.finish()?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azcs_id_round_trip() {
        assert_eq!(AzcsId::from_u32(0x7388_7D3A), Some(AzcsId::Zlib));
        assert_eq!(AzcsId::from_u32(0x72FD_505E), Some(AzcsId::Zstd));
        assert_eq!(AzcsId::from_u32(0xDEAD_BEEF), None);
        assert_eq!(AzcsId::Zlib.as_u32(), 0x7388_7D3A);
        assert_eq!(AzcsId::Zstd.as_u32(), 0x72FD_505E);
    }

    #[test]
    fn is_azcs_check() {
        assert!(is_azcs(b"AZCS extra"));
        assert!(!is_azcs(b"AZC"));
        assert!(!is_azcs(b"NOPE"));
        assert!(!is_azcs(b""));
    }

    #[test]
    fn peek_header() {
        let mut buf = Vec::new();
        buf.extend_from_slice(AZCS_SIGNATURE);
        buf.extend_from_slice(&AzcsId::Zlib.as_u32().to_be_bytes());
        buf.extend_from_slice(&1234u64.to_be_bytes());
        let h = AzcsHeader::peek(&buf).unwrap();
        assert_eq!(h.compressor_id, AzcsId::Zlib);
        assert_eq!(h.uncompressed_size, 1234);

        assert!(AzcsHeader::peek(b"NOPE12345678abcd").is_none());
        assert!(AzcsHeader::peek(b"AZCS").is_none()); // too short
    }

    #[test]
    fn zlib_compress_round_trips_through_reader() {
        let expected = b"hello object stream payload";
        let wrapped = compress_zlib(expected).unwrap();
        let header = AzcsHeader::peek(&wrapped).unwrap();
        assert_eq!(header.compressor_id, AzcsId::Zlib);
        assert_eq!(header.uncompressed_size, expected.len() as u64);

        let mut cursor = std::io::Cursor::new(wrapped);
        let mut reader = decompress(&mut cursor).unwrap();
        let mut actual = Vec::new();
        reader.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, expected);
    }
}
