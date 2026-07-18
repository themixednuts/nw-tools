//! Parser for LmbrCentral vertex shape assets.
//!
//! Lumberyard-derived engines register these assets as `VertexShapeAsset`.

use glam::Vec3;
use serde::Serialize;
use thiserror::Error;
use uuid::{Uuid, uuid};

/// `VertexShapeAsset` type UUID.
pub const VERTEX_SHAPE_ASSET_TYPE_ID: Uuid = uuid!("EE5F2696-D2A4-4C91-8614-82352BB33D90");

/// `VertexShapeAssetHandler` type UUID.
pub const VERTEX_SHAPE_ASSET_HANDLER_TYPE_ID: Uuid = uuid!("464A863C-0ABD-49B5-864E-39AE7E0E71D8");

const SUPPORTED_VERSION: u32 = 0;
const VERTEX_SIZE: usize = 12;

/// Borrowed view of a `.vshapec` vertex shape asset.
#[derive(Debug, Clone)]
pub struct VertexShapeAsset<'a> {
    version: u32,
    vertices: VertexBytes<'a>,
    metadata: Vec<VertexShapeMetadata<'a>>,
    reserved: VertexShapeReserved,
    height: f32,
}

/// Complete owned projection used by extraction and package metadata.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VertexShape {
    pub version: u32,
    pub vertices: Vec<Vec3>,
    pub metadata: Vec<VertexShapeMetadataOwned>,
    pub reserved: VertexShapeReserved,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VertexShapeMetadataOwned {
    pub key: String,
    pub value: String,
}

/// Parse and own every field in a `.vshapec` product.
pub fn parse_vertex_shape(bytes: &[u8]) -> Result<VertexShape, ParseError> {
    let asset = parse_vertex_shape_asset(bytes)?;
    Ok(VertexShape {
        version: asset.version(),
        vertices: asset.vertices().iter().collect(),
        metadata: asset
            .metadata()
            .iter()
            .map(|entry| VertexShapeMetadataOwned {
                key: entry.key.to_owned(),
                value: entry.value.to_owned(),
            })
            .collect(),
        reserved: asset.reserved(),
        height: asset.height(),
    })
}

impl<'a> VertexShapeAsset<'a> {
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn vertices(&self) -> VertexBytes<'a> {
        self.vertices
    }

    #[must_use]
    pub fn metadata(&self) -> &[VertexShapeMetadata<'a>] {
        &self.metadata
    }

    #[must_use]
    pub const fn reserved(&self) -> VertexShapeReserved {
        self.reserved
    }

    #[must_use]
    pub const fn height(&self) -> f32 {
        self.height
    }
}

/// Reserved values stored after the metadata table.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VertexShapeReserved {
    pub first: u32,
    pub second: u32,
    pub third: u32,
}

impl VertexShapeReserved {
    #[must_use]
    pub const fn new(first: u32, second: u32, third: u32) -> Self {
        Self {
            first,
            second,
            third,
        }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.first == 0 && self.second == 0 && self.third == 0
    }
}

/// Borrowed vertex table.
#[derive(Debug, Clone, Copy)]
pub struct VertexBytes<'a> {
    bytes: &'a [u8],
}

impl<'a> VertexBytes<'a> {
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len() / VERTEX_SIZE
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<Vec3> {
        let offset = index.checked_mul(VERTEX_SIZE)?;
        let chunk = self.bytes.get(offset..offset + VERTEX_SIZE)?;
        Some(read_vec3_chunk(chunk))
    }

    #[must_use]
    pub fn iter(&self) -> VertexIter<'a> {
        VertexIter {
            chunks: self.bytes.chunks_exact(VERTEX_SIZE),
        }
    }
}

impl<'a> IntoIterator for VertexBytes<'a> {
    type Item = Vec3;
    type IntoIter = VertexIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        VertexIter {
            chunks: self.bytes.chunks_exact(VERTEX_SIZE),
        }
    }
}

/// Iterator over borrowed vertex bytes.
#[derive(Debug, Clone)]
pub struct VertexIter<'a> {
    chunks: std::slice::ChunksExact<'a, u8>,
}

impl Iterator for VertexIter<'_> {
    type Item = Vec3;

    fn next(&mut self) -> Option<Self::Item> {
        self.chunks.next().map(read_vec3_chunk)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}

impl ExactSizeIterator for VertexIter<'_> {}

/// One vertex shape metadata entry.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VertexShapeMetadata<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

