use super::ManagerTableInputSpec;

pub(super) const SPECS: &[ManagerTableInputSpec] = &[
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Achievements",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_ActivityCard",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Arena",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_CategoricalProgression",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Combat",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_CommitResource",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Consume",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Craft",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Duel",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_EquipItem",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Expedition",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_FactionControl",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Fishing",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_GameEvent",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Gather",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_JourneyTask",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Kill",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Level",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_OutpostRush",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Quest",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Salvage",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_Song",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SeasonsTrackedStatDataManager",
        table_name: "SeasonsRewardsStats_War",
        row_type_name: "SeasonsRewardsStats",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ShopDataManager",
        table_name: "ShopData",
        row_type_name: "ShopData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SongBookDataManager",
        table_name: "SongBookData",
        row_type_name: "SongBookData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SongBookSheetDataManager",
        table_name: "SongBookSheets",
        row_type_name: "SongBookSheets",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SimpleTreeCategoryDataManager",
        table_name: "MetaAchievementCategoryDataTable",
        row_type_name: "SimpleTreeCategoryData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::SimpleTreeCategoryDataManager",
        table_name: "PlayerTitleCategoryDataTable",
        row_type_name: "SimpleTreeCategoryData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StaticBackstoryDataManager",
        table_name: "Backstory",
        row_type_name: "BackstoryDefinition",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "ConsumableItemDefinitions",
        row_type_name: "ConsumableItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "Blueprint",
        row_type_name: "BlueprintItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "AffixStatDataTable",
        row_type_name: "AffixStatData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "ArmorItemDefinitions",
        row_type_name: "ArmorItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "RuneItemDefinitions",
        row_type_name: "WeaponItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "WeaponItemDefinitions",
        row_type_name: "WeaponItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "WeaponItemDefinitions_IsleOfNight",
        row_type_name: "WeaponItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_AI",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_AI_IsleOfNight",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Artifacts",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Blunderbuss",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Bow",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_CarryMe",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Catacombs",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Common",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Common_IsleOfNight",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_ConquerorsItems",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_CTF",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Dagger",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_DifficultyScaling",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Firestaff",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Flail",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Greataxe",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Greatsword",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Hatchet",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_IceMagic",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Items",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_JumpPad",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Lifestaff",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Musket",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Perks",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Perks2025",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_PerksGems",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_PerksInfix",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Rapier",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Runes",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_SetBonusesInfix",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Spear",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Sword",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_VoidGauntlet",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "StatusEffects_Warhammer",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "BaseVitals_Catacombs",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "BaseVitals_Common",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "BaseVitals_CutlassKeys",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "BaseVitals_Dunwood",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "BaseVitals_FirstLight",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "BaseVitals_IsleOfNight",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "BaseVitals_Player",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "BaseVitals_Raid_CutlassKeys",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "BaseVitals_WorldBoss",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatModifierDataManager",
        table_name: "FishingPolesMastersheet",
        row_type_name: "FishingPolesData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatMultiplierDataManager",
        table_name: "StatMultiplierTable",
        row_type_name: "StatMultiplierData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectCategoryDataManager",
        table_name: "StatusEffectCategories",
        row_type_name: "StatusEffectCategoryData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_AI",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_AI_IsleOfNight",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Artifacts",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Blunderbuss",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Bow",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_CarryMe",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Catacombs",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Common",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Common_IsleOfNight",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_ConquerorsItems",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_CTF",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Dagger",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_DifficultyScaling",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Firestaff",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Flail",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Greataxe",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Greatsword",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Hatchet",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_IceMagic",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Items",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_JumpPad",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Lifestaff",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Musket",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Perks",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Perks2025",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_PerksGems",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_PerksInfix",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Rapier",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Runes",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_SetBonusesInfix",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Spear",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Sword",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_VoidGauntlet",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StatusEffectDataManager",
        table_name: "StatusEffects_Warhammer",
        row_type_name: "StatusEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StoreCategoryDataManager",
        table_name: "StoreCategoryPropertiesTable",
        row_type_name: "StoreCategoryProperties",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StoreProductDataManager",
        table_name: "StoreProductData",
        row_type_name: "StoreProductData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StoryProgressDataManager",
        table_name: "StoryProgress",
        row_type_name: "StoryProgressData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StructureDataManager",
        table_name: "WallFootprint",
        row_type_name: "StructureFootprintData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::StructureDataManager",
        table_name: "T0_Wall_Pieces",
        row_type_name: "StructurePieceData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ThrowableItemDataManager",
        table_name: "ThrowableItemDefinitions",
        row_type_name: "ThrowableItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::TitleDataManager",
        table_name: "PlayerTitleDataTable",
        row_type_name: "PlayerTitleData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::TwitchDropsStatDataManager",
        table_name: "TwitchDropsStatDefinitions",
        row_type_name: "TwitchDropsStatDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsBaseDataManager",
        table_name: "BaseVitals_Catacombs",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsBaseDataManager",
        table_name: "BaseVitals_Common",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsBaseDataManager",
        table_name: "BaseVitals_CutlassKeys",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsBaseDataManager",
        table_name: "BaseVitals_Dunwood",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsBaseDataManager",
        table_name: "BaseVitals_FirstLight",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsBaseDataManager",
        table_name: "BaseVitals_IsleOfNight",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsBaseDataManager",
        table_name: "BaseVitals_Player",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsBaseDataManager",
        table_name: "BaseVitals_Raid_CutlassKeys",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsBaseDataManager",
        table_name: "BaseVitals_WorldBoss",
        row_type_name: "VitalsBaseData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsCategoryDataManager",
        table_name: "VitalsCategories",
        row_type_name: "VitalsCategoryData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsDataManager",
        table_name: "LevelVariantVitals_Catacombs",
        row_type_name: "VitalsLevelVariantData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsDataManager",
        table_name: "LevelVariantVitals_Common",
        row_type_name: "VitalsLevelVariantData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsDataManager",
        table_name: "LevelVariantVitals_CutlassKeys",
        row_type_name: "VitalsLevelVariantData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsDataManager",
        table_name: "LevelVariantVitals_Dunwood",
        row_type_name: "VitalsLevelVariantData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsDataManager",
        table_name: "LevelVariantVitals_FirstLight",
        row_type_name: "VitalsLevelVariantData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsDataManager",
        table_name: "LevelVariantVitals_IsleOfNight",
        row_type_name: "VitalsLevelVariantData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsDataManager",
        table_name: "LevelVariantVitals_Player",
        row_type_name: "VitalsLevelVariantData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsDataManager",
        table_name: "LevelVariantVitals_WorldBoss",
        row_type_name: "VitalsLevelVariantData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::VitalsDataManager",
        table_name: "Vitals_Raid_CutlassKeys",
        row_type_name: "VitalsLevelVariantData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WarPvpBalanceDataManager",
        table_name: "WarPvpBalanceTable",
        row_type_name: "WarBalanceData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponAccessoryDataManager",
        table_name: "WeaponAccessoryDefinitions",
        row_type_name: "WeaponAccessoryDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponAppearanceDataManager",
        table_name: "InstrumentsAppearanceDefinitions",
        row_type_name: "WeaponAppearanceDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponAppearanceDataManager",
        table_name: "WeaponAppearanceDefinitions",
        row_type_name: "WeaponAppearanceDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponAppearanceDataManager",
        table_name: "WeaponAppearanceDefinitions_MountAttachments",
        row_type_name: "WeaponAppearanceDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponEffectDataManager",
        table_name: "WeaponEffects",
        row_type_name: "WeaponEffectData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponItemDataManager",
        table_name: "RuneItemDefinitions",
        row_type_name: "WeaponItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponItemDataManager",
        table_name: "WeaponItemDefinitions",
        row_type_name: "WeaponItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponItemDataManager",
        table_name: "WeaponItemDefinitions_IsleOfNight",
        row_type_name: "WeaponItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponRefDataManager",
        table_name: "RuneItemDefinitions",
        row_type_name: "WeaponItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponRefDataManager",
        table_name: "WeaponItemDefinitions",
        row_type_name: "WeaponItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WeaponRefDataManager",
        table_name: "WeaponItemDefinitions_IsleOfNight",
        row_type_name: "WeaponItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::TimelineRegistryManager",
        table_name: "GenericTimelineRegistryEntry",
        row_type_name: "TimelineRegistryEntryData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::TimelineRegistryManager",
        table_name: "TimelineRegistryEntry",
        row_type_name: "TimelineRegistryEntryData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::TimelineRegistryManager",
        table_name: "WhisperTimelineRegistryEntry",
        row_type_name: "TimelineRegistryEntryData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WhisperDataManager",
        table_name: "WhisperDataManager",
        row_type_name: "WhisperData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WhisperDataManager",
        table_name: "WhisperVFXData",
        row_type_name: "WhisperVfxData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WorldEventCategoryDataManager",
        table_name: "WorldEventCategories",
        row_type_name: "WorldEventCategoryData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::WorldEventRuleDataManager",
        table_name: "WorldEventRules",
        row_type_name: "WorldEventRuleData",
    },
];
