use super::*;

pub(super) fn replication_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::ReplicationDataManager",
        "crate::ReplicationDataManager",
        vec![NativeManagerInput::manager(rust_type(
            "crate::PerkDataManager",
        ))],
        vec!["Javelin::ReplicationDataManager::ReplicationDataManager"],
    )
    .with_shape(NativeManagerShape::replication_data(
        NativeReplicationDataManager::new(ident("replication_data")),
    ))
}

pub(super) fn character_attribute_data_manager_spec() -> NativeManagerSpec {
    let tables = native_table_family_tables(
        inputs::manager_table_family_specs("crate::CharacterAttributeDataManager")
            .into_iter()
            .filter(|table| table.row_type_name == "AttributeDefinition"),
    );
    let shape = NativeCharacterAttributeDataManager::new(ident("character_attribute_data"), tables)
        .expect("validated character attribute data manager shape");

    manager_spec_with_inputs(
        "Javelin::CharacterAttributeDataManager",
        "crate::CharacterAttributeDataManager",
        Vec::new(),
        vec![
            "Javelin::CharacterAttributeDataManager::CharacterAttributeDataManager",
            "Javelin::CharacterAttributeDataManager::CacheAllAttributeDataTables",
            "Javelin::CharacterAttributeDataManager::GetAttributeData",
        ],
    )
    .with_shape(NativeManagerShape::character_attribute_data(shape))
}

pub(super) fn damage_data_manager_spec() -> NativeManagerSpec {
    let damage_tables = native_table_family_tables(
        inputs::manager_table_family_specs("crate::DamageDataManager")
            .into_iter()
            .filter(|table| table.row_type_name == "DamageData"),
    );
    let shape = NativeDamageDataManager::new(ident("damage_data"), damage_tables)
        .expect("validated damage data manager shape");

    manager_spec_with_inputs(
        "Javelin::DamageDataManager",
        "crate::DamageDataManager",
        Vec::new(),
        vec![
            "Javelin::DamageDataManager::DamageDataManager",
            "Javelin::DamageDataManager::CacheDamageDataTable",
            "Javelin::DamageDataManager::CacheDamageTypeDataTable",
            "Javelin::DamageDataManager::CacheAfflictionDataTable",
            "Javelin::DamageDataManager::GetDamageData",
            "Javelin::DamageDataManager::GetDamageDataRowKey",
            "Javelin::DamageDataManager::GetDamageTypeData",
            "Javelin::DamageDataManager::GetDamageTypeIntID",
            "Javelin::DamageDataManager::GetAfflictionData",
            "Javelin::DamageDataManager::GetAfflictionIntID",
        ],
    )
    .with_shape(NativeManagerShape::damage_data(shape))
}

