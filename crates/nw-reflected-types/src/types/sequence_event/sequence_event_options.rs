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
pub enum SequenceEventOptions {
    #[default]
    Activate = 0,
    Deactivate = 1,
    NoEffect = 2,
    Reset = 3,
}

impl From<SequenceEventOptions> for i32 {
    fn from(value: SequenceEventOptions) -> Self {
        match value {
            SequenceEventOptions::Activate => 0,
            SequenceEventOptions::Deactivate => 1,
            SequenceEventOptions::NoEffect => 2,
            SequenceEventOptions::Reset => 3,
        }
    }
}

impl ::core::convert::TryFrom<i32> for SequenceEventOptions {
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::Activate),
            1 => Ok(Self::Deactivate),
            2 => Ok(Self::NoEffect),
            3 => Ok(Self::Reset),
            _ => Err(value),
        }
    }
}

impl SequenceEventOptions {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Activate => "Activate",
            Self::Deactivate => "Deactivate",
            Self::NoEffect => "NoEffect",
            Self::Reset => "Reset",
        }
    }
}

impl AsRef<str> for SequenceEventOptions {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for SequenceEventOptions {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Activate" => Ok(Self::Activate),
            "Deactivate" => Ok(Self::Deactivate),
            "NoEffect" => Ok(Self::NoEffect),
            "Reset" => Ok(Self::Reset),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for SequenceEventOptions {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for SequenceEventOptions {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for SequenceEventOptions {
    const NAME: &'static str = "SequenceEventOptions";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x325523CB_CE7A_4F08_85CE_3C1F530DC6CF);
}
