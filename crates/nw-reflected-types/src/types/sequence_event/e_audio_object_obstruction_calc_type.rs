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
#[repr(u32)]
#[serde(try_from = "u32", into = "u32")]
#[reflect(Serialize, Deserialize)]
pub enum EAudioObjectObstructionCalcType {
    EAooctIgnore = 0,
    EAooctSingleRay = 1,
    EAooctMultiRay = 2,
    EAooctScatterRaySmall = 3,
    EAooctScatterRayLarge = 4,
    #[default]
    EAooctNone = 5,
    EAooctUseLinkedProxy = 6,
}

impl From<EAudioObjectObstructionCalcType> for u32 {
    fn from(value: EAudioObjectObstructionCalcType) -> Self {
        match value {
            EAudioObjectObstructionCalcType::EAooctIgnore => 0,
            EAudioObjectObstructionCalcType::EAooctSingleRay => 1,
            EAudioObjectObstructionCalcType::EAooctMultiRay => 2,
            EAudioObjectObstructionCalcType::EAooctScatterRaySmall => 3,
            EAudioObjectObstructionCalcType::EAooctScatterRayLarge => 4,
            EAudioObjectObstructionCalcType::EAooctNone => 5,
            EAudioObjectObstructionCalcType::EAooctUseLinkedProxy => 6,
        }
    }
}

impl ::core::convert::TryFrom<u32> for EAudioObjectObstructionCalcType {
    type Error = u32;
    fn try_from(value: u32) -> Result<Self, u32> {
        match value {
            0 => Ok(Self::EAooctIgnore),
            1 => Ok(Self::EAooctSingleRay),
            2 => Ok(Self::EAooctMultiRay),
            3 => Ok(Self::EAooctScatterRaySmall),
            4 => Ok(Self::EAooctScatterRayLarge),
            5 => Ok(Self::EAooctNone),
            6 => Ok(Self::EAooctUseLinkedProxy),
            _ => Err(value),
        }
    }
}

impl EAudioObjectObstructionCalcType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EAooctIgnore => "eAOOCT_IGNORE",
            Self::EAooctSingleRay => "eAOOCT_SINGLE_RAY",
            Self::EAooctMultiRay => "eAOOCT_MULTI_RAY",
            Self::EAooctScatterRaySmall => "eAOOCT_SCATTER_RAY_SMALL",
            Self::EAooctScatterRayLarge => "eAOOCT_SCATTER_RAY_LARGE",
            Self::EAooctNone => "eAOOCT_NONE",
            Self::EAooctUseLinkedProxy => "eAOOCT_USE_LINKED_PROXY",
        }
    }
}

impl AsRef<str> for EAudioObjectObstructionCalcType {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for EAudioObjectObstructionCalcType {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "eAOOCT_IGNORE" => Ok(Self::EAooctIgnore),
            "eAOOCT_SINGLE_RAY" => Ok(Self::EAooctSingleRay),
            "eAOOCT_MULTI_RAY" => Ok(Self::EAooctMultiRay),
            "eAOOCT_SCATTER_RAY_SMALL" => Ok(Self::EAooctScatterRaySmall),
            "eAOOCT_SCATTER_RAY_LARGE" => Ok(Self::EAooctScatterRayLarge),
            "eAOOCT_NONE" => Ok(Self::EAooctNone),
            "eAOOCT_USE_LINKED_PROXY" => Ok(Self::EAooctUseLinkedProxy),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for EAudioObjectObstructionCalcType {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for EAudioObjectObstructionCalcType {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for EAudioObjectObstructionCalcType {
    const NAME: &'static str = "EAudioObjectObstructionCalcType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x60824763_3B5B_4993_BF27_E405B95F115F);
}
