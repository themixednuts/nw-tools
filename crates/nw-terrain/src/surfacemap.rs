//! Parser for New World `.surfacemap` files.
//!
//! `.surfacemap` is a hand-rolled binary format used by
//! `Coatlicue::SurfaceMapAssetHandler` (asset UUID
//! `{0F9D3341-6C8D-4DD1-A636-3622878FF8F6}`) to store the per-cell
//! material splat for one terrain region.
//!
//! # Layout (little-endian)
//!
//! ```text
//! offset  size  field
//! ------  ----  -----------------------------------------------------
//!   0x00   u8   version       (always 0x02 in shipped data)
//!   0x01   u8   material_count (number of newline-separated names)
//!   0x02   u16  layer_id_bits (1 for trivial tiles, 3 for full grids)
//!   0x04   u32  grid_dim      (cells per side, 0 for trivial tiles,
//!                              1024 for full grids)
//!   0x08   u32  data_size     (bytes in the packed splat block;
//!                              0 for trivial tiles)
//!   0x0c   u32  strings_size  (bytes in the material name block)
//!
//!   0x10            material name block (newline-separated UTF-8,
//!                   each entry followed by `\n`)
//!   0x10+strings    splat block (4 cells per 3 bytes for full grids)
//! ```
//!
//! # Trivial vs. full tiles
//!
//! Tiles where the entire region is a single material use a trivial
//! header (`grid_dim = 0`, `data_size = 0`, `material_count = 1`)
//! and ship no splat block — every implied cell is `materials[0]`.
//!
//! # Full splat block
//!
//! For full grids the dimension is `1024 × 1024` cells, each cell
//! packed as a 6-bit value (`layer_id_bits * 2`):
//!
//! ```text
//! cell = (primary_layer << 3) | secondary_layer
//! ```
//!
//! Both `primary_layer` and `secondary_layer` are 3-bit indices into
//! `materials[]`. The reserved value `7` is the "no layer" sentinel —
//! a cell with `secondary == 7` has only the primary material. Cells
//! are LE-packed: 4 cells share each 3-byte (24-bit) word, with cell
//! `n` occupying bits `[n*6 .. n*6+6]` of the word, lowest cell first.
//!
//! Format reverse-engineered from `NewWorld 3-26.exe` Ghidra
//! analysis + extracted samples in `tmp/surfacemap/`.

use std::collections::BTreeMap;
use std::{
    fmt, io,
    path::{Path, PathBuf},
};

/// `.surfacemap` files always start with this version byte.
pub const VERSION: u8 = 0x02;

/// Sentinel value indicating "no layer" in a cell slot.
pub const NO_LAYER: u8 = 7;

const HEADER_SIZE: usize = 16;

