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
pub enum CharacterActionGridCellScopeBehavior {
    #[default]
    Invalid = 0,
    Passive = 1,
    Moderate = 2,
    Aggressive = 3,
}

impl From<CharacterActionGridCellScopeBehavior> for i32 {
    fn from(value: CharacterActionGridCellScopeBehavior) -> Self {
        match value {
            CharacterActionGridCellScopeBehavior::Invalid => 0,
            CharacterActionGridCellScopeBehavior::Passive => 1,
            CharacterActionGridCellScopeBehavior::Moderate => 2,
            CharacterActionGridCellScopeBehavior::Aggressive => 3,
        }
    }
}

impl ::core::convert::TryFrom<i32> for CharacterActionGridCellScopeBehavior {
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::Invalid),
            1 => Ok(Self::Passive),
            2 => Ok(Self::Moderate),
            3 => Ok(Self::Aggressive),
            _ => Err(value),
        }
    }
}

impl CharacterActionGridCellScopeBehavior {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Passive => "Passive",
            Self::Moderate => "Moderate",
            Self::Aggressive => "Aggressive",
        }
    }
}

impl AsRef<str> for CharacterActionGridCellScopeBehavior {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for CharacterActionGridCellScopeBehavior {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Invalid" => Ok(Self::Invalid),
            "Passive" => Ok(Self::Passive),
            "Moderate" => Ok(Self::Moderate),
            "Aggressive" => Ok(Self::Aggressive),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for CharacterActionGridCellScopeBehavior {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for CharacterActionGridCellScopeBehavior {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for CharacterActionGridCellScopeBehavior {
    const NAME: &'static str = "CharacterActionGridCellScopeBehavior";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x64632B27_B75E_4778_8BAB_6519454A38C3);
}
