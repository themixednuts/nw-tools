//! ObjectStream traversal statistics.

use std::io;

use crate::lookup::NameLookup;
use crate::visit::{ElementHeader, ElementVisitor, VisitFlow, parse_streaming_bytes};
use crate::{Element, ObjectStream, ObjectStreamError, StreamTag};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Binary payload walked through the streaming visitor.
    Streaming,
    /// XML/JSON payload parsed through the in-memory graph.
    Dom,
}

impl Mode {
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
pub struct Stats {
    pub mode: Mode,
    pub tag: StreamTag,
    pub version: u32,
    pub elements: u64,
    pub max_depth: usize,
    pub bytes: usize,
    pub resolved_types: u64,
    pub resolved_fields: u64,
}

impl Stats {
    #[inline]
    #[must_use]
    pub const fn mode_label(&self) -> &'static str {
        self.mode.label()
    }

    #[must_use]
    pub fn from_stream(tag: StreamTag, bytes: usize, stream: &ObjectStream) -> Self {
        let mut stats = Self {
            mode: Mode::Dom,
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

    /// Count ObjectStream elements without forcing binary payloads through
    /// the allocation-heavy graph parser.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectStreamError`] if the payload is empty, has an unknown
    /// stream tag, or cannot be parsed as its declared encoding.
    pub fn from_bytes(
        bytes: &[u8],
        lookup: Option<&NameLookup>,
    ) -> Result<Self, ObjectStreamError> {
        let Some((&first, _)) = bytes.split_first() else {
            return Err(ObjectStreamError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "empty ObjectStream payload",
            )));
        };

        let tag = StreamTag::from_byte(first).ok_or(ObjectStreamError::InvalidStreamTag(first))?;
        if tag.is_binary() {
            let mut stats = StreamingStats {
                inner: Self {
                    mode: Mode::Streaming,
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
            stats.inner.version = parse_streaming_bytes(bytes, lookup, &mut stats)?;
            Ok(stats.inner)
        } else {
            let stream = ObjectStream::from_bytes(bytes, lookup)?;
            Ok(Self::from_stream(tag, bytes.len(), &stream))
        }
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

        for child in element.children() {
            self.visit_dom_element(child, depth + 1);
        }
    }
}

struct StreamingStats {
    inner: Stats,
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
    fn stats_from_stream_counts_recursive_elements() {
        let mut stream = ObjectStream::new(3);
        stream.elements = vec![Element {
            name: "AZStd::string".into(),
            elements: vec![Element {
                name: "AZStd::string".into(),
                ..Element::new(AZSTD_STRING)
            }],
            ..Element::new(AZSTD_STRING)
        }];

        let stats = Stats::from_stream(StreamTag::BINARY, 128, &stream);

        assert_eq!(stats.mode, Mode::Dom);
        assert_eq!(stats.version, 3);
        assert_eq!(stats.elements, 2);
        assert_eq!(stats.max_depth, 2);
        assert_eq!(stats.bytes, 128);
        assert_eq!(stats.resolved_types, 2);
    }

    #[test]
    fn stats_from_bytes_uses_streaming_for_binary_payloads() {
        let bytes = ObjectStream::new(3).to_bytes();
        let stats = Stats::from_bytes(&bytes, None).unwrap();

        assert_eq!(stats.mode, Mode::Streaming);
        assert_eq!(stats.mode_label(), "streaming");
        assert_eq!(stats.version, 3);
        assert_eq!(stats.elements, 0);
        assert_eq!(stats.bytes, bytes.len());
    }
}
