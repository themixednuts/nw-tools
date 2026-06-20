use std::io::Read;

use arcstr::ArcStr;
use uuid::Uuid;

use crate::lookup::NameLookup;
use crate::validate_stream_version;
use crate::{
    ObjectStreamError, ST_BINARY_VALUE_SIZE_MASK, ST_BINARYFLAG_ELEMENT_END,
    ST_BINARYFLAG_ELEMENT_HEADER, ST_BINARYFLAG_EXTRA_SIZE_FIELD, ST_BINARYFLAG_HAS_NAME,
    ST_BINARYFLAG_HAS_VALUE, ST_BINARYFLAG_HAS_VERSION, StreamTag,
};

#[derive(Debug)]
pub(crate) struct BinaryElementHeader<'a> {
    pub(crate) flags: u8,
    pub(crate) name_crc: Option<u32>,
    pub(crate) version: Option<u8>,
    pub(crate) id: Uuid,
    pub(crate) specialization: Option<Uuid>,
    pub(crate) name: Option<&'a ArcStr>,
    pub(crate) field: Option<&'a ArcStr>,
    pub(crate) data_size: Option<usize>,
}

#[derive(Debug)]
pub(crate) enum BinaryElement<'a> {
    Header(BinaryElementHeader<'a>),
    EndOfList,
}

pub(crate) fn read_stream_header<R: Read>(reader: &mut R) -> Result<u32, ObjectStreamError> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    let tag = buf[0];
    if tag != StreamTag::BINARY.0 {
        return Err(ObjectStreamError::InvalidStreamTag(tag));
    }

    let mut buf4 = [0u8; 4];
    reader.read_exact(&mut buf4)?;
    let version = u32::from_be_bytes(buf4);
    validate_stream_version(version)
}

pub(crate) fn read_element_header<'a, R: Read>(
    reader: &mut R,
    stream_version: u32,
    hashes: Option<&'a NameLookup>,
) -> Result<BinaryElement<'a>, ObjectStreamError> {
    let mut buf = [0u8; 16];

    reader.read_exact(&mut buf[..1])?;
    let flags = buf[0];
    if flags == ST_BINARYFLAG_ELEMENT_END {
        return Ok(BinaryElement::EndOfList);
    }
    validate_element_flags(flags)?;

    let mut name_crc = None;
    let mut field = None;
    if flags & ST_BINARYFLAG_HAS_NAME != 0 {
        reader.read_exact(&mut buf[..4])?;
        let crc = u32::from_be_bytes(buf[..4].try_into().expect("slice width is four"));
        name_crc = Some(crc);
        field = hashes.and_then(|h| h.field_name(crc));
    }

    let mut version = None;
    if flags & ST_BINARYFLAG_HAS_VERSION != 0 {
        reader.read_exact(&mut buf[..1])?;
        version = Some(buf[0]);
    }

    reader.read_exact(&mut buf)?;
    let id = Uuid::from_slice(&buf)?;
    let name = hashes.and_then(|h| h.type_name(&id));

    let mut specialization = None;
    if stream_version == 2 {
        reader.read_exact(&mut buf)?;
        specialization = Some(Uuid::from_slice(&buf)?);
    }

    let data_size = read_value_size(reader, flags)?;

    Ok(BinaryElement::Header(BinaryElementHeader {
        flags,
        name_crc,
        version,
        id,
        specialization,
        name,
        field,
        data_size,
    }))
}

pub(crate) fn ensure_reader_exhausted<R: Read>(reader: &mut R) -> Result<(), ObjectStreamError> {
    let mut byte = [0u8; 1];
    match reader.read(&mut byte) {
        Ok(0) => Ok(()),
        Ok(_) => Err(ObjectStreamError::TrailingDataAfterRoot),
        Err(error) => Err(ObjectStreamError::Io(error)),
    }
}

fn validate_element_flags(flags: u8) -> Result<(), ObjectStreamError> {
    if flags & ST_BINARYFLAG_ELEMENT_HEADER == 0 {
        return Err(ObjectStreamError::InvalidElementFlags(flags));
    }

    let value_bits = flags & ST_BINARY_VALUE_SIZE_MASK;
    let has_value = flags & ST_BINARYFLAG_HAS_VALUE != 0;
    let has_extra_size = flags & ST_BINARYFLAG_EXTRA_SIZE_FIELD != 0;
    if !has_value && (value_bits != 0 || has_extra_size) {
        return Err(ObjectStreamError::InvalidElementFlags(flags));
    }

    Ok(())
}

fn read_value_size<R: Read>(reader: &mut R, flags: u8) -> Result<Option<usize>, ObjectStreamError> {
    if flags & ST_BINARYFLAG_HAS_VALUE == 0 {
        return Ok(None);
    }

    let value_bytes = flags & ST_BINARY_VALUE_SIZE_MASK;
    if flags & ST_BINARYFLAG_EXTRA_SIZE_FIELD == 0 {
        return Ok(Some(value_bytes as usize));
    }

    let mut buf = [0u8; 4];
    match value_bytes {
        1 => {
            reader.read_exact(&mut buf[..1])?;
            Ok(Some(buf[0] as usize))
        }
        2 => {
            reader.read_exact(&mut buf[..2])?;
            Ok(Some(
                u16::from_be_bytes(buf[..2].try_into().expect("slice width is two")) as usize,
            ))
        }
        4 => {
            reader.read_exact(&mut buf)?;
            Ok(Some(
                u32::from_be_bytes(buf[..4].try_into().expect("slice width is four")) as usize,
            ))
        }
        other => Err(ObjectStreamError::UnsupportedSizeWidth(other)),
    }
}
