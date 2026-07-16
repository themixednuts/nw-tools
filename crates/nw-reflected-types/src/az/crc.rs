use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid;
use bevy_reflect::Reflect;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
    Reflect,
)]
#[repr(transparent)]
pub struct Crc32(pub u32);

impl Crc32 {
    pub const ZERO: Self = Self(0);
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn from_str_lower(value: &str) -> Self {
        Self(Self::crc32_lower(value.as_bytes()))
    }
    #[must_use]
    pub const fn from_bytes_lower(bytes: &[u8]) -> Self {
        Self(Self::crc32_lower(bytes))
    }
    #[must_use]
    pub const fn from_bytes(bytes: &[u8]) -> Self {
        Self(Self::crc32(bytes))
    }
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
    #[must_use]
    pub const fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFF_u32;
        let mut index = 0;
        while index < bytes.len() {
            crc ^= bytes[index] as u32;
            let mut bit = 0;
            while bit < 8 {
                crc = if crc & 1 != 0 {
                    0xEDB8_8320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
                bit += 1;
            }
            index += 1;
        }
        crc ^ 0xFFFF_FFFF
    }
    #[must_use]
    pub const fn crc32_lower(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFF_u32;
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            let folded = if byte >= b'A' && byte <= b'Z' {
                byte + 32
            } else {
                byte
            };
            crc ^= folded as u32;
            let mut bit = 0;
            while bit < 8 {
                crc = if crc & 1 != 0 {
                    0xEDB8_8320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
                bit += 1;
            }
            index += 1;
        }
        crc ^ 0xFFFF_FFFF
    }
}

impl From<&str> for Crc32 {
    fn from(value: &str) -> Self {
        Self::from_str_lower(value)
    }
}

impl From<u32> for Crc32 {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Crc32> for u32 {
    fn from(value: Crc32) -> Self {
        value.0
    }
}

impl AzRtti for Crc32 {
    const NAME: &'static str = "AZ::Crc32";
    const TYPE_ID: Uuid = Uuid::from_u128(0x9F4E062E_06A0_46D4_85DF_E0DA96467D3A);
}
