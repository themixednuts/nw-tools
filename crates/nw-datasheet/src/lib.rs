use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    fmt, io,
    iter::FusedIterator,
    ops::{Deref, Index},
    path::{Path, PathBuf},
    slice, str,
};
use thiserror::Error;

const SUPPORTED_VERSIONS: [u32; 2] = [0x11, 0x12];

const VERSION_OFFSET: usize = 0x00;
const NAME_CRC_OFFSET: usize = 0x04;
const NAME_STRING_OFFSET: usize = 0x08;
const TYPE_CRC_OFFSET: usize = 0x0c;
const TYPE_STRING_OFFSET: usize = 0x10;
const DATA_SIZE_OFFSET: usize = 0x38;
const COLUMN_COUNT_OFFSET: usize = 0x44;
const ROW_COUNT_OFFSET: usize = 0x48;
const COLUMN_RECORDS_OFFSET: usize = 0x5c;

const COLUMN_RECORD_SIZE: usize = 12;
const CELL_RECORD_SIZE: usize = 8;
const DATA_END_OFFSET: usize = 0x38;
const U32_SIZE: usize = 4;
pub const DATASHEET_EXTENSION: &str = "datasheet";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ParseError {
    #[error("datasheet too short at 0x{offset:x}: need {needed} bytes, input has {len}")]
    UnexpectedEof {
        offset: usize,
        needed: usize,
        len: usize,
    },

    #[error("unsupported datasheet version 0x{actual:x}")]
    UnsupportedVersion { actual: u32 },

    #[error("datasheet layout overflow")]
    LayoutOverflow,

    #[error(
        "datasheet data_size {data_size} points string table at 0x{strings_offset:x}, expected 0x{expected_offset:x}"
    )]
    InvalidDataSize {
        data_size: usize,
        strings_offset: usize,
        expected_offset: usize,
    },

    #[error("negative string offset {offset}")]
    NegativeStringOffset { offset: i32 },

    #[error("string offset {offset} is outside string table length {len}")]
    StringOffsetOutOfBounds { offset: u32, len: usize },

    #[error("string offset {offset} does not point to a string boundary")]
    StringOffsetNotAtBoundary { offset: u32 },

    #[error("unterminated string at file offset 0x{offset:x}")]
    UnterminatedString { offset: usize },

    #[error("invalid utf-8 string at file offset 0x{offset:x}")]
    InvalidUtf8 {
        offset: usize,
        #[source]
        source: str::Utf8Error,
    },

    #[error("unknown datasheet column type {value} at column {column}")]
    UnknownColumnType { column: usize, value: u32 },
}

pub type DatasheetParseError = ParseError;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DatasheetInspectionError {
    #[error("read {path:?}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("parse datasheet {path:?}: {source}")]
    Parse { path: PathBuf, source: ParseError },
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ColumnType {
    String = 0x01,
    Number = 0x02,
    Boolean = 0x03,
}

impl ColumnType {
    #[must_use]
    #[inline]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::String),
            2 => Some(Self::Number),
            3 => Some(Self::Boolean),
            _ => None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    #[must_use]
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
        }
    }
}

impl TryFrom<u32> for ColumnType {
    type Error = u32;

    #[inline]
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::from_u32(value).ok_or(value)
    }
}

impl From<ColumnType> for u32 {
    #[inline]
    fn from(value: ColumnType) -> Self {
        value.as_u32()
    }
}

