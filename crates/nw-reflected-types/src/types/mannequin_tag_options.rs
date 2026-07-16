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
pub enum MannequinTagOptions {
    #[default]
    Set = 0,
    Unset = 1,
    Toggle = 2,
    NoEffect = 3,
}

impl From<MannequinTagOptions> for i32 {
    fn from(value: MannequinTagOptions) -> Self {
        match value {
            MannequinTagOptions::Set => 0,
            MannequinTagOptions::Unset => 1,
            MannequinTagOptions::Toggle => 2,
            MannequinTagOptions::NoEffect => 3,
        }
    }
}

impl ::core::convert::TryFrom<i32> for MannequinTagOptions {
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::Set),
            1 => Ok(Self::Unset),
            2 => Ok(Self::Toggle),
            3 => Ok(Self::NoEffect),
            _ => Err(value),
        }
    }
}

impl MannequinTagOptions {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Set => "Set",
            Self::Unset => "Unset",
            Self::Toggle => "Toggle",
            Self::NoEffect => "NoEffect",
        }
    }
}

impl AsRef<str> for MannequinTagOptions {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for MannequinTagOptions {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Set" => Ok(Self::Set),
            "Unset" => Ok(Self::Unset),
            "Toggle" => Ok(Self::Toggle),
            "NoEffect" => Ok(Self::NoEffect),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for MannequinTagOptions {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for MannequinTagOptions {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for MannequinTagOptions {
    const NAME: &'static str = "MannequinTagOptions";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xC4D6C366_8FC6_47BE_B35C_52056631C95C);
}
