use std::borrow::Cow;

use thiserror::Error as ThisError;

use crate::{
    DDPF_ALPHA, DDPF_ALPHA_PIXELS, DDPF_BUMP_DUDV, DDPF_LUMINANCE, DDPF_RGB, DDS_FILE_HEADER_LEN,
    Dds, DdsDimension, DdsError, DdsShape, PixelFormat, SplitPart,
};

const KTX2_ID: &[u8; 12] = b"\xABKTX 20\xBB\r\n\x1A\n";
const KTX2_HEADER_LEN: u64 = 80;
const KTX2_LEVEL_INDEX_LEN: u64 = 24;
const KTX2_SUPERCOMPRESSION_NONE: u32 = 0;

const VK_FORMAT_R8_UNORM: u32 = 9;
const VK_FORMAT_R8G8_UNORM: u32 = 16;
const VK_FORMAT_R8G8B8_UNORM: u32 = 23;
const VK_FORMAT_B8G8R8_UNORM: u32 = 30;
const VK_FORMAT_R8G8B8A8_UNORM: u32 = 37;
const VK_FORMAT_R8G8B8A8_SRGB: u32 = 43;
const VK_FORMAT_B8G8R8A8_UNORM: u32 = 44;
const VK_FORMAT_B8G8R8A8_SRGB: u32 = 50;
const VK_FORMAT_R16_UNORM: u32 = 70;
const VK_FORMAT_R16_SFLOAT: u32 = 76;
const VK_FORMAT_R16G16_UNORM: u32 = 77;
const VK_FORMAT_R16G16_SNORM: u32 = 78;
const VK_FORMAT_R16G16_SFLOAT: u32 = 83;
const VK_FORMAT_R16G16B16A16_UNORM: u32 = 91;
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
const VK_FORMAT_A8_UNORM_KHR: u32 = 1_000_470_001;

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

    #[error("DDS shape {shape:?} cannot be decoded as one 2D image")]
    MultiImageShape { shape: DdsShape },

    #[error("unsupported Vulkan format {vk_format}")]
    UnsupportedVulkanFormat { vk_format: u32 },

    #[error("DDS payload contains {actual} bytes, expected {expected}")]
    PayloadSize { expected: u64, actual: usize },

    #[error("RGBA8 pixels contain {actual} bytes, expected {expected}")]
    RgbaSize { expected: u64, actual: usize },

    #[error("RGBA16 pixels contain {actual} samples, expected {expected}")]
    Rgba16Size { expected: u64, actual: usize },

    #[error("RGBA32F pixels contain {actual} samples, expected {expected}")]
    Rgba32FloatSize { expected: u64, actual: usize },

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

    #[error(
        "attached-alpha DDS dimensions {alpha_width}x{alpha_height} do not match color {color_width}x{color_height}"
    )]
    AttachedAlphaDimensions {
        color_width: u32,
        color_height: u32,
        alpha_width: u32,
        alpha_height: u32,
    },

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

    /// Write a single-mip RGBA8 image to a KTX2 container.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if dimensions are zero, the RGBA8 byte count is not
    /// exactly `width * height * 4`, or the resulting KTX2 indexes would
    /// overflow.
    pub fn from_rgba8(width: u32, height: u32, rgba: &[u8]) -> Result<Self, Error> {
        Self::from_rgba8_with_srgb(width, height, rgba, false)
    }

    /// Write a single-mip RGBA8 image to a KTX2 container, selecting UNORM or
    /// sRGB storage from the source color space.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if dimensions are zero, the RGBA8 byte count is not
    /// exactly `width * height * 4`, or the resulting KTX2 indexes would
    /// overflow.
    pub fn from_rgba8_with_srgb(
        width: u32,
        height: u32,
        rgba: &[u8],
        srgb: bool,
    ) -> Result<Self, Error> {
        let expected = expected_rgba_elements(width, height, "RGBA8 pixels")?;
        if u64::try_from(rgba.len()).map_err(|_| Error::SizeOverflow {
            what: "RGBA8 pixels",
        })? != expected
        {
            return Err(Error::RgbaSize {
                expected,
                actual: rgba.len(),
            });
        }

        let vk = if srgb {
            VK_FORMAT_R8G8B8A8_SRGB
        } else {
            VK_FORMAT_R8G8B8A8_UNORM
        };
        let texture = single_mip_texture(Format::plain(vk, 4, 1), width, height)?;
        let levels = Levels {
            bytes: vec![Cow::Borrowed(rgba)],
        };
        let bytes = texture.write(&levels)?;
        Ok(Self { bytes })
    }

    /// Write a single-mip RGBA16 UNORM image to a KTX2 container.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if dimensions are zero, the RGBA16 sample count is not
    /// exactly `width * height * 4`, or the resulting KTX2 indexes would
    /// overflow.
    pub fn from_rgba16(width: u32, height: u32, rgba: &[u16]) -> Result<Self, Error> {
        let expected = expected_rgba_elements(width, height, "RGBA16 pixels")?;
        if u64::try_from(rgba.len()).map_err(|_| Error::SizeOverflow {
            what: "RGBA16 pixels",
        })? != expected
        {
            return Err(Error::Rgba16Size {
                expected,
                actual: rgba.len(),
            });
        }

        let mut bytes =
            Vec::with_capacity(rgba.len().checked_mul(2).ok_or(Error::SizeOverflow {
                what: "RGBA16 pixels",
            })?);
        for sample in rgba {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let texture = single_mip_texture(
            Format::plain(VK_FORMAT_R16G16B16A16_UNORM, 8, 2),
            width,
            height,
        )?;
        let levels = Levels {
            bytes: vec![Cow::Borrowed(bytes.as_slice())],
        };
        let out = texture.write(&levels)?;
        Ok(Self { bytes: out })
    }

    /// Write a single-mip RGBA32F image to a KTX2 container.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if dimensions are zero, the RGBA float sample count is
    /// not exactly `width * height * 4`, or the resulting KTX2 indexes would
    /// overflow.
    pub fn from_rgba32f(width: u32, height: u32, rgba: &[f32]) -> Result<Self, Error> {
        let expected = expected_rgba_elements(width, height, "RGBA32F pixels")?;
        if u64::try_from(rgba.len()).map_err(|_| Error::SizeOverflow {
            what: "RGBA32F pixels",
        })? != expected
        {
            return Err(Error::Rgba32FloatSize {
                expected,
                actual: rgba.len(),
            });
        }

        let mut bytes =
            Vec::with_capacity(rgba.len().checked_mul(4).ok_or(Error::SizeOverflow {
                what: "RGBA32F pixels",
            })?);
        for sample in rgba {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let texture = single_mip_texture(
            Format::plain(VK_FORMAT_R32G32B32A32_SFLOAT, 16, 4),
            width,
            height,
        )?;
        let levels = Levels {
            bytes: vec![Cow::Borrowed(bytes.as_slice())],
        };
        let out = texture.write(&levels)?;
        Ok(Self { bytes: out })
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

/// An RGBA16 UNORM image decoded from a texture (row-major, tightly packed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage16 {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u16>,
}

/// An RGBA32F image decoded from a float texture (row-major, tightly packed).
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedFloatImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<f32>,
}

/// Images decoded from the largest DDS mip in KTX2 image order.
///
/// The order is array layer, cubemap face, then volume depth slice. Plain 2D
/// textures contain exactly one image.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedImageSet<T> {
    pub shape: DdsShape,
    pub images: Vec<T>,
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
pub fn decode_top_mip<'a>(
    bytes: &'a [u8],
    sidecars: &[Sidecar<'a>],
) -> Result<DecodedImage, Error> {
    let set = decode_top_mip_images(bytes, sidecars)?;
    into_plain_2d_image(set)
}

/// Decode every image in the largest DDS mip to RGBA8.
///
/// Array layers, cubemap faces, and volume depth slices are retained in
/// [`DecodedImageSet::images`] in KTX2 image order.
pub fn decode_top_mip_images<'a>(
    bytes: &'a [u8],
    sidecars: &[Sidecar<'a>],
) -> Result<DecodedImageSet<DecodedImage>, Error> {
    let top = top_mip_level(bytes, sidecars)?;
    let images = top_mip_image_blocks(top.texture, top.shape, top.blocks.as_ref())?
        .map(|blocks| {
            let rgba = decode_texture_rgba(
                top.texture.format,
                blocks,
                top.texture.width as usize,
                top.texture.height as usize,
            )?;
            Ok(DecodedImage {
                width: top.texture.width,
                height: top.texture.height,
                rgba,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(DecodedImageSet {
        shape: top.shape,
        images,
    })
}

/// Decode the largest color mip and merge Cry's optional attached-alpha DDS
/// (`.dds.a` plus `.dds.Na`) into the returned RGBA alpha channel.
///
/// The signal-channel selection matches Lumberyard/New World's texture source
/// transform: single/dual-channel and BC4/BC5 alpha surfaces use red; ordinary
/// RGBA surfaces use alpha.
pub fn decode_top_mip_with_attached_alpha<'a>(
    bytes: &'a [u8],
    sidecars: &[Sidecar<'a>],
    attached_alpha: Option<(&'a [u8], &[Sidecar<'a>])>,
) -> Result<DecodedImage, Error> {
    let mut color = decode_top_mip(bytes, sidecars)?;
    let Some((alpha_bytes, alpha_sidecars)) = attached_alpha else {
        return Ok(color);
    };
    let alpha_dds = Dds::parse(alpha_bytes)?;
    let alpha_shape = alpha_dds.shape();
    require_plain_2d_shape(alpha_shape)?;
    let alpha_texture = Texture::from_dds(&alpha_dds)?;
    let alpha_image = decode_top_mip(alpha_bytes, alpha_sidecars)?;
    let alpha_width = alpha_image.width;
    let alpha_height = alpha_image.height;
    if color.width != alpha_width || color.height != alpha_height {
        return Err(Error::AttachedAlphaDimensions {
            color_width: color.width,
            color_height: color.height,
            alpha_width,
            alpha_height,
        });
    }
    let alpha = alpha_image.rgba;
    let signal = if matches!(
        alpha_texture.format.vk,
        VK_FORMAT_R8_UNORM
            | VK_FORMAT_R8G8_UNORM
            | VK_FORMAT_R16_UNORM
            | VK_FORMAT_R16_SFLOAT
            | VK_FORMAT_R16G16_UNORM
            | VK_FORMAT_R16G16_SFLOAT
            | VK_FORMAT_R32_SFLOAT
            | VK_FORMAT_R32G32_SFLOAT
            | VK_FORMAT_BC4_UNORM_BLOCK
            | VK_FORMAT_BC4_SNORM_BLOCK
            | VK_FORMAT_BC5_UNORM_BLOCK
            | VK_FORMAT_BC5_SNORM_BLOCK
    ) {
        0
    } else {
        3
    };
    for (pixel, alpha_pixel) in color.rgba.chunks_exact_mut(4).zip(alpha.chunks_exact(4)) {
        pixel[3] = alpha_pixel[signal];
    }
    Ok(color)
}

/// Decode the largest mip of a DDS to RGBA16 UNORM, assembling split sidecars
/// first.
///
/// Supports plain 16-bit integer DDS formats. Block-compressed and float
/// formats return [`Error::UnsupportedVulkanFormat`].
///
/// # Errors
///
/// Returns [`Error`] when the DDS is invalid, sidecars are missing/mismatched, or
/// the encoded format cannot be decoded to RGBA16.
pub fn decode_top_mip_rgba16<'a>(
    bytes: &'a [u8],
    sidecars: &[Sidecar<'a>],
) -> Result<DecodedImage16, Error> {
    let set = decode_top_mip_rgba16_images(bytes, sidecars)?;
    into_plain_2d_image(set)
}

/// Decode every image in the largest DDS mip to RGBA16 UNORM.
pub fn decode_top_mip_rgba16_images<'a>(
    bytes: &'a [u8],
    sidecars: &[Sidecar<'a>],
) -> Result<DecodedImageSet<DecodedImage16>, Error> {
    let top = top_mip_level(bytes, sidecars)?;
    let images = top_mip_image_blocks(top.texture, top.shape, top.blocks.as_ref())?
        .map(|blocks| {
            let rgba = decode_rgba16(
                top.texture.format.vk,
                blocks,
                top.texture.width as usize,
                top.texture.height as usize,
            )?;
            Ok(DecodedImage16 {
                width: top.texture.width,
                height: top.texture.height,
                rgba,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(DecodedImageSet {
        shape: top.shape,
        images,
    })
}

/// Decode the largest mip of a float DDS to RGBA32F, assembling split sidecars
/// first.
///
/// Supports plain 16-bit and 32-bit float DDS formats plus signed and unsigned
/// BC6H. BC6H is decoded directly to floating-point RGB so HDR values are not
/// quantized through the RGBA8 preview path.
///
/// # Errors
///
/// Returns [`Error`] when the DDS is invalid, sidecars are missing/mismatched, or
/// the encoded format cannot be decoded to RGBA32F.
pub fn decode_top_mip_float<'a>(
    bytes: &'a [u8],
    sidecars: &[Sidecar<'a>],
) -> Result<DecodedFloatImage, Error> {
    let set = decode_top_mip_float_images(bytes, sidecars)?;
    into_plain_2d_image(set)
}

/// Decode every image in the largest DDS mip to RGBA32F.
pub fn decode_top_mip_float_images<'a>(
    bytes: &'a [u8],
    sidecars: &[Sidecar<'a>],
) -> Result<DecodedImageSet<DecodedFloatImage>, Error> {
    let top = top_mip_level(bytes, sidecars)?;
    let images = top_mip_image_blocks(top.texture, top.shape, top.blocks.as_ref())?
        .map(|blocks| {
            let rgba = decode_float(
                top.texture.format.vk,
                blocks,
                top.texture.width as usize,
                top.texture.height as usize,
            )?;
            Ok(DecodedFloatImage {
                width: top.texture.width,
                height: top.texture.height,
                rgba,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(DecodedImageSet {
        shape: top.shape,
        images,
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
    decode_all_mips_until(bytes, sidecars, &|| true)
}

/// Decode the mip chain like [`decode_all_mips`], but check `should_continue`
/// before each level and stop early — returning the levels decoded so far —
/// when it returns `false`.
///
/// This lets a caller on a worker thread abandon a large mip chain between
/// levels (e.g. when the user navigates away) without finishing the whole
/// texture. The check happens before each level, so it can stop before doing
/// any decode work, including the first (largest) level.
///
/// # Errors
///
/// Returns [`Error`] when the DDS is invalid, sidecars are missing/mismatched, or
/// any level's format cannot be decoded.
pub fn decode_all_mips_until<'a>(
    bytes: &'a [u8],
    sidecars: &[Sidecar<'a>],
    should_continue: &dyn Fn() -> bool,
) -> Result<Vec<DecodedImage>, Error> {
    let dds = Dds::parse(bytes)?;
    require_plain_2d(&dds)?;
    let texture = Texture::from_dds(&dds)?;
    let payload = dds.payload(bytes).ok_or(Error::PayloadSize {
        expected: u64::try_from(dds.payload_bytes()).unwrap_or(u64::MAX),
        actual: bytes.len().saturating_sub(DDS_FILE_HEADER_LEN),
    })?;
    let levels = Levels::from_dds(&dds, texture, payload, sidecars)?;
    let mut images = Vec::with_capacity(levels.bytes.len());
    for (level, blocks) in levels.bytes.iter().enumerate() {
        if !should_continue() {
            break;
        }
        let level = level as u32;
        let width = mip_extent(texture.width, level).max(1);
        let height = mip_extent(texture.height, level).max(1);
        let rgba = decode_texture_rgba(
            texture.format,
            blocks.as_ref(),
            width as usize,
            height as usize,
        )?;
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
    require_plain_2d(&dds)?;
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
    let blocks = chain
        .first()
        .map(Cow::as_ref)
        .ok_or(Error::UnsupportedShape {
            reason: "header has no mip levels",
        })?;
    let level = u32::try_from(start_level).unwrap_or(0);
    let width = mip_extent(texture.width, level).max(1);
    let height = mip_extent(texture.height, level).max(1);
    let rgba = decode_texture_rgba(texture.format, blocks, width as usize, height as usize)?;
    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
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
    require_plain_2d(&dds)?;
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
    let dim_at = |level: u32| {
        mip_extent(texture.width, level)
            .max(mip_extent(texture.height, level))
            .max(1)
    };
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
        let blocks = chain
            .get(target_usize - split_count)
            .map(Cow::as_ref)
            .ok_or(Error::UnsupportedShape {
                reason: "missing header mip",
            })?;
        let rgba = decode_texture_rgba(texture.format, blocks, width as usize, height as usize)?;
        Ok(DecodedImage {
            width,
            height,
            rgba,
        })
    } else {
        // Split mip — read just this one sidecar (index = split_count - level).
        let index = u32::try_from(split_count - target_usize).unwrap_or(u32::MAX);
        let sidecar = fetch(SplitPart::Mip {
            index,
            alpha: false,
        })
        .ok_or(Error::MissingSidecar { index })?;
        check_mip_size(target, sizes[target_usize], sidecar.len())?;
        let rgba = decode_texture_rgba(texture.format, &sidecar, width as usize, height as usize)?;
        Ok(DecodedImage {
            width,
            height,
            rgba,
        })
    }
}

struct TopMip<'a> {
    texture: Texture,
    shape: DdsShape,
    blocks: Cow<'a, [u8]>,
}

fn top_mip_level<'a>(bytes: &'a [u8], sidecars: &[Sidecar<'a>]) -> Result<TopMip<'a>, Error> {
    let dds = Dds::parse(bytes)?;
    let shape = dds.shape();
    let texture = Texture::from_dds(&dds)?;
    let payload = dds.payload(bytes).ok_or(Error::PayloadSize {
        expected: u64::try_from(dds.payload_bytes()).unwrap_or(u64::MAX),
        actual: bytes.len().saturating_sub(DDS_FILE_HEADER_LEN),
    })?;
    let sizes = texture.level_sizes()?;
    let blocks = if dds.is_split() {
        let split =
            collect_split_levels(&dds, sidecars, &sizes, SplitLevelRequirement::TopContiguous)?;
        validate_payload_size(payload, &sizes[split.declared_count..])?;
        if let Some(top) = split.levels.first() {
            Cow::Borrowed(*top)
        } else {
            select_surface_major_level(payload, &sizes, 0, texture.surface_count())?
        }
    } else {
        if let Some(sidecar) = sidecars.first() {
            return Err(Error::UnexpectedSidecar {
                part: sidecar.part(),
            });
        }
        select_surface_major_level(payload, &sizes, 0, texture.surface_count())?
    };
    Ok(TopMip {
        texture,
        shape,
        blocks,
    })
}

fn top_mip_image_blocks<'a>(
    texture: Texture,
    shape: DdsShape,
    blocks: &'a [u8],
) -> Result<impl Iterator<Item = &'a [u8]>, Error> {
    let image_count = usize::try_from(shape.image_count()).map_err(|_| Error::SizeOverflow {
        what: "DDS image count",
    })?;
    let level_size = texture.level_size(0)?;
    let image_size =
        level_size
            .checked_div(shape.image_count())
            .ok_or(Error::UnsupportedShape {
                reason: "texture has no images",
            })?;
    let image_size =
        usize::try_from(image_size).map_err(|_| Error::SizeOverflow { what: "DDS image" })?;
    let expected = image_size
        .checked_mul(image_count)
        .ok_or(Error::SizeOverflow {
            what: "DDS mip level",
        })?;
    if blocks.len() != expected {
        return Err(Error::MipSize {
            level: 0,
            expected: u64::try_from(expected).unwrap_or(u64::MAX),
            actual: blocks.len(),
        });
    }
    Ok(blocks.chunks_exact(image_size))
}

fn into_plain_2d_image<T>(mut set: DecodedImageSet<T>) -> Result<T, Error> {
    require_plain_2d_shape(set.shape)?;
    set.images.pop().ok_or(Error::UnsupportedShape {
        reason: "texture has no images",
    })
}

fn require_plain_2d(dds: &Dds) -> Result<(), Error> {
    require_plain_2d_shape(dds.shape())
}

fn require_plain_2d_shape(shape: DdsShape) -> Result<(), Error> {
    if shape.is_plain_2d() {
        Ok(())
    } else {
        Err(Error::MultiImageShape { shape })
    }
}

fn decode_texture_rgba(
    format: Format,
    data: &[u8],
    width: usize,
    height: usize,
) -> Result<Vec<u8>, Error> {
    let mut rgba = decode_rgba(format.vk, data, width, height)?;
    if format.alpha_mode == AlphaMode::Opaque {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[3] = u8::MAX;
        }
    }
    Ok(rgba)
}

fn decode_rgba(vk: u32, data: &[u8], width: usize, height: usize) -> Result<Vec<u8>, Error> {
    let pixels = width.checked_mul(height).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let unsupported = || Error::UnsupportedVulkanFormat { vk_format: vk };
    match vk {
        VK_FORMAT_R8_UNORM => {
            return plain_r8(data, pixels);
        }
        VK_FORMAT_R8G8_UNORM => {
            return plain_rg8(data, pixels);
        }
        VK_FORMAT_R8G8B8_UNORM => {
            return plain_rgb8(data, pixels, false);
        }
        VK_FORMAT_B8G8R8_UNORM => {
            return plain_rgb8(data, pixels, true);
        }
        VK_FORMAT_R8G8B8A8_UNORM | VK_FORMAT_R8G8B8A8_SRGB => {
            return plain_rgba(data, pixels, false);
        }
        VK_FORMAT_B8G8R8A8_UNORM | VK_FORMAT_B8G8R8A8_SRGB => {
            return plain_rgba(data, pixels, true);
        }
        VK_FORMAT_A8_UNORM_KHR => {
            return plain_a8(data, pixels);
        }
        // BC7 decodes via bcdec_rs: it writes RGBA bytes straight into the
        // output (no 0xAARRGGBB repack pass) and benches faster than
        // texture2ddecoder, with byte-identical output (verified by test).
        VK_FORMAT_BC7_UNORM_BLOCK | VK_FORMAT_BC7_SRGB_BLOCK => {
            return decode_bcn_blocks(data, width, height, 16, bcdec_rs::bc7);
        }
        _ => {}
    }
    // BC1/BC2/BC3 (different 565-endpoint expansion + interpolation rounding and
    // 1-bit alpha semantics from bcdec_rs) and BC4/BC5/BC6H (channel mapping
    // into 0xAARRGGBB) stay on texture2ddecoder so output is preserved exactly.
    let mut out = vec![0u32; pixels];
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
        _ => return Err(unsupported()),
    };
    result.map_err(|_| unsupported())?;
    // texture2ddecoder yields 0xAARRGGBB per pixel; expand to RGBA bytes. Write into
    // a pre-sized buffer in 4-byte chunks (no per-pixel bounds checks / reallocs, and
    // vectorizable) rather than pushing one byte at a time.
    let mut rgba = vec![0u8; pixels * 4];
    for (chunk, &color) in rgba.chunks_exact_mut(4).zip(out.iter()) {
        chunk[0] = (color >> 16) as u8;
        chunk[1] = (color >> 8) as u8;
        chunk[2] = color as u8;
        chunk[3] = (color >> 24) as u8;
    }
    Ok(rgba)
}

fn decode_rgba16(vk: u32, data: &[u8], width: usize, height: usize) -> Result<Vec<u16>, Error> {
    let pixels = width.checked_mul(height).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    match vk {
        VK_FORMAT_R16_UNORM => plain_r16(data, pixels),
        VK_FORMAT_R16G16_UNORM => plain_rg16(data, pixels),
        VK_FORMAT_R16G16B16A16_UNORM => plain_rgba16(data, pixels),
        _ => Err(Error::UnsupportedVulkanFormat { vk_format: vk }),
    }
}

fn decode_float(vk: u32, data: &[u8], width: usize, height: usize) -> Result<Vec<f32>, Error> {
    let pixels = width.checked_mul(height).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    match vk {
        VK_FORMAT_R16_SFLOAT => plain_r16f(data, pixels),
        VK_FORMAT_R16G16_SNORM => plain_rg16_snorm(data, pixels),
        VK_FORMAT_R16G16_SFLOAT => plain_rg16f(data, pixels),
        VK_FORMAT_R16G16B16A16_SFLOAT => plain_rgba16f(data, pixels),
        VK_FORMAT_R32_SFLOAT => plain_r32f(data, pixels),
        VK_FORMAT_R32G32_SFLOAT => plain_rg32f(data, pixels),
        VK_FORMAT_R32G32B32_SFLOAT => plain_rgb32f(data, pixels),
        VK_FORMAT_R32G32B32A32_SFLOAT => plain_rgba32f(data, pixels),
        VK_FORMAT_BC6H_UFLOAT_BLOCK => decode_bc6h_blocks(data, width, height, false),
        VK_FORMAT_BC6H_SFLOAT_BLOCK => decode_bc6h_blocks(data, width, height, true),
        _ => Err(Error::UnsupportedVulkanFormat { vk_format: vk }),
    }
}

fn plain_rg16_snorm(data: &[u8], pixels: usize) -> Result<Vec<f32>, Error> {
    let needed = pixels.checked_mul(4).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0.0f32; pixels * 4];
    for (pixel, rg) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(4)) {
        pixel[0] = snorm16_to_f32(i16::from_le_bytes([rg[0], rg[1]]));
        pixel[1] = snorm16_to_f32(i16::from_le_bytes([rg[2], rg[3]]));
        pixel[3] = 1.0;
    }
    Ok(rgba)
}

fn snorm16_to_f32(value: i16) -> f32 {
    (f32::from(value) / f32::from(i16::MAX)).max(-1.0)
}

fn decode_bc6h_blocks(
    data: &[u8],
    width: usize,
    height: usize,
    is_signed: bool,
) -> Result<Vec<f32>, Error> {
    let row_pitch = width.checked_mul(4).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let total = row_pitch.checked_mul(height).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let expected = blocks_x
        .checked_mul(blocks_y)
        .and_then(|blocks| blocks.checked_mul(16))
        .ok_or(Error::SizeOverflow {
            what: "image dimensions",
        })?;
    if data.len() < expected {
        return Err(Error::PayloadSize {
            expected: expected as u64,
            actual: data.len(),
        });
    }

    let mut rgba = vec![0.0; total];
    let mut rgb = [0.0; 4 * 4 * 3];
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_offset = (by * blocks_x + bx) * 16;
            bcdec_rs::bc6h_float(
                &data[block_offset..block_offset + 16],
                &mut rgb,
                4 * 3,
                is_signed,
            );
            let px = bx * 4;
            let py = by * 4;
            let rows = (height - py).min(4);
            let cols = (width - px).min(4);
            for row in 0..rows {
                for col in 0..cols {
                    let source = (row * 4 + col) * 3;
                    let target = (py + row) * row_pitch + (px + col) * 4;
                    rgba[target..target + 3].copy_from_slice(&rgb[source..source + 3]);
                    rgba[target + 3] = 1.0;
                }
            }
        }
    }
    Ok(rgba)
}

/// Decode a BCn texture whose blocks expand to RGBA8 using a per-block `decode`
/// (from `bcdec_rs`), writing directly into a pre-sized RGBA output.
///
/// Interior blocks (those fully inside `width`/`height`) decode straight into
/// the output at the right pitch; edge blocks that overhang the image are
/// decoded into a 4x4 scratch and the in-bounds rows/columns copied back.
fn decode_bcn_blocks(
    data: &[u8],
    width: usize,
    height: usize,
    block_bytes: usize,
    decode: fn(&[u8], &mut [u8], usize),
) -> Result<Vec<u8>, Error> {
    let row_pitch = width.checked_mul(4).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let total = row_pitch.checked_mul(height).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let expected = blocks_x
        .checked_mul(blocks_y)
        .and_then(|blocks| blocks.checked_mul(block_bytes))
        .ok_or(Error::SizeOverflow {
            what: "image dimensions",
        })?;
    if data.len() < expected {
        return Err(Error::PayloadSize {
            expected: expected as u64,
            actual: data.len(),
        });
    }

    let mut rgba = vec![0u8; total];
    let mut scratch = [0u8; 4 * 4 * 4];
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block = &data[(by * blocks_x + bx) * block_bytes..][..block_bytes];
            let px = bx * 4;
            let py = by * 4;
            if px + 4 <= width && py + 4 <= height {
                // Interior block: write the 4x4 tile straight into the image.
                let offset = py * row_pitch + px * 4;
                decode(block, &mut rgba[offset..], row_pitch);
            } else {
                // Edge block: decode into scratch, then copy the in-bounds region.
                decode(block, &mut scratch, 4 * 4);
                let rows = (height - py).min(4);
                let cols = (width - px).min(4);
                for row in 0..rows {
                    let src = &scratch[row * 16..row * 16 + cols * 4];
                    let dst_offset = (py + row) * row_pitch + px * 4;
                    rgba[dst_offset..dst_offset + cols * 4].copy_from_slice(src);
                }
            }
        }
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

fn plain_r8(data: &[u8], pixels: usize) -> Result<Vec<u8>, Error> {
    let bytes = data.get(..pixels).ok_or(Error::PayloadSize {
        expected: pixels as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0u8; pixels * 4];
    for (pixel, &r) in rgba.chunks_exact_mut(4).zip(bytes.iter()) {
        pixel[0] = r;
        pixel[3] = 255;
    }
    Ok(rgba)
}

fn plain_rg8(data: &[u8], pixels: usize) -> Result<Vec<u8>, Error> {
    let needed = pixels.checked_mul(2).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0u8; pixels * 4];
    for (pixel, rg) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(2)) {
        pixel[0] = rg[0];
        pixel[1] = rg[1];
        pixel[3] = 255;
    }
    Ok(rgba)
}

fn plain_rgb8(data: &[u8], pixels: usize, swap_rb: bool) -> Result<Vec<u8>, Error> {
    let needed = pixels.checked_mul(3).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0u8; pixels * 4];
    for (pixel, rgb) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(3)) {
        pixel[..3].copy_from_slice(rgb);
        if swap_rb {
            pixel.swap(0, 2);
        }
        pixel[3] = u8::MAX;
    }
    Ok(rgba)
}

fn plain_a8(data: &[u8], pixels: usize) -> Result<Vec<u8>, Error> {
    let bytes = data.get(..pixels).ok_or(Error::PayloadSize {
        expected: pixels as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![u8::MAX; pixels * 4];
    for (pixel, &alpha) in rgba.chunks_exact_mut(4).zip(bytes) {
        pixel[3] = alpha;
    }
    Ok(rgba)
}

fn plain_r16(data: &[u8], pixels: usize) -> Result<Vec<u16>, Error> {
    let needed = pixels.checked_mul(2).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0u16; pixels * 4];
    for (pixel, r) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(2)) {
        pixel[0] = u16::from_le_bytes([r[0], r[1]]);
        pixel[3] = u16::MAX;
    }
    Ok(rgba)
}

fn plain_rg16(data: &[u8], pixels: usize) -> Result<Vec<u16>, Error> {
    let needed = pixels.checked_mul(4).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0u16; pixels * 4];
    for (pixel, rg) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(4)) {
        pixel[0] = u16::from_le_bytes([rg[0], rg[1]]);
        pixel[1] = u16::from_le_bytes([rg[2], rg[3]]);
        pixel[3] = u16::MAX;
    }
    Ok(rgba)
}

fn plain_rgba16(data: &[u8], pixels: usize) -> Result<Vec<u16>, Error> {
    let needed = pixels.checked_mul(8).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = Vec::with_capacity(pixels * 4);
    for sample in bytes.chunks_exact(2) {
        rgba.push(u16::from_le_bytes([sample[0], sample[1]]));
    }
    Ok(rgba)
}

fn plain_r16f(data: &[u8], pixels: usize) -> Result<Vec<f32>, Error> {
    let needed = pixels.checked_mul(2).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0.0f32; pixels * 4];
    for (pixel, r) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(2)) {
        pixel[0] = half_to_f32(u16::from_le_bytes([r[0], r[1]]));
        pixel[3] = 1.0;
    }
    Ok(rgba)
}

