//! ObjectStream serialization and transcoding.
//!
//! The canonical binary writer emits the binary ObjectStream shape.
//! XML output is written directly so very deep reflected assets do
//! not need a recursive XML mirror tree.

use std::io::{self, Cursor, Read, Write};

use quick_xml::events::{BytesEnd, BytesStart, Event};
use serde_json::Value;

use crate::binary::{
    BinaryElement, ensure_reader_exhausted, read_element_header, read_stream_header,
};
use crate::deserialize;
use crate::lookup::NameLookup;
use crate::types::uuid_data_to_serialize;
use crate::{Element, ObjectStream, ObjectStreamEncoding, ObjectStreamError, StreamTag};

const XML_OBJECT_STREAM: &str = "ObjectStream";
const XML_CLASS: &str = "Class";

/// Encode `stream` in the requested ObjectStream encoding.
pub fn to_encoding_bytes(
    stream: &ObjectStream,
    encoding: ObjectStreamEncoding,
) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_as(stream, encoding, &mut bytes)?;
    Ok(bytes)
}

/// Write `stream` in the requested ObjectStream encoding.
pub fn write_as<W: Write>(
    stream: &ObjectStream,
    encoding: ObjectStreamEncoding,
    writer: &mut W,
) -> io::Result<()> {
    match encoding {
        ObjectStreamEncoding::Binary => write_to(stream, writer),
        ObjectStreamEncoding::Xml => write_xml(stream, writer),
        ObjectStreamEncoding::Json => write_json(stream, writer),
    }
}

/// Convert ObjectStream bytes into another ObjectStream encoding.
///
/// Binary inputs are streamed into the requested output encoding so
/// deep reflected assets do not grow the call stack or require a
/// second mirror tree.
pub fn transcode_bytes(
    bytes: &[u8],
    encoding: ObjectStreamEncoding,
    hashes: Option<&NameLookup>,
) -> Result<Vec<u8>, ObjectStreamError> {
    let mut output = Vec::new();
    transcode_to_writer(bytes, encoding, hashes, &mut output)?;
    Ok(output)
}

/// Write ObjectStream bytes in another ObjectStream encoding.
pub fn transcode_to_writer<W: Write>(
    bytes: &[u8],
    encoding: ObjectStreamEncoding,
    hashes: Option<&NameLookup>,
    writer: &mut W,
) -> Result<(), ObjectStreamError> {
    let Some((&tag, _)) = bytes.split_first() else {
        return Err(ObjectStreamError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "empty ObjectStream payload",
        )));
    };

    match (StreamTag::from_byte(tag), encoding) {
        (Some(StreamTag::BINARY), ObjectStreamEncoding::Binary) => {
            validate_binary(&mut Cursor::new(bytes), hashes)?;
            writer.write_all(bytes)?;
            Ok(())
        }
        (Some(StreamTag::BINARY), ObjectStreamEncoding::Xml) => {
            write_binary_xml(&mut Cursor::new(bytes), hashes, writer)
        }
        (Some(StreamTag::BINARY), ObjectStreamEncoding::Json) => {
            write_binary_json(&mut Cursor::new(bytes), hashes, writer)
        }
        (Some(StreamTag::XML | StreamTag::JSON), encoding) => {
            let stream = deserialize::from_bytes(bytes, hashes)?;
            write_as(&stream, encoding, writer)?;
            Ok(())
        }
        _ => Err(ObjectStreamError::InvalidStreamTag(tag)),
    }
}

/// Write the binary ObjectStream form to `writer`.
pub fn write_to<W: Write>(stream: &ObjectStream, writer: &mut W) -> io::Result<()> {
    writer.write_all(&[StreamTag::BINARY.0])?;
    writer.write_all(&stream.version.to_be_bytes())?;
    let mut stack = Vec::new();
    for element in stream.elements.iter().rev() {
        stack.push(BinaryWriteFrame::Element(element));
    }

    while let Some(frame) = stack.pop() {
        match frame {
            BinaryWriteFrame::Element(element) => {
                write_element_header_and_data(element, writer)?;
                stack.push(BinaryWriteFrame::EndOfList);
                for child in element.elements.iter().rev() {
                    stack.push(BinaryWriteFrame::Element(child));
                }
            }
            BinaryWriteFrame::EndOfList => writer.write_all(&[0])?,
        }
    }
    writer.write_all(&[0])?;
    Ok(())
}