impl fmt::Display for ColumnType {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Datasheet<'a> {
    version: u32,
    name_crc: u32,
    name: &'a str,
    type_crc: u32,
    type_name: &'a str,
    columns: Vec<Column<'a>>,
    cells: Vec<Cell<'a>>,
    row_count: usize,
    localization: Option<&'a DashMap<String, Option<String>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatasheetSummary<'a> {
    pub version: u32,
    pub rows: usize,
    pub columns: usize,
    pub cells: usize,
    pub name: &'a str,
    pub type_name: &'a str,
    pub string_columns: usize,
    pub number_columns: usize,
    pub boolean_columns: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedDatasheetSummary {
    pub version: u32,
    pub rows: usize,
    pub columns: usize,
    pub cells: usize,
    pub name: String,
    pub type_name: String,
    pub string_columns: usize,
    pub number_columns: usize,
    pub boolean_columns: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasheetFileSummary {
    pub source: String,
    pub summary: OwnedDatasheetSummary,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DatasheetInspection {
    pub rows: Vec<DatasheetFileSummary>,
    pub totals: DatasheetTotals,
}

#[derive(Debug, Clone, Copy)]
pub struct DatasheetInspectionReport<'a> {
    inspection: &'a DatasheetInspection,
    limit: usize,
}

impl<'a> DatasheetSummary<'a> {
    #[must_use]
    pub fn from_datasheet(sheet: &Datasheet<'a>) -> Self {
        let mut summary = Self {
            version: sheet.version(),
            rows: sheet.len(),
            columns: sheet.column_len(),
            cells: sheet.cells().len(),
            name: sheet.name(),
            type_name: sheet.type_name(),
            string_columns: 0,
            number_columns: 0,
            boolean_columns: 0,
        };

        for column in sheet.columns() {
            match column.column_type() {
                ColumnType::String => summary.string_columns += 1,
                ColumnType::Number => summary.number_columns += 1,
                ColumnType::Boolean => summary.boolean_columns += 1,
            }
        }

        summary
    }
}

impl OwnedDatasheetSummary {
    #[must_use]
    pub fn as_borrowed(&self) -> DatasheetSummary<'_> {
        DatasheetSummary {
            version: self.version,
            rows: self.rows,
            columns: self.columns,
            cells: self.cells,
            name: &self.name,
            type_name: &self.type_name,
            string_columns: self.string_columns,
            number_columns: self.number_columns,
            boolean_columns: self.boolean_columns,
        }
    }
}

impl From<DatasheetSummary<'_>> for OwnedDatasheetSummary {
    fn from(summary: DatasheetSummary<'_>) -> Self {
        Self {
            version: summary.version,
            rows: summary.rows,
            columns: summary.columns,
            cells: summary.cells,
            name: summary.name.to_owned(),
            type_name: summary.type_name.to_owned(),
            string_columns: summary.string_columns,
            number_columns: summary.number_columns,
            boolean_columns: summary.boolean_columns,
        }
    }
}

impl fmt::Display for DatasheetSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "v0x{:x}, {} rows, {} columns, {} cells, sheet {:?}, type {:?}",
            self.version, self.rows, self.columns, self.cells, self.name, self.type_name
        )
    }
}

impl fmt::Display for OwnedDatasheetSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_borrowed().fmt(f)
    }
}

/// Parse bytes and return the datasheet's summary counts.
///
/// # Errors
///
/// Returns [`ParseError`] if `bytes` is not a supported binary datasheet.
pub fn summarize_datasheet(bytes: &[u8]) -> Result<DatasheetSummary<'_>, ParseError> {
    Datasheet::parse(bytes).map(|sheet| DatasheetSummary::from_datasheet(&sheet))
}

/// Parse bytes from one datasheet file and attach the source path to the summary.
///
/// # Errors
///
/// Returns [`ParseError`] if `bytes` is not a supported binary datasheet.
pub fn inspect_datasheet_file(
    path: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<DatasheetFileSummary, ParseError> {
    Ok(DatasheetFileSummary {
        source: path.as_ref().display().to_string(),
        summary: summarize_datasheet(bytes)?.into(),
    })
}

/// Read and inspect one datasheet path.
///
/// # Errors
///
/// Returns [`DatasheetInspectionError`] if the file cannot be read or parsed.
pub fn inspect_datasheet_path(
    path: impl AsRef<Path>,
) -> Result<DatasheetFileSummary, DatasheetInspectionError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| DatasheetInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    inspect_datasheet_file(path, &bytes).map_err(|source| DatasheetInspectionError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Read and inspect multiple datasheet paths.
///
/// # Errors
///
/// Returns [`DatasheetInspectionError`] for the first file that cannot be read
/// or parsed.
pub fn inspect_datasheet_files<I, P>(
    paths: I,
) -> Result<DatasheetInspection, DatasheetInspectionError>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut inspection = DatasheetInspection::default();
    for path in paths {
        inspection.add_file_summary(inspect_datasheet_path(path)?);
    }
    Ok(inspection)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DatasheetTotals {
    pub files: usize,
    pub rows: usize,
    pub columns: usize,
    pub cells: usize,
    pub string_columns: usize,
    pub number_columns: usize,
    pub boolean_columns: usize,
}

impl DatasheetTotals {
    pub fn add_summary(&mut self, summary: DatasheetSummary<'_>) {
        self.files += 1;
        self.rows += summary.rows;
        self.columns += summary.columns;
        self.cells += summary.cells;
        self.string_columns += summary.string_columns;
        self.number_columns += summary.number_columns;
        self.boolean_columns += summary.boolean_columns;
    }
}

impl DatasheetInspection {
    pub fn add_file_summary(&mut self, row: DatasheetFileSummary) {
        self.totals.add_summary(row.summary.as_borrowed());
        self.rows.push(row);
    }

    #[must_use]
    pub const fn report(&self, limit: usize) -> DatasheetInspectionReport<'_> {
        DatasheetInspectionReport {
            inspection: self,
            limit,
        }
    }
}

impl fmt::Display for DatasheetTotals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  files: {}", self.files)?;
        writeln!(f, "  rows: {}", self.rows)?;
        writeln!(f, "  columns: {}", self.columns)?;
        writeln!(f, "  cells: {}", self.cells)?;
        writeln!(f, "  string columns: {}", self.string_columns)?;
        writeln!(f, "  number columns: {}", self.number_columns)?;
        writeln!(f, "  boolean columns: {}", self.boolean_columns)
    }
}

