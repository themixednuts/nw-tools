//! Lightweight ObjectStream inspection helpers.
//!
//! This module owns format-level traversal/stat collection so tools
//! can report on ObjectStream payloads without reimplementing parser
//! details.

use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use crate::lookup::NameLookup;
use crate::visit::{ElementHeader, ElementVisitor, VisitFlow, parse_streaming_bytes};
use crate::{Element, ObjectStream, ObjectStreamError, StreamTag};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectStreamStatsMode {
    /// Binary payload walked through the streaming visitor.
    Streaming,
    /// XML/JSON payload parsed through the DOM shape first.
    Dom,
}

impl ObjectStreamStatsMode {
    #[inline]
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Streaming => "streaming",
            Self::Dom => "DOM",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectStreamInspectionMode {
    /// Inspect with the streaming visitor where possible.
    Streaming,
    /// Parse the full DOM and print a bounded element listing.
    Dom { limit: usize },
}

impl ObjectStreamInspectionMode {
    #[inline]
    #[must_use]
    pub const fn streaming() -> Self {
        Self::Streaming
    }

    #[inline]
    #[must_use]
    pub const fn dom(limit: usize) -> Self {
        Self::Dom { limit }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectStreamStats {
    pub mode: ObjectStreamStatsMode,
    pub tag: StreamTag,
    pub version: u32,
    pub elements: u64,
    pub max_depth: usize,
    pub bytes: usize,
    pub resolved_types: u64,
    pub resolved_fields: u64,
}

impl ObjectStreamStats {
    #[inline]
    #[must_use]
    pub const fn mode_label(&self) -> &'static str {
        self.mode.label()
    }

    #[inline]
    #[must_use]
    pub const fn report(&self, hashes_available: bool) -> ObjectStreamStatsReport<'_> {
        ObjectStreamStatsReport {
            stats: self,
            hashes_available,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectStreamStatsReport<'a> {
    stats: &'a ObjectStreamStats,
    hashes_available: bool,
}

impl fmt::Display for ObjectStreamStatsReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stats = self.stats;
        writeln!(f, "  version:   {}", stats.version)?;
        writeln!(f, "  elements:  {}", stats.elements)?;
        writeln!(f, "  max depth: {}", stats.max_depth)?;
        writeln!(f, "  bytes:     {}", stats.bytes)?;
        if self.hashes_available {
            writeln!(
                f,
                "  resolved:  {} elements had a known type, {} fields had a known name",
                stats.resolved_types, stats.resolved_fields
            )
        } else {
            writeln!(f, "  resolved:  (no serialize.json - names unresolved)")
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectStreamFileStatsReport<'a> {
    pub path: &'a Path,
    pub stats: ObjectStreamStats,
    pub hashes_available: bool,
}

impl fmt::Display for ObjectStreamFileStatsReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} ({})", self.path.display(), self.stats.mode_label())?;
        write!(f, "{}", self.stats.report(self.hashes_available))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStreamDomInspection {
    pub version: u32,
    pub top_level_elements: usize,
    pub total_elements: usize,
    pub rows: Vec<ObjectStreamDomRow>,
}

impl ObjectStreamDomInspection {
    #[must_use]
    pub fn remaining_elements(&self) -> usize {
        self.total_elements.saturating_sub(self.rows.len())
    }

