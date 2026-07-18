use super::*;

pub(super) fn particle_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableParticleDataManager::new(
        ident("particle_data"),
        game_table("ParticleDataTable"),
        game_row_type("ParticleData"),
        ident("ParticleData"),
        ident("ParticleGroupData"),
        ident("ParticleDataLookup"),
        ident("ParticleIndexes"),
        ident("entries"),
        ident("local_player_factor"),
        ident("max_total_number_emitters"),
        ident("max_total_group_number_emitters"),
        column("Effect Name"),
        ident("effect_name"),
        column("Group"),
        ident("group"),
        column("Max Number"),
        ident("max_number"),
        column("Priority"),
        ident("priority"),
        column("Constants"),
        ident("constants"),
        ident("particle_data_from_id"),
        ident("particle_data"),
        ident("particle_data_by_key"),
        ident("local_player_factor"),
        ident("max_total_number_emitters"),
        ident("max_total_group_number_emitters"),
        ident("len"),
        ident("is_empty"),
    );
    manager_spec(
        "Javelin::ParticleDataManager",
        "crate::ParticleDataManager",
        "ParticleDataTable",
        "ParticleData",
        vec![
            "Javelin::ParticleDataManager::GetParticleData",
            "Javelin::ParticleDataManager::CacheConstants",
        ],
    )
    .with_shape(NativeManagerShape::one_table_particle_data(shape))
}

pub(super) fn categorical_progression_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "categorical_progression_data",
        table_name: "CategoricalProgression",
        row_type_name: "CategoricalProgressionData",
        data_type: "CategoricalProgressionData",
        entries_field: "progressions",
        index_field: "progressions_by_crc",
        key_field: "categorical_progression_id",
        crc_field: "categorical_progression_id_crc",
        key_column: "CategoricalProgressionId",
        key_getter: "categorical_progression_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "display_name",
                "DisplayName",
                "display_name",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "max_level",
                "MaxLevel",
                "max_level",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "expansion01_max_level",
                "Expansion01MaxLevel",
                "expansion01_max_level",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "expansion02_max_level",
                "Expansion02MaxLevel",
                "expansion02_max_level",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "progression_type",
                "ProgressionType",
                "progression_type",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "progression_currency_id",
                "ProgressionCurrencyId",
                "progression_currency_id",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "auto_rank_up",
                "AutoRankUp",
                "auto_rank_up",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "infinite_levels",
                "InfiniteLevels",
                "infinite_levels",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "repeat_max_level",
                "RepeatMaxLevel",
                "repeat_max_level",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "allow_overflow",
                "AllowOverflow",
                "allow_overflow",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "rank_table_id",
                "RankTableId",
                "rank_table_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "progression_point_pool",
                "ProgressionPointPool",
                "progression_point_pool",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "pre_skill_cap_skill",
                "PreSkillCapSkill",
                "pre_skill_cap_skill",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "post_skill_cap_skill",
                "PostSkillCapSkill",
                "post_skill_cap_skill",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "economy_tracker_type",
                "EconomyTrackerType",
                "economy_tracker_type",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "game_event_id",
                "GameEventId",
                "game_event_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "achievement_id_on_max_rank",
                "AchievementIdOnMaxRank",
                "achievement_id_on_max_rank",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "min_tracked_level",
                "MinTrackedLevel",
                "min_tracked_level",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "show_as_objective_reward",
                "ShowAsObjectiveReward",
                "show_as_objective_reward",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "icon_path",
                "IconPath",
                "icon_path",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "uses_global_progression_mod",
                "UsesGlobalProgressionMod",
                "uses_global_progression_mod",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "item_class",
                "ItemClass",
                "item_class",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "display_description",
                "DisplayDescription",
                "display_description",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "event_id",
                "EventId",
                "event_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "loot_limit_id",
                "LootLimitId",
                "loot_limit_id",
                NativeProjectionTransform::OptionalString,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "categorical_progression_data_from_id",
                "categorical_progression_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "categorical_progression_data",
                "categorical_progression_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "categorical_progression_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("progressions"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::CategoricalProgressionDataManager",
        rust_type: "crate::CategoricalProgressionDataManager",
        ghidra_functions: vec![
            "Javelin::CategoricalProgressionDataManager::CategoricalProgressionDataManager",
        ],
    })
}

