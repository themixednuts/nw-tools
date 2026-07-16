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
pub enum InstancedLootType {
    PoiLoot = 0,
    AiLoot = 1,
    Gathering = 2,
    Collectible = 3,
    Quest = 4,
    Discarded = 5,
    GameMode = 6,
    #[default]
    None = 7,
}

impl From<InstancedLootType> for i32 {
    fn from(value: InstancedLootType) -> Self {
        match value {
            InstancedLootType::PoiLoot => 0,
            InstancedLootType::AiLoot => 1,
            InstancedLootType::Gathering => 2,
            InstancedLootType::Collectible => 3,
            InstancedLootType::Quest => 4,
            InstancedLootType::Discarded => 5,
            InstancedLootType::GameMode => 6,
            InstancedLootType::None => 7,
        }
    }
}

impl ::core::convert::TryFrom<i32> for InstancedLootType {
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::PoiLoot),
            1 => Ok(Self::AiLoot),
            2 => Ok(Self::Gathering),
            3 => Ok(Self::Collectible),
            4 => Ok(Self::Quest),
            5 => Ok(Self::Discarded),
            6 => Ok(Self::GameMode),
            7 => Ok(Self::None),
            _ => Err(value),
        }
    }
}

impl InstancedLootType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PoiLoot => "POILoot",
            Self::AiLoot => "AILoot",
            Self::Gathering => "Gathering",
            Self::Collectible => "Collectible",
            Self::Quest => "Quest",
            Self::Discarded => "Discarded",
            Self::GameMode => "GameMode",
            Self::None => "None",
        }
    }
}

impl AsRef<str> for InstancedLootType {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for InstancedLootType {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "POILoot" => Ok(Self::PoiLoot),
            "AILoot" => Ok(Self::AiLoot),
            "Gathering" => Ok(Self::Gathering),
            "Collectible" => Ok(Self::Collectible),
            "Quest" => Ok(Self::Quest),
            "Discarded" => Ok(Self::Discarded),
            "GameMode" => Ok(Self::GameMode),
            "None" => Ok(Self::None),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for InstancedLootType {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for InstancedLootType {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for InstancedLootType {
    const NAME: &'static str = "InstancedLootType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xB1B302BE_BAB3_42A9_941D_2B156219032B);
}
