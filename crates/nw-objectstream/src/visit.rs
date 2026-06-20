//! Streaming visitor API for ObjectStream payloads.
//!
//! [`ObjectStream::from_reader`](crate::ObjectStream::from_reader) materializes the entire
//! element tree into RAM. For large slices/prefabs that's wasteful
//! when the caller only wants to extract a few fields. This module
//! provides a SAX-style visitor: each element header is reported
//! as the parser walks it, with no allocation beyond the per-call
//! data scratch buffer.
//!
//! For the common "find one / find all" case, prefer the higher-
//! level [`crate::query`] helpers — they wrap this module and avoid
//! the boilerplate of a custom visitor struct.
//!
//! ## Typed root schemas
//!
//! When an asset is just “one reflected root object + a fixed set of CRC fields”, prefer
//! [`StreamField`], [`StreamingPartial`], and [`StreamObjectVisitor`] over another bespoke
//! [`ElementVisitor`]. [`present`] and [`once`] cover the two recurring bits of glue:
//! required [`Option`] fields and duplicate-checked assignment into a slot.
//!
//! # Example
//!
//! ```no_run
//! use nw_objectstream::ObjectStreamError;
//! use nw_objectstream::visit::{ElementHeader, ElementVisitor, VisitFlow, parse_streaming_bytes};
//! use uuid::Uuid;
//!
//! struct CountByType {
//!     counts: std::collections::HashMap<Uuid, usize>,
//! }
//!
//! impl ElementVisitor for CountByType {
//!     type Error = ObjectStreamError;
//!     fn open_element(&mut self, header: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
//!         *self.counts.entry(header.id).or_default() += 1;
//!         Ok(VisitFlow::Continue)
//!     }
//! }
//!
//! let bytes = std::fs::read("some.slice").unwrap();
//! let mut counter = CountByType { counts: Default::default() };
//! let _version = parse_streaming_bytes(&bytes, None, &mut counter).unwrap();
//! ```

use std::io::{self, Cursor, Read};

use arcstr::ArcStr;
use thiserror::Error;
use uuid::Uuid;

use crate::ObjectStreamError;
use crate::binary::{BinaryElement, read_element_header, read_stream_header};
use crate::lookup::NameLookup;
use crate::value::{DecodeAzValue, ObjectStreamValueError};

/// Header information for one element. The `data` slice is borrowed
/// from a per-call scratch buffer and is invalidated after
/// `open_element` returns; if you need to keep it, copy it. The
/// [`ArcStr`] handles for `name` / `field` are owned by the
/// [`NameLookup`] table and can be cheaply cloned to outlive
/// this header.
#[derive(Debug)]
pub struct ElementHeader<'a> {
    pub flags: u8,
    pub name_crc: Option<u32>,
    pub version: Option<u8>,
    pub id: Uuid,
    pub specialization: Option<Uuid>,
    /// Resolved type name from the [`NameLookup`] dump (if
    /// supplied to the parser); `None` if unknown.
    pub name: Option<&'a ArcStr>,
    /// Resolved field name from the [`NameLookup`] dump.
    pub field: Option<&'a ArcStr>,
    /// Inline payload bytes. Empty when this element has no value.
    pub data: &'a [u8],
}

impl<'a> ElementHeader<'a> {
    /// Decode this header's leaf payload as an AZ reflected value.
    pub fn decode<T>(&'a self) -> Result<T, crate::value::ObjectStreamValueError>
    where
        T: crate::value::DecodeAzValue<'a>,
    {
        T::decode_az_value(self)
    }

    /// Read this header as a typed field value, validating the reflected type first.
    pub fn value_as<T>(
        &'a self,
        expected: Uuid,
        expected_name: &'static str,
    ) -> Result<T, crate::value::ObjectStreamValueError>
    where
        T: crate::value::DecodeAzValue<'a>,
    {
        if self.id != expected {
            return Err(crate::value::ObjectStreamValueError::UnexpectedType {
                field: self
                    .field
                    .map_or_else(|| "<unnamed>".to_string(), ToString::to_string),
                expected: expected_name,
                actual: self.id,
            });
        }
        self.decode()
    }
}

/// Whether the visitor wants to descend into an element's children
/// — and whether it wants to keep walking the rest of the tree at
/// all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitFlow {
    /// Recurse into children, then continue with siblings.
    Continue,
    /// Skip children — the parser advances past them silently —
    /// then continue with siblings.
    Skip,
    /// Stop the walk entirely. No further `open_element` /
    /// `close_element` callbacks will fire.
    Stop,
}