fn plain_rg16f(data: &[u8], pixels: usize) -> Result<Vec<f32>, Error> {
    let needed = pixels.checked_mul(4).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0.0f32; pixels * 4];
    for (pixel, rg) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(4)) {
        pixel[0] = half_to_f32(u16::from_le_bytes([rg[0], rg[1]]));
        pixel[1] = half_to_f32(u16::from_le_bytes([rg[2], rg[3]]));
        pixel[3] = 1.0;
    }
    Ok(rgba)
}

fn plain_rgba16f(data: &[u8], pixels: usize) -> Result<Vec<f32>, Error> {
    let needed = pixels.checked_mul(8).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = Vec::with_capacity(pixels * 4);
    for sample in bytes.chunks_exact(2) {
        rgba.push(half_to_f32(u16::from_le_bytes([sample[0], sample[1]])));
    }
    Ok(rgba)
}

fn plain_r32f(data: &[u8], pixels: usize) -> Result<Vec<f32>, Error> {
    let needed = pixels.checked_mul(4).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0.0f32; pixels * 4];
    for (pixel, r) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(4)) {
        pixel[0] = f32::from_le_bytes([r[0], r[1], r[2], r[3]]);
        pixel[3] = 1.0;
    }
    Ok(rgba)
}

fn plain_rg32f(data: &[u8], pixels: usize) -> Result<Vec<f32>, Error> {
    let needed = pixels.checked_mul(8).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0.0f32; pixels * 4];
    for (pixel, rg) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(8)) {
        pixel[0] = f32::from_le_bytes([rg[0], rg[1], rg[2], rg[3]]);
        pixel[1] = f32::from_le_bytes([rg[4], rg[5], rg[6], rg[7]]);
        pixel[3] = 1.0;
    }
    Ok(rgba)
}