pub(super) fn vitals_data_manager_spec() -> NativeManagerSpec {
    let tables = native_table_family_tables(
        inputs::manager_table_family_specs("crate::VitalsDataManager")
            .into_iter()
            .filter(|table| table.row_type_name == "VitalsLevelVariantData"),
    );
    let shape = NativeVitalsDataManager::new(ident("vitals_data"), tables)
        .expect("validated vitals data manager shape");

    manager_spec_with_inputs(
        "Javelin::VitalsDataManager",
        "crate::VitalsDataManager",
        vec![NativeManagerInput::manager(rust_type(
            "crate::VitalsBaseDataManager",
        ))],
        vec![
            "Javelin::VitalsDataManager::VitalsDataManager",
            "Javelin::VitalsDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::vitals_data(shape))
}

pub(super) fn status_effect_data_manager_spec() -> NativeManagerSpec {
    let tables = native_table_family_tables(
        inputs::manager_table_family_specs("crate::StatusEffectDataManager")
            .into_iter()
            .filter(|table| table.row_type_name == "StatusEffectData"),
    );
    let shape = NativeStatusEffectDataManager::new(ident("status_effect_data"), tables)
        .expect("validated status effect data manager shape");

    manager_spec_with_inputs(
        "Javelin::StatusEffectDataManager",
        "crate::StatusEffectDataManager",
        Vec::new(),
        vec![
            "Javelin::StatusEffectDataManager::StatusEffectDataManager",
            "Javelin::StatusEffectDataManager::CacheAllStatusEffectDataTables",
            "Javelin::StatusEffectDataManager::GetStatusEffectDataById",
            "Javelin::StatusEffectDataManager::GetStatusEffectDataByName",
            "Javelin::StatusEffectDataManager::TryGetStatusEffectDataById",
            "Javelin::StatusEffectDataManager::TryGetStatusEffectDataByName",
            "Javelin::StatusEffectDataManager::GetStatusEffectIds",
        ],
    )
    .with_shape(NativeManagerShape::status_effect_data(shape))
}

pub(super) fn player_data_manager_spec() -> NativeManagerSpec {
    let products = vec![
        product_asset_resource(
            "sharedassets/genericassets/playerbaseattributes.pbadb",
            "newworld_plugin::assets::player_base_attributes::PlayerBaseAttributesAsset",
            "newworld_plugin::assets::player_base_attributes::PlayerBaseAttributes",
            "player_base_attributes",
            "attributes",
            "player_base_attributes",
        ),
        product_asset_resource(
            "sharedassets/genericassets/settlementprogression.sprd",
            "newworld_plugin::assets::settlement_progression_data::SettlementProgressionDataAsset",
            "newworld_plugin::assets::settlement_progression_data::SettlementProgressionData",
            "settlement_progression_data",
            "data",
            "settlement_progression_data",
        ),
    ];
    let product_inputs = products
        .iter()
        .map(|product| product.product)
        .collect::<Vec<_>>();
    let product_assets = NativeProductAssetResourceManager::new(
        rust_type("crate::PlayerDataManager"),
        ident("new"),
        products
            .into_iter()
            .map(|product| {
                NativeProductAssetResource::new(
                    rust_type(product.product.rust_type),
                    rust_type(product.value_type),
                    ident(product.handle_getter),
                    ident(product.asset_getter),
                    ident(product.manager_getter),
                )
            })
            .collect(),
    )
    .expect("validated player data product asset shape");

    product_manager_spec_with_format(
        "Javelin::PlayerDataManager",
        "crate::PlayerDataManager",
        NativeManagerProductFormat::ObjectStream,
        product_inputs,
        vec![
            "Javelin::PlayerDataManager::PlayerDataManager",
            "Javelin::PlayerDataManager::CachePlayerData",
            "Javelin::PlayerDataManager::FUN_7ff6067eda30",
        ],
    )
    .with_shape(NativeManagerShape::player_data(
        NativePlayerDataManager::new(ident("player_data"), product_assets),
    ))
}

pub(super) fn tradeskill_rank_data_manager_spec() -> NativeManagerSpec {
    let rank_tables = native_table_family_tables(
        inputs::manager_table_family_specs("crate::TradeskillRankDataManager")
            .into_iter()
            .filter(|table| table.row_type_name == "TradeskillRankData"),
    );
    let shape = NativeTradeskillRankDataManager::new(
        ident("tradeskill_rank_data"),
        game_table("XPLevels"),
        game_row_type("ExperienceData"),
        rank_tables,
    )
    .expect("validated tradeskill rank data manager shape");

    manager_spec_with_inputs(
        "Javelin::TradeskillRankDataManager",
        "crate::TradeskillRankDataManager",
        Vec::new(),
        vec![
            "Javelin::TradeskillRankDataManager::TradeskillRankDataManager",
            "Javelin::TradeskillRankDataManager::CacheAllDataTables",
            "Javelin::StaticTradeskillRankDataMappingManager::StaticTradeskillRankDataMappingManager",
            "Javelin::StaticTradeskillRankDataMappingManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::tradeskill_rank_data(shape))
}

pub(super) fn static_tradeskill_rank_data_mapping_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::StaticTradeskillRankDataMappingManager",
        "crate::StaticTradeskillRankDataMappingManager",
        vec![
            NativeManagerInput::manager(rust_type("crate::ExperienceDataManager")),
            NativeManagerInput::manager(rust_type("crate::PlayerDataManager")),
            NativeManagerInput::manager(rust_type("crate::CategoricalProgressionDataManager")),
            NativeManagerInput::manager(rust_type("crate::TradeskillRankDataManager")),
        ],
        vec![
            "Javelin::StaticTradeskillRankDataMappingManager::StaticTradeskillRankDataMappingManager",
            "Javelin::StaticTradeskillRankDataMappingManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::static_tradeskill_rank_data_mapping(
        NativeStaticTradeskillRankDataMappingManager::new(ident(
            "static_tradeskill_rank_data_mapping",
        )),
    ))
}

pub(super) fn currency_exchange_mapping_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::CurrencyExchangeMappingManager",
        "crate::CurrencyExchangeMappingManager",
        vec![
            NativeManagerInput::manager(rust_type("crate::CurrencyExchangeDataManager")),
            NativeManagerInput::manager(rust_type("crate::CategoricalProgressionDataManager")),
        ],
        // Behavior-only evidence for the validated mapping cache routine at
        // NewWorld+0x43e7f90. Keep the FUN_* label until the exact native
        // method name is proven.
        vec!["Javelin::CurrencyExchangeMappingManager::FUN_7ff6043e7f90"],
    )
    .with_shape(NativeManagerShape::currency_exchange_mapping(
        NativeCurrencyExchangeMappingManager::new(ident("currency_exchange_mapping")),
    ))
}