    #[inline]
    #[must_use]
    pub const fn report(&self) -> ObjectStreamDomInspectionReport<'_> {
        ObjectStreamDomInspectionReport { inspection: self }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStreamDomRow {
    pub index: usize,
    pub flags: u8,
    pub id: Uuid,
    pub type_name: String,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectStreamDomInspectionReport<'a> {
    inspection: &'a ObjectStreamDomInspection,
}

impl fmt::Display for ObjectStreamDomInspectionReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inspection = self.inspection;
        writeln!(f, "  version: {}", inspection.version)?;
        writeln!(f, "  top-level elements: {}", inspection.top_level_elements)?;
        writeln!(f)?;
        for row in &inspection.rows {
            let field = if let Some(field) = &row.field {
                format!(" field={field}")
            } else {
                String::new()
            };
            writeln!(
                f,
                "  [{:>5}] flags={:#04x} id={} {}{}",
                row.index, row.flags, row.id, row.type_name, field
            )?;
        }
        let remaining = inspection.remaining_elements();
        if remaining > 0 {
            writeln!(f, "  ... ({remaining} more)")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStreamFileDomReport<'a> {
    pub path: &'a Path,
    pub inspection: ObjectStreamDomInspection,
}

impl fmt::Display for ObjectStreamFileDomReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} (DOM)", self.path.display())?;
        write!(f, "{}", self.inspection.report())
    }
}

#[derive(Debug, Clone)]
pub enum ObjectStreamFileInspectionReport<'a> {
    Streaming(ObjectStreamFileStatsReport<'a>),
    Dom(ObjectStreamFileDomReport<'a>),
}

impl fmt::Display for ObjectStreamFileInspectionReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Streaming(report) => report.fmt(f),
            Self::Dom(report) => report.fmt(f),
        }
    }
}

impl ObjectStreamStats {
    #[must_use]
    pub fn from_stream(tag: StreamTag, bytes: usize, stream: &ObjectStream) -> Self {
        let mut stats = Self {
            mode: ObjectStreamStatsMode::Dom,
            tag,
            version: stream.version(),
            elements: 0,
            max_depth: 0,
            bytes,
            resolved_types: 0,
            resolved_fields: 0,
        };

        for element in stream.elements() {
            stats.visit_dom_element(element, 1);
        }
        stats
    }

    fn visit_dom_element(&mut self, element: &Element, depth: usize) {
        self.elements += 1;
        if !element.name().is_empty() {
            self.resolved_types += 1;
        }
        if element.field().is_some() {
            self.resolved_fields += 1;
        }
        self.max_depth = self.max_depth.max(depth);

        for child in &element.elements {
            self.visit_dom_element(child, depth + 1);
        }
    }
}

/// Parse an ObjectStream into a bounded, display-ready DOM inspection.
pub fn inspect_dom_bytes(
    bytes: &[u8],
    limit: usize,
    hashes: Option<&NameLookup>,
) -> Result<ObjectStreamDomInspection, ObjectStreamError> {
    let stream = ObjectStream::from_bytes(bytes, hashes)?;
    Ok(inspect_dom_stream(&stream, limit))
}

/// Parse and format a bounded ObjectStream DOM inspection for a file path.
pub fn inspect_dom_file_bytes<'a>(
    path: &'a Path,
    bytes: &[u8],
    limit: usize,
    hashes: Option<&NameLookup>,
) -> Result<ObjectStreamFileDomReport<'a>, ObjectStreamError> {
    inspect_dom_bytes(bytes, limit, hashes)
        .map(|inspection| ObjectStreamFileDomReport { path, inspection })
}

/// Build a bounded, display-ready DOM inspection from a parsed stream.
#[must_use]
pub fn inspect_dom_stream(stream: &ObjectStream, limit: usize) -> ObjectStreamDomInspection {
    let mut rows = Vec::new();
    let mut total_elements = 0usize;
    for (index, element) in stream.iter_recursive().enumerate() {
        total_elements += 1;
        if rows.len() < limit {
            rows.push(ObjectStreamDomRow::from_element(index, element));
        }
    }

    ObjectStreamDomInspection {
        version: stream.version(),
        top_level_elements: stream.elements().len(),
        total_elements,
        rows,
    }
}

impl ObjectStreamDomRow {
    #[must_use]
    pub fn from_element(index: usize, element: &Element) -> Self {
        let type_name = if element.name().is_empty() {
            "<unknown-type>".to_string()
        } else {
            element.name().to_string()
        };
        let field = element.field().map(arcstr::ArcStr::to_string);
        Self {
            index,
            flags: element.flags,
            id: *element.id(),
            type_name,
            field,
        }
    }
}