/// Parse error returned by [`SurfaceMap::parse`].
#[derive(Debug)]
pub enum ParseError {
    /// File shorter than the 16-byte header.
    TruncatedHeader { len: usize },
    /// Version byte is not [`VERSION`].
    BadVersion { found: u8 },
    /// Strings block runs past end of file.
    TruncatedStrings { need: usize, have: usize },
    /// Splat block runs past end of file.
    TruncatedData { need: usize, have: usize },
    /// `material_count` from header doesn't match the number of
    /// newline-terminated names actually present in the strings
    /// block.
    MaterialCountMismatch { header: u8, parsed: usize },
    /// Full-grid `data_size` doesn't match `grid_dim*grid_dim*6/8`.
    DataSizeMismatch { expected: u64, found: u32 },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { len } => {
                write!(f, "truncated header: {len} bytes (need {HEADER_SIZE})")
            }
            Self::BadVersion { found } => {
                write!(
                    f,
                    "unexpected version byte {found:#04x} (expected {VERSION:#04x})"
                )
            }
            Self::TruncatedStrings { need, have } => {
                write!(f, "truncated strings block: need {need}, have {have}")
            }
            Self::TruncatedData { need, have } => {
                write!(f, "truncated data block: need {need}, have {have}")
            }
            Self::MaterialCountMismatch { header, parsed } => {
                write!(
                    f,
                    "material_count mismatch: header says {header}, strings block has {parsed}"
                )
            }
            Self::DataSizeMismatch { expected, found } => {
                write!(
                    f,
                    "data_size mismatch: expected {expected} for full grid, got {found}"
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parsed view over a `.surfacemap` file.
///
/// The struct borrows from the input slice — callers can either read
/// the file fully, mmap it, or slice it from a pak entry.
#[derive(Clone)]
pub struct SurfaceMap<'a> {
    /// Format version (currently always [`VERSION`]).
    pub version: u8,
    /// Bits per layer-index slot (1 for trivial tiles, 3 for full).
    pub layer_id_bits: u16,
    /// Cells per side. `0` for trivial tiles, `1024` for full grids.
    pub grid_dim: u32,
    /// Material names, in declaration order. Index into this is the
    /// per-cell layer id used by the splat block.
    pub materials: Vec<&'a str>,
    /// Raw splat block. Empty for trivial tiles.
    pub data: &'a [u8],
}

impl<'a> SurfaceMap<'a> {
    /// Parse a `.surfacemap` file from a byte slice.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ParseError> {
        if bytes.len() < HEADER_SIZE {
            return Err(ParseError::TruncatedHeader { len: bytes.len() });
        }

        let version = bytes[0];
        if version != VERSION {
            return Err(ParseError::BadVersion { found: version });
        }
        let material_count = bytes[1];
        let layer_id_bits = u16::from_le_bytes([bytes[2], bytes[3]]);
        let grid_dim = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let data_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let strings_size =
            u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;

        let strings_end =
            HEADER_SIZE
                .checked_add(strings_size)
                .ok_or(ParseError::TruncatedStrings {
                    need: usize::MAX,
                    have: bytes.len(),
                })?;
        if strings_end > bytes.len() {
            return Err(ParseError::TruncatedStrings {
                need: strings_end,
                have: bytes.len(),
            });
        }

        let strings_block = &bytes[HEADER_SIZE..strings_end];
        let materials: Vec<&str> = strings_block
            .split(|b| *b == b'\n')
            .filter(|s| !s.is_empty())
            .map(|s| std::str::from_utf8(s).unwrap_or(""))
            .collect();

        if materials.len() != material_count as usize {
            return Err(ParseError::MaterialCountMismatch {
                header: material_count,
                parsed: materials.len(),
            });
        }

        let data_size = data_size as usize;
        let data_end = strings_end
            .checked_add(data_size)
            .ok_or(ParseError::TruncatedData {
                need: usize::MAX,
                have: bytes.len(),
            })?;
        if data_end > bytes.len() {
            return Err(ParseError::TruncatedData {
                need: data_end,
                have: bytes.len(),
            });
        }

        if grid_dim != 0 {
            let expected = u64::from(grid_dim) * u64::from(grid_dim) * 6 / 8;
            if expected != data_size as u64 {
                return Err(ParseError::DataSizeMismatch {
                    expected,
                    found: data_size as u32,
                });
            }
        }

        Ok(Self {
            version,
            layer_id_bits,
            grid_dim,
            materials,
            data: &bytes[strings_end..data_end],
        })
    }

    /// True if this tile has no splat block (single uniform material
    /// covers the whole region).
    pub fn is_trivial(&self) -> bool {
        self.grid_dim == 0
    }

    /// Total number of cells. `0` for trivial tiles.
    pub fn cell_count(&self) -> usize {
        let dim = self.grid_dim as usize;
        dim * dim
    }

    /// Decode a single cell. Returns `(primary, secondary)` layer
    /// indices, where each side is either an index into
    /// [`Self::materials`] or [`NO_LAYER`] (`7`) for "absent".
    ///
    /// Returns `None` if `index` is out of bounds, or if this is a
    /// trivial tile (use [`Self::trivial_layer`] instead).
    pub fn cell(&self, index: usize) -> Option<Cell> {
        if self.is_trivial() || index >= self.cell_count() {
            return None;
        }
        let bit = index * 6;
        let byte = bit / 8;
        let shift = bit & 7;
        // 6-bit fields fit within at most 2 bytes; read 2 bytes
        // little-endian and mask out the field.
        let lo = self.data[byte] as u16;
        let hi = if byte + 1 < self.data.len() {
            self.data[byte + 1] as u16
        } else {
            0
        };
        let word = lo | (hi << 8);
        let raw = ((word >> shift) & 0x3F) as u8;
        Some(Cell {
            primary: (raw >> 3) & 7,
            secondary: raw & 7,
            raw,
        })
    }

    /// For trivial tiles, return the single material name covering
    /// the entire region. `None` for non-trivial tiles or tiles that
    /// somehow have no materials at all.
    pub fn trivial_layer(&self) -> Option<&str> {
        if self.is_trivial() {
            self.materials.first().copied()
        } else {
            None
        }
    }

    /// Iterator over `(index, Cell)` for every cell.
    pub fn cells(&self) -> Cells<'_, 'a> {
        Cells {
            map: self,
            index: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceMapSummary {
    pub bytes: usize,
    pub version: u8,
    pub layer_id_bits: u16,
    pub grid_dim: u32,
    pub material_count: usize,
    pub splat_bytes: usize,
    pub trivial_layer: Option<String>,
}

impl SurfaceMapSummary {
    #[must_use]
    pub fn from_map(bytes: usize, map: &SurfaceMap<'_>) -> Self {
        Self {
            bytes,
            version: map.version,
            layer_id_bits: map.layer_id_bits,
            grid_dim: map.grid_dim,
            material_count: map.materials.len(),
            splat_bytes: map.data.len(),
            trivial_layer: map.trivial_layer().map(str::to_string),
        }
    }
}

impl fmt::Display for SurfaceMapSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  version:       0x{:02x}", self.version)?;
        writeln!(f, "  layer bits:    {}", self.layer_id_bits)?;
        writeln!(f, "  grid_dim:      {}", self.grid_dim)?;
        writeln!(f, "  materials:     {}", self.material_count)?;
        writeln!(f, "  splat bytes:   {}", self.splat_bytes)?;
        if let Some(layer) = &self.trivial_layer {
            writeln!(f, "  trivial:       {layer}")?;
        }
        write!(f, "  bytes:         {}", self.bytes)
    }
}

pub fn summarize_surface_map(bytes: &[u8]) -> Result<SurfaceMapSummary, ParseError> {
    let map = SurfaceMap::parse(bytes)?;
    Ok(SurfaceMapSummary::from_map(bytes.len(), &map))
}

#[derive(Debug, Clone)]
pub struct SurfaceMapInspection<'a> {
    pub summary: SurfaceMapSummary,
    map: SurfaceMap<'a>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurfaceMapInspectionReportOptions {
    pub histogram_limit: Option<usize>,
    pub ascii_step: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct SurfaceMapInspectionReport<'inspection, 'data> {
    inspection: &'inspection SurfaceMapInspection<'data>,
    options: SurfaceMapInspectionReportOptions,
}

#[derive(Debug, Clone)]
pub struct SurfaceMapFileInspectionReport<'path, 'data> {
    pub path: &'path Path,
    pub inspection: SurfaceMapInspection<'data>,
    pub options: SurfaceMapInspectionReportOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceMapHistogramRow {
    pub raw: u8,
    pub count: u64,
    pub primary: u8,
    pub secondary: u8,
}

pub fn inspect_surface_map(bytes: &[u8]) -> Result<SurfaceMapInspection<'_>, ParseError> {
    let map = SurfaceMap::parse(bytes)?;
    Ok(SurfaceMapInspection {
        summary: SurfaceMapSummary::from_map(bytes.len(), &map),
        map,
    })
}

pub fn inspect_surface_map_file<'path, 'data>(
    path: &'path Path,
    bytes: &'data [u8],
    options: SurfaceMapInspectionReportOptions,
) -> Result<SurfaceMapFileInspectionReport<'path, 'data>, ParseError> {
    let inspection = inspect_surface_map(bytes)?;
    Ok(SurfaceMapFileInspectionReport {
        path,
        inspection,
        options,
    })
}

#[derive(Debug)]
pub enum SurfaceMapInspectionError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, source: ParseError },
}

