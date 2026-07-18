//! Parser for New World region distribution files.
//!
//! Region distribution files are columnar placement tables referenced by
//! `\region.distribution` in the New World 3-26 region path setup.

use std::{
    borrow::Cow,
    fmt, io,
    path::{Path, PathBuf},
};

use nw_asset::normalize_virtual_path;
use thiserror::Error;

pub const PACKED_POSITION_SIZE: usize = 4;
pub const PACKED_ROTATION_SIZE: usize = 4;
pub const PACKED_SCALE_TO_FLOAT: f32 = 0.01;

const VERSIONED_MAGIC: u16 = 0xb9d6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Distribution<'a> {
    bytes: &'a [u8],
    layout: DistributionLayout,
    entries: DistributionEntries<'a>,
    primary: PrimaryPlacements<'a>,
    point_layers: [PointLayer<'a>; 2],
}

impl<'a> Distribution<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ParseError> {
        let layout = if read_u16(bytes, 0)? == VERSIONED_MAGIC {
            DistributionLayout::Versioned {
                version: read_u16(bytes, 2)?,
            }
        } else {
            DistributionLayout::Compact
        };
        Self::parse_layout(bytes, layout)
    }

    fn parse_layout(bytes: &'a [u8], layout: DistributionLayout) -> Result<Self, ParseError> {
        let mut cursor = Cursor::new(bytes, layout.table_offset());
        let entries = DistributionEntries::parse(&mut cursor)?;
        let primary = PrimaryPlacements::parse(&mut cursor, layout)?;
        let point_layers = [
            PointLayer::parse(&mut cursor)?,
            PointLayer::parse(&mut cursor)?,
        ];

        if cursor.remaining() != 0 {
            return Err(ParseError::TrailingBytes {
                offset: cursor.offset,
                trailing: cursor.remaining(),
            });
        }

        Ok(Self {
            bytes,
            layout,
            entries,
            primary,
            point_layers,
        })
    }

    #[inline]
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[inline]
    #[must_use]
    pub const fn layout(&self) -> DistributionLayout {
        self.layout
    }

    #[inline]
    #[must_use]
    pub const fn entries(&self) -> DistributionEntries<'a> {
        self.entries
    }

    #[inline]
    #[must_use]
    pub const fn primary_placements(&self) -> PrimaryPlacements<'a> {
        self.primary
    }

    #[inline]
    #[must_use]
    pub const fn point_layers(&self) -> [PointLayer<'a>; 2] {
        self.point_layers
    }

    #[inline]
    #[must_use]
    pub fn summary(&self) -> DistributionSummary {
        DistributionSummary::from_distribution(self)
    }

    #[inline]
    #[must_use]
    pub const fn inspection_report(&self, entries: bool) -> DistributionInspectionReport<'a> {
        DistributionInspectionReport {
            distribution: *self,
            entries,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DistributionInspectionReport<'a> {
    distribution: Distribution<'a>,
    entries: bool,
}

impl fmt::Display for DistributionInspectionReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.distribution.summary())?;
        if self.entries {
            for (index, entry) in self.distribution.entries().iter().enumerate() {
                writeln!(f, "  entry[{index}]: {entry}")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DistributionFileInspectionReport<'path, 'data> {
    path: &'path Path,
    report: DistributionInspectionReport<'data>,
}

impl fmt::Display for DistributionFileInspectionReport<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.path.display())?;
        write!(f, "{}", self.report)
    }
}

pub fn inspect_distribution_file<'path, 'data>(
    path: &'path Path,
    bytes: &'data [u8],
    entries: bool,
) -> Result<DistributionFileInspectionReport<'path, 'data>, ParseError> {
    let distribution = Distribution::parse(bytes)?;
    Ok(DistributionFileInspectionReport {
        path,
        report: distribution.inspection_report(entries),
    })
}

#[derive(Debug, Error)]
pub enum DistributionInspectionError {
    #[error("read distribution asset {path:?}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("parse distribution asset {path:?}")]
    Parse {
        path: PathBuf,
        #[source]
        source: ParseError,
    },
}

