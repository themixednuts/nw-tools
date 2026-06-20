use std::error::Error;
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

use uuid::Uuid;

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId {
    pub guid: Uuid,
    pub sub_id: u32,
}

impl AssetId {
    #[must_use]
    pub const fn new(guid: Uuid, sub_id: u32) -> Self {
        Self { guid, sub_id }
    }

    #[must_use]
    pub const fn nil() -> Self {
        Self {
            guid: Uuid::from_u128(0),
            sub_id: 0,
        }
    }

    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.sub_id == 0 && self.guid.is_nil()
    }
}

impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetId")
            .field("guid", &format_args!("{}", self.guid))
            .field("sub_id", &format_args!("{:#x}", self.sub_id))
            .finish()
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{:x}",
            self.guid.as_braced().to_string().to_uppercase(),
            self.sub_id
        )
    }
}

impl From<Uuid> for AssetId {
    fn from(guid: Uuid) -> Self {
        Self { guid, sub_id: 0 }
    }
}

impl From<(Uuid, u32)> for AssetId {
    fn from((guid, sub_id): (Uuid, u32)) -> Self {
        Self { guid, sub_id }
    }
}

impl FromStr for AssetId {
    type Err = AssetIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (guid_part, sub_id_part) = value
            .rsplit_once(':')
            .ok_or(AssetIdParseError::MissingSeparator)?;
        let guid = Uuid::parse_str(guid_part.trim_start_matches('{').trim_end_matches('}'))?;
        let sub_id = u32::from_str_radix(sub_id_part, 16)?;
        Ok(Self { guid, sub_id })
    }
}

#[derive(Debug)]
pub enum AssetIdParseError {
    MissingSeparator,
    BadGuid(uuid::Error),
    BadSubId(ParseIntError),
}

impl fmt::Display for AssetIdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSeparator => f.write_str("missing ':' separator between guid and sub_id"),
            Self::BadGuid(error) => write!(f, "invalid guid: {error}"),
            Self::BadSubId(error) => write!(f, "invalid sub_id: {error}"),
        }
    }
}

impl Error for AssetIdParseError {}

impl From<uuid::Error> for AssetIdParseError {
    fn from(error: uuid::Error) -> Self {
        Self::BadGuid(error)
    }
}

impl From<ParseIntError> for AssetIdParseError {
    fn from(error: ParseIntError) -> Self {
        Self::BadSubId(error)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetType(Uuid);

impl AssetType {
    #[must_use]
    pub const fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    #[must_use]
    pub const fn nil() -> Self {
        Self(Uuid::from_u128(0))
    }

    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    #[must_use]
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Debug for AssetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AssetType")
            .field(&format_args!("{self}"))
            .finish()
    }
}

impl fmt::Display for AssetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.as_braced().to_string().to_uppercase())
    }
}

impl From<Uuid> for AssetType {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl FromStr for AssetType {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value.trim_start_matches('{').trim_end_matches('}')).map(Self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssetReference {
    pub asset_id: AssetId,
    pub asset_type: AssetType,
    pub hint: Option<String>,
}

impl AssetReference {
    #[must_use]
    pub fn new(asset_id: AssetId, asset_type: AssetType, hint: Option<impl Into<String>>) -> Self {
        Self {
            asset_id,
            asset_type,
            hint: hint.map(Into::into),
        }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self {
            asset_id: AssetId::nil(),
            asset_type: AssetType::nil(),
            hint: None,
        }
    }

    #[must_use]
    pub fn from_hint(hint: impl Into<String>) -> Self {
        Self {
            hint: Some(hint.into()),
            ..Self::empty()
        }
    }

    #[must_use]
    pub fn hint(&self) -> Option<&str> {
        self.hint
            .as_deref()
            .map(str::trim)
            .filter(|hint| !hint.is_empty())
    }
}