impl fmt::Display for SurfaceMapInspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "read surface map asset {path:?}: {source}")
            }
            Self::Parse { path, source } => {
                write!(f, "parse surface map asset {path:?}: {source}")
            }
        }
    }
}

impl std::error::Error for SurfaceMapInspectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

pub fn inspect_surface_map_path(
    path: impl AsRef<Path>,
    options: SurfaceMapInspectionReportOptions,
) -> Result<String, SurfaceMapInspectionError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| SurfaceMapInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    inspect_surface_map_file(path, &bytes, options)
        .map(|report| report.to_string())
        .map_err(|source| SurfaceMapInspectionError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

impl<'a> SurfaceMapInspection<'a> {
    #[inline]
    #[must_use]
    pub const fn report(&self) -> SurfaceMapInspectionReport<'_, 'a> {
        SurfaceMapInspectionReport {
            inspection: self,
            options: SurfaceMapInspectionReportOptions {
                histogram_limit: None,
                ascii_step: None,
            },
        }
    }

    #[must_use]
    pub fn materials(&self) -> &[&'a str] {
        &self.map.materials
    }

    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.map.cell_count()
    }

    #[must_use]
    pub fn material_name(&self, layer: u8) -> Option<&'a str> {
        self.map.materials.get(layer as usize).copied()
    }

    /// Human-readable layer name for inspection output.
    #[must_use]
    pub fn display_layer_name(&self, layer: u8) -> &'a str {
        if layer == NO_LAYER {
            "<none>"
        } else {
            self.material_name(layer).unwrap_or("<oob>")
        }
    }

    #[must_use]
    pub fn layer_pair_histogram(&self) -> Vec<SurfaceMapHistogramRow> {
        let mut counts: BTreeMap<u8, u64> = BTreeMap::new();
        for cell in self.map.cells() {
            *counts.entry(cell.raw).or_default() += 1;
        }
        let mut rows: Vec<_> = counts
            .into_iter()
            .map(|(raw, count)| SurfaceMapHistogramRow {
                raw,
                count,
                primary: (raw >> 3) & 7,
                secondary: raw & 7,
            })
            .collect();
        rows.sort_by_key(|row| std::cmp::Reverse(row.count));
        rows
    }

    #[must_use]
    pub fn primary_ascii_rows(&self, step: u32) -> Vec<String> {
        let palette = b".:-=+*#%@";
        let dim = self.map.grid_dim as usize;
        let step = step.max(1) as usize;
        let cells: Vec<_> = self.map.cells().collect();
        let mut rows = Vec::new();
        let mut y = 0;
        while y < dim {
            let mut row = String::with_capacity(dim / step + 1);
            let mut x = 0;
            while x < dim {
                let mut sum: u32 = 0;
                let mut populated = 0u32;
                for dy in 0..step.min(dim - y) {
                    for dx in 0..step.min(dim - x) {
                        let cell = cells[(y + dy) * dim + (x + dx)];
                        if cell.primary != NO_LAYER {
                            sum += cell.primary as u32;
                            populated += 1;
                        }
                    }
                }
                let glyph = if let Some(avg) = sum.checked_div(populated) {
                    let avg = avg as usize;
                    palette[avg.min(palette.len() - 1)]
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

    #[must_use]
    pub fn into_map(self) -> SurfaceMap<'a> {
        self.map
    }
}

impl<'inspection, 'data> SurfaceMapInspectionReport<'inspection, 'data> {
    #[inline]
    #[must_use]
    pub const fn with_histogram_limit(mut self, limit: usize) -> Self {
        self.options.histogram_limit = Some(limit);
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_ascii_step(mut self, step: u32) -> Self {
        self.options.ascii_step = Some(step);
        self
    }
}

impl fmt::Display for SurfaceMapInspectionReport<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inspection = self.inspection;
        let summary = &inspection.summary;

        writeln!(f, "  version:        0x{:02x}", summary.version)?;
        writeln!(f, "  layer_id_bits:  {}", summary.layer_id_bits)?;
        writeln!(f, "  grid_dim:       {}", summary.grid_dim)?;
        writeln!(f, "  materials ({}):", summary.material_count)?;
        for (index, name) in inspection.materials().iter().enumerate() {
            writeln!(f, "    [{index}] {name}")?;
        }
        writeln!(f, "  data bytes:     {}", summary.splat_bytes)?;

        if let Some(trivial_layer) = &summary.trivial_layer {
            writeln!(f, "  (trivial tile - entire region is `{trivial_layer}`)")?;
            return Ok(());
        }

        if let Some(limit) = self.options.histogram_limit {
            write_histogram(f, inspection, limit)?;
        }

        if let Some(step) = self.options.ascii_step {
            write_ascii(f, inspection, step.max(1))?;
        }

        Ok(())
    }
}

impl fmt::Display for SurfaceMapFileInspectionReport<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.path.display())?;
        let mut report = self.inspection.report();
        if let Some(limit) = self.options.histogram_limit {
            report = report.with_histogram_limit(limit);
        }
        if let Some(step) = self.options.ascii_step {
            report = report.with_ascii_step(step);
        }
        write!(f, "{report}")
    }
}

