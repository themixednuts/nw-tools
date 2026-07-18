//! Parser for New World `region.heightmap` files.
//!
//! Terrain region heightmaps are 16-bit TIFF images. The samples are
//! stored in source image order, with terrain-space lookups flipping
//! the Y coordinate to match the region coordinate system used by the
//! terrain tile files.

use std::{
    fmt, io,
    io::Cursor,
    path::{Path, PathBuf},
};

use crate::RegionPathMeta;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Luma};

use crate::mapsettings::MapSettings;

/// Parsed `region.heightmap` data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionHeightmap {
    /// Source image width in samples.
    pub width: u32,
    /// Source image height in samples.
    pub height: u32,
    /// Height samples in source image row-major order.
    pub samples: Vec<u16>,
}

impl RegionHeightmap {
    /// Parse a `region.heightmap` TIFF payload from bytes.
    pub fn parse_tiff(bytes: &[u8]) -> Result<Self, ParseError> {
        let mut patched = bytes.to_vec();
        patch_single_channel_rgb_photometric(&mut patched);
        let image = image::load_from_memory_with_format(&patched, ImageFormat::Tiff)
            .map_err(ParseError::Decode)?;
        Self::from_dynamic_image(image)
    }

    /// Encode this heightmap as a `region.heightmap` TIFF payload.
    pub fn to_tiff_bytes(&self) -> Result<Vec<u8>, WriteError> {
        let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(
            self.width,
            self.height,
            self.samples.clone(),
        )
        .ok_or(WriteError::SampleCountMismatch {
            width: self.width,
            height: self.height,
            samples: self.samples.len(),
        })?;
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageLuma16(image)
            .write_to(&mut cursor, ImageFormat::Tiff)
            .map_err(WriteError::Encode)?;
        Ok(cursor.into_inner())
    }

    fn from_dynamic_image(image: DynamicImage) -> Result<Self, ParseError> {
        let (width, height) = image.dimensions();
        if width == 0 || height == 0 {
            return Err(ParseError::EmptyImage { width, height });
        }

        let expected_len = sample_len(width, height)?;
        let samples = match image {
            DynamicImage::ImageLuma8(buffer) => buffer
                .into_raw()
                .into_iter()
                .map(expand_u8_sample)
                .collect(),
            DynamicImage::ImageLumaA8(buffer) => buffer
                .pixels()
                .map(|pixel| expand_u8_sample(pixel.0[0]))
                .collect(),
            DynamicImage::ImageRgb8(buffer) => buffer
                .pixels()
                .map(|pixel| expand_u8_sample(pixel.0[0]))
                .collect(),
            DynamicImage::ImageRgba8(buffer) => buffer
                .pixels()
                .map(|pixel| expand_u8_sample(pixel.0[0]))
                .collect(),
            DynamicImage::ImageLuma16(buffer) => buffer.into_raw(),
            DynamicImage::ImageLumaA16(buffer) => buffer.pixels().map(|pixel| pixel.0[0]).collect(),
            DynamicImage::ImageRgb16(buffer) => buffer.pixels().map(|pixel| pixel.0[0]).collect(),
            DynamicImage::ImageRgba16(buffer) => buffer.pixels().map(|pixel| pixel.0[0]).collect(),
            DynamicImage::ImageRgb32F(buffer) => buffer
                .pixels()
                .map(|pixel| expand_f32_sample(pixel.0[0]))
                .collect(),
            DynamicImage::ImageRgba32F(buffer) => buffer
                .pixels()
                .map(|pixel| expand_f32_sample(pixel.0[0]))
                .collect(),
            other => other.to_luma16().into_raw(),
        };

        if samples.len() != expected_len {
            return Err(ParseError::SampleCountMismatch {
                width,
                height,
                expected: expected_len,
                found: samples.len(),
            });
        }

        Ok(Self {
            width,
            height,
            samples,
        })
    }

    /// Number of samples in the heightmap.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// True when width and height match.
    pub fn is_square(&self) -> bool {
        self.width == self.height
    }

    /// Minimum sample value in the heightmap.
    pub fn min_sample(&self) -> Option<u16> {
        self.samples.iter().copied().min()
    }