pub fn inspect_distribution_path(
    path: impl AsRef<Path>,
    entries: bool,
) -> Result<String, DistributionInspectionError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| DistributionInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    inspect_distribution_file(path, &bytes, entries)
        .map(|report| report.to_string())
        .map_err(|source| DistributionInspectionError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionLayout {
    Compact,
    Versioned { version: u16 },
}

impl DistributionLayout {
    #[inline]
    #[must_use]
    pub const fn table_offset(self) -> usize {
        match self {
            Self::Compact => 0,
            Self::Versioned { .. } => 4,
        }
    }

    #[inline]
    #[must_use]
    pub const fn has_height_modes(self) -> bool {
        match self {
            Self::Compact => false,
            Self::Versioned { version } => version >= 1,
        }
    }

    #[inline]
    #[must_use]
    pub const fn version(self) -> Option<u16> {
        match self {
            Self::Compact => None,
            Self::Versioned { version } => Some(version),
        }
    }
}

impl fmt::Display for DistributionLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Compact => "compact",
            Self::Versioned { .. } => "versioned",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DistributionSummary {
    pub layout: DistributionLayout,
    pub entries: u16,
    pub primary_placements: u32,
    pub point_layer_counts: [u32; 2],
    pub has_height_modes: bool,
    pub bytes: usize,
}

impl DistributionSummary {
    #[inline]
    #[must_use]
    pub fn from_distribution(distribution: &Distribution<'_>) -> Self {
        let layers = distribution.point_layers();
        Self {
            layout: distribution.layout(),
            entries: distribution.entries().count(),
            primary_placements: distribution.primary_placements().count(),
            point_layer_counts: [layers[0].count(), layers[1].count()],
            has_height_modes: distribution.primary_placements().has_height_modes(),
            bytes: distribution.bytes().len(),
        }
    }
}

impl fmt::Display for DistributionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  layout:             {}", self.layout)?;
        if let Some(version) = self.layout.version() {
            writeln!(f, "  version:            {version}")?;
        }
        writeln!(f, "  entries:            {}", self.entries)?;
        writeln!(f, "  primary placements: {}", self.primary_placements)?;
        writeln!(f, "  point layer 0:      {}", self.point_layer_counts[0])?;
        writeln!(f, "  point layer 1:      {}", self.point_layer_counts[1])?;
        writeln!(f, "  height modes:       {}", self.has_height_modes)?;
        write!(f, "  bytes:              {}", self.bytes)
    }
}

pub fn summarize_distribution(bytes: &[u8]) -> Result<DistributionSummary, ParseError> {
    Distribution::parse(bytes).map(|distribution| distribution.summary())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DistributionEntries<'a> {
    count: u16,
    slice_paths: StringColumn<'a>,
    variants: StringColumn<'a>,
}

impl<'a> DistributionEntries<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseError> {
        let count = cursor.read_u16()?;
        let slice_paths = StringColumn::parse(cursor, count, StringSection::SlicePath)?;
        let variants = StringColumn::parse(cursor, count, StringSection::Variant)?;
        Ok(Self {
            count,
            slice_paths,
            variants,
        })
    }

    #[inline]
    #[must_use]
    pub const fn count(self) -> u16 {
        self.count
    }

    #[inline]
    #[must_use]
    pub const fn slice_paths(self) -> StringColumn<'a> {
        self.slice_paths
    }

    #[inline]
    #[must_use]
    pub const fn variants(self) -> StringColumn<'a> {
        self.variants
    }

    #[inline]
    #[must_use]
    pub const fn iter(self) -> Entries<'a> {
        Entries {
            remaining: self.count,
            slice_paths: self.slice_paths.iter(),
            variants: self.variants.iter(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DistributionEntry<'a> {
    pub slice_path: &'a str,
    pub variant: &'a str,
}

impl DistributionEntry<'_> {
    /// Resolve the descriptor's compact slice name to its authored source
    /// path. New World omits both `slices/` and `.dynamicslice` in the common
    /// compact representation.
    #[must_use]
    pub fn dynamic_slice_source_path(&self) -> Option<Cow<'_, str>> {
        dynamic_slice_source_path(self.slice_path)
    }

    /// Resolve the descriptor's exact variant metadata companion.
    #[must_use]
    pub fn variant_metadata_source_path(&self) -> Option<String> {
        let slice = self.dynamic_slice_source_path()?;
        slice_metadata_source_path(slice.as_ref(), self.variant)
    }
}

