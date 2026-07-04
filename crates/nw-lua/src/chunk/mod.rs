//! Version-aware Lua binary chunk parsing.

use bstr::BString;

use crate::{LuaError, version::LuaVersion};

pub mod header;
pub mod proto;
pub mod reader;

/// A parsed Lua binary chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    /// Header metadata that controls scalar decoding.
    pub header: Header,
    /// Root function prototype.
    pub root: Proto,
}

/// Lua binary chunk header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Lua bytecode version.
    pub version: LuaVersion,
    /// Binary chunk format byte. Official Lua 5.1 chunks use `0`.
    pub format: u8,
    /// Whether numeric fields are little-endian.
    pub little_endian: bool,
    /// Size of C `int` fields.
    pub int_size: u8,
    /// Size of C `size_t` fields.
    pub size_t_size: u8,
    /// Size of raw instruction words. Lua 5.1 uses `4`.
    pub instruction_size: u8,
    /// Size of `lua_Number`.
    pub number_size: u8,
    /// Lua 5.1 integral-number flag.
    pub integral: bool,
    /// Future-version field, absent in Lua 5.1.
    pub integer_size: u8,
    /// Future-version field, absent in Lua 5.1.
    pub float_size: u8,
}

/// Function prototype loaded from a chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct Proto {
    /// Source name, as raw Lua bytes.
    pub source: BString,
    /// First source line for this function.
    pub line_defined: i32,
    /// Last source line for this function.
    pub last_line_defined: i32,
    /// Raw 32-bit instruction words. Decoding is Phase 1.
    pub code: Vec<u32>,
    /// Source line per instruction, if debug info was retained.
    pub line_info: Vec<i32>,
    /// Constants referenced by the function.
    pub constants: Vec<Constant>,
    /// Upvalue descriptors and names.
    pub upvalues: Vec<UpvalDesc>,
    /// Nested function prototypes.
    pub protos: Vec<Proto>,
    /// Local variable debug ranges.
    pub loc_vars: Vec<LocVar>,
    /// Number of upvalues declared by the Lua 5.1 proto header.
    pub nups: u8,
    /// Maximum stack slots used by the function.
    pub max_stack: u8,
    /// Fixed parameter count.
    pub num_params: u8,
    /// Lua 5.1 vararg flag byte.
    pub is_vararg: u8,
    /// Version that produced this proto.
    pub version: LuaVersion,
}

/// Constant pool entry.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    /// `nil`.
    Nil,
    /// Boolean constant.
    Boolean(bool),
    /// Floating point numeric constant.
    Number(f64),
    /// Integer numeric constant for later Lua versions.
    Integer(i64),
    /// Lua byte string constant.
    Str(BString),
}

/// Local variable debug information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocVar {
    /// Variable name, as raw Lua bytes.
    pub name: BString,
    /// First instruction where the local is active.
    pub start_pc: i32,
    /// First instruction where the local is dead.
    pub end_pc: i32,
}

/// Upvalue descriptor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpvalDesc {
    /// Whether the upvalue is captured from the stack. Added by Lua 5.2.
    pub in_stack: bool,
    /// Upvalue index. Added by Lua 5.2.
    pub idx: u8,
    /// Upvalue kind. Added by Lua 5.4.
    pub kind: u8,
    /// Debug name, as raw Lua bytes.
    pub name: BString,
}

/// Parse a Lua binary chunk.
///
/// # Errors
///
/// Returns [`LuaError`] when the chunk header or prototype tree is invalid.
pub fn parse(bytes: &[u8]) -> Result<Chunk, LuaError> {
    let mut reader = reader::ByteReader::new(bytes);
    let header = header::parse_header(&mut reader)?;
    reader.configure(header);
    let root = proto::parse_root(&mut reader, &header)?;
    Ok(Chunk { header, root })
}
