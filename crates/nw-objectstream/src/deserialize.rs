//! ObjectStream payload deserialization.
//!
//! This module owns the byte/XML/JSON input boundary. Callers can
//! consume the resulting [`ObjectStream`] graph or the streaming
//! visitor API.

use std::io::{self, Cursor, Read};
use std::str;

use crate::binary::{
    BinaryElement, ensure_reader_exhausted, read_element_header, read_stream_header,
};
use crate::lookup::NameLookup;
use crate::validate_stream_version;
use crate::{
    Element, JSONObjectStream, ObjectStream, ObjectStreamEncoding, ObjectStreamError, StreamTag,
    XMLObjectStream,
};

/// Read an ObjectStream binary payload from a [`Read`] source.
///
/// `hashes` is optional; if supplied, element name and field
/// resolutions are populated from caller-supplied lookup data.
pub fn from_reader<R: Read>(
    reader: &mut R,
    hashes: Option<&NameLookup>,
) -> Result<ObjectStream, ObjectStreamError> {
    let version = read_stream_header(reader)?;
    let mut stream = ObjectStream::new(version);
    let mut stack = Vec::new();

    loop {
        match read_element_header(reader, version, hashes)? {
            BinaryElement::Header(header) => {
                let mut element = Element {
                    flags: header.flags,
                    name_crc: header.name_crc,
                    version: header.version,
                    id: header.id,
                    specialization: header.specialization,
                    name: header.name.cloned().unwrap_or_default(),
                    field: header.field.cloned(),
                    data_size: header.data_size,
                    ..Default::default()
                };
                if let Some(data_size) = element.data_size {
                    let mut data = vec![0; data_size];
                    reader.read_exact(&mut data)?;
                    element.data = Some(data);
                }
                stack.push(element);
            }
            BinaryElement::EndOfList => {
                let Some(element) = stack.pop() else {
                    break;
                };

                if let Some(parent) = stack.last_mut() {
                    parent.elements.push(element);
                } else {
                    stream.elements.push(element);
                }
            }
        }
    }
    ensure_reader_exhausted(reader)?;
    Ok(stream)
}

/// Convenience: read binary, XML, or JSON ObjectStream bytes.
#[inline]
pub fn from_bytes(
    bytes: &[u8],
    hashes: Option<&NameLookup>,
) -> Result<ObjectStream, ObjectStreamError> {
    let Some((&tag, _)) = bytes.split_first() else {
        return Err(ObjectStreamError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "empty ObjectStream payload",
        )));
    };

    match StreamTag::from_byte(tag) {
        Some(StreamTag::BINARY) => {
            let mut cursor = Cursor::new(bytes);
            from_reader(&mut cursor, hashes)
        }
        Some(StreamTag::XML) => {
            let xml: XMLObjectStream = quick_xml::de::from_str(str::from_utf8(bytes)?)?;
            validate_stream_version(xml.version)?;
            Ok(xml.into())
        }
        Some(StreamTag::JSON) => {
            let json: JSONObjectStream = serde_json::from_slice(bytes)?;
            validate_stream_version(json.version)?;
            Ok(json.into())
        }
        _ => Err(ObjectStreamError::InvalidStreamTag(tag)),
    }
}

/// Read ObjectStream bytes that must use `encoding`.
#[inline]
pub fn from_encoding_bytes(
    bytes: &[u8],
    encoding: ObjectStreamEncoding,
    hashes: Option<&NameLookup>,
) -> Result<ObjectStream, ObjectStreamError> {
    let Some((&tag, _)) = bytes.split_first() else {
        return Err(ObjectStreamError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "empty ObjectStream payload",
        )));
    };
    let Some(actual) = ObjectStreamEncoding::from_tag_byte(tag) else {
        return Err(ObjectStreamError::InvalidStreamTag(tag));
    };
    if actual != encoding {
        return Err(ObjectStreamError::UnexpectedEncoding {
            expected: encoding,
            actual,
        });
    }
    from_bytes(bytes, hashes)
}
