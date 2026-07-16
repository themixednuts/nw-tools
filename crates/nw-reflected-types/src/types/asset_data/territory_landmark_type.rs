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
pub enum TerritoryLandmarkType {
    #[default]
    Invalid = 0,
    Claim = 4,
    CommunityGoalProvider = 8,
    CraftingStation = 11,
    FactionMissionProvider = 7,
    Fort = 2,
    GlobalStorage = 12,
    GovernorsDesk = 9,
    GuildRespawn = 15,
    HomeRespawn = 18,
    InnInteract = 21,
    InnRespawn = 20,
    Poi = 10,
    PrivateRespawn = 14,
    PublicRespawn = 16,
    RaidRespawn = 17,
    Settlement = 1,
    SettlementRespawn = 19,
    TradingPost = 13,
    TerritoryPlanningBoard = 6,
    WarBoard = 5,
    WarCamp = 3,
    Outpost = 22,
    CampRespawn = 24,
    FastTravelPoint = 27,
    Gatherable = 38,
    InfluenceTower = 39,
    FishingHotSpots = 23,
    OutpostRushSignUp = 25,
    WildZone = 26,
    WarCapturePointA = 28,
    WarCapturePointB = 29,
    WarCapturePointC = 30,
    WarClaimPoint = 31,
    FortGateA = 32,
    FortGateB = 33,
    FortGateC = 34,
    FortGateD = 35,
    FortGateE = 36,
    TransmogProvider = 40,
    TerritoryStorage = 41,
    WolfProvider = 43,
    LionProvider = 44,
    HorseProvider = 42,
    EventShop = 37,
    Shop = 45,
    Count = 46,
}

impl From<TerritoryLandmarkType> for u8 {
    fn from(value: TerritoryLandmarkType) -> Self {
        match value {
            TerritoryLandmarkType::Invalid => 0,
            TerritoryLandmarkType::Claim => 4,
            TerritoryLandmarkType::CommunityGoalProvider => 8,
            TerritoryLandmarkType::CraftingStation => 11,
            TerritoryLandmarkType::FactionMissionProvider => 7,
            TerritoryLandmarkType::Fort => 2,
            TerritoryLandmarkType::GlobalStorage => 12,
            TerritoryLandmarkType::GovernorsDesk => 9,
            TerritoryLandmarkType::GuildRespawn => 15,
            TerritoryLandmarkType::HomeRespawn => 18,
            TerritoryLandmarkType::InnInteract => 21,
            TerritoryLandmarkType::InnRespawn => 20,
            TerritoryLandmarkType::Poi => 10,
            TerritoryLandmarkType::PrivateRespawn => 14,
            TerritoryLandmarkType::PublicRespawn => 16,
            TerritoryLandmarkType::RaidRespawn => 17,
            TerritoryLandmarkType::Settlement => 1,
            TerritoryLandmarkType::SettlementRespawn => 19,
            TerritoryLandmarkType::TradingPost => 13,
            TerritoryLandmarkType::TerritoryPlanningBoard => 6,
            TerritoryLandmarkType::WarBoard => 5,
            TerritoryLandmarkType::WarCamp => 3,
            TerritoryLandmarkType::Outpost => 22,
            TerritoryLandmarkType::CampRespawn => 24,
            TerritoryLandmarkType::FastTravelPoint => 27,
            TerritoryLandmarkType::Gatherable => 38,
            TerritoryLandmarkType::InfluenceTower => 39,
            TerritoryLandmarkType::FishingHotSpots => 23,
            TerritoryLandmarkType::OutpostRushSignUp => 25,
            TerritoryLandmarkType::WildZone => 26,
            TerritoryLandmarkType::WarCapturePointA => 28,
            TerritoryLandmarkType::WarCapturePointB => 29,
            TerritoryLandmarkType::WarCapturePointC => 30,
            TerritoryLandmarkType::WarClaimPoint => 31,
            TerritoryLandmarkType::FortGateA => 32,
            TerritoryLandmarkType::FortGateB => 33,
            TerritoryLandmarkType::FortGateC => 34,
            TerritoryLandmarkType::FortGateD => 35,
            TerritoryLandmarkType::FortGateE => 36,
            TerritoryLandmarkType::TransmogProvider => 40,
            TerritoryLandmarkType::TerritoryStorage => 41,
            TerritoryLandmarkType::WolfProvider => 43,
            TerritoryLandmarkType::LionProvider => 44,
            TerritoryLandmarkType::HorseProvider => 42,
            TerritoryLandmarkType::EventShop => 37,
            TerritoryLandmarkType::Shop => 45,
            TerritoryLandmarkType::Count => 46,
        }
    }
}

