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
pub enum DEPRECATEDCollisionType {
    #[default]
    Asset = 0,
    Shape = 1,
    ShapeProvider = 2,
    PolygonPrismCollision = 3,
}

impl From<DEPRECATEDCollisionType> for i32 {
    fn from(value: DEPRECATEDCollisionType) -> Self {
        match value {
            DEPRECATEDCollisionType::Asset => 0,
            DEPRECATEDCollisionType::Shape => 1,
            DEPRECATEDCollisionType::ShapeProvider => 2,
            DEPRECATEDCollisionType::PolygonPrismCollision => 3,
        }
    }
}

impl ::core::convert::TryFrom<i32> for DEPRECATEDCollisionType {
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::Asset),
            1 => Ok(Self::Shape),
            2 => Ok(Self::ShapeProvider),
            3 => Ok(Self::PolygonPrismCollision),
            _ => Err(value),
        }
    }
}

impl DEPRECATEDCollisionType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asset => "Asset",
            Self::Shape => "Shape",
            Self::ShapeProvider => "ShapeProvider",
            Self::PolygonPrismCollision => "PolygonPrismCollision",
        }
    }
}

impl AsRef<str> for DEPRECATEDCollisionType {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for DEPRECATEDCollisionType {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Asset" => Ok(Self::Asset),
            "Shape" => Ok(Self::Shape),
            "ShapeProvider" => Ok(Self::ShapeProvider),
            "PolygonPrismCollision" => Ok(Self::PolygonPrismCollision),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for DEPRECATEDCollisionType {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for DEPRECATEDCollisionType {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for DEPRECATEDCollisionType {
    const NAME: &'static str = "DEPRECATED CollisionType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x382C284B_70CB_4A5D_A5EC_BD753DBD1EAC);
}