impl fmt::Display for DatasheetInspectionReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.limit > 0 {
            for row in self.inspection.rows.iter().take(self.limit) {
                writeln!(f, "{}: {}", row.source, row.summary)?;
            }

            if self.inspection.rows.len() > self.limit {
                writeln!(
                    f,
                    "... {} more files",
                    self.inspection.rows.len() - self.limit
                )?;
            }
        }

        write!(f, "{}", self.inspection.totals)
    }
}

#[must_use]
pub fn is_datasheet_extension(extension: &str) -> bool {
    extension.eq_ignore_ascii_case(DATASHEET_EXTENSION)
}

#[must_use]
pub fn is_datasheet_name(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| is_datasheet_extension(extension))
}

#[must_use]
pub fn is_datasheet_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_datasheet_extension)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Column<'a> {
    crc: u32,
    name: &'a str,
    #[serde(rename = "column_type")]
    kind: ColumnType,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Cell<'a> {
    crc: u32,
    value: CellValue<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum CellValue<'a> {
    String(&'a str),
    Number(f32),
    Boolean(bool),
}

#[derive(Debug, Clone, Copy)]
pub struct Row<'sheet, 'data> {
    columns: &'sheet [Column<'data>],
    cells: &'sheet [Cell<'data>],
}

#[derive(Debug, Clone)]
pub struct Rows<'sheet, 'data> {
    columns: &'sheet [Column<'data>],
    cells: &'sheet [Cell<'data>],
    column_count: usize,
    front: usize,
    back: usize,
}

#[derive(Debug, Clone, Copy)]
struct Layout {
    column_count: usize,
    row_count: usize,
    columns_offset: usize,
    columns_len: usize,
    cells_offset: usize,
    cells_len: usize,
    strings_offset: usize,
}

#[derive(Debug, Clone, Copy)]
struct StringEntry<'a> {
    offset: u32,
    value: &'a str,
}

#[derive(Debug, Clone)]
struct StringTable<'a> {
    len: usize,
    entries: Vec<StringEntry<'a>>,
}

impl<'a> Datasheet<'a> {
    /// Parse a New World binary `.datasheet`.
    ///
    /// The parser walks each fixed-width record section linearly and borrows
    /// strings from the input buffer. Keep the input bytes alive for as long as
    /// the returned [`Datasheet`] is used.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the input is truncated, declares an unsupported
    /// version, has inconsistent section sizes, references strings outside the
    /// string table, contains invalid UTF-8, or declares an unknown column type.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ParseError> {
        let version = le_u32_at(bytes, VERSION_OFFSET)?;
        if !version_supported(version) {
            return Err(ParseError::UnsupportedVersion { actual: version });
        }

        let name_crc = le_u32_at(bytes, NAME_CRC_OFFSET)?;
        let name_offset = le_u32_at(bytes, NAME_STRING_OFFSET)?;
        let type_crc = le_u32_at(bytes, TYPE_CRC_OFFSET)?;
        let type_offset = le_u32_at(bytes, TYPE_STRING_OFFSET)?;
        let data_size = le_u32_at(bytes, DATA_SIZE_OFFSET)? as usize;
        let column_count = le_u32_at(bytes, COLUMN_COUNT_OFFSET)? as usize;
        let row_count = le_u32_at(bytes, ROW_COUNT_OFFSET)? as usize;

