use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[repr(u8)]
#[serde(try_from = "u8", into = "u8")]
#[reflect(Serialize, Deserialize)]
pub enum FactionType {
    #[default]
    None = 0,
    Faction1 = 1,
    Faction2 = 2,
    Faction3 = 3,
    Any = 4,
}

impl From<FactionType> for u8 {
    fn from(value: FactionType) -> Self {
        match value {
            FactionType::None => 0,
            FactionType::Faction1 => 1,
            FactionType::Faction2 => 2,
            FactionType::Faction3 => 3,
            FactionType::Any => 4,
        }
    }
}

impl ::core::convert::TryFrom<u8> for FactionType {
    type Error = u8;
    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Faction1),
            2 => Ok(Self::Faction2),
            3 => Ok(Self::Faction3),
            4 => Ok(Self::Any),
            _ => Err(value),
        }
    }
}

impl FactionType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Faction1 => "Faction1",
            Self::Faction2 => "Faction2",
            Self::Faction3 => "Faction3",
            Self::Any => "Any",
        }
    }
}

impl AsRef<str> for FactionType {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for FactionType {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "None" => Ok(Self::None),
            "Faction1" => Ok(Self::Faction1),
            "Faction2" => Ok(Self::Faction2),
            "Faction3" => Ok(Self::Faction3),
            "Any" => Ok(Self::Any),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for FactionType {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for FactionType {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for FactionType {
    const NAME: &'static str = "FactionType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3983D142_5E97_42E5_AD7D_9EADC6C2C896);
}