    /// Maximum sample value in the heightmap.
    pub fn max_sample(&self) -> Option<u16> {
        self.samples.iter().copied().max()
    }

    /// Read a sample using source image coordinates, where `(0, 0)`
    /// is the first pixel in the TIFF.
    pub fn sample_top_left(&self, x: u32, y: u32) -> Option<u16> {
        let index = self.index(x, y)?;
        self.samples.get(index).copied()
    }

    /// Read a sample using terrain coordinates, where `(0, 0)` is
    /// the bottom-left sample of the region.
    pub fn sample_terrain_xy(&self, x: u32, y: u32) -> Option<u16> {
        if y >= self.height {
            return None;
        }
        self.sample_top_left(x, self.height - y - 1)
    }

    /// Validate this heightmap against the region's terrain settings.
    pub fn validate_settings(&self, settings: MapSettings) -> Result<(), SettingsError> {
        if self.width != settings.region_size || self.height != settings.region_size {
            return Err(SettingsError::RegionSizeMismatch {
                settings: settings.region_size,
                width: self.width,
                height: self.height,
            });
        }
        Ok(())
    }

    /// Region width/depth in world units.
    #[must_use]
    pub fn region_world_size(&self, settings: MapSettings) -> f32 {
        settings.region_size as f32 * settings.cell_resolution as f32
    }

    /// Region origin in world coordinates.
    #[must_use]
    pub fn region_origin(&self, meta: &RegionPathMeta, settings: MapSettings) -> (f32, f32) {
        let size = self.region_world_size(settings);
        (meta.x as f32 * size, meta.y as f32 * size)
    }

    /// Sample a world-space height using region metadata and settings.
    #[must_use]
    pub fn height_at_world(
        &self,
        meta: &RegionPathMeta,
        settings: MapSettings,
        x: f32,
        y: f32,
    ) -> Option<f32> {
        let cell_size = settings.cell_resolution as f32;
        if cell_size <= 0.0 {
            return None;
        }

        let (origin_x, origin_y) = self.region_origin(meta, settings);
        let local_x = (x - origin_x) / cell_size;
        let local_y = (y - origin_y) / cell_size;
        let region_size = settings.region_size as f32;
        if !(0.0..=region_size).contains(&local_x) || !(0.0..=region_size).contains(&local_y) {
            return None;
        }
        let max_x = self.width.checked_sub(1)? as f32;
        let max_y = self.height.checked_sub(1)? as f32;
        self.bilinear_height(local_x.clamp(0.0, max_x), local_y.clamp(0.0, max_y))
    }

    /// Bilinear-sample a terrain-space height in sample coordinates.
    #[must_use]
    pub fn bilinear_height(&self, x: f32, y: f32) -> Option<f32> {
        let max_x = self.width.checked_sub(1)? as f32;
        let max_y = self.height.checked_sub(1)? as f32;
        if !(0.0..=max_x).contains(&x) || !(0.0..=max_y).contains(&y) {
            return None;
        }

        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;

        let h00 = self.sample_terrain_xy(x0, y0)? as f32;
        let h10 = self.sample_terrain_xy(x1, y0)? as f32;
        let h01 = self.sample_terrain_xy(x0, y1)? as f32;
        let h11 = self.sample_terrain_xy(x1, y1)? as f32;

        let h0 = lerp(h00, h10, fx);
        let h1 = lerp(h01, h11, fx);
        Some(lerp(h0, h1, fy))
    }

    #[must_use]
    pub fn summary(&self) -> RegionHeightmapSummary {
        RegionHeightmapSummary::from_map(self)
    }

