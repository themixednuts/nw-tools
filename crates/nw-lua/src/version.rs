//! Lua release detection and supported compiler targets.

use std::fmt;

use crate::error::LuaError;

/// Lua binary chunk releases recognized at the input boundary.
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

impl fmt::Display for LuaVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::V51 => "5.1",
            Self::V52 => "5.2",
            Self::V53 => "5.3",
            Self::V54 => "5.4",
            Self::V55 => "5.5",
        })
    }
}

/// A Lua release supported by the complete parse-to-source pipeline.
///
/// Keep this enum intentionally narrower than [`LuaVersion`]. Adding a variant
/// here makes every compiler stage handle that release exhaustively instead of
/// allowing a partially implemented version to leak into the IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuaTarget {
    /// Complete Lua 5.1 pipeline.
    V51,
}

impl LuaTarget {
    /// Resolve a recognized release to a complete compiler target.
    ///
    /// # Errors
    ///
    /// Returns [`LuaError::UnsupportedVersion`] until the complete pipeline for
    /// that release has been implemented and added to this enum.
    pub const fn for_version(version: LuaVersion) -> Result<Self, LuaError> {
        match version {
            LuaVersion::V51 => Ok(Self::V51),
            version => Err(LuaError::UnsupportedVersion(version.to_byte())),
        }
    }

    /// Return the binary chunk release implemented by this target.
    #[must_use]
    pub const fn version(self) -> LuaVersion {
        match self {
            Self::V51 => LuaVersion::V51,
        }
    }
}

impl fmt::Display for LuaTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.version().fmt(formatter)
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

#[cfg(test)]
mod tests {
    use super::{LuaTarget, LuaVersion};

    #[test]
    fn complete_targets_are_narrower_than_recognized_releases() {
        assert!(matches!(
            LuaTarget::for_version(LuaVersion::V51),
            Ok(LuaTarget::V51)
        ));
        for version in [
            LuaVersion::V52,
            LuaVersion::V53,
            LuaVersion::V54,
            LuaVersion::V55,
        ] {
            assert!(LuaTarget::for_version(version).is_err());
        }
    }

    #[test]
    fn release_and_target_labels_have_one_owner() {
        assert_eq!(LuaVersion::V54.to_string(), "5.4");
        assert_eq!(LuaTarget::V51.to_string(), "5.1");
    }
}