        let layout = Layout::new(bytes.len(), data_size, column_count, row_count)?;
        let string_table = StringTable::parse(
            slice_at(
                bytes,
                layout.strings_offset,
                bytes.len() - layout.strings_offset,
            )?,
            layout.strings_offset,
        )?;

        let mut columns = Vec::with_capacity(layout.column_count);
        for (column, record) in slice_at(bytes, layout.columns_offset, layout.columns_len)?
            .chunks_exact(COLUMN_RECORD_SIZE)
            .enumerate()
        {
            let crc = le_u32(record);
            let name_offset = le_i32(&record[4..]);
            let raw_type = le_u32(&record[8..]);
            let column_type =
                ColumnType::from_u32(raw_type).ok_or(ParseError::UnknownColumnType {
                    column,
                    value: raw_type,
                })?;
            columns.push(Column {
                crc,
                name: string_table.get_i32(name_offset)?,
                kind: column_type,
            });
        }

        let cell_records = slice_at(bytes, layout.cells_offset, layout.cells_len)?;
        let mut cells = Vec::with_capacity(layout.cell_count()?);
        let mut offset = 0;
        for _ in 0..layout.row_count {
            for column in &columns {
                let record = &cell_records[offset..offset + CELL_RECORD_SIZE];
                offset += CELL_RECORD_SIZE;

                let crc = le_u32(record);
                let raw_value = le_u32(&record[4..]);
                let value = match column.kind {
                    ColumnType::String => CellValue::String(string_table.get(raw_value)?),
                    ColumnType::Number => {
                        CellValue::Number(f32::from_le_bytes(u32::to_le_bytes(raw_value)))
                    }
                    ColumnType::Boolean => {
                        CellValue::Boolean(i32::from_le_bytes(u32::to_le_bytes(raw_value)) != 0)
                    }
                };
                cells.push(Cell { crc, value });
            }
        }

        Ok(Self {
            version,
            name_crc,
            name: string_table.get(name_offset)?,
            type_crc,
            type_name: string_table.get(type_offset)?,
            columns,
            cells,
            row_count,
            localization: None,
        })
    }

    #[must_use]
    #[inline]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    #[inline]
    pub const fn name_crc(&self) -> u32 {
        self.name_crc
    }

    #[must_use]
    #[inline]
    pub const fn name(&self) -> &'a str {
        self.name
    }

    #[must_use]
    #[inline]
    pub const fn type_crc(&self) -> u32 {
        self.type_crc
    }

    #[must_use]
    #[inline]
    pub const fn type_name(&self) -> &'a str {
        self.type_name
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.row_count
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.row_count == 0
    }

    #[must_use]
    #[inline]
    pub fn column_len(&self) -> usize {
        self.columns.len()
    }

    #[must_use]
    #[inline]
    pub fn columns(&self) -> &[Column<'a>] {
        &self.columns
    }

    #[must_use]
    #[inline]
    pub fn cells(&self) -> &[Cell<'a>] {
        &self.cells
    }

    #[must_use]
    #[inline]
    pub fn rows(&self) -> Rows<'_, 'a> {
        Rows {
            columns: &self.columns,
            cells: &self.cells,
            column_count: self.columns.len(),
            front: 0,
            back: self.row_count,
        }
    }

    #[must_use]
    #[inline]
    pub fn iter(&self) -> Rows<'_, 'a> {
        self.rows()
    }

    #[must_use]
    #[inline]
    pub fn row(&self, index: usize) -> Option<Row<'_, 'a>> {
        Some(Row {
            columns: &self.columns,
            cells: self.row_cells(index)?,
        })
    }

    #[must_use]
    #[inline]
    pub fn row_cells(&self, index: usize) -> Option<&[Cell<'a>]> {
        if index >= self.row_count {
            return None;
        }
        let start = index.checked_mul(self.columns.len())?;
        let end = start.checked_add(self.columns.len())?;
        self.cells.get(start..end)
    }

    #[must_use]
    #[inline]
    pub fn find_row(&self, name_crc: u32) -> Option<Row<'_, 'a>> {
        self.rows()
            .find(|row| row.cell(0).is_some_and(|cell| cell.crc() == name_crc))
    }

    #[must_use]
    #[inline]
    pub fn column(&self, index: usize) -> Option<&Column<'a>> {
        self.columns.get(index)
    }

    #[must_use]
    #[inline]
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|column| column.name == name)
    }

    #[must_use]
    #[inline]
    pub fn column_index_by_crc(&self, crc: u32) -> Option<usize> {
        self.columns.iter().position(|column| column.crc == crc)
    }

    #[must_use]
    #[inline]
    pub fn cell(&self, row: usize, column: usize) -> Option<&Cell<'a>> {
        self.row_cells(row)?.get(column)
    }

    #[must_use]
    #[inline]
    pub fn cell_by_name(&self, row: usize, column: &str) -> Option<&Cell<'a>> {
        self.cell(row, self.column_index(column)?)
    }

    #[must_use]
    #[inline]
    pub fn cell_by_crc(&self, row: usize, column_crc: u32) -> Option<&Cell<'a>> {
        self.cell(row, self.column_index_by_crc(column_crc)?)
    }

    #[must_use]
    pub fn with_localization(mut self, localization: &'a DashMap<String, Option<String>>) -> Self {
        self.localization = Some(localization);
        self
    }

    pub fn set_localization(&mut self, localization: Option<&'a DashMap<String, Option<String>>>) {
        self.localization = localization;
    }

    #[must_use]
    #[inline]
    pub const fn localization(&self) -> Option<&'a DashMap<String, Option<String>>> {
        self.localization
    }

    #[must_use]
    pub fn localized<'value>(&self, value: &'value str) -> Cow<'value, str> {
        let Some(key) = value.strip_prefix('@') else {
            return Cow::Borrowed(value);
        };

        let Some(map) = self.localization else {
            return Cow::Borrowed(value);
        };

        match map.get(&key.to_lowercase()).and_then(|entry| entry.clone()) {
            Some(localized) => Cow::Owned(localized),
            None => Cow::Borrowed(value),
        }
    }
}

