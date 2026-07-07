use super::ManagerDependencyInputSpec;

pub(in crate::manager::specs::inputs) const MANAGER_DEPENDENCY_INPUT_SPECS:
    &[ManagerDependencyInputSpec] = &[
    ManagerDependencyInputSpec {
        rust_type: "crate::ElementalMutationStaticDataManager",
        resource: "crate::BuffBucketDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::ElementalMutationStaticDataManager",
        resource: "crate::StatusEffectDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::PromotionMutationStaticDataManager",
        resource: "crate::ElementalMutationStaticDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::PromotionMutationStaticDataManager",
        resource: "crate::BuffBucketDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::PromotionMutationStaticDataManager",
        resource: "crate::StatusEffectDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::StaticTradeskillRankDataMappingManager",
        resource: "crate::ExperienceDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::StaticTradeskillRankDataMappingManager",
        resource: "crate::PlayerDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::StaticTradeskillRankDataMappingManager",
        resource: "crate::CategoricalProgressionDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::StaticTradeskillRankDataMappingManager",
        resource: "crate::TradeskillRankDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::SeasonsRewardsActivitiesTasksDataManager",
        resource: "crate::SeasonsRewardsTaskDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::SeasonsRewardsBattlePassDataManager",
        resource: "crate::SeasonsRewardsSeasonDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::SeasonsRewardsChapterDataManager",
        resource: "crate::SeasonsRewardsSeasonDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::SeasonsRewardsJourneyDataManager",
        resource: "crate::SeasonsRewardsSeasonDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::SeasonsRewardsJourneyDataManager",
        resource: "crate::SeasonsRewardsTaskDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::SeasonsRewardsJourneyDataManager",
        resource: "crate::SeasonsRewardsChapterDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::ItemDataManager",
        resource: "crate::DyeItemDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::ItemDataManager",
        resource: "crate::MountDyeItemDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::ItemDataManager",
        resource: "crate::DyeColorDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::PerkBucketDataManager",
        resource: "crate::PerkDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::PerkBucketDataManager",
        resource: "crate::PerkExclusiveLabelDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::ProgressionPointDataManager",
        resource: "crate::ProgressionPoolDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::MusicalRewardsDataManager",
        resource: "crate::MusicalRankingDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::MusicalRewardsDataManager",
        resource: "crate::GameEventDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::SongBookSheetDataManager",
        resource: "crate::MusicalInstrumentSlotDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::SongBookDataManager",
        resource: "crate::SongBookSheetDataManager",
    },
    ManagerDependencyInputSpec {
        rust_type: "crate::ShopDataManager",
        resource: "crate::NPCDataManager",
    },
];