fn write_histogram(
    f: &mut fmt::Formatter<'_>,
    inspection: &SurfaceMapInspection<'_>,
    limit: usize,
) -> fmt::Result {
    let rows = inspection.layer_pair_histogram();
    let total = inspection.cell_count() as u64;
    writeln!(f)?;
    writeln!(f, "  cell histogram (top {}):", rows.len().min(limit))?;
    writeln!(
        f,
        "    {:>6} {:>10} {:>6}  primary -> secondary",
        "raw", "count", "pct"
    )?;
    for row in rows.into_iter().take(limit) {
        let pct = if total == 0 {
            0.0
        } else {
            (row.count as f64 * 100.0) / total as f64
        };
        let primary_name = inspection.display_layer_name(row.primary);
        let secondary_name = inspection.display_layer_name(row.secondary);
        writeln!(
            f,
            "    0x{:02x}  {:>10} {:>5.1}%  {} -> {}",
            row.raw, row.count, pct, primary_name, secondary_name
        )?;
    }
    Ok(())
}

fn write_ascii(
    f: &mut fmt::Formatter<'_>,
    inspection: &SurfaceMapInspection<'_>,
    step: u32,
) -> fmt::Result {
    writeln!(f)?;
    writeln!(f, "  primary-layer ASCII art (step={step}):")?;
    for row in inspection.primary_ascii_rows(step) {
        writeln!(f, "    {row}")?;
    }
    Ok(())
}

