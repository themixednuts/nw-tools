//! Parser for Coat terrain `region.tractmap.tif` source assets.

use std::{io::Cursor, path::Path};

use thiserror::Error;
use tiff::{TiffError, decoder::Decoder, tags::Tag};
use weezl::{BitOrder, LzwError, LzwStatus};

pub const EXPECTED_BITS_PER_SAMPLE: u16 = 8;
pub const EXPECTED_SAMPLES_PER_PIXEL: u16 = 1;
pub const EXPECTED_ORIENTATION: u16 = 1;
pub const EXPECTED_PLANAR_CONFIGURATION: u16 = 1;
pub const EXPECTED_PHOTOMETRIC_INTERPRETATION: u16 = 3;
pub const TRACT_MAP_FILE_NAME: &str = "region.tractmap.tif";

const COMPRESSION_NONE: u16 = 1;
const COMPRESSION_LZW: u16 = 5;

/// Parsed tract indices in terrain-space row-major order, with `(0, 0)` at
/// bottom-left (TIFF stores the first row at the top).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TractMap {
    pub width: u32,
    pub height: u32,
    pub tags: TractMapTags,
    pub tracts: Vec<u8>,
}

impl TractMap {
    pub fn parse_tiff(bytes: &[u8]) -> Result<Self, TractMapError> {
        let mut decoder = Decoder::new(Cursor::new(bytes)).map_err(TractMapError::Decode)?;
        let tags = TractMapTags::read(&mut decoder)?;
        let strips = TiffStrips::read(&mut decoder)?;
        tags.validate()?;

        if tags.width == 0 || tags.height == 0 {
            return Err(TractMapError::EmptyImage {
                width: tags.width,
                height: tags.height,
            });
        }
        if tags.width != tags.height {
            return Err(TractMapError::NonSquare {
                width: tags.width,
                height: tags.height,
            });
        }

        let expected_len = sample_len(tags.width, tags.height)?;
        let mut tracts = decode_strips(bytes, tags, &strips, expected_len)?;
        if tracts.len() != expected_len {
            return Err(TractMapError::SampleCountMismatch {
                width: tags.width,
                height: tags.height,
                expected: expected_len,
                found: tracts.len(),
            });
        }
        flip_rows(&mut tracts, tags.width as usize);

        Ok(Self {
            width: tags.width,
            height: tags.height,
            tags,
            tracts,
        })
    }

    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.tracts.len()
    }

    #[must_use]
    pub fn min_tract(&self) -> Option<u8> {
        self.tracts.iter().copied().min()
    }

    #[must_use]
    pub fn max_tract(&self) -> Option<u8> {
        self.tracts.iter().copied().max()
    }

    #[must_use]
    pub fn summary(&self) -> TractMapSummary {
        TractMapSummary {
            width: self.width,
            height: self.height,
            samples: self.sample_count(),
            min_tract: self.min_tract(),
            max_tract: self.max_tract(),
        }
    }

    #[must_use]
    pub fn sample_terrain_xy(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()?;
        self.tracts.get(index).copied()
    }

    #[must_use]
    pub fn sample_top_left(&self, x: u32, y: u32) -> Option<u8> {
        (y < self.height)
            .then(|| self.sample_terrain_xy(x, self.height - y - 1))
            .flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TractMapSummary {
    pub width: u32,
    pub height: u32,
    pub samples: usize,
    pub min_tract: Option<u8>,
    pub max_tract: Option<u8>,
}

pub fn summarize_tract_map(bytes: &[u8]) -> Result<TractMapSummary, TractMapError> {
    Ok(TractMap::parse_tiff(bytes)?.summary())
}

#[must_use]
pub fn is_tract_map_name(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(TRACT_MAP_FILE_NAME))
}

/// TIFF tags validated by the Coat client before applying tract data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TractMapTags {
    pub width: u32,
    pub height: u32,
    pub bits_per_sample: u16,
    pub compression: u16,
    pub samples_per_pixel: u16,
    pub rows_per_strip: u32,
    pub orientation: u16,
    pub planar_configuration: u16,
    pub photometric_interpretation: u16,
}

