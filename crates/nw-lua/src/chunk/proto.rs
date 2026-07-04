//! Lua 5.1 function prototype parsing.

use bstr::BString;

use crate::{LuaError, version::LuaVersion};

use super::{Constant, Header, LocVar, Proto, UpvalDesc, reader::ByteReader};

const LUA_TNIL: u8 = 0;
const LUA_TBOOLEAN: u8 = 1;
const LUA_TNUMBER: u8 = 3;
const LUA_TSTRING: u8 = 4;

/// Parse the root prototype for a chunk.
///
/// # Errors
///
/// Returns [`LuaError`] when the prototype tree is truncated or malformed.
pub fn parse_root(reader: &mut ByteReader<'_>, header: &Header) -> Result<Proto, LuaError> {
    match header.version {
        LuaVersion::V51 => parse_proto_51(reader, None),
        version => Err(LuaError::UnsupportedVersion(version.to_byte())),
    }
}

fn parse_proto_51(
    reader: &mut ByteReader<'_>,
    parent_source: Option<&BString>,
) -> Result<Proto, LuaError> {
    let source = match reader.read_string_opt()? {
        Some(source) => source,
        None => parent_source.cloned().unwrap_or_default(),
    };
    let line_defined = reader.read_int()?;
    let last_line_defined = reader.read_int()?;
    let nups = reader.read_byte()?;
    let num_params = reader.read_byte()?;
    let is_vararg = reader.read_byte()?;
    let max_stack = reader.read_byte()?;

    let code = parse_code(reader)?;
    let constants = parse_constants(reader)?;
    let protos = parse_sub_protos(reader, &source)?;
    let line_info = parse_line_info(reader)?;
    let loc_vars = parse_loc_vars(reader)?;
    let upvalues = parse_upvalue_names(reader, nups)?;

    Ok(Proto {
        source,
        line_defined,
        last_line_defined,
        code,
        line_info,
        constants,
        upvalues,
        protos,
        loc_vars,
        nups,
        max_stack,
        num_params,
        is_vararg,
        version: LuaVersion::V51,
    })
}

fn parse_code(reader: &mut ByteReader<'_>) -> Result<Vec<u32>, LuaError> {
    let count = read_count(reader, "code")?;
    let mut code = Vec::with_capacity(count);
    for _ in 0..count {
        code.push(reader.read_instruction()?);
    }
    Ok(code)
}

fn parse_constants(reader: &mut ByteReader<'_>) -> Result<Vec<Constant>, LuaError> {
    let count = read_count(reader, "constants")?;
    let mut constants = Vec::with_capacity(count);
    for index in 0..count {
        let tag = reader.read_byte()?;
        let constant = match tag {
            LUA_TNIL => Constant::Nil,
            LUA_TBOOLEAN => Constant::Boolean(reader.read_byte()? != 0),
            LUA_TNUMBER => Constant::Number(reader.read_number()?),
            LUA_TSTRING => Constant::Str(reader.read_string()?),
            _ => {
                return Err(LuaError::Malformed(format!(
                    "unknown constant tag {tag} at index {index}"
                )));
            }
        };
        constants.push(constant);
    }
    Ok(constants)
}

fn parse_sub_protos(
    reader: &mut ByteReader<'_>,
    parent_source: &BString,
) -> Result<Vec<Proto>, LuaError> {
    let count = read_count(reader, "nested protos")?;
    let mut protos = Vec::with_capacity(count);
    for _ in 0..count {
        protos.push(parse_proto_51(reader, Some(parent_source))?);
    }
    Ok(protos)
}

fn parse_line_info(reader: &mut ByteReader<'_>) -> Result<Vec<i32>, LuaError> {
    let count = read_count(reader, "line info")?;
    let mut line_info = Vec::with_capacity(count);
    for _ in 0..count {
        line_info.push(reader.read_int()?);
    }
    Ok(line_info)
}

fn parse_loc_vars(reader: &mut ByteReader<'_>) -> Result<Vec<LocVar>, LuaError> {
    let count = read_count(reader, "local variables")?;
    let mut loc_vars = Vec::with_capacity(count);
    for _ in 0..count {
        loc_vars.push(LocVar {
            name: reader.read_string()?,
            start_pc: reader.read_int()?,
            end_pc: reader.read_int()?,
        });
    }
    Ok(loc_vars)
}

fn parse_upvalue_names(reader: &mut ByteReader<'_>, nups: u8) -> Result<Vec<UpvalDesc>, LuaError> {
    let count = read_count(reader, "upvalue names")?;
    let mut upvalues = vec![UpvalDesc::default(); usize::from(nups).max(count)];
    for upvalue in upvalues.iter_mut().take(count) {
        upvalue.name = reader.read_string()?;
    }
    Ok(upvalues)
}

fn read_count(reader: &mut ByteReader<'_>, field: &str) -> Result<usize, LuaError> {
    let count = reader.read_int()?;
    if count < 0 {
        return Err(LuaError::Malformed(format!(
            "negative {field} count {count}"
        )));
    }
    usize::try_from(count)
        .map_err(|_| LuaError::Malformed(format!("{field} count {count} does not fit in usize")))
}
