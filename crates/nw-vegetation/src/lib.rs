//! Parser for New World vegetation image assets.
//!
//! New World 3-26 registers these assets as `VegetationImageAsset`.

pub mod distribution;

use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use glam::{Quat, Vec2, Vec3};
use nw_asset::AssetId;
use nw_objectstream::lookup::NameLookup;
use nw_objectstream::value::ObjectStreamValueError;
use nw_objectstream::{Element, ObjectStream, ObjectStreamError};
use thiserror::Error;
use uuid::{Uuid, uuid};

pub const HEADER_SIZE: usize = 8;
pub const BLOCK_COUNT: usize = 16_384;
pub const OFFSET_SIZE: usize = 4;
pub const OFFSET_TABLE_SIZE: usize = BLOCK_COUNT * OFFSET_SIZE;
pub const DATA_START: usize = HEADER_SIZE + OFFSET_TABLE_SIZE;
pub const ASSET_TABLE_BLOCK: usize = 0;
pub const VEGETATION_IMAGE_ASSET_TYPE_ID: Uuid = uuid!("E0F05299-DB68-4158-A207-1FD8E1ADC280");
pub const ASSET_DATA_TYPE_ID: Uuid = uuid!("AF3F7D32-1536-422A-89F3-A11E1F5B5A9C");
pub const LOCAL_POSITION_QUANTIZATION: f32 = 65_535.0;
pub const HEIGHT_QUANTIZATION: f32 = 31.999_512;
pub const SCALE_QUANTIZATION: f32 = 100.0;
pub const ROTATION_XY_QUANTIZATION: f32 = 255.0;
pub const ROTATION_XY_BIAS: f32 = 255.5;
pub const ROTATION_Z_QUANTIZATION: f32 = 511.0;
pub const ROTATION_Z_BIAS: f32 = 511.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VegetationImageFormat {
    U8,
}

impl VegetationImageFormat {
    #[inline]
    #[must_use]
    pub const fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::U8),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn raw(self) -> u32 {
        match self {
            Self::U8 => 0,
        }
    }

    #[inline]
    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::U8 => 1,
        }
    }

    #[inline]
    #[must_use]
    pub fn byte_len(self, width: u32, height: u32) -> Option<usize> {
        usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            .checked_mul(self.bytes_per_pixel())
    }
}

impl fmt::Display for VegetationImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::U8 => "U8",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VegetationImageAsset {
    width: u32,
    height: u32,
    format: VegetationImageFormat,
    data: Box<[u8]>,
}

impl VegetationImageAsset {
    #[inline]
    pub fn parse(bytes: &[u8]) -> Result<Self, ImageAssetParseError> {
        Self::parse_with_hashes(bytes, None)
    }

    pub fn parse_with_hashes(
        bytes: &[u8],
        hashes: Option<&NameLookup>,
    ) -> Result<Self, ImageAssetParseError> {
        let stream = ObjectStream::from_bytes(bytes, hashes)?;
        if stream.version() != 3 {
            return Err(ImageAssetParseError::UnsupportedVersion {
                actual: stream.version(),
            });
        }

        let mut roots = stream.elements.into_iter();
        let root = roots.next().ok_or(ImageAssetParseError::MissingRoot)?;
        if roots.next().is_some() {
            return Err(ImageAssetParseError::UnexpectedRootCount);
        }
        if *root.id() != VEGETATION_IMAGE_ASSET_TYPE_ID {
            return Err(ImageAssetParseError::UnexpectedRootType { actual: root.id });
        }

        let mut saw_base = false;
        let mut width = None;
        let mut height = None;
        let mut raw_format = None;
        let mut data = None;

        for mut child in root.elements {
            if field_is(&child, "BaseClass1") {
                expect_type(
                    &child,
                    "BaseClass1",
                    ASSET_DATA_TYPE_ID,
                    "AZ::Data::AssetData",
                )?;
                expect_leaf(&child, "BaseClass1")?;
                if saw_base {
                    return Err(ImageAssetParseError::DuplicateField("BaseClass1"));
                }
                saw_base = true;
            } else if field_is(&child, "Width") {
                expect_leaf(&child, "Width")?;
                assign_slot(&mut width, "Width", child.decode()?)?;
            } else if field_is(&child, "Height") {
                expect_leaf(&child, "Height")?;
                assign_slot(&mut height, "Height", child.decode()?)?;
            } else if field_is(&child, "Format") {
                expect_leaf(&child, "Format")?;
                assign_slot(&mut raw_format, "Format", child.decode()?)?;
            } else if field_is(&child, "Data") {
                expect_leaf(&child, "Data")?;
                let _: &[u8] = child.decode()?;
                let bytes = child
                    .data
                    .take()
                    .ok_or(ImageAssetParseError::MissingFieldValue("Data"))?;
                assign_slot(&mut data, "Data", bytes)?;
            } else {
                return Err(ImageAssetParseError::UnexpectedField {
                    field: child
                        .field()
                        .map(|field| field.to_string())
                        .unwrap_or_else(|| "<unnamed>".to_string()),
                });
            }
        }

        if !saw_base {
            return Err(ImageAssetParseError::MissingField("BaseClass1"));
        }
        let width = width.ok_or(ImageAssetParseError::MissingField("Width"))?;
        let height = height.ok_or(ImageAssetParseError::MissingField("Height"))?;
        let raw_format = raw_format.ok_or(ImageAssetParseError::MissingField("Format"))?;
        let format = VegetationImageFormat::from_raw(raw_format)
            .ok_or(ImageAssetParseError::UnsupportedFormat(raw_format))?;
        let data = data.ok_or(ImageAssetParseError::MissingField("Data"))?;
        let expected_len =
            format
                .byte_len(width, height)
                .ok_or(ImageAssetParseError::DimensionOverflow {
                    width,
                    height,
                    format,
                })?;
        if data.len() != expected_len {
            return Err(ImageAssetParseError::DataLength {
                width,
                height,
                format,
                expected: expected_len,
                actual: data.len(),
            });
        }

        Ok(Self {
            width,
            height,
            format,
            data: data.into_boxed_slice(),
        })
    }

    #[inline]
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    #[must_use]
    pub const fn format(&self) -> VegetationImageFormat {
        self.format
    }

    #[inline]
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[inline]
    #[must_use]
    pub fn into_data(self) -> Box<[u8]> {
        self.data
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VegetationImageAssetSummary {
    pub width: u32,
    pub height: u32,
    pub format: VegetationImageFormat,
    pub data_bytes: usize,
}

impl VegetationImageAssetSummary {
    #[inline]
    #[must_use]
    pub fn from_asset(asset: &VegetationImageAsset) -> Self {
        Self {
            width: asset.width(),
            height: asset.height(),
            format: asset.format(),
            data_bytes: asset.data().len(),
        }
    }
}

impl fmt::Display for VegetationImageAssetSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  width:      {}", self.width)?;
        writeln!(f, "  height:     {}", self.height)?;
        writeln!(f, "  format:     {} ({})", self.format.raw(), self.format)?;
        write!(f, "  data bytes: {}", self.data_bytes)
    }
}

