//! Lua bytecode version tags.

use crate::error::LuaError;

/// Lua binary chunk versions understood by the crate roadmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuaVersion {
    /// Lua 5.1 (`0x51`).
    V51,
    /// Lua 5.2 (`0x52`).
    V52,
    /// Lua 5.3 (`0x53`).
    V53,
    /// Lua 5.4 (`0x54`).
    V54,
    /// Lua 5.5 (`0x55`).
    V55,
}

impl LuaVersion {
    /// Convert a binary chunk version byte to a version enum.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x51 => Some(Self::V51),
            0x52 => Some(Self::V52),
            0x53 => Some(Self::V53),
            0x54 => Some(Self::V54),
            0x55 => Some(Self::V55),
            _ => None,
        }
    }

    /// Return the byte used in a Lua binary chunk header.
    #[must_use]
    pub const fn to_byte(self) -> u8 {
        match self {
            Self::V51 => 0x51,
            Self::V52 => 0x52,
            Self::V53 => 0x53,
            Self::V54 => 0x54,
            Self::V55 => 0x55,
        }
    }
}

/// Detect the Lua version byte from a binary chunk header.
///
/// # Errors
///
/// Returns [`LuaError::Truncated`] if the header is too short,
/// [`LuaError::BadMagic`] if the Lua signature is missing, or
/// [`LuaError::UnsupportedVersion`] for an unknown version byte.
pub fn detect_header_version(bytes: &[u8]) -> Result<LuaVersion, LuaError> {
    const MIN_VERSION_OFFSET: usize = 5;
    if bytes.len() < MIN_VERSION_OFFSET {
        return Err(LuaError::truncated(0, MIN_VERSION_OFFSET, bytes.len()));
    }
    if &bytes[..4] != b"\x1bLua" {
        return Err(LuaError::BadMagic);
    }
    let byte = bytes[4];
    LuaVersion::from_byte(byte).ok_or(LuaError::UnsupportedVersion(byte))
}