/// Streaming visitor invoked by [`parse_streaming`] /
/// [`parse_streaming_bytes`].
pub trait ElementVisitor {
    type Error: From<ObjectStreamError>;

    /// Called when an element header is read.
    fn open_element(&mut self, header: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error>;

    /// Called after all of an element's children have been visited
    /// (or skipped). Default is a no-op. Not called for elements
    /// where `open_element` returned [`VisitFlow::Stop`].
    fn close_element(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum StreamingObjectError {
    #[error("objectstream parse error")]
    ObjectStream(#[from] ObjectStreamError),
    #[error("objectstream value error")]
    Value(#[from] ObjectStreamValueError),
    #[error("{owner} is missing {field}")]
    MissingField {
        owner: &'static str,
        field: &'static str,
    },
    #[error("missing {owner} root")]
    MissingRoot { owner: &'static str },
    #[error("multiple {owner} roots")]
    MultipleRoots { owner: &'static str },
    #[error("{owner} root has unexpected type {actual}")]
    UnexpectedRootType { owner: &'static str, actual: Uuid },
    #[error("{owner} has unexpected field name CRC {name_crc:?} with type {actual}")]
    UnexpectedField {
        owner: &'static str,
        actual: Uuid,
        name_crc: Option<u32>,
    },
    #[error("{owner} has unexpected nested type {actual} for name CRC {name_crc:?}")]
    UnexpectedNestedType {
        owner: &'static str,
        actual: Uuid,
        name_crc: Option<u32>,
    },
    #[error("{owner} has duplicate {field}")]
    DuplicateField {
        owner: &'static str,
        field: &'static str,
    },
}

/// Accumulator filled during a streaming walk, then [`finalize`](StreamingPartial::finalize)d
/// into the public Rust type (same role as serde’s internal “partial state” before `deserialize`
/// returns).
pub trait StreamingPartial: Sized + 'static {
    type Output;

    fn finalize(self) -> Result<Self::Output, StreamingObjectError>;
}

#[derive(Clone, Copy)]
pub struct StreamField<T> {
    pub name_crc: u32,
    pub field: &'static str,
    pub read: for<'a> fn(&mut T, &'a ElementHeader<'a>) -> Result<(), StreamingObjectError>,
}

impl<T> StreamField<T> {
    #[inline]
    #[must_use]
    pub const fn new(
        name_crc: u32,
        field: &'static str,
        read: for<'a> fn(&mut T, &'a ElementHeader<'a>) -> Result<(), StreamingObjectError>,
    ) -> Self {
        Self {
            name_crc,
            field,
            read,
        }
    }
}

pub struct StreamObjectVisitor<T: 'static> {
    owner: &'static str,
    root_type: Uuid,
    fields: &'static [StreamField<T>],
    value: T,
    roots: usize,
    depth: usize,
}

impl<T: 'static> StreamObjectVisitor<T> {
    #[inline]
    #[must_use]
    pub const fn new(
        owner: &'static str,
        root_type: Uuid,
        fields: &'static [StreamField<T>],
        value: T,
    ) -> Self {
        Self {
            owner,
            root_type,
            fields,
            value,
            roots: 0,
            depth: 0,
        }
    }

    pub fn into_value(self) -> Result<T, StreamingObjectError> {
        if self.roots == 0 {
            return Err(StreamingObjectError::MissingRoot { owner: self.owner });
        }
        Ok(self.value)
    }
}

pub struct StreamNestedObjectVisitor<T: 'static, C: StreamingPartial> {
    owner: &'static str,
    root_type: Uuid,
    child_owner: &'static str,
    child_field_crc: u32,
    child_field: &'static str,
    child_type: Uuid,
    child_fields: &'static [StreamField<C>],
    value: T,
    child: Option<C>,
    new_child: fn() -> C,
    finish_child: fn(&mut T, C::Output) -> Result<(), StreamingObjectError>,
    roots: usize,
    depth: usize,
}

pub struct NestedObjectSchema<T: 'static, C: StreamingPartial> {
    pub owner: &'static str,
    pub root_type: Uuid,
    pub child_owner: &'static str,
    pub child_field_crc: u32,
    pub child_field: &'static str,
    pub child_type: Uuid,
    pub child_fields: &'static [StreamField<C>],
    pub new_child: fn() -> C,
    pub finish_child: fn(&mut T, C::Output) -> Result<(), StreamingObjectError>,
}

impl<T: 'static, C: StreamingPartial> StreamNestedObjectVisitor<T, C> {
    #[inline]
    #[must_use]
    pub const fn new(schema: &NestedObjectSchema<T, C>, value: T) -> Self {
        Self {
            owner: schema.owner,
            root_type: schema.root_type,
            child_owner: schema.child_owner,
            child_field_crc: schema.child_field_crc,
            child_field: schema.child_field,
            child_type: schema.child_type,
            child_fields: schema.child_fields,
            value,
            child: None,
            new_child: schema.new_child,
            finish_child: schema.finish_child,
            roots: 0,
            depth: 0,
        }
    }

    pub fn into_value(self) -> Result<T, StreamingObjectError> {
        if self.roots == 0 {
            return Err(StreamingObjectError::MissingRoot { owner: self.owner });
        }
        Ok(self.value)
    }
}

impl<T: StreamingPartial> StreamObjectVisitor<T> {
    pub fn into_output(self) -> Result<T::Output, StreamingObjectError> {
        self.into_value()?.finalize()
    }
}

impl<T: StreamingPartial, C: StreamingPartial> StreamNestedObjectVisitor<T, C> {
    pub fn into_output(self) -> Result<T::Output, StreamingObjectError> {
        self.into_value()?.finalize()
    }
}

impl<T: 'static> ElementVisitor for StreamObjectVisitor<T> {
    type Error = StreamingObjectError;

    fn open_element(&mut self, header: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
        match self.depth {
            0 => {
                self.roots += 1;
                if self.roots > 1 {
                    return Err(StreamingObjectError::MultipleRoots { owner: self.owner });
                }
                if header.id != self.root_type {
                    return Err(StreamingObjectError::UnexpectedRootType {
                        owner: self.owner,
                        actual: header.id,
                    });
                }
            }
            1 => {
                let Some(field_crc) = header.name_crc else {
                    return Err(StreamingObjectError::UnexpectedField {
                        owner: self.owner,
                        actual: header.id,
                        name_crc: header.name_crc,
                    });
                };
                let Some(field) = self.fields.iter().find(|field| field.name_crc == field_crc)
                else {
                    return Err(StreamingObjectError::UnexpectedField {
                        owner: self.owner,
                        actual: header.id,
                        name_crc: header.name_crc,
                    });
                };
                (field.read)(&mut self.value, header)?;
            }
            _ => {
                return Err(StreamingObjectError::UnexpectedNestedType {
                    owner: self.owner,
                    actual: header.id,
                    name_crc: header.name_crc,
                });
            }
        }

        self.depth += 1;
        Ok(VisitFlow::Continue)
    }

    fn close_element(&mut self) -> Result<(), Self::Error> {
        self.depth = self.depth.saturating_sub(1);
        Ok(())
    }
}

impl<T: 'static, C: StreamingPartial> ElementVisitor for StreamNestedObjectVisitor<T, C> {
    type Error = StreamingObjectError;