pub fn summarize_vegetation_image_asset(
    bytes: &[u8],
) -> Result<VegetationImageAssetSummary, ImageAssetParseError> {
    VegetationImageAsset::parse(bytes).map(|asset| VegetationImageAssetSummary::from_asset(&asset))
}

/// Checks whether a filesystem path names a `VegetationImageAsset` ObjectStream payload.
pub fn is_vegetation_image_asset_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vegimage"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VegetationImage<'a> {
    bytes: &'a [u8],
    header: VegetationImageHeader,
    block_count: usize,
    data_start: usize,
}

impl<'a> VegetationImage<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ParseError> {
        if bytes.len() < HEADER_SIZE {
            return Err(ParseError::TooShort {
                needed: HEADER_SIZE,
                actual: bytes.len(),
            });
        }

        let header = VegetationImageHeader {
            tile_size: read_u16(bytes, 0)?,
            region_size: read_u16(bytes, 2)?,
            baked_data_cache_size: read_le_u32(bytes, 4)?,
        };
        let block_count = header
            .expected_block_count()
            .ok_or(ParseError::InvalidDimensions {
                tile_size: header.tile_size,
                region_size: header.region_size,
            })?;
        let offset_table_size =
            block_count
                .checked_mul(OFFSET_SIZE)
                .ok_or(ParseError::InvalidDimensions {
                    tile_size: header.tile_size,
                    region_size: header.region_size,
                })?;
        let data_start =
            HEADER_SIZE
                .checked_add(offset_table_size)
                .ok_or(ParseError::InvalidDimensions {
                    tile_size: header.tile_size,
                    region_size: header.region_size,
                })?;
        if bytes.len() < data_start {
            return Err(ParseError::TooShort {
                needed: data_start,
                actual: bytes.len(),
            });
        }

        let mut previous = data_start;
        for index in 0..block_count {
            let offset = read_offset(bytes, index)?;
            if offset < previous || offset > bytes.len() {
                return Err(ParseError::InvalidOffset {
                    index,
                    offset,
                    previous,
                    len: bytes.len(),
                });
            }
            previous = offset;
        }

        Ok(Self {
            bytes,
            header,
            block_count,
            data_start,
        })
    }

    #[inline]
    #[must_use]
    pub const fn header(&self) -> VegetationImageHeader {
        self.header
    }

    #[inline]
    #[must_use]
    pub const fn block_count(&self) -> usize {
        self.block_count
    }

    #[inline]
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[inline]
    pub fn block(&self, index: usize) -> Result<VegetationBlock<'a>, ParseError> {
        if index >= self.block_count {
            return Err(ParseError::BlockOutOfRange { index });
        }

        let start = if index == 0 {
            self.data_start
        } else {
            read_offset(self.bytes, index - 1)?
        };
        let end = read_offset(self.bytes, index)?;
        Ok(VegetationBlock {
            index,
            bytes: &self.bytes[start..end],
        })
    }

    #[inline]
    pub fn asset_table(&self) -> Result<AssetTable<'a>, ParseError> {
        AssetTable::parse(self.block(ASSET_TABLE_BLOCK)?)
    }

    #[inline]
    pub fn blocks(&self) -> Blocks<'_, 'a> {
        Blocks {
            image: self,
            next: 0,
        }
    }

    #[inline]
    pub fn summary(&self) -> Result<VegetationRegionSummary, ParseError> {
        VegetationRegionSummary::from_image(self)
    }

    #[inline]
    pub fn inspection_summary(&self) -> Result<VegetationImageInspectionSummary, ParseError> {
        VegetationImageInspectionSummary::from_image(self)
    }

    pub fn inspection_report(
        &self,
        include_assets: bool,
        instance_limit: usize,
    ) -> Result<VegetationImageInspectionReport, ParseError> {
        VegetationImageInspectionReport::from_image(self, include_assets, instance_limit)
    }

    pub fn instance_samples(
        &self,
        limit: usize,
    ) -> Result<Vec<VegetationInstanceSample>, ParseError> {
        let mut samples = Vec::new();
        if limit == 0 {
            return Ok(samples);
        }

        let region_size = self.header.region_size as f32;
        for block in self.blocks() {
            let block = block?;
            let BlockKind::Cell(cell) = block.kind()? else {
                continue;
            };
            for group in cell.groups() {
                let group = group?;
                for (instance_index, instance) in group.instances().enumerate() {
                    samples.push(VegetationInstanceSample::from_placement(
                        block.index,
                        group.asset_index,
                        instance_index,
                        instance.placement(),
                        region_size,
                    ));
                    if samples.len() == limit {
                        return Ok(samples);
                    }
                }
            }
        }
        Ok(samples)
    }

    #[inline]
    #[must_use]
    pub fn tail(&self) -> &'a [u8] {
        let start = read_offset(self.bytes, self.block_count - 1).unwrap_or(self.bytes.len());
        &self.bytes[start..]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VegetationImageHeader {
    pub tile_size: u16,
    pub region_size: u16,
    pub baked_data_cache_size: u32,
}

impl VegetationImageHeader {
    #[must_use]
    pub const fn new(tile_size: u16, region_size: u16, baked_data_cache_size: u32) -> Self {
        Self {
            tile_size,
            region_size,
            baked_data_cache_size,
        }
    }

