//! Bounds-checked scalar reader for Lua binary chunks.

use bstr::BString;
use byteorder::{BigEndian, ByteOrder, LittleEndian};

use crate::LuaError;

use super::Header;

/// Byte reader configured from a parsed Lua chunk header.
#[derive(Debug, Clone)]
pub struct ByteReader<'a> {
    bytes: &'a [u8],
    pos: usize,
    little_endian: bool,
    int_size: usize,
    size_t_size: usize,
    instruction_size: usize,
    number_size: usize,
    integral_number: bool,
}

impl<'a> ByteReader<'a> {
    /// Create a reader. Header-controlled scalar fields are configured after
    /// [`crate::chunk::header::parse_header`] succeeds.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            little_endian: true,
            int_size: 4,
            size_t_size: 8,
            instruction_size: 4,
            number_size: 8,
            integral_number: false,
        }
    }

    /// Configure scalar layout from a chunk header.
    pub fn configure(&mut self, header: Header) {
        self.little_endian = header.little_endian;
        self.int_size = usize::from(header.int_size);
        self.size_t_size = usize::from(header.size_t_size);
        self.instruction_size = usize::from(header.instruction_size);
        self.number_size = usize::from(header.number_size);
        self.integral_number = header.integral;
    }

    /// Current byte offset.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.pos
    }

    /// Read a single byte.
    ///
    /// # Errors
    ///
    /// Returns [`LuaError::Truncated`] if no byte remains.
    pub fn read_byte(&mut self) -> Result<u8, LuaError> {
        Ok(self.read_bytes(1)?[0])
    }

    /// Read exactly `len` bytes and advance the cursor.
    ///
    /// # Errors
    ///
    /// Returns [`LuaError::Truncated`] if fewer than `len` bytes remain.
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], LuaError> {
        let start = self.pos;
        let Some(end) = self.pos.checked_add(len) else {
            return Err(LuaError::truncated(start, len, self.bytes.len()));
        };
        if end > self.bytes.len() {
            return Err(LuaError::truncated(start, len, self.bytes.len()));
        }
        self.pos = end;
        Ok(&self.bytes[start..end])
    }

    /// Read a Lua `int` field as `i32`.
    ///
    /// # Errors
    ///
    /// Returns [`LuaError`] on truncation or if the configured `int` field does
    /// not fit in an `i32`.
    pub fn read_int(&mut self) -> Result<i32, LuaError> {
        let value = self.read_signed(self.int_size)?;
        i32::try_from(value)
            .map_err(|_| LuaError::Malformed(format!("int value {value} does not fit in i32")))
    }

    /// Read a Lua `size_t` field as `u64`.
    ///
    /// # Errors
    ///
    /// Returns [`LuaError`] on truncation or an unsupported configured size.
    pub fn read_size_t(&mut self) -> Result<u64, LuaError> {
        self.read_unsigned(self.size_t_size)
    }

    /// Read a raw instruction word.
    ///
    /// # Errors
    ///
    /// Returns [`LuaError`] on truncation or if instruction size is not 4.
    pub fn read_instruction(&mut self) -> Result<u32, LuaError> {
        if self.instruction_size != 4 {
            return Err(LuaError::Malformed(format!(
                "unsupported instruction size {}",
                self.instruction_size
            )));
        }
        let bytes = self.read_bytes(4)?;
        Ok(if self.little_endian {
            LittleEndian::read_u32(bytes)
        } else {
            BigEndian::read_u32(bytes)
        })
    }

    /// Read a Lua 5.1 `lua_Number`.
    ///
    /// # Errors
    ///
    /// Returns [`LuaError`] on truncation or an unsupported configured number
    /// representation.
    pub fn read_number(&mut self) -> Result<f64, LuaError> {
        if self.integral_number {
            return Ok(self.read_signed(self.number_size)? as f64);
        }

        let number_size = self.number_size;
        let bytes = self.read_bytes(number_size)?;
        match number_size {
            4 => Ok(if self.little_endian {
                LittleEndian::read_f32(bytes)
            } else {
                BigEndian::read_f32(bytes)
            }
            .into()),
            8 => Ok(if self.little_endian {
                LittleEndian::read_f64(bytes)
            } else {
                BigEndian::read_f64(bytes)
            }),
            _ => Err(LuaError::Malformed(format!(
                "unsupported lua_Number size {number_size}"
            ))),
        }
    }

    /// Read a Lua 5.1 string, returning an empty byte string for a null string.
    ///
    /// The serialized length includes the trailing NUL byte. The returned value
    /// excludes that trailing terminator.
    ///
    /// # Errors
    ///
    /// Returns [`LuaError`] on truncation, a length too large for this process,
    /// or a non-NUL string terminator.
    pub fn read_string(&mut self) -> Result<BString, LuaError> {
        Ok(self.read_string_opt()?.unwrap_or_default())
    }

    pub(crate) fn read_string_opt(&mut self) -> Result<Option<BString>, LuaError> {
        let len = self.read_size_t()?;
        if len == 0 {
            return Ok(None);
        }
        let len = usize::try_from(len).map_err(|_| {
            LuaError::Malformed(format!("string length {len} does not fit in usize"))
        })?;
        let bytes = self.read_bytes(len)?;
        let Some((&terminator, payload)) = bytes.split_last() else {
            return Ok(Some(BString::new(Vec::new())));
        };
        if terminator != 0 {
            return Err(LuaError::Malformed(
                "Lua string is missing trailing NUL terminator".to_owned(),
            ));
        }
        Ok(Some(BString::from(payload.to_vec())))
    }

    fn read_unsigned(&mut self, size: usize) -> Result<u64, LuaError> {
        let bytes = self.read_bytes(size)?;
        let value = match size {
            1 => u64::from(bytes[0]),
            2 => {
                if self.little_endian {
                    u64::from(LittleEndian::read_u16(bytes))
                } else {
                    u64::from(BigEndian::read_u16(bytes))
                }
            }
            4 => {
                if self.little_endian {
                    u64::from(LittleEndian::read_u32(bytes))
                } else {
                    u64::from(BigEndian::read_u32(bytes))
                }
            }
            8 => {
                if self.little_endian {
                    LittleEndian::read_u64(bytes)
                } else {
                    BigEndian::read_u64(bytes)
                }
            }
            _ => {
                return Err(LuaError::Malformed(format!(
                    "unsupported integer field size {size}"
                )));
            }
        };
        Ok(value)
    }

    fn read_signed(&mut self, size: usize) -> Result<i64, LuaError> {
        let bytes = self.read_bytes(size)?;
        let value = match size {
            1 => i64::from(i8::from_ne_bytes([bytes[0]])),
            2 => {
                if self.little_endian {
                    i64::from(LittleEndian::read_i16(bytes))
                } else {
                    i64::from(BigEndian::read_i16(bytes))
                }
            }
            4 => {
                if self.little_endian {
                    i64::from(LittleEndian::read_i32(bytes))
                } else {
                    i64::from(BigEndian::read_i32(bytes))
                }
            }
            8 => {
                if self.little_endian {
                    LittleEndian::read_i64(bytes)
                } else {
                    BigEndian::read_i64(bytes)
                }
            }
            _ => {
                return Err(LuaError::Malformed(format!(
                    "unsupported integer field size {size}"
                )));
            }
        };
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::{chunk::Header, version::LuaVersion};

    use super::ByteReader;

    fn header(little_endian: bool, int_size: u8, size_t_size: u8) -> Header {
        Header {
            version: LuaVersion::V51,
            format: 0,
            little_endian,
            int_size,
            size_t_size,
            instruction_size: 4,
            number_size: 8,
            integral: false,
            integer_size: 0,
            float_size: 0,
        }
    }

    #[test]
    fn reads_little_endian_int_and_size_t() {
        let bytes = [0x78, 0x56, 0x34, 0x12, 0x08, 0x07, 0x06, 0x05];
        let mut reader = ByteReader::new(&bytes);
        reader.configure(header(true, 4, 4));

        assert_eq!(reader.read_int().expect("int"), 0x1234_5678);
        assert_eq!(reader.read_size_t().expect("size_t"), 0x0506_0708);
    }

    #[test]
    fn reads_big_endian_int_and_size_t() {
        let bytes = [0x12, 0x34, 0x56, 0x78, 0x05, 0x06, 0x07, 0x08];
        let mut reader = ByteReader::new(&bytes);
        reader.configure(header(false, 4, 4));

        assert_eq!(reader.read_int().expect("int"), 0x1234_5678);
        assert_eq!(reader.read_size_t().expect("size_t"), 0x0506_0708);
    }

    #[test]
    fn reads_lua_51_string_without_trailing_nul() {
        let bytes = [4, 0, 0, 0, b'f', b'o', b'o', 0];
        let mut reader = ByteReader::new(&bytes);
        reader.configure(header(true, 4, 4));

        assert_eq!(reader.read_string().expect("string").as_slice(), b"foo");
        assert_eq!(reader.position(), bytes.len());
    }

    #[test]
    fn returns_truncated_on_overrun() {
        let bytes = [0x01, 0x02];
        let mut reader = ByteReader::new(&bytes);
        reader.configure(header(true, 4, 4));

        assert!(reader.read_int().is_err());
    }
}