pub(super) fn dynamic_difficulty_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::DynamicDifficultyDataManager",
        "crate::DynamicDifficultyDataManager",
        vec![
            NativeManagerInput::table(
                game_table("DynamicDifficulty"),
                game_row_type("DynamicDifficultyStaticData"),
            ),
            NativeManagerInput::manager(rust_type("crate::VitalsDataManager")),
        ],
        vec![
            "Javelin::DynamicDifficultyDataManager::DynamicDifficultyDataManager",
            "Javelin::DynamicDifficultyDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::dynamic_difficulty_data(
        NativeDynamicDifficultyDataManager::new(ident("dynamic_difficulty_data")),
    ))
}

pub(super) fn elemental_mutation_static_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::ElementalMutationStaticDataManager",
        "crate::ElementalMutationStaticDataManager",
        vec![
            NativeManagerInput::table(
                game_table("ElementalMutation"),
                game_row_type("ElementalMutationStaticData"),
            ),
            NativeManagerInput::manager(rust_type("crate::BuffBucketDataManager")),
            NativeManagerInput::manager(rust_type("crate::StatusEffectDataManager")),
        ],
        vec![
            "Javelin::ElementalMutationStaticDataManager::ElementalMutationStaticDataManager",
            "Javelin::ElementalMutationStaticDataManager::GetPossibleElementalStatusEffects",
        ],
    )
    .with_shape(NativeManagerShape::elemental_mutation_static_data(
        NativeElementalMutationStaticDataManager::new(ident("elemental_mutation_static_data")),
    ))
}

pub(super) fn promotion_mutation_static_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::PromotionMutationStaticDataManager",
        "crate::PromotionMutationStaticDataManager",
        vec![
            NativeManagerInput::table(
                game_table("PromotionMutation"),
                game_row_type("PromotionMutationStaticData"),
            ),
            NativeManagerInput::manager(rust_type(
                "crate::ElementalMutationStaticDataManager",
            )),
            NativeManagerInput::manager(rust_type("crate::BuffBucketDataManager")),
            NativeManagerInput::manager(rust_type("crate::StatusEffectDataManager")),
        ],
        vec![
            "Javelin::PromotionMutationStaticDataManager::PromotionMutationStaticDataManager",
            "Javelin::PromotionMutationStaticDataManager::CacheAllDataTables",
            "Javelin::PromotionMutationStaticDataManager::GetPossiblePromotionalStatusEffectsForElement",
        ],
    )
    .with_shape(NativeManagerShape::promotion_mutation_static_data(
        NativePromotionMutationStaticDataManager::new(ident("promotion_mutation_static_data")),
    ))
}

pub(super) fn musical_rewards_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::MusicalRewardsDataManager",
        "crate::MusicalRewardsDataManager",
        vec![
            NativeManagerInput::table(
                game_table("MusicalPerformanceRewardsTable"),
                game_row_type("MusicalPerformanceRewards"),
            ),
            NativeManagerInput::manager(rust_type("crate::MusicalRankingDataManager")),
            NativeManagerInput::manager(rust_type("crate::GameEventDataManager")),
        ],
        vec![
            "Javelin::MusicalRewardsDataManager::MusicalRewardsDataManager",
            "Javelin::MusicalRewardsDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::musical_rewards_data(
        NativeMusicalRewardsDataManager::new(ident("musical_rewards_data")),
    ))
}

pub(super) fn progression_point_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::ProgressionPointDataManager",
        "crate::ProgressionPointDataManager",
        vec![
            NativeManagerInput::table(
                game_table("ProgressionPoints"),
                game_row_type("ProgressionPointData"),
            ),
            NativeManagerInput::manager(rust_type("crate::ProgressionPoolDataManager")),
        ],
        vec![
            "Javelin::ProgressionPointDataManager::ProgressionPointDataManager",
            "Javelin::ProgressionPointDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::progression_point_data(
        NativeProgressionPointDataManager::new(ident("progression_point_data")),
    ))
}