    #[must_use]
    pub const fn expected_block_count(self) -> Option<usize> {
        if self.tile_size == 0
            || self.region_size == 0
            || !self.region_size.is_multiple_of(self.tile_size)
        {
            return None;
        }
        let width = (self.region_size / self.tile_size) as usize;
        width.checked_mul(width)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VegetationRegionSummary {
    pub asset_entries: usize,
    pub empty_blocks: usize,
    pub cell_blocks: usize,
    pub cell_groups: usize,
    pub instances: usize,
}

impl VegetationRegionSummary {
    pub fn from_image(image: &VegetationImage<'_>) -> Result<Self, ParseError> {
        let mut summary = Self {
            asset_entries: image.asset_table()?.count() as usize,
            ..Self::default()
        };

        for block in image.blocks() {
            match block?.kind()? {
                BlockKind::Empty => summary.empty_blocks += 1,
                BlockKind::AssetTable(_) => {}
                BlockKind::Cell(cell) => {
                    cell.validate()?;
                    summary.cell_blocks += 1;
                    for group in cell.groups() {
                        let group = group?;
                        summary.cell_groups += 1;
                        summary.instances += usize::from(group.instance_count());
                    }
                }
            }
        }

        Ok(summary)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VegetationImageInspectionSummary {
    pub tile_size: u16,
    pub region_size: u16,
    pub baked_data_cache_size: u32,
    pub blocks: usize,
    pub empty_blocks: usize,
    pub cell_blocks: usize,
    pub cell_groups: usize,
    pub instances: usize,
    pub asset_entries: usize,
    pub tail_bytes: usize,
}

impl VegetationImageInspectionSummary {
    pub fn from_image(image: &VegetationImage<'_>) -> Result<Self, ParseError> {
        let header = image.header();
        let summary = image.summary()?;
        Ok(Self {
            tile_size: header.tile_size,
            region_size: header.region_size,
            baked_data_cache_size: header.baked_data_cache_size,
            blocks: header.expected_block_count().unwrap_or(image.block_count()),
            empty_blocks: summary.empty_blocks,
            cell_blocks: summary.cell_blocks,
            cell_groups: summary.cell_groups,
            instances: summary.instances,
            asset_entries: summary.asset_entries,
            tail_bytes: image.tail().len(),
        })
    }
}

impl fmt::Display for VegetationImageInspectionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  tile size:     {}", self.tile_size)?;
        writeln!(f, "  region size:   {}", self.region_size)?;
        writeln!(f, "  cache size:    {}", self.baked_data_cache_size)?;
        writeln!(f, "  blocks:        {}", self.blocks)?;
        writeln!(f, "  empty blocks:  {}", self.empty_blocks)?;
        writeln!(f, "  cell blocks:   {}", self.cell_blocks)?;
        writeln!(f, "  cell groups:   {}", self.cell_groups)?;
        writeln!(f, "  instances:     {}", self.instances)?;
        writeln!(f, "  asset entries: {}", self.asset_entries)?;
        write!(f, "  tail bytes:    {}", self.tail_bytes)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VegetationImageInspectionReport {
    pub summary: VegetationImageInspectionSummary,
    pub assets: Vec<VegetationAssetEntrySummary>,
    pub instances: Vec<VegetationInstanceSample>,
}

impl VegetationImageInspectionReport {
    pub fn from_image(
        image: &VegetationImage<'_>,
        include_assets: bool,
        instance_limit: usize,
    ) -> Result<Self, ParseError> {
        let summary = image.inspection_summary()?;
        let assets = if include_assets {
            image
                .asset_table()?
                .entries()
                .enumerate()
                .map(|(index, entry)| {
                    entry.map(|entry| VegetationAssetEntrySummary::new(index, entry))
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        let instances = image.instance_samples(instance_limit)?;
        Ok(Self {
            summary,
            assets,
            instances,
        })
    }
}

impl fmt::Display for VegetationImageInspectionReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.summary)?;
        for asset in &self.assets {
            writeln!(f, "{asset}")?;
        }
        for sample in &self.instances {
            writeln!(f, "  {sample}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VegetationFileInspectionReport<'a> {
    ImageAsset {
        path: &'a Path,
        summary: VegetationImageAssetSummary,
    },
    Region {
        path: &'a Path,
        report: VegetationImageInspectionReport,
    },
}

impl fmt::Display for VegetationFileInspectionReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImageAsset { path, summary } => {
                writeln!(f, "{}", path.display())?;
                write!(f, "{summary}")
            }
            Self::Region { path, report } => {
                writeln!(f, "{}", path.display())?;
                write!(f, "{report}")
            }
        }
    }
}

pub fn inspect_vegetation_file<'a>(
    path: &'a Path,
    bytes: &[u8],
    include_assets: bool,
    instance_limit: usize,
) -> Result<VegetationFileInspectionReport<'a>, VegetationFileInspectionError> {
    if is_vegetation_image_asset_path(path) {
        let summary = summarize_vegetation_image_asset(bytes)?;
        return Ok(VegetationFileInspectionReport::ImageAsset { path, summary });
    }

    let image = VegetationImage::parse(bytes)?;
    let report = image.inspection_report(include_assets, instance_limit)?;
    Ok(VegetationFileInspectionReport::Region { path, report })
}

#[derive(Debug, Error)]
pub enum VegetationInspectionError {
    #[error("read vegetation asset {path:?}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("inspect vegetation asset {path:?}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: VegetationFileInspectionError,
    },
}

pub fn inspect_vegetation_path<'a>(
    path: &'a Path,
    include_assets: bool,
    instance_limit: usize,
) -> Result<VegetationFileInspectionReport<'a>, VegetationInspectionError> {
    let bytes = std::fs::read(path).map_err(|source| VegetationInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    inspect_vegetation_file(path, &bytes, include_assets, instance_limit).map_err(|source| {
        VegetationInspectionError::Inspect {
            path: path.to_path_buf(),
            source,
        }
    })
}

pub fn summarize_vegetation_image(bytes: &[u8]) -> Result<VegetationRegionSummary, ParseError> {
    let image = VegetationImage::parse(bytes)?;
    image.summary()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VegetationInstanceSample {
    pub block: usize,
    pub asset_index: u16,
    pub instance_index: usize,
    pub local: [f32; 3],
    pub scale: f32,
    pub rotation_x: u16,
    pub rotation_y: u16,
    pub rotation_z: u16,
    pub rotation_w_is_negative: bool,
    pub source_index: u16,
    pub origin_x_high_byte: u8,
    pub origin_y_high_half: u16,
}

impl VegetationInstanceSample {
    #[inline]
    #[must_use]
    pub fn from_placement(
        block: usize,
        asset_index: u16,
        instance_index: usize,
        placement: CellInstancePlacement,
        region_size: f32,
    ) -> Self {
        let local = placement.local_offset(region_size);
        let rotation = placement.rotation;
        Self {
            block,
            asset_index,
            instance_index,
            local: [local.x, local.y, local.z],
            scale: placement.scale(),
            rotation_x: rotation.x_code(),
            rotation_y: rotation.y_code(),
            rotation_z: rotation.z_code(),
            rotation_w_is_negative: rotation.w_is_negative(),
            source_index: placement.source_index,
            origin_x_high_byte: placement.origin_x_high_byte,
            origin_y_high_half: placement.origin_y_high_half,
        }
    }
}

impl fmt::Display for VegetationInstanceSample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "instance block={} asset={} index={}: local=({:.3},{:.3},{:.3}) scale={:.2} rot=({}, {}, {}, sign={}) source={} origin_hi=({:#04x},{:#06x})",
            self.block,
            self.asset_index,
            self.instance_index,
            self.local[0],
            self.local[1],
            self.local[2],
            self.scale,
            self.rotation_x,
            self.rotation_y,
            self.rotation_z,
            if self.rotation_w_is_negative {
                "-"
            } else {
                "+"
            },
            self.source_index,
            self.origin_x_high_byte,
            self.origin_y_high_half,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VegetationBlock<'a> {
    pub index: usize,
    bytes: &'a [u8],
}

impl<'a> VegetationBlock<'a> {
    #[inline]
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    #[inline]
    pub fn item_count(self) -> Result<u16, ParseError> {
        if self.bytes.len() < 2 {
            return Err(ParseError::BlockTooShort {
                index: self.index,
                len: self.bytes.len(),
            });
        }
        Ok(u16::from_le_bytes([self.bytes[0], self.bytes[1]]))
    }

