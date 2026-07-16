use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid;
use bevy_reflect::Reflect;

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
            guid: Uuid::nil(),
            sub_id: 0,
        }
    }
    #[must_use]
    pub const fn is_nil(&self) -> bool {
        self.sub_id == 0 && self.guid.is_nil()
    }
}

impl AsRef<Self> for AssetId {
    fn as_ref(&self) -> &Self {
        self
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

impl core::fmt::Debug for AssetId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AssetId")
            .field("guid", &format_args!("{}", self.guid))
            .field("sub_id", &format_args!("{:#x}", self.sub_id))
            .finish()
    }
}

impl core::fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}:{:x}", self.guid.braced_upper(), self.sub_id)
    }
}

#[derive(Debug)]
pub enum AssetIdParseError {
    MissingSeparator,
    BadGuid(::uuid::Error),
    BadSubId(core::num::ParseIntError),
}

impl core::fmt::Display for AssetIdParseError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingSeparator => {
                formatter.write_str("missing ':' separator between guid and sub_id")
            }
            Self::BadGuid(error) => write!(formatter, "invalid guid: {error}"),
            Self::BadSubId(error) => write!(formatter, "invalid sub_id: {error}"),
        }
    }
}

impl std::error::Error for AssetIdParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MissingSeparator => None,
            Self::BadGuid(error) => Some(error),
            Self::BadSubId(error) => Some(error),
        }
    }
}

impl core::str::FromStr for AssetId {
    type Err = AssetIdParseError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (guid_part, sub_id_part) = value
            .rsplit_once(':')
            .ok_or(AssetIdParseError::MissingSeparator)?;
        let guid = Uuid::parse_str(guid_part.trim_start_matches('{').trim_end_matches('}'))
            .map_err(AssetIdParseError::BadGuid)?;
        let sub_id = u32::from_str_radix(sub_id_part, 16).map_err(AssetIdParseError::BadSubId)?;
        Ok(Self { guid, sub_id })
    }
}

impl AzRtti for AssetId {
    const NAME: &'static str = "AZ::Data::AssetId";
    const TYPE_ID: Uuid = Uuid::from_u128(0x652ED536_3402_439B_AEBE_4A5DBC554085);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Reflect)]
pub struct Asset {
    #[reflect(ignore)]
    pub asset_id: AssetId,
    #[reflect(ignore)]
    pub asset_type: Uuid,
    pub hint: Option<String>,
}

impl Asset {
    #[must_use]
    pub fn new(asset_id: AssetId, asset_type: Uuid, hint: Option<impl Into<String>>) -> Self {
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
            asset_type: Uuid::nil(),
            hint: None,
        }
    }
    #[must_use]
    pub const fn from_id(asset_id: AssetId, asset_type: Uuid) -> Self {
        Self {
            asset_id,
            asset_type,
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
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    #[must_use]
    pub fn hint(&self) -> Option<&str> {
        self.hint
            .as_deref()
            .map(str::trim)
            .filter(|hint| !hint.is_empty())
    }
    #[must_use]
    pub const fn is_nil(&self) -> bool {
        self.asset_id.is_nil()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.is_nil() && self.asset_type.is_nil() && self.hint().is_none()
    }
}

impl Default for Asset {
    fn default() -> Self {
        Self::empty()
    }
}

impl AzRtti for Asset {
    const NAME: &'static str = "AZ::Data::Asset";
    const TYPE_ID: Uuid = Uuid::from_u128(0xC891BF19_B60C_45E2_BFD0_027D15DDC939);
}