impl<'a> TryFrom<&'a [u8]> for Datasheet<'a> {
    type Error = ParseError;

    #[inline]
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl<'a> TryFrom<&'a Vec<u8>> for Datasheet<'a> {
    type Error = ParseError;

    #[inline]
    fn try_from(value: &'a Vec<u8>) -> Result<Self, Self::Error> {
        Self::parse(value.as_slice())
    }
}

impl<'a> Index<usize> for Datasheet<'a> {
    type Output = [Cell<'a>];

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.row_cells(index)
            .unwrap_or_else(|| panic!("datasheet row index {index} out of bounds"))
    }
}

impl<'sheet, 'data> IntoIterator for &'sheet Datasheet<'data> {
    type Item = Row<'sheet, 'data>;
    type IntoIter = Rows<'sheet, 'data>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.rows()
    }
}

impl<'a> Column<'a> {
    #[must_use]
    #[inline]
    pub const fn crc(&self) -> u32 {
        self.crc
    }

    #[must_use]
    #[inline]
    pub const fn name(&self) -> &'a str {
        self.name
    }

    #[must_use]
    #[inline]
    pub const fn column_type(&self) -> ColumnType {
        self.kind
    }
}

impl<'a> Cell<'a> {
    #[must_use]
    #[inline]
    pub const fn new(crc: u32, value: CellValue<'a>) -> Self {
        Self { crc, value }
    }

    #[must_use]
    #[inline]
    pub const fn crc(&self) -> u32 {
        self.crc
    }

    #[must_use]
    #[inline]
    pub const fn value(&self) -> &CellValue<'a> {
        &self.value
    }

    #[must_use]
    #[inline]
    pub const fn column_type(&self) -> ColumnType {
        self.value.column_type()
    }

    #[must_use]
    #[inline]
    pub const fn as_str(&self) -> Option<&'a str> {
        self.value.as_str()
    }

    #[must_use]
    #[inline]
    pub const fn as_f32(&self) -> Option<f32> {
        self.value.as_f32()
    }

    #[must_use]
    #[inline]
    pub const fn as_f64(&self) -> Option<f64> {
        self.value.as_f64()
    }

    #[must_use]
    #[inline]
    pub const fn as_bool(&self) -> Option<bool> {
        self.value.as_bool()
    }
}

