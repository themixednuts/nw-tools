use super::ManagerTableInputSpec;

pub(super) const SPECS: &[ManagerTableInputSpec] = &[
    ManagerTableInputSpec {
        rust_type: "crate::AchievementDataManager",
        table_name: "AchievementDataTable",
        row_type_name: "AchievementData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::AchievementMetaDataManager",
        table_name: "AchievementMetaDataTable",
        row_type_name: "AchievementMetaData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::AffixDataManager",
        table_name: "AffixDataTable",
        row_type_name: "AffixData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::AffixDataManager",
        table_name: "AffixStatDataTable",
        row_type_name: "AffixStatData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::AITargetingDataManager",
        table_name: "AITargeting",
        row_type_name: "AITargetingData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::AmmoItemDataManager",
        table_name: "AmmoItemDefinitions",
        row_type_name: "AmmoItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::AmmoItemDataManager",
        table_name: "AmmoItemDefinitions_IsleOfNight",
        row_type_name: "AmmoItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::AppearanceTransformDataManager",
        table_name: "DefaultAppearanceTransforms",
        row_type_name: "AppearanceTransforms",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ArchetypeDataManager",
        table_name: "ArchetypeDataTable",
        row_type_name: "ArchetypeData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ArenaPvpBalanceDataManager",
        table_name: "ArenaPvpBalanceTable",
        row_type_name: "ArenaBalanceData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ArmorAppearanceDataManager",
        table_name: "ArmorAppearances",
        row_type_name: "ArmorAppearanceDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ArmorItemDataManager",
        table_name: "ArmorItemDefinitions",
        row_type_name: "ArmorItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::BeamDataManager",
        table_name: "Beams",
        row_type_name: "BeamData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::BlueprintItemDataManager",
        table_name: "Blueprint",
        row_type_name: "BlueprintItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::BuffBucketDataManager",
        table_name: "BuffBuckets",
        row_type_name: "BuffBucketData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CameraShakeDataManager",
        table_name: "CameraShakeDataTable",
        row_type_name: "CameraShakeData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CameraShakeDataManager",
        table_name: "CameraShakeDataTable_IsleOfNight",
        row_type_name: "CameraShakeData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CampSkinDataManager",
        table_name: "CampSkinDataTable",
        row_type_name: "CampSkinData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CaptureTheFlagPvpBalanceDataManager",
        table_name: "CaptureTheFlagPvpBalanceTable",
        row_type_name: "CaptureTheFlagBalanceData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CategoricalProgressionDataManager",
        table_name: "CategoricalProgression",
        row_type_name: "CategoricalProgressionData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CharacterAttributeDataManager",
        table_name: "Constitution",
        row_type_name: "AttributeDefinition",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CharacterAttributeDataManager",
        table_name: "Dexterity",
        row_type_name: "AttributeDefinition",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CharacterAttributeDataManager",
        table_name: "Focus",
        row_type_name: "AttributeDefinition",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CharacterAttributeDataManager",
        table_name: "Intelligence",
        row_type_name: "AttributeDefinition",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CharacterAttributeDataManager",
        table_name: "Strength",
        row_type_name: "AttributeDefinition",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CharmFilterDataManager",
        table_name: "CharmFilters",
        row_type_name: "CharmFilterData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CinematicVideoStaticDataManager",
        table_name: "CinematicVideo",
        row_type_name: "CinematicVideoStaticData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CollectibleStaticDataManager",
        table_name: "Collectibles",
        row_type_name: "CollectibleStaticData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CombatProfilesDataManager",
        table_name: "CombatProfilesDataTable",
        row_type_name: "CombatProfilesData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::CombatSettingsDataManager",
        table_name: "CombatSettingsDataTable",
        row_type_name: "CombatSettingsData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConsumableItemDataManager",
        table_name: "ConsumableItemDefinitions",
        row_type_name: "ConsumableItemDefinitions",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ContributionDataManager",
        table_name: "ArenaContribution",
        row_type_name: "ContributionData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ContributionDataManager",
        table_name: "Contribution",
        row_type_name: "ContributionData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ContributionDataManager",
        table_name: "DarknessContribution",
        row_type_name: "ContributionData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ContributionDataManager",
        table_name: "DefendObjectContribution",
        row_type_name: "ContributionData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ContributionDataManager",
        table_name: "InvasionContribution",
        row_type_name: "ContributionData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ContributionDataManager",
        table_name: "QuestEncContribution",
        row_type_name: "ContributionData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ContributionDataManager",
        table_name: "Season_02_Event_Contribution",
        row_type_name: "ContributionData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_74",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_75",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C01",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C02A",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C03",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C04A",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C05",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C06A",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C07",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C08",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C09A",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C10A",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C11",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C12A",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C13A",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C14",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C15",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C16",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C80",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C81",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C91",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C94",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C95_S04",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C95",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C95A",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C98",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C99A",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C99B",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C99C",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C99D",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C99E",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C99F",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates_C99G",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "ConversationStates",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_03976.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04288.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04289.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04290.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04291.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04292.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04293.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04294.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04295.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04296.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04297.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04627.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04628.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04629.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04630.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04631.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04632.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04633.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04634.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04635.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04636.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_04667.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_08012.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_08013.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16003.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16004.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16005.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16006.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16007.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16008.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16009.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16010.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16011.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16012.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16013.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16014.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16015.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16017.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16018.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16019.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16020.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16021.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16022.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16023.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16024.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16025.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16026.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16027.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16028.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16029.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16030.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16031.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16032.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16033.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16034.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16035.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16036.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16037.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16038.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16039.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16040.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16041.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16042.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16043.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16044.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16045.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16046.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16047.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16048.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16049.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16050.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16051.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16052.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16053.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16054.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16055.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16058.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16059.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16060.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16061.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16062.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16063.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16064.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16065.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16066.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16067.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16068.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16069.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16070.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16071.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16072.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16073.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16074.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16075.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16076.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16077.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16078.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16079.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16080.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16081.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16082.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16083.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16084.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16085.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16086.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16087.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16088.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16089.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16090.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16091.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16092.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16093.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16094.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16095.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16096.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16097.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16098.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16099.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16129.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16130.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16131.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16132.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16133.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16134.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16135.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16136.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16137.datasheet",
        row_type_name: "ConversationStateData",
    },
    ManagerTableInputSpec {
        rust_type: "crate::ConversationStateDataManager",
        table_name: "NPC_16138.datasheet",
        row_type_name: "ConversationStateData",
    },
];