fn plain_rgb32f(data: &[u8], pixels: usize) -> Result<Vec<f32>, Error> {
    let needed = pixels.checked_mul(12).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = vec![0.0f32; pixels * 4];
    for (pixel, rgb) in rgba.chunks_exact_mut(4).zip(bytes.chunks_exact(12)) {
        pixel[0] = f32::from_le_bytes([rgb[0], rgb[1], rgb[2], rgb[3]]);
        pixel[1] = f32::from_le_bytes([rgb[4], rgb[5], rgb[6], rgb[7]]);
        pixel[2] = f32::from_le_bytes([rgb[8], rgb[9], rgb[10], rgb[11]]);
        pixel[3] = 1.0;
    }
    Ok(rgba)
}

fn plain_rgba32f(data: &[u8], pixels: usize) -> Result<Vec<f32>, Error> {
    let needed = pixels.checked_mul(16).ok_or(Error::SizeOverflow {
        what: "image dimensions",
    })?;
    let bytes = data.get(..needed).ok_or(Error::PayloadSize {
        expected: needed as u64,
        actual: data.len(),
    })?;
    let mut rgba = Vec::with_capacity(pixels * 4);
    for sample in bytes.chunks_exact(4) {
        rgba.push(f32::from_le_bytes([
            sample[0], sample[1], sample[2], sample[3],
        ]));
    }
    Ok(rgba)
}