pub(super) fn combat_profiles_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::CombatProfilesDataManager",
        "crate::CombatProfilesDataManager",
        inputs::manager_inputs_for_manifest(
            "crate::CombatProfilesDataManager",
            vec![NativeManagerInput::manager(rust_type(
                "crate::CombatSettingsDataManager",
            ))],
        ),
        vec![
            "Javelin::CombatProfilesDataManager::CombatProfilesDataManager",
            "Javelin::CombatProfilesDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::combat_profiles_data(
        NativeCombatProfilesDataManager::new(ident("combat_profiles_data")),
    ))
}

pub(super) fn item_transform_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::ItemTransformDataManager",
        "crate::ItemTransformDataManager",
        inputs::manager_inputs_for_manifest(
            "crate::ItemTransformDataManager",
            vec![NativeManagerInput::manager(rust_type(
                "crate::ItemDataManager",
            ))],
        ),
        vec!["Javelin::ItemTransformDataManager::ItemTransformDataManager"],
    )
    .with_shape(NativeManagerShape::item_transform_data(
        NativeItemTransformDataManager::new(ident("item_transform_data")),
    ))
}

pub(super) fn gatherable_data_manager_spec() -> NativeManagerSpec {
    let gathering_database = product_asset_resource(
        "sharedassets/genericassets/gathering/gatheringdatabase.gdb",
        "newworld_plugin::assets::gathering_database::GatheringDatabaseAsset",
        "newworld_plugin::assets::gathering_database::GatheringDatabase",
        "gathering_database",
        "database",
        "gathering_database",
    );
    let gathering_action_database = product_asset_resource(
        "sharedassets/genericassets/gatheringactiondatabase.gactdb",
        "newworld_plugin::assets::gathering_database::GatheringActionDatabaseAsset",
        "newworld_plugin::assets::gathering_database::GatheringActionDatabase",
        "gathering_action_database",
        "database",
        "gathering_action_database",
    );

    manager_spec_with_inputs(
        "Javelin::GatherableDataManager",
        "crate::GatherableDataManager",
        inputs::manager_inputs_for_manifest(
            "crate::GatherableDataManager",
            vec![
                NativeManagerInput::object_stream_product(
                    asset_path(gathering_database.product.asset_path),
                    rust_type(gathering_database.product.rust_type),
                ),
                NativeManagerInput::object_stream_product(
                    asset_path(gathering_action_database.product.asset_path),
                    rust_type(gathering_action_database.product.rust_type),
                ),
            ],
        ),
        vec![
            "Javelin::GatherableDataManager::GatherableDataManager",
            "Javelin::GatherableDataManager::CacheGatherableDataTables",
        ],
    )
    .with_shape(NativeManagerShape::gatherable_data(
        NativeGatherableDataManager::new(
            ident("gatherable_data"),
            native_product_asset_resource(gathering_database),
            native_product_asset_resource(gathering_action_database),
        ),
    ))
}

pub(super) fn social_data_manager_spec() -> NativeManagerSpec {
    let rank_database = product_asset_resource(
        "sharedassets/genericassets/rankdatabase.rankdb",
        "newworld_plugin::assets::rank_database::SocialRankDatabaseAsset",
        "newworld_plugin::assets::rank_database::SocialRankDatabase",
        "social_rank_database",
        "database",
        "rank_database",
    );

    manager_spec_with_inputs(
        "Javelin::SocialDataManager",
        "crate::SocialDataManager",
        inputs::manager_inputs_for_manifest(
            "crate::SocialDataManager",
            vec![
                NativeManagerInput::manager(rust_type("crate::CrestPartDataManager")),
                NativeManagerInput::object_stream_product(
                    asset_path(rank_database.product.asset_path),
                    rust_type(rank_database.product.rust_type),
                ),
            ],
        ),
        vec!["Javelin::SocialDataManager::SocialDataManager"],
    )
    .with_shape(NativeManagerShape::social_data(
        NativeSocialDataManager::new(
            ident("social_data"),
            native_product_asset_resource(rank_database),
        ),
    ))
}

