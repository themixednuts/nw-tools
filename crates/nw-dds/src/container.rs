use thiserror::Error as ThisError;

use crate::{
    DDPF_ALPHA_PIXELS, DDPF_LUMINANCE, DDPF_RGB, DDS_FILE_HEADER_LEN, Dds, DdsError, PixelFormat,
    SplitPart,
};

const KTX2_ID: &[u8; 12] = b"\xABKTX 20\xBB\r\n\x1A\n";
const KTX2_HEADER_LEN: u64 = 80;
const KTX2_LEVEL_INDEX_LEN: u64 = 24;
const KTX2_SUPERCOMPRESSION_NONE: u32 = 0;

const DDS_CAPS2_CUBEMAP: u32 = 0x0000_0200;
const DX10_RESOURCE_DIMENSION_TEXTURE_1D: u32 = 2;
const DX10_RESOURCE_DIMENSION_TEXTURE_3D: u32 = 4;
const DX10_RESOURCE_MISC_TEXTURE_CUBE: u32 = 0x4;

const VK_FORMAT_R8_UNORM: u32 = 9;
const VK_FORMAT_R8G8_UNORM: u32 = 16;
const VK_FORMAT_R8G8B8A8_UNORM: u32 = 37;
const VK_FORMAT_R8G8B8A8_SRGB: u32 = 43;
const VK_FORMAT_B8G8R8A8_UNORM: u32 = 44;
const VK_FORMAT_B8G8R8A8_SRGB: u32 = 50;
const VK_FORMAT_R16_SFLOAT: u32 = 76;
const VK_FORMAT_R16G16_SFLOAT: u32 = 83;
const VK_FORMAT_R16G16B16A16_SFLOAT: u32 = 97;
const VK_FORMAT_R32_SFLOAT: u32 = 100;
const VK_FORMAT_R32G32_SFLOAT: u32 = 103;
const VK_FORMAT_R32G32B32_SFLOAT: u32 = 106;
const VK_FORMAT_R32G32B32A32_SFLOAT: u32 = 109;
const VK_FORMAT_BC1_RGBA_UNORM_BLOCK: u32 = 133;
const VK_FORMAT_BC1_RGBA_SRGB_BLOCK: u32 = 134;
const VK_FORMAT_BC2_UNORM_BLOCK: u32 = 135;
const VK_FORMAT_BC2_SRGB_BLOCK: u32 = 136;
const VK_FORMAT_BC3_UNORM_BLOCK: u32 = 137;
const VK_FORMAT_BC3_SRGB_BLOCK: u32 = 138;
const VK_FORMAT_BC4_UNORM_BLOCK: u32 = 139;
const VK_FORMAT_BC4_SNORM_BLOCK: u32 = 140;
const VK_FORMAT_BC5_UNORM_BLOCK: u32 = 141;
const VK_FORMAT_BC5_SNORM_BLOCK: u32 = 142;
const VK_FORMAT_BC6H_UFLOAT_BLOCK: u32 = 143;
const VK_FORMAT_BC6H_SFLOAT_BLOCK: u32 = 144;
const VK_FORMAT_BC7_UNORM_BLOCK: u32 = 145;
const VK_FORMAT_BC7_SRGB_BLOCK: u32 = 146;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ktx2 {
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sidecar<'a> {
    part: SplitPart,
    bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq, ThisError)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Dds(#[from] DdsError),

    #[error("unsupported DDS format {format}")]
    UnsupportedFormat { format: String },

    #[error("unsupported DDS shape: {reason}")]
    UnsupportedShape { reason: &'static str },

    #[error("unsupported Vulkan format {vk_format}")]
    UnsupportedVulkanFormat { vk_format: u32 },

    #[error("DDS payload contains {actual} bytes, expected {expected}")]
    PayloadSize { expected: u64, actual: usize },

    #[error("DDS mip level {level} contains {actual} bytes, expected {expected}")]
    MipSize {
        level: u32,
        expected: u64,
        actual: usize,
    },

    #[error("missing DDS split mip {index}")]
    MissingSidecar { index: u32 },

    #[error("duplicate DDS split mip {index}")]
    DuplicateSidecar { index: u32 },

    #[error("unexpected DDS split part {part}")]
    UnexpectedSidecar { part: SplitPart },

    #[error("{what} is too large for KTX2")]
    SizeOverflow { what: &'static str },
}

impl Ktx2 {
    /// Convert DDS bytes and optional split-mip sidecars to a KTX2 container.
    ///
    /// This preserves the original encoded texture blocks. It does not decode
    /// or transcode the texture format.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when the DDS header is invalid, the encoded format is
    /// not supported, required split sidecars are missing, mip byte counts do
    /// not match the header, or the resulting KTX2 indexes would overflow.
    pub fn from_dds<'a>(bytes: &'a [u8], sidecars: &[Sidecar<'a>]) -> Result<Self, Error> {
        let dds = Dds::parse(bytes)?;
        let texture = Texture::from_dds(&dds)?;
        let payload = dds.payload(bytes).ok_or(Error::PayloadSize {
            expected: u64::try_from(dds.payload_bytes()).map_err(|_| Error::SizeOverflow {
                what: "DDS payload length",
            })?,
            actual: bytes.len().saturating_sub(DDS_FILE_HEADER_LEN),
        })?;
        let levels = Levels::from_dds(&dds, texture, payload, sidecars)?;
        let bytes = texture.write(&levels)?;
        Ok(Self { bytes })
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// An RGBA8 image decoded from a texture (row-major, tightly packed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decode the largest mip of a DDS to RGBA8, assembling split sidecars first.
///
/// Supports the block formats New World ships (BC1–BC7) and plain 32-bit
/// RGBA/BGRA. Other formats return [`Error::UnsupportedVulkanFormat`].
///
/// # Errors
///
/// Returns [`Error`] when the DDS is invalid, sidecars are missing/mismatched, or
/// the encoded format cannot be decoded.
pub fn decode_top_mip<'a>(bytes: &'a [u8], sidecars: &[Sidecar<'a>]) -> Result<DecodedImage, Error> {
    let dds = Dds::parse(bytes)?;
    let texture = Texture::from_dds(&dds)?;
    let payload = dds.payload(bytes).ok_or(Error::PayloadSize {
        expected: u64::try_from(dds.payload_bytes()).unwrap_or(u64::MAX),
        actual: bytes.len().saturating_sub(DDS_FILE_HEADER_LEN),
    })?;
    let levels = Levels::from_dds(&dds, texture, payload, sidecars)?;
    let blocks = levels.bytes.first().copied().ok_or(Error::UnsupportedShape {
        reason: "texture has no mip levels",
    })?;
    let width = texture.width.max(1);
    let height = texture.height.max(1);
    let rgba = decode_rgba(texture.format.vk, blocks, width as usize, height as usize)?;
    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

/// Decode every mip level of a DDS to RGBA8, largest first, assembling split
/// sidecars first. Like [`decode_top_mip`] but returns the full mip chain so a
/// viewer can step through levels.
///
/// # Errors
///
/// Returns [`Error`] when the DDS is invalid, sidecars are missing/mismatched, or
/// any level's format cannot be decoded.
pub fn decode_all_mips<'a>(
    bytes: &'a [u8],
    sidecars: &[Sidecar<'a>],
) -> Result<Vec<DecodedImage>, Error> {
    let dds = Dds::parse(bytes)?;
    let texture = Texture::from_dds(&dds)?;
    let payload = dds.payload(bytes).ok_or(Error::PayloadSize {
        expected: u64::try_from(dds.payload_bytes()).unwrap_or(u64::MAX),
        actual: bytes.len().saturating_sub(DDS_FILE_HEADER_LEN),
    })?;
    let levels = Levels::from_dds(&dds, texture, payload, sidecars)?;
    let mut images = Vec::with_capacity(levels.bytes.len());
    for (level, blocks) in levels.bytes.iter().enumerate() {
        let level = level as u32;
        let width = mip_extent(texture.width, level).max(1);
        let height = mip_extent(texture.height, level).max(1);
        let rgba = decode_rgba(texture.format.vk, blocks, width as usize, height as usize)?;
        images.push(DecodedImage {
            width,
            height,
            rgba,
        });
    }
    Ok(images)
}

/// Decode the largest mip that lives in the DDS header file itself — the
/// persistent (smallest) mips for a split texture, or the whole image for a
/// non-split one — *without reading any split sidecars*.
///
/// This is the cheap path for thumbnails: no large sidecar reads, and only a small
/// mip is decoded. For a full-resolution image use [`decode_top_mip`].
///
/// # Errors
///
/// Returns [`Error`] when the DDS is invalid or the header mip cannot be decoded.
pub fn decode_header_mip(bytes: &[u8]) -> Result<DecodedImage, Error> {
    let dds = Dds::parse(bytes)?;
    let texture = Texture::from_dds(&dds)?;
    let payload = dds.payload(bytes).ok_or(Error::PayloadSize {
        expected: u64::try_from(dds.payload_bytes()).unwrap_or(u64::MAX),
        actual: bytes.len().saturating_sub(DDS_FILE_HEADER_LEN),
    })?;
    let sizes = texture.level_sizes()?;
    let mipmaps = sizes.len();
    // For a split texture the header holds the persistent (smallest) mips, at the
    // tail of the chain; for a non-split texture it holds the whole chain.
    let start_level = if dds.is_split() {
        let persistent = usize::from(dds.header().persistent_mips()).min(mipmaps);
        mipmaps.saturating_sub(persistent)
    } else {
        0
    };
    let chain = slice_chain(payload, &sizes[start_level..], start_level)?;
    let blocks = chain.first().copied().ok_or(Error::UnsupportedShape {
        reason: "header has no mip levels",
    })?;
    let level = u32::try_from(start_level).unwrap_or(0);
    let width = mip_extent(texture.width, level).max(1);
    let height = mip_extent(texture.height, level).max(1);
    let rgba = decode_rgba(texture.format.vk, blocks, width as usize, height as usize)?;
    Ok(DecodedImage { width, height, rgba })
}

/// Decode one mip sized to cover `max_dim` pixels on its longest edge (the
/// smallest mip that still does, so it's crisp when downscaled to a thumbnail) —
/// reading only the single split sidecar that mip needs, via `fetch`, or nothing
/// but the header when the target mip is persistent.
///
/// This is the thumbnail path: it fills a grid cell without decoding the full
/// top mip or reading every large sidecar.
///
/// # Errors
///
/// Returns [`Error`] when the DDS is invalid, the needed sidecar is missing or the
/// wrong size, or the mip cannot be decoded.
pub fn decode_mip_max(
    bytes: &[u8],
    max_dim: u32,
    fetch: impl FnOnce(SplitPart) -> Option<Vec<u8>>,
) -> Result<DecodedImage, Error> {
    let dds = Dds::parse(bytes)?;
    let texture = Texture::from_dds(&dds)?;
    let sizes = texture.level_sizes()?;
    let mipmaps = sizes.len();
    if mipmaps == 0 {
        return Err(Error::UnsupportedShape {
            reason: "texture has no mip levels",
        });
    }

    // Levels run largest (0) to smallest. Walk down while still >= max_dim, so we
    // land on the smallest mip that still covers the target (or level 0 if the
    // whole texture is smaller).
    let dim_at = |level: u32| mip_extent(texture.width, level).max(mip_extent(texture.height, level)).max(1);
    let mut target = 0u32;
    for level in 0..mipmaps as u32 {
        if dim_at(level) >= max_dim {
            target = level;
        } else {
            break;
        }
    }

    let split_count = if dds.is_split() {
        let persistent = usize::from(dds.header().persistent_mips()).min(mipmaps);
        mipmaps - persistent
    } else {
        0
    };
    let width = mip_extent(texture.width, target).max(1);
    let height = mip_extent(texture.height, target).max(1);
    let target_usize = target as usize;

    if target_usize >= split_count {
        // Persistent mip — sliced straight from the header payload, no sidecar read.
        let payload = dds.payload(bytes).ok_or(Error::PayloadSize {
            expected: u64::try_from(dds.payload_bytes()).unwrap_or(u64::MAX),
            actual: bytes.len().saturating_sub(DDS_FILE_HEADER_LEN),
        })?;
        let chain = slice_chain(payload, &sizes[split_count..], split_count)?;
        let blocks = *chain.get(target_usize - split_count).ok_or(Error::UnsupportedShape {
            reason: "missing header mip",
        })?;
        let rgba = decode_rgba(texture.format.vk, blocks, width as usize, height as usize)?;
        Ok(DecodedImage { width, height, rgba })
    } else {
        // Split mip — read just this one sidecar (index = split_count - level).
        let index = u32::try_from(split_count - target_usize).unwrap_or(u32::MAX);
        let sidecar = fetch(SplitPart::Mip { index, alpha: false })
            .ok_or(Error::MissingSidecar { index })?;
        check_mip_size(target, sizes[target_usize], sidecar.len())?;
        let rgba = decode_rgba(texture.format.vk, &sidecar, width as usize, height as usize)?;
        Ok(DecodedImage { width, height, rgba })
    }
}

fn decode_rgba(vk: u32, data: &[u8], width: usize, height: usize) -> Result<Vec<u8>, Error> {
    let pixels = width.checked_mul(height).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    match vk {
        VK_FORMAT_R8G8B8A8_UNORM | VK_FORMAT_R8G8B8A8_SRGB => return plain_rgba(data, pixels, false),
        VK_FORMAT_B8G8R8A8_UNORM | VK_FORMAT_B8G8R8A8_SRGB => return plain_rgba(data, pixels, true),
        _ => {}
    }
    let mut out = vec![0u32; pixels];
    let unsupported = || Error::UnsupportedVulkanFormat { vk_format: vk };
    let result = match vk {
        VK_FORMAT_BC1_RGBA_UNORM_BLOCK | VK_FORMAT_BC1_RGBA_SRGB_BLOCK => {
            texture2ddecoder::decode_bc1(data, width, height, &mut out)
        }
        VK_FORMAT_BC2_UNORM_BLOCK | VK_FORMAT_BC2_SRGB_BLOCK => {
            texture2ddecoder::decode_bc2(data, width, height, &mut out)
        }
        VK_FORMAT_BC3_UNORM_BLOCK | VK_FORMAT_BC3_SRGB_BLOCK => {
            texture2ddecoder::decode_bc3(data, width, height, &mut out)
        }
        VK_FORMAT_BC4_UNORM_BLOCK | VK_FORMAT_BC4_SNORM_BLOCK => {
            texture2ddecoder::decode_bc4(data, width, height, &mut out)
        }
        VK_FORMAT_BC5_UNORM_BLOCK | VK_FORMAT_BC5_SNORM_BLOCK => {
            texture2ddecoder::decode_bc5(data, width, height, &mut out)
        }
        VK_FORMAT_BC6H_UFLOAT_BLOCK => {
            texture2ddecoder::decode_bc6_unsigned(data, width, height, &mut out)
        }
        VK_FORMAT_BC6H_SFLOAT_BLOCK => {
            texture2ddecoder::decode_bc6_signed(data, width, height, &mut out)
        }
        VK_FORMAT_BC7_UNORM_BLOCK | VK_FORMAT_BC7_SRGB_BLOCK => {
            texture2ddecoder::decode_bc7(data, width, height, &mut out)
        }
        _ => return Err(unsupported()),
    };
    result.map_err(|_| unsupported())?;
    // texture2ddecoder yields 0xAARRGGBB per pixel; expand to RGBA bytes.
    let mut rgba = Vec::with_capacity(pixels * 4);
    for color in out {
        rgba.push((color >> 16) as u8);
        rgba.push((color >> 8) as u8);
        rgba.push(color as u8);
        rgba.push((color >> 24) as u8);
    }
    Ok(rgba)
}

fn plain_rgba(data: &[u8], pixels: usize, swap_rb: bool) -> Result<Vec<u8>, Error> {
    let needed = pixels.checked_mul(4).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let mut rgba = data
        .get(..needed)
        .ok_or(Error::PayloadSize {
            expected: needed as u64,
            actual: data.len(),
        })?
        .to_vec();
    if swap_rb {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    Ok(rgba)
}

impl<'a> Sidecar<'a> {
    #[must_use]
    pub const fn new(part: SplitPart, bytes: &'a [u8]) -> Self {
        Self { part, bytes }
    }

    #[must_use]
    pub const fn part(self) -> SplitPart {
        self.part
    }

    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Texture {
    format: Format,
    width: u32,
    height: u32,
    depth: u32,
    pixel_height: u32,
    pixel_depth: u32,
    layer_count: u32,
    face_count: u32,
    level_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Format {
    vk: u32,
    type_size: u32,
    block_width: u32,
    block_height: u32,
    block_depth: u32,
    block_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LevelIndex {
    byte_offset: u64,
    byte_length: u64,
    uncompressed_byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Levels<'a> {
    bytes: Vec<&'a [u8]>,
}

impl Texture {
    fn from_dds(dds: &Dds) -> Result<Self, Error> {
        let width = dds.width();
        let height = dds.height().max(1);
        if width == 0 {
            return Err(Error::UnsupportedShape {
                reason: "zero texture width",
            });
        }

        let format = Format::from_dds(dds)?;
        let dx10 = dds.dx10();
        let is_1d = dx10.is_some_and(|header| {
            header.resource_dimension() == DX10_RESOURCE_DIMENSION_TEXTURE_1D
        });
        let is_3d = dds.depth() > 1
            || dds
                .header()
                .cry_flags()
                .contains(crate::CryFlags::VOLUME_TEXTURE)
            || dx10.is_some_and(|header| {
                header.resource_dimension() == DX10_RESOURCE_DIMENSION_TEXTURE_3D
            });
        let is_cube = dds.header().caps2() & DDS_CAPS2_CUBEMAP != 0
            || dds.header().cry_flags().contains(crate::CryFlags::CUBEMAP)
            || dx10.is_some_and(|header| header.misc_flag() & DX10_RESOURCE_MISC_TEXTURE_CUBE != 0);
        if is_3d && is_cube {
            return Err(Error::UnsupportedShape {
                reason: "cubemap volume texture",
            });
        }

        let array_size = dx10.map_or(1, |header| header.array_size().max(1));
        Ok(Self {
            format,
            width,
            height,
            depth: dds.depth().max(1),
            pixel_height: if is_1d { 0 } else { height },
            pixel_depth: if is_3d { dds.depth().max(1) } else { 0 },
            layer_count: if array_size > 1 { array_size } else { 0 },
            face_count: if is_cube { 6 } else { 1 },
            level_count: dds.mipmaps().max(1),
        })
    }

    fn level_size(self, level: u32) -> Result<u64, Error> {
        let width = mip_extent(self.width, level);
        let height = mip_extent(self.height, level);
        let depth = mip_extent(self.depth, level);
        let blocks_x = width.div_ceil(self.format.block_width);
        let blocks_y = height.div_ceil(self.format.block_height);
        let blocks_z = depth.div_ceil(self.format.block_depth);
        let images = u64::from(self.face_count) * u64::from(self.layer_count.max(1));

        u64::from(blocks_x)
            .checked_mul(u64::from(blocks_y))
            .and_then(|value| value.checked_mul(u64::from(blocks_z)))
            .and_then(|value| value.checked_mul(self.format.block_bytes))
            .and_then(|value| value.checked_mul(images))
            .ok_or(Error::SizeOverflow {
                what: "DDS mip level",
            })
    }

    fn level_sizes(self) -> Result<Vec<u64>, Error> {
        (0..self.level_count)
            .map(|level| self.level_size(level))
            .collect()
    }

    fn write(self, levels: &Levels<'_>) -> Result<Vec<u8>, Error> {
        let level_count = u32::try_from(levels.bytes.len()).map_err(|_| Error::SizeOverflow {
            what: "KTX2 level count",
        })?;
        let dfd = self.dfd_bytes()?;
        let dfd_offset = KTX2_HEADER_LEN
            .checked_add(KTX2_LEVEL_INDEX_LEN * u64::from(level_count))
            .ok_or(Error::SizeOverflow {
                what: "KTX2 DFD offset",
            })?;
        let dfd_len = u64::try_from(dfd.len()).map_err(|_| Error::SizeOverflow {
            what: "KTX2 DFD length",
        })?;
        let dfd_offset_u32 = u32::try_from(dfd_offset).map_err(|_| Error::SizeOverflow {
            what: "KTX2 DFD offset",
        })?;
        let dfd_len_u32 = u32::try_from(dfd_len).map_err(|_| Error::SizeOverflow {
            what: "KTX2 DFD length",
        })?;

        let mut offset = align_to(
            dfd_offset.checked_add(dfd_len).ok_or(Error::SizeOverflow {
                what: "KTX2 image offset",
            })?,
            self.alignment(),
        );
        let mut index = vec![LevelIndex::default(); levels.bytes.len()];
        for logical_level in (0..levels.bytes.len()).rev() {
            offset = align_to(offset, self.alignment());
            let bytes = levels.bytes[logical_level];
            let byte_length = u64::try_from(bytes.len()).map_err(|_| Error::SizeOverflow {
                what: "KTX2 level length",
            })?;
            index[logical_level] = LevelIndex {
                byte_offset: offset,
                byte_length,
                uncompressed_byte_length: byte_length,
            };
            offset = offset.checked_add(byte_length).ok_or(Error::SizeOverflow {
                what: "KTX2 file length",
            })?;
        }

        let total_len = usize::try_from(offset).map_err(|_| Error::SizeOverflow {
            what: "KTX2 file length",
        })?;
        let mut out = Vec::with_capacity(total_len);
        self.write_header(&mut out, level_count, dfd_offset_u32, dfd_len_u32);
        for level in &index {
            push_u64(&mut out, level.byte_offset);
            push_u64(&mut out, level.byte_length);
            push_u64(&mut out, level.uncompressed_byte_length);
        }
        out.extend_from_slice(&dfd);

        for logical_level in (0..levels.bytes.len()).rev() {
            pad_to(&mut out, index[logical_level].byte_offset)?;
            out.extend_from_slice(levels.bytes[logical_level]);
        }
        Ok(out)
    }

    fn write_header(self, out: &mut Vec<u8>, level_count: u32, dfd_offset: u32, dfd_len: u32) {
        out.extend_from_slice(KTX2_ID);
        push_u32(out, self.format.vk);
        push_u32(out, self.format.type_size);
        push_u32(out, self.width);
        push_u32(out, self.pixel_height);
        push_u32(out, self.pixel_depth);
        push_u32(out, self.layer_count);
        push_u32(out, self.face_count);
        push_u32(out, level_count);
        push_u32(out, KTX2_SUPERCOMPRESSION_NONE);
        push_u32(out, dfd_offset);
        push_u32(out, dfd_len);
        push_u32(out, 0);
        push_u32(out, 0);
        push_u64(out, 0);
        push_u64(out, 0);
    }

    fn dfd_bytes(self) -> Result<Vec<u8>, Error> {
        let words = vk2dfd::vk2dfd(self.format.vk).map_err(|_| Error::UnsupportedVulkanFormat {
            vk_format: self.format.vk,
        })?;
        let mut bytes = Vec::with_capacity(std::mem::size_of_val(words));
        for word in words {
            push_u32(&mut bytes, *word);
        }
        Ok(bytes)
    }

    fn alignment(self) -> u64 {
        u64::from(self.format.type_size).max(4)
    }
}

impl Format {
    fn from_dds(dds: &Dds) -> Result<Self, Error> {
        if let Some(header) = dds.dx10() {
            return Self::from_dxgi(header.dxgi_format(), dds.format_name());
        }

        let format = dds.pixel_format();
        if format.has_four_cc() {
            return Self::from_four_cc(format, dds.header().cry_flags());
        }
        Self::from_pixel_masks(format, dds.header().cry_flags(), dds.format_name())
    }

    fn from_dxgi(dxgi: u32, name: String) -> Result<Self, Error> {
        match dxgi {
            2 => Ok(Self::plain(VK_FORMAT_R32G32B32A32_SFLOAT, 16, 4)),
            6 => Ok(Self::plain(VK_FORMAT_R32G32B32_SFLOAT, 12, 4)),
            10 => Ok(Self::plain(VK_FORMAT_R16G16B16A16_SFLOAT, 8, 2)),
            16 => Ok(Self::plain(VK_FORMAT_R32G32_SFLOAT, 8, 4)),
            28 => Ok(Self::plain(VK_FORMAT_R8G8B8A8_UNORM, 4, 1)),
            29 => Ok(Self::plain(VK_FORMAT_R8G8B8A8_SRGB, 4, 1)),
            34 => Ok(Self::plain(VK_FORMAT_R16G16_SFLOAT, 4, 2)),
            41 => Ok(Self::plain(VK_FORMAT_R32_SFLOAT, 4, 4)),
            49 => Ok(Self::plain(VK_FORMAT_R8G8_UNORM, 2, 1)),
            54 => Ok(Self::plain(VK_FORMAT_R16_SFLOAT, 2, 2)),
            61 => Ok(Self::plain(VK_FORMAT_R8_UNORM, 1, 1)),
            70 => Ok(Self::block(VK_FORMAT_BC1_RGBA_UNORM_BLOCK, 8)),
            71 => Ok(Self::block(VK_FORMAT_BC1_RGBA_SRGB_BLOCK, 8)),
            72 => Ok(Self::block(VK_FORMAT_BC1_RGBA_SRGB_BLOCK, 8)),
            73 => Ok(Self::block(VK_FORMAT_BC2_UNORM_BLOCK, 16)),
            74 => Ok(Self::block(VK_FORMAT_BC2_SRGB_BLOCK, 16)),
            75 => Ok(Self::block(VK_FORMAT_BC2_SRGB_BLOCK, 16)),
            76 => Ok(Self::block(VK_FORMAT_BC3_UNORM_BLOCK, 16)),
            77 => Ok(Self::block(VK_FORMAT_BC3_SRGB_BLOCK, 16)),
            78 => Ok(Self::block(VK_FORMAT_BC3_SRGB_BLOCK, 16)),
            80 => Ok(Self::block(VK_FORMAT_BC4_UNORM_BLOCK, 8)),
            81 => Ok(Self::block(VK_FORMAT_BC4_SNORM_BLOCK, 8)),
            83 => Ok(Self::block(VK_FORMAT_BC5_UNORM_BLOCK, 16)),
            84 => Ok(Self::block(VK_FORMAT_BC5_SNORM_BLOCK, 16)),
            87 => Ok(Self::plain(VK_FORMAT_B8G8R8A8_UNORM, 4, 1)),
            91 => Ok(Self::plain(VK_FORMAT_B8G8R8A8_SRGB, 4, 1)),
            95 => Ok(Self::block(VK_FORMAT_BC6H_UFLOAT_BLOCK, 16)),
            96 => Ok(Self::block(VK_FORMAT_BC6H_SFLOAT_BLOCK, 16)),
            98 => Ok(Self::block(VK_FORMAT_BC7_UNORM_BLOCK, 16)),
            99 => Ok(Self::block(VK_FORMAT_BC7_SRGB_BLOCK, 16)),
            _ => Err(Error::UnsupportedFormat { format: name }),
        }
    }

    fn from_four_cc(format: PixelFormat, flags: crate::CryFlags) -> Result<Self, Error> {
        let four_cc = format.four_cc();
        let srgb = flags.contains(crate::CryFlags::SRGB_READ);
        match &four_cc {
            b"DXT1" => Ok(Self::block(
                if srgb {
                    VK_FORMAT_BC1_RGBA_SRGB_BLOCK
                } else {
                    VK_FORMAT_BC1_RGBA_UNORM_BLOCK
                },
                8,
            )),
            b"DXT3" => Ok(Self::block(
                if srgb {
                    VK_FORMAT_BC2_SRGB_BLOCK
                } else {
                    VK_FORMAT_BC2_UNORM_BLOCK
                },
                16,
            )),
            b"DXT5" => Ok(Self::block(
                if srgb {
                    VK_FORMAT_BC3_SRGB_BLOCK
                } else {
                    VK_FORMAT_BC3_UNORM_BLOCK
                },
                16,
            )),
            b"ATI1" | b"BC4U" => Ok(Self::block(VK_FORMAT_BC4_UNORM_BLOCK, 8)),
            b"BC4S" => Ok(Self::block(VK_FORMAT_BC4_SNORM_BLOCK, 8)),
            b"ATI2" | b"BC5U" => Ok(Self::block(VK_FORMAT_BC5_UNORM_BLOCK, 16)),
            b"BC5S" => Ok(Self::block(VK_FORMAT_BC5_SNORM_BLOCK, 16)),
            _ => Err(Error::UnsupportedFormat {
                format: four_cc_name(four_cc),
            }),
        }
    }

    fn from_pixel_masks(
        format: PixelFormat,
        flags: crate::CryFlags,
        name: String,
    ) -> Result<Self, Error> {
        if format.flags() & DDPF_LUMINANCE != 0 && format.rgb_bit_count() == 8 {
            return Ok(Self::plain(VK_FORMAT_R8_UNORM, 1, 1));
        }
        if format.flags() & DDPF_RGB == 0 || format.rgb_bit_count() != 32 {
            return Err(Error::UnsupportedFormat { format: name });
        }

        let has_alpha = format.flags() & DDPF_ALPHA_PIXELS != 0 || format.alpha_mask() != 0;
        if !has_alpha {
            return Err(Error::UnsupportedFormat { format: name });
        }

        let srgb = flags.contains(crate::CryFlags::SRGB_READ);
        match (
            format.red_mask(),
            format.green_mask(),
            format.blue_mask(),
            format.alpha_mask(),
        ) {
            (0x0000_00ff, 0x0000_ff00, 0x00ff_0000, 0xff00_0000) => Ok(Self::plain(
                if srgb {
                    VK_FORMAT_R8G8B8A8_SRGB
                } else {
                    VK_FORMAT_R8G8B8A8_UNORM
                },
                4,
                1,
            )),
            (0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000) => Ok(Self::plain(
                if srgb {
                    VK_FORMAT_B8G8R8A8_SRGB
                } else {
                    VK_FORMAT_B8G8R8A8_UNORM
                },
                4,
                1,
            )),
            _ => Err(Error::UnsupportedFormat { format: name }),
        }
    }

    const fn block(vk_format: u32, block_bytes: u64) -> Self {
        Self {
            vk: vk_format,
            type_size: 1,
            block_width: 4,
            block_height: 4,
            block_depth: 1,
            block_bytes,
        }
    }

    const fn plain(vk_format: u32, block_bytes: u64, type_size: u32) -> Self {
        Self {
            vk: vk_format,
            type_size,
            block_width: 1,
            block_height: 1,
            block_depth: 1,
            block_bytes,
        }
    }
}

impl<'a> Levels<'a> {
    fn from_dds(
        dds: &Dds,
        texture: Texture,
        payload: &'a [u8],
        sidecars: &[Sidecar<'a>],
    ) -> Result<Self, Error> {
        let sizes = texture.level_sizes()?;
        let bytes = if dds.is_split() {
            Self::split(dds, payload, sidecars, &sizes)?
        } else {
            if let Some(sidecar) = sidecars.first() {
                return Err(Error::UnexpectedSidecar {
                    part: sidecar.part(),
                });
            }
            slice_chain(payload, &sizes, 0)?
        };
        Ok(Self { bytes })
    }

    fn split(
        dds: &Dds,
        payload: &'a [u8],
        sidecars: &[Sidecar<'a>],
        sizes: &[u64],
    ) -> Result<Vec<&'a [u8]>, Error> {
        let persistent = usize::from(dds.header().persistent_mips());
        let mipmaps = sizes.len();
        if persistent > mipmaps {
            return Err(Error::UnsupportedShape {
                reason: "persistent mip count exceeds total mip count",
            });
        }

        let split_count = mipmaps - persistent;
        let mut split = vec![None; split_count];
        let mut alpha_group = None;
        for sidecar in sidecars {
            let SplitPart::Mip { index, alpha } = sidecar.part() else {
                return Err(Error::UnexpectedSidecar {
                    part: sidecar.part(),
                });
            };
            if alpha_group.is_some_and(|expected| expected != alpha) {
                return Err(Error::UnexpectedSidecar {
                    part: sidecar.part(),
                });
            }
            alpha_group = Some(alpha);
            let index_usize = usize::try_from(index).map_err(|_| Error::SizeOverflow {
                what: "DDS split mip index",
            })?;
            // Split sidecars are numbered 1..=split_count, smallest mip to largest,
            // so the highest index is the largest mip (level 0):
            // level = split_count - index. (Cry/Lumberyard convention; the header
            // file holds the persistent, smallest mips after the split chain.)
            if index_usize == 0 || index_usize > split_count {
                return Err(Error::UnexpectedSidecar {
                    part: sidecar.part(),
                });
            }
            let level = split_count - index_usize;
            if split[level].is_some() {
                return Err(Error::DuplicateSidecar { index });
            }
            check_mip_size(
                u32::try_from(level).unwrap_or(u32::MAX),
                sizes[level],
                sidecar.bytes().len(),
            )?;
            split[level] = Some(sidecar.bytes());
        }

        let mut levels = Vec::with_capacity(mipmaps);
        for (level, bytes) in split.into_iter().enumerate() {
            levels.push(bytes.ok_or_else(|| Error::MissingSidecar {
                // Report the missing sidecar's 1-based index, not its mip level.
                index: u32::try_from(split_count - level).unwrap_or(u32::MAX),
            })?);
        }
        levels.extend(slice_chain(payload, &sizes[split_count..], split_count)?);
        Ok(levels)
    }
}

fn slice_chain<'a>(
    payload: &'a [u8],
    sizes: &[u64],
    start_level: usize,
) -> Result<Vec<&'a [u8]>, Error> {
    let expected = checked_sum(sizes)?;
    if u64::try_from(payload.len()).map_err(|_| Error::SizeOverflow {
        what: "DDS payload length",
    })? != expected
    {
        return Err(Error::PayloadSize {
            expected,
            actual: payload.len(),
        });
    }

    let mut levels = Vec::with_capacity(sizes.len());
    let mut offset = 0usize;
    for (index, size) in sizes.iter().enumerate() {
        let expected = *size;
        let size = usize::try_from(expected).map_err(|_| Error::SizeOverflow {
            what: "DDS mip level",
        })?;
        let end = offset.checked_add(size).ok_or(Error::SizeOverflow {
            what: "DDS mip offset",
        })?;
        let level_index = u32::try_from(start_level + index).map_err(|_| Error::SizeOverflow {
            what: "DDS mip index",
        })?;
        let bytes = payload.get(offset..end).ok_or(Error::MipSize {
            level: level_index,
            expected,
            actual: payload.len().saturating_sub(offset),
        })?;
        levels.push(bytes);
        offset = end;
    }
    Ok(levels)
}

fn check_mip_size(level: u32, expected: u64, actual: usize) -> Result<(), Error> {
    let actual_u64 = u64::try_from(actual).map_err(|_| Error::SizeOverflow {
        what: "DDS mip length",
    })?;
    if actual_u64 == expected {
        Ok(())
    } else {
        Err(Error::MipSize {
            level,
            expected,
            actual,
        })
    }
}

fn checked_sum(values: &[u64]) -> Result<u64, Error> {
    values.iter().try_fold(0u64, |sum, value| {
        sum.checked_add(*value).ok_or(Error::SizeOverflow {
            what: "DDS payload length",
        })
    })
}

fn mip_extent(value: u32, level: u32) -> u32 {
    value.checked_shr(level).unwrap_or(0).max(1)
}

fn align_to(offset: u64, alignment: u64) -> u64 {
    let remainder = offset % alignment;
    if remainder == 0 {
        offset
    } else {
        offset + (alignment - remainder)
    }
}

fn pad_to(out: &mut Vec<u8>, offset: u64) -> Result<(), Error> {
    let offset = usize::try_from(offset).map_err(|_| Error::SizeOverflow {
        what: "KTX2 offset",
    })?;
    out.resize(offset, 0);
    Ok(())
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn four_cc_name(four_cc: [u8; 4]) -> String {
    if four_cc.iter().all(u8::is_ascii_graphic) {
        String::from_utf8_lossy(&four_cc).to_string()
    } else {
        format!(
            "0x{:02x}{:02x}{:02x}{:02x}",
            four_cc[0], four_cc[1], four_cc[2], four_cc[3]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DDPF_FOUR_CC, DDS_HEADER_SIZE, DDS_MAGIC, DDS_PIXEL_FORMAT_SIZE, FOUR_CC_FYRC};

    #[test]
    fn writes_valid_ktx2_for_single_bc1_dds() {
        let mut bytes = dds_header(*b"DXT1", 4, 4, 1, 0);
        bytes.extend_from_slice(&[0x55; 8]);

        let ktx = Ktx2::from_dds(&bytes, &[]).unwrap();
        let reader = ktx2::Reader::new(ktx.bytes()).unwrap();
        let levels = reader.levels().collect::<Vec<_>>();

        assert_eq!(reader.header().pixel_width, 4);
        assert_eq!(reader.header().pixel_height, 4);
        assert_eq!(reader.header().level_count, 1);
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].data.len(), 8);
    }

    #[test]
    fn writes_valid_ktx2_from_split_mips() {
        let mut header = dds_header(*b"DXT1", 8, 8, 3, 1);
        put_u32(
            &mut header,
            36,
            crate::CryFlags::SPLIT.bits() | crate::CryFlags::SRGB_READ.bits(),
        );
        header[124..128].copy_from_slice(&FOUR_CC_FYRC);
        header.extend_from_slice(&[0x33; 8]);
        // Split mips are numbered smallest→largest: `.dds.2` is the largest (level
        // 0), `.dds.1` the next. The level-2 mip is persistent (in the header).
        let mip0 = [0x11; 32];
        let mip1 = [0x22; 8];
        let sidecars = [
            Sidecar::new(
                SplitPart::Mip {
                    index: 2,
                    alpha: false,
                },
                &mip0,
            ),
            Sidecar::new(
                SplitPart::Mip {
                    index: 1,
                    alpha: false,
                },
                &mip1,
            ),
        ];

        let ktx = Ktx2::from_dds(&header, &sidecars).unwrap();
        let reader = ktx2::Reader::new(ktx.bytes()).unwrap();
        let levels = reader.levels().collect::<Vec<_>>();

        assert_eq!(reader.header().level_count, 3);
        assert_eq!(
            reader.header().format.map(|format| format.value()),
            Some(VK_FORMAT_BC1_RGBA_SRGB_BLOCK)
        );
        assert_eq!(levels[0].data.len(), 32);
        assert_eq!(levels[1].data.len(), 8);
        assert_eq!(levels[2].data.len(), 8);
    }

    #[test]
    fn split_conversion_requires_all_external_mips() {
        let mut header = dds_header(*b"DXT1", 8, 8, 3, 1);
        put_u32(&mut header, 36, crate::CryFlags::SPLIT.bits());
        header[124..128].copy_from_slice(&FOUR_CC_FYRC);
        header.extend_from_slice(&[0x33; 8]);
        // Provide only the largest split mip (`.dds.2`); the level-1 mip (`.dds.1`)
        // is missing.
        let mip0 = [0x11; 32];
        let sidecars = [Sidecar::new(
            SplitPart::Mip {
                index: 2,
                alpha: false,
            },
            &mip0,
        )];

        assert_eq!(
            Ktx2::from_dds(&header, &sidecars),
            Err(Error::MissingSidecar { index: 1 })
        );
    }

    fn dds_header(
        four_cc: [u8; 4],
        width: u32,
        height: u32,
        mipmaps: u32,
        persistent_mips: u8,
    ) -> Vec<u8> {
        let mut bytes = vec![0; DDS_FILE_HEADER_LEN];
        bytes[0..4].copy_from_slice(DDS_MAGIC);
        put_u32(&mut bytes, 4, DDS_HEADER_SIZE);
        put_u32(&mut bytes, 8, 0x1 | 0x2 | 0x4 | 0x1000);
        put_u32(&mut bytes, 12, height);
        put_u32(&mut bytes, 16, width);
        put_u32(&mut bytes, 28, mipmaps);
        put_u32(&mut bytes, 76, DDS_PIXEL_FORMAT_SIZE);
        put_u32(&mut bytes, 80, DDPF_FOUR_CC);
        bytes[84..88].copy_from_slice(&four_cc);
        put_u32(&mut bytes, 108, 0x1000);
        bytes[116] = persistent_mips;
        bytes
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
