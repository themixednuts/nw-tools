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
pub enum PlayerTeleportContext {
    Debug = 0,
    Ftue = 1,
    Spawn = 2,
    Stuck = 3,
    Overpopulation = 4,
    Encounter = 5,
    Interact = 6,
    WarEnd = 7,
    #[default]
    InvalidLocation = 8,
    HouseEnter = 9,
    HouseLeave = 10,
    FastTravelHousing = 11,
    FastTravelTerritory = 12,
    Other = 13,
    Respawn = 14,
    InnRecall = 15,
    HouseRespawn = 16,
    DungeonEnter = 17,
    DungeonLeave = 18,
    StructureObstruction = 19,
    GmTeleport = 20,
    MinorTeleport = 21,
    ServerTransfer = 22,
    PoiExit = 23,
    MilestoneCorrection = 24,
    TerritoryRework = 25,
    FfaMovement = 26,
    FfaDispersal = 27,
    Phasing = 28,
}

impl From<PlayerTeleportContext> for i32 {
    fn from(value: PlayerTeleportContext) -> Self {
        match value {
            PlayerTeleportContext::Debug => 0,
            PlayerTeleportContext::Ftue => 1,
            PlayerTeleportContext::Spawn => 2,
            PlayerTeleportContext::Stuck => 3,
            PlayerTeleportContext::Overpopulation => 4,
            PlayerTeleportContext::Encounter => 5,
            PlayerTeleportContext::Interact => 6,
            PlayerTeleportContext::WarEnd => 7,
            PlayerTeleportContext::InvalidLocation => 8,
            PlayerTeleportContext::HouseEnter => 9,
            PlayerTeleportContext::HouseLeave => 10,
            PlayerTeleportContext::FastTravelHousing => 11,
            PlayerTeleportContext::FastTravelTerritory => 12,
            PlayerTeleportContext::Other => 13,
            PlayerTeleportContext::Respawn => 14,
            PlayerTeleportContext::InnRecall => 15,
            PlayerTeleportContext::HouseRespawn => 16,
            PlayerTeleportContext::DungeonEnter => 17,
            PlayerTeleportContext::DungeonLeave => 18,
            PlayerTeleportContext::StructureObstruction => 19,
            PlayerTeleportContext::GmTeleport => 20,
            PlayerTeleportContext::MinorTeleport => 21,
            PlayerTeleportContext::ServerTransfer => 22,
            PlayerTeleportContext::PoiExit => 23,
            PlayerTeleportContext::MilestoneCorrection => 24,
            PlayerTeleportContext::TerritoryRework => 25,
            PlayerTeleportContext::FfaMovement => 26,
            PlayerTeleportContext::FfaDispersal => 27,
            PlayerTeleportContext::Phasing => 28,
        }
    }
}

impl ::core::convert::TryFrom<i32> for PlayerTeleportContext {
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::Debug),
            1 => Ok(Self::Ftue),
            2 => Ok(Self::Spawn),
            3 => Ok(Self::Stuck),
            4 => Ok(Self::Overpopulation),
            5 => Ok(Self::Encounter),
            6 => Ok(Self::Interact),
            7 => Ok(Self::WarEnd),
            8 => Ok(Self::InvalidLocation),
            9 => Ok(Self::HouseEnter),
            10 => Ok(Self::HouseLeave),
            11 => Ok(Self::FastTravelHousing),
            12 => Ok(Self::FastTravelTerritory),
            13 => Ok(Self::Other),
            14 => Ok(Self::Respawn),
            15 => Ok(Self::InnRecall),
            16 => Ok(Self::HouseRespawn),
            17 => Ok(Self::DungeonEnter),
            18 => Ok(Self::DungeonLeave),
            19 => Ok(Self::StructureObstruction),
            20 => Ok(Self::GmTeleport),
            21 => Ok(Self::MinorTeleport),
            22 => Ok(Self::ServerTransfer),
            23 => Ok(Self::PoiExit),
            24 => Ok(Self::MilestoneCorrection),
            25 => Ok(Self::TerritoryRework),
            26 => Ok(Self::FfaMovement),
            27 => Ok(Self::FfaDispersal),
            28 => Ok(Self::Phasing),
            _ => Err(value),
        }
    }
}