    fn open_element(&mut self, header: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
        match self.depth {
            0 => {
                self.roots += 1;
                if self.roots > 1 {
                    return Err(StreamingObjectError::MultipleRoots { owner: self.owner });
                }
                if header.id != self.root_type {
                    return Err(StreamingObjectError::UnexpectedRootType {
                        owner: self.owner,
                        actual: header.id,
                    });
                }
            }
            1 => {
                if header.name_crc != Some(self.child_field_crc) || header.id != self.child_type {
                    return Err(StreamingObjectError::UnexpectedField {
                        owner: self.owner,
                        actual: header.id,
                        name_crc: header.name_crc,
                    });
                }
                if self.child.is_some() {
                    return Err(StreamingObjectError::DuplicateField {
                        owner: self.owner,
                        field: self.child_field,
                    });
                }
                self.child = Some((self.new_child)());
            }
            2 => {
                let Some(child) = self.child.as_mut() else {
                    return Err(StreamingObjectError::UnexpectedNestedType {
                        owner: self.child_owner,
                        actual: header.id,
                        name_crc: header.name_crc,
                    });
                };
                let Some(field_crc) = header.name_crc else {
                    return Err(StreamingObjectError::UnexpectedField {
                        owner: self.child_owner,
                        actual: header.id,
                        name_crc: header.name_crc,
                    });
                };
                let Some(field) = self
                    .child_fields
                    .iter()
                    .find(|field| field.name_crc == field_crc)
                else {
                    return Err(StreamingObjectError::UnexpectedField {
                        owner: self.child_owner,
                        actual: header.id,
                        name_crc: header.name_crc,
                    });
                };
                (field.read)(child, header)?;
            }
            _ => {
                return Err(StreamingObjectError::UnexpectedNestedType {
                    owner: self.child_owner,
                    actual: header.id,
                    name_crc: header.name_crc,
                });
            }
        }

        self.depth += 1;
        Ok(VisitFlow::Continue)
    }