fn half_to_f32(bits: u16) -> f32 {
    let sign = (u32::from(bits & 0x8000)) << 16;
    let exp = (bits & 0x7c00) >> 10;
    let mant = u32::from(bits & 0x03ff);
    let f_bits = match exp {
        0 if mant == 0 => sign,
        0 => {
            let mut mantissa = mant;
            let mut exponent = -14i32;
            while mantissa & 0x0400 == 0 {
                mantissa <<= 1;
                exponent -= 1;
            }
            mantissa &= 0x03ff;
            let exp32 = u32::try_from(exponent + 127).unwrap_or(0);
            sign | (exp32 << 23) | (mantissa << 13)
        }
        0x1f => sign | 0x7f80_0000 | (mant << 13),
        _ => {
            let exp32 = u32::from(exp) + (127 - 15);
            sign | (exp32 << 23) | (mant << 13)
        }
    };
    f32::from_bits(f_bits)
}

fn expected_rgba_elements(width: u32, height: u32, what: &'static str) -> Result<u64, Error> {
    if width == 0 {
        return Err(Error::UnsupportedShape {
            reason: "zero texture width",
        });
    }
    if height == 0 {
        return Err(Error::UnsupportedShape {
            reason: "zero texture height",
        });
    }
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(Error::SizeOverflow { what })
}