    #[must_use]
    pub fn source_ascii_rows(&self, step: u32) -> Vec<String> {
        let palette = b" .:-=+*#%@";
        let min = self.min_sample().unwrap_or(0);
        let max = self.max_sample().unwrap_or(min);
        let span = u32::from(max.saturating_sub(min)).max(1);
        let step = step.max(1) as usize;

        let mut rows = Vec::new();
        let mut y = 0usize;
        while y < self.height as usize {
            let mut row = String::with_capacity(self.width as usize / step + 1);
            let mut x = 0usize;
            while x < self.width as usize {
                let mut sum = 0u64;
                let mut count = 0u64;
                for dy in 0..step.min(self.height as usize - y) {
                    for dx in 0..step.min(self.width as usize - x) {
                        if let Some(sample) = self.sample_top_left((x + dx) as u32, (y + dy) as u32)
                        {
                            sum += u64::from(sample);
                            count += 1;
                        }
                    }
                }

                let glyph = if let Some(average) = sum.checked_div(count) {
                    let average = average as u16;
                    let normalized =
                        u32::from(average.saturating_sub(min)) * (palette.len() as u32 - 1) / span;
                    palette[normalized as usize]
                } else {
                    b' '
                };
                row.push(glyph as char);
                x += step;
            }
            rows.push(row);
            y += step;
        }
        rows
    }

    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionHeightmapSummary {
    pub width: u32,
    pub height: u32,
    pub samples: usize,
    pub square: bool,
    pub min_sample: Option<u16>,
    pub max_sample: Option<u16>,
}

impl RegionHeightmapSummary {
    #[must_use]
    pub fn from_map(map: &RegionHeightmap) -> Self {
        Self {
            width: map.width,
            height: map.height,
            samples: map.sample_count(),
            square: map.is_square(),
            min_sample: map.min_sample(),
            max_sample: map.max_sample(),
        }
    }
}

impl fmt::Display for RegionHeightmapSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  dimensions:     {} x {}", self.width, self.height)?;
        writeln!(f, "  samples:        {}", self.samples)?;
        writeln!(f, "  square:         {}", self.square)?;
        write!(
            f,
            "  min/max:        {} / {}",
            self.min_sample.unwrap_or(0),
            self.max_sample.unwrap_or(0)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionHeightmapInspection {
    pub summary: RegionHeightmapSummary,
    map: RegionHeightmap,
}

impl RegionHeightmapInspection {
    #[must_use]
    pub fn sample_terrain_xy(&self, x: u32, y: u32) -> Option<u16> {
        self.map.sample_terrain_xy(x, y)
    }

    #[must_use]
    pub fn source_ascii_rows(&self, step: u32) -> Vec<String> {
        self.map.source_ascii_rows(step)
    }

    #[must_use]
    pub const fn source_ascii_report(&self, step: u32) -> RegionHeightmapAsciiReport<'_> {
        RegionHeightmapAsciiReport {
            inspection: self,
            step,
        }
    }

    #[must_use]
    pub const fn inspection_report<'a>(
        &'a self,
        meta: Option<&'a RegionPathMeta>,
        samples: &'a [(u32, u32)],
        ascii_step: Option<u32>,
    ) -> RegionHeightmapInspectionReport<'a> {
        RegionHeightmapInspectionReport {
            inspection: self,
            meta,
            samples,
            ascii_step,
        }
    }

    #[must_use]
    pub fn into_map(self) -> RegionHeightmap {
        self.map
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RegionHeightmapInspectionReport<'a> {
    inspection: &'a RegionHeightmapInspection,
    meta: Option<&'a RegionPathMeta>,
    samples: &'a [(u32, u32)],
    ascii_step: Option<u32>,
}

impl fmt::Display for RegionHeightmapInspectionReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(meta) = self.meta {
            writeln!(f, "  level:          {}", meta.level)?;
            writeln!(f, "  region:         {}, {}", meta.x, meta.y)?;
        }
        writeln!(f, "{}", self.inspection.summary)?;

        for &(x, y) in self.samples {
            match self.inspection.sample_terrain_xy(x, y) {
                Some(value) => writeln!(f, "  sample[{x},{y}]: {value}")?,
                None => writeln!(f, "  sample[{x},{y}]: <out of bounds>")?,
            }
        }

        if let Some(step) = self.ascii_step {
            write!(f, "{}", self.inspection.source_ascii_report(step))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RegionHeightmapAsciiReport<'a> {
    inspection: &'a RegionHeightmapInspection,
    step: u32,
}

