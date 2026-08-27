use super::*;

/// `StaticBackstoryDataManager` is a CRC-key projection wrapped in a bespoke
/// shape, because three of its columns cannot be expressed as a
/// [`NativeProjectionTransform`]:
///
///   * `InventoryItem` - `ItemDescriptor` values with bracket tags, a native
///     `strtol` gear score, and five fixed perk slots
///   * `WeaponMasteries` - `Name:Amount` pairs, also carried as parallel cells
///   * `CategoricalProgression` - the same pair splitting, folded together with
///     eighteen per-skill columns
///
/// Those three belong to the emitter, which authors its own parse helpers into
/// the emitted module the way `loot_bucket_data` does. Everything the transform
/// vocabulary *can* express stays in the projection below, so the standalone
/// language products keep lowering this manager exactly as they did when it was
/// a plain [`NativeManagerShape::OneTableCrcKeyProjection`] - they project these
/// columns and have never carried the bespoke three.
pub(super) fn static_backstory_data_manager_spec() -> NativeManagerSpec {
    let projected = crc_projection(CrcProjectionSpec {
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
            projection_field(
                "add_to_loadouts",
                "AddToLoadouts",
                "add_to_loadouts",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "objective_unlock_override",
                "ObjectiveUnlockOverride",
                "objective_unlock_override",
                NativeProjectionTransform::StringList,
            ),
            projection_field(
                "achievement_unlock_override",
                "AchievementUnlockOverride",
                "achievement_unlock_override",
                NativeProjectionTransform::StringList,
            ),
            projection_field(
                "force_ftue",
                "ForceFTUE",
                "force_ftue",
                NativeProjectionTransform::Bool,
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
    });

    let Some(NativeManagerShape::OneTableCrcKeyProjection(projection)) = projected.shape().cloned()
    else {
        unreachable!("crc_projection builds a one-table CRC-key projection shape");
    };

    projected.with_shape(NativeManagerShape::static_backstory_data(
        NativeStaticBackstoryDataManager::new(ident("static_backstory_data"), projection),
    ))
}
