use super::*;

pub(super) fn schedule_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "schedule_data",
        table_name: "Schedules",
        row_type_name: "ScheduleData",
        data_type: "ScheduleStaticData",
        entries_field: "schedules",
        index_field: "schedules_by_id",
        key_field: "schedule_key",
        crc_field: "schedule_id",
        key_column: "ScheduleId",
        key_getter: "schedule_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("schedule_for_source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "start_date",
                "StartDate",
                "start_date",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "end_date",
                "EndDate",
                "end_date",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "start_weekday",
                "StartWeekday",
                "start_weekday",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "end_weekday",
                "EndWeekday",
                "end_weekday",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field("utc", "UTC", "utc", NativeProjectionTransform::Bool),
            projection_field(
                "scheduled_quest_description",
                "ScheduledQuestDescription",
                "scheduled_quest_description",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "main_menu_override",
                "MainMenuOverride",
                "main_menu_override",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "main_menu_coat_override",
                "MainMenuCoatOverride",
                "main_menu_coat_override",
                NativeProjectionTransform::OptionalString,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "schedule",
                "schedule_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "schedule_by_key",
                "schedule_key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: Some("schedule_ids"),
        rows_method: Some("schedules"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::ScheduleDataManager",
        rust_type: "crate::ScheduleDataManager",
        ghidra_functions: vec![
            "Javelin::ScheduleDataManager::ScheduleDataManager",
            "Javelin::ScheduleDataManager::CacheAllDataTables",
            "Javelin::ScheduleDataManager::GetGameSystemData",
            "Javelin::ScheduleDataManager::FindGameSystemDataByKey",
        ],
    })
}