/// Convenience: encode an ObjectStream into binary ObjectStream bytes.
///
/// # Panics
///
/// Panics only if writing to the in-memory output buffer fails.
#[inline]
#[must_use]
pub fn to_bytes(stream: &ObjectStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_to(stream, &mut bytes).expect("writing ObjectStream into Vec cannot fail");
    bytes
}

#[derive(Debug, Clone, Copy)]
enum BinaryWriteFrame<'a> {
    Element(&'a Element),
    EndOfList,
}

fn write_element_header_and_data<W: Write>(element: &Element, writer: &mut W) -> io::Result<()> {
    writer.write_all(&[element.flags])?;
    if let Some(crc) = element.name_crc {
        writer.write_all(&crc.to_be_bytes())?;
    }
    if let Some(version) = element.version {
        writer.write_all(&[version])?;
    }
    writer.write_all(&element.id.as_u128().to_be_bytes())?;

    if let Some(specialized) = element.specialization {
        writer.write_all(&specialized.as_u128().to_be_bytes())?;
    }
    if element.has_value()
        && element.has_extra_size_field()
        && let Some(size) = element.data_size
    {
        match element.value_width() {
            1 => writer.write_all(
                &u8::try_from(size)
                    .map_err(|_| invalid_size_width(size, 1))?
                    .to_be_bytes(),
            )?,
            2 => writer.write_all(
                &u16::try_from(size)
                    .map_err(|_| invalid_size_width(size, 2))?
                    .to_be_bytes(),
            )?,
            4 => writer.write_all(
                &u32::try_from(size)
                    .map_err(|_| invalid_size_width(size, 4))?
                    .to_be_bytes(),
            )?,
            _ => {}
        }
    }

    if let Some(data) = &element.data {
        writer.write_all(data)?;
    }
    Ok(())
}

fn invalid_size_width(size: usize, width: u8) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("ObjectStream value size {size} does not fit in {width}-byte size field"),
    )
}

fn write_xml<W: Write>(stream: &ObjectStream, writer: &mut W) -> io::Result<()> {
    let mut xml = quick_xml::Writer::new_with_indent(writer, b'\t', 2);
    write_xml_root_start(&mut xml, stream.version)?;

    let mut stack = Vec::new();
    for element in stream.elements.iter().rev() {
        stack.push(XmlDomFrame::Element(element));
    }

    while let Some(frame) = stack.pop() {
        match frame {
            XmlDomFrame::Element(element) => {
                let attrs = XmlClassAttrs::from_element(element);
                if element.elements.is_empty() {
                    write_xml_class(&mut xml, &attrs, true, true)?;
                } else {
                    write_xml_class(&mut xml, &attrs, false, false)?;
                    stack.push(XmlDomFrame::EndOfClass);
                    for child in element.elements.iter().rev() {
                        stack.push(XmlDomFrame::Element(child));
                    }
                }
            }
            XmlDomFrame::EndOfClass => write_xml_class_end(&mut xml)?,
        }
    }

    write_xml_root_end(&mut xml)
}

#[derive(Debug, Clone, Copy)]
enum XmlDomFrame<'a> {
    Element(&'a Element),
    EndOfClass,
}