impl<'a> AsRef<CellValue<'a>> for Cell<'a> {
    #[inline]
    fn as_ref(&self) -> &CellValue<'a> {
        &self.value
    }
}

impl<'a> Deref for Cell<'a> {
    type Target = CellValue<'a>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl fmt::Display for Cell<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl<'a> CellValue<'a> {
    #[must_use]
    #[inline]
    pub const fn column_type(&self) -> ColumnType {
        match self {
            Self::String(_) => ColumnType::String,
            Self::Number(_) => ColumnType::Number,
            Self::Boolean(_) => ColumnType::Boolean,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_str(&self) -> Option<&'a str> {
        match self {
            Self::String(value) => Some(*value),
            Self::Number(_) | Self::Boolean(_) => None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_f32(&self) -> Option<f32> {
        match self {
            Self::Number(value) => Some(*value),
            Self::String(_) | Self::Boolean(_) => None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value as f64),
            Self::String(_) | Self::Boolean(_) => None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            Self::String(_) | Self::Number(_) => None,
        }
    }
}

impl fmt::Display for CellValue<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(value) => f.write_str(value),
            Self::Number(value) => value.fmt(f),
            Self::Boolean(value) => value.fmt(f),
        }
    }
}

impl<'sheet, 'data> Row<'sheet, 'data> {
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.cells.len()
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    #[must_use]
    #[inline]
    pub const fn columns(&self) -> &'sheet [Column<'data>] {
        self.columns
    }

    #[must_use]
    #[inline]
    pub const fn cells(&self) -> &'sheet [Cell<'data>] {
        self.cells
    }

    #[must_use]
    #[inline]
    pub fn cell(&self, index: usize) -> Option<&'sheet Cell<'data>> {
        self.cells.get(index)
    }

    #[must_use]
    #[inline]
    pub fn cell_by_name(&self, name: &str) -> Option<&'sheet Cell<'data>> {
        let index = self.columns.iter().position(|column| column.name == name)?;
        self.cell(index)
    }

    #[must_use]
    #[inline]
    pub fn cell_by_crc(&self, crc: u32) -> Option<&'sheet Cell<'data>> {
        let index = self.columns.iter().position(|column| column.crc == crc)?;
        self.cell(index)
    }

    #[inline]
    pub fn iter(&self) -> slice::Iter<'sheet, Cell<'data>> {
        self.cells.iter()
    }
}

impl<'sheet, 'data> IntoIterator for &'sheet Row<'sheet, 'data> {
    type Item = &'sheet Cell<'data>;
    type IntoIter = slice::Iter<'sheet, Cell<'data>>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'data> AsRef<[Cell<'data>]> for Row<'_, 'data> {
    #[inline]
    fn as_ref(&self) -> &[Cell<'data>] {
        self.cells
    }
}

impl<'data> Deref for Row<'_, 'data> {
    type Target = [Cell<'data>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.cells
    }
}

impl<'data> Index<usize> for Row<'_, 'data> {
    type Output = Cell<'data>;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.cells[index]
    }
}

impl<'sheet, 'data> IntoIterator for Row<'sheet, 'data> {
    type Item = &'sheet Cell<'data>;
    type IntoIter = slice::Iter<'sheet, Cell<'data>>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.cells.iter()
    }
}

impl<'sheet, 'data> Iterator for Rows<'sheet, 'data> {
    type Item = Row<'sheet, 'data>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let row = self.row(self.front);
        self.front += 1;
        Some(row)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl DoubleEndedIterator for Rows<'_, '_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        Some(self.row(self.back))
    }
}

impl ExactSizeIterator for Rows<'_, '_> {
    #[inline]
    fn len(&self) -> usize {
        self.back - self.front
    }
}

impl FusedIterator for Rows<'_, '_> {}

impl<'sheet, 'data> Rows<'sheet, 'data> {
    #[inline]
    fn row(&self, index: usize) -> Row<'sheet, 'data> {
        let start = index * self.column_count;
        let end = start + self.column_count;
        Row {
            columns: self.columns,
            cells: &self.cells[start..end],
        }
    }
}