pub(super) fn seasons_rewards_activities_tasks_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::SeasonsRewardsActivitiesTasksDataManager",
        "crate::SeasonsRewardsActivitiesTasksDataManager",
        inputs::manager_inputs_for_manifest(
            "crate::SeasonsRewardsActivitiesTasksDataManager",
            Vec::new(),
        ),
        vec![
            "Javelin::SeasonsRewardsActivitiesTasksDataManager::SeasonsRewardsActivitiesTasksDataManager",
            "Javelin::SeasonsRewardsActivitiesTasksDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsActivitiesTasksDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsActivitiesTasksDataManager::CacheTableRows",
        ],
    )
    .with_shape(NativeManagerShape::seasons_rewards_activities_tasks_data(
        NativeSeasonsRewardsActivitiesTasksDataManager::new(ident(
            "seasons_rewards_activities_tasks_data",
        )),
    ))
}

pub(super) fn seasons_rewards_battle_pass_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::SeasonsRewardsBattlePassDataManager",
        "crate::SeasonsRewardsBattlePassDataManager",
        inputs::manager_inputs_for_manifest(
            "crate::SeasonsRewardsBattlePassDataManager",
            Vec::new(),
        ),
        vec![
            "Javelin::SeasonsRewardsBattlePassDataManager::SeasonsRewardsBattlePassDataManager",
            "Javelin::SeasonsRewardsBattlePassDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsBattlePassDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsBattlePassDataManager::CacheTableRows",
        ],
    )
    .with_shape(NativeManagerShape::seasons_rewards_battle_pass_data(
        NativeSeasonsRewardsBattlePassDataManager::new(ident("seasons_rewards_battle_pass_data")),
    ))
}

pub(super) fn seasons_rewards_card_template_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::SeasonsRewardsCardTemplateDataManager",
        "crate::SeasonsRewardsCardTemplateDataManager",
        inputs::manager_inputs_for_manifest(
            "crate::SeasonsRewardsCardTemplateDataManager",
            Vec::new(),
        ),
        vec![
            "Javelin::SeasonsRewardsCardTemplateDataManager::SeasonsRewardsCardTemplateDataManager",
            "Javelin::SeasonsRewardsCardTemplateDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsCardTemplateDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsCardTemplateDataManager::CacheTableRows",
            "Javelin::SeasonsRewardsCardTemplateDataManager::DecodeRow",
        ],
    )
    .with_shape(NativeManagerShape::seasons_rewards_card_template_data(
        NativeSeasonsRewardsCardTemplateDataManager::new(ident(
            "seasons_rewards_card_template_data",
        )),
    ))
}

pub(super) fn seasons_rewards_chapter_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::SeasonsRewardsChapterDataManager",
        "crate::SeasonsRewardsChapterDataManager",
        inputs::manager_inputs_for_manifest("crate::SeasonsRewardsChapterDataManager", Vec::new()),
        vec![
            "Javelin::SeasonsRewardsChapterDataManager::SeasonsRewardsChapterDataManager",
            "Javelin::SeasonsRewardsChapterDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsChapterDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsChapterDataManager::CacheTableRows",
            "Javelin::SeasonsRewardsChapterDataManager::FindGameSystemDataByKey",
            "Javelin::SeasonsRewardsChapterDataManager::GetGameSystemData",
        ],
    )
    .with_shape(NativeManagerShape::seasons_rewards_chapter_data(
        NativeSeasonsRewardsChapterDataManager::new(ident("seasons_rewards_chapter_data")),
    ))
}

pub(super) fn seasons_rewards_journey_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::SeasonsRewardsJourneyDataManager",
        "crate::SeasonsRewardsJourneyDataManager",
        inputs::manager_inputs_for_manifest("crate::SeasonsRewardsJourneyDataManager", Vec::new()),
        vec![
            "Javelin::SeasonsRewardsJourneyDataManager::SeasonsRewardsJourneyDataManager",
            "Javelin::SeasonsRewardsJourneyDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsJourneyDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsJourneyDataManager::CacheTableRows",
            "Javelin::SeasonsRewardsJourneyDataManager::PopulateDataTableEntryKeys",
            "Javelin::SeasonsRewardsJourneyDataManager::FindGameSystemDataByKey",
            "Javelin::SeasonsRewardsJourneyDataManager::GetGameSystemData",
        ],
    )
    .with_shape(NativeManagerShape::seasons_rewards_journey_data(
        NativeSeasonsRewardsJourneyDataManager::new(ident("seasons_rewards_journey_data")),
    ))
}