impl fmt::Debug for SurfaceMap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SurfaceMap")
            .field("version", &self.version)
            .field("layer_id_bits", &self.layer_id_bits)
            .field("grid_dim", &self.grid_dim)
            .field("materials", &self.materials)
            .field("data_bytes", &self.data.len())
            .finish()
    }
}

/// One decoded cell from the splat block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// Primary layer index (3 bits). [`NO_LAYER`] (`7`) means the
    /// slot is unused — only the secondary slot contributes.
    pub primary: u8,
    /// Secondary layer index (3 bits). [`NO_LAYER`] (`7`) means the
    /// cell is a single-material cell of `primary`.
    pub secondary: u8,
    /// Raw 6-bit packed value `(primary << 3) | secondary`.
    pub raw: u8,
}

/// Iterator over every cell in a [`SurfaceMap`].
pub struct Cells<'m, 'a> {
    map: &'m SurfaceMap<'a>,
    index: usize,
}

impl Iterator for Cells<'_, '_> {
    type Item = Cell;

    fn next(&mut self) -> Option<Cell> {
        let cell = self.map.cell(self.index)?;
        self.index += 1;
        Some(cell)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.map.cell_count().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-rolled trivial tile: version=2, 1 material "mat_default".
    #[test]
    fn parses_trivial_tile() {
        let bytes: &[u8] = &[
            0x02, 0x01, 0x01, 0x00, // version, count, layer_id_bits
            0x00, 0x00, 0x00, 0x00, // grid_dim = 0
            0x00, 0x00, 0x00, 0x00, // data_size = 0
            0x0c, 0x00, 0x00, 0x00, // strings_size = 12
            b'm', b'a', b't', b'_', b'd', b'e', b'f', b'a', b'u', b'l', b't', b'\n',
        ];
        let map = SurfaceMap::parse(bytes).unwrap();
        assert!(map.is_trivial());
        assert_eq!(map.materials, vec!["mat_default"]);
        assert_eq!(map.trivial_layer(), Some("mat_default"));
        assert_eq!(map.cell_count(), 0);

        let summary = summarize_surface_map(bytes).unwrap();
        assert_eq!(
            summary,
            SurfaceMapSummary {
                bytes: bytes.len(),
                version: VERSION,
                layer_id_bits: 1,
                grid_dim: 0,
                material_count: 1,
                splat_bytes: 0,
                trivial_layer: Some("mat_default".to_string()),
            }
        );
        assert_eq!(
            summary.to_string(),
            "  version:       0x02\n  layer bits:    1\n  grid_dim:      0\n  materials:     1\n  splat bytes:   0\n  trivial:       mat_default\n  bytes:         28"
        );

        let inspection = inspect_surface_map(bytes).unwrap();
        assert_eq!(inspection.summary, summary);
        assert_eq!(inspection.materials(), ["mat_default"].as_slice());
        assert_eq!(inspection.display_layer_name(0), "mat_default");
        assert_eq!(inspection.display_layer_name(NO_LAYER), "<none>");
        assert_eq!(inspection.display_layer_name(3), "<oob>");
        assert!(inspection.report().to_string().contains("trivial tile"));
        assert_eq!(
            inspect_surface_map_file(
                Path::new("levels/a/region.surfacemap"),
                bytes,
                SurfaceMapInspectionReportOptions::default()
            )
            .unwrap()
            .to_string(),
            "levels/a/region.surfacemap\n  version:        0x02\n  layer_id_bits:  1\n  grid_dim:       0\n  materials (1):\n    [0] mat_default\n  data bytes:     0\n  (trivial tile - entire region is `mat_default`)\n"
        );
    }

    #[test]
    fn rejects_bad_version() {
        let mut bytes = vec![0u8; HEADER_SIZE];
        bytes[0] = 0x03;
        let err = SurfaceMap::parse(&bytes).unwrap_err();
        assert!(matches!(err, ParseError::BadVersion { found: 0x03 }));
    }

    /// 24-bit word `0xd7755d` is the actual repeating triple seen in
    /// shipped boundary tiles. With LE 6-bit-per-cell packing it
    /// decodes to a 4-cell stipple: (29, 21, 55, 53).
    #[test]
    fn decodes_packed_cells_boundary_pattern() {
        let raw = [0x5du8, 0x75, 0xd7];
        let map = SurfaceMap {
            version: VERSION,
            layer_id_bits: 3,
            grid_dim: 2,
            materials: vec!["a", "b", "c", "d", "e", "f", "g"],
            data: &raw,
        };
        let cells: Vec<_> = (0..4).map(|i| map.cell(i).unwrap().raw).collect();
        assert_eq!(cells, vec![29, 21, 55, 53]);

        // 29 = 0b011101 = (primary=3, secondary=5)
        let c0 = map.cell(0).unwrap();
        assert_eq!((c0.primary, c0.secondary), (3, 5));
        // 55 = 0b110111 = (primary=6, secondary=7=NO_LAYER)
        let c2 = map.cell(2).unwrap();
        assert_eq!((c2.primary, c2.secondary), (6, NO_LAYER));

        let inspection = SurfaceMapInspection {
            summary: SurfaceMapSummary::from_map(raw.len(), &map),
            map,
        };
        let report = inspection
            .report()
            .with_histogram_limit(1)
            .with_ascii_step(1)
            .to_string();
        assert!(report.contains("cell histogram"));
        assert!(report.contains("primary-layer ASCII art"));
    }

    /// Four uniform cells of (primary=6, secondary=NO_LAYER), i.e.
    /// raw=0x37 = 0b110111 in every slot, pack to LE bytes
    /// `f7 7d df`.
    #[test]
    fn decodes_uniform_cells() {
        let raw = [0xf7u8, 0x7d, 0xdf];
        let map = SurfaceMap {
            version: VERSION,
            layer_id_bits: 3,
            grid_dim: 2,
            materials: vec!["a", "b", "c", "d", "e", "f", "g"],
            data: &raw,
        };
        for i in 0..4 {
            let cell = map.cell(i).unwrap();
            assert_eq!(cell.raw, 0x37);
            assert_eq!(cell.primary, 6);
            assert_eq!(cell.secondary, NO_LAYER);
        }
    }
}
