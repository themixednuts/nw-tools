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
pub enum MannequinTagGroupOptions {
    #[default]
    Clear = 0,
    NoEffect = 1,
}

impl From<MannequinTagGroupOptions> for i32 {
    fn from(value: MannequinTagGroupOptions) -> Self {
        match value {
            MannequinTagGroupOptions::Clear => 0,
            MannequinTagGroupOptions::NoEffect => 1,
        }
    }
}

impl ::core::convert::TryFrom<i32> for MannequinTagGroupOptions {
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::Clear),
            1 => Ok(Self::NoEffect),
            _ => Err(value),
        }
    }
}

impl MannequinTagGroupOptions {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "Clear",
            Self::NoEffect => "NoEffect",
        }
    }
}

impl AsRef<str> for MannequinTagGroupOptions {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for MannequinTagGroupOptions {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Clear" => Ok(Self::Clear),
            "NoEffect" => Ok(Self::NoEffect),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for MannequinTagGroupOptions {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for MannequinTagGroupOptions {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for MannequinTagGroupOptions {
    const NAME: &'static str = "MannequinTagGroupOptions";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6B2BFA12_3B9F_4B67_8328_9C1C74FABB3B);
}