pub(super) fn song_book_sheet_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::SongBookSheetDataManager",
        "crate::SongBookSheetDataManager",
        vec![
            NativeManagerInput::table(
                game_table("SongBookSheets"),
                game_row_type("SongBookSheets"),
            ),
            NativeManagerInput::manager(rust_type("crate::MusicalInstrumentSlotDataManager")),
        ],
        vec![
            "Javelin::SongBookSheetDataManager::SongBookSheetDataManager",
            "Javelin::SongBookSheetDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::song_book_sheet_data(
        NativeSongBookSheetDataManager::new(ident("song_book_sheet_data")),
    ))
}

pub(super) fn song_book_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::SongBookDataManager",
        "crate::SongBookDataManager",
        vec![
            NativeManagerInput::table(game_table("SongBookData"), game_row_type("SongBookData")),
            NativeManagerInput::manager(rust_type("crate::SongBookSheetDataManager")),
        ],
        vec![
            "Javelin::SongBookDataManager::SongBookDataManager",
            "Javelin::SongBookDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::song_book_data(
        NativeSongBookDataManager::new(ident("song_book_data")),
    ))
}

pub(super) fn vitals_modifier_mapping_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::VitalsModifierMappingManager",
        "crate::VitalsModifierMappingManager",
        vec![
            NativeManagerInput::manager(rust_type("crate::VitalsDataManager")),
            NativeManagerInput::manager(rust_type("crate::DamageDataManager")),
            NativeManagerInput::manager(rust_type("crate::ItemDataManager")),
        ],
        vec!["Javelin::VitalsModifierMappingManager::FUN_7ff6044e6ce0"],
    )
    .with_shape(NativeManagerShape::vitals_modifier_mapping(
        NativeVitalsModifierMappingManager::new(ident("vitals_modifier_mapping")),
    ))
}

pub(super) fn elemental_mutation_perks_static_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "elemental_mutation_perks_static_data",
        table_name: "MutationPerks",
        row_type_name: "MutationPerksStaticData",
        data_type: "ElementalMutationPerksStaticData",
        entries_field: "perks",
        index_field: "perks_by_crc",
        key_field: "elemental_mutation_type_key",
        crc_field: "elemental_mutation_type_id",
        key_column: "ElementalMutationTypeId",
        key_getter: "elemental_mutation_type_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("elemental_mutation_perks_for_source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "injected_perk_bucket1_key",
                "InjectedPerkBucket1",
                "injected_perk_bucket1",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "injected_perk_bucket1",
                "InjectedPerkBucket1",
                "injected_perk_bucket1",
                NativeProjectionTransform::OptionalLowercaseCrcString,
            ),
            projection_field(
                "injected_perk_bucket_weight1",
                "InjectedPerkBucketWeight1",
                "injected_perk_bucket_weight1",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "injected_perk_bucket2_row",
                "InjectedPerkBucket2",
                "injected_perk_bucket2",
                NativeProjectionTransform::ForeignKeyRow,
            ),
            projection_field(
                "injected_perk_bucket_weight2",
                "InjectedPerkBucketWeight2",
                "injected_perk_bucket_weight2",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "injected_perk_bucket3_key",
                "InjectedPerkBucket3",
                "injected_perk_bucket3",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "injected_perk_bucket3",
                "InjectedPerkBucket3",
                "injected_perk_bucket3",
                NativeProjectionTransform::OptionalLowercaseCrcString,
            ),
            projection_field(
                "injected_perk_bucket_weight3",
                "InjectedPerkBucketWeight3",
                "injected_perk_bucket_weight3",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "injected_perk_bucket4_key",
                "InjectedPerkBucket4",
                "injected_perk_bucket4",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "injected_perk_bucket4",
                "InjectedPerkBucket4",
                "injected_perk_bucket4",
                NativeProjectionTransform::OptionalLowercaseCrcString,
            ),
            projection_field(
                "injected_perk_bucket_weight4",
                "InjectedPerkBucketWeight4",
                "injected_perk_bucket_weight4",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "injected_perk_bucket5_key",
                "InjectedPerkBucket5",
                "injected_perk_bucket5",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "injected_perk_bucket5",
                "InjectedPerkBucket5",
                "injected_perk_bucket5",
                NativeProjectionTransform::OptionalLowercaseCrcString,
            ),
            projection_field(
                "injected_perk_bucket_weight5",
                "InjectedPerkBucketWeight5",
                "injected_perk_bucket_weight5",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "injected_creature_loot",
                "InjectedCreatureLoot",
                "injected_creature_loot",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "injected_container_loot",
                "InjectedContainerLoot",
                "injected_container_loot",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "injected_loot_tags",
                "InjectedLootTags",
                "injected_loot_tags",
                NativeProjectionTransform::String,
            ),
            projection_field("name", "Name", "name", NativeProjectionTransform::String),
            projection_field(
                "description",
                "Description",
                "description",
                NativeProjectionTransform::String,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "elemental_mutation_perks_from_id",
                "elemental_mutation_type_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "elemental_mutation_perks",
                "elemental_mutation_type_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "elemental_mutation_perks_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("perks"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::ElementalMutationPerksStaticDataManager",
        rust_type: "crate::ElementalMutationPerksStaticDataManager",
        ghidra_functions: Vec::new(),
    })
}

