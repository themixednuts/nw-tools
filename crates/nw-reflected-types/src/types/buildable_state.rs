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
#[repr(i32)]
#[serde(try_from = "i32", into = "i32")]
#[reflect(Serialize, Deserialize)]
pub enum BuildableState {
    #[default]
    Invalid = 0,
    Unclaimed = 1,
    Plot = 2,
    Ghost = 3,
    Foundation = 4,
    Complete = 5,
    Ruin = 6,
    Upgrade = 7,
}

impl From<BuildableState> for i32 {
    fn from(value: BuildableState) -> Self {
        match value {
            BuildableState::Invalid => 0,
            BuildableState::Unclaimed => 1,
            BuildableState::Plot => 2,
            BuildableState::Ghost => 3,
            BuildableState::Foundation => 4,
            BuildableState::Complete => 5,
            BuildableState::Ruin => 6,
            BuildableState::Upgrade => 7,
        }
    }
}

impl ::core::convert::TryFrom<i32> for BuildableState {
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::Invalid),
            1 => Ok(Self::Unclaimed),
            2 => Ok(Self::Plot),
            3 => Ok(Self::Ghost),
            4 => Ok(Self::Foundation),
            5 => Ok(Self::Complete),
            6 => Ok(Self::Ruin),
            7 => Ok(Self::Upgrade),
            _ => Err(value),
        }
    }
}

impl BuildableState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Unclaimed => "Unclaimed",
            Self::Plot => "Plot",
            Self::Ghost => "Ghost",
            Self::Foundation => "Foundation",
            Self::Complete => "Complete",
            Self::Ruin => "Ruin",
            Self::Upgrade => "Upgrade",
        }
    }
}

impl AsRef<str> for BuildableState {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for BuildableState {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Invalid" => Ok(Self::Invalid),
            "Unclaimed" => Ok(Self::Unclaimed),
            "Plot" => Ok(Self::Plot),
            "Ghost" => Ok(Self::Ghost),
            "Foundation" => Ok(Self::Foundation),
            "Complete" => Ok(Self::Complete),
            "Ruin" => Ok(Self::Ruin),
            "Upgrade" => Ok(Self::Upgrade),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for BuildableState {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for BuildableState {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for BuildableState {
    const NAME: &'static str = "BuildableState";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x5519C49A_D818_4D3F_A258_73A4CA2AC0A8);
}