fn single_mip_texture(format: Format, width: u32, height: u32) -> Result<Texture, Error> {
    if width == 0 {
        return Err(Error::UnsupportedShape {
            reason: "zero texture width",
        });
    }
    if height == 0 {
        return Err(Error::UnsupportedShape {
            reason: "zero texture height",
        });
    }
    Ok(Texture {
        format,
        width,
        height,
        depth: 1,
        pixel_height: height,
        pixel_depth: 0,
        layer_count: 0,
        face_count: 1,
        level_count: 1,
    })
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
    alpha_mode: AlphaMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlphaMode {
    Stored,
    Opaque,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LevelIndex {
    byte_offset: u64,
    byte_length: u64,
    uncompressed_byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Levels<'a> {
    bytes: Vec<Cow<'a, [u8]>>,
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
        let shape = dds.shape();
        if dds.is_cry_extended()
            && shape.faces == 6
            && dds
                .dx10()
                .is_some_and(|header| header.array_size() == 0 || header.array_size() % 6 != 0)
        {
            return Err(Error::UnsupportedShape {
                reason: "Lumberyard cubemap face count is not divisible by 6",
            });
        }
        if shape.dimension == DdsDimension::Three && shape.faces > 1 {
            return Err(Error::UnsupportedShape {
                reason: "cubemap volume texture",
            });
        }
        if shape.dimension == DdsDimension::Three && shape.array_layers > 1 {
            return Err(Error::UnsupportedShape {
                reason: "3D texture arrays are not valid DDS resources",
            });
        }

        Ok(Self {
            format,
            width,
            height,
            depth: shape.depth,
            pixel_height: if shape.dimension == DdsDimension::One {
                0
            } else {
                height
            },
            pixel_depth: if shape.dimension == DdsDimension::Three {
                shape.depth
            } else {
                0
            },
            layer_count: if shape.array_layers > 1 {
                shape.array_layers
            } else {
                0
            },
            face_count: shape.faces,
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
        let images = self.surface_count();

        u64::from(blocks_x)
            .checked_mul(u64::from(blocks_y))
            .and_then(|value| value.checked_mul(u64::from(blocks_z)))
            .and_then(|value| value.checked_mul(self.format.block_bytes))
            .and_then(|value| value.checked_mul(images))
            .ok_or(Error::SizeOverflow {
                what: "DDS mip level",
            })
    }

    fn surface_count(self) -> u64 {
        u64::from(self.face_count) * u64::from(self.layer_count.max(1))
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
            let bytes = levels.bytes[logical_level].as_ref();
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
            let level = levels.bytes[logical_level].as_ref();
            if self.format.alpha_mode == AlphaMode::Opaque {
                for pixel in level.chunks_exact(4) {
                    out.extend_from_slice(&pixel[..3]);
                    out.push(u8::MAX);
                }
            } else {
                out.extend_from_slice(level);
            }
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
            11 => Ok(Self::plain(VK_FORMAT_R16G16B16A16_UNORM, 8, 2)),
            16 => Ok(Self::plain(VK_FORMAT_R32G32_SFLOAT, 8, 4)),
            28 => Ok(Self::plain(VK_FORMAT_R8G8B8A8_UNORM, 4, 1)),
            29 => Ok(Self::plain(VK_FORMAT_R8G8B8A8_SRGB, 4, 1)),
            34 => Ok(Self::plain(VK_FORMAT_R16G16_SFLOAT, 4, 2)),
            35 => Ok(Self::plain(VK_FORMAT_R16G16_UNORM, 4, 2)),
            41 => Ok(Self::plain(VK_FORMAT_R32_SFLOAT, 4, 4)),
            49 => Ok(Self::plain(VK_FORMAT_R8G8_UNORM, 2, 1)),
            54 => Ok(Self::plain(VK_FORMAT_R16_SFLOAT, 2, 2)),
            56 => Ok(Self::plain(VK_FORMAT_R16_UNORM, 2, 2)),
            61 => Ok(Self::plain(VK_FORMAT_R8_UNORM, 1, 1)),
            65 => Ok(Self::plain(VK_FORMAT_A8_UNORM_KHR, 1, 1)),
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
            // D3D9 wrote numeric D3DFORMAT values into the FourCC field.
            // 112 is D3DFMT_G16R16F, whose byte layout is R16G16 float.
            [112, 0, 0, 0] => Ok(Self::plain(VK_FORMAT_R16G16_SFLOAT, 4, 2)),
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
        if format.flags() & DDPF_BUMP_DUDV != 0
            && format.rgb_bit_count() == 32
            && format.red_mask() == 0x0000_ffff
            && format.green_mask() == 0xffff_0000
            && format.blue_mask() == 0
            && format.alpha_mask() == 0
        {
            return Ok(Self::plain(VK_FORMAT_R16G16_SNORM, 4, 2));
        }
        if (format.flags() & DDPF_LUMINANCE != 0 || format.flags() & DDPF_ALPHA != 0)
            && format.rgb_bit_count() == 8
        {
            return Ok(Self::plain(VK_FORMAT_R8_UNORM, 1, 1));
        }
        if format.flags() & DDPF_RGB == 0 {
            return Err(Error::UnsupportedFormat { format: name });
        }

        let has_alpha = format.flags() & DDPF_ALPHA_PIXELS != 0 || format.alpha_mask() != 0;
        let srgb = flags.contains(crate::CryFlags::SRGB_READ);
        match (
            format.rgb_bit_count(),
            format.red_mask(),
            format.green_mask(),
            format.blue_mask(),
            format.alpha_mask(),
        ) {
            (24, 0x0000_00ff, 0x0000_ff00, 0x00ff_0000, 0) => {
                Ok(Self::plain(VK_FORMAT_R8G8B8_UNORM, 3, 1))
            }
            (24, 0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0) => {
                Ok(Self::plain(VK_FORMAT_B8G8R8_UNORM, 3, 1))
            }
            (32, 0x0000_00ff, 0x0000_ff00, 0x00ff_0000, 0) if !has_alpha => {
                Ok(Self::plain_opaque(VK_FORMAT_R8G8B8A8_UNORM, 4, 1))
            }
            (32, 0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0) if !has_alpha => {
                Ok(Self::plain_opaque(VK_FORMAT_B8G8R8A8_UNORM, 4, 1))
            }
            (32, 0x0000_00ff, 0x0000_ff00, 0x00ff_0000, 0xff00_0000) => Ok(Self::plain(
                if srgb {
                    VK_FORMAT_R8G8B8A8_SRGB
                } else {
                    VK_FORMAT_R8G8B8A8_UNORM
                },
                4,
                1,
            )),
            (32, 0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000) => Ok(Self::plain(
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
            alpha_mode: AlphaMode::Stored,
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
            alpha_mode: AlphaMode::Stored,
        }
    }

    const fn plain_opaque(vk_format: u32, block_bytes: u64, type_size: u32) -> Self {
        Self {
            alpha_mode: AlphaMode::Opaque,
            ..Self::plain(vk_format, block_bytes, type_size)
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
            Self::split(dds, texture, payload, sidecars, &sizes)?
        } else {
            if let Some(sidecar) = sidecars.first() {
                return Err(Error::UnexpectedSidecar {
                    part: sidecar.part(),
                });
            }
            slice_surface_major(payload, &sizes, 0, texture.surface_count())?
        };
        Ok(Self { bytes })
    }

    fn split(
        dds: &Dds,
        texture: Texture,
        payload: &'a [u8],
        sidecars: &[Sidecar<'a>],
        sizes: &[u64],
    ) -> Result<Vec<Cow<'a, [u8]>>, Error> {
        let split = collect_split_levels(dds, sidecars, sizes, SplitLevelRequirement::Complete)?;
        let mut levels = Vec::with_capacity(sizes.len());
        levels.extend(split.levels.into_iter().map(Cow::Borrowed));
        levels.extend(slice_surface_major(
            payload,
            &sizes[split.declared_count..],
            split.declared_count,
            texture.surface_count(),
        )?);
        Ok(levels)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitLevelRequirement {
    /// A full mip-chain container needs every level declared outside the header.
    Complete,
    /// An authoring transform needs the contiguous high-resolution prefix.
    ///
    /// Some shipped attached-alpha chains omit intermediate levels between that
    /// prefix and the persistent tail. Their largest image is still complete and
    /// the authoring builder regenerates its mip chain from that image.
    TopContiguous,
}

struct SplitLevels<'a> {
    declared_count: usize,
    levels: Vec<&'a [u8]>,
}

fn collect_split_levels<'a>(
    dds: &Dds,
    sidecars: &[Sidecar<'a>],
    sizes: &[u64],
    requirement: SplitLevelRequirement,
) -> Result<SplitLevels<'a>, Error> {
    let persistent = usize::from(dds.header().persistent_mips());
    let mipmaps = sizes.len();
    if persistent > mipmaps {
        return Err(Error::UnsupportedShape {
            reason: "persistent mip count exceeds total mip count",
        });
    }

    let declared_count = mipmaps - persistent;
    let available_count = match requirement {
        SplitLevelRequirement::Complete => declared_count,
        SplitLevelRequirement::TopContiguous => {
            let mut highest = 0usize;
            for sidecar in sidecars {
                let SplitPart::Mip { index, .. } = sidecar.part() else {
                    return Err(Error::UnexpectedSidecar {
                        part: sidecar.part(),
                    });
                };
                highest = highest.max(usize::try_from(index).map_err(|_| Error::SizeOverflow {
                    what: "DDS split mip index",
                })?);
            }
            if highest == 0 && declared_count != 0 {
                return Err(Error::MissingSidecar {
                    index: u32::try_from(declared_count).unwrap_or(u32::MAX),
                });
            }
            highest
        }
    };
    if available_count > declared_count {
        let part = sidecars
            .iter()
            .max_by_key(|sidecar| match sidecar.part() {
                SplitPart::Mip { index, .. } => index,
                SplitPart::Header | SplitPart::AlphaHeader => 0,
            })
            .map_or(SplitPart::Header, |sidecar| sidecar.part());
        return Err(Error::UnexpectedSidecar { part });
    }

    let mut split = vec![None; available_count];
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
        // Split sidecars are numbered 1..=available_count, smallest available
        // mip to largest, so the highest index is the largest mip (level 0).
        if index_usize == 0 || index_usize > available_count {
            return Err(Error::UnexpectedSidecar {
                part: sidecar.part(),
            });
        }
        let level = available_count - index_usize;
        if split[level].is_some() {
            return Err(Error::DuplicateSidecar { index });
        }
        let expected = sizes[level];
        check_mip_size(
            u32::try_from(level).unwrap_or(u32::MAX),
            expected,
            sidecar.bytes().len(),
        )?;
        let expected_len = usize::try_from(expected).map_err(|_| Error::SizeOverflow {
            what: "DDS mip level",
        })?;
        split[level] = Some(sidecar.bytes().get(..expected_len).ok_or(Error::MipSize {
            level: u32::try_from(level).unwrap_or(u32::MAX),
            expected,
            actual: sidecar.bytes().len(),
        })?);
    }

    let levels = split
        .into_iter()
        .enumerate()
        .map(|(level, bytes)| {
            bytes.ok_or_else(|| Error::MissingSidecar {
                index: u32::try_from(available_count - level).unwrap_or(u32::MAX),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SplitLevels {
        declared_count,
        levels,
    })
}

fn slice_chain<'a>(
    payload: &'a [u8],
    sizes: &[u64],
    start_level: usize,
) -> Result<Vec<Cow<'a, [u8]>>, Error> {
    let expected = checked_sum(sizes)?;
    let actual_len = u64::try_from(payload.len()).map_err(|_| Error::SizeOverflow {
        what: "DDS payload length",
    })?;
    if actual_len < expected {
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
        levels.push(Cow::Borrowed(bytes));
        offset = end;
    }
    Ok(levels)
}

fn validate_payload_size(payload: &[u8], sizes: &[u64]) -> Result<(), Error> {
    let expected = checked_sum(sizes)?;
    let actual_len = u64::try_from(payload.len()).map_err(|_| Error::SizeOverflow {
        what: "DDS payload length",
    })?;
    if actual_len < expected {
        return Err(Error::PayloadSize {
            expected,
            actual: payload.len(),
        });
    }
    Ok(())
}

fn select_surface_major_level<'a>(
    payload: &'a [u8],
    sizes: &[u64],
    level: usize,
    surface_count: u64,
) -> Result<Cow<'a, [u8]>, Error> {
    validate_payload_size(payload, sizes)?;
    let level_size = *sizes.get(level).ok_or(Error::UnsupportedShape {
        reason: "texture has no requested mip level",
    })?;
    if surface_count == 1 {
        let offset =
            usize::try_from(checked_sum(&sizes[..level])?).map_err(|_| Error::SizeOverflow {
                what: "DDS mip offset",
            })?;
        let len = usize::try_from(level_size).map_err(|_| Error::SizeOverflow {
            what: "DDS mip level",
        })?;
        let end = offset.checked_add(len).ok_or(Error::SizeOverflow {
            what: "DDS mip offset",
        })?;
        return Ok(Cow::Borrowed(payload.get(offset..end).ok_or(
            Error::MipSize {
                level: u32::try_from(level).unwrap_or(u32::MAX),
                expected: level_size,
                actual: payload.len().saturating_sub(offset),
            },
        )?));
    }

    let surface_count_usize = usize::try_from(surface_count).map_err(|_| Error::SizeOverflow {
        what: "DDS surface count",
    })?;
    let mut per_surface_sizes = Vec::with_capacity(sizes.len());
    for size in sizes {
        if size % surface_count != 0 {
            return Err(Error::UnsupportedShape {
                reason: "DDS mip size is not divisible by its surface count",
            });
        }
        per_surface_sizes.push(usize::try_from(size / surface_count).map_err(|_| {
            Error::SizeOverflow {
                what: "DDS surface mip",
            }
        })?);
    }
    let surface_pitch = per_surface_sizes.iter().try_fold(0usize, |sum, size| {
        sum.checked_add(*size).ok_or(Error::SizeOverflow {
            what: "DDS surface chain",
        })
    })?;
    let level_offset = per_surface_sizes[..level]
        .iter()
        .try_fold(0usize, |sum, size| {
            sum.checked_add(*size).ok_or(Error::SizeOverflow {
                what: "DDS surface mip offset",
            })
        })?;
    let per_surface_level = per_surface_sizes[level];
    let mut selected =
        Vec::with_capacity(
            usize::try_from(level_size).map_err(|_| Error::SizeOverflow {
                what: "DDS mip level",
            })?,
        );
    for surface in 0..surface_count_usize {
        let start = surface
            .checked_mul(surface_pitch)
            .and_then(|offset| offset.checked_add(level_offset))
            .ok_or(Error::SizeOverflow {
                what: "DDS surface mip offset",
            })?;
        let end = start
            .checked_add(per_surface_level)
            .ok_or(Error::SizeOverflow {
                what: "DDS surface mip offset",
            })?;
        selected.extend_from_slice(payload.get(start..end).ok_or(Error::MipSize {
            level: u32::try_from(level).unwrap_or(u32::MAX),
            expected: u64::try_from(per_surface_level).unwrap_or(u64::MAX),
            actual: payload.len().saturating_sub(start),
        })?);
    }
    Ok(Cow::Owned(selected))
}

/// Convert the DDS surface-major layout into level-major image data.
///
/// Standard DDS stores every mip for one array element or cubemap face before
/// the next surface. KTX2 stores one whole mip level with all layers and faces
/// together. Volume textures have one surface and keep their depth slices
/// inside each mip, so they take the zero-copy path.
fn slice_surface_major<'a>(
    payload: &'a [u8],
    sizes: &[u64],
    start_level: usize,
    surface_count: u64,
) -> Result<Vec<Cow<'a, [u8]>>, Error> {
    if surface_count == 1 {
        return slice_chain(payload, sizes, start_level);
    }
    validate_payload_size(payload, sizes)?;
    let surface_count_usize = usize::try_from(surface_count).map_err(|_| Error::SizeOverflow {
        what: "DDS surface count",
    })?;
    let mut per_surface_sizes = Vec::with_capacity(sizes.len());
    let mut levels = Vec::with_capacity(sizes.len());
    for size in sizes {
        if size % surface_count != 0 {
            return Err(Error::UnsupportedShape {
                reason: "DDS mip size is not divisible by its surface count",
            });
        }
        let per_surface =
            usize::try_from(size / surface_count).map_err(|_| Error::SizeOverflow {
                what: "DDS surface mip",
            })?;
        per_surface_sizes.push(per_surface);
        levels.push(Vec::with_capacity(usize::try_from(*size).map_err(
            |_| Error::SizeOverflow {
                what: "DDS mip level",
            },
        )?));
    }

    let mut offset = 0usize;
    for _surface in 0..surface_count_usize {
        for (index, per_surface) in per_surface_sizes.iter().copied().enumerate() {
            let end = offset.checked_add(per_surface).ok_or(Error::SizeOverflow {
                what: "DDS surface mip offset",
            })?;
            let level = u32::try_from(start_level + index).map_err(|_| Error::SizeOverflow {
                what: "DDS mip index",
            })?;
            let bytes = payload.get(offset..end).ok_or(Error::MipSize {
                level,
                expected: u64::try_from(per_surface).unwrap_or(u64::MAX),
                actual: payload.len().saturating_sub(offset),
            })?;
            levels[index].extend_from_slice(bytes);
            offset = end;
        }
    }
    Ok(levels.into_iter().map(Cow::Owned).collect())
}

fn check_mip_size(level: u32, expected: u64, actual: usize) -> Result<(), Error> {
    let actual_u64 = u64::try_from(actual).map_err(|_| Error::SizeOverflow {
        what: "DDS mip length",
    })?;
    if actual_u64 >= expected {
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
    use crate::{
        DDPF_ALPHA, DDPF_FOUR_CC, DDS_HEADER_SIZE, DDS_MAGIC, DDS_PIXEL_FORMAT_SIZE, FOUR_CC_FYRC,
    };

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
    fn writes_valid_ktx2_from_rgba8() {
        let rgba = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];

        let ktx = Ktx2::from_rgba8(2, 2, &rgba).unwrap();
        let reader = ktx2::Reader::new(ktx.bytes()).unwrap();
        let levels = reader.levels().collect::<Vec<_>>();

        assert_eq!(reader.header().pixel_width, 2);
        assert_eq!(reader.header().pixel_height, 2);
        assert_eq!(
            reader.header().format.map(|format| format.value()),
            Some(VK_FORMAT_R8G8B8A8_UNORM)
        );
        assert_eq!(reader.header().level_count, 1);
        assert_eq!(levels[0].data, rgba);
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
    fn dds_payload_chain_ignores_trailing_bytes() {
        let mut bytes = dds_header(*b"DXT1", 4, 4, 1, 0);
        bytes.extend_from_slice(&[0x55; 8]);
        bytes.extend_from_slice(&[0xaa; 8]);

        let ktx = Ktx2::from_dds(&bytes, &[]).unwrap();
        let reader = ktx2::Reader::new(ktx.bytes()).unwrap();
        let levels = reader.levels().collect::<Vec<_>>();

        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].data, &[0x55; 8]);
    }

    #[test]
    fn split_sidecar_chain_ignores_trailing_bytes() {
        let mut header = dds_header(*b"DXT1", 8, 8, 3, 1);
        put_u32(&mut header, 36, crate::CryFlags::SPLIT.bits());
        header[124..128].copy_from_slice(&FOUR_CC_FYRC);
        header.extend_from_slice(&[0x33; 8]);
        let mip0 = [0x11; 40];
        let mip1 = [0x22; 16];
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

        assert_eq!(levels[0].data, &[0x11; 32]);
        assert_eq!(levels[1].data, &[0x22; 8]);
        assert_eq!(levels[2].data, &[0x33; 8]);
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

    #[test]
    fn top_mip_decode_accepts_a_contiguous_prefix_before_a_persistent_gap() {
        let mut header = dds_alpha8_header(16, 16);
        put_u32(&mut header, 28, 5);
        put_u32(&mut header, 36, crate::CryFlags::SPLIT.bits());
        header[116] = 1;
        header[124..128].copy_from_slice(&FOUR_CC_FYRC);
        header.extend_from_slice(&[0x11]);

        // The complete external chain would be `.dds.1a` through `.dds.4a`.
        // This source retains the two largest images as a contiguous prefix and
        // the 1x1 persistent tail; its 4x4 and 2x2 bridge levels are absent.
        let mip0 = [0x77; 16 * 16];
        let mip1 = [0x55; 8 * 8];
        let sidecars = [
            Sidecar::new(
                SplitPart::Mip {
                    index: 2,
                    alpha: true,
                },
                &mip0,
            ),
            Sidecar::new(
                SplitPart::Mip {
                    index: 1,
                    alpha: true,
                },
                &mip1,
            ),
        ];

        let decoded = decode_top_mip_images(&header, &sidecars).unwrap();
        assert_eq!((decoded.shape.width, decoded.shape.height), (16, 16));
        assert_eq!(decoded.images.len(), 1);
        assert!(
            decoded.images[0]
                .rgba
                .chunks_exact(4)
                .all(|pixel| pixel == [0x77, 0, 0, 0xff])
        );

        // Lossless full-chain conversion remains strict: it cannot invent the
        // absent encoded mip levels.
        assert_eq!(
            Ktx2::from_dds(&header, &sidecars),
            Err(Error::MissingSidecar { index: 4 })
        );
    }

    #[test]
    fn top_mip_decode_rejects_a_hole_inside_the_available_prefix() {
        let mut header = dds_alpha8_header(16, 16);
        put_u32(&mut header, 28, 5);
        put_u32(&mut header, 36, crate::CryFlags::SPLIT.bits());
        header[116] = 1;
        header[124..128].copy_from_slice(&FOUR_CC_FYRC);
        header.extend_from_slice(&[0x11]);
        let mip0 = [0x77; 16 * 16];
        let sidecars = [Sidecar::new(
            SplitPart::Mip {
                index: 2,
                alpha: true,
            },
            &mip0,
        )];

        assert_eq!(
            decode_top_mip_images(&header, &sidecars),
            Err(Error::MissingSidecar { index: 1 })
        );
    }

    #[test]
    fn decodes_alpha_only_pixel_mask_as_r8_plane() {
        let mut bytes = dds_alpha8_header(2, 2);
        bytes.extend_from_slice(&[0, 64, 128, 255]);

        let decoded = decode_top_mip(&bytes, &[]).unwrap();

        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
        assert_eq!(
            decoded.rgba,
            vec![0, 0, 0, 255, 64, 0, 0, 255, 128, 0, 0, 255, 255, 0, 0, 255,]
        );
    }

    #[test]
    fn merges_attached_alpha_red_plane_into_color() {
        let mut color = dds_header(*b"DXT1", 2, 2, 1, 0);
        // One BC1 block encoding opaque white.
        color.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]);
        let mut alpha = dds_alpha8_header(2, 2);
        alpha.extend_from_slice(&[0, 64, 128, 255]);

        let decoded = decode_top_mip_with_attached_alpha(&color, &[], Some((&alpha, &[]))).unwrap();
        assert_eq!(
            decoded
                .rgba
                .chunks_exact(4)
                .map(|pixel| pixel[3])
                .collect::<Vec<_>>(),
            vec![0, 64, 128, 255]
        );
    }

    #[test]
    fn decodes_bc6h_directly_to_float_rgba() {
        let mut bytes = dds_dx10_header(95, 4, 4);
        bytes.extend_from_slice(&[0; 16]);

        let decoded = decode_top_mip_float(&bytes, &[]).unwrap();

        assert_eq!(decoded.width, 4);
        assert_eq!(decoded.height, 4);
        assert_eq!(decoded.rgba.len(), 4 * 4 * 4);
        assert!(
            decoded
                .rgba
                .chunks_exact(4)
                .all(|pixel| pixel[0..3] == [0.0, 0.0, 0.0] && pixel[3] == 1.0)
        );
    }

    #[test]
    fn decodes_dxgi_a8_as_alpha_not_luminance() {
        let mut bytes = dds_dx10_header(65, 2, 1);
        bytes.extend_from_slice(&[0, 128]);

        let decoded = decode_top_mip(&bytes, &[]).unwrap();

        assert_eq!(decoded.rgba, vec![255, 255, 255, 0, 255, 255, 255, 128]);
    }

    #[test]
    fn decodes_numeric_d3dfmt_g16r16f_to_float_rgba() {
        let mut bytes = dds_header([112, 0, 0, 0], 1, 1, 1, 0);
        bytes.extend_from_slice(&0x3c00u16.to_le_bytes());
        bytes.extend_from_slice(&0xc000u16.to_le_bytes());

        let decoded = decode_top_mip_float(&bytes, &[]).unwrap();

        assert_eq!(decoded.rgba, vec![1.0, -2.0, 0.0, 1.0]);
    }

    #[test]
    fn decodes_standard_bump_dudv_as_signed_normalized_rg() {
        let mut bytes =
            dds_pixel_header(DDPF_BUMP_DUDV, 32, [0x0000_ffff, 0xffff_0000, 0, 0], 2, 1);
        bytes.extend_from_slice(&i16::MIN.to_le_bytes());
        bytes.extend_from_slice(&0i16.to_le_bytes());
        bytes.extend_from_slice(&i16::MAX.to_le_bytes());
        bytes.extend_from_slice(&i16::MIN.to_le_bytes());

        let decoded = decode_top_mip_float(&bytes, &[]).unwrap();

        assert_eq!(decoded.rgba, vec![-1.0, 0.0, 0.0, 1.0, 1.0, -1.0, 0.0, 1.0]);
    }

    #[test]
    fn decodes_standard_bgr24_and_bgrx32_layouts() {
        let mut bgr24 = dds_pixel_header(
            DDPF_RGB,
            24,
            [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0],
            1,
            1,
        );
        bgr24.extend_from_slice(&[3, 2, 1]);
        assert_eq!(
            decode_top_mip(&bgr24, &[]).unwrap().rgba,
            vec![1, 2, 3, 255]
        );

        let mut bgrx32 = dds_pixel_header(
            DDPF_RGB,
            32,
            [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0],
            1,
            1,
        );
        bgrx32.extend_from_slice(&[3, 2, 1, 0]);
        assert_eq!(
            decode_top_mip(&bgrx32, &[]).unwrap().rgba,
            vec![1, 2, 3, 255]
        );

        let ktx = Ktx2::from_dds(&bgrx32, &[]).unwrap();
        let reader = ktx2::Reader::new(ktx.bytes()).unwrap();
        assert_eq!(reader.levels().next().unwrap().data, &[3, 2, 1, 255]);
    }

    #[test]
    fn preserves_cubemap_array_shape_in_ktx2_and_rejects_2d_decode() {
        let mut bytes = dds_dx10_header(28, 4, 4);
        put_u32(&mut bytes, DDS_FILE_HEADER_LEN + 8, 0x4);
        put_u32(&mut bytes, DDS_FILE_HEADER_LEN + 12, 2);
        bytes.extend_from_slice(&vec![0; 4 * 4 * 4 * 6 * 2]);

        let dds = Dds::parse(&bytes).unwrap();
        assert_eq!(
            dds.shape(),
            DdsShape {
                dimension: DdsDimension::Two,
                width: 4,
                height: 4,
                depth: 1,
                array_layers: 2,
                faces: 6,
            }
        );
        assert!(matches!(
            decode_top_mip(&bytes, &[]),
            Err(Error::MultiImageShape { .. })
        ));
        let decoded = decode_top_mip_images(&bytes, &[]).unwrap();
        assert_eq!(decoded.shape, dds.shape());
        assert_eq!(decoded.images.len(), 12);
        assert!(
            decoded
                .images
                .iter()
                .all(|image| (image.width, image.height, image.rgba.len()) == (4, 4, 64))
        );

        let ktx = Ktx2::from_dds(&bytes, &[]).unwrap();
        let reader = ktx2::Reader::new(ktx.bytes()).unwrap();
        assert_eq!(reader.header().layer_count, 2);
        assert_eq!(reader.header().face_count, 6);
    }

    #[test]
    fn interprets_lumberyard_cube_array_size_as_total_faces() {
        let mut bytes = dds_dx10_header(95, 4, 4);
        bytes[124..128].copy_from_slice(b"FYRC");
        put_u32(&mut bytes, DDS_FILE_HEADER_LEN + 8, 0x4);
        put_u32(&mut bytes, DDS_FILE_HEADER_LEN + 12, 6);
        bytes.extend_from_slice(&vec![0; 16 * 6]);

        let dds = Dds::parse(&bytes).unwrap();
        assert_eq!(dds.shape().array_layers, 1);
        assert_eq!(dds.shape().faces, 6);

        let ktx = Ktx2::from_dds(&bytes, &[]).unwrap();
        let reader = ktx2::Reader::new(ktx.bytes()).unwrap();
        assert_eq!(reader.header().layer_count, 0);
        assert_eq!(reader.header().face_count, 6);
    }

    #[test]
    fn transposes_surface_major_dds_mips_into_level_major_images() {
        let mut bytes = dds_dx10_header(28, 4, 4);
        put_u32(&mut bytes, 28, 2);
        put_u32(&mut bytes, DDS_FILE_HEADER_LEN + 8, 0x4);
        for face in 0..6u8 {
            bytes.extend_from_slice(&[face * 10, 0, 0, 255].repeat(4 * 4));
            bytes.extend_from_slice(&[face * 10 + 1, 0, 0, 255].repeat(2 * 2));
        }

        let decoded = decode_top_mip_images(&bytes, &[]).unwrap();
        assert_eq!(decoded.images.len(), 6);
        for (face, image) in decoded.images.iter().enumerate() {
            assert!(
                image
                    .rgba
                    .chunks_exact(4)
                    .all(|pixel| pixel[0] == face as u8 * 10)
            );
        }

        let ktx = Ktx2::from_dds(&bytes, &[]).unwrap();
        let reader = ktx2::Reader::new(ktx.bytes()).unwrap();
        let levels = reader.levels().collect::<Vec<_>>();
        assert_eq!(levels.len(), 2);
        for (face, image) in levels[0].data.chunks_exact(4 * 4 * 4).enumerate() {
            assert!(
                image
                    .chunks_exact(4)
                    .all(|pixel| pixel[0] == face as u8 * 10)
            );
        }
        for (face, image) in levels[1].data.chunks_exact(2 * 2 * 4).enumerate() {
            assert!(
                image
                    .chunks_exact(4)
                    .all(|pixel| pixel[0] == face as u8 * 10 + 1)
            );
        }
    }

    #[test]
    fn transposes_persistent_cubemap_mips_after_split_levels() {
        let mut header = dds_dx10_header(28, 4, 4);
        put_u32(&mut header, 28, 3);
        put_u32(&mut header, 36, crate::CryFlags::SPLIT.bits());
        header[116] = 2;
        put_u32(&mut header, DDS_FILE_HEADER_LEN + 8, 0x4);
        for face in 0..6u8 {
            header.extend_from_slice(&[face * 10 + 1, 0, 0, 255].repeat(2 * 2));
            header.extend_from_slice(&[face * 10 + 2, 0, 0, 255]);
        }
        let mut top = Vec::new();
        for face in 0..6u8 {
            top.extend_from_slice(&[face * 10, 0, 0, 255].repeat(4 * 4));
        }
        let sidecars = [Sidecar::new(
            SplitPart::Mip {
                index: 1,
                alpha: false,
            },
            &top,
        )];

        let ktx = Ktx2::from_dds(&header, &sidecars).unwrap();
        let reader = ktx2::Reader::new(ktx.bytes()).unwrap();
        let levels = reader.levels().collect::<Vec<_>>();
        assert_eq!(levels.len(), 3);
        for (level, extent) in [(0usize, 4usize), (1, 2), (2, 1)] {
            for (face, image) in levels[level]
                .data
                .chunks_exact(extent * extent * 4)
                .enumerate()
            {
                assert!(
                    image
                        .chunks_exact(4)
                        .all(|pixel| pixel[0] == face as u8 * 10 + level as u8)
                );
            }
        }
    }

    #[test]
    fn bc6h_float_decode_crops_partial_edge_blocks() {
        let rgba = decode_bc6h_blocks(&[0; 16], 3, 2, false).unwrap();

        assert_eq!(rgba.len(), 3 * 2 * 4);
        assert!(
            rgba.chunks_exact(4)
                .all(|pixel| pixel[0..3] == [0.0, 0.0, 0.0] && pixel[3] == 1.0)
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

    fn dds_dx10_header(dxgi_format: u32, width: u32, height: u32) -> Vec<u8> {
        let mut bytes = dds_header(*b"DX10", width, height, 1, 0);
        bytes.resize(DDS_FILE_HEADER_LEN + 20, 0);
        put_u32(&mut bytes, DDS_FILE_HEADER_LEN, dxgi_format);
        put_u32(&mut bytes, DDS_FILE_HEADER_LEN + 4, 3);
        put_u32(&mut bytes, DDS_FILE_HEADER_LEN + 12, 1);
        bytes
    }

    fn dds_alpha8_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0; DDS_FILE_HEADER_LEN];
        bytes[0..4].copy_from_slice(DDS_MAGIC);
        put_u32(&mut bytes, 4, DDS_HEADER_SIZE);
        put_u32(&mut bytes, 8, 0x1 | 0x2 | 0x4 | 0x1000);
        put_u32(&mut bytes, 12, height);
        put_u32(&mut bytes, 16, width);
        put_u32(&mut bytes, 28, 1);
        put_u32(&mut bytes, 76, DDS_PIXEL_FORMAT_SIZE);
        put_u32(&mut bytes, 80, DDPF_ALPHA);
        put_u32(&mut bytes, 88, 8);
        put_u32(&mut bytes, 108, 0x1000);
        bytes
    }

    fn dds_pixel_header(
        flags: u32,
        rgb_bit_count: u32,
        masks: [u32; 4],
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        let mut bytes = vec![0; DDS_FILE_HEADER_LEN];
        bytes[0..4].copy_from_slice(DDS_MAGIC);
        put_u32(&mut bytes, 4, DDS_HEADER_SIZE);
        put_u32(&mut bytes, 8, 0x1 | 0x2 | 0x4 | 0x1000);
        put_u32(&mut bytes, 12, height);
        put_u32(&mut bytes, 16, width);
        put_u32(&mut bytes, 28, 1);
        put_u32(&mut bytes, 76, DDS_PIXEL_FORMAT_SIZE);
        put_u32(&mut bytes, 80, flags);
        put_u32(&mut bytes, 88, rgb_bit_count);
        for (index, mask) in masks.into_iter().enumerate() {
            put_u32(&mut bytes, 92 + index * 4, mask);
        }
        put_u32(&mut bytes, 108, 0x1000);
        bytes
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    type Texture2dDecode = fn(&[u8], usize, usize, &mut [u32]) -> Result<(), &'static str>;

    /// Reference decode via texture2ddecoder + 0xAARRGGBB repack, matching the
    /// pre-bcdec_rs behaviour, so we can assert the new path is equivalent.
    fn reference_rgba(
        decode: Texture2dDecode,
        data: &[u8],
        width: usize,
        height: usize,
    ) -> Vec<u8> {
        let mut out = vec![0u32; width * height];
        decode(data, width, height, &mut out).unwrap();
        let mut rgba = vec![0u8; width * height * 4];
        for (chunk, &color) in rgba.chunks_exact_mut(4).zip(out.iter()) {
            chunk[0] = (color >> 16) as u8;
            chunk[1] = (color >> 8) as u8;
            chunk[2] = color as u8;
            chunk[3] = (color >> 24) as u8;
        }
        rgba
    }

    fn pseudo_random(len: usize) -> Vec<u8> {
        let mut state: u64 = 0x1234_5678_9abc_def0;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 24) as u8
            })
            .collect()
    }

    fn assert_close(a: &[u8], b: &[u8], format: &str) {
        assert_eq!(a.len(), b.len(), "{format}: length mismatch");
        for (i, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
            let diff = i32::from(x) - i32::from(y);
            assert!(
                diff.abs() <= 1,
                "{format}: byte {i} differs by {diff} ({x} vs {y})"
            );
        }
    }

    /// BC7 is the one format we moved onto bcdec_rs; assert its output stays
    /// equivalent to texture2ddecoder for both interior- and edge-block sizes.
    #[test]
    fn bc7_decode_matches_texture2ddecoder() {
        // 16x16 (all interior blocks) and 12x20 (non-4-aligned edge blocks).
        for (width, height) in [(16usize, 16usize), (12, 20)] {
            let blocks = width.div_ceil(4) * height.div_ceil(4);
            let data = pseudo_random(blocks * 16);
            assert_close(
                &decode_rgba(VK_FORMAT_BC7_UNORM_BLOCK, &data, width, height).unwrap(),
                &reference_rgba(texture2ddecoder::decode_bc7, &data, width, height),
                "bc7",
            );
        }
    }

    #[test]
    fn decode_all_mips_until_stops_early() {
        // 8x8 BC1 with 3 mips, no split: levels 8x8(32B), 4x4(8B), 2x2(8B).
        let mut header = dds_header(*b"DXT1", 8, 8, 3, 0);
        let payload = pseudo_random(32 + 8 + 8);
        header.extend_from_slice(&payload);

        let all = decode_all_mips(&header, &[]).unwrap();
        assert_eq!(all.len(), 3);

        // Stop after the first level: continue once, then refuse.
        let count = std::cell::Cell::new(0u32);
        let one = decode_all_mips_until(&header, &[], &|| {
            let n = count.get();
            count.set(n + 1);
            n < 1
        })
        .unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].width, 8);
        assert_eq!(one[0].height, 8);

        // Refuse immediately: no levels decoded.
        let none = decode_all_mips_until(&header, &[], &|| false).unwrap();
        assert!(none.is_empty());
    }
}