impl PlayerTeleportContext {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "Debug",
            Self::Ftue => "Ftue",
            Self::Spawn => "Spawn",
            Self::Stuck => "Stuck",
            Self::Overpopulation => "Overpopulation",
            Self::Encounter => "Encounter",
            Self::Interact => "Interact",
            Self::WarEnd => "War_End",
            Self::InvalidLocation => "Invalid_Location",
            Self::HouseEnter => "House_Enter",
            Self::HouseLeave => "House_Leave",
            Self::FastTravelHousing => "Fast_Travel_Housing",
            Self::FastTravelTerritory => "Fast_Travel_Territory",
            Self::Other => "Other",
            Self::Respawn => "Respawn",
            Self::InnRecall => "InnRecall",
            Self::HouseRespawn => "House_Respawn",
            Self::DungeonEnter => "Dungeon_Enter",
            Self::DungeonLeave => "Dungeon_Leave",
            Self::StructureObstruction => "StructureObstruction",
            Self::GmTeleport => "GM_Teleport",
            Self::MinorTeleport => "Minor_Teleport",
            Self::ServerTransfer => "ServerTransfer",
            Self::PoiExit => "POI_Exit",
            Self::MilestoneCorrection => "Milestone_Correction",
            Self::TerritoryRework => "Territory_Rework",
            Self::FfaMovement => "FFA_Movement",
            Self::FfaDispersal => "FFA_Dispersal",
            Self::Phasing => "Phasing",
        }
    }
}

impl AsRef<str> for PlayerTeleportContext {
    fn as_ref(&self) -> &str {
        (*self).as_str()
    }
}

impl<'a> ::core::convert::TryFrom<&'a str> for PlayerTeleportContext {
    type Error = &'a str;
    fn try_from(value: &'a str) -> Result<Self, &'a str> {
        match value {
            "Debug" => Ok(Self::Debug),
            "Ftue" => Ok(Self::Ftue),
            "Spawn" => Ok(Self::Spawn),
            "Stuck" => Ok(Self::Stuck),
            "Overpopulation" => Ok(Self::Overpopulation),
            "Encounter" => Ok(Self::Encounter),
            "Interact" => Ok(Self::Interact),
            "War_End" => Ok(Self::WarEnd),
            "Invalid_Location" => Ok(Self::InvalidLocation),
            "House_Enter" => Ok(Self::HouseEnter),
            "House_Leave" => Ok(Self::HouseLeave),
            "Fast_Travel_Housing" => Ok(Self::FastTravelHousing),
            "Fast_Travel_Territory" => Ok(Self::FastTravelTerritory),
            "Other" => Ok(Self::Other),
            "Respawn" => Ok(Self::Respawn),
            "InnRecall" => Ok(Self::InnRecall),
            "House_Respawn" => Ok(Self::HouseRespawn),
            "Dungeon_Enter" => Ok(Self::DungeonEnter),
            "Dungeon_Leave" => Ok(Self::DungeonLeave),
            "StructureObstruction" => Ok(Self::StructureObstruction),
            "GM_Teleport" => Ok(Self::GmTeleport),
            "Minor_Teleport" => Ok(Self::MinorTeleport),
            "ServerTransfer" => Ok(Self::ServerTransfer),
            "POI_Exit" => Ok(Self::PoiExit),
            "Milestone_Correction" => Ok(Self::MilestoneCorrection),
            "Territory_Rework" => Ok(Self::TerritoryRework),
            "FFA_Movement" => Ok(Self::FfaMovement),
            "FFA_Dispersal" => Ok(Self::FfaDispersal),
            "Phasing" => Ok(Self::Phasing),
            _ => Err(value),
        }
    }
}

impl ::core::str::FromStr for PlayerTeleportContext {
    type Err = ::std::string::String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value).map_err(str::to_owned)
    }
}

impl ::core::fmt::Display for PlayerTeleportContext {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl AzRtti for PlayerTeleportContext {
    const NAME: &'static str = "PlayerTeleportContext";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xB5B97E29_3B4E_464D_AFB6_B0F42E4947DD);
}
