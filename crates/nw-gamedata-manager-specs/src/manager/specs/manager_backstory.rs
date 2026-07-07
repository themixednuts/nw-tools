use super::*;

pub(super) fn static_backstory_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "static_backstory_data",
        table_name: "Backstory",
        row_type_name: "BackstoryDefinition",
        data_type: "StaticBackstoryData",
        entries_field: "rows",
        index_field: "rows_by_id",
        key_field: "backstory_key",
        crc_field: "backstory_id",
        key_column: "BackstoryID",
        key_getter: "backstory_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("backstory_for_source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "backstory_name",
                "BackstoryName",
                "backstory_name",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "level_override",
                "LevelOverride",
                "level_override",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "constitution",
                "Constitution",
                "constitution",
                NativeProjectionTransform::U32,
            ),
            projection_field("focus", "Focus", "focus", NativeProjectionTransform::U32),
            projection_field(
                "strength",
                "Strength",
                "strength",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "dexterity",
                "Dexterity",
                "dexterity",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "intelligence",
                "Intelligence",
                "intelligence",
                NativeProjectionTransform::U32,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "backstory",
                "backstory_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "backstory_by_key",
                "backstory_key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: Some("backstory_ids"),
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::StaticBackstoryDataManager",
        rust_type: "crate::StaticBackstoryDataManager",
        ghidra_functions: vec![
            "Javelin::StaticBackstoryDataManager::StaticBackstoryDataManager",
            "Javelin::StaticBackstoryDataManager::CacheAllDataTables",
        ],
    })
}