impl TractMapTags {
    fn read<R: std::io::Read + std::io::Seek>(
        decoder: &mut Decoder<R>,
    ) -> Result<Self, TractMapError> {
        let (width, height) = decoder.dimensions().map_err(TractMapError::Decode)?;
        let bits_per_sample = read_u16_tag(decoder, Tag::BitsPerSample)?;
        let photometric_interpretation = read_u16_tag(decoder, Tag::PhotometricInterpretation)?;
        let samples_per_pixel = match decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(TractMapError::Decode)?
        {
            Some(samples) => samples,
            None if matches!(photometric_interpretation, 1 | 3) => 1,
            None if photometric_interpretation == 2 => 3,
            None => {
                return Err(TractMapError::MissingTag {
                    tag: "TIFFTAG_SAMPLESPERPIXEL",
                });
            }
        };

        Ok(Self {
            width,
            height,
            bits_per_sample,
            compression: decoder
                .find_tag_unsigned::<u16>(Tag::Compression)
                .map_err(TractMapError::Decode)?
                .unwrap_or(COMPRESSION_NONE),
            samples_per_pixel,
            rows_per_strip: decoder
                .find_tag_unsigned::<u32>(Tag::RowsPerStrip)
                .map_err(TractMapError::Decode)?
                .unwrap_or(height),
            orientation: read_u16_tag(decoder, Tag::Orientation)?,
            planar_configuration: read_u16_tag(decoder, Tag::PlanarConfiguration)?,
            photometric_interpretation,
        })
    }

