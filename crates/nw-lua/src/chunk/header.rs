//! Lua binary chunk header parsing.

use crate::{
    LuaError,
    version::{LuaTarget, LuaVersion},
};

use super::{Header, reader::ByteReader};

const LUA_MAGIC: &[u8; 4] = b"\x1bLua";
const LUAC_FORMAT_OFFICIAL: u8 = 0;
const LUAC_INSTRUCTION_SIZE_51: u8 = 4;

/// Parse and validate a Lua binary chunk header.
///
/// # Errors
///
/// Returns [`LuaError`] when the header is truncated, has bad magic, uses an
/// unsupported version, or contains unsupported scalar sizes.
pub fn parse_header(reader: &mut ByteReader<'_>) -> Result<Header, LuaError> {
    let magic = reader.read_bytes(4)?;
    if magic != LUA_MAGIC {
        return Err(LuaError::BadMagic);
    }

    let version_byte = reader.read_byte()?;
    let Some(version) = LuaVersion::from_byte(version_byte) else {
        return Err(LuaError::UnsupportedVersion(version_byte));
    };
    match LuaTarget::for_version(version)? {
        LuaTarget::V51 => parse_header_51(reader),
    }
}

fn parse_header_51(reader: &mut ByteReader<'_>) -> Result<Header, LuaError> {
    let format = reader.read_byte()?;
    if format != LUAC_FORMAT_OFFICIAL {
        return Err(LuaError::Malformed(format!(
            "unsupported Lua 5.1 chunk format {format}"
        )));
    }

    let endian = reader.read_byte()?;
    let little_endian = match endian {
        0 => false,
        1 => true,
        _ => {
            return Err(LuaError::Malformed(format!(
                "invalid endianness flag {endian}"
            )));
        }
    };

    let int_size = reader.read_byte()?;
    validate_integer_size("int", int_size)?;
    let size_t_size = reader.read_byte()?;
    validate_integer_size("size_t", size_t_size)?;

    let instruction_size = reader.read_byte()?;
    if instruction_size != LUAC_INSTRUCTION_SIZE_51 {
        return Err(LuaError::Malformed(format!(
            "unsupported instruction size {instruction_size}"
        )));
    }

    let number_size = reader.read_byte()?;
    validate_number_size(number_size)?;

    let integral = match reader.read_byte()? {
        0 => false,
        1 => true,
        flag => return Err(LuaError::Malformed(format!("invalid integral flag {flag}"))),
    };

    Ok(Header {
        version: LuaTarget::V51,
        format,
        little_endian,
        int_size,
        size_t_size,
        instruction_size,
        number_size,
        integral,
        integer_size: 0,
        float_size: 0,
    })
}

fn validate_integer_size(name: &str, size: u8) -> Result<(), LuaError> {
    match size {
        1 | 2 | 4 | 8 => Ok(()),
        _ => Err(LuaError::Malformed(format!(
            "unsupported {name} size {size}"
        ))),
    }
}

fn validate_number_size(size: u8) -> Result<(), LuaError> {
    match size {
        4 | 8 => Ok(()),
        _ => Err(LuaError::Malformed(format!(
            "unsupported lua_Number size {size}"
        ))),
    }
}
