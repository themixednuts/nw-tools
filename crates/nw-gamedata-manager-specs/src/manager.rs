//! GameData manager codegen IR.
//!
//! Every validated [`NativeManagerSpec`] is emitted into the generated manager
//! manifest. Codegen derives each manager surface from its inputs and
//! [`NativeManagerShape`]: table-only requirement managers are direct schema
//! table resources, while shaped managers expose semantic manager APIs only for
//! behavior represented by their validated native shape.
//!
//! - [`NativeManagerInput::Table`] lowers to a cooked-table product requirement
//!   in the runtime manifest.
//! - [`NativeManagerInput::Product`] is a typed product loaded through the
//!   engine asset catalog.
//! - [`NativeManagerInput::Manager`] is a manager dependency: a readiness edge
//!   between generated Bevy resources.
//! - [`NativeTableFamilyTable`] plus its generated family module forms a table
//!   family; family row references lower to table-tagged handles, not a single
//!   bare `RowIndex` space.
//! - [`NativeProjectionTransform`], row filters, duplicate-key policy, and key
//!   transforms decide whether a reference exposes a table handle, row handle,
//!   CRC, string, numeric key, or manager-owned key. Consumers must not assume
//!   every reference is a bare row index or every key is a `Crc32`.

use thiserror::Error;

mod specs;

use crate::naming::to_module_ident;
use crate::native::{NativeClassSpec, NativeClassSpecError, validate_native_class_spec_inputs};
use crate::symbols::{
    GameAssetPath, GameDataColumnName, GameDataRowTypeName, GameDataTableName, GhidraClassPath,
    GhidraFunctionPath, RustIdentifier, RustPath, RustTypePath,
};