impl ::core::convert::TryFrom<u8> for TerritoryLandmarkType {
    type Error = u8;
    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            0 => Ok(Self::Invalid),
            4 => Ok(Self::Claim),
            8 => Ok(Self::CommunityGoalProvider),
            11 => Ok(Self::CraftingStation),
            7 => Ok(Self::FactionMissionProvider),
            2 => Ok(Self::Fort),
            12 => Ok(Self::GlobalStorage),
            9 => Ok(Self::GovernorsDesk),
            15 => Ok(Self::GuildRespawn),
            18 => Ok(Self::HomeRespawn),
            21 => Ok(Self::InnInteract),
            20 => Ok(Self::InnRespawn),
            10 => Ok(Self::Poi),
            14 => Ok(Self::PrivateRespawn),
            16 => Ok(Self::PublicRespawn),
            17 => Ok(Self::RaidRespawn),
            1 => Ok(Self::Settlement),
            19 => Ok(Self::SettlementRespawn),
            13 => Ok(Self::TradingPost),
            6 => Ok(Self::TerritoryPlanningBoard),
            5 => Ok(Self::WarBoard),
            3 => Ok(Self::WarCamp),
            22 => Ok(Self::Outpost),
            24 => Ok(Self::CampRespawn),
            27 => Ok(Self::FastTravelPoint),
            38 => Ok(Self::Gatherable),
            39 => Ok(Self::InfluenceTower),
            23 => Ok(Self::FishingHotSpots),
            25 => Ok(Self::OutpostRushSignUp),
            26 => Ok(Self::WildZone),
            28 => Ok(Self::WarCapturePointA),
            29 => Ok(Self::WarCapturePointB),
            30 => Ok(Self::WarCapturePointC),
            31 => Ok(Self::WarClaimPoint),
            32 => Ok(Self::FortGateA),
            33 => Ok(Self::FortGateB),
            34 => Ok(Self::FortGateC),
            35 => Ok(Self::FortGateD),
            36 => Ok(Self::FortGateE),
            40 => Ok(Self::TransmogProvider),
            41 => Ok(Self::TerritoryStorage),
            43 => Ok(Self::WolfProvider),
            44 => Ok(Self::LionProvider),
            42 => Ok(Self::HorseProvider),
            37 => Ok(Self::EventShop),
            45 => Ok(Self::Shop),
            46 => Ok(Self::Count),
            _ => Err(value),
        }
    }
}

impl TerritoryLandmarkType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Claim => "Claim",
            Self::CommunityGoalProvider => "CommunityGoalProvider",
            Self::CraftingStation => "CraftingStation",
            Self::FactionMissionProvider => "FactionMissionProvider",
            Self::Fort => "Fort",
            Self::GlobalStorage => "GlobalStorage",
            Self::GovernorsDesk => "GovernorsDesk",
            Self::GuildRespawn => "GuildRespawn",
            Self::HomeRespawn => "HomeRespawn",
            Self::InnInteract => "InnInteract",
            Self::InnRespawn => "InnRespawn",
            Self::Poi => "POI",
            Self::PrivateRespawn => "PrivateRespawn",
            Self::PublicRespawn => "PublicRespawn",
            Self::RaidRespawn => "RaidRespawn",
            Self::Settlement => "Settlement",
            Self::SettlementRespawn => "SettlementRespawn",
            Self::TradingPost => "TradingPost",
            Self::TerritoryPlanningBoard => "TerritoryPlanningBoard",
            Self::WarBoard => "WarBoard",
            Self::WarCamp => "WarCamp",
            Self::Outpost => "Outpost",
            Self::CampRespawn => "CampRespawn",
            Self::FastTravelPoint => "FastTravelPoint",
            Self::Gatherable => "Gatherable",
            Self::InfluenceTower => "InfluenceTower",
            Self::FishingHotSpots => "FishingHotSpots",
            Self::OutpostRushSignUp => "OutpostRushSignUp",
            Self::WildZone => "WildZone",
            Self::WarCapturePointA => "WarCapturePoint_A",
            Self::WarCapturePointB => "WarCapturePoint_B",
            Self::WarCapturePointC => "WarCapturePoint_C",
            Self::WarClaimPoint => "WarClaimPoint",
            Self::FortGateA => "FortGate_A",
            Self::FortGateB => "FortGate_B",
            Self::FortGateC => "FortGate_C",
            Self::FortGateD => "FortGate_D",
            Self::FortGateE => "FortGate_E",
            Self::TransmogProvider => "TransmogProvider",
            Self::TerritoryStorage => "TerritoryStorage",
            Self::WolfProvider => "WolfProvider",
            Self::LionProvider => "LionProvider",
            Self::HorseProvider => "HorseProvider",
            Self::EventShop => "EventShop",
            Self::Shop => "Shop",
            Self::Count => "Count",
        }
    }
}