    #[inline]
    pub fn kind(self) -> Result<BlockKind<'a>, ParseError> {
        if self.index == ASSET_TABLE_BLOCK && self.bytes == [0] {
            return Ok(BlockKind::Empty);
        }
        if self.item_count()? == 0 && self.bytes.len() == 2 {
            return Ok(BlockKind::Empty);
        }
        if self.index == ASSET_TABLE_BLOCK {
            return AssetTable::parse(self).map(BlockKind::AssetTable);
        }
        Ok(BlockKind::Cell(CellBlock {
            index: self.index,
            item_count: self.item_count()?,
            payload: &self.bytes[2..],
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind<'a> {
    Empty,
    AssetTable(AssetTable<'a>),
    Cell(CellBlock<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellBlock<'a> {
    pub index: usize,
    pub item_count: u16,
    payload: &'a [u8],
}

impl<'a> CellBlock<'a> {
    #[inline]
    #[must_use]
    pub const fn payload(self) -> &'a [u8] {
        self.payload
    }

    #[inline]
    pub fn groups(self) -> CellGroups<'a> {
        CellGroups {
            remaining: self.item_count,
            bytes: self.payload,
            position: 0,
        }
    }

    pub fn validate(self) -> Result<(), ParseError> {
        let mut groups = self.groups();
        for group in groups.by_ref() {
            group?;
        }
        if groups.position != groups.bytes.len() {
            return Err(ParseError::TrailingCellBytes {
                block: self.index,
                trailing: groups.bytes.len() - groups.position,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellGroup<'a> {
    pub asset_index: u16,
    instance_count: u16,
    instances: &'a [u8],
}

impl<'a> CellGroup<'a> {
    #[inline]
    #[must_use]
    pub const fn instance_count(self) -> u16 {
        self.instance_count
    }

    #[inline]
    pub fn instances(self) -> CellInstances<'a> {
        CellInstances {
            remaining: self.instance_count,
            bytes: self.instances,
            position: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellGroups<'a> {
    remaining: u16,
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Iterator for CellGroups<'a> {
    type Item = Result<CellGroup<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        let start = self.position;
        let header_end = start + CELL_GROUP_HEADER_SIZE;
        if header_end > self.bytes.len() {
            self.position = self.bytes.len();
            return Some(Err(ParseError::CellGroupTooShort {
                needed: header_end,
                actual: self.bytes.len(),
            }));
        }

        let asset_index = u16::from_le_bytes([self.bytes[start], self.bytes[start + 1]]);
        let instance_count = u16::from_le_bytes([self.bytes[start + 2], self.bytes[start + 3]]);
        let instances_start = header_end;
        let instances_len = usize::from(instance_count) * CELL_INSTANCE_SIZE;
        let instances_end = instances_start + instances_len;
        if instances_end > self.bytes.len() {
            self.position = self.bytes.len();
            return Some(Err(ParseError::CellInstancesTooShort {
                needed: instances_end,
                actual: self.bytes.len(),
            }));
        }

        self.position = instances_end;
        Some(Ok(CellGroup {
            asset_index,
            instance_count,
            instances: &self.bytes[instances_start..instances_end],
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellInstance<'a> {
    bytes: &'a [u8; CELL_INSTANCE_SIZE],
}

impl<'a> CellInstance<'a> {
    #[inline]
    #[must_use]
    pub const fn bytes(self) -> &'a [u8; CELL_INSTANCE_SIZE] {
        self.bytes
    }

    #[inline]
    #[must_use]
    pub const fn placement(self) -> CellInstancePlacement {
        CellInstancePlacement::from_bytes(*self.bytes)
    }

    #[inline]
    #[must_use]
    pub const fn rotation(self) -> PackedVegetationRotation {
        self.placement().rotation
    }

    #[inline]
    #[must_use]
    pub const fn local_x_code(self) -> u16 {
        self.placement().local_x_code
    }

    #[inline]
    #[must_use]
    pub const fn local_y_code(self) -> u16 {
        self.placement().local_y_code
    }

    #[inline]
    #[must_use]
    pub const fn height_code(self) -> u16 {
        self.placement().height_code
    }

    #[inline]
    #[must_use]
    pub const fn scale_code(self) -> u8 {
        self.placement().scale_code
    }

    #[inline]
    #[must_use]
    pub const fn source_index(self) -> u16 {
        self.placement().source_index
    }

    #[inline]
    #[must_use]
    pub const fn packed_words(self) -> [u32; 4] {
        [
            u32::from_le_bytes([self.bytes[0], self.bytes[1], self.bytes[2], self.bytes[3]]),
            u32::from_le_bytes([self.bytes[4], self.bytes[5], self.bytes[6], self.bytes[7]]),
            u32::from_le_bytes([self.bytes[8], self.bytes[9], self.bytes[10], self.bytes[11]]),
            u32::from_le_bytes([
                self.bytes[12],
                self.bytes[13],
                self.bytes[14],
                self.bytes[15],
            ]),
        ]
    }

    #[inline]
    #[must_use]
    pub const fn packed_halves(self) -> [u16; 8] {
        [
            u16::from_le_bytes([self.bytes[0], self.bytes[1]]),
            u16::from_le_bytes([self.bytes[2], self.bytes[3]]),
            u16::from_le_bytes([self.bytes[4], self.bytes[5]]),
            u16::from_le_bytes([self.bytes[6], self.bytes[7]]),
            u16::from_le_bytes([self.bytes[8], self.bytes[9]]),
            u16::from_le_bytes([self.bytes[10], self.bytes[11]]),
            u16::from_le_bytes([self.bytes[12], self.bytes[13]]),
            u16::from_le_bytes([self.bytes[14], self.bytes[15]]),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellInstancePlacement {
    pub rotation: PackedVegetationRotation,
    pub local_x_code: u16,
    pub local_y_code: u16,
    pub height_code: u16,
    pub scale_code: u8,
    pub origin_x_high_byte: u8,
    pub source_index: u16,
    pub origin_y_high_half: u16,
}

impl CellInstancePlacement {
    #[inline]
    #[must_use]
    pub const fn from_bytes(bytes: [u8; CELL_INSTANCE_SIZE]) -> Self {
        Self {
            rotation: PackedVegetationRotation::new(u32::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
            ])),
            local_x_code: u16::from_le_bytes([bytes[4], bytes[5]]),
            local_y_code: u16::from_le_bytes([bytes[6], bytes[7]]),
            height_code: u16::from_le_bytes([bytes[8], bytes[9]]),
            scale_code: bytes[10],
            origin_x_high_byte: bytes[11],
            source_index: u16::from_le_bytes([bytes[12], bytes[13]]),
            origin_y_high_half: u16::from_le_bytes([bytes[14], bytes[15]]),
        }
    }

    #[inline]
    #[must_use]
    pub fn local_offset(self, region_size: f32) -> Vec3 {
        Vec3::new(
            self.local_x_code as f32 * region_size / LOCAL_POSITION_QUANTIZATION,
            self.local_y_code as f32 * region_size / LOCAL_POSITION_QUANTIZATION,
            self.height(),
        )
    }

    #[inline]
    #[must_use]
    pub fn position(self, region_origin: Vec2, region_size: f32) -> Vec3 {
        let local = self.local_offset(region_size);
        Vec3::new(
            region_origin.x + local.x,
            region_origin.y + local.y,
            local.z,
        )
    }

    #[inline]
    #[must_use]
    pub fn height(self) -> f32 {
        self.height_code as f32 / HEIGHT_QUANTIZATION
    }

    #[inline]
    #[must_use]
    pub fn scale(self) -> f32 {
        self.scale_code as f32 / SCALE_QUANTIZATION
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PackedVegetationRotation(u32);

impl PackedVegetationRotation {
    #[inline]
    #[must_use]
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    #[inline]
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn x_code(self) -> u16 {
        ((self.0 >> 21) & 0x03ff) as u16
    }

    #[inline]
    #[must_use]
    pub const fn y_code(self) -> u16 {
        ((self.0 >> 11) & 0x03ff) as u16
    }

    #[inline]
    #[must_use]
    pub const fn z_code(self) -> u16 {
        (self.0 & 0x03ff) as u16
    }

    #[inline]
    #[must_use]
    pub const fn w_is_negative(self) -> bool {
        self.0 & 0x8000_0000 != 0
    }

    #[inline]
    #[must_use]
    pub fn xyz(self) -> Vec3 {
        Vec3::new(
            (self.x_code() as f32 - ROTATION_XY_BIAS) / ROTATION_XY_QUANTIZATION,
            (self.y_code() as f32 - ROTATION_XY_BIAS) / ROTATION_XY_QUANTIZATION,
            (self.z_code() as f32 - ROTATION_Z_BIAS) / ROTATION_Z_QUANTIZATION,
        )
    }

    #[inline]
    #[must_use]
    pub fn quat(self) -> Quat {
        let xyz = self.xyz();
        let w_abs = (1.0 - xyz.length_squared()).max(0.0).sqrt();
        let w = if self.w_is_negative() { -w_abs } else { w_abs };
        Quat::from_xyzw(xyz.x, xyz.y, xyz.z, w).normalize()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellInstances<'a> {
    remaining: u16,
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Iterator for CellInstances<'a> {
    type Item = CellInstance<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        let start = self.position;
        let end = start + CELL_INSTANCE_SIZE;
        self.position = end;
        Some(CellInstance {
            bytes: self.bytes[start..end].try_into().expect("slice size"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetTable<'a> {
    count: u16,
    entries: &'a [u8],
}

impl<'a> AssetTable<'a> {
    pub fn parse(block: VegetationBlock<'a>) -> Result<Self, ParseError> {
        let (count, bytes) = if block.index == ASSET_TABLE_BLOCK && block.bytes == [0] {
            (0, &[][..])
        } else {
            (block.item_count()?, &block.bytes[2..])
        };
        let mut entries = AssetEntries {
            remaining: count,
            bytes,
            position: 0,
        };
        for entry in entries.by_ref() {
            entry?;
        }
        if entries.position != entries.bytes.len() {
            return Err(ParseError::TrailingAssetTableBytes {
                trailing: entries.bytes.len() - entries.position,
            });
        }
        Ok(Self {
            count,
            entries: bytes,
        })
    }

    #[inline]
    #[must_use]
    pub const fn count(self) -> u16 {
        self.count
    }

    #[inline]
    pub fn entries(self) -> AssetEntries<'a> {
        AssetEntries {
            remaining: self.count,
            bytes: self.entries,
            position: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetEntry<'a> {
    pub id: AssetId,
    pub flags: u8,
    pub kind: u8,
    pub path: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VegetationAssetEntrySummary {
    pub index: usize,
    pub id: AssetId,
    pub flags: u8,
    pub kind: u8,
    pub path: String,
}

impl VegetationAssetEntrySummary {
    #[must_use]
    pub fn new(index: usize, entry: AssetEntry<'_>) -> Self {
        Self {
            index,
            id: entry.id,
            flags: entry.flags,
            kind: entry.kind,
            path: entry.path.to_string(),
        }
    }
}

impl fmt::Display for VegetationAssetEntrySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "  asset[{}]: {:?} flags={} kind={} path={}",
            self.index, self.id, self.flags, self.kind, self.path
        )
    }
}

impl fmt::Display for AssetEntry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} flags={} kind={} path={}",
            self.id, self.flags, self.kind, self.path
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetEntries<'a> {
    remaining: u16,
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Iterator for AssetEntries<'a> {
    type Item = Result<AssetEntry<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        let start = self.position;
        let fixed_end = start + ASSET_ENTRY_FIXED_SIZE;
        if fixed_end > self.bytes.len() {
            self.position = self.bytes.len();
            return Some(Err(ParseError::AssetEntryTooShort {
                needed: fixed_end,
                actual: self.bytes.len(),
            }));
        }

        let guid = Uuid::from_bytes(
            self.bytes[start..start + 16]
                .try_into()
                .expect("slice size"),
        );
        let sub_id = u32::from_le_bytes(
            self.bytes[start + 16..start + 20]
                .try_into()
                .expect("slice size"),
        );
        let flags = self.bytes[start + 20];
        let kind = self.bytes[start + 21];
        let path_len = self.bytes[start + 22] as usize;
        let path_start = fixed_end;
        let path_end = path_start + path_len;
        if path_end > self.bytes.len() {
            self.position = self.bytes.len();
            return Some(Err(ParseError::AssetPathTooShort {
                needed: path_end,
                actual: self.bytes.len(),
            }));
        }

        let path = match std::str::from_utf8(&self.bytes[path_start..path_end]) {
            Ok(path) => path,
            Err(source) => {
                self.position = path_end;
                return Some(Err(ParseError::InvalidAssetPath { source }));
            }
        };
        self.position = path_end;

        Some(Ok(AssetEntry {
            id: AssetId::new(guid, sub_id),
            flags,
            kind,
            path,
        }))
    }
}

const ASSET_ENTRY_FIXED_SIZE: usize = 23;
const CELL_GROUP_HEADER_SIZE: usize = 4;
pub const CELL_INSTANCE_SIZE: usize = 16;

#[derive(Debug, Clone, Copy)]
pub struct Blocks<'image, 'a> {
    image: &'image VegetationImage<'a>,
    next: usize,
}

impl<'a> Iterator for Blocks<'_, 'a> {
    type Item = Result<VegetationBlock<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.image.block_count {
            return None;
        }
        let index = self.next;
        self.next += 1;
        Some(self.image.block(index))
    }
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("vegetation image is too short: need at least {needed} bytes, got {actual}")]
    TooShort { needed: usize, actual: usize },

    #[error(
        "vegetation image dimensions are invalid: region size {region_size}, tile size {tile_size}"
    )]
    InvalidDimensions { tile_size: u16, region_size: u16 },

    #[error(
        "vegetation image offset {index} is invalid: offset {offset}, previous {previous}, len {len}"
    )]
    InvalidOffset {
        index: usize,
        offset: usize,
        previous: usize,
        len: usize,
    },

    #[error("vegetation block index {index} is out of range")]
    BlockOutOfRange { index: usize },

    #[error("vegetation block {index} is too short: {len} bytes")]
    BlockTooShort { index: usize, len: usize },

    #[error("asset table entry is too short: need {needed} bytes, got {actual}")]
    AssetEntryTooShort { needed: usize, actual: usize },

    #[error("asset table path is too short: need {needed} bytes, got {actual}")]
    AssetPathTooShort { needed: usize, actual: usize },

    #[error("asset table path is not UTF-8")]
    InvalidAssetPath { source: std::str::Utf8Error },

    #[error("asset table has {trailing} trailing byte(s)")]
    TrailingAssetTableBytes { trailing: usize },

    #[error("cell group is too short: need {needed} bytes, got {actual}")]
    CellGroupTooShort { needed: usize, actual: usize },

    #[error("cell instances are too short: need {needed} bytes, got {actual}")]
    CellInstancesTooShort { needed: usize, actual: usize },

    #[error("cell block {block} has {trailing} trailing byte(s)")]
    TrailingCellBytes { block: usize, trailing: usize },
}

#[derive(Debug, Error)]
pub enum VegetationFileInspectionError {
    #[error("parse VegetationImageAsset: {0}")]
    ImageAsset(#[from] ImageAssetParseError),

    #[error("parse vegetation region image: {0}")]
    Region(#[from] ParseError),
}

#[derive(Debug, Error)]
pub enum ImageAssetParseError {
    #[error("ObjectStream error: {0}")]
    ObjectStream(#[from] ObjectStreamError),

    #[error("ObjectStream value error: {0}")]
    Value(#[from] ObjectStreamValueError),

    #[error("unsupported ObjectStream version {actual}")]
    UnsupportedVersion { actual: u32 },

    #[error("VegetationImageAsset has no root element")]
    MissingRoot,

    #[error("VegetationImageAsset stream has more than one root element")]
    UnexpectedRootCount,

    #[error(
        "VegetationImageAsset root has type {actual}, expected E0F05299-DB68-4158-A207-1FD8E1ADC280"
    )]
    UnexpectedRootType { actual: Uuid },

    #[error("field `{field}` has type {actual}, expected {expected_name} ({expected})")]
    UnexpectedFieldType {
        field: &'static str,
        expected_name: &'static str,
        expected: Uuid,
        actual: Uuid,
    },

    #[error("field `{field}` has {children} nested element(s), expected none")]
    NestedField {
        field: &'static str,
        children: usize,
    },

    #[error("unexpected VegetationImageAsset field `{field}`")]
    UnexpectedField { field: String },

    #[error("missing VegetationImageAsset field `{0}`")]
    MissingField(&'static str),

    #[error("duplicate VegetationImageAsset field `{0}`")]
    DuplicateField(&'static str),

    #[error("field `{0}` has no value bytes")]
    MissingFieldValue(&'static str),

    #[error("unsupported VegetationImageAsset format {0}")]
    UnsupportedFormat(u32),

    #[error("VegetationImageAsset dimensions {width}x{height} overflow for {format}")]
    DimensionOverflow {
        width: u32,
        height: u32,
        format: VegetationImageFormat,
    },

    #[error(
        "VegetationImageAsset data has {actual} bytes, expected {expected} for {width}x{height} {format}"
    )]
    DataLength {
        width: u32,
        height: u32,
        format: VegetationImageFormat,
        expected: usize,
        actual: usize,
    },
}

fn field_is(element: &Element, field: &str) -> bool {
    element
        .field()
        .is_some_and(|actual| actual.as_str() == field)
}

fn expect_type(
    element: &Element,
    field: &'static str,
    expected: Uuid,
    expected_name: &'static str,
) -> Result<(), ImageAssetParseError> {
    if *element.id() == expected {
        Ok(())
    } else {
        Err(ImageAssetParseError::UnexpectedFieldType {
            field,
            expected_name,
            expected,
            actual: *element.id(),
        })
    }
}

fn expect_leaf(element: &Element, field: &'static str) -> Result<(), ImageAssetParseError> {
    if element.children().is_empty() {
        Ok(())
    } else {
        Err(ImageAssetParseError::NestedField {
            field,
            children: element.children().len(),
        })
    }
}

fn assign_slot<T>(
    slot: &mut Option<T>,
    field: &'static str,
    value: T,
) -> Result<(), ImageAssetParseError> {
    if slot.replace(value).is_some() {
        Err(ImageAssetParseError::DuplicateField(field))
    } else {
        Ok(())
    }
}

fn read_offset(bytes: &[u8], index: usize) -> Result<usize, ParseError> {
    let start = HEADER_SIZE + index * OFFSET_SIZE;
    let value = u32::from_le_bytes(
        bytes[start..start + OFFSET_SIZE]
            .try_into()
            .expect("slice size"),
    );
    Ok(value as usize)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ParseError> {
    let end = offset + 2;
    if bytes.len() < end {
        return Err(ParseError::TooShort {
            needed: end,
            actual: bytes.len(),
        });
    }
    Ok(u16::from_le_bytes(
        bytes[offset..end].try_into().expect("slice size"),
    ))
}

fn read_le_u32(bytes: &[u8], offset: usize) -> Result<u32, ParseError> {
    let end = offset + 4;
    if bytes.len() < end {
        return Err(ParseError::TooShort {
            needed: end,
            actual: bytes.len(),
        });
    }
    Ok(u32::from_le_bytes(
        bytes[offset..end].try_into().expect("slice size"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header_offsets_and_asset_table() {
        let mut table = Vec::new();
        table.extend_from_slice(&1u16.to_le_bytes());
        let guid = Uuid::from_u128(0x1122_3344_5566_7788_99aa_bbcc_ddee_ff00);
        table.extend_from_slice(guid.as_bytes());
        table.extend_from_slice(&7u32.to_le_bytes());
        table.push(1);
        table.push(10);
        table.push(11);
        table.extend_from_slice(b"objects/foo");

        let bytes = image_bytes(table, &[0, 0]);
        let image = VegetationImage::parse(&bytes).unwrap();

        assert_eq!(image.header(), VegetationImageHeader::new(16, 2048, 1024));
        assert_eq!(image.header().expected_block_count(), Some(BLOCK_COUNT));
        assert_eq!(image.tail(), &[0, 0]);

        let assets = image.asset_table().unwrap();
        assert_eq!(assets.count(), 1);
        let entry = assets.entries().next().unwrap().unwrap();
        assert_eq!(entry.id, AssetId::new(guid, 7));
        assert_eq!(entry.flags, 1);
        assert_eq!(entry.kind, 10);
        assert_eq!(entry.path, "objects/foo");

        let report = image.inspection_report(true, 0).unwrap();
        assert_eq!(report.assets.len(), 1);
        assert!(report.to_string().contains("asset[0]"));
        assert!(report.to_string().contains("objects/foo"));
    }

    #[test]
    fn parses_objectstream_vegetation_image_asset() {
        let asset = VegetationImageAsset::parse(
            br#"<ObjectStream version="3"><Class name="VegetationImageAsset" type="{E0F05299-DB68-4158-A207-1FD8E1ADC280}"><Class name="AssetData" field="BaseClass1" version="1" type="{AF3F7D32-1536-422A-89F3-A11E1F5B5A9C}"/><Class name="unsigned int" field="Width" value="2" type="{43DA906B-7DEF-4CA8-9790-854106D3F983}"/><Class name="unsigned int" field="Height" value="2" type="{43DA906B-7DEF-4CA8-9790-854106D3F983}"/><Class name="unsigned int" field="Format" value="0" type="{43DA906B-7DEF-4CA8-9790-854106D3F983}"/><Class name="ByteStream" field="Data" value="000102FF" type="{ADFD596B-7177-5519-9752-BC418FE42963}"/></Class></ObjectStream>"#,
        )
        .unwrap();

        assert_eq!(asset.width(), 2);
        assert_eq!(asset.height(), 2);
        assert_eq!(asset.format(), VegetationImageFormat::U8);
        assert_eq!(asset.data(), &[0, 1, 2, 255]);
        assert_eq!(
            VegetationImageAssetSummary::from_asset(&asset),
            VegetationImageAssetSummary {
                width: 2,
                height: 2,
                format: VegetationImageFormat::U8,
                data_bytes: 4,
            }
        );
        assert_eq!(
            VegetationImageAssetSummary::from_asset(&asset).to_string(),
            "  width:      2\n  height:     2\n  format:     0 (U8)\n  data bytes: 4"
        );
    }

    #[test]
    fn detects_vegetation_image_asset_paths() {
        assert!(is_vegetation_image_asset_path(Path::new(
            "textures/foo.vegimage"
        )));
        assert!(is_vegetation_image_asset_path(Path::new(
            "textures/foo.VEGIMAGE"
        )));
        assert!(!is_vegetation_image_asset_path(Path::new(
            "region.vegetation"
        )));
    }

    #[test]
    fn inspects_vegetation_files_by_path_kind() {
        let asset_report = inspect_vegetation_file(
            Path::new("textures/foo.vegimage"),
            br#"<ObjectStream version="3"><Class name="VegetationImageAsset" type="{E0F05299-DB68-4158-A207-1FD8E1ADC280}"><Class name="AssetData" field="BaseClass1" version="1" type="{AF3F7D32-1536-422A-89F3-A11E1F5B5A9C}"/><Class name="unsigned int" field="Width" value="2" type="{43DA906B-7DEF-4CA8-9790-854106D3F983}"/><Class name="unsigned int" field="Height" value="2" type="{43DA906B-7DEF-4CA8-9790-854106D3F983}"/><Class name="unsigned int" field="Format" value="0" type="{43DA906B-7DEF-4CA8-9790-854106D3F983}"/><Class name="ByteStream" field="Data" value="000102FF" type="{ADFD596B-7177-5519-9752-BC418FE42963}"/></Class></ObjectStream>"#,
            false,
            0,
        )
        .unwrap()
        .to_string();

        assert_eq!(
            asset_report,
            "textures/foo.vegimage\n  width:      2\n  height:     2\n  format:     0 (U8)\n  data bytes: 4"
        );

        let region_bytes = image_bytes(vec![0, 0], &[0, 0]);
        let region_report = inspect_vegetation_file(
            Path::new("levels/a/region.vegetation"),
            &region_bytes,
            false,
            0,
        )
        .unwrap()
        .to_string();

        assert!(region_report.starts_with("levels/a/region.vegetation\n  tile size:"));
        assert!(region_report.contains("asset entries: 0"));
    }

    #[test]
    fn formats_instance_sample_for_inspection() {
        let sample = VegetationInstanceSample {
            block: 10,
            asset_index: 2,
            instance_index: 3,
            local: [1.0, 2.5, 3.25],
            scale: 1.5,
            rotation_x: 11,
            rotation_y: 22,
            rotation_z: 33,
            rotation_w_is_negative: false,
            source_index: 4,
            origin_x_high_byte: 0xab,
            origin_y_high_half: 0xcdef,
        };

        assert_eq!(
            sample.to_string(),
            "instance block=10 asset=2 index=3: local=(1.000,2.500,3.250) scale=1.50 rot=(11, 22, 33, sign=+) source=4 origin_hi=(0xab,0xcdef)"
        );
    }

    #[test]
    fn rejects_objectstream_vegetation_image_data_length_mismatch() {
        let err = VegetationImageAsset::parse(
            br#"<ObjectStream version="3"><Class name="VegetationImageAsset" type="{E0F05299-DB68-4158-A207-1FD8E1ADC280}"><Class name="AssetData" field="BaseClass1" version="1" type="{AF3F7D32-1536-422A-89F3-A11E1F5B5A9C}"/><Class name="unsigned int" field="Width" value="2" type="{43DA906B-7DEF-4CA8-9790-854106D3F983}"/><Class name="unsigned int" field="Height" value="2" type="{43DA906B-7DEF-4CA8-9790-854106D3F983}"/><Class name="unsigned int" field="Format" value="0" type="{43DA906B-7DEF-4CA8-9790-854106D3F983}"/><Class name="ByteStream" field="Data" value="000102" type="{ADFD596B-7177-5519-9752-BC418FE42963}"/></Class></ObjectStream>"#,
        )
        .unwrap_err();

        assert!(matches!(
            err,
            ImageAssetParseError::DataLength {
                expected: 4,
                actual: 3,
                ..
            }
        ));
    }

    #[test]
    fn reports_non_monotonic_offsets() {
        let mut bytes = image_bytes(vec![0, 0], &[]);
        let bad = (DATA_START as u32 - 1).to_le_bytes();
        bytes[HEADER_SIZE..HEADER_SIZE + OFFSET_SIZE].copy_from_slice(&bad);

        let err = VegetationImage::parse(&bytes).unwrap_err();
        assert!(matches!(err, ParseError::InvalidOffset { index: 0, .. }));
    }

    #[test]
    fn classifies_empty_and_cell_blocks() {
        let bytes = image_bytes(vec![0, 0], &[0, 0]);
        let image = VegetationImage::parse(&bytes).unwrap();

        assert!(matches!(
            image.block(0).unwrap().kind().unwrap(),
            BlockKind::Empty
        ));

        let cell = VegetationBlock {
            index: 1,
            bytes: &[1, 0, 0xaa, 0xbb],
        }
        .kind()
        .unwrap();
        assert_eq!(
            cell,
            BlockKind::Cell(CellBlock {
                index: 1,
                item_count: 1,
                payload: &[0xaa, 0xbb],
            })
        );
    }

    #[test]
    fn parses_shipped_one_byte_empty_asset_table_sentinel() {
        let bytes = image_bytes(vec![0], &[0, 0]);
        let image = VegetationImage::parse(&bytes).unwrap();

        assert_eq!(image.asset_table().unwrap().count(), 0);
        assert!(image.asset_table().unwrap().entries().next().is_none());
        assert!(matches!(
            image.block(ASSET_TABLE_BLOCK).unwrap().kind().unwrap(),
            BlockKind::Empty
        ));
        assert_eq!(image.summary().unwrap().asset_entries, 0);
    }

    #[test]
    fn derives_smaller_region_block_grid_from_header() {
        let bytes = image_bytes_for_region(16, 1024, vec![0, 0], &[0, 0]);
        let image = VegetationImage::parse(&bytes).unwrap();

        assert_eq!(image.block_count(), 4096);
        assert_eq!(image.block(0).unwrap().bytes(), &[0, 0]);
        assert_eq!(image.block(4095).unwrap().bytes(), &[0, 0]);
        assert!(matches!(
            image.block(4096).unwrap_err(),
            ParseError::BlockOutOfRange { index: 4096 }
        ));
    }

    #[test]
    fn parses_cell_groups_and_instances() {
        let bytes = image_bytes(
            vec![0, 0],
            &[
                2, 0, // group count
                7, 0, 1, 0, // asset index 7, one instance
                0x10, 0x11, 0x12, 0x13, 0x20, 0x21, 0x22, 0x23, 0x30, 0x31, 0x32, 0x33, 0x40, 0x41,
                0x42, 0x43, 9, 0, 0, 0, // asset index 9, no instances
            ],
        );
        let image = VegetationImage::parse(&bytes).unwrap();
        let block = image.block(1).unwrap();
        let cell = match block.kind().unwrap() {
            BlockKind::Cell(cell) => cell,
            other => panic!("unexpected block kind: {other:?}"),
        };

        cell.validate().unwrap();
        let mut groups = cell.groups();
        let first = groups.next().unwrap().unwrap();
        assert_eq!(first.asset_index, 7);
        assert_eq!(first.instance_count(), 1);

        let instance = first.instances().next().unwrap();
        assert_eq!(
            instance.packed_words(),
            [0x1312_1110, 0x2322_2120, 0x3332_3130, 0x4342_4140]
        );
        assert_eq!(
            instance.packed_halves(),
            [
                0x1110, 0x1312, 0x2120, 0x2322, 0x3130, 0x3332, 0x4140, 0x4342,
            ]
        );

        let second = groups.next().unwrap().unwrap();
        assert_eq!(second.asset_index, 9);
        assert_eq!(second.instance_count(), 0);
        assert!(groups.next().is_none());

        let summary = image.summary().unwrap();
        assert_eq!(summary.asset_entries, 0);
        assert_eq!(summary.cell_blocks, 1);
        assert_eq!(summary.cell_groups, 2);
        assert_eq!(summary.instances, 1);
        assert_eq!(
            image.inspection_summary().unwrap(),
            VegetationImageInspectionSummary {
                tile_size: 16,
                region_size: 2048,
                baked_data_cache_size: 1024,
                blocks: BLOCK_COUNT,
                empty_blocks: BLOCK_COUNT - 1,
                cell_blocks: 1,
                cell_groups: 2,
                instances: 1,
                asset_entries: 0,
                tail_bytes: 2,
            }
        );

        let samples = image.instance_samples(1).unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].block, 1);
        assert_eq!(samples[0].asset_index, 7);
        assert_eq!(samples[0].instance_index, 0);

        let report = image.inspection_report(false, 1).unwrap();
        assert_eq!(report.instances.len(), 1);
        assert!(report.to_string().contains("instance block=1"));
    }

    #[test]
    fn decodes_cell_instance_placement() {
        let rotation_bits = 0x8000_0000 | (300 << 21) | (200 << 11) | 700;
        let bytes = [
            rotation_bits as u8,
            (rotation_bits >> 8) as u8,
            (rotation_bits >> 16) as u8,
            (rotation_bits >> 24) as u8,
            0xe8,
            0x03,
            0xd0,
            0x07,
            0x00,
            0x10,
            123,
            0x46,
            77,
            0,
            0x20,
            0x46,
        ];
        let instance = CellInstance { bytes: &bytes };
        let placement = instance.placement();

        assert_eq!(placement.rotation.bits(), rotation_bits);
        assert_eq!(placement.rotation.x_code(), 300);
        assert_eq!(placement.rotation.y_code(), 200);
        assert_eq!(placement.rotation.z_code(), 700);
        assert!(placement.rotation.w_is_negative());
        assert_eq!(placement.local_x_code, 1000);
        assert_eq!(placement.local_y_code, 2000);
        assert_eq!(placement.height_code, 4096);
        assert_eq!(placement.scale_code, 123);
        assert_eq!(placement.origin_x_high_byte, 0x46);
        assert_eq!(placement.source_index, 77);
        assert_eq!(placement.origin_y_high_half, 0x4620);
        assert_eq!(placement.scale(), 1.23);
        assert!((placement.height() - 128.00195).abs() < 0.0001);

        let offset = placement.local_offset(2048.0);
        assert!((offset.x - 31.250477).abs() < 0.0001);
        assert!((offset.y - 62.500954).abs() < 0.0001);
        assert_eq!(offset.z, placement.height());

        let position = placement.position(Vec2::new(12_288.0, 10_240.0), 2048.0);
        assert_eq!(
            position,
            Vec3::new(12_288.0 + offset.x, 10_240.0 + offset.y, offset.z)
        );
    }

    fn image_bytes(first_block: Vec<u8>, second_block: &[u8]) -> Vec<u8> {
        image_bytes_for_region(16, 2048, first_block, second_block)
    }

    fn image_bytes_for_region(
        tile_size: u16,
        region_size: u16,
        first_block: Vec<u8>,
        second_block: &[u8],
    ) -> Vec<u8> {
        let width = usize::from(region_size / tile_size);
        let block_count = width * width;
        let data_start = HEADER_SIZE + block_count * OFFSET_SIZE;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&tile_size.to_le_bytes());
        bytes.extend_from_slice(&region_size.to_le_bytes());
        bytes.extend_from_slice(&1024u32.to_le_bytes());
        bytes.resize(data_start, 0);

        let mut data = Vec::new();
        data.extend_from_slice(&first_block);
        let mut end = data_start + data.len();
        for index in 0..block_count {
            let offset = HEADER_SIZE + index * OFFSET_SIZE;
            bytes[offset..offset + OFFSET_SIZE].copy_from_slice(&(end as u32).to_le_bytes());
            if index + 1 < block_count {
                let block = if index == 0 { second_block } else { &[0, 0] };
                data.extend_from_slice(block);
                end += block.len();
            }
        }

        bytes.extend_from_slice(&data);
        bytes.extend_from_slice(&[0, 0]);
        bytes
    }
}