fn write_binary_xml<R: Read, W: Write>(
    reader: &mut R,
    hashes: Option<&NameLookup>,
    writer: &mut W,
) -> Result<(), ObjectStreamError> {
    let version = read_stream_header(reader)?;
    let mut xml = quick_xml::Writer::new_with_indent(writer, b'\t', 2);
    write_xml_root_start(&mut xml, version)?;

    let mut stack = Vec::new();
    let mut data = Vec::new();

    loop {
        match read_element_header(reader, version, hashes)? {
            BinaryElement::Header(header) => {
                if let Some(parent) = stack.last_mut() {
                    write_pending_xml_class(&mut xml, parent, false)?;
                }

                data.clear();
                let data_size = header.data_size.unwrap_or(0);
                data.resize(data_size, 0);
                if data_size > 0 {
                    reader.read_exact(&mut data)?;
                }

                stack.push(PendingXmlClass::from_binary_header(&header, &data));
            }
            BinaryElement::EndOfList => {
                let Some(mut class) = stack.pop() else {
                    break;
                };
                write_pending_xml_class(&mut xml, &mut class, true)?;
                if class.start_written {
                    write_xml_class_end(&mut xml)?;
                }
            }
        }
    }

    ensure_reader_exhausted(reader)?;
    write_xml_root_end(&mut xml)?;
    Ok(())
}

fn write_binary_json<R: Read, W: Write>(
    reader: &mut R,
    hashes: Option<&NameLookup>,
    writer: &mut W,
) -> Result<(), ObjectStreamError> {
    let version = read_stream_header(reader)?;
    writer.write_all(b"{")?;
    writer.write_all(b"\n  ")?;
    write_json_name(writer, "name")?;
    writer.write_all(b": ")?;
    write_json_serialized(writer, "ObjectStream")?;
    writer.write_all(b",\n  ")?;
    write_json_name(writer, "version")?;
    write!(writer, ": {version}")?;
    writer.write_all(b",\n  ")?;
    write_json_name(writer, "Objects")?;
    writer.write_all(b": [")?;

    let mut stack = Vec::new();
    let mut data = Vec::new();
    let mut root_children = 0usize;

    loop {
        match read_element_header(reader, version, hashes)? {
            BinaryElement::Header(header) => {
                let indent = if let Some(parent) = stack.last_mut() {
                    prepare_json_child_slot(writer, parent)?;
                    parent.indent + 2
                } else {
                    if root_children == 0 {
                        writer.write_all(b"\n")?;
                    } else {
                        writer.write_all(b",\n")?;
                    }
                    root_children += 1;
                    2
                };

                data.clear();
                let data_size = header.data_size.unwrap_or(0);
                data.resize(data_size, 0);
                if data_size > 0 {
                    reader.read_exact(&mut data)?;
                }

                stack.push(PendingJsonClass::from_binary_header(&header, &data, indent));
            }
            BinaryElement::EndOfList => {
                let Some(class) = stack.pop() else {
                    break;
                };

                if class.objects_open {
                    writer.write_all(b"\n")?;
                    write_indent(writer, class.indent + 1)?;
                    writer.write_all(b"]\n")?;
                    write_indent(writer, class.indent)?;
                    writer.write_all(b"}")?;
                } else {
                    write_json_class(writer, &class.attrs, class.indent, true)?;
                }
            }
        }
    }

    ensure_reader_exhausted(reader)?;
    if root_children > 0 {
        writer.write_all(b"\n  ]\n}")?;
    } else {
        writer.write_all(b"]\n}")?;
    }
    Ok(())
}

fn validate_binary<R: Read>(
    reader: &mut R,
    hashes: Option<&NameLookup>,
) -> Result<(), ObjectStreamError> {
    let version = read_stream_header(reader)?;
    let mut open_classes = 0usize;
    let mut scratch = [0u8; 8192];

    loop {
        match read_element_header(reader, version, hashes)? {
            BinaryElement::Header(header) => {
                if let Some(data_size) = header.data_size {
                    read_exact_discard(reader, data_size, &mut scratch)?;
                }
                open_classes += 1;
            }
            BinaryElement::EndOfList => {
                let Some(next) = open_classes.checked_sub(1) else {
                    break;
                };
                open_classes = next;
            }
        }
    }
    ensure_reader_exhausted(reader)?;
    Ok(())
}