/// Resolve a compact vegetation descriptor path to a dynamic-slice source.
#[must_use]
pub fn dynamic_slice_source_path(path: &str) -> Option<Cow<'_, str>> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let file_name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    if file_name.contains('.') {
        return Some(Cow::Owned(normalize_virtual_path(path)));
    }
    let normalized = normalize_virtual_path(path);
    if normalized.starts_with("slices/") {
        Some(Cow::Owned(format!("{normalized}.dynamicslice")))
    } else {
        Some(Cow::Owned(format!("slices/{normalized}.dynamicslice")))
    }
}

/// Build the synthetic source path for one named slice variant.
#[must_use]
pub fn slice_metadata_source_path(source_path: &str, variant: &str) -> Option<String> {
    let source_path = normalize_virtual_path(source_path);
    let mut variant = normalize_virtual_path(variant);
    variant = variant.replace('/', "_");
    if variant.is_empty() {
        return None;
    }
    let stem = source_path
        .strip_suffix(".dynamicslice")
        .or_else(|| source_path.strip_suffix(".slice"))?;
    Some(format!("{stem}_{variant}.slice.meta"))
}

impl fmt::Display for DistributionEntry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "slice={:?} variant={:?}", self.slice_path, self.variant)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entries<'a> {
    remaining: u16,
    slice_paths: StringValues<'a>,
    variants: StringValues<'a>,
}