impl fmt::Display for RegionHeightmapAsciiReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let step = self.step.max(1);
        writeln!(f)?;
        writeln!(f, "  source-image ASCII art (step={step}):")?;
        for row in self.inspection.source_ascii_rows(step) {
            writeln!(f, "    {row}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionHeightmapFileInspectionReport<'path, 'samples> {
    pub path: &'path Path,
    pub inspection: RegionHeightmapInspection,
    pub meta: Option<RegionPathMeta>,
    pub samples: &'samples [(u32, u32)],
    pub ascii_step: Option<u32>,
}

impl fmt::Display for RegionHeightmapFileInspectionReport<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.path.display())?;
        write!(
            f,
            "{}",
            self.inspection
                .inspection_report(self.meta.as_ref(), self.samples, self.ascii_step)
        )
    }
}

pub fn inspect_heightmap(bytes: &[u8]) -> Result<RegionHeightmapInspection, ParseError> {
    let map = RegionHeightmap::parse_tiff(bytes)?;
    Ok(RegionHeightmapInspection {
        summary: map.summary(),
        map,
    })
}

pub fn inspect_heightmap_file<'path, 'samples>(
    path: &'path Path,
    bytes: &[u8],
    samples: &'samples [(u32, u32)],
    ascii_step: Option<u32>,
) -> Result<RegionHeightmapFileInspectionReport<'path, 'samples>, ParseError> {
    let inspection = inspect_heightmap(bytes)?;
    let meta = RegionPathMeta::parse(path.to_string_lossy());
    Ok(RegionHeightmapFileInspectionReport {
        path,
        inspection,
        meta,
        samples,
        ascii_step,
    })
}

#[derive(Debug)]
pub enum HeightmapInspectionError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, source: ParseError },
}

impl fmt::Display for HeightmapInspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "read heightmap asset {path:?}: {source}")
            }
            Self::Parse { path, source } => {
                write!(f, "parse heightmap asset {path:?}: {source}")
            }
        }
    }
}

impl std::error::Error for HeightmapInspectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

pub fn inspect_heightmap_path<'path, 'samples>(
    path: &'path Path,
    samples: &'samples [(u32, u32)],
    ascii_step: Option<u32>,
) -> Result<RegionHeightmapFileInspectionReport<'path, 'samples>, HeightmapInspectionError> {
    let bytes = std::fs::read(path).map_err(|source| HeightmapInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    inspect_heightmap_file(path, &bytes, samples, ascii_step).map_err(|source| {
        HeightmapInspectionError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })
}

pub fn summarize_heightmap(bytes: &[u8]) -> Result<RegionHeightmapSummary, ParseError> {
    let map = RegionHeightmap::parse_tiff(bytes)?;
    Ok(map.summary())
}

/// Parse error returned by [`RegionHeightmap::parse_tiff`].
#[derive(Debug)]
pub enum ParseError {
    /// TIFF decoding failed.
    Decode(image::ImageError),
    /// Image dimensions were empty.
    EmptyImage { width: u32, height: u32 },
    /// Image dimensions exceeded host addressable memory.
    DimensionTooLarge { width: u32, height: u32 },
    /// Decoded sample count did not match `width * height`.
    SampleCountMismatch {
        width: u32,
        height: u32,
        expected: usize,
        found: usize,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(err) => write!(f, "decode TIFF: {err}"),
            Self::EmptyImage { width, height } => {
                write!(f, "empty heightmap image: {width} x {height}")
            }
            Self::DimensionTooLarge { width, height } => {
                write!(f, "heightmap dimensions are too large: {width} x {height}")
            }
            Self::SampleCountMismatch {
                width,
                height,
                expected,
                found,
            } => write!(
                f,
                "heightmap sample count mismatch for {width} x {height}: expected {expected}, found {found}"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug)]
pub enum WriteError {
    SampleCountMismatch {
        width: u32,
        height: u32,
        samples: usize,
    },
    Encode(image::ImageError),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SampleCountMismatch {
                width,
                height,
                samples,
            } => write!(f, "heightmap {width} x {height} has {samples} samples"),
            Self::Encode(err) => write!(f, "encode heightmap TIFF: {err}"),
        }
    }
}