fn read_exact_discard<R: Read>(
    reader: &mut R,
    mut remaining: usize,
    scratch: &mut [u8],
) -> Result<(), ObjectStreamError> {
    while remaining > 0 {
        let chunk = remaining.min(scratch.len());
        reader.read_exact(&mut scratch[..chunk])?;
        remaining -= chunk;
    }
    Ok(())
}

#[derive(Debug)]
struct PendingXmlClass {
    attrs: XmlClassAttrs,
    start_written: bool,
}

impl PendingXmlClass {
    fn from_binary_header(header: &crate::binary::BinaryElementHeader<'_>, data: &[u8]) -> Self {
        Self {
            attrs: XmlClassAttrs::from_binary_header(header, data),
            start_written: false,
        }
    }
}

fn write_pending_xml_class<W: Write>(
    xml: &mut quick_xml::Writer<W>,
    class: &mut PendingXmlClass,
    leaf: bool,
) -> io::Result<()> {
    if class.start_written {
        return Ok(());
    }
    write_xml_class(xml, &class.attrs, leaf, leaf)?;
    class.start_written = !leaf;
    Ok(())
}

#[derive(Debug)]
struct XmlClassAttrs {
    name: String,
    field: Option<String>,
    value: Option<String>,
    empty_leaf_value: Option<String>,
    version: Option<String>,
    type_id: String,
}

#[derive(Debug)]
struct PendingJsonClass {
    attrs: JsonClassAttrs,
    indent: usize,
    objects_open: bool,
    children: usize,
}

impl PendingJsonClass {
    fn from_binary_header(
        header: &crate::binary::BinaryElementHeader<'_>,
        data: &[u8],
        indent: usize,
    ) -> Self {
        Self {
            attrs: JsonClassAttrs::from_binary_header(header, data),
            indent,
            objects_open: false,
            children: 0,
        }
    }
}

fn prepare_json_child_slot<W: Write>(
    writer: &mut W,
    parent: &mut PendingJsonClass,
) -> io::Result<()> {
    if !parent.objects_open {
        write_json_class_fields(writer, &parent.attrs, parent.indent, false)?;
        writer.write_all(b",\n")?;
        write_indent(writer, parent.indent + 1)?;
        write_json_name(writer, "Objects")?;
        writer.write_all(b": [\n")?;
        parent.objects_open = true;
    } else if parent.children > 0 {
        writer.write_all(b",\n")?;
    }
    parent.children += 1;
    Ok(())
}

fn json_value_attrs(id: &uuid::Uuid, data: Option<&[u8]>) -> (Option<Value>, Option<Value>) {
    let Some(data) = data else {
        return (None, None);
    };
    if data.is_empty() {
        return (None, Some(Value::String(String::new())));
    }
    let value = uuid_data_to_serialize(id, data, true).ok().map(|value| {
        if value.is_string() {
            value
        } else {
            Value::String(value.to_string())
        }
    });
    (value, None)
}

#[derive(Debug)]
struct JsonClassAttrs {
    field: Option<String>,
    type_id: String,
    name: String,
    specialization: Option<String>,
    value: Option<Value>,
    empty_leaf_value: Option<Value>,
    version: Option<u8>,
    data_present: bool,
}

impl JsonClassAttrs {
    fn from_element(element: &Element) -> Self {
        let (value, empty_leaf_value) = json_value_attrs(&element.id, element.data.as_deref());
        Self {
            field: element.field.as_ref().map(ToString::to_string),
            type_id: uuid_xml_attr(&element.id),
            name: element.name.to_string(),
            specialization: element.specialization.map(|uuid| uuid_xml_attr(&uuid)),
            value,
            empty_leaf_value,
            version: element.version,
            data_present: element.data.is_some(),
        }
    }

    fn from_binary_header(header: &crate::binary::BinaryElementHeader<'_>, data: &[u8]) -> Self {
        let data = header.data_size.map(|_| data);
        let (value, empty_leaf_value) = json_value_attrs(&header.id, data);
        Self {
            field: header.field.map(ToString::to_string),
            type_id: uuid_xml_attr(&header.id),
            name: header.name.map_or_else(String::new, ToString::to_string),
            specialization: header.specialization.map(|uuid| uuid_xml_attr(&uuid)),
            value,
            empty_leaf_value,
            version: header.version,
            data_present: data.is_some(),
        }
    }
}

