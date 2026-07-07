use super::*;

pub(super) fn meta_achievement_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "meta_achievement_data",
        table_name: "MetaAchievementDataTable",
        row_type_name: "MetaAchievementData",
        data_type: "MetaAchievementData",
        entries_field: "rows",
        index_field: "rows_by_id",
        key_field: "meta_achievement_key",
        crc_field: "meta_achievement_id",
        key_column: "MetaAchievementId",
        key_getter: "meta_achievement_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("meta_achievement_for_source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "steam_api_name",
                "SteamApiName",
                "steam_api_name",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field("kind", "Type", "type_", NativeProjectionTransform::String),
            projection_field("title", "Title", "title", NativeProjectionTransform::String),
            projection_field(
                "description",
                "Description",
                "description",
                NativeProjectionTransform::String,
            ),
            projection_field("total", "Total", "total", NativeProjectionTransform::U32),
            projection_field(
                "quest_group_tag",
                "QuestGroupTag",
                "quest_group_tag",
                NativeProjectionTransform::OptionalLowercaseCrcString,
            ),
            projection_field(
                "territory_id",
                "TerritoryId",
                "territory_id",
                NativeProjectionTransform::U32,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "meta_achievement",
                "meta_achievement_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "meta_achievement_by_key",
                "meta_achievement_key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: Some("meta_achievement_ids"),
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::MetaAchievementDataManager",
        rust_type: "crate::MetaAchievementDataManager",
        ghidra_functions: vec!["Javelin::MetaAchievementDataManager::MetaAchievementDataManager"],
    })
}