/// Parse a `.vshapec` vertex shape asset.
pub fn parse_vertex_shape_asset(bytes: &[u8]) -> Result<VertexShapeAsset<'_>, ParseError> {
    let mut cursor = 0;
    let version = read_u32(bytes, &mut cursor)?;
    if version != SUPPORTED_VERSION {
        return Err(ParseError::UnsupportedVersion {
            version,
            expected: SUPPORTED_VERSION,
        });
    }

    let vertex_count = read_count(bytes, &mut cursor, "vertices")?;
    let vertex_bytes = take_array(bytes, &mut cursor, vertex_count, VERTEX_SIZE, "vertices")?;
    let metadata = read_metadata(bytes, &mut cursor)?;
    let reserved_first = read_u32(bytes, &mut cursor)?;
    let reserved_second = read_u32(bytes, &mut cursor)?;
    let height = f32::from_bits(read_u32(bytes, &mut cursor)?);
    let reserved_third = read_u32(bytes, &mut cursor)?;

    if cursor != bytes.len() {
        return Err(ParseError::TrailingBytes {
            remaining: bytes.len() - cursor,
        });
    }

    Ok(VertexShapeAsset {
        version,
        vertices: VertexBytes {
            bytes: vertex_bytes,
        },
        metadata,
        reserved: VertexShapeReserved::new(reserved_first, reserved_second, reserved_third),
        height,
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("vertex shape asset ended while reading {what}")]
    UnexpectedEof { what: &'static str },
    #[error("unsupported vertex shape version {version}, expected {expected}")]
    UnsupportedVersion { version: u32, expected: u32 },
    #[error("vertex shape {what} byte length overflowed")]
    ByteLengthOverflow { what: &'static str },
    #[error("vertex shape has {remaining} trailing bytes")]
    TrailingBytes { remaining: usize },
    #[error("vertex shape {what} is not valid UTF-8")]
    InvalidUtf8 { what: &'static str },
}

fn read_count(bytes: &[u8], cursor: &mut usize, what: &'static str) -> Result<usize, ParseError> {
    usize::try_from(read_u32(bytes, cursor)?).map_err(|_| ParseError::ByteLengthOverflow { what })
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ParseError> {
    let chunk = take(bytes, cursor, 4, "u32")?;
    Ok(read_u32_chunk(chunk))
}

fn take_array<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    count: usize,
    item_len: usize,
    what: &'static str,
) -> Result<&'a [u8], ParseError> {
    let len = count
        .checked_mul(item_len)
        .ok_or(ParseError::ByteLengthOverflow { what })?;
    take(bytes, cursor, len, what)
}

fn take<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    len: usize,
    what: &'static str,
) -> Result<&'a [u8], ParseError> {
    let end = cursor
        .checked_add(len)
        .ok_or(ParseError::ByteLengthOverflow { what })?;
    let chunk = bytes
        .get(*cursor..end)
        .ok_or(ParseError::UnexpectedEof { what })?;
    *cursor = end;
    Ok(chunk)
}

fn read_vec3_chunk(bytes: &[u8]) -> Vec3 {
    debug_assert_eq!(bytes.len(), VERTEX_SIZE);
    Vec3::new(
        f32::from_bits(read_u32_chunk(&bytes[0..4])),
        f32::from_bits(read_u32_chunk(&bytes[4..8])),
        f32::from_bits(read_u32_chunk(&bytes[8..12])),
    )
}

fn read_metadata<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
) -> Result<Vec<VertexShapeMetadata<'a>>, ParseError> {
    let count = read_count(bytes, cursor, "metadata")?;
    let mut metadata = Vec::with_capacity(count);
    for _ in 0..count {
        metadata.push(VertexShapeMetadata {
            key: read_string(bytes, cursor, "metadata key")?,
            value: read_string(bytes, cursor, "metadata value")?,
        });
    }
    Ok(metadata)
}

fn read_string<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    what: &'static str,
) -> Result<&'a str, ParseError> {
    let len = read_count(bytes, cursor, what)?;
    let value = take(bytes, cursor, len, what)?;
    std::str::from_utf8(value).map_err(|_| ParseError::InvalidUtf8 { what })
}

fn read_u32_chunk(bytes: &[u8]) -> u32 {
    debug_assert_eq!(bytes.len(), 4);
    u32::from_le_bytes(bytes.try_into().expect("four-byte u32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_vertex_shape() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        write_vec3(&mut bytes, Vec3::new(1.0, 2.0, 0.0));
        write_vec3(&mut bytes, Vec3::new(4.0, 5.0, 0.0));
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        write_string(&mut bytes, "TerritoryId");
        write_string(&mut bytes, "14:@Edengrove");
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&7_u32.to_le_bytes());
        bytes.extend_from_slice(&64.0_f32.to_le_bytes());
        bytes.extend_from_slice(&9_u32.to_le_bytes());

        let asset = parse_vertex_shape_asset(&bytes).unwrap();

        assert_eq!(asset.vertices().len(), 2);
        assert_eq!(asset.vertices().get(1), Some(Vec3::new(4.0, 5.0, 0.0)));
        assert_eq!(asset.metadata()[0].key, "TerritoryId");
        assert_eq!(asset.reserved(), VertexShapeReserved::new(0, 7, 9));
        assert_eq!(asset.height(), 64.0);
    }

    fn write_vec3(bytes: &mut Vec<u8>, value: Vec3) {
        bytes.extend_from_slice(&value.x.to_le_bytes());
        bytes.extend_from_slice(&value.y.to_le_bytes());
        bytes.extend_from_slice(&value.z.to_le_bytes());
    }

    fn write_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }
}
