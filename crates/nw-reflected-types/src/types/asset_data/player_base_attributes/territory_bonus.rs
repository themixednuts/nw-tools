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
pub enum TerritoryBonus {
    #[default]
    Invalid = 0,
    GainStanding = 1,
    GainXp = 2,
    GainStorage = 3,
    GainGatherRate = 4,
    GainFactionRep = 5,
    GainHousePoints = 6,
    TradingTax = 7,
    StationTax = 8,
    PropertyTax = 9,
    HousePurchase = 10,
}

impl From<TerritoryBonus> for u8 {
    fn from(value: TerritoryBonus) -> Self {
        match value {
            TerritoryBonus::Invalid => 0,
            TerritoryBonus::GainStanding => 1,
            TerritoryBonus::GainXp => 2,
            TerritoryBonus::GainStorage => 3,
            TerritoryBonus::GainGatherRate => 4,
            TerritoryBonus::GainFactionRep => 5,
            TerritoryBonus::GainHousePoints => 6,
            TerritoryBonus::TradingTax => 7,
            TerritoryBonus::StationTax => 8,
            TerritoryBonus::PropertyTax => 9,
            TerritoryBonus::HousePurchase => 10,
        }
    }
}

impl ::core::convert::TryFrom<u8> for TerritoryBonus {
    type Error = u8;
    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            0 => Ok(Self::Invalid),
            1 => Ok(Self::GainStanding),
            2 => Ok(Self::GainXp),
            3 => Ok(Self::GainStorage),
            4 => Ok(Self::GainGatherRate),
            5 => Ok(Self::GainFactionRep),
            6 => Ok(Self::GainHousePoints),
            7 => Ok(Self::TradingTax),
            8 => Ok(Self::StationTax),
            9 => Ok(Self::PropertyTax),
            10 => Ok(Self::HousePurchase),
            _ => Err(value),
        }
    }
}

impl TerritoryBonus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::GainStanding => "GainStanding",
            Self::GainXp => "GainXP",
            Self::GainStorage => "GainStorage",
            Self::GainGatherRate => "GainGatherRate",
            Self::GainFactionRep => "GainFactionRep",
            Self::GainHousePoints => "GainHousePoints",
            Self::TradingTax => "TradingTax",
            Self::StationTax => "StationTax",
            Self::PropertyTax => "PropertyTax",
            Self::HousePurchase => "HousePurchase",
        }
    }
}

impl AsRef<str> for TerritoryBonus {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for TerritoryBonus {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Invalid" => Ok(Self::Invalid),
            "GainStanding" => Ok(Self::GainStanding),
            "GainXP" => Ok(Self::GainXp),
            "GainStorage" => Ok(Self::GainStorage),
            "GainGatherRate" => Ok(Self::GainGatherRate),
            "GainFactionRep" => Ok(Self::GainFactionRep),
            "GainHousePoints" => Ok(Self::GainHousePoints),
            "TradingTax" => Ok(Self::TradingTax),
            "StationTax" => Ok(Self::StationTax),
            "PropertyTax" => Ok(Self::PropertyTax),
            "HousePurchase" => Ok(Self::HousePurchase),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for TerritoryBonus {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for TerritoryBonus {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for TerritoryBonus {
    const NAME: &'static str = "TerritoryBonus";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xED39905C_72B0_4299_AB4C_FD71E01D7AD0);
}