impl std::error::Error for WriteError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsError {
    RegionSizeMismatch {
        settings: u32,
        width: u32,
        height: u32,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RegionSizeMismatch {
                settings,
                width,
                height,
            } => write!(
                f,
                "mapsettings regionSize {settings} does not match heightmap {width} x {height}"
            ),
        }
    }
}

impl std::error::Error for SettingsError {}

fn sample_len(width: u32, height: u32) -> Result<usize, ParseError> {
    let len = u64::from(width) * u64::from(height);
    usize::try_from(len).map_err(|_| ParseError::DimensionTooLarge { width, height })
}

fn lerp(start: f32, end: f32, amount: f32) -> f32 {
    start + (end - start) * amount
}

fn expand_u8_sample(sample: u8) -> u16 {
    u16::from(sample) * 257
}

fn expand_f32_sample(sample: f32) -> u16 {
    if sample.is_finite() {
        (sample.clamp(0.0, 1.0) * u16::MAX as f32).round() as u16
    } else {
        0
    }
}

fn patch_single_channel_rgb_photometric(bytes: &mut [u8]) -> bool {
    let Some(order) = TiffByteOrder::parse(bytes) else {
        return false;
    };
    if bytes.len() < 8 {
        return false;
    }
    if order.read_u16(&bytes[2..4]) != 42 {
        return false;
    }

    let ifd_offset = order.read_u32(&bytes[4..8]) as usize;
    let Some(ifd_header_end) = ifd_offset.checked_add(2) else {
        return false;
    };
    if ifd_header_end > bytes.len() {
        return false;
    }

    let entry_count = order.read_u16(&bytes[ifd_offset..ifd_offset + 2]) as usize;
    let entries_start = ifd_offset + 2;
    let entries_end = match entries_start.checked_add(entry_count.saturating_mul(12)) {
        Some(end) if end <= bytes.len() => end,
        _ => return false,
    };

    let mut photometric = None;
    let mut samples_per_pixel = None;
    for entry_offset in (entries_start..entries_end).step_by(12) {
        let entry = &bytes[entry_offset..entry_offset + 12];
        let tag = order.read_u16(&entry[0..2]);
        let field_type = order.read_u16(&entry[2..4]);
        let count = order.read_u32(&entry[4..8]);
        if field_type != 3 || count != 1 {
            continue;
        }

        match tag {
            262 => photometric = Some(entry_offset + 8),
            277 => samples_per_pixel = Some(order.read_u16(&entry[8..10])),
            _ => {}
        }
    }

    const PHOTOMETRIC_RGB: u16 = 2;
    const PHOTOMETRIC_BLACK_IS_ZERO: u16 = 1;
    if let (Some(offset), Some(1)) = (photometric, samples_per_pixel)
        && offset + 2 <= bytes.len()
        && order.read_u16(&bytes[offset..offset + 2]) == PHOTOMETRIC_RGB
    {
        order.write_u16(&mut bytes[offset..offset + 2], PHOTOMETRIC_BLACK_IS_ZERO);
        return true;
    }
    false
}

#[derive(Debug, Clone, Copy)]
enum TiffByteOrder {
    Little,
    Big,
}

impl TiffByteOrder {
    fn parse(bytes: &[u8]) -> Option<Self> {
        match bytes.get(0..2)? {
            b"II" => Some(Self::Little),
            b"MM" => Some(Self::Big),
            _ => None,
        }
    }

    fn read_u16(self, bytes: &[u8]) -> u16 {
        match self {
            Self::Little => u16::from_le_bytes([bytes[0], bytes[1]]),
            Self::Big => u16::from_be_bytes([bytes[0], bytes[1]]),
        }
    }

    fn read_u32(self, bytes: &[u8]) -> u32 {
        match self {
            Self::Little => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            Self::Big => u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        }
    }

    fn write_u16(self, bytes: &mut [u8], value: u16) {
        let encoded = match self {
            Self::Little => value.to_le_bytes(),
            Self::Big => value.to_be_bytes(),
        };
        bytes.copy_from_slice(&encoded);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageBuffer, ImageFormat, Luma};

    use super::*;

