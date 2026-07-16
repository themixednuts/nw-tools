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
#[repr(transparent)]
#[serde(try_from = "i32", into = "i32")]
#[reflect(Serialize, Deserialize)]
pub struct CharacterActionGridCellValue(pub i32);

impl CharacterActionGridCellValue {
    pub const VARIANTS: &[(i32, &str)] = &[
        (0, "Invalid"),
        (1, "TransitionAllowed"),
        (2, "TransitionAllowedWithConditions"),
        (3, "TransitionNotAllowed"),
        (1, "Always"),
        (2, "Sometimes"),
        (3, "Never"),
    ];
    pub const INVALID: Self = Self(0);
    pub const TRANSITION_ALLOWED: Self = Self(1);
    pub const TRANSITION_ALLOWED_WITH_CONDITIONS: Self = Self(2);
    pub const TRANSITION_NOT_ALLOWED: Self = Self(3);
    pub const ALWAYS: Self = Self(1);
    pub const SOMETIMES: Self = Self(2);
    pub const NEVER: Self = Self(3);
    #[must_use]
    pub const fn value(self) -> i32 {
        self.0
    }
    #[must_use]
    pub fn as_str(self) -> Option<&'static str> {
        Self::VARIANTS
            .iter()
            .find_map(|(value, name)| (*value == self.0).then_some(*name))
    }
}

impl From<CharacterActionGridCellValue> for i32 {
    fn from(value: CharacterActionGridCellValue) -> Self {
        value.0
    }
}

impl From<i32> for CharacterActionGridCellValue {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

impl AsRef<i32> for CharacterActionGridCellValue {
    fn as_ref(&self) -> &i32 {
        &self.0
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for CharacterActionGridCellValue {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        Self::VARIANTS
            .iter()
            .find_map(|(raw, name)| (*name == value).then_some(Self(*raw)))
            .ok_or(value)
    }
}

impl ::core::str::FromStr for CharacterActionGridCellValue {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::VARIANTS
            .iter()
            .find_map(|(raw, name)| (*name == value).then_some(Self(*raw)))
            .or_else(|| value.parse::<i32>().ok().map(Self))
            .ok_or_else(|| value.to_owned())
    }
}

impl ::core::fmt::Display for CharacterActionGridCellValue {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        match (*self).as_str() {
            Some(name) => formatter.write_str(name),
            None => ::core::fmt::Display::fmt(&self.0, formatter),
        }
    }
}

impl AzRtti for CharacterActionGridCellValue {
    const NAME: &'static str = "CharacterActionGridCellValue";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x33DF76E2_03AD_44E9_9631_B5E819CBF64B);
}