    fn close_element(&mut self) -> Result<(), Self::Error> {
        let old_depth = self.depth;
        self.depth = self.depth.saturating_sub(1);
        if old_depth == 2 {
            let Some(child) = self.child.take() else {
                return Err(StreamingObjectError::MissingRoot {
                    owner: self.child_owner,
                });
            };
            (self.finish_child)(&mut self.value, child.finalize()?)?;
        }
        Ok(())
    }
}

pub fn stream_value<'a, T>(
    header: &'a ElementHeader<'a>,
    expected: Uuid,
    expected_name: &'static str,
) -> Result<T, StreamingObjectError>
where
    T: DecodeAzValue<'a>,
{
    header.value_as(expected, expected_name).map_err(Into::into)
}

/// Assign `value` into `slot`, returning [`StreamingObjectError::DuplicateField`] if it was
/// already set.
pub fn once<T>(
    slot: &mut Option<T>,
    value: T,
    owner: &'static str,
    field: &'static str,
) -> Result<(), StreamingObjectError> {
    if slot.replace(value).is_some() {
        return Err(StreamingObjectError::DuplicateField { owner, field });
    }
    Ok(())
}

/// [`Some`] value, or [`StreamingObjectError::MissingField`].
pub fn present<T>(
    value: Option<T>,
    owner: &'static str,
    field: &'static str,
) -> Result<T, StreamingObjectError> {
    value.ok_or(StreamingObjectError::MissingField { owner, field })
}

/// Parse an ObjectStream in streaming mode, calling `visitor` for
/// each element. Returns the stream's `version` field on success.
pub fn parse_streaming<R: Read, V: ElementVisitor>(
    reader: &mut R,
    hashes: Option<&NameLookup>,
    visitor: &mut V,
) -> Result<u32, V::Error> {
    let version = read_stream_header(reader)?;
    let mut data_buf = Vec::new();
    let mut stack = Vec::new();
    let mut hidden_depth = 0usize;

    loop {
        match read_element_header(reader, version, hashes)? {
            BinaryElement::Header(header) => {
                data_buf.clear();
                let data_size = header.data_size.unwrap_or(0);
                data_buf.resize(data_size, 0);
                if data_size > 0 {
                    reader.read_exact(&mut data_buf).map_err(io_err)?;
                }

                if hidden_depth > 0 {
                    stack.push(WalkFrame {
                        close_user_element: false,
                        hides_descendants: true,
                    });
                    hidden_depth += 1;
                    continue;
                }

                let flow = {
                    let header = ElementHeader {
                        flags: header.flags,
                        name_crc: header.name_crc,
                        version: header.version,
                        id: header.id,
                        specialization: header.specialization,
                        name: header.name,
                        field: header.field,
                        data: data_buf.as_slice(),
                    };
                    visitor.open_element(&header)?
                };

                match flow {
                    VisitFlow::Continue => stack.push(WalkFrame {
                        close_user_element: true,
                        hides_descendants: false,
                    }),
                    VisitFlow::Skip => {
                        stack.push(WalkFrame {
                            close_user_element: true,
                            hides_descendants: true,
                        });
                        hidden_depth += 1;
                    }
                    VisitFlow::Stop => return Ok(version),
                }
            }
            BinaryElement::EndOfList => {
                let Some(frame) = stack.pop() else {
                    break;
                };
                if frame.hides_descendants {
                    hidden_depth -= 1;
                }
                if frame.close_user_element {
                    visitor.close_element()?;
                }
            }
        }
    }
    Ok(version)
}

/// Convenience: streaming parse from a `&[u8]`.
#[inline]
pub fn parse_streaming_bytes<V: ElementVisitor>(
    bytes: &[u8],
    hashes: Option<&NameLookup>,
    visitor: &mut V,
) -> Result<u32, V::Error> {
    let mut cursor = Cursor::new(bytes);
    parse_streaming(&mut cursor, hashes, visitor)
}

#[derive(Debug, Clone, Copy)]
struct WalkFrame {
    close_user_element: bool,
    hides_descendants: bool,
}