impl AsRef<str> for TerritoryLandmarkType {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for TerritoryLandmarkType {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Invalid" => Ok(Self::Invalid),
            "Claim" => Ok(Self::Claim),
            "CommunityGoalProvider" => Ok(Self::CommunityGoalProvider),
            "CraftingStation" => Ok(Self::CraftingStation),
            "FactionMissionProvider" => Ok(Self::FactionMissionProvider),
            "Fort" => Ok(Self::Fort),
            "GlobalStorage" => Ok(Self::GlobalStorage),
            "GovernorsDesk" => Ok(Self::GovernorsDesk),
            "GuildRespawn" => Ok(Self::GuildRespawn),
            "HomeRespawn" => Ok(Self::HomeRespawn),
            "InnInteract" => Ok(Self::InnInteract),
            "InnRespawn" => Ok(Self::InnRespawn),
            "POI" => Ok(Self::Poi),
            "PrivateRespawn" => Ok(Self::PrivateRespawn),
            "PublicRespawn" => Ok(Self::PublicRespawn),
            "RaidRespawn" => Ok(Self::RaidRespawn),
            "Settlement" => Ok(Self::Settlement),
            "SettlementRespawn" => Ok(Self::SettlementRespawn),
            "TradingPost" => Ok(Self::TradingPost),
            "TerritoryPlanningBoard" => Ok(Self::TerritoryPlanningBoard),
            "WarBoard" => Ok(Self::WarBoard),
            "WarCamp" => Ok(Self::WarCamp),
            "Outpost" => Ok(Self::Outpost),
            "CampRespawn" => Ok(Self::CampRespawn),
            "FastTravelPoint" => Ok(Self::FastTravelPoint),
            "Gatherable" => Ok(Self::Gatherable),
            "InfluenceTower" => Ok(Self::InfluenceTower),
            "FishingHotSpots" => Ok(Self::FishingHotSpots),
            "OutpostRushSignUp" => Ok(Self::OutpostRushSignUp),
            "WildZone" => Ok(Self::WildZone),
            "WarCapturePoint_A" => Ok(Self::WarCapturePointA),
            "WarCapturePoint_B" => Ok(Self::WarCapturePointB),
            "WarCapturePoint_C" => Ok(Self::WarCapturePointC),
            "WarClaimPoint" => Ok(Self::WarClaimPoint),
            "FortGate_A" => Ok(Self::FortGateA),
            "FortGate_B" => Ok(Self::FortGateB),
            "FortGate_C" => Ok(Self::FortGateC),
            "FortGate_D" => Ok(Self::FortGateD),
            "FortGate_E" => Ok(Self::FortGateE),
            "TransmogProvider" => Ok(Self::TransmogProvider),
            "TerritoryStorage" => Ok(Self::TerritoryStorage),
            "WolfProvider" => Ok(Self::WolfProvider),
            "LionProvider" => Ok(Self::LionProvider),
            "HorseProvider" => Ok(Self::HorseProvider),
            "EventShop" => Ok(Self::EventShop),
            "Shop" => Ok(Self::Shop),
            "Count" => Ok(Self::Count),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for TerritoryLandmarkType {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for TerritoryLandmarkType {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for TerritoryLandmarkType {
    const NAME: &'static str = "TerritoryLandmarkType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0BE7E72F_C7E2_4AD6_9E6F_FDDFF94CC221);
}