impl<'a> Iterator for Entries<'a> {
    type Item = DistributionEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some(DistributionEntry {
            slice_path: self
                .slice_paths
                .next()
                .expect("distribution entry table was validated"),
            variant: self
                .variants
                .next()
                .expect("distribution entry table was validated"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringColumn<'a> {
    count: u16,
    bytes: &'a [u8],
}

impl<'a> StringColumn<'a> {
    fn parse(
        cursor: &mut Cursor<'a>,
        count: u16,
        section: StringSection,
    ) -> Result<Self, ParseError> {
        let start = cursor.offset;
        for index in 0..count {
            let len = cursor.read_u8()? as usize;
            let bytes = cursor.read_bytes(len)?;
            std::str::from_utf8(bytes).map_err(|source| ParseError::InvalidString {
                section,
                index,
                source,
            })?;
        }
        let bytes = &cursor.bytes[start..cursor.offset];
        Ok(Self { count, bytes })
    }

    #[inline]
    #[must_use]
    pub const fn count(self) -> u16 {
        self.count
    }

    #[inline]
    #[must_use]
    pub const fn iter(self) -> StringValues<'a> {
        StringValues {
            remaining: self.count,
            bytes: self.bytes,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringValues<'a> {
    remaining: u16,
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for StringValues<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        let len = self.bytes[self.offset] as usize;
        self.offset += 1;
        let start = self.offset;
        self.offset += len;
        Some(std::str::from_utf8(&self.bytes[start..self.offset]).expect("validated UTF-8"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimaryPlacements<'a> {
    count: u32,
    descriptor_indices: &'a [u8],
    positions: &'a [u8],
    rotations: &'a [u8],
    scales: &'a [u8],
    height_modes: Option<&'a [u8]>,
}

impl<'a> PrimaryPlacements<'a> {
    fn parse(cursor: &mut Cursor<'a>, layout: DistributionLayout) -> Result<Self, ParseError> {
        let count = cursor.read_u32()?;
        let len = count as usize;
        let descriptor_indices = cursor.read_bytes(checked_mul(len, 2)?)?;
        let positions = cursor.read_bytes(checked_mul(len, PACKED_POSITION_SIZE)?)?;
        let rotations = cursor.read_bytes(checked_mul(len, PACKED_ROTATION_SIZE)?)?;
        let scales = cursor.read_bytes(len)?;
        let height_modes = layout
            .has_height_modes()
            .then(|| cursor.read_bytes(len))
            .transpose()?;
        Ok(Self {
            count,
            descriptor_indices,
            positions,
            rotations,
            scales,
            height_modes,
        })
    }

    #[inline]
    #[must_use]
    pub const fn count(self) -> u32 {
        self.count
    }

    #[inline]
    #[must_use]
    pub const fn has_height_modes(self) -> bool {
        self.height_modes.is_some()
    }

    #[inline]
    #[must_use]
    pub const fn iter(self) -> PrimaryPlacementIter<'a> {
        PrimaryPlacementIter {
            remaining: self.count,
            placements: self,
            index: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimaryPlacement {
    pub descriptor_index: u16,
    pub position: PackedPosition,
    pub rotation: PackedRotation,
    pub scale: PackedScale,
    pub height_mode: PlacementHeightMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimaryPlacementIter<'a> {
    remaining: u32,
    placements: PrimaryPlacements<'a>,
    index: usize,
}

impl Iterator for PrimaryPlacementIter<'_> {
    type Item = PrimaryPlacement;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let index = self.index;
        self.index += 1;

        Some(PrimaryPlacement {
            descriptor_index: read_u16_unchecked(self.placements.descriptor_indices, index * 2),
            position: read_position_unchecked(self.placements.positions, index * 4),
            rotation: PackedRotation::new(read_u32_unchecked(self.placements.rotations, index * 4)),
            scale: PackedScale::new(self.placements.scales[index]),
            height_mode: self
                .placements
                .height_modes
                .map_or(PlacementHeightMode::Terrain, |height_modes| {
                    PlacementHeightMode::from_u8(height_modes[index])
                }),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointLayer<'a> {
    count: u32,
    positions: &'a [u8],
    tags: &'a [u8],
}

impl<'a> PointLayer<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseError> {
        let count = cursor.read_u32()?;
        let len = count as usize;
        let positions = cursor.read_bytes(checked_mul(len, PACKED_POSITION_SIZE)?)?;
        let tags = cursor.read_bytes(len)?;
        Ok(Self {
            count,
            positions,
            tags,
        })
    }

    #[inline]
    #[must_use]
    pub const fn count(self) -> u32 {
        self.count
    }

    #[inline]
    #[must_use]
    pub const fn iter(self) -> PointIter<'a> {
        PointIter {
            remaining: self.count,
            layer: self,
            index: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub position: PackedPosition,
    pub tag: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointIter<'a> {
    remaining: u32,
    layer: PointLayer<'a>,
    index: usize,
}

impl Iterator for PointIter<'_> {
    type Item = Point;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let index = self.index;
        self.index += 1;
        Some(Point {
            position: read_position_unchecked(self.layer.positions, index * 4),
            tag: self.layer.tags[index],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedPosition {
    pub x: u16,
    pub y: u16,
}

impl PackedPosition {
    #[inline]
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackedRotation(u32);

impl PackedRotation {
    #[inline]
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[inline]
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackedScale(u8);

impl PackedScale {
    #[inline]
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    #[inline]
    #[must_use]
    pub const fn raw(self) -> u8 {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn as_f32(self) -> f32 {
        self.0 as f32 * PACKED_SCALE_TO_FLOAT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementHeightMode {
    Terrain,
    MaxTerrainAndSurface,
    Surface,
    Other(u8),
}

impl PlacementHeightMode {
    #[inline]
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Terrain,
            1 => Self::MaxTerrainAndSurface,
            2 => Self::Surface,
            value => Self::Other(value),
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Terrain => 0,
            Self::MaxTerrainAndSurface => 1,
            Self::Surface => 2,
            Self::Other(value) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringSection {
    SlicePath,
    Variant,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("distribution file is too short: need {needed} bytes, got {actual}")]
    TooShort { needed: usize, actual: usize },

    #[error("distribution string in {section:?}[{index}] is not UTF-8")]
    InvalidString {
        section: StringSection,
        index: u16,
        source: std::str::Utf8Error,
    },

    #[error("distribution file has {trailing} trailing byte(s) at offset {offset}")]
    TrailingBytes { offset: usize, trailing: usize },

    #[error("distribution field length overflow")]
    LengthOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    #[inline]
    const fn new(bytes: &'a [u8], offset: usize) -> Self {
        Self { bytes, offset }
    }

    fn read_u8(&mut self) -> Result<u8, ParseError> {
        let bytes = self.read_bytes(1)?;
        Ok(bytes[0])
    }

    fn read_u16(&mut self) -> Result<u16, ParseError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().expect("slice size")))
    }

    fn read_u32(&mut self) -> Result<u32, ParseError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("slice size")))
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ParseError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(ParseError::LengthOverflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(ParseError::TooShort {
                needed: end,
                actual: self.bytes.len(),
            })?;
        self.offset = end;
        Ok(bytes)
    }

    #[inline]
    const fn remaining(self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
}

fn checked_mul(left: usize, right: usize) -> Result<usize, ParseError> {
    left.checked_mul(right).ok_or(ParseError::LengthOverflow)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ParseError> {
    let end = offset.checked_add(2).ok_or(ParseError::LengthOverflow)?;
    let bytes = bytes.get(offset..end).ok_or(ParseError::TooShort {
        needed: end,
        actual: bytes.len(),
    })?;
    Ok(u16::from_le_bytes(bytes.try_into().expect("slice size")))
}

fn read_u16_unchecked(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("slice size"))
}

fn read_u32_unchecked(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice size"))
}

fn read_position_unchecked(bytes: &[u8], offset: usize) -> PackedPosition {
    PackedPosition::new(
        read_u16_unchecked(bytes, offset),
        read_u16_unchecked(bytes, offset + 2),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_descriptor_slice_and_variant_paths() {
        let entry = DistributionEntry {
            slice_path: "Gatherables/OakTree",
            variant: "Oak/Young",
        };
        assert_eq!(
            entry.dynamic_slice_source_path().as_deref(),
            Some("slices/gatherables/oaktree.dynamicslice")
        );
        assert_eq!(
            entry.variant_metadata_source_path().as_deref(),
            Some("slices/gatherables/oaktree_oak_young.slice.meta")
        );
    }

    #[test]
    fn parses_compact_distribution() {
        let bytes = compact_bytes();
        let distribution = Distribution::parse(&bytes).unwrap();

        assert_eq!(distribution.layout(), DistributionLayout::Compact);
        assert_eq!(distribution.entries().count(), 2);
        assert_eq!(
            distribution.summary(),
            DistributionSummary {
                layout: DistributionLayout::Compact,
                entries: 2,
                primary_placements: 1,
                point_layer_counts: [1, 0],
                has_height_modes: false,
                bytes: bytes.len(),
            }
        );
        assert_eq!(
            distribution.summary().to_string(),
            "  layout:             compact\n  entries:            2\n  primary placements: 1\n  point layer 0:      1\n  point layer 1:      0\n  height modes:       false\n  bytes:              62"
        );
        assert_eq!(
            distribution.inspection_report(true).to_string(),
            "  layout:             compact\n  entries:            2\n  primary placements: 1\n  point layer 0:      1\n  point layer 1:      0\n  height modes:       false\n  bytes:              62\n  entry[0]: slice=\"\" variant=\"\"\n  entry[1]: slice=\"gatherables/master_tree\" variant=\"TreeA\"\n"
        );
        assert_eq!(
            inspect_distribution_file(Path::new("levels/a/region.distribution"), &bytes, false)
                .unwrap()
                .to_string(),
            "levels/a/region.distribution\n  layout:             compact\n  entries:            2\n  primary placements: 1\n  point layer 0:      1\n  point layer 1:      0\n  height modes:       false\n  bytes:              62\n"
        );

        let entries: Vec<_> = distribution.entries().iter().collect();
        assert_eq!(
            entries,
            vec![
                DistributionEntry {
                    slice_path: "",
                    variant: "",
                },
                DistributionEntry {
                    slice_path: "gatherables/master_tree",
                    variant: "TreeA",
                },
            ]
        );
        assert_eq!(
            entries[1].to_string(),
            "slice=\"gatherables/master_tree\" variant=\"TreeA\""
        );

        let primary: Vec<_> = distribution.primary_placements().iter().collect();
        assert_eq!(
            primary,
            vec![PrimaryPlacement {
                descriptor_index: 1,
                position: PackedPosition::new(100, 200),
                rotation: PackedRotation::new(0x0016_000b),
                scale: PackedScale::new(5),
                height_mode: PlacementHeightMode::Terrain,
            }]
        );
        assert!(!distribution.primary_placements().has_height_modes());

        let layers = distribution.point_layers();
        assert_eq!(layers[0].count(), 1);
        assert_eq!(
            layers[0].iter().next(),
            Some(Point {
                position: PackedPosition::new(10, 20),
                tag: 7,
            })
        );
        assert_eq!(layers[1].count(), 0);
    }

    #[test]
    fn parses_versioned_distribution() {
        let bytes = extended_bytes();
        let distribution = Distribution::parse(&bytes).unwrap();

        assert_eq!(
            distribution.layout(),
            DistributionLayout::Versioned { version: 1 }
        );
        assert!(distribution.primary_placements().has_height_modes());
        assert_eq!(
            distribution.primary_placements().iter().next(),
            Some(PrimaryPlacement {
                descriptor_index: 1,
                position: PackedPosition::new(100, 200),
                rotation: PackedRotation::new(0x0016_000b),
                scale: PackedScale::new(5),
                height_mode: PlacementHeightMode::Surface,
            })
        );
    }

    #[test]
    fn formats_layout_labels() {
        assert_eq!(DistributionLayout::Compact.to_string(), "compact");
        let layout = DistributionLayout::Versioned { version: 1 };
        assert_eq!(layout.to_string(), "versioned");
        assert_eq!(layout.version(), Some(1));
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut bytes = compact_bytes();
        bytes.push(0);

        assert!(matches!(
            Distribution::parse(&bytes).unwrap_err(),
            ParseError::TrailingBytes { .. }
        ));
    }

    fn compact_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        push_entry_table(&mut bytes);
        push_primary(&mut bytes, false);
        push_point_layer(&mut bytes, &[(10, 20, 7)]);
        push_point_layer(&mut bytes, &[]);
        bytes
    }

    fn extended_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&VERSIONED_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        push_entry_table(&mut bytes);
        push_primary(&mut bytes, true);
        push_point_layer(&mut bytes, &[]);
        push_point_layer(&mut bytes, &[]);
        bytes
    }

    fn push_entry_table(bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&2u16.to_le_bytes());
        push_string(bytes, "");
        push_string(bytes, "gatherables/master_tree");
        push_string(bytes, "");
        push_string(bytes, "TreeA");
    }

    fn push_primary(bytes: &mut Vec<u8>, include_height_modes: bool) {
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&100u16.to_le_bytes());
        bytes.extend_from_slice(&200u16.to_le_bytes());
        bytes.extend_from_slice(&11u16.to_le_bytes());
        bytes.extend_from_slice(&22u16.to_le_bytes());
        bytes.push(5);
        if include_height_modes {
            bytes.push(2);
        }
    }

    fn push_point_layer(bytes: &mut Vec<u8>, points: &[(u16, u16, u8)]) {
        bytes.extend_from_slice(&(points.len() as u32).to_le_bytes());
        for &(x, y, _) in points {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }
        for &(_, _, tag) in points {
            bytes.push(tag);
        }
    }

    fn push_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.push(value.len() as u8);
        bytes.extend_from_slice(value.as_bytes());
    }
}