    #[test]
    fn parses_synthetic_luma16_tiff() {
        let source: ImageBuffer<Luma<u16>, Vec<u16>> =
            ImageBuffer::from_raw(2, 2, vec![10, 20, 30, 40]).unwrap();
        let image = DynamicImage::ImageLuma16(source);
        let mut cursor = Cursor::new(Vec::new());
        image.write_to(&mut cursor, ImageFormat::Tiff).unwrap();

        let map = RegionHeightmap::parse_tiff(cursor.get_ref()).unwrap();
        assert_eq!(map.width, 2);
        assert_eq!(map.height, 2);
        assert_eq!(map.samples, vec![10, 20, 30, 40]);
        assert_eq!(map.sample_top_left(0, 0), Some(10));
        assert_eq!(map.sample_terrain_xy(0, 0), Some(30));
        assert_eq!(map.bilinear_height(0.5, 0.5), Some(25.0));
        let settings = MapSettings {
            cell_resolution: 2,
            region_size: 2,
            region_type: 0,
        };
        let meta = RegionPathMeta {
            level: "world".to_string(),
            x: 1,
            y: 2,
        };
        assert_eq!(map.validate_settings(settings), Ok(()));
        assert_eq!(map.region_world_size(settings), 4.0);
        assert_eq!(map.region_origin(&meta, settings), (4.0, 8.0));
        assert_eq!(map.height_at_world(&meta, settings, 5.0, 9.0), Some(25.0));
        assert_eq!(
            RegionHeightmap::parse_tiff(&map.to_tiff_bytes().unwrap()).unwrap(),
            map
        );
        assert_eq!(
            map.summary(),
            RegionHeightmapSummary {
                width: 2,
                height: 2,
                samples: 4,
                square: true,
                min_sample: Some(10),
                max_sample: Some(40),
            }
        );
        assert_eq!(
            map.summary().to_string(),
            "  dimensions:     2 x 2\n  samples:        4\n  square:         true\n  min/max:        10 / 40"
        );
        assert_eq!(
            map.source_ascii_rows(1),
            vec![" -".to_string(), "*@".to_string()]
        );

        let inspection = inspect_heightmap(cursor.get_ref()).unwrap();
        assert_eq!(inspection.summary, map.summary());
        assert_eq!(inspection.sample_terrain_xy(0, 0), Some(30));
        assert_eq!(
            inspection.source_ascii_rows(1),
            vec![" -".to_string(), "*@".to_string()]
        );
        assert!(
            inspection
                .source_ascii_report(1)
                .to_string()
                .contains("source-image ASCII art")
        );
        assert_eq!(
            inspection
                .inspection_report(None, &[(0, 0), (5, 5)], None)
                .to_string(),
            "  dimensions:     2 x 2\n  samples:        4\n  square:         true\n  min/max:        10 / 40\n  sample[0,0]: 30\n  sample[5,5]: <out of bounds>\n"
        );
        let file_report = inspect_heightmap_file(
            Path::new("levels/world/regions/r_+01_-02/region.heightmap"),
            cursor.get_ref(),
            &[(0, 0)],
            None,
        )
        .unwrap()
        .to_string();
        assert!(file_report.starts_with(
            "levels/world/regions/r_+01_-02/region.heightmap\n  level:          world\n  region:         1, -2\n"
        ));
        assert!(file_report.contains("sample[0,0]: 30"));
    }

    #[test]
    fn expands_8_bit_samples_to_16_bit_range() {
        let image = DynamicImage::ImageLuma8(ImageBuffer::from_raw(2, 1, vec![0, 255]).unwrap());

        let map = RegionHeightmap::from_dynamic_image(image).unwrap();
        assert_eq!(map.samples, vec![0, 65535]);
    }

    #[test]
    fn parses_region_path_metadata() {
        let meta = RegionPathMeta::parse(
            r"levels\newworld_vitae_et_mors\regions\r_+03_+02\region.heightmap",
        )
        .unwrap();
        assert_eq!(meta.level, "newworld_vitae_et_mors");
        assert_eq!(meta.x, 3);
        assert_eq!(meta.y, 2);
    }
}