pub use specs::validated_native_manager_specs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeManagerSpec {
    native: NativeClassSpec<NativeManagerInput>,
    shape: Option<NativeManagerShape>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeManagerInput {
    Table(NativeManagerTableInput),
    Product(NativeManagerProductInput),
    Manager(RustTypePath),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeManagerTableInput {
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeManagerProductInput {
    format: NativeManagerProductFormat,
    asset_path: GameAssetPath,
    rust_type: RustTypePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeManagerProductFormat {
    ObjectStream,
    Xml,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagerContract<'a> {
    manager: &'a RustTypePath,
    inputs: Vec<ManagerContractInput<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagerContractInput<'a> {
    Table {
        name: &'a GameDataTableName,
        row: &'a GameDataRowTypeName,
        product_path: String,
    },
    Asset {
        path: &'a GameAssetPath,
        asset_type: &'a RustTypePath,
    },
    Manager {
        manager: &'a RustTypePath,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeManagerShape {
    RequirementsOnly,
    AbilityData(NativeAbilityDataManager),
    ObjectivesData(NativeObjectivesDataManager),
    ContributionData(NativeContributionDataManager),
    BuffBucketData(NativeBuffBucketDataManager),
    StructureData(NativeStructureDataManager),
    ReusableScoreboardData(NativeReusableScoreboardDataManager),
    MountHitVolumeData(NativeMountHitVolumeDataManager),
    OneTableCrcIndex(NativeOneTableCrcIndexManager),
    TableFamilyCrcIndex(NativeTableFamilyCrcIndexManager),
    OneTableOwnedStringCrcIndex(NativeOneTableOwnedStringCrcIndexManager),
    TableFamilyOwnedStringCrcIndex(NativeTableFamilyOwnedStringCrcIndexManager),
    OneTableCrcKeyProjection(NativeOneTableCrcKeyProjectionManager),
    MultiTableCrcKeyProjection(NativeMultiTableCrcKeyProjectionManager),
    TableFamilyCrcKeyProjection(NativeTableFamilyCrcKeyProjectionManager),
    TableFamilyFallbackCrcKeyProjection(NativeTableFamilyFallbackCrcKeyProjectionManager),
    TableFamilyPartitionedCrcKeyProjection(NativeTableFamilyPartitionedCrcKeyProjectionManager),
    OneTableNumericKeyProjection(NativeOneTableNumericKeyProjectionManager),
    TableFamilyNumericKeyProjection(NativeTableFamilyNumericKeyProjectionManager),
    OneTableEnumKeyProjection(NativeOneTableEnumKeyProjectionManager),
    OneTableStringKeyProjection(NativeOneTableStringKeyProjectionManager),
    OneTableRowProjection(NativeOneTableRowProjectionManager),
    OneTablePvpBalance(NativeOneTablePvpBalanceManager),
    OneTableCampSkin(NativeOneTableCampSkinManager),
    OneTableDyeColor(NativeOneTableDyeColorManager),
    OneTableEmote(NativeOneTableEmoteManager),
    OneTableExperience(NativeOneTableExperienceManager),
    OneTableStoreCategory(NativeOneTableStoreCategoryManager),
    OneTableStoreProduct(NativeOneTableStoreProductManager),
    OneTableRewardTrackItem(NativeOneTableRewardTrackItemManager),
    RewardTrackData(NativeRewardTrackDataManager),
    PostSkillCapProgression(NativePostSkillCapProgressionDataManager),
    WhisperData(NativeWhisperDataManager),
    OneTableWorldEventRule(NativeOneTableWorldEventRuleManager),
    QuickCourseData(NativeQuickCourseDataManager),
    RotationalQueueData(NativeRotationalQueueDataManager),
    OneTableCostumeChange(NativeOneTableCostumeChangeManager),
    OneTableCrestPart(NativeOneTableCrestPartManager),
    OneTableDungeonTile(NativeOneTableDungeonTileManager),
    OneTableLevelDisparity(NativeOneTableLevelDisparityManager),
    OneTableEncumbrance(NativeOneTableEncumbranceManager),
    OneTableDifficultyScaling(NativeOneTableDifficultyScalingManager),
    OneTableDarkness(NativeOneTableDarknessManager),
    OneTableParticleData(NativeOneTableParticleDataManager),
    ItemData(NativeItemDataManager),
    ItemConversionData(NativeItemConversionDataManager),
    CharacterAttributeData(NativeCharacterAttributeDataManager),
    DamageData(NativeDamageDataManager),
    VitalsData(NativeVitalsDataManager),
    StatusEffectData(NativeStatusEffectDataManager),
    CurrencyExchangeMapping(NativeCurrencyExchangeMappingManager),
    GovernanceData(NativeGovernanceDataManager),
    LootBucketData(NativeLootBucketDataManager),
    EntitlementData(NativeEntitlementDataManager),
    EquipmentSetData(NativeEquipmentSetDataManager),
    TerritoryDefinitionsData(NativeTerritoryDefinitionsDataManager),
    SeasonsRewardsData(NativeSeasonsRewardsDataManager),
    SeasonsTrackedStatData(NativeSeasonsTrackedStatDataManager),
    StatModifierData(NativeStatModifierDataManager),
    DynamicDifficultyData(NativeDynamicDifficultyDataManager),
    ElementalMutationStaticData(NativeElementalMutationStaticDataManager),
    PromotionMutationStaticData(NativePromotionMutationStaticDataManager),
    MusicalRewardsData(NativeMusicalRewardsDataManager),
    ProgressionPointData(NativeProgressionPointDataManager),
    CombatProfilesData(NativeCombatProfilesDataManager),
    ItemTransformData(NativeItemTransformDataManager),
    GatherableData(NativeGatherableDataManager),
    SocialData(NativeSocialDataManager),
    SeasonsRewardsActivitiesTasksData(NativeSeasonsRewardsActivitiesTasksDataManager),
    SeasonsRewardsBattlePassData(NativeSeasonsRewardsBattlePassDataManager),
    SeasonsRewardsCardTemplateData(NativeSeasonsRewardsCardTemplateDataManager),
    SeasonsRewardsChapterData(NativeSeasonsRewardsChapterDataManager),
    SeasonsRewardsJourneyData(NativeSeasonsRewardsJourneyDataManager),
    SongBookSheetData(NativeSongBookSheetDataManager),
    SongBookData(NativeSongBookDataManager),
    PlayerData(NativePlayerDataManager),
    TradeskillRankData(NativeTradeskillRankDataManager),
    StaticTradeskillRankDataMapping(NativeStaticTradeskillRankDataMappingManager),
    VitalsModifierMapping(NativeVitalsModifierMappingManager),
    RecipeData(NativeRecipeDataManager),
    ReplicationData(NativeReplicationDataManager),
    ProductAssetResource(NativeProductAssetResourceManager),
    ComposedResource(NativeComposedResourceManager),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeneratedManagerSurface {
    AvailabilityResource,
    TypedAssetResource,
    NativeApiManager,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableCrcIndexManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    row_alias: RustIdentifier,
    table_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    row_key_method: RustIdentifier,
    row_crc_method: Option<RustIdentifier>,
    hash_policy: NativeCrcHashPolicy,
    reject_zero_crc: bool,
    methods: Vec<NativeCrcIndexLookupMethod>,
    source_row_method: Option<RustIdentifier>,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTableFamilyCrcIndexManager {
    module: RustIdentifier,
    tables_type: RustIdentifier,
    table_type: RustIdentifier,
    handle_type: RustIdentifier,
    row_alias: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    row_key_method: RustIdentifier,
    row_crc_method: Option<RustIdentifier>,
    reject_zero_crc: bool,
    methods: Vec<NativeCrcIndexLookupMethod>,
    source_handle_method: Option<NativeSourceHandleMethod>,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableOwnedStringCrcIndexManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    row_alias: RustIdentifier,
    table_field: RustIdentifier,
    indexes_field: RustIdentifier,
    indexed_type: RustIdentifier,
    indexes_type: RustIdentifier,
    indexed_key_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    skip_empty_key: bool,
    ascii_case_insensitive: bool,
    duplicate_manager_label: RustIdentifier,
    duplicate_key_label: RustIdentifier,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    methods: Vec<NativeCrcIndexLookupMethod>,
    source_row_method: Option<RustIdentifier>,
    ids_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTableFamilyOwnedStringCrcIndexManager {
    module: RustIdentifier,
    table_module: Option<RustIdentifier>,
    tables: Vec<NativeTableFamilyTable>,
    table_type: RustIdentifier,
    handle_type: RustIdentifier,
    row_alias: RustIdentifier,
    tables_field: RustIdentifier,
    indexes_field: RustIdentifier,
    indexed_type: RustIdentifier,
    indexes_type: RustIdentifier,
    indexed_key_field: RustIdentifier,
    indexed_source_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    skip_empty_key: bool,
    ascii_case_insensitive: bool,
    duplicate_manager_label: RustIdentifier,
    duplicate_key_label: RustIdentifier,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    methods: Vec<NativeCrcIndexLookupMethod>,
    source_handle_method: Option<NativeSourceHandleMethod>,
    ids_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableCrcKeyProjectionManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    key_field: RustIdentifier,
    crc_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    key_wrapper_type: Option<RustIdentifier>,
    key_storage_transform: NativeCrcKeyStorageTransform,
    hash_policy: NativeCrcHashPolicy,
    skip_empty_key: bool,
    trim_key: bool,
    reject_zero_crc: bool,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_row_field: Option<RustIdentifier>,
    source_row_method: Option<RustIdentifier>,
    source_handle_type: Option<RustIdentifier>,
    row_filters: Vec<NativeCrcProjectionRowFilter>,
    fields: Vec<NativeProjectionField>,
    schema_fields: Option<NativeSchemaProjectionFields>,
    schema_validation_fields: Option<NativeSchemaProjectionFields>,
    secondary_indexes: Vec<NativeCrcProjectionSecondaryIndex>,
    descending_f32_indexes: Vec<NativeDescendingF32Index>,
    methods: Vec<NativeCrcIndexLookupMethod>,
    dependency_lookup_methods: Vec<NativeCrcProjectionDependencyLookupMethod>,
    store_key_text: bool,
    ids_method: Option<RustIdentifier>,
    crc_ids_method: Option<RustIdentifier>,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeMultiTableCrcKeyProjectionManager {
    module: RustIdentifier,
    projections: Vec<NativeOneTableCrcKeyProjectionManager>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeAbilityDataManager {
    module: RustIdentifier,
    tables: Vec<NativeTableFamilyTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeObjectivesDataManager {
    module: RustIdentifier,
    objective_tables: Vec<NativeTableFamilyTable>,
    objective_task_tables: Vec<NativeTableFamilyTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeContributionDataManager {
    module: RustIdentifier,
    tables: Vec<NativeTableFamilyTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuffBucketDataManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeStructureDataManager {
    module: RustIdentifier,
    footprint_table_name: GameDataTableName,
    footprint_row_type_name: GameDataRowTypeName,
    piece_table_name: GameDataTableName,
    piece_row_type_name: GameDataRowTypeName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeReusableScoreboardDataManager {
    module: RustIdentifier,
    pug_activity_table_name: GameDataTableName,
    pug_activity_row_type_name: GameDataRowTypeName,
    scoreboard_table_name: GameDataTableName,
    scoreboard_row_type_name: GameDataRowTypeName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeMountHitVolumeDataManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    master_dynamic_slice: GameAssetPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTableFamilyCrcKeyProjectionManager {
    module: RustIdentifier,
    table_module: Option<RustIdentifier>,
    tables: Vec<NativeTableFamilyTable>,
    tables_type: RustIdentifier,
    table_type: RustIdentifier,
    handle_type: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    key_field: RustIdentifier,
    crc_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    key_wrapper_type: Option<RustIdentifier>,
    key_storage_transform: NativeCrcKeyStorageTransform,
    hash_policy: NativeCrcHashPolicy,
    skip_empty_key: bool,
    trim_key: bool,
    reject_zero_crc: bool,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_row_field: Option<RustIdentifier>,
    source_row_method: Option<RustIdentifier>,
    source_row_by_crc_method: Option<RustIdentifier>,
    source_handle_field: Option<RustIdentifier>,
    source_handle_method: Option<NativeSourceHandleMethod>,
    row_filters: Vec<NativeCrcProjectionRowFilter>,
    fields: Vec<NativeProjectionField>,
    schema_validation_fields: Option<NativeSchemaProjectionFields>,
    table_indexes: Vec<NativeTableFamilyCrcTableIndex>,
    methods: Vec<NativeCrcIndexLookupMethod>,
    field_lookup_methods: Vec<NativeCrcProjectionFieldLookupMethod>,
    store_key_text: bool,
    ids_method: Option<RustIdentifier>,
    crc_ids_method: Option<RustIdentifier>,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTableFamilyCrcTableIndex {
    index_field: RustIdentifier,
    table_variant: RustIdentifier,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    methods: Vec<NativeCrcIndexLookupMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePartitionedCrcGlobalIndex {
    index_field: RustIdentifier,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    methods: Vec<NativeCrcIndexLookupMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTableFamilyFallbackCrcKeyProjectionManager {
    module: RustIdentifier,
    table_module: Option<RustIdentifier>,
    tables: Vec<NativeTableFamilyTable>,
    tables_type: RustIdentifier,
    data_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    key_kind_field: RustIdentifier,
    key_kind_type: RustIdentifier,
    primary_key_kind: RustIdentifier,
    fallback_key_kind: RustIdentifier,
    key_field: RustIdentifier,
    crc_field: RustIdentifier,
    primary_key_column: GameDataColumnName,
    primary_key_getter: RustIdentifier,
    fallback_key_column: GameDataColumnName,
    fallback_key_getter: RustIdentifier,
    skip_empty_key: bool,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    methods: Vec<NativeCrcIndexLookupMethod>,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTableFamilyPartitionedCrcKeyProjectionManager {
    module: RustIdentifier,
    tables: Vec<NativeTableFamilyTable>,
    tables_type: RustIdentifier,
    table_type: RustIdentifier,
    data_type: RustIdentifier,
    entries_field: RustIdentifier,
    key_field: RustIdentifier,
    crc_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    hash_policy: NativeCrcHashPolicy,
    skip_empty_key: bool,
    trim_key: bool,
    reject_zero_crc: bool,
    global_index: Option<NativePartitionedCrcGlobalIndex>,
    table_indexes: Vec<NativeTableFamilyCrcTableIndex>,
    fields: Vec<NativeProjectionField>,
    vec3_fields: Vec<NativeVec3ProjectionField>,
    store_key_text: bool,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeVec3ProjectionField {
    field: RustIdentifier,
    x_column: GameDataColumnName,
    x_getter: RustIdentifier,
    y_column: GameDataColumnName,
    y_getter: RustIdentifier,
    z_column: GameDataColumnName,
    z_getter: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCrcProjectionDependencyLookupMethod {
    name: RustIdentifier,
    dependency_type: RustTypePath,
    dependency_parameter: RustIdentifier,
    key_parameter: NativeCrcIndexLookupParameter,
    dependency_method: RustIdentifier,
    lookup_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCrcProjectionFieldLookupMethod {
    name: RustIdentifier,
    key_parameter: NativeCrcIndexLookupParameter,
    field: RustIdentifier,
    value_type: RustTypePath,
    optional_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableNumericKeyProjectionManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    key_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    key_type: NativeNumericKeyType,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_row_field: Option<RustIdentifier>,
    source_row_method: Option<RustIdentifier>,
    fields: Vec<NativeProjectionField>,
    methods: Vec<NativeNumericLookupMethod>,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTableFamilyNumericKeyProjectionManager {
    module: RustIdentifier,
    tables: Vec<NativeTableFamilyTable>,
    tables_type: RustIdentifier,
    table_type: RustIdentifier,
    handle_type: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    key_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    key_type: NativeNumericKeyType,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_handle_field: Option<RustIdentifier>,
    source_handle_method: Option<NativeSourceHandleMethod>,
    fields: Vec<NativeProjectionField>,
    methods: Vec<NativeNumericLookupMethod>,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableEnumKeyProjectionManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    key_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    key_type: RustTypePath,
    key_type_alias: Option<RustIdentifier>,
    table_view_alias: Option<RustIdentifier>,
    expose_table_constructor: bool,
    invalid_key_variants: Vec<RustIdentifier>,
    skip_empty_key: bool,
    trim_key: bool,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_row_field: Option<RustIdentifier>,
    source_row_method: Option<RustIdentifier>,
    secondary_crc_index: Option<NativeEnumProjectionCrcIndex>,
    fields: Vec<NativeProjectionField>,
    methods: Vec<NativeEnumLookupMethod>,
    ids_method: Option<RustIdentifier>,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableStringKeyProjectionManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    map_field: RustIdentifier,
    key_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    skip_empty_key: bool,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    fields: Vec<NativeProjectionField>,
    methods: Vec<NativeStringLookupMethod>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableRowProjectionManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    entries_field: RustIdentifier,
    source_row_field: Option<RustIdentifier>,
    source_row_method: Option<RustIdentifier>,
    source_row_for_method: Option<RustIdentifier>,
    fields: Vec<NativeProjectionField>,
    rows_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTablePvpBalanceManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    target_column: GameDataColumnName,
    target_getter: RustIdentifier,
    category_column: GameDataColumnName,
    category_getter: RustIdentifier,
    methods: Vec<NativeCrcIndexLookupMethod>,
    balances_method: Option<RustIdentifier>,
    len_method: Option<RustIdentifier>,
    is_empty_method: Option<RustIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableCampSkinManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    table_field: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    settings_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    item_id_getter: RustIdentifier,
    required_achievement_id_getter: RustIdentifier,
    is_entitlement_getter: RustIdentifier,
    is_enabled_getter: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_key_method: RustIdentifier,
    ids_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableDyeColorManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    table_field: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    index_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    entitlement_indexes_field: RustIdentifier,
    index_column: GameDataColumnName,
    index_getter: RustIdentifier,
    name_getter: RustIdentifier,
    color_getter: RustIdentifier,
    category_getter: RustIdentifier,
    is_entitlement_getter: RustIdentifier,
    color_amount_getter: RustIdentifier,
    color_override_getter: RustIdentifier,
    spec_color_getter: RustIdentifier,
    spec_amount_getter: RustIdentifier,
    mask_gloss_shift_getter: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_from_index_method: RustIdentifier,
    lookup_by_key_method: RustIdentifier,
    rows_method: RustIdentifier,
    entitlement_indexes_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableEmoteManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    table_field: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    settings_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    lookup_from_id_method: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_key_method: RustIdentifier,
    status_effect_lookup_by_crc_method: RustIdentifier,
    status_effect_lookup_method: RustIdentifier,
    rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableExperienceManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    table_field: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    named_value_type: RustIdentifier,
    gear_score_bonus_type: RustIdentifier,
    lookup_from_id_method: RustIdentifier,
    lookup_method: RustIdentifier,
    gear_score_lookup_method: RustIdentifier,
    level_for_xp_method: RustIdentifier,
    max_level_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableStoreCategoryManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    table_field: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    tab_type: RustIdentifier,
    invalid_product_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    num_categories_method: RustIdentifier,
    rows_method: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_name_method: RustIdentifier,
    lookup_by_index_method: RustIdentifier,
    product_type_lookup_method: RustIdentifier,
    invalid_product_types_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableStoreProductManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    table_field: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    invalid_product_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_tag_method: RustIdentifier,
    rows_method: RustIdentifier,
    invalid_product_types_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableRewardTrackItemManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    table_field: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    payload_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    lookup_from_id_method: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_key_method: RustIdentifier,
    rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePostSkillCapProgressionDataManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    table_field: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    level_rewards_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_from_id_method: RustIdentifier,
    rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeWhisperDataManager {
    module: RustIdentifier,
    whisper_table_name: GameDataTableName,
    whisper_row_type_name: GameDataRowTypeName,
    whisper_table_field: RustIdentifier,
    whisper_row_alias: RustIdentifier,
    vfx_table_name: GameDataTableName,
    vfx_row_type_name: GameDataRowTypeName,
    vfx_table_field: RustIdentifier,
    vfx_row_alias: RustIdentifier,
    data_type: RustIdentifier,
    vfx_data_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    lookup_from_id_method: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_key_method: RustIdentifier,
    rows_method: RustIdentifier,
    ids_method: RustIdentifier,
    vfx_lookup_from_id_method: RustIdentifier,
    vfx_lookup_method: RustIdentifier,
    vfx_for_method: RustIdentifier,
    vfx_rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableWorldEventRuleManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    table_field: RustIdentifier,
    row_alias: RustIdentifier,
    data_type: RustIdentifier,
    crc_filter_type: RustIdentifier,
    zone_filter_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_crc_method: RustIdentifier,
    rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeQuickCourseDataManager {
    module: RustIdentifier,
    quick_course_table_name: GameDataTableName,
    quick_course_row_type_name: GameDataRowTypeName,
    quick_course_table_field: RustIdentifier,
    quick_course_row_alias: RustIdentifier,
    node_type_table_name: GameDataTableName,
    node_type_row_type_name: GameDataRowTypeName,
    node_type_table_field: RustIdentifier,
    node_type_row_alias: RustIdentifier,
    data_type: RustIdentifier,
    node_type_data_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    quick_course_lookup_method: RustIdentifier,
    quick_course_lookup_by_crc_method: RustIdentifier,
    quick_courses_method: RustIdentifier,
    quick_course_ids_method: RustIdentifier,
    first_quick_course_id_method: RustIdentifier,
    node_type_lookup_method: RustIdentifier,
    node_type_lookup_by_crc_method: RustIdentifier,
    node_types_method: RustIdentifier,
    node_type_ids_method: RustIdentifier,
    first_node_type_id_method: RustIdentifier,
    quick_course_len_method: RustIdentifier,
    node_type_len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRotationalQueueDataManager {
    module: RustIdentifier,
    queue_table_name: GameDataTableName,
    queue_row_type_name: GameDataRowTypeName,
    queue_table_field: RustIdentifier,
    queue_row_alias: RustIdentifier,
    game_mode_table_name: GameDataTableName,
    game_mode_row_type_name: GameDataRowTypeName,
    game_mode_table_field: RustIdentifier,
    data_type: RustIdentifier,
    cache_type: RustIdentifier,
    cache_field: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_from_id_method: RustIdentifier,
    rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableCostumeChangeManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    slot_type: RustIdentifier,
    override_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    source_row_index_field: RustIdentifier,
    key_field: RustIdentifier,
    key_text_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    mesh_field: RustIdentifier,
    mesh_column: GameDataColumnName,
    mesh_getter: RustIdentifier,
    matches_skeleton_field: RustIdentifier,
    matches_skeleton_column: GameDataColumnName,
    matches_skeleton_getter: RustIdentifier,
    z_offset_field: RustIdentifier,
    z_offset_column: GameDataColumnName,
    z_offset_getter: RustIdentifier,
    audio_overrides_field: RustIdentifier,
    source_row_field: RustIdentifier,
    source_row_method: RustIdentifier,
    slots: Vec<NativeCostumeAudioSlot>,
    lookup_from_id_method: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_key_method: RustIdentifier,
    audio_override_from_id_method: RustIdentifier,
    audio_override_method: RustIdentifier,
    rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCostumeAudioSlot {
    variant: RustIdentifier,
    display: RustIdentifier,
    left_column: GameDataColumnName,
    left_getter: RustIdentifier,
    right_column: GameDataColumnName,
    right_getter: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableCrestPartManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    kind_type: RustIdentifier,
    faction_type: RustIdentifier,
    parse_error_type: RustIdentifier,
    indexes_type: RustIdentifier,
    entries_field: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableDungeonTileManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    source_row_index_field: RustIdentifier,
    variant_index_field: RustIdentifier,
    key_field: RustIdentifier,
    crc_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    feature_key_field: RustIdentifier,
    feature_crc_field: RustIdentifier,
    feature_column: GameDataColumnName,
    feature_getter: RustIdentifier,
    connections_field: RustIdentifier,
    connections_column: GameDataColumnName,
    connections_getter: RustIdentifier,
    rotations_field: RustIdentifier,
    rotations_column: GameDataColumnName,
    rotations_getter: RustIdentifier,
    tile_size_field: RustIdentifier,
    tile_size_column: GameDataColumnName,
    tile_size_getter: RustIdentifier,
    weight_field: RustIdentifier,
    weight_column: GameDataColumnName,
    weight_getter: RustIdentifier,
    variation_asset_paths_field: RustIdentifier,
    variation_asset_paths_column: GameDataColumnName,
    variation_asset_paths_getter: RustIdentifier,
    supported_room_types_field: RustIdentifier,
    supported_room_types_column: GameDataColumnName,
    supported_room_types_getter: RustIdentifier,
    source_row_field: RustIdentifier,
    source_row_method: RustIdentifier,
    methods: Vec<NativeCrcIndexLookupMethod>,
    tile_variants_method: RustIdentifier,
    tile_variant_row_method: RustIdentifier,
    rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableLevelDisparityManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    range_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    source_row_index_field: RustIdentifier,
    range_field: RustIdentifier,
    max_capped_field: RustIdentifier,
    key_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    source_row_field: RustIdentifier,
    source_row_method: RustIdentifier,
    capped_value_source_field: RustIdentifier,
    fields: Vec<NativeProjectionField>,
    lookup_method: RustIdentifier,
    levels_method: RustIdentifier,
    clamped_levels_method: RustIdentifier,
    capped_levels_method: RustIdentifier,
    capped_clamped_levels_method: RustIdentifier,
    loaded_range_method: RustIdentifier,
    clamped_key_method: RustIdentifier,
    max_capped_value_method: RustIdentifier,
    rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableEncumbranceManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    load_state_type: RustIdentifier,
    load_values_type: RustIdentifier,
    indexes_type: RustIdentifier,
    entries_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    lookup_from_id_method: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_key_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableDifficultyScalingManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    affected_creature_types_type: RustIdentifier,
    health_modifier_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    source_row_field: RustIdentifier,
    key_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    lookup_from_id_method: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_key_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableDarknessManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    threshold_type: RustIdentifier,
    level_type: RustIdentifier,
    activation_spec_type: RustIdentifier,
    group_spec_type: RustIdentifier,
    entries_field: RustIdentifier,
    index_field: RustIdentifier,
    source_index_field: RustIdentifier,
    source_row_field: RustIdentifier,
    key_field: RustIdentifier,
    crc_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    lookup_crc_method: RustIdentifier,
    lookup_method: RustIdentifier,
    source_row_method: RustIdentifier,
    rows_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOneTableParticleDataManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    data_type: RustIdentifier,
    group_type: RustIdentifier,
    lookup_type: RustIdentifier,
    indexes_type: RustIdentifier,
    entries_field: RustIdentifier,
    local_player_factor_field: RustIdentifier,
    max_total_number_emitters_field: RustIdentifier,
    max_total_group_number_emitters_field: RustIdentifier,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
    group_column: GameDataColumnName,
    group_getter: RustIdentifier,
    max_number_column: GameDataColumnName,
    max_number_getter: RustIdentifier,
    priority_column: GameDataColumnName,
    priority_getter: RustIdentifier,
    constants_column: GameDataColumnName,
    constants_getter: RustIdentifier,
    lookup_from_id_method: RustIdentifier,
    lookup_method: RustIdentifier,
    lookup_by_key_method: RustIdentifier,
    local_player_factor_method: RustIdentifier,
    max_total_number_emitters_method: RustIdentifier,
    max_total_group_number_emitters_method: RustIdentifier,
    len_method: RustIdentifier,
    is_empty_method: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProductAssetResourceManager {
    manager_type: RustTypePath,
    constructor: RustIdentifier,
    products: Vec<NativeProductAssetResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProductAssetResource {
    product_type: RustTypePath,
    value_type: RustTypePath,
    handle_getter: RustIdentifier,
    asset_getter: RustIdentifier,
    manager_getter: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRecipeDataManager {
    module: RustIdentifier,
    tables: Vec<NativeTableFamilyTable>,
    table_type: RustIdentifier,
    handle_type: RustIdentifier,
    data_type: RustIdentifier,
    product: NativeProductAssetResource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeItemDataManager {
    module: RustIdentifier,
    tables: Vec<NativeTableFamilyTable>,
    table_type: RustIdentifier,
    handle_type: RustIdentifier,
    data_type: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeItemConversionDataManager {
    module: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    handle_type: RustIdentifier,
    data_type: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCharacterAttributeDataManager {
    module: RustIdentifier,
    tables: Vec<NativeTableFamilyTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDamageDataManager {
    module: RustIdentifier,
    damage_tables: Vec<NativeTableFamilyTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeVitalsDataManager {
    module: RustIdentifier,
    tables: Vec<NativeTableFamilyTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeStatusEffectDataManager {
    module: RustIdentifier,
    tables: Vec<NativeTableFamilyTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCurrencyExchangeMappingManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRewardTrackDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDynamicDifficultyDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeGovernanceDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLootBucketDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntitlementDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEquipmentSetDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTerritoryDefinitionsDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSeasonsRewardsDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSeasonsTrackedStatDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeStatModifierDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeElementalMutationStaticDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePromotionMutationStaticDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeMusicalRewardsDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProgressionPointDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCombatProfilesDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeItemTransformDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeGatherableDataManager {
    module: RustIdentifier,
    gathering_database: NativeProductAssetResource,
    gathering_action_database: NativeProductAssetResource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSocialDataManager {
    module: RustIdentifier,
    rank_database: NativeProductAssetResource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSeasonsRewardsActivitiesTasksDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSeasonsRewardsBattlePassDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSeasonsRewardsCardTemplateDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSeasonsRewardsChapterDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSeasonsRewardsJourneyDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSongBookSheetDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSongBookDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlayerDataManager {
    module: RustIdentifier,
    product_assets: NativeProductAssetResourceManager,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTradeskillRankDataManager {
    module: RustIdentifier,
    xp_table_name: GameDataTableName,
    xp_row_type_name: GameDataRowTypeName,
    rank_tables: Vec<NativeTableFamilyTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeStaticTradeskillRankDataMappingManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeVitalsModifierMappingManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeReplicationDataManager {
    module: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeComposedResourceManager {
    manager_type: RustTypePath,
    constructor: RustIdentifier,
    arguments: Vec<NativeComposedResourceArgument>,
    returns_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeComposedResourceArgument {
    Tables,
    Manager(RustTypePath),
    Product(NativeProductAssetResource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTableFamilyTable {
    variant: RustIdentifier,
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCrcIndexLookupMethod {
    name: RustIdentifier,
    parameter: NativeCrcIndexLookupParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSourceHandleMethod {
    name: RustIdentifier,
    parameter: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCrcIndexLookupParameter {
    name: RustIdentifier,
    kind: NativeCrcIndexLookupParameterKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeCrcIndexLookupParameterKind {
    StrRef,
    AsRefStr,
    Crc32,
    IntoCrc32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeCrcHashPolicy {
    Lowercase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeCrcKeyStorageTransform {
    Raw,
    RemoveSpaceAndTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeDuplicateKeyPolicy {
    Error,
    FirstWins,
    Overwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeNumericKeyType {
    NonZeroU32,
    NonZeroU8,
    U16,
    U16FromNonZeroU32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeNumericLookupMethod {
    name: RustIdentifier,
    parameter: RustIdentifier,
    parameter_kind: NativeNumericLookupParameterKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeNumericLookupParameterKind {
    NonZeroU32,
    NonZeroU8,
    U16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEnumLookupMethod {
    name: RustIdentifier,
    parameter: RustIdentifier,
    parameter_kind: NativeEnumLookupParameterKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeEnumLookupParameterKind {
    Enum,
    AsRefStr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEnumProjectionCrcIndex {
    index_field: RustIdentifier,
    crc_field: RustIdentifier,
    methods: Vec<NativeCrcIndexLookupMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeStringLookupMethod {
    name: RustIdentifier,
    parameter: RustIdentifier,
    parameter_kind: NativeStringLookupParameterKind,
    target: NativeStringLookupTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeStringLookupParameterKind {
    StrRef,
    AsRefStr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeStringLookupTarget {
    Key,
    Field(RustIdentifier),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProjectionField {
    field: RustIdentifier,
    public_getter: Option<RustIdentifier>,
    column: GameDataColumnName,
    getter: RustIdentifier,
    transform: NativeProjectionTransform,
    value_type: Option<RustTypePath>,
    default_value: Option<RustPath>,
    reference_field: Option<RustIdentifier>,
    foreign_key_target: Option<NativeProjectionForeignKeyTarget>,
    u16_max_exclusive: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProjectionForeignKeyTarget {
    table_name: GameDataTableName,
    row_type_name: GameDataRowTypeName,
    key_column: GameDataColumnName,
    key_getter: RustIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSchemaProjectionFields {
    skipped_columns: Vec<GameDataColumnName>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeProjectionTransform {
    String,
    PlusJoinedList,
    NonEmptyString,
    OptionalString,
    OptionalFirstString,
    OptionalStringDefaultEmpty,
    TypedCell,
    OptionalTypedCellDefaultValue,
    U8Enum,
    OptionalU8EnumDefaultValue,
    U8,
    OptionalU8DefaultZero,
    OptionalU8DefaultMax,
    U32,
    OptionalU32,
    OptionalU32DefaultZero,
    U32ToU16BelowMax,
    I32,
    Bool,
    OptionalBool,
    OptionalBoolDefaultFalse,
    F32,
    F32RangeInclusive,
    U32RangeInclusive,
    OptionalF32,
    F32MinutesToSeconds,
    F32ListDefaultEmpty,
    I32ListDefaultEmpty,
    Crc32,
    Crc32NonZeroBool,
    OptionalCrc32,
    OptionalCrc32ZeroAsNone,
    LowercaseCrcString,
    OptionalLowercaseCrcString,
    CrcList,
    OptionalCrcListDefaultEmpty,
    LowercaseCrcStringList,
    TrimmedLowercaseCrcStringList,
    OptionalLowercaseCrcStringDefaultZero,
    OptionalFirstLowercaseCrcStringDefaultZero,
    OptionalTrimmedLowercaseCrcString,
    OptionalTrimmedLowercaseCrcStringDefaultZero,
    ForeignKeyTargetKey,
    ForeignKeyTargetLowercaseCrc,
    ForeignKeyRow,
    OptionalForeignKeyRow,
    ForeignKeyRowList,
    OptionalForeignKeyRowListDefaultEmpty,
    EnumString,
    EnumStringRejectDefault,
    NonZeroU32,
    OptionalNonZeroU32,
    StringList,
    NonEmptyStringList,
    OptionalStringList,
    F32UpperBound10000ZeroIsDefault,
    F32LowerBound10000CappedToField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCrcProjectionRowFilter {
    column: GameDataColumnName,
    getter: RustIdentifier,
    predicate: NativeCrcProjectionRowFilterPredicate,
    compare_getter: Option<RustIdentifier>,
    extra_getters: Vec<RustIdentifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeCrcProjectionRowFilterPredicate {
    BoolTrueWhenPresent,
    BoolMustBeTrue,
    F32GreaterThanOrEqualZero,
    F32LessThanOrEqualZero,
    F32AnyGreaterThanZero,
    I32LessThanOrEqualZero,
    LowercaseCrcStringNonZero,
    StringNotEqualToColumn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCrcProjectionSecondaryIndex {
    index_field: RustIdentifier,
    key_field: RustIdentifier,
    key_type: NativeSecondaryIndexKeyType,
    storage: NativeSecondaryIndexStorage,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    methods: Vec<NativeSecondaryIndexLookupMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDescendingF32Index {
    index_field: RustIdentifier,
    value_field: RustIdentifier,
    rows_method: RustIdentifier,
    threshold_lookup_method: RustIdentifier,
    threshold_parameter: RustIdentifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeSecondaryIndexKeyType {
    U16,
    NonZeroU32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeSecondaryIndexStorage {
    HashMap,
    SparseVec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSecondaryIndexLookupMethod {
    name: RustIdentifier,
    parameter: RustIdentifier,
    parameter_kind: NativeSecondaryIndexLookupParameterKind,
    result: NativeSecondaryIndexLookupResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeSecondaryIndexLookupParameterKind {
    U16,
    U32,
    NonZeroU32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeSecondaryIndexLookupResult {
    DataRef,
    StringField(RustIdentifier),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NativeManagerShapeError {
    #[error("manager shape `{module}` must expose at least one lookup method")]
    MissingLookupMethods { module: RustIdentifier },

    #[error("manager shape `{module}` must expose at least one table")]
    MissingTables { module: RustIdentifier },

    #[error("manager shape `{module}` missing partitioned table index for `{table}`")]
    MissingTableIndex {
        module: RustIdentifier,
        table: RustIdentifier,
    },

    #[error("manager shape `{module}` must expose at least one product")]
    MissingProducts { module: RustIdentifier },

    #[error("manager shape `{module}` must expose at least one constructor argument")]
    MissingArguments { module: RustIdentifier },
}

impl Default for NativeCrcHashPolicy {
    fn default() -> Self {
        Self::Lowercase
    }
}

impl Default for NativeCrcKeyStorageTransform {
    fn default() -> Self {
        Self::Raw
    }
}

impl NativeManagerSpec {
    #[must_use]
    pub fn new(
        ghidra_class: GhidraClassPath,
        rust_type: RustTypePath,
        inputs: Vec<NativeManagerInput>,
        ghidra_functions: Vec<GhidraFunctionPath>,
    ) -> Result<Self, NativeClassSpecError> {
        validate_native_class_spec_inputs(
            "native manager",
            "typed input",
            &ghidra_class,
            &rust_type,
            &inputs,
            &ghidra_functions,
        )?;
        Ok(Self {
            native: NativeClassSpec::new(ghidra_class, rust_type, inputs, ghidra_functions),
            shape: None,
        })
    }

    #[must_use]
    pub fn class_evidence(
        ghidra_class: GhidraClassPath,
        rust_type: RustTypePath,
        inputs: Vec<NativeManagerInput>,
    ) -> Result<Self, NativeClassSpecError> {
        if inputs.is_empty() {
            return Err(NativeClassSpecError::MissingReferences {
                kind: "native manager",
                reference_kind: "typed input",
                ghidra_class,
                rust_type,
            });
        }

        Ok(Self {
            native: NativeClassSpec::new(ghidra_class, rust_type, inputs, Vec::new()),
            shape: None,
        })
    }

    #[must_use]
    pub fn runtime_manifest(
        ghidra_class: GhidraClassPath,
        rust_type: RustTypePath,
        inputs: Vec<NativeManagerInput>,
    ) -> Result<Self, NativeClassSpecError> {
        if inputs.is_empty() {
            return Err(NativeClassSpecError::MissingReferences {
                kind: "native manager manifest",
                reference_kind: "typed input",
                ghidra_class,
                rust_type,
            });
        }

        Ok(Self {
            native: NativeClassSpec::new(ghidra_class, rust_type, inputs, Vec::new()),
            shape: Some(NativeManagerShape::requirements_only()),
        })
    }

    #[must_use]
    pub fn with_shape(mut self, shape: NativeManagerShape) -> Self {
        self.shape = Some(shape);
        self
    }

    #[must_use]
    pub fn without_shape(mut self) -> Self {
        self.shape = None;
        self
    }

    #[must_use]
    pub const fn ghidra_class(&self) -> &GhidraClassPath {
        self.native.ghidra_class()
    }

    #[must_use]
    pub const fn rust_type(&self) -> &RustTypePath {
        self.native.rust_type()
    }

    #[must_use]
    pub fn inputs(&self) -> &[NativeManagerInput] {
        self.native.references()
    }

    #[must_use]
    pub fn ghidra_functions(&self) -> &[GhidraFunctionPath] {
        self.native.ghidra_functions()
    }

    #[must_use]
    pub const fn shape(&self) -> Option<&NativeManagerShape> {
        self.shape.as_ref()
    }

    #[must_use]
    pub fn contract(&self) -> ManagerContract<'_> {
        ManagerContract {
            manager: self.rust_type(),
            inputs: self
                .inputs()
                .iter()
                .map(NativeManagerInput::contract)
                .collect(),
        }
    }
}

impl NativeManagerInput {
    #[must_use]
    pub const fn table(table_name: GameDataTableName, row_type_name: GameDataRowTypeName) -> Self {
        Self::Table(NativeManagerTableInput::new(table_name, row_type_name))
    }

    #[must_use]
    pub const fn product(product: NativeManagerProductInput) -> Self {
        Self::Product(product)
    }

    #[must_use]
    pub const fn object_stream_product(asset_path: GameAssetPath, rust_type: RustTypePath) -> Self {
        Self::Product(NativeManagerProductInput::new(
            NativeManagerProductFormat::ObjectStream,
            asset_path,
            rust_type,
        ))
    }

    #[must_use]
    pub const fn manager(rust_type: RustTypePath) -> Self {
        Self::Manager(rust_type)
    }

    fn contract(&self) -> ManagerContractInput<'_> {
        match self {
            Self::Table(table) => ManagerContractInput::Table {
                name: table.table_name(),
                row: table.row_type_name(),
                product_path: table.product_path(),
            },
            Self::Product(product) => ManagerContractInput::Asset {
                path: product.asset_path(),
                asset_type: product.rust_type(),
            },
            Self::Manager(manager) => ManagerContractInput::Manager { manager },
        }
    }
}

impl<'a> ManagerContract<'a> {
    #[must_use]
    pub const fn manager(&self) -> &'a RustTypePath {
        self.manager
    }

    #[must_use]
    pub fn inputs(&self) -> &[ManagerContractInput<'a>] {
        &self.inputs
    }
}

impl NativeManagerTableInput {
    #[must_use]
    pub const fn new(table_name: GameDataTableName, row_type_name: GameDataRowTypeName) -> Self {
        Self {
            table_name,
            row_type_name,
        }
    }

    #[must_use]
    pub const fn table_name(&self) -> &GameDataTableName {
        &self.table_name
    }

    #[must_use]
    pub const fn row_type_name(&self) -> &GameDataRowTypeName {
        &self.row_type_name
    }

    #[must_use]
    pub fn product_path(&self) -> String {
        let row_type = to_module_ident(self.row_type_name.as_str(), "rowtype");
        let table = to_module_ident(self.table_name.as_str(), "table");
        format!("tables/{row_type}/{table}.aztbl")
    }
}

impl NativeManagerProductInput {
    #[must_use]
    pub const fn new(
        format: NativeManagerProductFormat,
        asset_path: GameAssetPath,
        rust_type: RustTypePath,
    ) -> Self {
        Self {
            format,
            asset_path,
            rust_type,
        }
    }

    #[must_use]
    pub const fn format(&self) -> NativeManagerProductFormat {
        self.format
    }

    #[must_use]
    pub const fn asset_path(&self) -> &GameAssetPath {
        &self.asset_path
    }

    #[must_use]
    pub const fn rust_type(&self) -> &RustTypePath {
        &self.rust_type
    }
}

impl NativeManagerShape {
    #[must_use]
    pub const fn requirements_only() -> Self {
        Self::RequirementsOnly
    }

    #[must_use]
    pub const fn is_requirements_only(&self) -> bool {
        matches!(self, Self::RequirementsOnly)
    }

    #[must_use]
    pub fn resource_surface(&self) -> GeneratedManagerSurface {
        match self {
            Self::RequirementsOnly => GeneratedManagerSurface::AvailabilityResource,
            Self::ProductAssetResource(_) => GeneratedManagerSurface::TypedAssetResource,
            Self::ComposedResource(manager) if manager.has_product_arguments() => {
                GeneratedManagerSurface::TypedAssetResource
            }
            Self::ComposedResource(_) => GeneratedManagerSurface::AvailabilityResource,
            _ => GeneratedManagerSurface::NativeApiManager,
        }
    }

    #[must_use]
    pub fn exposes_native_api(&self) -> bool {
        matches!(
            self.resource_surface(),
            GeneratedManagerSurface::NativeApiManager
        )
    }

    #[must_use]
    pub const fn ability_data(manager: NativeAbilityDataManager) -> Self {
        Self::AbilityData(manager)
    }

    #[must_use]
    pub const fn objectives_data(manager: NativeObjectivesDataManager) -> Self {
        Self::ObjectivesData(manager)
    }

    #[must_use]
    pub const fn contribution_data(manager: NativeContributionDataManager) -> Self {
        Self::ContributionData(manager)
    }

    #[must_use]
    pub const fn buff_bucket_data(manager: NativeBuffBucketDataManager) -> Self {
        Self::BuffBucketData(manager)
    }

    #[must_use]
    pub const fn structure_data(manager: NativeStructureDataManager) -> Self {
        Self::StructureData(manager)
    }

    #[must_use]
    pub const fn reusable_scoreboard_data(manager: NativeReusableScoreboardDataManager) -> Self {
        Self::ReusableScoreboardData(manager)
    }

    #[must_use]
    pub const fn mount_hit_volume_data(manager: NativeMountHitVolumeDataManager) -> Self {
        Self::MountHitVolumeData(manager)
    }

    #[must_use]
    pub const fn one_table_crc_index(manager: NativeOneTableCrcIndexManager) -> Self {
        Self::OneTableCrcIndex(manager)
    }

    #[must_use]
    pub const fn table_family_crc_index(manager: NativeTableFamilyCrcIndexManager) -> Self {
        Self::TableFamilyCrcIndex(manager)
    }

    #[must_use]
    pub const fn one_table_owned_string_crc_index(
        manager: NativeOneTableOwnedStringCrcIndexManager,
    ) -> Self {
        Self::OneTableOwnedStringCrcIndex(manager)
    }

    #[must_use]
    pub const fn table_family_owned_string_crc_index(
        manager: NativeTableFamilyOwnedStringCrcIndexManager,
    ) -> Self {
        Self::TableFamilyOwnedStringCrcIndex(manager)
    }

    #[must_use]
    pub const fn one_table_crc_key_projection(
        manager: NativeOneTableCrcKeyProjectionManager,
    ) -> Self {
        Self::OneTableCrcKeyProjection(manager)
    }

    #[must_use]
    pub const fn multi_table_crc_key_projection(
        manager: NativeMultiTableCrcKeyProjectionManager,
    ) -> Self {
        Self::MultiTableCrcKeyProjection(manager)
    }

    #[must_use]
    pub const fn table_family_crc_key_projection(
        manager: NativeTableFamilyCrcKeyProjectionManager,
    ) -> Self {
        Self::TableFamilyCrcKeyProjection(manager)
    }

    #[must_use]
    pub const fn table_family_fallback_crc_key_projection(
        manager: NativeTableFamilyFallbackCrcKeyProjectionManager,
    ) -> Self {
        Self::TableFamilyFallbackCrcKeyProjection(manager)
    }

    #[must_use]
    pub const fn table_family_partitioned_crc_key_projection(
        manager: NativeTableFamilyPartitionedCrcKeyProjectionManager,
    ) -> Self {
        Self::TableFamilyPartitionedCrcKeyProjection(manager)
    }

    #[must_use]
    pub const fn one_table_numeric_key_projection(
        manager: NativeOneTableNumericKeyProjectionManager,
    ) -> Self {
        Self::OneTableNumericKeyProjection(manager)
    }

    #[must_use]
    pub const fn table_family_numeric_key_projection(
        manager: NativeTableFamilyNumericKeyProjectionManager,
    ) -> Self {
        Self::TableFamilyNumericKeyProjection(manager)
    }

    #[must_use]
    pub const fn one_table_enum_key_projection(
        manager: NativeOneTableEnumKeyProjectionManager,
    ) -> Self {
        Self::OneTableEnumKeyProjection(manager)
    }

    #[must_use]
    pub const fn one_table_string_key_projection(
        manager: NativeOneTableStringKeyProjectionManager,
    ) -> Self {
        Self::OneTableStringKeyProjection(manager)
    }

    #[must_use]
    pub const fn one_table_row_projection(manager: NativeOneTableRowProjectionManager) -> Self {
        Self::OneTableRowProjection(manager)
    }

    #[must_use]
    pub const fn one_table_pvp_balance(manager: NativeOneTablePvpBalanceManager) -> Self {
        Self::OneTablePvpBalance(manager)
    }

    #[must_use]
    pub const fn one_table_camp_skin(manager: NativeOneTableCampSkinManager) -> Self {
        Self::OneTableCampSkin(manager)
    }

    #[must_use]
    pub const fn one_table_dye_color(manager: NativeOneTableDyeColorManager) -> Self {
        Self::OneTableDyeColor(manager)
    }

    #[must_use]
    pub const fn one_table_emote(manager: NativeOneTableEmoteManager) -> Self {
        Self::OneTableEmote(manager)
    }

    #[must_use]
    pub const fn one_table_experience(manager: NativeOneTableExperienceManager) -> Self {
        Self::OneTableExperience(manager)
    }

    #[must_use]
    pub const fn one_table_store_category(manager: NativeOneTableStoreCategoryManager) -> Self {
        Self::OneTableStoreCategory(manager)
    }

    #[must_use]
    pub const fn one_table_store_product(manager: NativeOneTableStoreProductManager) -> Self {
        Self::OneTableStoreProduct(manager)
    }

    #[must_use]
    pub const fn one_table_reward_track_item(
        manager: NativeOneTableRewardTrackItemManager,
    ) -> Self {
        Self::OneTableRewardTrackItem(manager)
    }

    #[must_use]
    pub const fn reward_track_data(manager: NativeRewardTrackDataManager) -> Self {
        Self::RewardTrackData(manager)
    }

    #[must_use]
    pub const fn post_skill_cap_progression(
        manager: NativePostSkillCapProgressionDataManager,
    ) -> Self {
        Self::PostSkillCapProgression(manager)
    }

    #[must_use]
    pub const fn whisper_data(manager: NativeWhisperDataManager) -> Self {
        Self::WhisperData(manager)
    }

    #[must_use]
    pub const fn one_table_world_event_rule(manager: NativeOneTableWorldEventRuleManager) -> Self {
        Self::OneTableWorldEventRule(manager)
    }

    #[must_use]
    pub const fn quick_course_data(manager: NativeQuickCourseDataManager) -> Self {
        Self::QuickCourseData(manager)
    }

    #[must_use]
    pub const fn rotational_queue_data(manager: NativeRotationalQueueDataManager) -> Self {
        Self::RotationalQueueData(manager)
    }

    #[must_use]
    pub const fn one_table_costume_change(manager: NativeOneTableCostumeChangeManager) -> Self {
        Self::OneTableCostumeChange(manager)
    }

    #[must_use]
    pub const fn one_table_crest_part(manager: NativeOneTableCrestPartManager) -> Self {
        Self::OneTableCrestPart(manager)
    }

    #[must_use]
    pub const fn one_table_dungeon_tile(manager: NativeOneTableDungeonTileManager) -> Self {
        Self::OneTableDungeonTile(manager)
    }

    #[must_use]
    pub const fn one_table_level_disparity(manager: NativeOneTableLevelDisparityManager) -> Self {
        Self::OneTableLevelDisparity(manager)
    }

    #[must_use]
    pub const fn one_table_encumbrance(manager: NativeOneTableEncumbranceManager) -> Self {
        Self::OneTableEncumbrance(manager)
    }

    #[must_use]
    pub const fn one_table_difficulty_scaling(
        manager: NativeOneTableDifficultyScalingManager,
    ) -> Self {
        Self::OneTableDifficultyScaling(manager)
    }

    #[must_use]
    pub const fn one_table_darkness(manager: NativeOneTableDarknessManager) -> Self {
        Self::OneTableDarkness(manager)
    }

    #[must_use]
    pub const fn one_table_particle_data(manager: NativeOneTableParticleDataManager) -> Self {
        Self::OneTableParticleData(manager)
    }

    #[must_use]
    pub const fn item_data(manager: NativeItemDataManager) -> Self {
        Self::ItemData(manager)
    }

    #[must_use]
    pub const fn item_conversion_data(manager: NativeItemConversionDataManager) -> Self {
        Self::ItemConversionData(manager)
    }

    #[must_use]
    pub const fn character_attribute_data(manager: NativeCharacterAttributeDataManager) -> Self {
        Self::CharacterAttributeData(manager)
    }

    #[must_use]
    pub const fn damage_data(manager: NativeDamageDataManager) -> Self {
        Self::DamageData(manager)
    }

    #[must_use]
    pub const fn vitals_data(manager: NativeVitalsDataManager) -> Self {
        Self::VitalsData(manager)
    }

    #[must_use]
    pub const fn status_effect_data(manager: NativeStatusEffectDataManager) -> Self {
        Self::StatusEffectData(manager)
    }

    #[must_use]
    pub const fn currency_exchange_mapping(manager: NativeCurrencyExchangeMappingManager) -> Self {
        Self::CurrencyExchangeMapping(manager)
    }

    #[must_use]
    pub const fn governance_data(manager: NativeGovernanceDataManager) -> Self {
        Self::GovernanceData(manager)
    }

    #[must_use]
    pub const fn loot_bucket_data(manager: NativeLootBucketDataManager) -> Self {
        Self::LootBucketData(manager)
    }

    #[must_use]
    pub const fn entitlement_data(manager: NativeEntitlementDataManager) -> Self {
        Self::EntitlementData(manager)
    }

    #[must_use]
    pub const fn equipment_set_data(manager: NativeEquipmentSetDataManager) -> Self {
        Self::EquipmentSetData(manager)
    }

    #[must_use]
    pub const fn territory_definitions_data(
        manager: NativeTerritoryDefinitionsDataManager,
    ) -> Self {
        Self::TerritoryDefinitionsData(manager)
    }

    #[must_use]
    pub const fn seasons_rewards_data(manager: NativeSeasonsRewardsDataManager) -> Self {
        Self::SeasonsRewardsData(manager)
    }

    #[must_use]
    pub const fn seasons_tracked_stat_data(manager: NativeSeasonsTrackedStatDataManager) -> Self {
        Self::SeasonsTrackedStatData(manager)
    }

    #[must_use]
    pub const fn stat_modifier_data(manager: NativeStatModifierDataManager) -> Self {
        Self::StatModifierData(manager)
    }

    #[must_use]
    pub const fn dynamic_difficulty_data(manager: NativeDynamicDifficultyDataManager) -> Self {
        Self::DynamicDifficultyData(manager)
    }

    #[must_use]
    pub const fn elemental_mutation_static_data(
        manager: NativeElementalMutationStaticDataManager,
    ) -> Self {
        Self::ElementalMutationStaticData(manager)
    }

    #[must_use]
    pub const fn promotion_mutation_static_data(
        manager: NativePromotionMutationStaticDataManager,
    ) -> Self {
        Self::PromotionMutationStaticData(manager)
    }

    #[must_use]
    pub const fn musical_rewards_data(manager: NativeMusicalRewardsDataManager) -> Self {
        Self::MusicalRewardsData(manager)
    }

    #[must_use]
    pub const fn progression_point_data(manager: NativeProgressionPointDataManager) -> Self {
        Self::ProgressionPointData(manager)
    }

    #[must_use]
    pub const fn combat_profiles_data(manager: NativeCombatProfilesDataManager) -> Self {
        Self::CombatProfilesData(manager)
    }

    #[must_use]
    pub const fn item_transform_data(manager: NativeItemTransformDataManager) -> Self {
        Self::ItemTransformData(manager)
    }

    #[must_use]
    pub const fn gatherable_data(manager: NativeGatherableDataManager) -> Self {
        Self::GatherableData(manager)
    }

    #[must_use]
    pub const fn social_data(manager: NativeSocialDataManager) -> Self {
        Self::SocialData(manager)
    }

    #[must_use]
    pub const fn seasons_rewards_activities_tasks_data(
        manager: NativeSeasonsRewardsActivitiesTasksDataManager,
    ) -> Self {
        Self::SeasonsRewardsActivitiesTasksData(manager)
    }

    #[must_use]
    pub const fn seasons_rewards_battle_pass_data(
        manager: NativeSeasonsRewardsBattlePassDataManager,
    ) -> Self {
        Self::SeasonsRewardsBattlePassData(manager)
    }

    #[must_use]
    pub const fn seasons_rewards_card_template_data(
        manager: NativeSeasonsRewardsCardTemplateDataManager,
    ) -> Self {
        Self::SeasonsRewardsCardTemplateData(manager)
    }

    #[must_use]
    pub const fn seasons_rewards_chapter_data(
        manager: NativeSeasonsRewardsChapterDataManager,
    ) -> Self {
        Self::SeasonsRewardsChapterData(manager)
    }

    #[must_use]
    pub const fn seasons_rewards_journey_data(
        manager: NativeSeasonsRewardsJourneyDataManager,
    ) -> Self {
        Self::SeasonsRewardsJourneyData(manager)
    }

    #[must_use]
    pub const fn song_book_sheet_data(manager: NativeSongBookSheetDataManager) -> Self {
        Self::SongBookSheetData(manager)
    }

    #[must_use]
    pub const fn song_book_data(manager: NativeSongBookDataManager) -> Self {
        Self::SongBookData(manager)
    }

    #[must_use]
    pub const fn player_data(manager: NativePlayerDataManager) -> Self {
        Self::PlayerData(manager)
    }

    #[must_use]
    pub const fn tradeskill_rank_data(manager: NativeTradeskillRankDataManager) -> Self {
        Self::TradeskillRankData(manager)
    }

    #[must_use]
    pub const fn static_tradeskill_rank_data_mapping(
        manager: NativeStaticTradeskillRankDataMappingManager,
    ) -> Self {
        Self::StaticTradeskillRankDataMapping(manager)
    }

    #[must_use]
    pub const fn vitals_modifier_mapping(manager: NativeVitalsModifierMappingManager) -> Self {
        Self::VitalsModifierMapping(manager)
    }

    #[must_use]
    pub const fn recipe_data(manager: NativeRecipeDataManager) -> Self {
        Self::RecipeData(manager)
    }

    #[must_use]
    pub const fn replication_data(manager: NativeReplicationDataManager) -> Self {
        Self::ReplicationData(manager)
    }

    #[must_use]
    pub const fn product_asset_resource(manager: NativeProductAssetResourceManager) -> Self {
        Self::ProductAssetResource(manager)
    }

    #[must_use]
    pub const fn composed_resource(manager: NativeComposedResourceManager) -> Self {
        Self::ComposedResource(manager)
    }
}

macro_rules! simple_accessors {
    ($($field:ident: $ty:ty),+ $(,)?) => {
        $(
            #[must_use]
            pub const fn $field(&self) -> &$ty {
                &self.$field
            }
        )+
    };
}

macro_rules! bool_accessors {
    ($($field:ident),+ $(,)?) => {
        $(
            #[must_use]
            pub const fn $field(&self) -> bool {
                self.$field
            }
        )+
    };
}

macro_rules! manager_common_methods {
    ($($field:ident => $with_fn:ident),+ $(,)?) => {
        $(
            #[must_use]
            pub fn $with_fn(mut self, method: RustIdentifier) -> Self {
                self.$field = Some(method);
                self
            }

            #[must_use]
            pub const fn $field(&self) -> Option<&RustIdentifier> {
                self.$field.as_ref()
            }
        )+
    };
}

macro_rules! projection_common_methods {
    () => {
        manager_common_methods! {
            source_row_method => with_source_row_method,
            rows_method => with_rows_method,
            len_method => with_len_method,
            is_empty_method => with_is_empty_method
        }

        #[must_use]
        pub fn with_source_row_field(mut self, field: RustIdentifier) -> Self {
            self.source_row_field = Some(field);
            self
        }

        #[must_use]
        pub const fn source_row_field(&self) -> Option<&RustIdentifier> {
            self.source_row_field.as_ref()
        }
    };
}

impl NativeOneTableCrcIndexManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        row_alias: RustIdentifier,
        table_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        reject_zero_crc: bool,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_name,
            row_type_name,
            row_alias,
            table_field,
            key_column,
            row_key_method: key_getter.clone(),
            key_getter,
            row_crc_method: None,
            hash_policy: NativeCrcHashPolicy::default(),
            reject_zero_crc,
            methods,
            source_row_method: None,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    #[must_use]
    pub fn with_row_key_method(mut self, method: RustIdentifier) -> Self {
        self.row_key_method = method;
        self
    }

    #[must_use]
    pub fn with_row_crc_method(mut self, method: RustIdentifier) -> Self {
        self.row_crc_method = Some(method);
        self
    }

    #[must_use]
    pub const fn with_hash_policy(mut self, policy: NativeCrcHashPolicy) -> Self {
        self.hash_policy = policy;
        self
    }

    manager_common_methods! {
        source_row_method => with_source_row_method,
        rows_method => with_rows_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        row_alias: RustIdentifier,
        table_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        row_key_method: RustIdentifier
    }

    #[must_use]
    pub const fn row_crc_method(&self) -> Option<&RustIdentifier> {
        self.row_crc_method.as_ref()
    }

    #[must_use]
    pub const fn hash_policy(&self) -> NativeCrcHashPolicy {
        self.hash_policy
    }

    #[must_use]
    pub const fn reject_zero_crc(&self) -> bool {
        self.reject_zero_crc
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }
}

impl NativeTableFamilyCrcIndexManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        tables_type: RustIdentifier,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        row_alias: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        reject_zero_crc: bool,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            tables_type,
            table_type,
            handle_type,
            row_alias,
            key_column,
            row_key_method: key_getter.clone(),
            key_getter,
            row_crc_method: None,
            reject_zero_crc,
            methods,
            source_handle_method: None,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    #[must_use]
    pub fn with_row_key_method(mut self, method: RustIdentifier) -> Self {
        self.row_key_method = method;
        self
    }

    #[must_use]
    pub fn with_row_crc_method(mut self, method: RustIdentifier) -> Self {
        self.row_crc_method = Some(method);
        self
    }

    #[must_use]
    pub fn with_source_handle_method(
        mut self,
        name: RustIdentifier,
        parameter: RustIdentifier,
    ) -> Self {
        self.source_handle_method = Some(NativeSourceHandleMethod::new(name, parameter));
        self
    }

    manager_common_methods! {
        rows_method => with_rows_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    simple_accessors! {
        module: RustIdentifier,
        tables_type: RustIdentifier,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        row_alias: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        row_key_method: RustIdentifier
    }

    #[must_use]
    pub const fn row_crc_method(&self) -> Option<&RustIdentifier> {
        self.row_crc_method.as_ref()
    }

    #[must_use]
    pub const fn reject_zero_crc(&self) -> bool {
        self.reject_zero_crc
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }

    #[must_use]
    pub const fn source_handle_method(&self) -> Option<&NativeSourceHandleMethod> {
        self.source_handle_method.as_ref()
    }
}

impl NativeOneTableOwnedStringCrcIndexManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        row_alias: RustIdentifier,
        table_field: RustIdentifier,
        indexes_field: RustIdentifier,
        indexed_type: RustIdentifier,
        indexes_type: RustIdentifier,
        indexed_key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        skip_empty_key: bool,
        ascii_case_insensitive: bool,
        duplicate_manager_label: RustIdentifier,
        duplicate_key_label: RustIdentifier,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_name,
            row_type_name,
            row_alias,
            table_field,
            indexes_field,
            indexed_type,
            indexes_type,
            indexed_key_field,
            key_column,
            key_getter,
            skip_empty_key,
            ascii_case_insensitive,
            duplicate_manager_label,
            duplicate_key_label,
            duplicate_key_policy: NativeDuplicateKeyPolicy::Error,
            methods,
            source_row_method: None,
            ids_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    #[must_use]
    pub const fn with_duplicate_key_policy(mut self, policy: NativeDuplicateKeyPolicy) -> Self {
        self.duplicate_key_policy = policy;
        self
    }

    manager_common_methods! {
        source_row_method => with_source_row_method,
        ids_method => with_ids_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        row_alias: RustIdentifier,
        table_field: RustIdentifier,
        indexes_field: RustIdentifier,
        indexed_type: RustIdentifier,
        indexes_type: RustIdentifier,
        indexed_key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        duplicate_manager_label: RustIdentifier,
        duplicate_key_label: RustIdentifier
    }

    bool_accessors! {
        skip_empty_key,
        ascii_case_insensitive
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }
}

impl NativeTableFamilyOwnedStringCrcIndexManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        row_alias: RustIdentifier,
        tables_field: RustIdentifier,
        indexes_field: RustIdentifier,
        indexed_type: RustIdentifier,
        indexes_type: RustIdentifier,
        indexed_key_field: RustIdentifier,
        indexed_source_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        skip_empty_key: bool,
        ascii_case_insensitive: bool,
        duplicate_manager_label: RustIdentifier,
        duplicate_key_label: RustIdentifier,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_tables(&module, &tables)?;
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_module: None,
            tables,
            table_type,
            handle_type,
            row_alias,
            tables_field,
            indexes_field,
            indexed_type,
            indexes_type,
            indexed_key_field,
            indexed_source_field,
            key_column,
            key_getter,
            skip_empty_key,
            ascii_case_insensitive,
            duplicate_manager_label,
            duplicate_key_label,
            duplicate_key_policy: NativeDuplicateKeyPolicy::Error,
            methods,
            source_handle_method: None,
            ids_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    #[must_use]
    pub const fn with_duplicate_key_policy(mut self, policy: NativeDuplicateKeyPolicy) -> Self {
        self.duplicate_key_policy = policy;
        self
    }

    #[must_use]
    pub fn with_source_handle_method(
        mut self,
        name: RustIdentifier,
        parameter: RustIdentifier,
    ) -> Self {
        self.source_handle_method = Some(NativeSourceHandleMethod::new(name, parameter));
        self
    }

    manager_common_methods! {
        ids_method => with_ids_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    #[must_use]
    pub fn with_table_module(mut self, module: RustIdentifier) -> Self {
        self.table_module = Some(module);
        self
    }

    #[must_use]
    pub fn table_module(&self) -> &RustIdentifier {
        self.table_module.as_ref().unwrap_or(&self.module)
    }

    simple_accessors! {
        module: RustIdentifier,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        row_alias: RustIdentifier,
        tables_field: RustIdentifier,
        indexes_field: RustIdentifier,
        indexed_type: RustIdentifier,
        indexes_type: RustIdentifier,
        indexed_key_field: RustIdentifier,
        indexed_source_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        duplicate_manager_label: RustIdentifier,
        duplicate_key_label: RustIdentifier
    }

    bool_accessors! {
        skip_empty_key,
        ascii_case_insensitive
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }

    #[must_use]
    pub const fn source_handle_method(&self) -> Option<&NativeSourceHandleMethod> {
        self.source_handle_method.as_ref()
    }
}

impl NativeOneTableCrcKeyProjectionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        skip_empty_key: bool,
        trim_key: bool,
        reject_zero_crc: bool,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        fields: Vec<NativeProjectionField>,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_name,
            row_type_name,
            data_type,
            entries_field,
            index_field,
            key_field,
            crc_field,
            key_column,
            key_getter,
            key_wrapper_type: None,
            key_storage_transform: NativeCrcKeyStorageTransform::default(),
            hash_policy: NativeCrcHashPolicy::default(),
            skip_empty_key,
            trim_key,
            reject_zero_crc,
            duplicate_key_policy,
            source_row_field: None,
            source_row_method: None,
            source_handle_type: None,
            row_filters: Vec::new(),
            fields,
            schema_fields: None,
            schema_validation_fields: None,
            secondary_indexes: Vec::new(),
            descending_f32_indexes: Vec::new(),
            methods,
            dependency_lookup_methods: Vec::new(),
            store_key_text: true,
            ids_method: None,
            crc_ids_method: None,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    #[must_use]
    pub const fn with_hash_policy(mut self, policy: NativeCrcHashPolicy) -> Self {
        self.hash_policy = policy;
        self
    }

    manager_common_methods! {
        source_row_method => with_source_row_method,
        ids_method => with_ids_method,
        crc_ids_method => with_crc_ids_method,
        rows_method => with_rows_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    #[must_use]
    pub fn with_source_row_field(mut self, field: RustIdentifier) -> Self {
        self.source_row_field = Some(field);
        self
    }

    #[must_use]
    pub const fn source_row_field(&self) -> Option<&RustIdentifier> {
        self.source_row_field.as_ref()
    }

    #[must_use]
    pub fn with_source_handle_type(mut self, handle_type: RustIdentifier) -> Self {
        self.source_handle_type = Some(handle_type);
        self
    }

    #[must_use]
    pub fn with_key_wrapper_type(mut self, key_wrapper_type: RustIdentifier) -> Self {
        self.key_wrapper_type = Some(key_wrapper_type);
        self
    }

    #[must_use]
    pub const fn with_key_storage_transform(
        mut self,
        transform: NativeCrcKeyStorageTransform,
    ) -> Self {
        self.key_storage_transform = transform;
        self
    }

    #[must_use]
    pub fn with_row_filter(mut self, filter: NativeCrcProjectionRowFilter) -> Self {
        self.row_filters.push(filter);
        self
    }

    #[must_use]
    pub fn with_secondary_index(mut self, index: NativeCrcProjectionSecondaryIndex) -> Self {
        self.secondary_indexes.push(index);
        self
    }

    #[must_use]
    pub fn with_descending_f32_index(mut self, index: NativeDescendingF32Index) -> Self {
        self.descending_f32_indexes.push(index);
        self
    }

    #[must_use]
    pub fn with_dependency_lookup_method(
        mut self,
        method: NativeCrcProjectionDependencyLookupMethod,
    ) -> Self {
        self.dependency_lookup_methods.push(method);
        self
    }

    #[must_use]
    pub const fn without_key_text(mut self) -> Self {
        self.store_key_text = false;
        self
    }

    #[must_use]
    pub fn with_schema_fields(mut self, fields: NativeSchemaProjectionFields) -> Self {
        self.schema_fields = Some(fields);
        self
    }

    #[must_use]
    pub fn with_schema_validation_fields(mut self, fields: NativeSchemaProjectionFields) -> Self {
        self.schema_validation_fields = Some(fields);
        self
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier
    }

    bool_accessors! {
        skip_empty_key,
        trim_key,
        reject_zero_crc
    }

    #[must_use]
    pub const fn hash_policy(&self) -> NativeCrcHashPolicy {
        self.hash_policy
    }

    #[must_use]
    pub const fn key_wrapper_type(&self) -> Option<&RustIdentifier> {
        self.key_wrapper_type.as_ref()
    }

    #[must_use]
    pub const fn key_storage_transform(&self) -> NativeCrcKeyStorageTransform {
        self.key_storage_transform
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub const fn source_handle_type(&self) -> Option<&RustIdentifier> {
        self.source_handle_type.as_ref()
    }

    #[must_use]
    pub fn row_filters(&self) -> &[NativeCrcProjectionRowFilter] {
        &self.row_filters
    }

    #[must_use]
    pub fn secondary_indexes(&self) -> &[NativeCrcProjectionSecondaryIndex] {
        &self.secondary_indexes
    }

    #[must_use]
    pub fn descending_f32_indexes(&self) -> &[NativeDescendingF32Index] {
        &self.descending_f32_indexes
    }

    #[must_use]
    pub fn fields(&self) -> &[NativeProjectionField] {
        &self.fields
    }

    #[must_use]
    pub const fn schema_fields(&self) -> Option<&NativeSchemaProjectionFields> {
        self.schema_fields.as_ref()
    }

    #[must_use]
    pub const fn schema_validation_fields(&self) -> Option<&NativeSchemaProjectionFields> {
        self.schema_validation_fields.as_ref()
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }

    #[must_use]
    pub fn dependency_lookup_methods(&self) -> &[NativeCrcProjectionDependencyLookupMethod] {
        &self.dependency_lookup_methods
    }

    #[must_use]
    pub const fn store_key_text(&self) -> bool {
        self.store_key_text
    }
}

impl NativeMultiTableCrcKeyProjectionManager {
    pub fn new(
        module: RustIdentifier,
        projections: Vec<NativeOneTableCrcKeyProjectionManager>,
    ) -> Result<Self, NativeManagerShapeError> {
        if projections.is_empty() {
            return Err(NativeManagerShapeError::MissingTables { module });
        }
        Ok(Self {
            module,
            projections,
        })
    }

    simple_accessors! {
        module: RustIdentifier
    }

    #[must_use]
    pub fn projections(&self) -> &[NativeOneTableCrcKeyProjectionManager] {
        &self.projections
    }
}

impl NativeAbilityDataManager {
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_tables(&module, &tables)?;
        Ok(Self { module, tables })
    }

    simple_accessors! {
        module: RustIdentifier
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }
}

impl NativeObjectivesDataManager {
    pub fn new(
        module: RustIdentifier,
        objective_tables: Vec<NativeTableFamilyTable>,
        objective_task_tables: Vec<NativeTableFamilyTable>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_tables(&module, &objective_tables)?;
        ensure_tables(&module, &objective_task_tables)?;
        Ok(Self {
            module,
            objective_tables,
            objective_task_tables,
        })
    }

    simple_accessors! {
        module: RustIdentifier
    }

    #[must_use]
    pub fn objective_tables(&self) -> &[NativeTableFamilyTable] {
        &self.objective_tables
    }

    #[must_use]
    pub fn objective_task_tables(&self) -> &[NativeTableFamilyTable] {
        &self.objective_task_tables
    }
}

impl NativeContributionDataManager {
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_tables(&module, &tables)?;
        Ok(Self { module, tables })
    }

    simple_accessors! {
        module: RustIdentifier
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }
}

impl NativeBuffBucketDataManager {
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
    }
}

impl NativeStructureDataManager {
    pub const fn new(
        module: RustIdentifier,
        footprint_table_name: GameDataTableName,
        footprint_row_type_name: GameDataRowTypeName,
        piece_table_name: GameDataTableName,
        piece_row_type_name: GameDataRowTypeName,
    ) -> Self {
        Self {
            module,
            footprint_table_name,
            footprint_row_type_name,
            piece_table_name,
            piece_row_type_name,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        footprint_table_name: GameDataTableName,
        footprint_row_type_name: GameDataRowTypeName,
        piece_table_name: GameDataTableName,
        piece_row_type_name: GameDataRowTypeName,
    }
}

impl NativeReusableScoreboardDataManager {
    pub const fn new(
        module: RustIdentifier,
        pug_activity_table_name: GameDataTableName,
        pug_activity_row_type_name: GameDataRowTypeName,
        scoreboard_table_name: GameDataTableName,
        scoreboard_row_type_name: GameDataRowTypeName,
    ) -> Self {
        Self {
            module,
            pug_activity_table_name,
            pug_activity_row_type_name,
            scoreboard_table_name,
            scoreboard_row_type_name,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        pug_activity_table_name: GameDataTableName,
        pug_activity_row_type_name: GameDataRowTypeName,
        scoreboard_table_name: GameDataTableName,
        scoreboard_row_type_name: GameDataRowTypeName,
    }
}

impl NativeMountHitVolumeDataManager {
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        master_dynamic_slice: GameAssetPath,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            master_dynamic_slice,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        master_dynamic_slice: GameAssetPath,
    }
}

impl NativeTableFamilyCrcKeyProjectionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
        tables_type: RustIdentifier,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        skip_empty_key: bool,
        trim_key: bool,
        reject_zero_crc: bool,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        fields: Vec<NativeProjectionField>,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_tables(&module, &tables)?;
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_module: None,
            tables,
            tables_type,
            table_type,
            handle_type,
            row_alias,
            data_type,
            entries_field,
            index_field,
            key_field,
            crc_field,
            key_column,
            key_getter,
            key_wrapper_type: None,
            key_storage_transform: NativeCrcKeyStorageTransform::default(),
            hash_policy: NativeCrcHashPolicy::default(),
            skip_empty_key,
            trim_key,
            reject_zero_crc,
            duplicate_key_policy,
            source_row_field: None,
            source_row_method: None,
            source_row_by_crc_method: None,
            source_handle_field: None,
            source_handle_method: None,
            row_filters: Vec::new(),
            fields,
            schema_validation_fields: None,
            table_indexes: Vec::new(),
            methods,
            field_lookup_methods: Vec::new(),
            store_key_text: true,
            ids_method: None,
            crc_ids_method: None,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    #[must_use]
    pub const fn with_hash_policy(mut self, policy: NativeCrcHashPolicy) -> Self {
        self.hash_policy = policy;
        self
    }

    #[must_use]
    pub fn with_table_module(mut self, module: RustIdentifier) -> Self {
        self.table_module = Some(module);
        self
    }

    #[must_use]
    pub fn table_module(&self) -> &RustIdentifier {
        self.table_module.as_ref().unwrap_or(&self.module)
    }

    projection_common_methods!();

    manager_common_methods! {
        source_row_by_crc_method => with_source_row_by_crc_method,
        ids_method => with_ids_method,
        crc_ids_method => with_crc_ids_method
    }

    #[must_use]
    pub fn with_key_wrapper_type(mut self, key_wrapper_type: RustIdentifier) -> Self {
        self.key_wrapper_type = Some(key_wrapper_type);
        self
    }

    #[must_use]
    pub const fn with_key_storage_transform(
        mut self,
        transform: NativeCrcKeyStorageTransform,
    ) -> Self {
        self.key_storage_transform = transform;
        self
    }

    #[must_use]
    pub fn with_row_filter(mut self, filter: NativeCrcProjectionRowFilter) -> Self {
        self.row_filters.push(filter);
        self
    }

    #[must_use]
    pub fn with_table_index(mut self, index: NativeTableFamilyCrcTableIndex) -> Self {
        self.table_indexes.push(index);
        self
    }

    #[must_use]
    pub fn with_field_lookup_method(
        mut self,
        method: NativeCrcProjectionFieldLookupMethod,
    ) -> Self {
        self.field_lookup_methods.push(method);
        self
    }

    #[must_use]
    pub fn with_schema_validation_fields(mut self, fields: NativeSchemaProjectionFields) -> Self {
        self.schema_validation_fields = Some(fields);
        self
    }

    #[must_use]
    pub const fn without_key_text(mut self) -> Self {
        self.store_key_text = false;
        self
    }

    #[must_use]
    pub fn with_source_handle_field(mut self, field: RustIdentifier) -> Self {
        self.source_handle_field = Some(field);
        self
    }

    #[must_use]
    pub fn with_source_handle_method(
        mut self,
        name: RustIdentifier,
        parameter: RustIdentifier,
    ) -> Self {
        self.source_handle_method = Some(NativeSourceHandleMethod::new(name, parameter));
        self
    }

    simple_accessors! {
        module: RustIdentifier,
        tables_type: RustIdentifier,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier
    }

    bool_accessors! {
        skip_empty_key,
        trim_key,
        reject_zero_crc
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }

    #[must_use]
    pub const fn hash_policy(&self) -> NativeCrcHashPolicy {
        self.hash_policy
    }

    #[must_use]
    pub const fn key_wrapper_type(&self) -> Option<&RustIdentifier> {
        self.key_wrapper_type.as_ref()
    }

    #[must_use]
    pub const fn key_storage_transform(&self) -> NativeCrcKeyStorageTransform {
        self.key_storage_transform
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub const fn source_handle_field(&self) -> Option<&RustIdentifier> {
        self.source_handle_field.as_ref()
    }

    #[must_use]
    pub const fn source_handle_method(&self) -> Option<&NativeSourceHandleMethod> {
        self.source_handle_method.as_ref()
    }

    #[must_use]
    pub fn row_filters(&self) -> &[NativeCrcProjectionRowFilter] {
        &self.row_filters
    }

    #[must_use]
    pub fn fields(&self) -> &[NativeProjectionField] {
        &self.fields
    }

    #[must_use]
    pub const fn schema_validation_fields(&self) -> Option<&NativeSchemaProjectionFields> {
        self.schema_validation_fields.as_ref()
    }

    #[must_use]
    pub fn table_indexes(&self) -> &[NativeTableFamilyCrcTableIndex] {
        &self.table_indexes
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }

    #[must_use]
    pub fn field_lookup_methods(&self) -> &[NativeCrcProjectionFieldLookupMethod] {
        &self.field_lookup_methods
    }

    #[must_use]
    pub const fn store_key_text(&self) -> bool {
        self.store_key_text
    }
}

impl NativeOneTableNumericKeyProjectionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        key_type: NativeNumericKeyType,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        fields: Vec<NativeProjectionField>,
        methods: Vec<NativeNumericLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_name,
            row_type_name,
            data_type,
            entries_field,
            index_field,
            key_field,
            key_column,
            key_getter,
            key_type,
            duplicate_key_policy,
            source_row_field: None,
            source_row_method: None,
            fields,
            methods,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    projection_common_methods!();

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier
    }

    #[must_use]
    pub const fn key_type(&self) -> NativeNumericKeyType {
        self.key_type
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub fn fields(&self) -> &[NativeProjectionField] {
        &self.fields
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeNumericLookupMethod] {
        &self.methods
    }
}

impl NativeTableFamilyNumericKeyProjectionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
        tables_type: RustIdentifier,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        key_type: NativeNumericKeyType,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        fields: Vec<NativeProjectionField>,
        methods: Vec<NativeNumericLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_tables(&module, &tables)?;
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            tables,
            tables_type,
            table_type,
            handle_type,
            row_alias,
            data_type,
            entries_field,
            index_field,
            key_field,
            key_column,
            key_getter,
            key_type,
            duplicate_key_policy,
            source_handle_field: None,
            source_handle_method: None,
            fields,
            methods,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    #[must_use]
    pub fn with_source_handle_field(mut self, field: RustIdentifier) -> Self {
        self.source_handle_field = Some(field);
        self
    }

    #[must_use]
    pub fn with_source_handle_method(
        mut self,
        name: RustIdentifier,
        parameter: RustIdentifier,
    ) -> Self {
        self.source_handle_method = Some(NativeSourceHandleMethod::new(name, parameter));
        self
    }

    manager_common_methods! {
        rows_method => with_rows_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    simple_accessors! {
        module: RustIdentifier,
        tables_type: RustIdentifier,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }

    #[must_use]
    pub const fn key_type(&self) -> NativeNumericKeyType {
        self.key_type
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub const fn source_handle_field(&self) -> Option<&RustIdentifier> {
        self.source_handle_field.as_ref()
    }

    #[must_use]
    pub const fn source_handle_method(&self) -> Option<&NativeSourceHandleMethod> {
        self.source_handle_method.as_ref()
    }

    #[must_use]
    pub fn fields(&self) -> &[NativeProjectionField] {
        &self.fields
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeNumericLookupMethod] {
        &self.methods
    }
}

impl NativeOneTableEnumKeyProjectionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        key_type: RustTypePath,
        invalid_key_variants: Vec<RustIdentifier>,
        skip_empty_key: bool,
        trim_key: bool,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        fields: Vec<NativeProjectionField>,
        methods: Vec<NativeEnumLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_name,
            row_type_name,
            data_type,
            entries_field,
            index_field,
            key_field,
            key_column,
            key_getter,
            key_type,
            key_type_alias: None,
            table_view_alias: None,
            expose_table_constructor: false,
            invalid_key_variants,
            skip_empty_key,
            trim_key,
            duplicate_key_policy,
            source_row_field: None,
            source_row_method: None,
            secondary_crc_index: None,
            fields,
            methods,
            ids_method: None,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    projection_common_methods!();

    manager_common_methods! {
        ids_method => with_ids_method
    }

    #[must_use]
    pub fn with_key_type_alias(mut self, alias: RustIdentifier) -> Self {
        self.key_type_alias = Some(alias);
        self
    }

    #[must_use]
    pub fn with_table_view_alias(mut self, alias: RustIdentifier) -> Self {
        self.table_view_alias = Some(alias);
        self
    }

    #[must_use]
    pub const fn with_exposed_table_constructor(mut self) -> Self {
        self.expose_table_constructor = true;
        self
    }

    #[must_use]
    pub fn with_secondary_crc_index(mut self, index: NativeEnumProjectionCrcIndex) -> Self {
        self.secondary_crc_index = Some(index);
        self
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        key_type: RustTypePath
    }

    bool_accessors! {
        skip_empty_key,
        trim_key,
        expose_table_constructor
    }

    #[must_use]
    pub const fn key_type_alias(&self) -> Option<&RustIdentifier> {
        self.key_type_alias.as_ref()
    }

    #[must_use]
    pub const fn table_view_alias(&self) -> Option<&RustIdentifier> {
        self.table_view_alias.as_ref()
    }

    #[must_use]
    pub fn invalid_key_variants(&self) -> &[RustIdentifier] {
        &self.invalid_key_variants
    }

    #[must_use]
    pub const fn secondary_crc_index(&self) -> Option<&NativeEnumProjectionCrcIndex> {
        self.secondary_crc_index.as_ref()
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub fn fields(&self) -> &[NativeProjectionField] {
        &self.fields
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeEnumLookupMethod] {
        &self.methods
    }
}

impl NativeEnumProjectionCrcIndex {
    #[must_use]
    pub fn new(
        index_field: RustIdentifier,
        crc_field: RustIdentifier,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Self {
        Self {
            index_field,
            crc_field,
            methods,
        }
    }

    simple_accessors! {
        index_field: RustIdentifier,
        crc_field: RustIdentifier
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }
}

impl NativeOneTableStringKeyProjectionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        map_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        skip_empty_key: bool,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        fields: Vec<NativeProjectionField>,
        methods: Vec<NativeStringLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_name,
            row_type_name,
            data_type,
            map_field,
            key_field,
            key_column,
            key_getter,
            skip_empty_key,
            duplicate_key_policy,
            fields,
            methods,
            len_method: None,
            is_empty_method: None,
        })
    }

    manager_common_methods! {
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        map_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier
    }

    #[must_use]
    pub const fn skip_empty_key(&self) -> bool {
        self.skip_empty_key
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub fn fields(&self) -> &[NativeProjectionField] {
        &self.fields
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeStringLookupMethod] {
        &self.methods
    }
}

impl NativeOneTableRowProjectionManager {
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        fields: Vec<NativeProjectionField>,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            data_type,
            entries_field,
            source_row_field: None,
            source_row_method: None,
            source_row_for_method: None,
            fields,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        }
    }

    manager_common_methods! {
        source_row_method => with_source_row_method,
        source_row_for_method => with_source_row_for_method,
        rows_method => with_rows_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    #[must_use]
    pub fn with_source_row_field(mut self, field: RustIdentifier) -> Self {
        self.source_row_field = Some(field);
        self
    }

    #[must_use]
    pub const fn source_row_field(&self) -> Option<&RustIdentifier> {
        self.source_row_field.as_ref()
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier
    }

    #[must_use]
    pub fn fields(&self) -> &[NativeProjectionField] {
        &self.fields
    }
}

impl NativeOneTablePvpBalanceManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        target_column: GameDataColumnName,
        target_getter: RustIdentifier,
        category_column: GameDataColumnName,
        category_getter: RustIdentifier,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_name,
            row_type_name,
            target_column,
            target_getter,
            category_column,
            category_getter,
            methods,
            balances_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    manager_common_methods! {
        balances_method => with_balances_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        target_column: GameDataColumnName,
        target_getter: RustIdentifier,
        category_column: GameDataColumnName,
        category_getter: RustIdentifier
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }
}

impl NativeOneTableCampSkinManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        settings_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        item_id_getter: RustIdentifier,
        required_achievement_id_getter: RustIdentifier,
        is_entitlement_getter: RustIdentifier,
        is_enabled_getter: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        ids_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            table_field,
            row_alias,
            data_type,
            settings_type,
            entries_field,
            index_field,
            key_column,
            key_getter,
            item_id_getter,
            required_achievement_id_getter,
            is_entitlement_getter,
            is_enabled_getter,
            lookup_method,
            lookup_by_key_method,
            ids_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        settings_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        item_id_getter: RustIdentifier,
        required_achievement_id_getter: RustIdentifier,
        is_entitlement_getter: RustIdentifier,
        is_enabled_getter: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        ids_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableDyeColorManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        index_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        entitlement_indexes_field: RustIdentifier,
        index_column: GameDataColumnName,
        index_getter: RustIdentifier,
        name_getter: RustIdentifier,
        color_getter: RustIdentifier,
        category_getter: RustIdentifier,
        is_entitlement_getter: RustIdentifier,
        color_amount_getter: RustIdentifier,
        color_override_getter: RustIdentifier,
        spec_color_getter: RustIdentifier,
        spec_amount_getter: RustIdentifier,
        mask_gloss_shift_getter: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_from_index_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        rows_method: RustIdentifier,
        entitlement_indexes_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            table_field,
            row_alias,
            data_type,
            index_type,
            entries_field,
            index_field,
            entitlement_indexes_field,
            index_column,
            index_getter,
            name_getter,
            color_getter,
            category_getter,
            is_entitlement_getter,
            color_amount_getter,
            color_override_getter,
            spec_color_getter,
            spec_amount_getter,
            mask_gloss_shift_getter,
            lookup_method,
            lookup_from_index_method,
            lookup_by_key_method,
            rows_method,
            entitlement_indexes_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        index_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        entitlement_indexes_field: RustIdentifier,
        index_column: GameDataColumnName,
        index_getter: RustIdentifier,
        name_getter: RustIdentifier,
        color_getter: RustIdentifier,
        category_getter: RustIdentifier,
        is_entitlement_getter: RustIdentifier,
        color_amount_getter: RustIdentifier,
        color_override_getter: RustIdentifier,
        spec_color_getter: RustIdentifier,
        spec_amount_getter: RustIdentifier,
        mask_gloss_shift_getter: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_from_index_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        rows_method: RustIdentifier,
        entitlement_indexes_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableEmoteManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        settings_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        status_effect_lookup_by_crc_method: RustIdentifier,
        status_effect_lookup_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            table_field,
            row_alias,
            data_type,
            settings_type,
            cache_type,
            cache_field,
            lookup_from_id_method,
            lookup_method,
            lookup_by_key_method,
            status_effect_lookup_by_crc_method,
            status_effect_lookup_method,
            rows_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        settings_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        status_effect_lookup_by_crc_method: RustIdentifier,
        status_effect_lookup_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableExperienceManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        named_value_type: RustIdentifier,
        gear_score_bonus_type: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        gear_score_lookup_method: RustIdentifier,
        level_for_xp_method: RustIdentifier,
        max_level_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            table_field,
            row_alias,
            data_type,
            cache_type,
            cache_field,
            named_value_type,
            gear_score_bonus_type,
            lookup_from_id_method,
            lookup_method,
            gear_score_lookup_method,
            level_for_xp_method,
            max_level_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        named_value_type: RustIdentifier,
        gear_score_bonus_type: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        gear_score_lookup_method: RustIdentifier,
        level_for_xp_method: RustIdentifier,
        max_level_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableStoreCategoryManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        tab_type: RustIdentifier,
        invalid_product_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        num_categories_method: RustIdentifier,
        rows_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_name_method: RustIdentifier,
        lookup_by_index_method: RustIdentifier,
        product_type_lookup_method: RustIdentifier,
        invalid_product_types_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            table_field,
            row_alias,
            data_type,
            tab_type,
            invalid_product_type,
            cache_type,
            cache_field,
            num_categories_method,
            rows_method,
            lookup_method,
            lookup_by_name_method,
            lookup_by_index_method,
            product_type_lookup_method,
            invalid_product_types_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        tab_type: RustIdentifier,
        invalid_product_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        num_categories_method: RustIdentifier,
        rows_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_name_method: RustIdentifier,
        lookup_by_index_method: RustIdentifier,
        product_type_lookup_method: RustIdentifier,
        invalid_product_types_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableStoreProductManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        invalid_product_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_tag_method: RustIdentifier,
        rows_method: RustIdentifier,
        invalid_product_types_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            table_field,
            row_alias,
            data_type,
            invalid_product_type,
            cache_type,
            cache_field,
            lookup_method,
            lookup_by_tag_method,
            rows_method,
            invalid_product_types_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        invalid_product_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_tag_method: RustIdentifier,
        rows_method: RustIdentifier,
        invalid_product_types_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableRewardTrackItemManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        payload_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            table_field,
            row_alias,
            data_type,
            payload_type,
            cache_type,
            cache_field,
            lookup_from_id_method,
            lookup_method,
            lookup_by_key_method,
            rows_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        payload_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativePostSkillCapProgressionDataManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        level_rewards_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            table_field,
            row_alias,
            data_type,
            level_rewards_type,
            cache_type,
            cache_field,
            lookup_method,
            lookup_from_id_method,
            rows_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        level_rewards_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeWhisperDataManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        whisper_table_name: GameDataTableName,
        whisper_row_type_name: GameDataRowTypeName,
        whisper_table_field: RustIdentifier,
        whisper_row_alias: RustIdentifier,
        vfx_table_name: GameDataTableName,
        vfx_row_type_name: GameDataRowTypeName,
        vfx_table_field: RustIdentifier,
        vfx_row_alias: RustIdentifier,
        data_type: RustIdentifier,
        vfx_data_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        rows_method: RustIdentifier,
        ids_method: RustIdentifier,
        vfx_lookup_from_id_method: RustIdentifier,
        vfx_lookup_method: RustIdentifier,
        vfx_for_method: RustIdentifier,
        vfx_rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            whisper_table_name,
            whisper_row_type_name,
            whisper_table_field,
            whisper_row_alias,
            vfx_table_name,
            vfx_row_type_name,
            vfx_table_field,
            vfx_row_alias,
            data_type,
            vfx_data_type,
            cache_type,
            cache_field,
            lookup_from_id_method,
            lookup_method,
            lookup_by_key_method,
            rows_method,
            ids_method,
            vfx_lookup_from_id_method,
            vfx_lookup_method,
            vfx_for_method,
            vfx_rows_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        whisper_table_name: GameDataTableName,
        whisper_row_type_name: GameDataRowTypeName,
        whisper_table_field: RustIdentifier,
        whisper_row_alias: RustIdentifier,
        vfx_table_name: GameDataTableName,
        vfx_row_type_name: GameDataRowTypeName,
        vfx_table_field: RustIdentifier,
        vfx_row_alias: RustIdentifier,
        data_type: RustIdentifier,
        vfx_data_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        rows_method: RustIdentifier,
        ids_method: RustIdentifier,
        vfx_lookup_from_id_method: RustIdentifier,
        vfx_lookup_method: RustIdentifier,
        vfx_for_method: RustIdentifier,
        vfx_rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableWorldEventRuleManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        crc_filter_type: RustIdentifier,
        zone_filter_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_crc_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            table_field,
            row_alias,
            data_type,
            crc_filter_type,
            zone_filter_type,
            cache_type,
            cache_field,
            lookup_method,
            lookup_by_crc_method,
            rows_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        table_field: RustIdentifier,
        row_alias: RustIdentifier,
        data_type: RustIdentifier,
        crc_filter_type: RustIdentifier,
        zone_filter_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_crc_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeQuickCourseDataManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        quick_course_table_name: GameDataTableName,
        quick_course_row_type_name: GameDataRowTypeName,
        quick_course_table_field: RustIdentifier,
        quick_course_row_alias: RustIdentifier,
        node_type_table_name: GameDataTableName,
        node_type_row_type_name: GameDataRowTypeName,
        node_type_table_field: RustIdentifier,
        node_type_row_alias: RustIdentifier,
        data_type: RustIdentifier,
        node_type_data_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        quick_course_lookup_method: RustIdentifier,
        quick_course_lookup_by_crc_method: RustIdentifier,
        quick_courses_method: RustIdentifier,
        quick_course_ids_method: RustIdentifier,
        first_quick_course_id_method: RustIdentifier,
        node_type_lookup_method: RustIdentifier,
        node_type_lookup_by_crc_method: RustIdentifier,
        node_types_method: RustIdentifier,
        node_type_ids_method: RustIdentifier,
        first_node_type_id_method: RustIdentifier,
        quick_course_len_method: RustIdentifier,
        node_type_len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            quick_course_table_name,
            quick_course_row_type_name,
            quick_course_table_field,
            quick_course_row_alias,
            node_type_table_name,
            node_type_row_type_name,
            node_type_table_field,
            node_type_row_alias,
            data_type,
            node_type_data_type,
            cache_type,
            cache_field,
            quick_course_lookup_method,
            quick_course_lookup_by_crc_method,
            quick_courses_method,
            quick_course_ids_method,
            first_quick_course_id_method,
            node_type_lookup_method,
            node_type_lookup_by_crc_method,
            node_types_method,
            node_type_ids_method,
            first_node_type_id_method,
            quick_course_len_method,
            node_type_len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        quick_course_table_name: GameDataTableName,
        quick_course_row_type_name: GameDataRowTypeName,
        quick_course_table_field: RustIdentifier,
        quick_course_row_alias: RustIdentifier,
        node_type_table_name: GameDataTableName,
        node_type_row_type_name: GameDataRowTypeName,
        node_type_table_field: RustIdentifier,
        node_type_row_alias: RustIdentifier,
        data_type: RustIdentifier,
        node_type_data_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        quick_course_lookup_method: RustIdentifier,
        quick_course_lookup_by_crc_method: RustIdentifier,
        quick_courses_method: RustIdentifier,
        quick_course_ids_method: RustIdentifier,
        first_quick_course_id_method: RustIdentifier,
        node_type_lookup_method: RustIdentifier,
        node_type_lookup_by_crc_method: RustIdentifier,
        node_types_method: RustIdentifier,
        node_type_ids_method: RustIdentifier,
        first_node_type_id_method: RustIdentifier,
        quick_course_len_method: RustIdentifier,
        node_type_len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeRotationalQueueDataManager {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        module: RustIdentifier,
        queue_table_name: GameDataTableName,
        queue_row_type_name: GameDataRowTypeName,
        queue_table_field: RustIdentifier,
        queue_row_alias: RustIdentifier,
        game_mode_table_name: GameDataTableName,
        game_mode_row_type_name: GameDataRowTypeName,
        game_mode_table_field: RustIdentifier,
        data_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            queue_table_name,
            queue_row_type_name,
            queue_table_field,
            queue_row_alias,
            game_mode_table_name,
            game_mode_row_type_name,
            game_mode_table_field,
            data_type,
            cache_type,
            cache_field,
            lookup_method,
            lookup_from_id_method,
            rows_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        queue_table_name: GameDataTableName,
        queue_row_type_name: GameDataRowTypeName,
        queue_table_field: RustIdentifier,
        queue_row_alias: RustIdentifier,
        game_mode_table_name: GameDataTableName,
        game_mode_row_type_name: GameDataRowTypeName,
        game_mode_table_field: RustIdentifier,
        data_type: RustIdentifier,
        cache_type: RustIdentifier,
        cache_field: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableCostumeChangeManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        slot_type: RustIdentifier,
        override_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_row_index_field: RustIdentifier,
        key_field: RustIdentifier,
        key_text_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        mesh_field: RustIdentifier,
        mesh_column: GameDataColumnName,
        mesh_getter: RustIdentifier,
        matches_skeleton_field: RustIdentifier,
        matches_skeleton_column: GameDataColumnName,
        matches_skeleton_getter: RustIdentifier,
        z_offset_field: RustIdentifier,
        z_offset_column: GameDataColumnName,
        z_offset_getter: RustIdentifier,
        audio_overrides_field: RustIdentifier,
        source_row_field: RustIdentifier,
        source_row_method: RustIdentifier,
        slots: Vec<NativeCostumeAudioSlot>,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        audio_override_from_id_method: RustIdentifier,
        audio_override_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Result<Self, NativeManagerShapeError> {
        if slots.is_empty() {
            return Err(NativeManagerShapeError::MissingLookupMethods { module });
        }
        Ok(Self {
            module,
            table_name,
            row_type_name,
            data_type,
            slot_type,
            override_type,
            entries_field,
            index_field,
            source_row_index_field,
            key_field,
            key_text_field,
            key_column,
            key_getter,
            mesh_field,
            mesh_column,
            mesh_getter,
            matches_skeleton_field,
            matches_skeleton_column,
            matches_skeleton_getter,
            z_offset_field,
            z_offset_column,
            z_offset_getter,
            audio_overrides_field,
            source_row_field,
            source_row_method,
            slots,
            lookup_from_id_method,
            lookup_method,
            lookup_by_key_method,
            audio_override_from_id_method,
            audio_override_method,
            rows_method,
            len_method,
            is_empty_method,
        })
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        slot_type: RustIdentifier,
        override_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_row_index_field: RustIdentifier,
        key_field: RustIdentifier,
        key_text_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        mesh_field: RustIdentifier,
        mesh_column: GameDataColumnName,
        mesh_getter: RustIdentifier,
        matches_skeleton_field: RustIdentifier,
        matches_skeleton_column: GameDataColumnName,
        matches_skeleton_getter: RustIdentifier,
        z_offset_field: RustIdentifier,
        z_offset_column: GameDataColumnName,
        z_offset_getter: RustIdentifier,
        audio_overrides_field: RustIdentifier,
        source_row_field: RustIdentifier,
        source_row_method: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        audio_override_from_id_method: RustIdentifier,
        audio_override_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }

    #[must_use]
    pub fn slots(&self) -> &[NativeCostumeAudioSlot] {
        &self.slots
    }
}

impl NativeCostumeAudioSlot {
    #[must_use]
    pub const fn new(
        variant: RustIdentifier,
        display: RustIdentifier,
        left_column: GameDataColumnName,
        left_getter: RustIdentifier,
        right_column: GameDataColumnName,
        right_getter: RustIdentifier,
    ) -> Self {
        Self {
            variant,
            display,
            left_column,
            left_getter,
            right_column,
            right_getter,
        }
    }

    simple_accessors! {
        variant: RustIdentifier,
        display: RustIdentifier,
        left_column: GameDataColumnName,
        left_getter: RustIdentifier,
        right_column: GameDataColumnName,
        right_getter: RustIdentifier
    }
}

impl NativeOneTableCrestPartManager {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        kind_type: RustIdentifier,
        faction_type: RustIdentifier,
        parse_error_type: RustIdentifier,
        indexes_type: RustIdentifier,
        entries_field: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            data_type,
            kind_type,
            faction_type,
            parse_error_type,
            indexes_type,
            entries_field,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        kind_type: RustIdentifier,
        faction_type: RustIdentifier,
        parse_error_type: RustIdentifier,
        indexes_type: RustIdentifier,
        entries_field: RustIdentifier
    }
}

impl NativeOneTableDungeonTileManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_row_index_field: RustIdentifier,
        variant_index_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        feature_key_field: RustIdentifier,
        feature_crc_field: RustIdentifier,
        feature_column: GameDataColumnName,
        feature_getter: RustIdentifier,
        connections_field: RustIdentifier,
        connections_column: GameDataColumnName,
        connections_getter: RustIdentifier,
        rotations_field: RustIdentifier,
        rotations_column: GameDataColumnName,
        rotations_getter: RustIdentifier,
        tile_size_field: RustIdentifier,
        tile_size_column: GameDataColumnName,
        tile_size_getter: RustIdentifier,
        weight_field: RustIdentifier,
        weight_column: GameDataColumnName,
        weight_getter: RustIdentifier,
        variation_asset_paths_field: RustIdentifier,
        variation_asset_paths_column: GameDataColumnName,
        variation_asset_paths_getter: RustIdentifier,
        supported_room_types_field: RustIdentifier,
        supported_room_types_column: GameDataColumnName,
        supported_room_types_getter: RustIdentifier,
        source_row_field: RustIdentifier,
        source_row_method: RustIdentifier,
        methods: Vec<NativeCrcIndexLookupMethod>,
        tile_variants_method: RustIdentifier,
        tile_variant_row_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_name,
            row_type_name,
            data_type,
            entries_field,
            index_field,
            source_row_index_field,
            variant_index_field,
            key_field,
            crc_field,
            key_column,
            key_getter,
            feature_key_field,
            feature_crc_field,
            feature_column,
            feature_getter,
            connections_field,
            connections_column,
            connections_getter,
            rotations_field,
            rotations_column,
            rotations_getter,
            tile_size_field,
            tile_size_column,
            tile_size_getter,
            weight_field,
            weight_column,
            weight_getter,
            variation_asset_paths_field,
            variation_asset_paths_column,
            variation_asset_paths_getter,
            supported_room_types_field,
            supported_room_types_column,
            supported_room_types_getter,
            source_row_field,
            source_row_method,
            methods,
            tile_variants_method,
            tile_variant_row_method,
            rows_method,
            len_method,
            is_empty_method,
        })
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_row_index_field: RustIdentifier,
        variant_index_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        feature_key_field: RustIdentifier,
        feature_crc_field: RustIdentifier,
        feature_column: GameDataColumnName,
        feature_getter: RustIdentifier,
        connections_field: RustIdentifier,
        connections_column: GameDataColumnName,
        connections_getter: RustIdentifier,
        rotations_field: RustIdentifier,
        rotations_column: GameDataColumnName,
        rotations_getter: RustIdentifier,
        tile_size_field: RustIdentifier,
        tile_size_column: GameDataColumnName,
        tile_size_getter: RustIdentifier,
        weight_field: RustIdentifier,
        weight_column: GameDataColumnName,
        weight_getter: RustIdentifier,
        variation_asset_paths_field: RustIdentifier,
        variation_asset_paths_column: GameDataColumnName,
        variation_asset_paths_getter: RustIdentifier,
        supported_room_types_field: RustIdentifier,
        supported_room_types_column: GameDataColumnName,
        supported_room_types_getter: RustIdentifier,
        source_row_field: RustIdentifier,
        source_row_method: RustIdentifier,
        tile_variants_method: RustIdentifier,
        tile_variant_row_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }
}

impl NativeOneTableLevelDisparityManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        range_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_row_index_field: RustIdentifier,
        range_field: RustIdentifier,
        max_capped_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        source_row_field: RustIdentifier,
        source_row_method: RustIdentifier,
        capped_value_source_field: RustIdentifier,
        fields: Vec<NativeProjectionField>,
        lookup_method: RustIdentifier,
        levels_method: RustIdentifier,
        clamped_levels_method: RustIdentifier,
        capped_levels_method: RustIdentifier,
        capped_clamped_levels_method: RustIdentifier,
        loaded_range_method: RustIdentifier,
        clamped_key_method: RustIdentifier,
        max_capped_value_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            data_type,
            range_type,
            entries_field,
            index_field,
            source_row_index_field,
            range_field,
            max_capped_field,
            key_field,
            key_column,
            key_getter,
            source_row_field,
            source_row_method,
            capped_value_source_field,
            fields,
            lookup_method,
            levels_method,
            clamped_levels_method,
            capped_levels_method,
            capped_clamped_levels_method,
            loaded_range_method,
            clamped_key_method,
            max_capped_value_method,
            rows_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        range_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_row_index_field: RustIdentifier,
        range_field: RustIdentifier,
        max_capped_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        source_row_field: RustIdentifier,
        source_row_method: RustIdentifier,
        capped_value_source_field: RustIdentifier,
        lookup_method: RustIdentifier,
        levels_method: RustIdentifier,
        clamped_levels_method: RustIdentifier,
        capped_levels_method: RustIdentifier,
        capped_clamped_levels_method: RustIdentifier,
        loaded_range_method: RustIdentifier,
        clamped_key_method: RustIdentifier,
        max_capped_value_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }

    #[must_use]
    pub fn fields(&self) -> &[NativeProjectionField] {
        &self.fields
    }
}

impl NativeOneTableEncumbranceManager {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        load_state_type: RustIdentifier,
        load_values_type: RustIdentifier,
        indexes_type: RustIdentifier,
        entries_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            data_type,
            load_state_type,
            load_values_type,
            indexes_type,
            entries_field,
            key_column,
            key_getter,
            lookup_from_id_method,
            lookup_method,
            lookup_by_key_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        load_state_type: RustIdentifier,
        load_values_type: RustIdentifier,
        indexes_type: RustIdentifier,
        entries_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableDifficultyScalingManager {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        affected_creature_types_type: RustIdentifier,
        health_modifier_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_row_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            data_type,
            affected_creature_types_type,
            health_modifier_type,
            entries_field,
            index_field,
            source_row_field,
            key_field,
            key_column,
            key_getter,
            lookup_from_id_method,
            lookup_method,
            lookup_by_key_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        affected_creature_types_type: RustIdentifier,
        health_modifier_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_row_field: RustIdentifier,
        key_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableDarknessManager {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        threshold_type: RustIdentifier,
        level_type: RustIdentifier,
        activation_spec_type: RustIdentifier,
        group_spec_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_index_field: RustIdentifier,
        source_row_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        lookup_crc_method: RustIdentifier,
        lookup_method: RustIdentifier,
        source_row_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            data_type,
            threshold_type,
            level_type,
            activation_spec_type,
            group_spec_type,
            entries_field,
            index_field,
            source_index_field,
            source_row_field,
            key_field,
            crc_field,
            key_column,
            key_getter,
            lookup_crc_method,
            lookup_method,
            source_row_method,
            rows_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        threshold_type: RustIdentifier,
        level_type: RustIdentifier,
        activation_spec_type: RustIdentifier,
        group_spec_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        source_index_field: RustIdentifier,
        source_row_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        lookup_crc_method: RustIdentifier,
        lookup_method: RustIdentifier,
        source_row_method: RustIdentifier,
        rows_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeOneTableParticleDataManager {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        group_type: RustIdentifier,
        lookup_type: RustIdentifier,
        indexes_type: RustIdentifier,
        entries_field: RustIdentifier,
        local_player_factor_field: RustIdentifier,
        max_total_number_emitters_field: RustIdentifier,
        max_total_group_number_emitters_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        group_column: GameDataColumnName,
        group_getter: RustIdentifier,
        max_number_column: GameDataColumnName,
        max_number_getter: RustIdentifier,
        priority_column: GameDataColumnName,
        priority_getter: RustIdentifier,
        constants_column: GameDataColumnName,
        constants_getter: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        local_player_factor_method: RustIdentifier,
        max_total_number_emitters_method: RustIdentifier,
        max_total_group_number_emitters_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier,
    ) -> Self {
        Self {
            module,
            table_name,
            row_type_name,
            data_type,
            group_type,
            lookup_type,
            indexes_type,
            entries_field,
            local_player_factor_field,
            max_total_number_emitters_field,
            max_total_group_number_emitters_field,
            key_column,
            key_getter,
            group_column,
            group_getter,
            max_number_column,
            max_number_getter,
            priority_column,
            priority_getter,
            constants_column,
            constants_getter,
            lookup_from_id_method,
            lookup_method,
            lookup_by_key_method,
            local_player_factor_method,
            max_total_number_emitters_method,
            max_total_group_number_emitters_method,
            len_method,
            is_empty_method,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        data_type: RustIdentifier,
        group_type: RustIdentifier,
        lookup_type: RustIdentifier,
        indexes_type: RustIdentifier,
        entries_field: RustIdentifier,
        local_player_factor_field: RustIdentifier,
        max_total_number_emitters_field: RustIdentifier,
        max_total_group_number_emitters_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        group_column: GameDataColumnName,
        group_getter: RustIdentifier,
        max_number_column: GameDataColumnName,
        max_number_getter: RustIdentifier,
        priority_column: GameDataColumnName,
        priority_getter: RustIdentifier,
        constants_column: GameDataColumnName,
        constants_getter: RustIdentifier,
        lookup_from_id_method: RustIdentifier,
        lookup_method: RustIdentifier,
        lookup_by_key_method: RustIdentifier,
        local_player_factor_method: RustIdentifier,
        max_total_number_emitters_method: RustIdentifier,
        max_total_group_number_emitters_method: RustIdentifier,
        len_method: RustIdentifier,
        is_empty_method: RustIdentifier
    }
}

impl NativeProductAssetResourceManager {
    pub fn new(
        manager_type: RustTypePath,
        constructor: RustIdentifier,
        products: Vec<NativeProductAssetResource>,
    ) -> Result<Self, NativeManagerShapeError> {
        if products.is_empty() {
            return Err(NativeManagerShapeError::MissingProducts {
                module: constructor,
            });
        }
        Ok(Self {
            manager_type,
            constructor,
            products,
        })
    }

    simple_accessors! {
        manager_type: RustTypePath,
        constructor: RustIdentifier
    }

    #[must_use]
    pub fn products(&self) -> &[NativeProductAssetResource] {
        &self.products
    }
}

impl NativeProductAssetResource {
    #[must_use]
    pub const fn new(
        product_type: RustTypePath,
        value_type: RustTypePath,
        handle_getter: RustIdentifier,
        asset_getter: RustIdentifier,
        manager_getter: RustIdentifier,
    ) -> Self {
        Self {
            product_type,
            value_type,
            handle_getter,
            asset_getter,
            manager_getter,
        }
    }

    simple_accessors! {
        product_type: RustTypePath,
        value_type: RustTypePath,
        handle_getter: RustIdentifier,
        asset_getter: RustIdentifier,
        manager_getter: RustIdentifier
    }
}

impl NativeRecipeDataManager {
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        data_type: RustIdentifier,
        product: NativeProductAssetResource,
    ) -> Result<Self, NativeManagerShapeError> {
        if tables.is_empty() {
            return Err(NativeManagerShapeError::MissingTables { module });
        }
        Ok(Self {
            module,
            tables,
            table_type,
            handle_type,
            data_type,
            product,
        })
    }

    simple_accessors! {
        module: RustIdentifier,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        data_type: RustIdentifier,
        product: NativeProductAssetResource
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }
}

impl NativeItemDataManager {
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        data_type: RustIdentifier,
    ) -> Result<Self, NativeManagerShapeError> {
        if tables.is_empty() {
            return Err(NativeManagerShapeError::MissingTables { module });
        }
        Ok(Self {
            module,
            tables,
            table_type,
            handle_type,
            data_type,
        })
    }

    simple_accessors! {
        module: RustIdentifier,
        table_type: RustIdentifier,
        handle_type: RustIdentifier,
        data_type: RustIdentifier
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }
}

impl NativeItemConversionDataManager {
    pub fn new(
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        handle_type: RustIdentifier,
        data_type: RustIdentifier,
    ) -> Result<Self, NativeManagerShapeError> {
        Ok(Self {
            module,
            table_name,
            row_type_name,
            handle_type,
            data_type,
        })
    }

    simple_accessors! {
        module: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        handle_type: RustIdentifier,
        data_type: RustIdentifier
    }
}

impl NativeCharacterAttributeDataManager {
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
    ) -> Result<Self, NativeManagerShapeError> {
        if tables.is_empty() {
            return Err(NativeManagerShapeError::MissingTables { module });
        }
        Ok(Self { module, tables })
    }

    simple_accessors! {
        module: RustIdentifier
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }
}

impl NativeDamageDataManager {
    pub fn new(
        module: RustIdentifier,
        damage_tables: Vec<NativeTableFamilyTable>,
    ) -> Result<Self, NativeManagerShapeError> {
        if damage_tables.is_empty() {
            return Err(NativeManagerShapeError::MissingTables { module });
        }
        Ok(Self {
            module,
            damage_tables,
        })
    }

    simple_accessors! {
        module: RustIdentifier
    }

    #[must_use]
    pub fn damage_tables(&self) -> &[NativeTableFamilyTable] {
        &self.damage_tables
    }
}

impl NativeVitalsDataManager {
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
    ) -> Result<Self, NativeManagerShapeError> {
        if tables.is_empty() {
            return Err(NativeManagerShapeError::MissingTables { module });
        }
        Ok(Self { module, tables })
    }

    simple_accessors! {
        module: RustIdentifier
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }
}

impl NativeStatusEffectDataManager {
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
    ) -> Result<Self, NativeManagerShapeError> {
        if tables.is_empty() {
            return Err(NativeManagerShapeError::MissingTables { module });
        }
        Ok(Self { module, tables })
    }

    simple_accessors! {
        module: RustIdentifier
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }
}

impl NativeCurrencyExchangeMappingManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeRewardTrackDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeDynamicDifficultyDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeGovernanceDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeLootBucketDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeEntitlementDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeEquipmentSetDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeTerritoryDefinitionsDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeSeasonsRewardsDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeSeasonsTrackedStatDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeStatModifierDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeElementalMutationStaticDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativePromotionMutationStaticDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeMusicalRewardsDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeProgressionPointDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeCombatProfilesDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeItemTransformDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeGatherableDataManager {
    #[must_use]
    pub const fn new(
        module: RustIdentifier,
        gathering_database: NativeProductAssetResource,
        gathering_action_database: NativeProductAssetResource,
    ) -> Self {
        Self {
            module,
            gathering_database,
            gathering_action_database,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        gathering_database: NativeProductAssetResource,
        gathering_action_database: NativeProductAssetResource
    }
}

impl NativeSocialDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier, rank_database: NativeProductAssetResource) -> Self {
        Self {
            module,
            rank_database,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        rank_database: NativeProductAssetResource
    }
}

impl NativeSeasonsRewardsActivitiesTasksDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeSeasonsRewardsBattlePassDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeSeasonsRewardsCardTemplateDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeSeasonsRewardsChapterDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeSeasonsRewardsJourneyDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeSongBookSheetDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeSongBookDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativePlayerDataManager {
    #[must_use]
    pub fn new(module: RustIdentifier, product_assets: NativeProductAssetResourceManager) -> Self {
        Self {
            module,
            product_assets,
        }
    }

    simple_accessors! {
        module: RustIdentifier,
        product_assets: NativeProductAssetResourceManager
    }
}

impl NativeTradeskillRankDataManager {
    pub fn new(
        module: RustIdentifier,
        xp_table_name: GameDataTableName,
        xp_row_type_name: GameDataRowTypeName,
        rank_tables: Vec<NativeTableFamilyTable>,
    ) -> Result<Self, NativeManagerShapeError> {
        if rank_tables.is_empty() {
            return Err(NativeManagerShapeError::MissingTables { module });
        }
        Ok(Self {
            module,
            xp_table_name,
            xp_row_type_name,
            rank_tables,
        })
    }

    simple_accessors! {
        module: RustIdentifier,
        xp_table_name: GameDataTableName,
        xp_row_type_name: GameDataRowTypeName
    }

    #[must_use]
    pub fn rank_tables(&self) -> &[NativeTableFamilyTable] {
        &self.rank_tables
    }
}

impl NativeStaticTradeskillRankDataMappingManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeVitalsModifierMappingManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeReplicationDataManager {
    #[must_use]
    pub const fn new(module: RustIdentifier) -> Self {
        Self { module }
    }

    simple_accessors! {
        module: RustIdentifier
    }
}

impl NativeComposedResourceManager {
    pub fn new(
        manager_type: RustTypePath,
        constructor: RustIdentifier,
        arguments: Vec<NativeComposedResourceArgument>,
        returns_result: bool,
    ) -> Result<Self, NativeManagerShapeError> {
        if arguments.is_empty() {
            return Err(NativeManagerShapeError::MissingArguments {
                module: constructor,
            });
        }
        Ok(Self {
            manager_type,
            constructor,
            arguments,
            returns_result,
        })
    }

    simple_accessors! {
        manager_type: RustTypePath,
        constructor: RustIdentifier
    }

    #[must_use]
    pub fn arguments(&self) -> &[NativeComposedResourceArgument] {
        &self.arguments
    }

    #[must_use]
    pub const fn returns_result(&self) -> bool {
        self.returns_result
    }

    #[must_use]
    pub fn has_product_arguments(&self) -> bool {
        self.arguments
            .iter()
            .any(|argument| matches!(argument, NativeComposedResourceArgument::Product(_)))
    }
}

impl NativeComposedResourceArgument {
    #[must_use]
    pub const fn tables() -> Self {
        Self::Tables
    }

    #[must_use]
    pub const fn manager(manager: RustTypePath) -> Self {
        Self::Manager(manager)
    }

    #[must_use]
    pub const fn product(product: NativeProductAssetResource) -> Self {
        Self::Product(product)
    }
}

impl NativeTableFamilyTable {
    #[must_use]
    pub const fn new(
        variant: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
    ) -> Self {
        Self {
            variant,
            table_name,
            row_type_name,
        }
    }

    simple_accessors! {
        variant: RustIdentifier,
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName
    }
}

impl NativeTableFamilyCrcTableIndex {
    pub fn new(
        index_field: RustIdentifier,
        table_variant: RustIdentifier,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&index_field, &methods)?;
        Ok(Self {
            index_field,
            table_variant,
            duplicate_key_policy,
            methods,
        })
    }

    #[must_use]
    pub fn private(
        index_field: RustIdentifier,
        table_variant: RustIdentifier,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
    ) -> Self {
        Self {
            index_field,
            table_variant,
            duplicate_key_policy,
            methods: Vec::new(),
        }
    }

    simple_accessors! {
        index_field: RustIdentifier,
        table_variant: RustIdentifier
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }
}

impl NativePartitionedCrcGlobalIndex {
    pub fn new(
        index_field: RustIdentifier,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&index_field, &methods)?;
        Ok(Self {
            index_field,
            duplicate_key_policy,
            methods,
        })
    }

    simple_accessors! {
        index_field: RustIdentifier
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }
}

impl NativeTableFamilyFallbackCrcKeyProjectionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
        tables_type: RustIdentifier,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_kind_field: RustIdentifier,
        key_kind_type: RustIdentifier,
        primary_key_kind: RustIdentifier,
        fallback_key_kind: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        primary_key_column: GameDataColumnName,
        primary_key_getter: RustIdentifier,
        fallback_key_column: GameDataColumnName,
        fallback_key_getter: RustIdentifier,
        skip_empty_key: bool,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        methods: Vec<NativeCrcIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_tables(&module, &tables)?;
        ensure_methods(&module, &methods)?;
        Ok(Self {
            module,
            table_module: None,
            tables,
            tables_type,
            data_type,
            entries_field,
            index_field,
            key_kind_field,
            key_kind_type,
            primary_key_kind,
            fallback_key_kind,
            key_field,
            crc_field,
            primary_key_column,
            primary_key_getter,
            fallback_key_column,
            fallback_key_getter,
            skip_empty_key,
            duplicate_key_policy,
            methods,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    #[must_use]
    pub fn with_table_module(mut self, module: RustIdentifier) -> Self {
        self.table_module = Some(module);
        self
    }

    #[must_use]
    pub fn table_module(&self) -> &RustIdentifier {
        self.table_module.as_ref().unwrap_or(&self.module)
    }

    manager_common_methods! {
        rows_method => with_rows_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    simple_accessors! {
        module: RustIdentifier,
        tables_type: RustIdentifier,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        index_field: RustIdentifier,
        key_kind_field: RustIdentifier,
        key_kind_type: RustIdentifier,
        primary_key_kind: RustIdentifier,
        fallback_key_kind: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        primary_key_column: GameDataColumnName,
        primary_key_getter: RustIdentifier,
        fallback_key_column: GameDataColumnName,
        fallback_key_getter: RustIdentifier
    }

    bool_accessors! {
        skip_empty_key
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeCrcIndexLookupMethod] {
        &self.methods
    }
}

impl NativeTableFamilyPartitionedCrcKeyProjectionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module: RustIdentifier,
        tables: Vec<NativeTableFamilyTable>,
        tables_type: RustIdentifier,
        table_type: RustIdentifier,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
        skip_empty_key: bool,
        trim_key: bool,
        reject_zero_crc: bool,
        table_indexes: Vec<NativeTableFamilyCrcTableIndex>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_tables(&module, &tables)?;
        if table_indexes.is_empty() {
            return Err(NativeManagerShapeError::MissingLookupMethods { module });
        }
        for table in &tables {
            if !table_indexes
                .iter()
                .any(|index| index.table_variant() == table.variant())
            {
                return Err(NativeManagerShapeError::MissingTableIndex {
                    module,
                    table: table.variant().clone(),
                });
            }
        }
        Ok(Self {
            module,
            tables,
            tables_type,
            table_type,
            data_type,
            entries_field,
            key_field,
            crc_field,
            key_column,
            key_getter,
            hash_policy: NativeCrcHashPolicy::default(),
            skip_empty_key,
            trim_key,
            reject_zero_crc,
            global_index: None,
            table_indexes,
            fields: Vec::new(),
            vec3_fields: Vec::new(),
            store_key_text: true,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        })
    }

    #[must_use]
    pub const fn with_hash_policy(mut self, policy: NativeCrcHashPolicy) -> Self {
        self.hash_policy = policy;
        self
    }

    #[must_use]
    pub fn with_field(mut self, field: NativeProjectionField) -> Self {
        self.fields.push(field);
        self
    }

    #[must_use]
    pub fn with_vec3_field(mut self, field: NativeVec3ProjectionField) -> Self {
        self.vec3_fields.push(field);
        self
    }

    #[must_use]
    pub const fn without_key_text(mut self) -> Self {
        self.store_key_text = false;
        self
    }

    #[must_use]
    pub fn with_global_index(mut self, index: NativePartitionedCrcGlobalIndex) -> Self {
        self.global_index = Some(index);
        self
    }

    manager_common_methods! {
        rows_method => with_rows_method,
        len_method => with_len_method,
        is_empty_method => with_is_empty_method
    }

    simple_accessors! {
        module: RustIdentifier,
        tables_type: RustIdentifier,
        table_type: RustIdentifier,
        data_type: RustIdentifier,
        entries_field: RustIdentifier,
        key_field: RustIdentifier,
        crc_field: RustIdentifier,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier
    }

    bool_accessors! {
        skip_empty_key,
        trim_key,
        reject_zero_crc
    }

    #[must_use]
    pub fn tables(&self) -> &[NativeTableFamilyTable] {
        &self.tables
    }

    #[must_use]
    pub const fn hash_policy(&self) -> NativeCrcHashPolicy {
        self.hash_policy
    }

    #[must_use]
    pub fn table_indexes(&self) -> &[NativeTableFamilyCrcTableIndex] {
        &self.table_indexes
    }

    #[must_use]
    pub const fn global_index(&self) -> Option<&NativePartitionedCrcGlobalIndex> {
        self.global_index.as_ref()
    }

    #[must_use]
    pub fn fields(&self) -> &[NativeProjectionField] {
        &self.fields
    }

    #[must_use]
    pub fn vec3_fields(&self) -> &[NativeVec3ProjectionField] {
        &self.vec3_fields
    }

    #[must_use]
    pub const fn store_key_text(&self) -> bool {
        self.store_key_text
    }
}

impl NativeVec3ProjectionField {
    #[must_use]
    pub const fn new(
        field: RustIdentifier,
        x_column: GameDataColumnName,
        x_getter: RustIdentifier,
        y_column: GameDataColumnName,
        y_getter: RustIdentifier,
        z_column: GameDataColumnName,
        z_getter: RustIdentifier,
    ) -> Self {
        Self {
            field,
            x_column,
            x_getter,
            y_column,
            y_getter,
            z_column,
            z_getter,
        }
    }

    simple_accessors! {
        field: RustIdentifier,
        x_column: GameDataColumnName,
        x_getter: RustIdentifier,
        y_column: GameDataColumnName,
        y_getter: RustIdentifier,
        z_column: GameDataColumnName,
        z_getter: RustIdentifier
    }
}

impl NativeCrcIndexLookupMethod {
    #[must_use]
    pub const fn new(name: RustIdentifier, parameter: NativeCrcIndexLookupParameter) -> Self {
        Self { name, parameter }
    }

    simple_accessors! {
        name: RustIdentifier,
        parameter: NativeCrcIndexLookupParameter
    }
}

impl NativeCrcProjectionDependencyLookupMethod {
    #[must_use]
    pub const fn new(
        name: RustIdentifier,
        dependency_type: RustTypePath,
        dependency_parameter: RustIdentifier,
        key_parameter: NativeCrcIndexLookupParameter,
        dependency_method: RustIdentifier,
        lookup_method: RustIdentifier,
    ) -> Self {
        Self {
            name,
            dependency_type,
            dependency_parameter,
            key_parameter,
            dependency_method,
            lookup_method,
        }
    }

    simple_accessors! {
        name: RustIdentifier,
        dependency_type: RustTypePath,
        dependency_parameter: RustIdentifier,
        key_parameter: NativeCrcIndexLookupParameter,
        dependency_method: RustIdentifier,
        lookup_method: RustIdentifier
    }
}

impl NativeCrcProjectionFieldLookupMethod {
    #[must_use]
    pub const fn new(
        name: RustIdentifier,
        key_parameter: NativeCrcIndexLookupParameter,
        field: RustIdentifier,
        value_type: RustTypePath,
        optional_result: bool,
    ) -> Self {
        Self {
            name,
            key_parameter,
            field,
            value_type,
            optional_result,
        }
    }

    simple_accessors! {
        name: RustIdentifier,
        key_parameter: NativeCrcIndexLookupParameter,
        field: RustIdentifier,
        value_type: RustTypePath
    }

    #[must_use]
    pub const fn optional_result(&self) -> bool {
        self.optional_result
    }
}

impl NativeSourceHandleMethod {
    #[must_use]
    pub const fn new(name: RustIdentifier, parameter: RustIdentifier) -> Self {
        Self { name, parameter }
    }

    simple_accessors! {
        name: RustIdentifier,
        parameter: RustIdentifier
    }
}

impl NativeCrcIndexLookupParameter {
    #[must_use]
    pub const fn new(name: RustIdentifier, kind: NativeCrcIndexLookupParameterKind) -> Self {
        Self { name, kind }
    }

    simple_accessors! {
        name: RustIdentifier
    }

    #[must_use]
    pub const fn kind(&self) -> NativeCrcIndexLookupParameterKind {
        self.kind
    }
}

impl NativeNumericLookupMethod {
    #[must_use]
    pub const fn new(
        name: RustIdentifier,
        parameter: RustIdentifier,
        parameter_kind: NativeNumericLookupParameterKind,
    ) -> Self {
        Self {
            name,
            parameter,
            parameter_kind,
        }
    }

    simple_accessors! {
        name: RustIdentifier,
        parameter: RustIdentifier
    }

    #[must_use]
    pub const fn parameter_kind(&self) -> NativeNumericLookupParameterKind {
        self.parameter_kind
    }
}

impl NativeEnumLookupMethod {
    #[must_use]
    pub const fn new(
        name: RustIdentifier,
        parameter: RustIdentifier,
        parameter_kind: NativeEnumLookupParameterKind,
    ) -> Self {
        Self {
            name,
            parameter,
            parameter_kind,
        }
    }

    simple_accessors! {
        name: RustIdentifier,
        parameter: RustIdentifier
    }

    #[must_use]
    pub const fn parameter_kind(&self) -> NativeEnumLookupParameterKind {
        self.parameter_kind
    }
}

impl NativeStringLookupMethod {
    #[must_use]
    pub const fn new(
        name: RustIdentifier,
        parameter: RustIdentifier,
        parameter_kind: NativeStringLookupParameterKind,
        target: NativeStringLookupTarget,
    ) -> Self {
        Self {
            name,
            parameter,
            parameter_kind,
            target,
        }
    }

    simple_accessors! {
        name: RustIdentifier,
        parameter: RustIdentifier,
        target: NativeStringLookupTarget
    }

    #[must_use]
    pub const fn parameter_kind(&self) -> NativeStringLookupParameterKind {
        self.parameter_kind
    }
}

impl NativeProjectionField {
    #[must_use]
    pub const fn new(
        field: RustIdentifier,
        column: GameDataColumnName,
        getter: RustIdentifier,
        transform: NativeProjectionTransform,
    ) -> Self {
        Self {
            field,
            public_getter: None,
            column,
            getter,
            transform,
            value_type: None,
            default_value: None,
            reference_field: None,
            foreign_key_target: None,
            u16_max_exclusive: None,
        }
    }

    #[must_use]
    pub fn with_value_type(mut self, value_type: RustTypePath) -> Self {
        self.value_type = Some(value_type);
        self
    }

    #[must_use]
    pub fn with_default_value(mut self, default_value: RustPath) -> Self {
        self.default_value = Some(default_value);
        self
    }

    #[must_use]
    pub fn with_reference_field(mut self, reference_field: RustIdentifier) -> Self {
        self.reference_field = Some(reference_field);
        self
    }

    #[must_use]
    pub fn with_foreign_key_target(mut self, target: NativeProjectionForeignKeyTarget) -> Self {
        self.foreign_key_target = Some(target);
        self
    }

    #[must_use]
    pub fn with_public_getter(mut self, getter: RustIdentifier) -> Self {
        self.public_getter = Some(getter);
        self
    }

    #[must_use]
    pub const fn with_u16_max_exclusive(mut self, max_exclusive: u32) -> Self {
        self.u16_max_exclusive = Some(max_exclusive);
        self
    }

    simple_accessors! {
        field: RustIdentifier,
        column: GameDataColumnName,
        getter: RustIdentifier
    }

    #[must_use]
    pub const fn public_getter(&self) -> Option<&RustIdentifier> {
        self.public_getter.as_ref()
    }

    #[must_use]
    pub const fn transform(&self) -> NativeProjectionTransform {
        self.transform
    }

    #[must_use]
    pub const fn value_type(&self) -> Option<&RustTypePath> {
        self.value_type.as_ref()
    }

    #[must_use]
    pub const fn default_value(&self) -> Option<&RustPath> {
        self.default_value.as_ref()
    }

    #[must_use]
    pub const fn reference_field(&self) -> Option<&RustIdentifier> {
        self.reference_field.as_ref()
    }

    #[must_use]
    pub const fn foreign_key_target(&self) -> Option<&NativeProjectionForeignKeyTarget> {
        self.foreign_key_target.as_ref()
    }

    #[must_use]
    pub const fn u16_max_exclusive(&self) -> Option<u32> {
        self.u16_max_exclusive
    }
}

impl NativeProjectionForeignKeyTarget {
    #[must_use]
    pub const fn new(
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier,
    ) -> Self {
        Self {
            table_name,
            row_type_name,
            key_column,
            key_getter,
        }
    }

    simple_accessors! {
        table_name: GameDataTableName,
        row_type_name: GameDataRowTypeName,
        key_column: GameDataColumnName,
        key_getter: RustIdentifier
    }
}

impl NativeSchemaProjectionFields {
    #[must_use]
    pub fn all_non_key() -> Self {
        Self {
            skipped_columns: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_skipped_columns(mut self, columns: Vec<GameDataColumnName>) -> Self {
        self.skipped_columns = columns;
        self
    }

    #[must_use]
    pub fn skipped_columns(&self) -> &[GameDataColumnName] {
        &self.skipped_columns
    }
}

impl NativeDescendingF32Index {
    #[must_use]
    pub const fn new(
        index_field: RustIdentifier,
        value_field: RustIdentifier,
        rows_method: RustIdentifier,
        threshold_lookup_method: RustIdentifier,
        threshold_parameter: RustIdentifier,
    ) -> Self {
        Self {
            index_field,
            value_field,
            rows_method,
            threshold_lookup_method,
            threshold_parameter,
        }
    }

    simple_accessors! {
        index_field: RustIdentifier,
        value_field: RustIdentifier,
        rows_method: RustIdentifier,
        threshold_lookup_method: RustIdentifier,
        threshold_parameter: RustIdentifier
    }
}

impl NativeCrcProjectionRowFilter {
    #[must_use]
    pub const fn new(
        column: GameDataColumnName,
        getter: RustIdentifier,
        predicate: NativeCrcProjectionRowFilterPredicate,
    ) -> Self {
        Self {
            column,
            getter,
            predicate,
            compare_getter: None,
            extra_getters: Vec::new(),
        }
    }

    simple_accessors! {
        column: GameDataColumnName,
        getter: RustIdentifier
    }

    #[must_use]
    pub const fn predicate(&self) -> NativeCrcProjectionRowFilterPredicate {
        self.predicate
    }

    #[must_use]
    pub fn with_compare_getter(mut self, getter: RustIdentifier) -> Self {
        self.compare_getter = Some(getter);
        self
    }

    #[must_use]
    pub fn with_extra_getter(mut self, getter: RustIdentifier) -> Self {
        self.extra_getters.push(getter);
        self
    }

    #[must_use]
    pub fn compare_getter(&self) -> Option<&RustIdentifier> {
        self.compare_getter.as_ref()
    }

    #[must_use]
    pub fn extra_getters(&self) -> &[RustIdentifier] {
        &self.extra_getters
    }
}

impl NativeCrcProjectionSecondaryIndex {
    pub fn new(
        index_field: RustIdentifier,
        key_field: RustIdentifier,
        key_type: NativeSecondaryIndexKeyType,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
        methods: Vec<NativeSecondaryIndexLookupMethod>,
    ) -> Result<Self, NativeManagerShapeError> {
        ensure_methods(&index_field, &methods)?;
        Ok(Self {
            index_field,
            key_field,
            key_type,
            storage: NativeSecondaryIndexStorage::HashMap,
            duplicate_key_policy,
            methods,
        })
    }

    #[must_use]
    pub const fn with_storage(mut self, storage: NativeSecondaryIndexStorage) -> Self {
        self.storage = storage;
        self
    }

    simple_accessors! {
        index_field: RustIdentifier,
        key_field: RustIdentifier
    }

    #[must_use]
    pub const fn key_type(&self) -> NativeSecondaryIndexKeyType {
        self.key_type
    }

    #[must_use]
    pub const fn storage(&self) -> NativeSecondaryIndexStorage {
        self.storage
    }

    #[must_use]
    pub const fn duplicate_key_policy(&self) -> NativeDuplicateKeyPolicy {
        self.duplicate_key_policy
    }

    #[must_use]
    pub fn methods(&self) -> &[NativeSecondaryIndexLookupMethod] {
        &self.methods
    }
}

impl NativeSecondaryIndexLookupMethod {
    #[must_use]
    pub const fn new(
        name: RustIdentifier,
        parameter: RustIdentifier,
        parameter_kind: NativeSecondaryIndexLookupParameterKind,
    ) -> Self {
        Self {
            name,
            parameter,
            parameter_kind,
            result: NativeSecondaryIndexLookupResult::DataRef,
        }
    }

    #[must_use]
    pub fn with_string_field_result(mut self, field: RustIdentifier) -> Self {
        self.result = NativeSecondaryIndexLookupResult::StringField(field);
        self
    }

    simple_accessors! {
        name: RustIdentifier,
        parameter: RustIdentifier
    }

    #[must_use]
    pub const fn parameter_kind(&self) -> NativeSecondaryIndexLookupParameterKind {
        self.parameter_kind
    }

    #[must_use]
    pub const fn result(&self) -> &NativeSecondaryIndexLookupResult {
        &self.result
    }
}

fn ensure_methods<T>(
    module: &RustIdentifier,
    methods: &[T],
) -> Result<(), NativeManagerShapeError> {
    if methods.is_empty() {
        return Err(NativeManagerShapeError::MissingLookupMethods {
            module: module.clone(),
        });
    }
    Ok(())
}

fn ensure_tables(
    module: &RustIdentifier,
    tables: &[NativeTableFamilyTable],
) -> Result<(), NativeManagerShapeError> {
    if tables.is_empty() {
        return Err(NativeManagerShapeError::MissingTables {
            module: module.clone(),
        });
    }
    Ok(())
}