pub(super) fn flexible_mission_board_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "flexible_mission_board_data",
        table_name: "FlexibleMissionBoardData",
        row_type_name: "FlexibleMissionBoardData",
        data_type: "FlexibleMissionBoardData",
        entries_field: "boards",
        index_field: "boards_by_crc",
        key_field: "flexible_mission_board_key",
        crc_field: "flexible_mission_board_id",
        key_column: "FlexibleMissionBoardId",
        key_getter: "flexible_mission_board_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("flexible_mission_board_data_for_source_row"),
        row_filters: vec![lowercase_crc_string_nonzero_filter(
            "ReputationId",
            "reputation_id",
        )],
        fields: vec![
            projection_field(
                "mission_board_name_key",
                "MissionBoardName",
                "mission_board_name",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "mission_board_name",
                "MissionBoardName",
                "mission_board_name",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "reputation_key",
                "ReputationId",
                "reputation_id",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "reputation_id",
                "ReputationId",
                "reputation_id",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "mission_objective_type_key",
                "MissionObjectiveType",
                "mission_objective_type",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "mission_objective_type",
                "MissionObjectiveType",
                "mission_objective_type",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "mission_weights_bucket_key",
                "MissionWeightsBucket",
                "mission_weights_bucket",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "mission_weights_bucket",
                "MissionWeightsBucket",
                "mission_weights_bucket",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "max_displayable_missions_count",
                "MaxDisplayableMissionsCount",
                "max_displayable_missions_count",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "daily_reward_modifier_key",
                "DailyRewardModifierId",
                "daily_reward_modifier_id",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "daily_reward_modifier_id",
                "DailyRewardModifierId",
                "daily_reward_modifier_id",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "daily_bonuses_count",
                "DailyBonusesCount",
                "daily_bonuses_count",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "mission_refresh_interval_minutes",
                "MissionRefreshIntervalMinutes",
                "mission_refresh_interval_minutes",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "display_npc_shop_button",
                "DisplayNpcShopButton",
                "display_npc_shop_button",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "rank_name_color",
                "RankNameColor",
                "rank_name_color",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "reputation_bar_rank_icon",
                "ReputationBarRankIcon",
                "reputation_bar_rank_icon",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "wallet_display_progression_row",
                "WalletDisplayProgression",
                "wallet_display_progression",
                NativeProjectionTransform::ForeignKeyRow,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "flexible_mission_board_data_from_id",
                "flexible_mission_board_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "flexible_mission_board_data",
                "flexible_mission_board_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "flexible_mission_board_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("boards"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::FlexibleMissionBoardDataManager",
        rust_type: "crate::FlexibleMissionBoardDataManager",
        ghidra_functions: vec![
            "Javelin::FlexibleMissionBoardDataManager::FlexibleMissionBoardDataManager",
            "Javelin::FlexibleMissionBoardDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn affix_data_manager_spec() -> NativeManagerSpec {
    let affix_projection = NativeOneTableCrcKeyProjectionManager::new(
        ident("affix_data"),
        game_table("AffixDataTable"),
        game_row_type("AffixData"),
        ident("AffixData"),
        ident("affixes"),
        ident("affixes_by_id"),
        ident("affix_key"),
        ident("affix_id"),
        column("AffixID"),
        ident("affix_id"),
        true,
        false,
        false,
        NativeDuplicateKeyPolicy::FirstWins,
        vec![
            projection_field(
                "display_name",
                "DisplayName",
                "display_name",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "category",
                "Category",
                "category",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "quality_contribution",
                "QualityContribution",
                "quality_contribution",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "gear_score_contribution",
                "GearScoreContribution",
                "gear_score_contribution",
                NativeProjectionTransform::I32,
            ),
            projection_field(
                "affix_data_weapon",
                "AffixDataWeapon",
                "affix_data_weapon",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "affix_data_armor",
                "AffixDataArmor",
                "affix_data_armor",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "affix_data_misc",
                "AffixDataMisc",
                "affix_data_misc",
                NativeProjectionTransform::OptionalString,
            ),
        ],
        vec![
            lookup(
                "affix_data_from_id",
                "affix_id",
                NativeCrcIndexLookupParameterKind::Crc32,
            ),
            lookup(
                "affix_data",
                "affix_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "affix_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
    )
    .expect("validated AffixData projection")
    .with_rows_method(ident("affixes"))
    .with_len_method(ident("affix_len"));

    let affix_stat_projection = NativeOneTableCrcKeyProjectionManager::new(
        ident("affix_data"),
        game_table("AffixStatDataTable"),
        game_row_type("AffixStatData"),
        ident("AffixStatData"),
        ident("affix_stats"),
        ident("affix_stats_by_id"),
        ident("status_key"),
        ident("status_id"),
        column("StatusID"),
        ident("status_id"),
        true,
        false,
        false,
        NativeDuplicateKeyPolicy::FirstWins,
        vec![
            projection_field(
                "base_damage_modifier",
                "BaseDamageModifier",
                "base_damage_modifier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "gathering_efficiency",
                "GatheringEfficiency",
                "gathering_efficiency",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "durability_mod",
                "DurabilityMod",
                "durability_mod",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "disable_durability_loss",
                "DisableDurabilityLoss",
                "disable_durability_loss",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "status_effect",
                "StatusEffect",
                "status_effect",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "base_damage_type",
                "BaseDamageType",
                "base_damage_type",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "affliction_damage",
                "AfflictionDamage",
                "affliction_damage",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "damage_type",
                "DamageType",
                "damage_type",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "damage_percentage",
                "DamagePercentage",
                "damage_percentage",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "additional_damage",
                "AdditionalDamage",
                "additional_damage",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "is_additive_damage",
                "IsAdditiveDamage",
                "is_additive_damage",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "effect_duration_multiplier",
                "EffectDurationMultiplier",
                "effect_duration_multiplier",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "affliction_multiplier",
                "AfflictionMultiplier",
                "affliction_multiplier",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "use_count_multiplier",
                "UseCountMultiplier",
                "use_count_multiplier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "append_to_tooltip",
                "AppendToTooltip",
                "append_to_tooltip",
                NativeProjectionTransform::StringList,
            ),
            projection_field(
                "use_weapon_attribute_scaling",
                "UseWeaponAttributeScaling",
                "use_weapon_attribute_scaling",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "prefer_higher_scaling",
                "PreferHigherScaling",
                "prefer_higher_scaling",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "scaling_strength",
                "ScalingStrength",
                "scaling_strength",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "scaling_dexterity",
                "ScalingDexterity",
                "scaling_dexterity",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "scaling_intelligence",
                "ScalingIntelligence",
                "scaling_intelligence",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "scaling_focus",
                "ScalingFocus",
                "scaling_focus",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "use_affix_scaling_on_base_damage",
                "UseAffixScalingOnBaseDamage",
                "use_affix_scaling_on_base_damage",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "weight_multiplier",
                "WeightMultiplier",
                "weight_multiplier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "effect_mod_multiplier",
                "EffectModMultiplier",
                "effect_mod_multiplier",
                NativeProjectionTransform::OptionalF32,
            ),
        ],
        vec![
            lookup(
                "affix_stat_data_from_id",
                "affix_stat_id",
                NativeCrcIndexLookupParameterKind::Crc32,
            ),
            lookup(
                "affix_stat_data",
                "affix_stat_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "affix_stat_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
    )
    .expect("validated AffixStatData projection")
    .with_rows_method(ident("affix_stats"))
    .with_len_method(ident("affix_stat_len"));

    manager_spec_with_inputs(
        "Javelin::AffixDataManager",
        "crate::AffixDataManager",
        vec![
            table_input("AffixDataTable", "AffixData"),
            table_input("AffixStatDataTable", "AffixStatData"),
        ],
        vec![
            "Javelin::AffixDataManager::CacheAllAffixDataTables",
            "Javelin::AffixDataManager::CacheAllAffixStatDataTables",
            "Javelin::AffixDataManager::GetAffixData",
            "Javelin::AffixDataManager::GetAffixStatData",
        ],
    )
    .with_shape(NativeManagerShape::multi_table_crc_key_projection(
        NativeMultiTableCrcKeyProjectionManager::new(
            ident("affix_data"),
            vec![affix_projection, affix_stat_projection],
        )
        .expect("validated AffixDataManager multi-table projection"),
    ))
}