#[inline]
fn io_err(e: io::Error) -> ObjectStreamError {
    ObjectStreamError::Io(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, ObjectStream, StreamTag};

    #[derive(Default)]
    struct CountVisitor {
        opens: usize,
        closes: usize,
    }

    impl ElementVisitor for CountVisitor {
        type Error = ObjectStreamError;
        fn open_element(&mut self, _h: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
            self.opens += 1;
            Ok(VisitFlow::Continue)
        }
        fn close_element(&mut self) -> Result<(), Self::Error> {
            self.closes += 1;
            Ok(())
        }
    }

    struct StopAtSecond {
        seen: usize,
    }

    impl ElementVisitor for StopAtSecond {
        type Error = ObjectStreamError;

        fn open_element(&mut self, _h: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
            self.seen += 1;
            if self.seen == 2 {
                Ok(VisitFlow::Stop)
            } else {
                Ok(VisitFlow::Continue)
            }
        }
    }

    #[derive(Default)]
    struct Payloads(Vec<Vec<u8>>);

    impl ElementVisitor for Payloads {
        type Error = ObjectStreamError;

        fn open_element(&mut self, h: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
            self.0.push(h.data.to_vec());
            Ok(VisitFlow::Continue)
        }
    }

    #[test]
    fn empty_stream_visits_nothing() -> Result<(), ObjectStreamError> {
        let stream = ObjectStream {
            tag: StreamTag::BINARY,
            version: 3,
            elements: Vec::new(),
        };
        let mut buf = Vec::new();
        stream.write_to(&mut buf)?;

        let mut visitor = CountVisitor::default();
        let v = parse_streaming_bytes(&buf, None, &mut visitor)?;
        assert_eq!(v, 3);
        assert_eq!(visitor.opens, 0);
        assert_eq!(visitor.closes, 0);
        Ok(())
    }

    #[test]
    fn stop_aborts_walk_early() -> Result<(), ObjectStreamError> {
        let stream = ObjectStream {
            tag: StreamTag::BINARY,
            version: 3,
            elements: vec![
                Element {
                    flags: crate::ST_BINARYFLAG_ELEMENT_HEADER,
                    id: Uuid::from_u128(1),
                    ..Default::default()
                },
                Element {
                    flags: crate::ST_BINARYFLAG_ELEMENT_HEADER,
                    id: Uuid::from_u128(2),
                    ..Default::default()
                },
                Element {
                    flags: crate::ST_BINARYFLAG_ELEMENT_HEADER,
                    id: Uuid::from_u128(3),
                    ..Default::default()
                },
            ],
        };
        let mut buf = Vec::new();
        stream.write_to(&mut buf)?;

        let mut v = StopAtSecond { seen: 0 };
        parse_streaming_bytes(&buf, None, &mut v)?;
        assert_eq!(v.seen, 2, "should stop at second element");
        Ok(())
    }

    #[test]
    fn streaming_reuses_scratch_without_losing_payloads() -> Result<(), ObjectStreamError> {
        let stream = ObjectStream {
            tag: StreamTag::BINARY,
            version: 3,
            elements: vec![
                Element {
                    flags: crate::ST_BINARYFLAG_ELEMENT_HEADER | crate::ST_BINARYFLAG_HAS_VALUE | 3,
                    id: Uuid::from_u128(1),
                    data: Some(vec![1, 2, 3]),
                    ..Default::default()
                },
                Element {
                    flags: crate::ST_BINARYFLAG_ELEMENT_HEADER | crate::ST_BINARYFLAG_HAS_VALUE | 1,
                    id: Uuid::from_u128(2),
                    data: Some(vec![4]),
                    ..Default::default()
                },
            ],
        };
        let mut buf = Vec::new();
        stream.write_to(&mut buf)?;

        let mut payloads = Payloads::default();
        parse_streaming_bytes(&buf, None, &mut payloads)?;
        assert_eq!(payloads.0, vec![vec![1, 2, 3], vec![4]]);
        Ok(())
    }

    #[test]
    fn streaming_reader_handles_deep_objectstream_without_recursion()
    -> Result<(), ObjectStreamError> {
        let bytes = deep_binary_chain(20_000);
        let mut visitor = CountVisitor::default();

        let version = parse_streaming_bytes(&bytes, None, &mut visitor)?;

        assert_eq!(version, 3);
        assert_eq!(visitor.opens, 20_000);
        assert_eq!(visitor.closes, 20_000);
        Ok(())
    }

    fn deep_binary_chain(depth: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(5 + depth * 17 + depth + 1);
        bytes.push(StreamTag::BINARY.0);
        bytes.extend_from_slice(&3u32.to_be_bytes());
        let id = Uuid::nil();
        for _ in 0..depth {
            bytes.push(crate::ST_BINARYFLAG_ELEMENT_HEADER);
            bytes.extend_from_slice(id.as_bytes());
        }
        bytes.extend(std::iter::repeat_n(0, depth + 1));
        bytes
    }
}
