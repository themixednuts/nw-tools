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
#[repr(u8)]
#[serde(try_from = "u8", into = "u8")]
#[reflect(Serialize, Deserialize)]
pub enum ItemType {
    #[default]
    Ammo = 0,
    Armor = 1,
    Blueprint = 2,
    Consumable = 3,
    Dye = 4,
    HousingItem = 5,
    Kit = 6,
    Lore = 7,
    Resource = 8,
    Weapon = 9,
    MountDye = 11,
}

impl From<ItemType> for u8 {
    fn from(value: ItemType) -> Self {
        match value {
            ItemType::Ammo => 0,
            ItemType::Armor => 1,
            ItemType::Blueprint => 2,
            ItemType::Consumable => 3,
            ItemType::Dye => 4,
            ItemType::HousingItem => 5,
            ItemType::Kit => 6,
            ItemType::Lore => 7,
            ItemType::Resource => 8,
            ItemType::Weapon => 9,
            ItemType::MountDye => 11,
        }
    }
}

impl ::core::convert::TryFrom<u8> for ItemType {
    type Error = u8;
    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            0 => Ok(Self::Ammo),
            1 => Ok(Self::Armor),
            2 => Ok(Self::Blueprint),
            3 => Ok(Self::Consumable),
            4 => Ok(Self::Dye),
            5 => Ok(Self::HousingItem),
            6 => Ok(Self::Kit),
            7 => Ok(Self::Lore),
            8 => Ok(Self::Resource),
            9 => Ok(Self::Weapon),
            11 => Ok(Self::MountDye),
            _ => Err(value),
        }
    }
}

impl ItemType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ammo => "Ammo",
            Self::Armor => "Armor",
            Self::Blueprint => "Blueprint",
            Self::Consumable => "Consumable",
            Self::Dye => "Dye",
            Self::HousingItem => "HousingItem",
            Self::Kit => "Kit",
            Self::Lore => "Lore",
            Self::Resource => "Resource",
            Self::Weapon => "Weapon",
            Self::MountDye => "MountDye",
        }
    }
}

impl AsRef<str> for ItemType {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for ItemType {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Ammo" => Ok(Self::Ammo),
            "Armor" => Ok(Self::Armor),
            "Blueprint" => Ok(Self::Blueprint),
            "Consumable" => Ok(Self::Consumable),
            "Dye" => Ok(Self::Dye),
            "HousingItem" => Ok(Self::HousingItem),
            "Kit" => Ok(Self::Kit),
            "Lore" => Ok(Self::Lore),
            "Resource" => Ok(Self::Resource),
            "Weapon" => Ok(Self::Weapon),
            "MountDye" => Ok(Self::MountDye),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for ItemType {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for ItemType {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for ItemType {
    const NAME: &'static str = "Item Type";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9C7D1BC3_1631_49C3_9F48_30DAEC50C479);
}