impl Layout {
    #[inline]
    fn new(
        input_len: usize,
        data_size: usize,
        column_count: usize,
        row_count: usize,
    ) -> Result<Self, ParseError> {
        let columns_len = checked_mul(column_count, COLUMN_RECORD_SIZE)?;
        let cells_offset = checked_add(COLUMN_RECORDS_OFFSET, columns_len)?;
        let cell_count = checked_mul(row_count, column_count)?;
        let cells_len = checked_mul(cell_count, CELL_RECORD_SIZE)?;
        let expected_strings_offset = checked_add(cells_offset, cells_len)?;
        let strings_offset = checked_add(checked_add(data_size, DATA_END_OFFSET)?, U32_SIZE)?;

        if strings_offset != expected_strings_offset {
            return Err(ParseError::InvalidDataSize {
                data_size,
                strings_offset,
                expected_offset: expected_strings_offset,
            });
        }

        require_range(input_len, COLUMN_RECORDS_OFFSET, columns_len)?;
        require_range(input_len, cells_offset, cells_len)?;
        require_range(input_len, strings_offset, 0)?;

        Ok(Self {
            column_count,
            row_count,
            columns_offset: COLUMN_RECORDS_OFFSET,
            columns_len,
            cells_offset,
            cells_len,
            strings_offset,
        })
    }

    #[inline]
    const fn cell_count(self) -> Result<usize, ParseError> {
        match self.row_count.checked_mul(self.column_count) {
            Some(value) => Ok(value),
            None => Err(ParseError::LayoutOverflow),
        }
    }
}

impl<'a> StringTable<'a> {
    fn parse(bytes: &'a [u8], file_offset: usize) -> Result<Self, ParseError> {
        let mut entries = Vec::new();
        let mut offset = 0usize;

        while offset < bytes.len() {
            let tail = &bytes[offset..];
            let len =
                tail.iter()
                    .position(|byte| *byte == 0)
                    .ok_or(ParseError::UnterminatedString {
                        offset: file_offset + offset,
                    })?;
            let value = str::from_utf8(&tail[..len]).map_err(|source| ParseError::InvalidUtf8 {
                offset: file_offset + offset,
                source,
            })?;
            entries.push(StringEntry {
                offset: u32::try_from(offset).map_err(|_| ParseError::LayoutOverflow)?,
                value,
            });
            offset += len + 1;
        }

        Ok(Self {
            len: bytes.len(),
            entries,
        })
    }

    #[inline]
    fn get_i32(&self, offset: i32) -> Result<&'a str, ParseError> {
        let offset =
            u32::try_from(offset).map_err(|_| ParseError::NegativeStringOffset { offset })?;
        self.get(offset)
    }

    #[inline]
    fn get(&self, offset: u32) -> Result<&'a str, ParseError> {
        if offset as usize >= self.len {
            return Err(ParseError::StringOffsetOutOfBounds {
                offset,
                len: self.len,
            });
        }

        self.entries
            .binary_search_by_key(&offset, |entry| entry.offset)
            .map(|index| self.entries[index].value)
            .map_err(|_| ParseError::StringOffsetNotAtBoundary { offset })
    }
}

#[must_use]
#[inline]
const fn version_supported(version: u32) -> bool {
    version == SUPPORTED_VERSIONS[0] || version == SUPPORTED_VERSIONS[1]
}

#[inline]
const fn checked_add(lhs: usize, rhs: usize) -> Result<usize, ParseError> {
    match lhs.checked_add(rhs) {
        Some(value) => Ok(value),
        None => Err(ParseError::LayoutOverflow),
    }
}

#[inline]
const fn checked_mul(lhs: usize, rhs: usize) -> Result<usize, ParseError> {
    match lhs.checked_mul(rhs) {
        Some(value) => Ok(value),
        None => Err(ParseError::LayoutOverflow),
    }
}

#[inline]
fn require_range(input_len: usize, offset: usize, needed: usize) -> Result<(), ParseError> {
    let end = checked_add(offset, needed)?;
    if end > input_len {
        return Err(ParseError::UnexpectedEof {
            offset,
            needed,
            len: input_len,
        });
    }
    Ok(())
}

#[inline]
fn slice_at(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], ParseError> {
    require_range(bytes.len(), offset, len)?;
    Ok(&bytes[offset..offset + len])
}

#[inline]
fn le_u32_at(bytes: &[u8], offset: usize) -> Result<u32, ParseError> {
    Ok(le_u32(slice_at(bytes, offset, U32_SIZE)?))
}

#[must_use]
#[inline]
fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[must_use]
#[inline]
fn le_i32(bytes: &[u8]) -> i32 {
    i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