pub(super) fn particle_priority_override_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableCrcKeyProjectionManager::new(
        ident("particle_priority_override_data"),
        game_table("ParticleContextualPriorityOverrides"),
        game_row_type("ParticleContextualPriorityOverrideData"),
        ident("ParticlePriorityOverrideData"),
        ident("rows"),
        ident("rows_by_effect_id"),
        ident("effect_name"),
        ident("effect_id"),
        column("EffectName"),
        ident("effect_name"),
        true,
        false,
        false,
        NativeDuplicateKeyPolicy::FirstWins,
        vec![typed_projection_field(
            "priority_override",
            "PriorityOverride",
            "priority_override",
            NativeProjectionTransform::U8Enum,
            "ParticlePriorityOverride",
        )],
        vec![
            lookup(
                "particle_priority_override_data",
                "effect_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "particle_priority_override_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
    )
    .expect("validated particle-priority manager shape")
    .with_hash_policy(NativeCrcHashPolicy::Lowercase)
    .with_key_storage_transform(NativeCrcKeyStorageTransform::RemoveSpaceAndTab)
    .with_source_row_field(ident("source_row"))
    .with_source_row_method(ident("particle_priority_override_data_for_source_row"))
    .with_rows_method(ident("rows"))
    .with_len_method(ident("len"))
    .with_is_empty_method(ident("is_empty"));

    manager_spec(
        "Javelin::ParticlePriorityOverrideDataManager",
        "crate::ParticlePriorityOverrideDataManager",
        "ParticleContextualPriorityOverrides",
        "ParticleContextualPriorityOverrideData",
        vec![
            "Javelin::ParticlePriorityOverrideDataManager::ParticlePriorityOverrideDataManager",
            "Javelin::ParticlePriorityOverrideDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::one_table_crc_key_projection(shape))
}

pub(super) fn player_tutorials_condition_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "player_tutorials_condition_data",
        table_name: "TutorialConditionData",
        row_type_name: "TutorialConditionData",
        data_type: "PlayerTutorialsConditionData",
        entries_field: "conditions",
        index_field: "conditions_by_crc",
        key_field: "condition_id",
        crc_field: "condition_id_crc",
        key_column: "ConditionId",
        key_getter: "condition_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("source_row"),
        row_filters: Vec::new(),
        fields: vec![
            optional_u8_enum_default_projection_field(
                "operation",
                "Operation",
                "operation",
                "TutorialConditionOperation",
                "TutorialConditionOperation::Equals",
            ),
            projection_field(
                "player_level",
                "PlayerLevel",
                "player_level",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "categorical_progression",
                "CategoricalProgression",
                "categorical_progression",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "game_event",
                "GameEvent",
                "game_event",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "achievement",
                "Achievement",
                "achievement",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "entitlement",
                "Entitlement",
                "entitlement",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "item",
                "Item",
                "item",
                NativeProjectionTransform::StringList,
            ),
            projection_field(
                "ui_event",
                "UIEvent",
                "ui_event",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "status_effects",
                "StatusEffects",
                "status_effects",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "notes",
                "Notes",
                "notes",
                NativeProjectionTransform::OptionalString,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "condition_data",
                "condition_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "condition_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::PlayerTutorialsConditionDataManager",
        rust_type: "crate::PlayerTutorialsConditionDataManager",
        ghidra_functions: vec![
            "Javelin::PlayerTutorialsConditionDataManager::PlayerTutorialsConditionDataManager",
            "Javelin::PlayerTutorialsConditionDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn player_tutorials_content_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "player_tutorials_content_data",
        table_name: "TutorialContentData",
        row_type_name: "TutorialContentData",
        data_type: "PlayerTutorialsContentData",
        entries_field: "content",
        index_field: "content_by_crc",
        key_field: "content_id",
        crc_field: "content_id_crc",
        key_column: "ContentId",
        key_getter: "content_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "subtitle_text",
                "SubtitleText",
                "subtitle_text",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "body_text",
                "BodyText",
                "body_text",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "keyboard_button_display_override",
                "KeyboardButtonDisplayOverride",
                "keyboard_button_display_override",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "image_path",
                "ImagePath",
                "image_path",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "icon_path",
                "IconPath",
                "icon_path",
                NativeProjectionTransform::OptionalString,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "content_data",
                "content_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "content_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::PlayerTutorialsContentDataManager",
        rust_type: "crate::PlayerTutorialsContentDataManager",
        ghidra_functions: vec![
            "Javelin::PlayerTutorialsContentDataManager::PlayerTutorialsContentDataManager",
            "Javelin::PlayerTutorialsContentDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn player_tutorials_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "player_tutorials_data",
        table_name: "TutorialData",
        row_type_name: "TutorialData",
        data_type: "PlayerTutorialsData",
        entries_field: "tutorials",
        index_field: "tutorials_by_crc",
        key_field: "tutorial_id",
        crc_field: "tutorial_id_crc",
        key_column: "TutorialId",
        key_getter: "tutorial_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("source_row"),
        row_filters: Vec::new(),
        fields: vec![
            typed_projection_field(
                "type_",
                "Type",
                "type_",
                NativeProjectionTransform::U8Enum,
                "TutorialType",
            ),
            projection_field(
                "prompt_content_ids",
                "PromptContentIds",
                "prompt_content_ids",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "dialogue_content_ids",
                "DialogueContentIds",
                "dialogue_content_ids",
                NativeProjectionTransform::OptionalForeignKeyRowListDefaultEmpty,
            ),
            projection_field(
                "condition_ids_and",
                "ConditionIdsAND",
                "condition_ids_and",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            optional_u8_enum_default_projection_field(
                "condition_ids_relation",
                "ConditionIdsRelation",
                "condition_ids_relation",
                "TutorialConditionIdsRelation",
                "TutorialConditionIdsRelation::OR",
            ),
            projection_field(
                "condition_ids_or",
                "ConditionIdsOR",
                "condition_ids_or",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            typed_projection_field(
                "classification",
                "Classification",
                "classification",
                NativeProjectionTransform::U8Enum,
                "TutorialClassification",
            ),
            optional_u8_enum_default_projection_field(
                "prompt_style",
                "PromptStyle",
                "prompt_style",
                "TutorialPromptStyle",
                "TutorialPromptStyle::None",
            ),
            projection_field(
                "title_text",
                "TitleText",
                "title_text",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "category",
                "Category",
                "category",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "cta_enabled",
                "CTAEnabled",
                "cta_enabled",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "exit_action_and_description",
                "ExitActionAndDescription",
                "exit_action_and_description",
                NativeProjectionTransform::OptionalString,
            ),
            optional_u8_enum_default_projection_field(
                "exit_duration",
                "ExitDuration",
                "exit_duration",
                "TutorialPromptExitDuration",
                "TutorialPromptExitDuration::None",
            ),
            projection_field(
                "hidden_trigger_condition_id",
                "HiddenTriggerConditionId",
                "hidden_trigger_condition_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "ignore_combat_suppression",
                "IgnoreCombatSuppression",
                "ignore_combat_suppression",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "reset_on_ftue_start",
                "ResetOnFTUEStart",
                "reset_on_ftue_start",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "search_keywords",
                "SearchKeywords",
                "search_keywords",
                NativeProjectionTransform::StringList,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "tutorial_data",
                "tutorial_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "tutorial_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::PlayerTutorialsDataManager",
        rust_type: "crate::PlayerTutorialsDataManager",
        ghidra_functions: vec![
            "Javelin::PlayerTutorialsDataManager::PlayerTutorialsDataManager",
            "Javelin::PlayerTutorialsDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn title_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "title_data",
        table_name: "PlayerTitleDataTable",
        row_type_name: "PlayerTitleData",
        data_type: "TitleData",
        entries_field: "entries",
        index_field: "rows_by_title_id",
        key_field: "title_id",
        crc_field: "title_id_crc",
        key_column: "TitleID",
        key_getter: "title_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("source_row"),
        row_filters: Vec::new(),
        fields: vec![
            typed_projection_field(
                "title_type",
                "TitleType",
                "title_type",
                NativeProjectionTransform::U8Enum,
                "gamedata::semantic::TitleType",
            ),
            projection_field(
                "ui_display_category",
                "UIDisplayCategory",
                "ui_display_category",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "title_male",
                "TitleMale",
                "title_male",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "title_female",
                "TitleFemale",
                "title_female",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "title_neutral",
                "TitleNeutral",
                "title_neutral",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "description",
                "Description",
                "description",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "meta_achievement_rows",
                "MetaAchievementId",
                "meta_achievement_id",
                NativeProjectionTransform::OptionalForeignKeyRowListDefaultEmpty,
            ),
            projection_field(
                "achievement_rows",
                "AchievementId",
                "achievement_id",
                NativeProjectionTransform::OptionalForeignKeyRowListDefaultEmpty,
            ),
            projection_field(
                "categorical_progression_row",
                "CategoricalProgressionId",
                "categorical_progression_id",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "required_categorical_progression_level",
                "RequiredCategoricalProgressionLevel",
                "required_categorical_progression_level",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "required_player_level",
                "RequiredPlayerLevel",
                "required_player_level",
                NativeProjectionTransform::U32,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "title_data_by_crc32",
                "title_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "title_data",
                "title_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("entries"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::TitleDataManager",
        rust_type: "crate::TitleDataManager",
        ghidra_functions: vec![
            "Javelin::TitleDataManager::TitleDataManager",
            "Javelin::TitleDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn story_progress_data_manager_spec() -> NativeManagerSpec {
    row_projection(RowProjectionSpec {
        module: "story_progress_data",
        table_name: "StoryProgress",
        row_type_name: "StoryProgressData",
        data_type: "StoryProgressData",
        entries_field: "rows",
        source_row_field: Some("source_row"),
        source_row_method: Some("story_progress_for_source_row"),
        source_row_for_method: Some("source_row_for"),
        fields: vec![
            projection_field(
                "achievement_ids",
                "AchievementIds",
                "achievement_ids",
                NativeProjectionTransform::CrcList,
            ),
            projection_field(
                "activity_task_name",
                "ActivityTaskName",
                "activity_task_name",
                NativeProjectionTransform::Crc32,
            ),
            crc_presence_projection_field(
                "has_activity_task_name",
                "ActivityTaskName",
                "activity_task_name",
                "activity_task_name",
            ),
        ],
        rows_method: Some("story_progress"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::StoryProgressDataManager",
        rust_type: "crate::StoryProgressDataManager",
        ghidra_functions: vec![
            "Javelin::StoryProgressDataManager::FUN_7ff604116ff0",
            "Javelin::StoryProgressDataManager::FUN_7ff604118970",
            "Javelin::StoryProgressDataManager::FUN_7ff604147300",
        ],
    })
}

pub(super) fn reward_milestone_data_manager_spec() -> NativeManagerSpec {
    const REWARD_MILESTONE_TYPE: &str = "RewardMilestoneType";
    const REWARD_MILESTONE_EXPANSION_ID: &str = "gamedata::semantic::ExpansionId";

    crc_projection(CrcProjectionSpec {
        module: "reward_milestone_data",
        table_name: "RewardMilestones",
        row_type_name: "RewardMilestoneData",
        data_type: "RewardMilestoneData",
        entries_field: "rows",
        index_field: "rows_by_reward_id",
        key_field: "reward_id_key",
        crc_field: "reward_id",
        key_column: "RewardID",
        key_getter: "reward_id",
        skip_empty_key: false,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("reward_milestone_data_for_source_row"),
        row_filters: Vec::new(),
        fields: vec![
            optional_u8_enum_default_projection_field(
                "milestone_type",
                "MilestoneType",
                "milestone_type",
                REWARD_MILESTONE_TYPE,
                "RewardMilestoneType::None",
            ),
            projection_field(
                "milestone_level",
                "MilestoneLevel",
                "milestone_level",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field("name", "Name", "name", NativeProjectionTransform::String),
            projection_field(
                "icon",
                "Icon",
                "icon",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "image",
                "Image",
                "image",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "tooltip",
                "Tooltip",
                "tooltip",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "quest_name",
                "QuestName",
                "quest_name",
                NativeProjectionTransform::OptionalString,
            ),
            optional_u8_enum_default_projection_field(
                "expansion_id_unlock",
                "ExpansionIdUnlock",
                "expansion_id_unlock",
                REWARD_MILESTONE_EXPANSION_ID,
                "gamedata::semantic::ExpansionId::None",
            ),
            projection_field(
                "notes",
                "Notes",
                "notes",
                NativeProjectionTransform::OptionalString,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "reward_milestone_data",
                "reward_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "reward_milestone_data_by_key",
                "reward_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::RewardMilestoneDataManager",
        rust_type: "crate::RewardMilestoneDataManager",
        ghidra_functions: vec!["Javelin::RewardMilestoneDataManager::RewardMilestoneDataManager"],
    })
}