    fn validate(&self) -> Result<(), TractMapError> {
        expect_tag(
            "TIFFTAG_BITSPERSAMPLE",
            EXPECTED_BITS_PER_SAMPLE,
            self.bits_per_sample,
        )?;
        expect_tag(
            "TIFFTAG_SAMPLESPERPIXEL",
            EXPECTED_SAMPLES_PER_PIXEL,
            self.samples_per_pixel,
        )?;
        expect_tag(
            "TIFFTAG_PLANARCONFIG",
            EXPECTED_PLANAR_CONFIGURATION,
            self.planar_configuration,
        )?;
        expect_tag(
            "TIFFTAG_ORIENTATION",
            EXPECTED_ORIENTATION,
            self.orientation,
        )?;
        expect_tag(
            "TIFFTAG_PHOTOMETRIC",
            EXPECTED_PHOTOMETRIC_INTERPRETATION,
            self.photometric_interpretation,
        )?;
        match self.compression {
            COMPRESSION_NONE | COMPRESSION_LZW => Ok(()),
            other => Err(TractMapError::UnsupportedCompression(other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TiffStrips {
    offsets: Vec<u32>,
    byte_counts: Vec<u32>,
}

impl TiffStrips {
    fn read<R: std::io::Read + std::io::Seek>(
        decoder: &mut Decoder<R>,
    ) -> Result<Self, TractMapError> {
        let offsets = decoder
            .get_tag_u32_vec(Tag::StripOffsets)
            .map_err(TractMapError::Decode)?;
        let byte_counts = decoder
            .get_tag_u32_vec(Tag::StripByteCounts)
            .map_err(TractMapError::Decode)?;
        if offsets.len() != byte_counts.len() {
            return Err(TractMapError::StripTableMismatch {
                offsets: offsets.len(),
                byte_counts: byte_counts.len(),
            });
        }
        Ok(Self {
            offsets,
            byte_counts,
        })
    }
}

#[derive(Debug, Error)]
pub enum TractMapError {
    #[error("decode tract map TIFF: {0}")]
    Decode(#[from] TiffError),
    #[error("missing required tract map TIFF tag `{tag}`")]
    MissingTag { tag: &'static str },
    #[error("tract map TIFF tag `{tag}` expected `{expected}`, found `{actual}`")]
    UnexpectedTag {
        tag: &'static str,
        expected: u16,
        actual: u16,
    },
    #[error("empty tract map image: {width} x {height}")]
    EmptyImage { width: u32, height: u32 },
    #[error("tract map image is not square: {width} x {height}")]
    NonSquare { width: u32, height: u32 },
    #[error("tract map dimensions are too large: {width} x {height}")]
    DimensionTooLarge { width: u32, height: u32 },
    #[error("unsupported tract map TIFF compression `{0}`")]
    UnsupportedCompression(u16),
    #[error("tract map TIFF strip table mismatch: {offsets} offsets, {byte_counts} byte counts")]
    StripTableMismatch { offsets: usize, byte_counts: usize },
    #[error("tract map TIFF strip {strip} is out of bounds")]
    StripOutOfBounds { strip: usize },
    #[error("decode tract map TIFF LZW: {0}")]
    Lzw(#[from] LzwError),
    #[error("tract map TIFF LZW made no progress")]
    LzwNoProgress,
    #[error(
        "tract map sample count mismatch for {width} x {height}: expected {expected}, found {found}"
    )]
    SampleCountMismatch {
        width: u32,
        height: u32,
        expected: usize,
        found: usize,
    },
}

fn decode_strips(
    bytes: &[u8],
    tags: TractMapTags,
    strips: &TiffStrips,
    expected_len: usize,
) -> Result<Vec<u8>, TractMapError> {
    let mut output = Vec::with_capacity(expected_len);
    for (strip, (&offset, &byte_count)) in
        strips.offsets.iter().zip(&strips.byte_counts).enumerate()
    {
        let compressed = strip_bytes(bytes, offset, byte_count, strip)?;
        match tags.compression {
            COMPRESSION_NONE => output.extend_from_slice(compressed),
            COMPRESSION_LZW => {
                let remaining = expected_len.saturating_sub(output.len());
                output.extend(decode_lzw_strip(compressed, remaining)?);
            }
            other => return Err(TractMapError::UnsupportedCompression(other)),
        }
    }
    Ok(output)
}

fn strip_bytes(
    bytes: &[u8],
    offset: u32,
    byte_count: u32,
    strip: usize,
) -> Result<&[u8], TractMapError> {
    let start = offset as usize;
    let end = start
        .checked_add(byte_count as usize)
        .ok_or(TractMapError::StripOutOfBounds { strip })?;
    bytes
        .get(start..end)
        .ok_or(TractMapError::StripOutOfBounds { strip })
}

fn decode_lzw_strip(bytes: &[u8], expected_len: usize) -> Result<Vec<u8>, TractMapError> {
    let mut decoder = weezl::decode::Configuration::with_tiff_size_switch(BitOrder::Msb, 8)
        .with_yield_on_full_buffer(true)
        .build();
    let mut output = vec![0; expected_len];
    let mut input_offset = 0;
    let mut output_offset = 0;

    loop {
        let result = decoder.decode_bytes(&bytes[input_offset..], &mut output[output_offset..]);
        input_offset += result.consumed_in;
        output_offset += result.consumed_out;
        match result.status {
            Ok(LzwStatus::Done) => break,
            Ok(LzwStatus::NoProgress) if output_offset == expected_len => break,
            Ok(LzwStatus::NoProgress) => return Err(TractMapError::LzwNoProgress),
            Ok(LzwStatus::Ok) if output_offset == expected_len => break,
            Ok(LzwStatus::Ok) if result.consumed_in != 0 || result.consumed_out != 0 => {}
            Ok(LzwStatus::Ok) => return Err(TractMapError::LzwNoProgress),
            Err(error) => return Err(TractMapError::Lzw(error)),
        }
    }
    output.truncate(output_offset);
    Ok(output)
}

fn read_u16_tag<R: std::io::Read + std::io::Seek>(
    decoder: &mut Decoder<R>,
    tag: Tag,
) -> Result<u16, TractMapError> {
    decoder
        .find_tag_unsigned::<u16>(tag)
        .map_err(TractMapError::Decode)?
        .ok_or(TractMapError::MissingTag { tag: tag_name(tag) })
}

const fn expect_tag(tag: &'static str, expected: u16, actual: u16) -> Result<(), TractMapError> {
    if actual == expected {
        Ok(())
    } else {
        Err(TractMapError::UnexpectedTag {
            tag,
            expected,
            actual,
        })
    }
}

const fn tag_name(tag: Tag) -> &'static str {
    match tag {
        Tag::BitsPerSample => "TIFFTAG_BITSPERSAMPLE",
        Tag::SamplesPerPixel => "TIFFTAG_SAMPLESPERPIXEL",
        Tag::PlanarConfiguration => "TIFFTAG_PLANARCONFIG",
        Tag::Orientation => "TIFFTAG_ORIENTATION",
        Tag::PhotometricInterpretation => "TIFFTAG_PHOTOMETRIC",
        _ => "TIFFTAG_UNKNOWN",
    }
}

fn sample_len(width: u32, height: u32) -> Result<usize, TractMapError> {
    usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| TractMapError::DimensionTooLarge { width, height })
}

fn flip_rows(samples: &mut [u8], width: usize) {
    if width == 0 {
        return;
    }
    let height = samples.len() / width;
    for y in 0..height / 2 {
        let top = y * width;
        let bottom = (height - y - 1) * width;
        for x in 0..width {
            samples.swap(top + x, bottom + x);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_palette_tiff_in_terrain_order() {
        let bytes = tract_tiff([1, 2, 3, 4], 2, 2, EXPECTED_PHOTOMETRIC_INTERPRETATION);
        let map = TractMap::parse_tiff(&bytes).unwrap();
        assert_eq!(map.tracts, [3, 4, 1, 2]);
        assert_eq!(map.sample_terrain_xy(0, 0), Some(3));
        assert_eq!(map.sample_top_left(0, 0), Some(1));
        assert!(is_tract_map_name("LEVELS/R/REGION.TRACTMAP.TIF"));
    }

    #[test]
    fn parses_lzw_palette_tiff() {
        let mut encoder =
            weezl::encode::Configuration::with_tiff_size_switch(BitOrder::Msb, 8).build();
        let payload = encoder.encode(&[1, 1, 2, 2]).unwrap();
        let bytes = tract_tiff_payload(
            &payload,
            2,
            2,
            EXPECTED_PHOTOMETRIC_INTERPRETATION,
            COMPRESSION_LZW,
        );
        assert_eq!(TractMap::parse_tiff(&bytes).unwrap().tracts, [2, 2, 1, 1]);
    }

    fn tract_tiff<const N: usize>(
        pixels: [u8; N],
        width: u32,
        height: u32,
        photometric: u16,
    ) -> Vec<u8> {
        tract_tiff_payload(&pixels, width, height, photometric, COMPRESSION_NONE)
    }

    fn tract_tiff_payload(
        payload: &[u8],
        width: u32,
        height: u32,
        photometric: u16,
        compression: u16,
    ) -> Vec<u8> {
        const ENTRY_COUNT: u16 = 12;
        let ifd_offset = 8_u32;
        let ifd_len = 2 + u32::from(ENTRY_COUNT) * 12 + 4;
        let pixel_offset = ifd_offset + ifd_len;
        let color_map_offset = pixel_offset + payload.len() as u32;
        let mut bytes = Vec::new();
        bytes.extend(b"II");
        bytes.extend(42_u16.to_le_bytes());
        bytes.extend(ifd_offset.to_le_bytes());
        bytes.extend(ENTRY_COUNT.to_le_bytes());
        entry(&mut bytes, 256, 4, 1, width);
        entry(&mut bytes, 257, 4, 1, height);
        entry(&mut bytes, 258, 3, 1, u32::from(EXPECTED_BITS_PER_SAMPLE));
        entry(&mut bytes, 259, 3, 1, u32::from(compression));
        entry(&mut bytes, 262, 3, 1, u32::from(photometric));
        entry(&mut bytes, 273, 4, 1, pixel_offset);
        entry(&mut bytes, 274, 3, 1, u32::from(EXPECTED_ORIENTATION));
        entry(&mut bytes, 277, 3, 1, u32::from(EXPECTED_SAMPLES_PER_PIXEL));
        entry(&mut bytes, 278, 4, 1, height);
        entry(&mut bytes, 279, 4, 1, payload.len() as u32);
        entry(
            &mut bytes,
            284,
            3,
            1,
            u32::from(EXPECTED_PLANAR_CONFIGURATION),
        );
        entry(&mut bytes, 320, 3, 768, color_map_offset);
        bytes.extend(0_u32.to_le_bytes());
        bytes.extend(payload);
        bytes.resize(color_map_offset as usize + 768 * 2, 0);
        bytes
    }

    fn entry(bytes: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value: u32) {
        bytes.extend(tag.to_le_bytes());
        bytes.extend(ty.to_le_bytes());
        bytes.extend(count.to_le_bytes());
        bytes.extend(value.to_le_bytes());
    }
}