/// Inspect binary ObjectStream payloads through the streaming parser,
/// and XML/JSON payloads through the DOM parser.
pub fn inspect_bytes(
    bytes: &[u8],
    hashes: Option<&NameLookup>,
) -> Result<ObjectStreamStats, ObjectStreamError> {
    let Some((&first, _)) = bytes.split_first() else {
        return Err(ObjectStreamError::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "empty ObjectStream payload",
        )));
    };

    let tag = StreamTag::from_byte(first).ok_or(ObjectStreamError::InvalidStreamTag(first))?;
    if tag.is_binary() {
        let mut stats = StreamingStats {
            inner: ObjectStreamStats {
                mode: ObjectStreamStatsMode::Streaming,
                tag,
                version: 0,
                elements: 0,
                max_depth: 0,
                bytes: bytes.len(),
                resolved_types: 0,
                resolved_fields: 0,
            },
            depth: 0,
        };
        stats.inner.version = parse_streaming_bytes(bytes, hashes, &mut stats)?;
        Ok(stats.inner)
    } else {
        let stream = ObjectStream::from_bytes(bytes, hashes)?;
        Ok(ObjectStreamStats::from_stream(tag, bytes.len(), &stream))
    }
}

/// Inspect an ObjectStream payload and pair the report with its source path.
pub fn inspect_file_bytes<'a>(
    path: &'a Path,
    bytes: &[u8],
    hashes: Option<&NameLookup>,
) -> Result<ObjectStreamFileStatsReport<'a>, ObjectStreamError> {
    inspect_bytes(bytes, hashes).map(|stats| ObjectStreamFileStatsReport {
        path,
        stats,
        hashes_available: hashes.is_some(),
    })
}

/// Inspect an ObjectStream payload using a caller-selected report mode.
pub fn inspect_file_bytes_with_mode<'a>(
    path: &'a Path,
    bytes: &[u8],
    mode: ObjectStreamInspectionMode,
    hashes: Option<&NameLookup>,
) -> Result<ObjectStreamFileInspectionReport<'a>, ObjectStreamError> {
    match mode {
        ObjectStreamInspectionMode::Streaming => {
            inspect_file_bytes(path, bytes, hashes).map(ObjectStreamFileInspectionReport::Streaming)
        }
        ObjectStreamInspectionMode::Dom { limit } => {
            inspect_dom_file_bytes(path, bytes, limit, hashes)
                .map(ObjectStreamFileInspectionReport::Dom)
        }
    }
}

#[derive(Debug, Error)]
pub enum ObjectStreamFileInspectionError {
    #[error("read ObjectStream asset {path:?}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("parse ObjectStream asset {path:?}")]
    Parse {
        path: PathBuf,
        #[source]
        source: ObjectStreamError,
    },
}

pub fn inspect_file_path_with_mode<'a>(
    path: &'a Path,
    mode: ObjectStreamInspectionMode,
    hashes: Option<&NameLookup>,
) -> Result<ObjectStreamFileInspectionReport<'a>, ObjectStreamFileInspectionError> {
    let bytes = std::fs::read(path).map_err(|source| ObjectStreamFileInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    inspect_file_bytes_with_mode(path, &bytes, mode, hashes).map_err(|source| {
        ObjectStreamFileInspectionError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })
}

struct StreamingStats {
    inner: ObjectStreamStats,
    depth: usize,
}

impl ElementVisitor for StreamingStats {
    type Error = ObjectStreamError;

    #[inline]
    fn open_element(&mut self, header: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
        self.inner.elements += 1;
        if header.name.is_some() {
            self.inner.resolved_types += 1;
        }
        if header.field.is_some() {
            self.inner.resolved_fields += 1;
        }
        self.depth += 1;
        self.inner.max_depth = self.inner.max_depth.max(self.depth);
        Ok(VisitFlow::Continue)
    }