impl XmlClassAttrs {
    fn from_element(element: &Element) -> Self {
        let (value, empty_leaf_value) = xml_value_attrs(
            &element.id,
            element.data.as_deref(),
            element.elements.is_empty(),
        );
        Self {
            name: element.name.to_string(),
            field: element.field.as_ref().map(ToString::to_string),
            value,
            empty_leaf_value,
            version: element.version.map(|version| version.to_string()),
            type_id: uuid_xml_attr(&element.id),
        }
    }

    fn from_binary_header(header: &crate::binary::BinaryElementHeader<'_>, data: &[u8]) -> Self {
        let data = header.data_size.map(|_| data);
        let (value, empty_leaf_value) = xml_value_attrs(&header.id, data, false);
        Self {
            name: header.name.map_or_else(String::new, ToString::to_string),
            field: header.field.map(ToString::to_string),
            value,
            empty_leaf_value,
            version: header.version.map(|version| version.to_string()),
            type_id: uuid_xml_attr(&header.id),
        }
    }
}

fn xml_value_attrs(
    id: &uuid::Uuid,
    data: Option<&[u8]>,
    known_leaf: bool,
) -> (Option<String>, Option<String>) {
    let Some(data) = data else {
        return (None, None);
    };
    let value = uuid_data_to_serialize(id, data, false)
        .ok()
        .map(xml_value_to_attr);
    if data.is_empty() && !known_leaf {
        (None, value)
    } else {
        (value, None)
    }
}

