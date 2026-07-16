use crate::az::rtti::AzRtti;
use bevy_reflect::Reflect;
use sha1::Digest;

#[derive(
    Clone,
    Copy,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Serialize,
    serde::Deserialize,
    Reflect,
)]
#[repr(transparent)]
pub struct Uuid(::uuid::Uuid);

impl Uuid {
    pub const NIL: Self = Self(::uuid::Uuid::nil());
    #[must_use]
    pub const fn nil() -> Self {
        Self::NIL
    }
    #[must_use]
    pub const fn from_u128(value: u128) -> Self {
        Self(::uuid::Uuid::from_u128(value))
    }
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(::uuid::Uuid::from_bytes(bytes))
    }
    /// Parses a UUID string into the AZ UUID wrapper.
    ///
    /// # Errors
    ///
    /// Returns the underlying `uuid` parse error when `value` is not a valid UUID.
    pub fn parse_str(value: &str) -> Result<Self, ::uuid::Error> {
        ::uuid::Uuid::parse_str(value).map(Self)
    }
    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.0.is_nil()
    }
    #[must_use]
    pub const fn as_inner(&self) -> &::uuid::Uuid {
        &self.0
    }
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
    #[must_use]
    pub fn braced_upper(self) -> String {
        self.0.as_braced().to_string().to_uppercase()
    }
    #[must_use]
    pub fn create_data(bytes: &[u8]) -> Self {
        if bytes.is_empty() {
            return Self::nil();
        }
        let mut hasher = sha1::Sha1::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        let mut data = [0_u8; 16];
        data.copy_from_slice(&digest[..16]);
        data[8] &= 0xBF;
        data[8] |= 0x80;
        data[6] &= 0x5F;
        data[6] |= 0x50;
        Self(::uuid::Uuid::from_bytes(data))
    }
    #[must_use]
    pub fn create_name(name: &[u8]) -> Self {
        Self::create_data(name)
    }
    #[must_use]
    pub fn combine(lhs: Self, rhs: Self) -> Self {
        let mut bytes = [0_u8; 32];
        bytes[..16].copy_from_slice(lhs.as_bytes());
        bytes[16..].copy_from_slice(rhs.as_bytes());
        Self::create_data(&bytes)
    }
    #[must_use]
    pub fn aggregate_type_ids(type_ids: impl IntoIterator<Item = Self>) -> Option<Self> {
        let mut iter = type_ids.into_iter();
        let mut acc = iter.next()?;
        for type_id in iter {
            acc = Self::combine(acc, type_id);
        }
        Some(acc)
    }
    #[must_use]
    pub fn aggregate_type_ids_right(type_ids: &[Self]) -> Option<Self> {
        let (first, tail) = type_ids.split_first()?;
        match Self::aggregate_type_ids_right(tail) {
            Some(tail) if !tail.is_nil() => Some(Self::combine(*first, tail)),
            _ => Some(*first),
        }
    }
    #[must_use]
    pub fn specialized_template_prefix(template_base: Self, args: &[Self]) -> Option<Self> {
        Self::aggregate_type_ids(args.iter().copied())
            .map(|args| Self::combine(template_base, args))
    }
    #[must_use]
    pub fn specialized_template_postfix(template_base: Self, args: &[Self]) -> Option<Self> {
        Self::aggregate_type_ids(args.iter().copied())
            .map(|args| Self::combine(args, template_base))
    }
    #[must_use]
    pub fn template_auto_type_id(value: usize) -> Self {
        Self::create_name(value.to_string().as_bytes())
    }
}

impl AzRtti for Uuid {
    const NAME: &'static str = "AZ::Uuid";
    const TYPE_ID: Self = Self::from_u128(0xE152C105_A133_4D03_BBF8_3D4B2FBA3E2A);
}

impl From<::uuid::Uuid> for Uuid {
    fn from(value: ::uuid::Uuid) -> Self {
        Self(value)
    }
}

impl From<Uuid> for ::uuid::Uuid {
    fn from(value: Uuid) -> Self {
        value.0
    }
}

impl AsRef<::uuid::Uuid> for Uuid {
    fn as_ref(&self) -> &::uuid::Uuid {
        &self.0
    }
}

impl core::fmt::Debug for Uuid {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self, formatter)
    }
}

impl core::fmt::Display for Uuid {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.0, formatter)
    }
}

impl core::str::FromStr for Uuid {
    type Err = ::uuid::Error;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_str(value)
    }
}

pub mod type_ids {
    use super::Uuid;
    pub const INT: Uuid = Uuid::from_u128(0x72039442_EB38_4D42_A1AD_CB68F7E0EEF6);
    pub const U8: Uuid = Uuid::from_u128(0x72B9409A_7D1A_4831_9CFE_FCB3FADD3426);
    pub const AZ_UUID: Uuid = Uuid::from_u128(0xE152C105_A133_4D03_BBF8_3D4B2FBA3E2A);
    pub const ENTITY_ID: Uuid = Uuid::from_u128(0x6383F1D3_BB27_4E6B_A49A_6409B2059EAA);
    pub const COMPONENT_ID: Uuid = Uuid::from_u128(0xD6597933_47CD_4FC8_B911_63F3E2B0993A);
    pub const COMPONENT_ID_VECTOR: Uuid = Uuid::from_u128(0xE7781CB0_E712_5E6A_948D_92FD4FE87F0D);
    pub const CRC32: Uuid = Uuid::from_u128(0x9F4E062E_06A0_46D4_85DF_E0DA96467D3A);
    pub const ASSET_ID: Uuid = Uuid::from_u128(0x652ED536_3402_439B_AEBE_4A5DBC554085);
}