    #[inline]
    fn close_element(&mut self) -> Result<(), Self::Error> {
        self.depth = self.depth.saturating_sub(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AZSTD_STRING;

    #[test]
    fn dom_inspection_limits_rows_and_reports_remainder() {
        let mut stream = ObjectStream::new(3);
        stream.elements = vec![
            Element {
                name: "AZStd::string".into(),
                ..Element::new(AZSTD_STRING)
            },
            Element {
                name: "AZStd::string".into(),
                ..Element::new(AZSTD_STRING)
            },
        ];

        let inspection = inspect_dom_stream(&stream, 1);

        assert_eq!(inspection.version, 3);
        assert_eq!(inspection.top_level_elements, 2);
        assert_eq!(inspection.total_elements, 2);
        assert_eq!(inspection.remaining_elements(), 1);
        assert_eq!(inspection.rows.len(), 1);
        assert_eq!(inspection.rows[0].type_name, "AZStd::string");

        let report = inspection.report().to_string();
        assert!(report.contains("top-level elements: 2"));
        assert!(report.contains("... (1 more)"));
    }

    #[test]
    fn stats_report_notes_hash_availability() {
        let stats = ObjectStreamStats {
            mode: ObjectStreamStatsMode::Streaming,
            tag: StreamTag::BINARY,
            version: 3,
            elements: 10,
            max_depth: 2,
            bytes: 128,
            resolved_types: 4,
            resolved_fields: 3,
        };

        assert_eq!(stats.mode_label(), "streaming");
        let resolved = stats.report(true).to_string();
        assert!(resolved.contains("4 elements had a known type"));

        let unresolved = stats.report(false).to_string();
        assert!(unresolved.contains("no serialize.json"));
    }

    #[test]
    fn file_stats_report_includes_path_and_mode() {
        let bytes = ObjectStream::new(3).to_bytes();
        let report = inspect_file_bytes(Path::new("slices/foo.slice"), &bytes, None).unwrap();

        assert_eq!(
            report.to_string(),
            "slices/foo.slice (streaming)\n  version:   3\n  elements:  0\n  max depth: 0\n  bytes:     6\n  resolved:  (no serialize.json - names unresolved)\n"
        );
    }

    #[test]
    fn file_dom_report_includes_path_and_dom_header() {
        let bytes = br#"<ObjectStream version="3"></ObjectStream>"#;
        let report =
            inspect_dom_file_bytes(Path::new("slices/foo.slice"), bytes, 50, None).unwrap();

        assert_eq!(
            report.to_string(),
            "slices/foo.slice (DOM)\n  version: 3\n  top-level elements: 0\n\n"
        );
    }

    #[test]
    fn file_inspection_report_uses_streaming_mode() {
        let bytes = ObjectStream::new(3).to_bytes();
        let report = inspect_file_bytes_with_mode(
            Path::new("slices/foo.slice"),
            &bytes,
            ObjectStreamInspectionMode::streaming(),
            None,
        )
        .unwrap();

        assert_eq!(
            report.to_string(),
            "slices/foo.slice (streaming)\n  version:   3\n  elements:  0\n  max depth: 0\n  bytes:     6\n  resolved:  (no serialize.json - names unresolved)\n"
        );
    }

    #[test]
    fn file_inspection_report_uses_dom_mode() {
        let bytes = br#"<ObjectStream version="3"></ObjectStream>"#;
        let report = inspect_file_bytes_with_mode(
            Path::new("slices/foo.slice"),
            bytes,
            ObjectStreamInspectionMode::dom(50),
            None,
        )
        .unwrap();

        assert_eq!(
            report.to_string(),
            "slices/foo.slice (DOM)\n  version: 3\n  top-level elements: 0\n\n"
        );
    }
}
