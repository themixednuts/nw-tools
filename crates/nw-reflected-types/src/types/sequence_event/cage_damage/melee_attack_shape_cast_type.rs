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
pub enum MeleeAttackShapeCastType {
    #[default]
    MeleeAttackShapeCastTypeNone = 0,
    MeleeAttackShapeCastTypeSphere = 1,
    MeleeAttackShapeCastTypeCapsule = 2,
    MeleeAttackShapeCastTypeBox = 3,
    MeleeAttackShapeCastTypeCylinder = 4,
}

impl From<MeleeAttackShapeCastType> for i32 {
    fn from(value: MeleeAttackShapeCastType) -> Self {
        match value {
            MeleeAttackShapeCastType::MeleeAttackShapeCastTypeNone => 0,
            MeleeAttackShapeCastType::MeleeAttackShapeCastTypeSphere => 1,
            MeleeAttackShapeCastType::MeleeAttackShapeCastTypeCapsule => 2,
            MeleeAttackShapeCastType::MeleeAttackShapeCastTypeBox => 3,
            MeleeAttackShapeCastType::MeleeAttackShapeCastTypeCylinder => 4,
        }
    }
}

impl ::core::convert::TryFrom<i32> for MeleeAttackShapeCastType {
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::MeleeAttackShapeCastTypeNone),
            1 => Ok(Self::MeleeAttackShapeCastTypeSphere),
            2 => Ok(Self::MeleeAttackShapeCastTypeCapsule),
            3 => Ok(Self::MeleeAttackShapeCastTypeBox),
            4 => Ok(Self::MeleeAttackShapeCastTypeCylinder),
            _ => Err(value),
        }
    }
}

impl MeleeAttackShapeCastType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MeleeAttackShapeCastTypeNone => "MeleeAttackShapeCastTypeNone",
            Self::MeleeAttackShapeCastTypeSphere => "MeleeAttackShapeCastTypeSphere",
            Self::MeleeAttackShapeCastTypeCapsule => "MeleeAttackShapeCastTypeCapsule",
            Self::MeleeAttackShapeCastTypeBox => "MeleeAttackShapeCastTypeBox",
            Self::MeleeAttackShapeCastTypeCylinder => "MeleeAttackShapeCastTypeCylinder",
        }
    }
}

impl AsRef<str> for MeleeAttackShapeCastType {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for MeleeAttackShapeCastType {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "MeleeAttackShapeCastTypeNone" => Ok(Self::MeleeAttackShapeCastTypeNone),
            "MeleeAttackShapeCastTypeSphere" => Ok(Self::MeleeAttackShapeCastTypeSphere),
            "MeleeAttackShapeCastTypeCapsule" => Ok(Self::MeleeAttackShapeCastTypeCapsule),
            "MeleeAttackShapeCastTypeBox" => Ok(Self::MeleeAttackShapeCastTypeBox),
            "MeleeAttackShapeCastTypeCylinder" => Ok(Self::MeleeAttackShapeCastTypeCylinder),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for MeleeAttackShapeCastType {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for MeleeAttackShapeCastType {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for MeleeAttackShapeCastType {
    const NAME: &'static str = "MeleeAttackShapeCastType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6BE5553F_2D3B_405E_8E64_31365BBAA4A8);
}
