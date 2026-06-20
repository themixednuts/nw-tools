//! DDS texture header parser with Cry texture metadata and split sidecar
//! classification.

mod container;

use std::fmt;
use std::ops::Range;
use std::path::Path;

use thiserror::Error;

pub use container::{Error as Ktx2Error, Ktx2, Sidecar};

pub const DDS_EXTENSION: &str = "dds";
pub const DDS_MAGIC: &[u8; 4] = b"DDS ";

pub const DDS_FILE_HEADER_LEN: usize = 128;
const DDS_HEADER_SIZE: u32 = 124;
const DDS_PIXEL_FORMAT_SIZE: u32 = 32;
const DX10_HEADER_LEN: usize = 20;
const FOUR_CC_DX10: [u8; 4] = *b"DX10";
const FOUR_CC_FYRC: [u8; 4] = *b"FYRC";

const DDPF_ALPHA_PIXELS: u32 = 0x1;
const DDPF_ALPHA: u32 = 0x2;
const DDPF_FOUR_CC: u32 = 0x4;
const DDPF_RGB: u32 = 0x40;
const DDPF_LUMINANCE: u32 = 0x2_0000;
const DDPF_BUMP_DUDV: u32 = 0x8_0000;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DdsError {
    #[error("DDS input too small: {len} bytes")]
    InputTooSmall { len: usize },

    #[error("invalid DDS magic {actual:02x?}")]
    InvalidMagic { actual: [u8; 4] },

    #[error("invalid DDS header size {actual}, expected 124")]
    InvalidHeaderSize { actual: u32 },

    #[error("invalid DDS pixel-format size {actual}, expected 32")]
    InvalidPixelFormatSize { actual: u32 },

    #[error("DDS DX10 header is missing")]
    MissingDx10Header,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dds {
    header: Header,
    pixel_format: PixelFormat,
    dx10: Option<Dx10Header>,
    payload_offset: usize,
    payload_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    flags: u32,
    height: u32,
    width: u32,
    pitch_or_linear_size: u32,
    depth: u32,
    mipmaps: u32,
    alpha_bit_depth: u32,
    cry_flags: CryFlags,
    average_brightness: u32,
    min_color: [u32; 4],
    max_color: [u32; 4],
    caps: u32,
    caps2: u32,
    caps3: u32,
    caps4: u32,
    persistent_mips: u8,
    tile_mode: u8,
    reserved2: [u8; 6],
    cry_marker: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelFormat {
    flags: u32,
    four_cc: [u8; 4],
    rgb_bit_count: u32,
    red_mask: u32,
    green_mask: u32,
    blue_mask: u32,
    alpha_mask: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx10Header {
    dxgi_format: u32,
    resource_dimension: u32,
    misc_flag: u32,
    array_size: u32,
    misc_flags2: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CryFlags(u32);

impl Dds {
    /// Parse a DDS file header.
    ///
    /// # Errors
    ///
    /// Returns [`DdsError`] if the buffer is too small, has the wrong magic,
    /// declares invalid fixed header sizes, or advertises a DX10 header that is
    /// not present.
    pub fn parse(bytes: &[u8]) -> Result<Self, DdsError> {
        if bytes.len() < DDS_FILE_HEADER_LEN {
            return Err(DdsError::InputTooSmall { len: bytes.len() });
        }

        let actual = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if &actual != DDS_MAGIC {
            return Err(DdsError::InvalidMagic { actual });
        }

        let header_size = le_u32(bytes, 4);
        if header_size != DDS_HEADER_SIZE {
            return Err(DdsError::InvalidHeaderSize {
                actual: header_size,
            });
        }

        let pixel_format_size = le_u32(bytes, 76);
        if pixel_format_size != DDS_PIXEL_FORMAT_SIZE {
            return Err(DdsError::InvalidPixelFormatSize {
                actual: pixel_format_size,
            });
        }

        let pixel_format = PixelFormat {
            flags: le_u32(bytes, 80),
            four_cc: [bytes[84], bytes[85], bytes[86], bytes[87]],
            rgb_bit_count: le_u32(bytes, 88),
            red_mask: le_u32(bytes, 92),
            green_mask: le_u32(bytes, 96),
            blue_mask: le_u32(bytes, 100),
            alpha_mask: le_u32(bytes, 104),
        };

        let mut payload_offset = DDS_FILE_HEADER_LEN;
        let dx10 = if pixel_format.four_cc == FOUR_CC_DX10 {
            let dx10 = bytes
                .get(DDS_FILE_HEADER_LEN..DDS_FILE_HEADER_LEN + DX10_HEADER_LEN)
                .ok_or(DdsError::MissingDx10Header)?;
            payload_offset += DX10_HEADER_LEN;
            Some(Dx10Header {
                dxgi_format: le_u32(dx10, 0),
                resource_dimension: le_u32(dx10, 4),
                misc_flag: le_u32(dx10, 8),
                array_size: le_u32(dx10, 12),
                misc_flags2: le_u32(dx10, 16),
            })
        } else {
            None
        };

        Ok(Self {
            header: Header {
                flags: le_u32(bytes, 8),
                height: le_u32(bytes, 12),
                width: le_u32(bytes, 16),
                pitch_or_linear_size: le_u32(bytes, 20),
                depth: le_u32(bytes, 24),
                mipmaps: le_u32(bytes, 28).max(1),
                alpha_bit_depth: le_u32(bytes, 32),
                cry_flags: CryFlags(le_u32(bytes, 36)),
                average_brightness: le_u32(bytes, 40),
                min_color: [
                    le_u32(bytes, 44),
                    le_u32(bytes, 48),
                    le_u32(bytes, 52),
                    le_u32(bytes, 56),
                ],
                max_color: [
                    le_u32(bytes, 60),
                    le_u32(bytes, 64),
                    le_u32(bytes, 68),
                    le_u32(bytes, 72),
                ],
                caps: le_u32(bytes, 108),
                caps2: le_u32(bytes, 112),
                caps3: le_u32(bytes, 116),
                caps4: le_u32(bytes, 120),
                persistent_mips: bytes[116],
                tile_mode: bytes[117],
                reserved2: [
                    bytes[118], bytes[119], bytes[120], bytes[121], bytes[122], bytes[123],
                ],
                cry_marker: [bytes[124], bytes[125], bytes[126], bytes[127]],
            },
            pixel_format,
            dx10,
            payload_offset,
            payload_bytes: bytes.len().saturating_sub(payload_offset),
        })
    }

    #[must_use]
    pub const fn header(&self) -> Header {
        self.header
    }

    #[must_use]
    pub const fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    #[must_use]
    pub const fn dx10(&self) -> Option<Dx10Header> {
        self.dx10
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.header.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.header.height
    }

    #[must_use]
    pub const fn depth(&self) -> u32 {
        self.header.depth
    }

    #[must_use]
    pub const fn mipmaps(&self) -> u32 {
        self.header.mipmaps
    }

    #[must_use]
    pub fn format_name(&self) -> String {
        self.pixel_format.format_name(self.dx10)
    }

    #[must_use]
    pub const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    #[must_use]
    pub const fn payload_offset(&self) -> usize {
        self.payload_offset
    }

    #[must_use]
    pub fn payload_range(&self) -> Range<usize> {
        self.payload_offset..self.payload_offset + self.payload_bytes
    }

    #[must_use]
    pub fn payload<'a>(&self, bytes: &'a [u8]) -> Option<&'a [u8]> {
        bytes.get(self.payload_range())
    }

    #[must_use]
    pub const fn is_cry_extended(&self) -> bool {
        let marker = self.header.cry_marker;
        marker[0] == FOUR_CC_FYRC[0]
            && marker[1] == FOUR_CC_FYRC[1]
            && marker[2] == FOUR_CC_FYRC[2]
            && marker[3] == FOUR_CC_FYRC[3]
    }

    #[must_use]
    pub const fn is_split(&self) -> bool {
        self.header.cry_flags.contains(CryFlags::SPLIT)
    }

    #[must_use]
    pub const fn has_attached_alpha(&self) -> bool {
        self.header.cry_flags.contains(CryFlags::ATTACHED_ALPHA)
    }
}

impl Header {
    #[must_use]
    pub const fn flags(self) -> u32 {
        self.flags
    }

    #[must_use]
    pub const fn pitch_or_linear_size(self) -> u32 {
        self.pitch_or_linear_size
    }

    #[must_use]
    pub const fn alpha_bit_depth(self) -> u32 {
        self.alpha_bit_depth
    }

    #[must_use]
    pub const fn cry_flags(self) -> CryFlags {
        self.cry_flags
    }

    #[must_use]
    pub const fn average_brightness(self) -> u32 {
        self.average_brightness
    }

    #[must_use]
    pub fn average_brightness_f32(self) -> f32 {
        f32::from_bits(self.average_brightness)
    }

    #[must_use]
    pub const fn min_color(self) -> [u32; 4] {
        self.min_color
    }

    #[must_use]
    pub fn min_color_f32(self) -> [f32; 4] {
        self.min_color.map(f32::from_bits)
    }

    #[must_use]
    pub const fn max_color(self) -> [u32; 4] {
        self.max_color
    }

    #[must_use]
    pub fn max_color_f32(self) -> [f32; 4] {
        self.max_color.map(f32::from_bits)
    }

    #[must_use]
    pub const fn caps(self) -> u32 {
        self.caps
    }

    #[must_use]
    pub const fn caps2(self) -> u32 {
        self.caps2
    }

    #[must_use]
    pub const fn caps3(self) -> u32 {
        self.caps3
    }

    #[must_use]
    pub const fn caps4(self) -> u32 {
        self.caps4
    }

    #[must_use]
    pub const fn persistent_mips(self) -> u8 {
        self.persistent_mips
    }

    #[must_use]
    pub const fn tile_mode(self) -> u8 {
        self.tile_mode
    }

    #[must_use]
    pub const fn reserved2(self) -> [u8; 6] {
        self.reserved2
    }

    #[must_use]
    pub const fn cry_marker(self) -> [u8; 4] {
        self.cry_marker
    }
}

impl PixelFormat {
    #[must_use]
    pub const fn flags(self) -> u32 {
        self.flags
    }

    #[must_use]
    pub const fn four_cc(self) -> [u8; 4] {
        self.four_cc
    }

    #[must_use]
    pub const fn rgb_bit_count(self) -> u32 {
        self.rgb_bit_count
    }

    #[must_use]
    pub const fn red_mask(self) -> u32 {
        self.red_mask
    }

    #[must_use]
    pub const fn green_mask(self) -> u32 {
        self.green_mask
    }

    #[must_use]
    pub const fn blue_mask(self) -> u32 {
        self.blue_mask
    }

    #[must_use]
    pub const fn alpha_mask(self) -> u32 {
        self.alpha_mask
    }

    #[must_use]
    pub const fn has_four_cc(self) -> bool {
        self.flags & DDPF_FOUR_CC != 0
    }

    #[must_use]
    pub fn format_name(self, dx10: Option<Dx10Header>) -> String {
        if self.four_cc == FOUR_CC_DX10 {
            return dx10.map_or_else(
                || "DX10".to_string(),
                |header| format!("DX10:{}", dxgi_format_name(header.dxgi_format())),
            );
        }

        if self.has_four_cc() {
            return four_cc_name(self.four_cc);
        }

        if self.flags & DDPF_RGB != 0 {
            let prefix = if self.flags & DDPF_ALPHA_PIXELS != 0 || self.alpha_mask != 0 {
                "rgba"
            } else {
                "rgb"
            };
            return format!("{prefix}{}", self.rgb_bit_count);
        }

        if self.flags & DDPF_ALPHA != 0 {
            return format!("alpha{}", self.rgb_bit_count);
        }

        if self.flags & DDPF_LUMINANCE != 0 {
            return format!("luminance{}", self.rgb_bit_count);
        }

        if self.flags & DDPF_BUMP_DUDV != 0 {
            return format!("bump{}", self.rgb_bit_count);
        }

        "unknown".to_string()
    }
}

impl Dx10Header {
    #[must_use]
    pub const fn dxgi_format(self) -> u32 {
        self.dxgi_format
    }

    #[must_use]
    pub const fn resource_dimension(self) -> u32 {
        self.resource_dimension
    }

    #[must_use]
    pub const fn misc_flag(self) -> u32 {
        self.misc_flag
    }

    #[must_use]
    pub const fn array_size(self) -> u32 {
        self.array_size
    }

    #[must_use]
    pub const fn misc_flags2(self) -> u32 {
        self.misc_flags2
    }
}

impl CryFlags {
    pub const CUBEMAP: Self = Self(0x1);
    pub const VOLUME_TEXTURE: Self = Self(0x2);
    pub const DECAL: Self = Self(0x4);
    pub const GREYSCALE: Self = Self(0x8);
    pub const SUPPRESS_ENGINE_REDUCE: Self = Self(0x10);
    pub const ATTACHED_ALPHA: Self = Self(0x400);
    pub const SRGB_READ: Self = Self(0x800);
    pub const DONT_RESIZE: Self = Self(0x8000);
    pub const RENORMALIZED_TEXTURE: Self = Self(0x1_0000);
    pub const TILED: Self = Self(0x8_0000);
    pub const SPLIT: Self = Self(0x20_0000);
    pub const COLOR_MODEL_MASK: Self = Self(0x700_0000);

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }
}

impl fmt::Display for Dds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{}x{} mipmaps={} format={}",
            self.width(),
            self.height(),
            self.depth().max(1),
            self.mipmaps(),
            self.format_name()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset<'a> {
    kind: AssetKind<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind<'a> {
    Header(Dds),
    Split(SplitPayload<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitPayload<'a> {
    part: SplitPart,
    bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplitPart {
    Header,
    AlphaHeader,
    Mip { index: u32, alpha: bool },
}

impl<'a> Asset<'a> {
    /// Parse a DDS asset using its path to classify split mip sidecars.
    ///
    /// # Errors
    ///
    /// Returns [`DdsError`] when the path identifies a DDS header and the
    /// bytes are not a valid DDS header.
    pub fn parse(path: &str, bytes: &'a [u8]) -> Result<Self, DdsError> {
        match SplitPart::from_path(path) {
            Some(SplitPart::Header | SplitPart::AlphaHeader) | None => {
                Dds::parse(bytes).map(|dds| Self {
                    kind: AssetKind::Header(dds),
                })
            }
            Some(part @ SplitPart::Mip { .. }) => Ok(Self {
                kind: AssetKind::Split(SplitPayload { part, bytes }),
            }),
        }
    }

    #[must_use]
    pub const fn kind(self) -> AssetKind<'a> {
        self.kind
    }
}

impl<'a> SplitPayload<'a> {
    #[must_use]
    pub const fn part(self) -> SplitPart {
        self.part
    }

    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

impl SplitPart {
    #[must_use]
    pub fn from_path(path: &str) -> Option<Self> {
        let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let (stem, suffix) = file_name.rsplit_once('.')?;
        if suffix.eq_ignore_ascii_case(DDS_EXTENSION) {
            return Some(Self::Header);
        }
        let (_, dds_ext) = stem.rsplit_once('.')?;
        if !dds_ext.eq_ignore_ascii_case(DDS_EXTENSION) {
            return None;
        }
        if suffix.eq_ignore_ascii_case("a") {
            return Some(Self::AlphaHeader);
        }

        let (digits, alpha) = suffix
            .strip_suffix('a')
            .or_else(|| suffix.strip_suffix('A'))
            .map_or((suffix, false), |digits| (digits, true));
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let index = digits.parse().ok()?;
        Some(Self::Mip { index, alpha })
    }

    #[must_use]
    pub const fn is_alpha(self) -> bool {
        matches!(self, Self::AlphaHeader | Self::Mip { alpha: true, .. })
    }

    #[must_use]
    pub const fn mip_index(self) -> Option<u32> {
        match self {
            Self::Mip { index, .. } => Some(index),
            Self::Header | Self::AlphaHeader => None,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Header => "DDS header",
            Self::AlphaHeader => "DDS alpha header",
            Self::Mip { .. } => "DDS split mip",
        }
    }
}

impl fmt::Display for SplitPart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[must_use]
pub fn is_dds_extension(extension: &str) -> bool {
    extension.eq_ignore_ascii_case(DDS_EXTENSION)
}

#[must_use]
pub fn is_dds_name(name: &str) -> bool {
    SplitPart::from_path(name).is_some()
}

#[must_use]
pub fn is_dds_path(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_dds_name)
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

fn dxgi_format_name(format: u32) -> String {
    match format {
        28 => "R8G8B8A8_UNORM".to_string(),
        29 => "R8G8B8A8_UNORM_SRGB".to_string(),
        70 => "BC1_UNORM".to_string(),
        71 => "BC1_UNORM_SRGB".to_string(),
        73 => "BC2_UNORM".to_string(),
        74 => "BC2_UNORM_SRGB".to_string(),
        76 => "BC3_UNORM".to_string(),
        77 => "BC3_UNORM_SRGB".to_string(),
        80 => "BC4_UNORM".to_string(),
        83 => "BC5_UNORM".to_string(),
        87 => "B8G8R8A8_UNORM".to_string(),
        90 => "B8G8R8X8_UNORM".to_string(),
        95 => "BC6H_UF16".to_string(),
        96 => "BC6H_SF16".to_string(),
        98 => "BC7_UNORM".to_string(),
        99 => "BC7_UNORM_SRGB".to_string(),
        other => other.to_string(),
    }
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("DDS fixed header offsets are range checked"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_four_cc_dds_header() {
        let bytes = dds_header(*b"DXT5", 0);
        let dds = Dds::parse(&bytes).unwrap();

        assert_eq!(dds.width(), 128);
        assert_eq!(dds.height(), 64);
        assert_eq!(dds.mipmaps(), 7);
        assert_eq!(dds.format_name(), "DXT5");
    }

    #[test]
    fn parses_dx10_header() {
        let mut bytes = dds_header(*b"DX10", 0);
        bytes.extend_from_slice(&98u32.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());

        let dds = Dds::parse(&bytes).unwrap();

        assert_eq!(dds.dx10().unwrap().dxgi_format(), 98);
        assert_eq!(dds.format_name(), "DX10:BC7_UNORM");
    }

    #[test]
    fn classifies_split_parts() {
        assert_eq!(
            SplitPart::from_path("textures/foo.dds"),
            Some(SplitPart::Header)
        );
        assert_eq!(
            SplitPart::from_path("textures/foo.dds.a"),
            Some(SplitPart::AlphaHeader)
        );
        assert_eq!(
            SplitPart::from_path("textures/foo.dds.12"),
            Some(SplitPart::Mip {
                index: 12,
                alpha: false,
            })
        );
        assert_eq!(
            SplitPart::from_path("textures/foo.dds.12a"),
            Some(SplitPart::Mip {
                index: 12,
                alpha: true,
            })
        );
        assert!(is_dds_name("textures/foo.dds.1a"));
        assert!(is_dds_path("textures/foo.dds.1a"));
        assert!(!is_dds_name("textures/foo.png"));
    }

    #[test]
    fn parses_cry_extension_fields() {
        let mut bytes = dds_header(FOUR_CC_DX10, 0);
        put_u32(
            &mut bytes,
            36,
            CryFlags::ATTACHED_ALPHA.bits() | CryFlags::SPLIT.bits(),
        );
        bytes[116] = 3;
        bytes[124..128].copy_from_slice(&FOUR_CC_FYRC);
        bytes.extend_from_slice(&77u32.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 4]);

        let dds = Dds::parse(&bytes).unwrap();

        assert!(dds.is_cry_extended());
        assert!(dds.is_split());
        assert!(dds.has_attached_alpha());
        assert_eq!(dds.header().persistent_mips(), 3);
        assert_eq!(dds.header().cry_marker(), FOUR_CC_FYRC);
        assert_eq!(dds.payload_bytes(), 4);
    }

    #[test]
    fn parses_split_payload_by_path() {
        let bytes = [0xde, 0xad, 0xbe, 0xef];
        let asset = Asset::parse("textures/foo.dds.1a", &bytes).unwrap();

        assert_eq!(
            asset.kind(),
            AssetKind::Split(SplitPayload {
                part: SplitPart::Mip {
                    index: 1,
                    alpha: true,
                },
                bytes: &bytes,
            })
        );
    }

    fn dds_header(four_cc: [u8; 4], rgb_bit_count: u32) -> Vec<u8> {
        let mut bytes = vec![0; DDS_FILE_HEADER_LEN];
        bytes[0..4].copy_from_slice(DDS_MAGIC);
        put_u32(&mut bytes, 4, DDS_HEADER_SIZE);
        put_u32(&mut bytes, 8, 0x1 | 0x2 | 0x4 | 0x1000);
        put_u32(&mut bytes, 12, 64);
        put_u32(&mut bytes, 16, 128);
        put_u32(&mut bytes, 28, 7);
        put_u32(&mut bytes, 76, DDS_PIXEL_FORMAT_SIZE);
        put_u32(&mut bytes, 80, DDPF_FOUR_CC);
        bytes[84..88].copy_from_slice(&four_cc);
        put_u32(&mut bytes, 88, rgb_bit_count);
        put_u32(&mut bytes, 108, 0x1000);
        bytes
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