fn xml_value_to_attr(value: Value) -> String {
    match value {
        Value::String(value) => value,
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => {
            if value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        other => other.to_string(),
    }
}

fn uuid_xml_attr(uuid: &uuid::Uuid) -> String {
    uuid.as_braced().to_string().to_uppercase()
}

fn write_xml_root_start<W: Write>(xml: &mut quick_xml::Writer<W>, version: u32) -> io::Result<()> {
    let mut root = BytesStart::new(XML_OBJECT_STREAM);
    let version = version.to_string();
    root.push_attribute(("version", version.as_str()));
    xml.write_event(Event::Start(root))
        .map_err(io::Error::other)
}

fn write_xml_root_end<W: Write>(xml: &mut quick_xml::Writer<W>) -> io::Result<()> {
    xml.write_event(Event::End(BytesEnd::new(XML_OBJECT_STREAM)))
        .map_err(io::Error::other)
}

fn write_xml_class<W: Write>(
    xml: &mut quick_xml::Writer<W>,
    attrs: &XmlClassAttrs,
    leaf: bool,
    empty: bool,
) -> io::Result<()> {
    let mut class = BytesStart::new(XML_CLASS);
    class.push_attribute(("name", attrs.name.as_str()));
    if let Some(field) = &attrs.field {
        class.push_attribute(("field", field.as_str()));
    }
    let value = attrs.value.as_ref().or(if leaf {
        attrs.empty_leaf_value.as_ref()
    } else {
        None
    });
    if let Some(value) = value {
        class.push_attribute(("value", value.as_str()));
    }
    if let Some(version) = &attrs.version {
        class.push_attribute(("version", version.as_str()));
    }
    class.push_attribute(("type", attrs.type_id.as_str()));
    let event = if empty {
        Event::Empty(class)
    } else {
        Event::Start(class)
    };
    xml.write_event(event).map_err(io::Error::other)
}

fn write_xml_class_end<W: Write>(xml: &mut quick_xml::Writer<W>) -> io::Result<()> {
    xml.write_event(Event::End(BytesEnd::new(XML_CLASS)))
        .map_err(io::Error::other)
}

fn write_json<W: Write>(stream: &ObjectStream, writer: &mut W) -> io::Result<()> {
    writer.write_all(b"{")?;
    writer.write_all(b"\n  ")?;
    write_json_name(writer, "name")?;
    writer.write_all(b": ")?;
    write_json_serialized(writer, "ObjectStream")?;
    writer.write_all(b",\n  ")?;
    write_json_name(writer, "version")?;
    write!(writer, ": {}", stream.version)?;
    writer.write_all(b",\n  ")?;
    write_json_name(writer, "Objects")?;
    writer.write_all(b": ")?;
    write_json_array(writer, &stream.elements, 2)?;
    writer.write_all(b"\n}")?;
    Ok(())
}

fn write_json_array<W: Write>(
    writer: &mut W,
    elements: &[Element],
    element_indent: usize,
) -> io::Result<()> {
    if elements.is_empty() {
        writer.write_all(b"[]")?;
        return Ok(());
    }

    writer.write_all(b"[\n")?;
    let mut stack = vec![JsonFrame::Array {
        elements,
        index: 0,
        element_indent,
    }];

    while let Some(frame) = stack.pop() {
        match frame {
            JsonFrame::Array {
                elements,
                index,
                element_indent,
            } => {
                if index >= elements.len() {
                    writer.write_all(b"\n")?;
                    write_indent(writer, element_indent - 1)?;
                    writer.write_all(b"]")?;
                    continue;
                }

                if index > 0 {
                    writer.write_all(b",\n")?;
                }

                let element = &elements[index];
                let attrs = JsonClassAttrs::from_element(element);
                write_json_class_fields(
                    writer,
                    &attrs,
                    element_indent,
                    element.elements.is_empty(),
                )?;

                stack.push(JsonFrame::Array {
                    elements,
                    index: index + 1,
                    element_indent,
                });

                if element.elements.is_empty() {
                    writer.write_all(b"\n")?;
                    write_indent(writer, element_indent)?;
                    writer.write_all(b"}")?;
                } else {
                    writer.write_all(b",\n")?;
                    write_indent(writer, element_indent + 1)?;
                    write_json_name(writer, "Objects")?;
                    writer.write_all(b": [\n")?;
                    stack.push(JsonFrame::EndElement { element_indent });
                    stack.push(JsonFrame::Array {
                        elements: &element.elements,
                        index: 0,
                        element_indent: element_indent + 2,
                    });
                }
            }
            JsonFrame::EndElement { element_indent } => {
                writer.write_all(b"\n")?;
                write_indent(writer, element_indent)?;
                writer.write_all(b"}")?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum JsonFrame<'a> {
    Array {
        elements: &'a [Element],
        index: usize,
        element_indent: usize,
    },
    EndElement {
        element_indent: usize,
    },
}

fn write_json_class<W: Write>(
    writer: &mut W,
    attrs: &JsonClassAttrs,
    indent: usize,
    leaf: bool,
) -> io::Result<()> {
    write_json_class_fields(writer, attrs, indent, leaf)?;
    writer.write_all(b"\n")?;
    write_indent(writer, indent)?;
    writer.write_all(b"}")
}

fn write_json_class_fields<W: Write>(
    writer: &mut W,
    attrs: &JsonClassAttrs,
    indent: usize,
    leaf: bool,
) -> io::Result<()> {
    write_indent(writer, indent)?;
    writer.write_all(b"{\n")?;

    let mut first = true;
    if let Some(field) = &attrs.field {
        write_json_member(writer, indent + 1, &mut first, "field", field.as_str())?;
    }
    write_json_member(
        writer,
        indent + 1,
        &mut first,
        "typeId",
        attrs.type_id.as_str(),
    )?;
    write_json_member(
        writer,
        indent + 1,
        &mut first,
        "typeName",
        attrs.name.as_str(),
    )?;
    if let Some(specialization) = &attrs.specialization {
        write_json_member(
            writer,
            indent + 1,
            &mut first,
            "specializationTypeId",
            specialization.as_str(),
        )?;
    }
    let value = attrs.value.as_ref().or(if leaf {
        attrs.empty_leaf_value.as_ref()
    } else {
        None
    });
    if let Some(value) = value {
        write_json_member_raw(writer, indent + 1, &mut first, "value", |writer| {
            write_json_serialized(writer, value)
        })?;
    }
    if let Some(version) = attrs.version {
        write_json_member_raw(writer, indent + 1, &mut first, "version", |writer| {
            write!(writer, "{version}")
        })?;
    }
    if leaf && !attrs.data_present {
        write_json_member_raw(writer, indent + 1, &mut first, "Objects", |writer| {
            writer.write_all(b"[]")
        })?;
    }
    Ok(())
}

fn write_json_member<W: Write, V: serde::Serialize>(
    writer: &mut W,
    indent: usize,
    first: &mut bool,
    name: &str,
    value: V,
) -> io::Result<()> {
    write_json_member_raw(writer, indent, first, name, |writer| {
        write_json_serialized(writer, &value)
    })
}

fn write_json_member_raw<W: Write>(
    writer: &mut W,
    indent: usize,
    first: &mut bool,
    name: &str,
    write_value: impl FnOnce(&mut W) -> io::Result<()>,
) -> io::Result<()> {
    if *first {
        *first = false;
    } else {
        writer.write_all(b",\n")?;
    }
    write_indent(writer, indent)?;
    write_json_name(writer, name)?;
    writer.write_all(b": ")?;
    write_value(writer)
}

fn write_json_name<W: Write>(writer: &mut W, name: &str) -> io::Result<()> {
    write_json_serialized(writer, name)
}

fn write_json_serialized<W: Write, V: serde::Serialize + ?Sized>(
    writer: &mut W,
    value: &V,
) -> io::Result<()> {
    serde_json::to_writer(writer, value).map_err(io::Error::other)
}

fn write_indent<W: Write>(writer: &mut W, indent: usize) -> io::Result<()> {
    for _ in 0..indent {
        writer.write_all(b"  ")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::ST_BINARYFLAG_ELEMENT_HEADER;

    #[test]
    fn binary_to_xml_handles_deep_objectstream_without_recursion() -> Result<(), ObjectStreamError>
    {
        let bytes = deep_binary_chain(20_000);
        let xml = transcode_bytes(&bytes, ObjectStreamEncoding::Xml, None)?;
        let xml = std::str::from_utf8(&xml).expect("xml output is utf-8");

        assert!(xml.starts_with("<ObjectStream version=\"3\">"));
        assert_eq!(xml.matches("<Class ").count(), 20_000);
        assert!(xml.ends_with("</ObjectStream>"));
        Ok(())
    }

    #[test]
    fn binary_to_json_handles_deep_objectstream_without_recursion() -> Result<(), ObjectStreamError>
    {
        let bytes = deep_binary_chain(20_000);
        let json = transcode_bytes(&bytes, ObjectStreamEncoding::Json, None)?;
        let json = std::str::from_utf8(&json).expect("json output is utf-8");

        assert!(json.starts_with("{\n  \"name\": \"ObjectStream\""));
        assert_eq!(json.matches("\"typeId\"").count(), 20_000);
        assert!(json.ends_with("\n}"));
        Ok(())
    }

    #[test]
    fn binary_copy_validates_deep_objectstream_without_recursion() -> Result<(), ObjectStreamError>
    {
        let bytes = deep_binary_chain(20_000);
        let output = transcode_bytes(&bytes, ObjectStreamEncoding::Binary, None)?;

        assert_eq!(output, bytes);
        Ok(())
    }

    fn deep_binary_chain(depth: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(5 + depth * 17 + depth + 1);
        bytes.push(StreamTag::BINARY.0);
        bytes.extend_from_slice(&3u32.to_be_bytes());
        let id = Uuid::nil();
        for _ in 0..depth {
            bytes.push(ST_BINARYFLAG_ELEMENT_HEADER);
            bytes.extend_from_slice(id.as_bytes());
        }
        bytes.extend(std::iter::repeat_n(0, depth + 1));
        bytes
    }
}
