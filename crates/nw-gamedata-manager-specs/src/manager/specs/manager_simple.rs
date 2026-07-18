use super::*;

pub(super) fn dye_color_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableDyeColorManager::new(
        ident("dye_color_data"),
        game_table("DyeColorDataTable"),
        game_row_type("DyeColorData"),
        ident("table"),
        ident("DyeColorDataTableRow"),
        ident("DyeColorData"),
        ident("DyeColorIndex"),
        ident("colors"),
        ident("rows_by_index"),
        ident("entitlement_indexes"),
        column("Index"),
        ident("index"),
        ident("name"),
        ident("color"),
        ident("category"),
        ident("is_entitlement"),
        ident("color_amount"),
        ident("color_override"),
        ident("spec_color"),
        ident("spec_amount"),
        ident("mask_gloss_shift"),
        ident("dye_color_data"),
        ident("dye_color_data_from_index"),
        ident("dye_color_data_by_key"),
        ident("rows"),
        ident("entitlement_indexes"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::DyeColorDataManager",
        "crate::DyeColorDataManager",
        "DyeColorDataTable",
        "DyeColorData",
        vec![
            "Javelin::DyeColorDataManager::DyeColorDataManager",
            "Javelin::DyeColorDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::one_table_dye_color(shape))
}

pub(super) fn emote_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableEmoteManager::new(
        ident("emote_data"),
        game_table("EmoteDefinitions"),
        game_row_type("EmoteData"),
        ident("table"),
        ident("EmoteDefinitionsRow"),
        ident("EmoteData"),
        ident("EmoteDataSettings"),
        ident("EmoteIndexes"),
        ident("emotes"),
        ident("emote_data_from_id"),
        ident("emote_data"),
        ident("emote_data_by_key"),
        ident("emote_id_by_status_effect"),
        ident("emote_id_for_status_effect"),
        ident("emotes"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::EmoteDataManager",
        "crate::EmoteDataManager",
        "EmoteDefinitions",
        "EmoteData",
        vec![
            "Javelin::EmoteDataManager::EmoteDataManager",
            "Javelin::EmoteDataManager::CacheAllEmoteDataTables",
            "Javelin::EmoteDataManager::GetEmoteList",
            "Javelin::EmoteDataManager::GetEmoteDataById",
            "Javelin::EmoteDataManager::GetEmoteIdByStatusEffect",
        ],
    )
    .with_shape(NativeManagerShape::one_table_emote(shape))
}

pub(super) fn experience_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableExperienceManager::new(
        ident("experience_data"),
        game_table("XPLevels"),
        game_row_type("ExperienceData"),
        ident("table"),
        ident("XpLevelsRow"),
        ident("ExperienceData"),
        ident("ExperienceDataIndexes"),
        ident("experience"),
        ident("ExperienceNamedValue"),
        ident("ExperienceGearScoreBonus"),
        ident("experience_data_from_id"),
        ident("experience_data"),
        ident("experience_data_for_max_equippable_gear_score"),
        ident("level_for_xp"),
        ident("max_level"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::ExperienceDataManager",
        "crate::ExperienceDataManager",
        "XPLevels",
        "ExperienceData",
        vec![
            "Javelin::ExperienceDataManager::ExperienceDataManager",
            "Javelin::ExperienceDataManager::CacheAllExperienceDataTables",
            "Javelin::ExperienceDataManager::GetExperienceData",
        ],
    )
    .with_shape(NativeManagerShape::one_table_experience(shape))
}

pub(super) fn store_category_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableStoreCategoryManager::new(
        ident("store_category_data"),
        game_table("StoreCategoryPropertiesTable"),
        game_row_type("StoreCategoryProperties"),
        ident("table"),
        ident("StoreCategoryDataRow"),
        ident("StoreCategoryProperties"),
        ident("GameStoreTab"),
        ident("InvalidStoreProductType"),
        ident("StoreCategoryIndexes"),
        ident("categories"),
        ident("num_categories"),
        ident("categories"),
        ident("store_category_properties"),
        ident("store_category_properties_by_name"),
        ident("store_category_by_index"),
        ident("category_for_product_type"),
        ident("invalid_product_types"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::StoreCategoryDataManager",
        "crate::StoreCategoryDataManager",
        "StoreCategoryPropertiesTable",
        "StoreCategoryProperties",
        vec![
            "Javelin::StoreCategoryDataManager::StoreCategoryDataManager",
            "Javelin::StoreCategoryDataManager::CacheAllDataTables",
            "Javelin::StoreCategoryDataManager::GetStoreCategoryByIndex",
            "Javelin::StoreCategoryDataManager::GetStoreCategoryProperties",
            "Javelin::StoreCategoryDataManager::GetCategoryForStoreProductType",
        ],
    )
    .with_shape(NativeManagerShape::one_table_store_category(shape))
}

pub(super) fn store_product_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableStoreProductManager::new(
        ident("store_product_data"),
        game_table("StoreProductData"),
        game_row_type("StoreProductData"),
        ident("table"),
        ident("StoreProductDataRow"),
        ident("StoreProductData"),
        ident("InvalidStoreProductDataProductType"),
        ident("StoreProductIndexes"),
        ident("products"),
        ident("store_product_data"),
        ident("store_product_data_by_tag"),
        ident("products"),
        ident("invalid_product_types"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::StoreProductDataManager",
        "crate::StoreProductDataManager",
        "StoreProductData",
        "StoreProductData",
        vec![
            "Javelin::StoreProductDataManager::CacheAllStoreProductDataTables",
            "Javelin::StoreProductDataManager::GetStoreProductData",
        ],
    )
    .with_shape(NativeManagerShape::one_table_store_product(shape))
}

pub(super) fn reward_track_item_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableRewardTrackItemManager::new(
        ident("reward_track_item_data"),
        game_table("RewardTrackItems"),
        game_row_type("RewardTrackItemData"),
        ident("table"),
        ident("RewardTrackItemsRow"),
        ident("RewardTrackItemData"),
        ident("RewardTrackItemPayload"),
        ident("RewardTrackItemIndexes"),
        ident("items"),
        ident("reward_track_item_data_from_id"),
        ident("reward_track_item_data"),
        ident("reward_track_item_data_by_key"),
        ident("reward_track_items"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::RewardTrackItemDataManager",
        "crate::RewardTrackItemDataManager",
        "RewardTrackItems",
        "RewardTrackItemData",
        vec![
            "Javelin::RewardTrackItemDataManager::RewardTrackItemDataManager",
            "Javelin::RewardTrackItemDataManager::CacheAllDataTables",
            "Javelin::RewardTrackItemDataManager::FindGameSystemDataByKey",
        ],
    )
    .with_shape(NativeManagerShape::one_table_reward_track_item(shape))
}

pub(super) fn post_skill_cap_progression_data_manager_spec() -> NativeManagerSpec {
    let shape = NativePostSkillCapProgressionDataManager::new(
        ident("post_skill_cap_progression_data"),
        game_table("TradeSkillPostCap"),
        game_row_type("TradeSkillPostCapData"),
        ident("table"),
        ident("PostSkillCapProgressionDataRow"),
        ident("StaticPostSkillCapProgressionData"),
        ident("PostSkillCapLevelRewards"),
        ident("PostSkillCapProgressionIndexes"),
        ident("progression"),
        ident("post_skill_cap_progression_data"),
        ident("post_skill_cap_progression_data_from_id"),
        ident("entries"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::PostSkillCapProgressionDataManager",
        "crate::PostSkillCapProgressionDataManager",
        "TradeSkillPostCap",
        "TradeSkillPostCapData",
        vec![
            "Javelin::PostSkillCapProgressionDataManager::PostSkillCapProgressionDataManager",
            "Javelin::PostSkillCapProgressionDataManager::CacheAllDataTables",
            "Javelin::PostSkillCapProgressionDataManager::FindGameSystemDataByKey",
        ],
    )
    .with_shape(NativeManagerShape::post_skill_cap_progression(shape))
}

pub(super) fn quick_course_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeQuickCourseDataManager::new(
        ident("quick_course_data"),
        game_table("QuickCourse_Master"),
        game_row_type("QuickCourseData"),
        ident("quick_courses"),
        ident("QuickCourseDataRow"),
        game_table("QuickCourse_NodeTypes"),
        game_row_type("QuickCourseNodeTypeData"),
        ident("node_types"),
        ident("QuickCourseNodeTypeDataRow"),
        ident("QuickCourseData"),
        ident("QuickCourseNodeTypeData"),
        ident("QuickCourseIndexes"),
        ident("indexes"),
        ident("quick_course"),
        ident("quick_course_by_crc32"),
        ident("quick_courses"),
        ident("quick_course_ids"),
        ident("first_quick_course_id"),
        ident("node_type"),
        ident("node_type_by_crc32"),
        ident("node_types"),
        ident("node_type_ids"),
        ident("first_node_type_id"),
        ident("quick_course_len"),
        ident("node_type_len"),
        ident("is_empty"),
    );

    manager_spec_with_inputs(
        "Javelin::QuickCourseDataManager",
        "crate::QuickCourseDataManager",
        vec![
            table_input("QuickCourse_Master", "QuickCourseData"),
            table_input("QuickCourse_NodeTypes", "QuickCourseNodeTypeData"),
        ],
        vec![
            "Javelin::QuickCourseDataManager::QuickCourseDataManager",
            "Javelin::QuickCourseDataManager::CacheAllQuickCourseDataTables",
            "Javelin::QuickCourseDataManager::GetQuickCourseData",
        ],
    )
    .with_shape(NativeManagerShape::quick_course_data(shape))
}

pub(super) fn rotational_queue_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeRotationalQueueDataManager::new(
        ident("rotational_queue_data"),
        game_table("RotationalQueue"),
        game_row_type("RotationalQueueData"),
        ident("queue_table"),
        ident("RotationalQueueDataRow"),
        game_table("PUGActivityInfo"),
        game_row_type("PUGActivityInfo"),
        ident("game_modes"),
        ident("RotationalQueueStaticData"),
        ident("RotationalQueueIndexes"),
        ident("queues"),
        ident("rotational_queue"),
        ident("rotational_queue_from_id"),
        ident("rotational_queues"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec_with_inputs(
        "Javelin::RotationalQueueDataManager",
        "crate::RotationalQueueDataManager",
        vec![
            table_input("RotationalQueue", "RotationalQueueData"),
            table_input("PUGActivityInfo", "PUGActivityInfo"),
        ],
        vec![
            "Javelin::RotationalQueueDataManager::RotationalQueueDataManager",
            "Javelin::RotationalQueueDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::rotational_queue_data(shape))
}

pub(super) fn whisper_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeWhisperDataManager::new(
        ident("whisper_data"),
        game_table("WhisperDataManager"),
        game_row_type("WhisperData"),
        ident("whisper_table"),
        ident("WhisperDataRow"),
        game_table("WhisperVFXData"),
        game_row_type("WhisperVfxData"),
        ident("vfx_table"),
        ident("WhisperVfxDataRow"),
        ident("WhisperData"),
        ident("WhisperVfxData"),
        ident("WhisperIndexes"),
        ident("indexes"),
        ident("whisper_data_from_id"),
        ident("whisper_data"),
        ident("whisper_data_by_key"),
        ident("whispers"),
        ident("whisper_ids"),
        ident("whisper_vfx_data_from_id"),
        ident("whisper_vfx_data"),
        ident("whisper_vfx_for"),
        ident("whisper_vfx_rows"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec_with_inputs(
        "Javelin::WhisperDataManager",
        "crate::WhisperDataManager",
        vec![
            table_input("WhisperDataManager", "WhisperData"),
            table_input("WhisperVFXData", "WhisperVfxData"),
        ],
        vec![
            "Javelin::WhisperDataManager::WhisperDataManager",
            "Javelin::WhisperDataManager::CacheWhisperDataTable",
        ],
    )
    .with_shape(NativeManagerShape::whisper_data(shape))
}

pub(super) fn world_event_rule_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableWorldEventRuleManager::new(
        ident("world_event_rule_data"),
        game_table("WorldEventRules"),
        game_row_type("WorldEventRuleData"),
        ident("table"),
        ident("WorldEventRuleDataRow"),
        ident("WorldEventRuleData"),
        ident("WorldEventRuleCrcFilter"),
        ident("WorldEventRuleZoneFilter"),
        ident("WorldEventRuleIndexes"),
        ident("rules"),
        ident("world_event_rule"),
        ident("world_event_rule_by_crc32"),
        ident("world_event_rules"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::WorldEventRuleDataManager",
        "crate::WorldEventRuleDataManager",
        "WorldEventRules",
        "WorldEventRuleData",
        vec![
            "Javelin::WorldEventRuleDataManager::WorldEventRuleDataManager",
            "Javelin::WorldEventRuleDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::one_table_world_event_rule(shape))
}

pub(super) fn camp_skin_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableCampSkinManager::new(
        ident("camp_skin_data"),
        game_table("CampSkinDataTable"),
        game_row_type("CampSkinData"),
        ident("table"),
        ident("CampSkinDataRow"),
        ident("CampSkinData"),
        ident("CampSkinDataSettings"),
        ident("camp_skin_rows"),
        ident("camp_skins_by_id"),
        column("CampSkinID"),
        ident("camp_skin_id"),
        ident("item_id"),
        ident("required_achievement_id"),
        ident("is_entitlement"),
        ident("is_enabled"),
        ident("camp_skin_data"),
        ident("camp_skin_data_by_key"),
        ident("camp_skin_ids"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::CampSkinDataManager",
        "crate::CampSkinDataManager",
        "CampSkinDataTable",
        "CampSkinData",
        vec![
            "Javelin::CampSkinDataManager::CampSkinDataManager",
            "Javelin::CampSkinDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::one_table_camp_skin(shape))
}

pub(super) fn camera_shake_data_manager_spec() -> NativeManagerSpec {
    table_family_partitioned_crc_projection(TableFamilyPartitionedCrcProjectionSpec {
        module: "camera_shake_data",
        tables: vec![
            TableFamilyTableSpec {
                variant: "Table".to_owned(),
                table_name: "CameraShakeDataTable",
                row_type_name: "CameraShakeData",
            },
            TableFamilyTableSpec {
                variant: "TableIsleOfNight".to_owned(),
                table_name: "CameraShakeDataTable_IsleOfNight",
                row_type_name: "CameraShakeData",
            },
        ],
        tables_type: "CameraShakeDataTables",
        table_type: "CameraShakeDataTable",
        data_type: "CameraShakeData",
        entries_field: "camera_shakes",
        key_field: "camera_shake_id",
        crc_field: "camera_shake_id_crc",
        key_column: "CameraShakeID",
        key_getter: "camera_shake_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        global_index: None,
        table_indexes: vec![
            TableFamilyCrcTableIndexSpec {
                index_field: "base_camera_shakes_by_crc",
                table_variant: "Table",
                duplicate_key_policy: NativeDuplicateKeyPolicy::Overwrite,
                lookup_methods: vec![
                    lookup(
                        "camera_shake_data_from_id",
                        "row_reference",
                        NativeCrcIndexLookupParameterKind::Crc32,
                    ),
                    lookup(
                        "camera_shake_data",
                        "camera_shake_id",
                        NativeCrcIndexLookupParameterKind::StrRef,
                    ),
                    lookup(
                        "camera_shake_data_by_key",
                        "key",
                        NativeCrcIndexLookupParameterKind::StrRef,
                    ),
                ],
            },
            private_table_index(
                "isle_of_night_camera_shakes_by_crc",
                "TableIsleOfNight",
                NativeDuplicateKeyPolicy::Overwrite,
            ),
        ],
        fields: vec![
            projection_field(
                "sustain_duration",
                "SustainDuration",
                "sustain_duration",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "fade_in_duration",
                "FadeInDuration",
                "fade_in_duration",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "fade_out_duration",
                "FadeOutDuration",
                "fade_out_duration",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "frequency",
                "Frequency",
                "frequency",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "randomness",
                "Randomness",
                "randomness",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "shake_channel",
                "ShakeChannel",
                "shake_channel",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "flip_vec",
                "FlipVec",
                "flip_vec",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "update_only",
                "UpdateOnly",
                "update_only",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "permanent",
                "Permanent",
                "permanent",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "is_smooth",
                "IsSmooth",
                "is_smooth",
                NativeProjectionTransform::Bool,
            ),
        ],
        vec3_fields: vec![
            vec3_projection_field(
                "shake_shift",
                "ShakeShiftX",
                "shake_shift_x",
                "ShakeShiftY",
                "shake_shift_y",
                "ShakeShiftZ",
                "shake_shift_z",
            ),
            vec3_projection_field(
                "shake_angle",
                "ShakeAngleX",
                "shake_angle_x",
                "ShakeAngleY",
                "shake_angle_y",
                "ShakeAngleZ",
                "shake_angle_z",
            ),
        ],
        store_key_text: true,
        rows_method: None,
        len_method: None,
        is_empty_method: None,
        ghidra_class: "Javelin::CameraShakeDataManager",
        rust_type: "crate::CameraShakeDataManager",
        ghidra_functions: vec!["Javelin::CameraShakeDataManager::GetCameraShakeData"],
    })
}

pub(super) fn shop_data_manager_spec() -> NativeManagerSpec {
    crc_projection_with_dependency_lookup_methods(
        CrcProjectionSpec {
            module: "shop_data",
            table_name: "ShopData",
            row_type_name: "ShopData",
            data_type: "ShopData",
            entries_field: "shops",
            index_field: "shops_by_crc",
            key_field: "shop_id",
            crc_field: "shop_id_crc",
            key_column: "ShopId",
            key_getter: "shop_id",
            skip_empty_key: true,
            trim_key: false,
            reject_zero_crc: false,
            duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
            source_row_field: Some("source_row"),
            source_row_method: Some("source_row"),
            row_filters: Vec::new(),
            fields: vec![
                projection_field(
                    "progression_id",
                    "ProgressionId",
                    "progression_id",
                    NativeProjectionTransform::String,
                ),
                projection_field(
                    "shop_name",
                    "ShopName",
                    "shop_name",
                    NativeProjectionTransform::String,
                ),
                projection_field(
                    "display_on_marker",
                    "DisplayOnMarker",
                    "display_on_marker",
                    NativeProjectionTransform::Bool,
                ),
                projection_field(
                    "display_on_compass",
                    "DisplayOnCompass",
                    "display_on_compass",
                    NativeProjectionTransform::Bool,
                ),
                projection_field(
                    "display_on_map",
                    "DisplayOnMap",
                    "display_on_map",
                    NativeProjectionTransform::Bool,
                ),
                projection_field(
                    "display_progress_panel",
                    "DisplayProgressPanel",
                    "display_progress_panel",
                    NativeProjectionTransform::Bool,
                ),
                projection_field(
                    "wallet_display_gold",
                    "WalletDisplayGold",
                    "wallet_display_gold",
                    NativeProjectionTransform::Bool,
                ),
                projection_field(
                    "wallet_display_azoth",
                    "WalletDisplayAzoth",
                    "wallet_display_azoth",
                    NativeProjectionTransform::Bool,
                ),
                projection_field(
                    "wallet_display_player_level",
                    "WalletDisplayPlayerLevel",
                    "wallet_display_player_level",
                    NativeProjectionTransform::Bool,
                ),
            ],
            secondary_indexes: Vec::new(),
            lookup_methods: vec![
                lookup(
                    "shop_data",
                    "shop_id",
                    NativeCrcIndexLookupParameterKind::Crc32,
                ),
                lookup(
                    "shop_data_by_key",
                    "shop_id",
                    NativeCrcIndexLookupParameterKind::AsRefStr,
                ),
            ],
            ids_method: None,
            rows_method: Some("shops"),
            len_method: Some("len"),
            is_empty_method: Some("is_empty"),
            ghidra_class: "Javelin::ShopDataManager",
            rust_type: "crate::ShopDataManager",
            ghidra_functions: vec!["Javelin::ShopDataManager::GetShopDataFromNpcId"],
        },
        vec![dependency_crc_lookup(
            "shop_data_from_npc_id",
            "crate::NPCDataManager",
            "npc_data",
            "npc_id",
            NativeCrcIndexLookupParameterKind::Crc32,
            "shop_id",
            "shop_data",
        )],
    )
}
