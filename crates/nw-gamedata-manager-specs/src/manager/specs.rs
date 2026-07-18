use crate::naming::to_upper_camel_ident;
use crate::symbols::{
    GameAssetPath, GameDataColumnName, GameDataRowTypeName, GameDataTableName, GhidraClassPath,
    GhidraFunctionPath, RustIdentifier, RustPath, RustTypePath,
};

use super::{
    NativeAbilityDataManager, NativeBuffBucketDataManager, NativeCharacterAttributeDataManager,
    NativeCombatProfilesDataManager, NativeContributionDataManager, NativeCostumeAudioSlot,
    NativeCrcHashPolicy, NativeCrcIndexLookupMethod, NativeCrcIndexLookupParameter,
    NativeCrcIndexLookupParameterKind, NativeCrcKeyStorageTransform,
    NativeCrcProjectionDependencyLookupMethod, NativeCrcProjectionFieldLookupMethod,
    NativeCrcProjectionRowFilter, NativeCrcProjectionRowFilterPredicate,
    NativeCrcProjectionSecondaryIndex, NativeCurrencyExchangeMappingManager,
    NativeDamageDataManager, NativeDescendingF32Index, NativeDuplicateKeyPolicy,
    NativeDynamicDifficultyDataManager, NativeElementalMutationStaticDataManager,
    NativeEntitlementDataManager, NativeEnumLookupMethod, NativeEnumLookupParameterKind,
    NativeEnumProjectionCrcIndex, NativeEquipmentSetDataManager, NativeGatherableDataManager,
    NativeGovernanceDataManager, NativeItemConversionDataManager, NativeItemDataManager,
    NativeItemTransformDataManager, NativeLootBucketDataManager, NativeManagerInput,
    NativeManagerProductFormat, NativeManagerProductInput, NativeManagerProductKind,
    NativeManagerShape, NativeManagerSpec, NativeMountHitVolumeDataManager,
    NativeMultiTableCrcKeyProjectionManager, NativeMusicalRewardsDataManager, NativeNumericKeyType,
    NativeNumericLookupMethod, NativeNumericLookupParameterKind, NativeObjectivesDataManager,
    NativeOneTableCampSkinManager, NativeOneTableCostumeChangeManager,
    NativeOneTableCrcIndexManager, NativeOneTableCrcKeyProjectionManager,
    NativeOneTableCrestPartManager, NativeOneTableDarknessManager,
    NativeOneTableDifficultyScalingManager, NativeOneTableDungeonTileManager,
    NativeOneTableDyeColorManager, NativeOneTableEmoteManager, NativeOneTableEncumbranceManager,
    NativeOneTableEnumKeyProjectionManager, NativeOneTableExperienceManager,
    NativeOneTableLevelDisparityManager, NativeOneTableNumericKeyProjectionManager,
    NativeOneTableOwnedStringCrcIndexManager, NativeOneTableParticleDataManager,
    NativeOneTablePvpBalanceManager, NativeOneTableRewardTrackItemManager,
    NativeOneTableRowProjectionManager, NativeOneTableStoreCategoryManager,
    NativeOneTableStoreProductManager, NativeOneTableStringKeyProjectionManager,
    NativeOneTableWorldEventRuleManager, NativePartitionedCrcGlobalIndex, NativePlayerDataManager,
    NativePostSkillCapProgressionDataManager, NativeProductAssetResource,
    NativeProductAssetResourceManager, NativeProgressionPointDataManager, NativeProjectionField,
    NativeProjectionForeignKeyTarget, NativeProjectionTransform,
    NativePromotionMutationStaticDataManager, NativeQuickCourseDataManager,
    NativeRecipeDataManager, NativeReplicationDataManager, NativeReusableScoreboardDataManager,
    NativeRewardTrackDataManager, NativeRotationalQueueDataManager, NativeSchemaProjectionFields,
    NativeSeasonsRewardsActivitiesTasksDataManager, NativeSeasonsRewardsBattlePassDataManager,
    NativeSeasonsRewardsCardTemplateDataManager, NativeSeasonsRewardsChapterDataManager,
    NativeSeasonsRewardsDataManager, NativeSeasonsRewardsJourneyDataManager,
    NativeSeasonsTrackedStatDataManager, NativeSecondaryIndexKeyType,
    NativeSecondaryIndexLookupMethod, NativeSecondaryIndexLookupParameterKind,
    NativeSecondaryIndexStorage, NativeSocialDataManager, NativeSongBookDataManager,
    NativeSongBookSheetDataManager, NativeStatModifierDataManager,
    NativeStaticTradeskillRankDataMappingManager, NativeStatusEffectDataManager,
    NativeStringLookupMethod, NativeStringLookupParameterKind, NativeStringLookupTarget,
    NativeStructureDataManager, NativeTableFamilyCrcIndexManager,
    NativeTableFamilyCrcKeyProjectionManager, NativeTableFamilyCrcTableIndex,
    NativeTableFamilyFallbackCrcKeyProjectionManager, NativeTableFamilyNumericKeyProjectionManager,
    NativeTableFamilyOwnedStringCrcIndexManager,
    NativeTableFamilyPartitionedCrcKeyProjectionManager, NativeTableFamilyTable,
    NativeTerritoryDefinitionsDataManager, NativeTradeskillRankDataManager,
    NativeVec3ProjectionField, NativeVitalsDataManager, NativeVitalsModifierMappingManager,
    NativeWhisperDataManager,
};

mod inputs;
mod manager_ability;
mod manager_achievements;
mod manager_backstory;
mod manager_buff_bucket;
mod manager_contribution;
mod manager_entitlement;
mod manager_equipment_set;
mod manager_governance;
mod manager_loot_bucket;
mod manager_mount_hit_volume;
mod manager_music;
mod manager_mutation;
mod manager_objectives;
mod manager_progression;
mod manager_reusable_scoreboard;
mod manager_reward_track;
mod manager_schedule;
mod manager_seasons_rewards;
mod manager_simple;
mod manager_special;
mod manager_stat_modifier;
mod manager_structure;
mod manager_table_families;
mod manager_territory;
mod registry;

use manager_ability::*;
use manager_achievements::*;
use manager_backstory::*;
use manager_buff_bucket::*;
use manager_contribution::*;
use manager_entitlement::*;
use manager_equipment_set::*;
use manager_governance::*;
use manager_loot_bucket::*;
use manager_mount_hit_volume::*;
use manager_music::*;
use manager_mutation::*;
use manager_objectives::*;
use manager_progression::*;
use manager_reusable_scoreboard::*;
use manager_reward_track::*;
use manager_schedule::*;
use manager_seasons_rewards::*;
use manager_simple::*;
use manager_special::*;
use manager_stat_modifier::*;
use manager_structure::*;
use manager_table_families::*;
use manager_territory::*;

pub fn validated_native_manager_specs() -> Vec<NativeManagerSpec> {
    registry::validated_native_manager_specs()
}

#[derive(Default)]
struct OneTableCrcSpec {
    module: &'static str,
    table_name: &'static str,
    row_type_name: &'static str,
    row_alias: &'static str,
    table_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    row_key_method: Option<&'static str>,
    row_crc_method: Option<&'static str>,
    hash_policy: NativeCrcHashPolicy,
    reject_zero_crc: bool,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
    source_row_method: Option<&'static str>,
    rows_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

#[derive(Default)]
struct TableFamilyCrcSpec {
    module: &'static str,
    tables_type: &'static str,
    table_type: &'static str,
    handle_type: &'static str,
    table_family_name: &'static str,
    row_type_name: &'static str,
    row_alias: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    row_key_method: Option<&'static str>,
    row_crc_method: Option<&'static str>,
    reject_zero_crc: bool,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
    source_handle_method: Option<(&'static str, &'static str)>,
    rows_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct OwnedStringSpec {
    module: &'static str,
    table_name: &'static str,
    row_type_name: &'static str,
    row_alias: &'static str,
    table_field: &'static str,
    indexes_field: &'static str,
    indexed_type: &'static str,
    indexes_type: &'static str,
    indexed_key_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    skip_empty_key: bool,
    ascii_case_insensitive: bool,
    duplicate_manager_label: &'static str,
    duplicate_key_label: &'static str,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
    source_row_method: Option<&'static str>,
    ids_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct PvpBalanceSpec {
    module: &'static str,
    table_name: &'static str,
    row_type_name: &'static str,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct RowProjectionSpec {
    module: &'static str,
    table_name: &'static str,
    row_type_name: &'static str,
    data_type: &'static str,
    entries_field: &'static str,
    source_row_field: Option<&'static str>,
    source_row_method: Option<&'static str>,
    source_row_for_method: Option<&'static str>,
    fields: Vec<NativeProjectionField>,
    rows_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct TableFamilyOwnedStringSpec {
    module: &'static str,
    tables: Vec<TableFamilyTableSpec>,
    table_type: &'static str,
    handle_type: &'static str,
    row_alias: &'static str,
    tables_field: &'static str,
    indexes_field: &'static str,
    indexed_type: &'static str,
    indexes_type: &'static str,
    indexed_key_field: &'static str,
    indexed_source_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    skip_empty_key: bool,
    ascii_case_insensitive: bool,
    duplicate_manager_label: &'static str,
    duplicate_key_label: &'static str,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
    source_handle_method: Option<(&'static str, &'static str)>,
    ids_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct CrcProjectionSpec {
    module: &'static str,
    table_name: &'static str,
    row_type_name: &'static str,
    data_type: &'static str,
    entries_field: &'static str,
    index_field: &'static str,
    key_field: &'static str,
    crc_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    skip_empty_key: bool,
    trim_key: bool,
    reject_zero_crc: bool,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_row_field: Option<&'static str>,
    source_row_method: Option<&'static str>,
    row_filters: Vec<NativeCrcProjectionRowFilter>,
    fields: Vec<NativeProjectionField>,
    secondary_indexes: Vec<NativeCrcProjectionSecondaryIndex>,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
    ids_method: Option<&'static str>,
    rows_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct TableFamilyCrcProjectionSpec {
    module: &'static str,
    table_module: Option<&'static str>,
    tables: Vec<TableFamilyTableSpec>,
    tables_type: &'static str,
    table_type: &'static str,
    handle_type: &'static str,
    row_alias: &'static str,
    data_type: &'static str,
    entries_field: &'static str,
    index_field: &'static str,
    key_field: &'static str,
    crc_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    skip_empty_key: bool,
    trim_key: bool,
    reject_zero_crc: bool,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_handle_field: Option<&'static str>,
    source_handle_method: Option<(&'static str, &'static str)>,
    row_filters: Vec<NativeCrcProjectionRowFilter>,
    fields: Vec<NativeProjectionField>,
    table_indexes: Vec<TableFamilyCrcTableIndexSpec>,
    field_lookup_methods: Vec<NativeCrcProjectionFieldLookupMethod>,
    store_key_text: bool,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
    ids_method: Option<&'static str>,
    rows_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct TableFamilyPartitionedCrcProjectionSpec {
    module: &'static str,
    tables: Vec<TableFamilyTableSpec>,
    tables_type: &'static str,
    table_type: &'static str,
    data_type: &'static str,
    entries_field: &'static str,
    key_field: &'static str,
    crc_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    skip_empty_key: bool,
    trim_key: bool,
    reject_zero_crc: bool,
    global_index: Option<PartitionedCrcGlobalIndexSpec>,
    table_indexes: Vec<TableFamilyCrcTableIndexSpec>,
    fields: Vec<NativeProjectionField>,
    vec3_fields: Vec<Vec3ProjectionFieldSpec>,
    store_key_text: bool,
    rows_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct PartitionedCrcGlobalIndexSpec {
    index_field: &'static str,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
}

struct NumericProjectionSpec {
    module: &'static str,
    table_name: &'static str,
    row_type_name: &'static str,
    data_type: &'static str,
    entries_field: &'static str,
    index_field: &'static str,
    key_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    key_type: NativeNumericKeyType,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_row_field: Option<&'static str>,
    source_row_method: Option<&'static str>,
    fields: Vec<NativeProjectionField>,
    lookup_methods: Vec<NativeNumericLookupMethod>,
    rows_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct TableFamilyNumericProjectionSpec {
    module: &'static str,
    tables: Vec<TableFamilyTableSpec>,
    tables_type: &'static str,
    table_type: &'static str,
    handle_type: &'static str,
    row_alias: &'static str,
    data_type: &'static str,
    entries_field: &'static str,
    index_field: &'static str,
    key_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    key_type: NativeNumericKeyType,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_handle_field: Option<&'static str>,
    source_handle_method: Option<(&'static str, &'static str)>,
    fields: Vec<NativeProjectionField>,
    lookup_methods: Vec<NativeNumericLookupMethod>,
    rows_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct EnumProjectionSpec {
    module: &'static str,
    table_name: &'static str,
    row_type_name: &'static str,
    data_type: &'static str,
    entries_field: &'static str,
    index_field: &'static str,
    key_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    key_type: &'static str,
    key_type_alias: Option<&'static str>,
    table_view_alias: Option<&'static str>,
    expose_table_constructor: bool,
    invalid_key_variants: Vec<&'static str>,
    skip_empty_key: bool,
    trim_key: bool,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    source_row_field: Option<&'static str>,
    source_row_method: Option<&'static str>,
    fields: Vec<NativeProjectionField>,
    lookup_methods: Vec<NativeEnumLookupMethod>,
    secondary_crc_index: Option<EnumProjectionCrcIndexSpec>,
    ids_method: Option<&'static str>,
    rows_method: Option<&'static str>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

struct EnumProjectionCrcIndexSpec {
    index_field: &'static str,
    crc_field: &'static str,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
}

struct StringProjectionSpec {
    module: &'static str,
    table_name: &'static str,
    row_type_name: &'static str,
    data_type: &'static str,
    map_field: &'static str,
    key_field: &'static str,
    key_column: &'static str,
    key_getter: &'static str,
    skip_empty_key: bool,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    fields: Vec<NativeProjectionField>,
    lookup_methods: Vec<NativeStringLookupMethod>,
    len_method: Option<&'static str>,
    is_empty_method: Option<&'static str>,
    ghidra_class: &'static str,
    rust_type: &'static str,
    ghidra_functions: Vec<&'static str>,
}

#[derive(Debug, Clone)]
struct TableFamilyTableSpec {
    variant: String,
    table_name: &'static str,
    row_type_name: &'static str,
}

fn native_table_family_tables(
    tables: impl IntoIterator<Item = TableFamilyTableSpec>,
) -> Vec<NativeTableFamilyTable> {
    tables
        .into_iter()
        .map(|table| {
            let TableFamilyTableSpec {
                variant,
                table_name,
                row_type_name,
            } = table;
            NativeTableFamilyTable::new(
                ident(variant),
                game_table(table_name),
                game_row_type(row_type_name),
            )
        })
        .collect()
}

struct TableFamilyCrcTableIndexSpec {
    index_field: &'static str,
    table_variant: &'static str,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
}

struct Vec3ProjectionFieldSpec {
    field: &'static str,
    x_column: &'static str,
    x_getter: &'static str,
    y_column: &'static str,
    y_getter: &'static str,
    z_column: &'static str,
    z_getter: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct AssetProductSpec {
    asset_path: &'static str,
    rust_type: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct ProductAssetResourceSpec {
    product: AssetProductSpec,
    handle_getter: &'static str,
    asset_getter: &'static str,
    manager_getter: &'static str,
}

impl Default for OwnedStringSpec {
    fn default() -> Self {
        Self {
            module: "",
            table_name: "",
            row_type_name: "",
            row_alias: "",
            table_field: "",
            indexes_field: "",
            indexed_type: "",
            indexes_type: "",
            indexed_key_field: "",
            key_column: "",
            key_getter: "",
            skip_empty_key: false,
            ascii_case_insensitive: false,
            duplicate_manager_label: "",
            duplicate_key_label: "",
            duplicate_key_policy: NativeDuplicateKeyPolicy::Error,
            lookup_methods: Vec::new(),
            source_row_method: None,
            ids_method: None,
            len_method: None,
            is_empty_method: None,
            ghidra_class: "",
            rust_type: "",
            ghidra_functions: Vec::new(),
        }
    }
}

impl Default for TableFamilyOwnedStringSpec {
    fn default() -> Self {
        Self {
            module: "",
            tables: Vec::new(),
            table_type: "",
            handle_type: "",
            row_alias: "",
            tables_field: "",
            indexes_field: "",
            indexed_type: "",
            indexes_type: "",
            indexed_key_field: "",
            indexed_source_field: "",
            key_column: "",
            key_getter: "",
            skip_empty_key: false,
            ascii_case_insensitive: false,
            duplicate_manager_label: "",
            duplicate_key_label: "",
            duplicate_key_policy: NativeDuplicateKeyPolicy::Error,
            lookup_methods: Vec::new(),
            source_handle_method: None,
            ids_method: None,
            len_method: None,
            is_empty_method: None,
            ghidra_class: "",
            rust_type: "",
            ghidra_functions: Vec::new(),
        }
    }
}

fn lookup(
    method: &'static str,
    parameter: &'static str,
    kind: NativeCrcIndexLookupParameterKind,
) -> NativeCrcIndexLookupMethod {
    NativeCrcIndexLookupMethod::new(
        ident(method),
        NativeCrcIndexLookupParameter::new(ident(parameter), kind),
    )
}

fn dependency_crc_lookup(
    method: &'static str,
    dependency_type: &'static str,
    dependency_parameter: &'static str,
    key_parameter: &'static str,
    key_parameter_kind: NativeCrcIndexLookupParameterKind,
    dependency_method: &'static str,
    lookup_method: &'static str,
) -> NativeCrcProjectionDependencyLookupMethod {
    NativeCrcProjectionDependencyLookupMethod::new(
        ident(method),
        rust_type(dependency_type),
        ident(dependency_parameter),
        NativeCrcIndexLookupParameter::new(ident(key_parameter), key_parameter_kind),
        ident(dependency_method),
        ident(lookup_method),
    )
}

fn field_lookup(
    method: &'static str,
    key_parameter: &'static str,
    key_parameter_kind: NativeCrcIndexLookupParameterKind,
    field: &'static str,
    value_type: &'static str,
    optional_result: bool,
) -> NativeCrcProjectionFieldLookupMethod {
    NativeCrcProjectionFieldLookupMethod::new(
        ident(method),
        NativeCrcIndexLookupParameter::new(ident(key_parameter), key_parameter_kind),
        ident(field),
        rust_type(value_type),
        optional_result,
    )
}

fn numeric_lookup(
    method: &'static str,
    parameter: &'static str,
    kind: NativeNumericLookupParameterKind,
) -> NativeNumericLookupMethod {
    NativeNumericLookupMethod::new(ident(method), ident(parameter), kind)
}

fn enum_lookup(
    method: &'static str,
    parameter: &'static str,
    kind: NativeEnumLookupParameterKind,
) -> NativeEnumLookupMethod {
    NativeEnumLookupMethod::new(ident(method), ident(parameter), kind)
}

fn projection_field(
    field: &'static str,
    column_name: &'static str,
    getter: &'static str,
    transform: NativeProjectionTransform,
) -> NativeProjectionField {
    NativeProjectionField::new(ident(field), column(column_name), ident(getter), transform)
}

// This builder mirrors the complete foreign-key evidence record.
#[allow(clippy::too_many_arguments)]
fn foreign_key_target_field(
    field: &'static str,
    column_name: &'static str,
    getter: &'static str,
    transform: NativeProjectionTransform,
    target_table_name: &'static str,
    target_row_type_name: &'static str,
    target_key_column_name: &'static str,
    target_key_getter: &'static str,
) -> NativeProjectionField {
    projection_field(field, column_name, getter, transform).with_foreign_key_target(
        NativeProjectionForeignKeyTarget::new(
            game_table(target_table_name),
            game_row_type(target_row_type_name),
            column(target_key_column_name),
            ident(target_key_getter),
        ),
    )
}

fn vec3_projection_field(
    field: &'static str,
    x_column: &'static str,
    x_getter: &'static str,
    y_column: &'static str,
    y_getter: &'static str,
    z_column: &'static str,
    z_getter: &'static str,
) -> Vec3ProjectionFieldSpec {
    Vec3ProjectionFieldSpec {
        field,
        x_column,
        x_getter,
        y_column,
        y_getter,
        z_column,
        z_getter,
    }
}

fn private_table_index(
    index_field: &'static str,
    table_variant: &'static str,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
) -> TableFamilyCrcTableIndexSpec {
    TableFamilyCrcTableIndexSpec {
        index_field,
        table_variant,
        duplicate_key_policy,
        lookup_methods: Vec::new(),
    }
}

fn partitioned_global_index(
    index_field: &'static str,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    lookup_methods: Vec<NativeCrcIndexLookupMethod>,
) -> PartitionedCrcGlobalIndexSpec {
    PartitionedCrcGlobalIndexSpec {
        index_field,
        duplicate_key_policy,
        lookup_methods,
    }
}

fn costume_audio_slot(
    variant: &'static str,
    display: &'static str,
    left_column: &'static str,
    left_getter: &'static str,
    right_column: &'static str,
    right_getter: &'static str,
) -> NativeCostumeAudioSlot {
    NativeCostumeAudioSlot::new(
        ident(variant),
        ident(display),
        column(left_column),
        ident(left_getter),
        column(right_column),
        ident(right_getter),
    )
}

fn typed_projection_field(
    field: &'static str,
    column_name: &'static str,
    getter: &'static str,
    transform: NativeProjectionTransform,
    value_type: &'static str,
) -> NativeProjectionField {
    projection_field(field, column_name, getter, transform).with_value_type(rust_type(value_type))
}

fn optional_u8_enum_default_projection_field(
    field: &'static str,
    column_name: &'static str,
    getter: &'static str,
    value_type: &'static str,
    default_value: &'static str,
) -> NativeProjectionField {
    projection_field(
        field,
        column_name,
        getter,
        NativeProjectionTransform::OptionalU8EnumDefaultValue,
    )
    .with_value_type(rust_type(value_type))
    .with_default_value(rust_path(default_value))
}

fn crc_presence_projection_field(
    field: &'static str,
    column_name: &'static str,
    getter: &'static str,
    reference_field: &'static str,
) -> NativeProjectionField {
    projection_field(
        field,
        column_name,
        getter,
        NativeProjectionTransform::Crc32NonZeroBool,
    )
    .with_reference_field(ident(reference_field))
}

fn capped_u16_projection_field(
    field: &'static str,
    column_name: &'static str,
    getter: &'static str,
    max_exclusive: u32,
) -> NativeProjectionField {
    projection_field(
        field,
        column_name,
        getter,
        NativeProjectionTransform::U32ToU16BelowMax,
    )
    .with_u16_max_exclusive(max_exclusive)
}

fn progression_pool_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableCrcKeyProjectionManager::new(
        ident("progression_pool_data"),
        game_table("ProgressionPools"),
        game_row_type("ProgressionPoolData"),
        ident("StaticProgressionPoolData"),
        ident("rows"),
        ident("rows_by_pool_crc"),
        ident("pool_id"),
        ident("pool_crc"),
        column("ProgressionPoolId"),
        ident("progression_pool_id"),
        true,
        false,
        false,
        NativeDuplicateKeyPolicy::FirstWins,
        vec![
            typed_projection_field(
                "category",
                "Category",
                "category",
                NativeProjectionTransform::U8Enum,
                "gamedata::semantic::PoolCategory",
            ),
            projection_field(
                "point_cap",
                "PointCap",
                "point_cap",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "initial_points",
                "InitialPoints",
                "initial_points",
                NativeProjectionTransform::OptionalU32DefaultZero,
            ),
            projection_field(
                "version_number",
                "VersionNumber",
                "version_number",
                NativeProjectionTransform::U32,
            ),
        ],
        vec![
            lookup(
                "progression_pool_data",
                "pool_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "progression_pool_data_by_key",
                "pool_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
    )
    .expect("validated progression pool manager shape")
    .with_hash_policy(NativeCrcHashPolicy::Lowercase)
    .with_key_wrapper_type(ident("ProgressionPoolId"))
    .with_source_row_field(ident("source_row"))
    .with_source_row_method(ident("source_row"))
    .with_crc_ids_method(ident("pool_ids"))
    .with_rows_method(ident("rows"))
    .with_len_method(ident("len"))
    .with_is_empty_method(ident("is_empty"));

    manager_spec(
        "Javelin::ProgressionPoolDataManager",
        "crate::ProgressionPoolDataManager",
        "ProgressionPools",
        "ProgressionPoolData",
        vec![
            "Javelin::ProgressionPoolDataManager::ProgressionPoolDataManager",
            "Javelin::ProgressionPoolDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::one_table_crc_key_projection(shape))
}

fn secondary_u16_index(
    index_field: &'static str,
    key_field: &'static str,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    methods: Vec<NativeSecondaryIndexLookupMethod>,
) -> NativeCrcProjectionSecondaryIndex {
    NativeCrcProjectionSecondaryIndex::new(
        ident(index_field),
        ident(key_field),
        NativeSecondaryIndexKeyType::U16,
        duplicate_key_policy,
        methods,
    )
    .expect("validated secondary index")
}

fn secondary_u16_lookup(
    method: &'static str,
    parameter: &'static str,
) -> NativeSecondaryIndexLookupMethod {
    NativeSecondaryIndexLookupMethod::new(
        ident(method),
        ident(parameter),
        NativeSecondaryIndexLookupParameterKind::U16,
    )
}

fn sparse_nonzero_u32_index(
    index_field: &'static str,
    key_field: &'static str,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    methods: Vec<NativeSecondaryIndexLookupMethod>,
) -> NativeCrcProjectionSecondaryIndex {
    NativeCrcProjectionSecondaryIndex::new(
        ident(index_field),
        ident(key_field),
        NativeSecondaryIndexKeyType::NonZeroU32,
        duplicate_key_policy,
        methods,
    )
    .expect("validated sparse secondary index")
    .with_storage(NativeSecondaryIndexStorage::SparseVec)
}

fn secondary_u32_lookup(
    method: &'static str,
    parameter: &'static str,
) -> NativeSecondaryIndexLookupMethod {
    NativeSecondaryIndexLookupMethod::new(
        ident(method),
        ident(parameter),
        NativeSecondaryIndexLookupParameterKind::U32,
    )
}

fn secondary_nonzero_u32_index(
    index_field: &'static str,
    key_field: &'static str,
    duplicate_key_policy: NativeDuplicateKeyPolicy,
    methods: Vec<NativeSecondaryIndexLookupMethod>,
) -> NativeCrcProjectionSecondaryIndex {
    NativeCrcProjectionSecondaryIndex::new(
        ident(index_field),
        ident(key_field),
        NativeSecondaryIndexKeyType::NonZeroU32,
        duplicate_key_policy,
        methods,
    )
    .expect("validated nonzero-u32 secondary index")
}

fn secondary_nonzero_u32_lookup(
    method: &'static str,
    parameter: &'static str,
) -> NativeSecondaryIndexLookupMethod {
    NativeSecondaryIndexLookupMethod::new(
        ident(method),
        ident(parameter),
        NativeSecondaryIndexLookupParameterKind::NonZeroU32,
    )
}

fn secondary_u32_string_field_lookup(
    method: &'static str,
    parameter: &'static str,
    field: &'static str,
) -> NativeSecondaryIndexLookupMethod {
    secondary_u32_lookup(method, parameter).with_string_field_result(ident(field))
}

fn referenced_projection_field(
    field: &'static str,
    column_name: &'static str,
    getter: &'static str,
    transform: NativeProjectionTransform,
    reference_field: &'static str,
) -> NativeProjectionField {
    projection_field(field, column_name, getter, transform)
        .with_reference_field(ident(reference_field))
}

fn bool_true_when_present_filter(
    column_name: &'static str,
    getter: &'static str,
) -> NativeCrcProjectionRowFilter {
    crc_row_filter(
        column_name,
        getter,
        NativeCrcProjectionRowFilterPredicate::BoolTrueWhenPresent,
    )
}

fn bool_must_be_true_filter(
    column_name: &'static str,
    getter: &'static str,
) -> NativeCrcProjectionRowFilter {
    crc_row_filter(
        column_name,
        getter,
        NativeCrcProjectionRowFilterPredicate::BoolMustBeTrue,
    )
}

fn f32_greater_than_or_equal_zero_filter(
    column_name: &'static str,
    getter: &'static str,
) -> NativeCrcProjectionRowFilter {
    crc_row_filter(
        column_name,
        getter,
        NativeCrcProjectionRowFilterPredicate::F32GreaterThanOrEqualZero,
    )
}

fn f32_less_than_or_equal_zero_filter(
    column_name: &'static str,
    getter: &'static str,
) -> NativeCrcProjectionRowFilter {
    crc_row_filter(
        column_name,
        getter,
        NativeCrcProjectionRowFilterPredicate::F32LessThanOrEqualZero,
    )
}

fn f32_any_greater_than_zero_filter(
    column_name: &'static str,
    getter: &'static str,
    extra_getters: &[&'static str],
) -> NativeCrcProjectionRowFilter {
    let mut filter = crc_row_filter(
        column_name,
        getter,
        NativeCrcProjectionRowFilterPredicate::F32AnyGreaterThanZero,
    );
    for extra_getter in extra_getters {
        filter = filter.with_extra_getter(ident(*extra_getter));
    }
    filter
}

fn i32_less_than_or_equal_zero_filter(
    column_name: &'static str,
    getter: &'static str,
) -> NativeCrcProjectionRowFilter {
    crc_row_filter(
        column_name,
        getter,
        NativeCrcProjectionRowFilterPredicate::I32LessThanOrEqualZero,
    )
}

fn string_not_equal_to_column_filter(
    column_name: &'static str,
    getter: &'static str,
    compare_getter: &'static str,
) -> NativeCrcProjectionRowFilter {
    crc_row_filter(
        column_name,
        getter,
        NativeCrcProjectionRowFilterPredicate::StringNotEqualToColumn,
    )
    .with_compare_getter(ident(compare_getter))
}

fn lowercase_crc_string_nonzero_filter(
    column_name: &'static str,
    getter: &'static str,
) -> NativeCrcProjectionRowFilter {
    crc_row_filter(
        column_name,
        getter,
        NativeCrcProjectionRowFilterPredicate::LowercaseCrcStringNonZero,
    )
}

fn crc_row_filter(
    column_name: &'static str,
    getter: &'static str,
    predicate: NativeCrcProjectionRowFilterPredicate,
) -> NativeCrcProjectionRowFilter {
    NativeCrcProjectionRowFilter::new(column(column_name), ident(getter), predicate)
}

fn string_lookup(
    method: &'static str,
    parameter: &'static str,
    kind: NativeStringLookupParameterKind,
    target: NativeStringLookupTarget,
) -> NativeStringLookupMethod {
    NativeStringLookupMethod::new(ident(method), ident(parameter), kind, target)
}

fn one_table_crc(spec: OneTableCrcSpec) -> NativeManagerSpec {
    let mut shape = NativeOneTableCrcIndexManager::new(
        ident(spec.module),
        game_table(spec.table_name),
        game_row_type(spec.row_type_name),
        ident(spec.row_alias),
        ident(spec.table_field),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.reject_zero_crc,
        spec.lookup_methods,
    )
    .expect("validated one-table CRC manager shape");

    if let Some(method) = spec.row_key_method {
        shape = shape.with_row_key_method(ident(method));
    }
    if let Some(method) = spec.row_crc_method {
        shape = shape.with_row_crc_method(ident(method));
    }
    shape = shape.with_hash_policy(spec.hash_policy);
    if let Some(method) = spec.source_row_method {
        shape = shape.with_source_row_method(ident(method));
    }
    if let Some(method) = spec.rows_method {
        shape = shape.with_rows_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    manager_spec(
        spec.ghidra_class,
        spec.rust_type,
        spec.table_name,
        spec.row_type_name,
        spec.ghidra_functions,
    )
    .with_shape(NativeManagerShape::one_table_crc_index(shape))
}

fn table_family_crc(spec: TableFamilyCrcSpec) -> NativeManagerSpec {
    let mut shape = NativeTableFamilyCrcIndexManager::new(
        ident(spec.module),
        ident(spec.tables_type),
        ident(spec.table_type),
        ident(spec.handle_type),
        ident(spec.row_alias),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.reject_zero_crc,
        spec.lookup_methods,
    )
    .expect("validated table-family CRC manager shape");

    if let Some(method) = spec.row_key_method {
        shape = shape.with_row_key_method(ident(method));
    }
    if let Some(method) = spec.row_crc_method {
        shape = shape.with_row_crc_method(ident(method));
    }
    if let Some((method, parameter)) = spec.source_handle_method {
        shape = shape.with_source_handle_method(ident(method), ident(parameter));
    }
    if let Some(method) = spec.rows_method {
        shape = shape.with_rows_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    manager_spec(
        spec.ghidra_class,
        spec.rust_type,
        spec.table_family_name,
        spec.row_type_name,
        spec.ghidra_functions,
    )
    .with_shape(NativeManagerShape::table_family_crc_index(shape))
}

fn owned_string(spec: OwnedStringSpec) -> NativeManagerSpec {
    let mut shape = NativeOneTableOwnedStringCrcIndexManager::new(
        ident(spec.module),
        game_table(spec.table_name),
        game_row_type(spec.row_type_name),
        ident(spec.row_alias),
        ident(spec.table_field),
        ident(spec.indexes_field),
        ident(spec.indexed_type),
        ident(spec.indexes_type),
        ident(spec.indexed_key_field),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.skip_empty_key,
        spec.ascii_case_insensitive,
        ident(spec.duplicate_manager_label),
        ident(spec.duplicate_key_label),
        spec.lookup_methods,
    )
    .expect("validated one-table owned-string CRC manager shape")
    .with_duplicate_key_policy(spec.duplicate_key_policy);

    if let Some(method) = spec.source_row_method {
        shape = shape.with_source_row_method(ident(method));
    }
    if let Some(method) = spec.ids_method {
        shape = shape.with_ids_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    manager_spec(
        spec.ghidra_class,
        spec.rust_type,
        spec.table_name,
        spec.row_type_name,
        spec.ghidra_functions,
    )
    .with_shape(NativeManagerShape::one_table_owned_string_crc_index(shape))
}

fn pvp_balance(spec: PvpBalanceSpec) -> NativeManagerSpec {
    let shape = NativeOneTablePvpBalanceManager::new(
        ident(spec.module),
        game_table(spec.table_name),
        game_row_type(spec.row_type_name),
        column("BalanceTarget"),
        ident("balance_target"),
        column("BalanceCategory"),
        ident("balance_category"),
        spec.lookup_methods,
    )
    .expect("validated PvP balance manager shape")
    .with_balances_method(ident("balances"))
    .with_len_method(ident("len"))
    .with_is_empty_method(ident("is_empty"));

    manager_spec(
        spec.ghidra_class,
        spec.rust_type,
        spec.table_name,
        spec.row_type_name,
        spec.ghidra_functions,
    )
    .with_shape(NativeManagerShape::one_table_pvp_balance(shape))
}

fn row_projection(spec: RowProjectionSpec) -> NativeManagerSpec {
    let mut shape = NativeOneTableRowProjectionManager::new(
        ident(spec.module),
        game_table(spec.table_name),
        game_row_type(spec.row_type_name),
        ident(spec.data_type),
        ident(spec.entries_field),
        spec.fields,
    );
    if let Some(field) = spec.source_row_field {
        shape = shape.with_source_row_field(ident(field));
    }
    if let Some(method) = spec.source_row_method {
        shape = shape.with_source_row_method(ident(method));
    }
    if let Some(method) = spec.source_row_for_method {
        shape = shape.with_source_row_for_method(ident(method));
    }
    if let Some(method) = spec.rows_method {
        shape = shape.with_rows_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    let manager = if spec.ghidra_functions.is_empty() {
        NativeManagerSpec::runtime_manifest(
            GhidraClassPath::new(spec.ghidra_class).expect("validated Ghidra class"),
            rust_type(spec.rust_type),
            inputs::manager_inputs_for_manifest(
                spec.rust_type,
                vec![table_input(spec.table_name, spec.row_type_name)],
            ),
        )
        .expect("validated row-projection runtime manifest")
    } else {
        manager_spec(
            spec.ghidra_class,
            spec.rust_type,
            spec.table_name,
            spec.row_type_name,
            spec.ghidra_functions,
        )
    };

    manager.with_shape(NativeManagerShape::one_table_row_projection(shape))
}

fn table_family_owned_string(spec: TableFamilyOwnedStringSpec) -> NativeManagerSpec {
    let input_tables = spec
        .tables
        .iter()
        .map(|table| table_input(table.table_name, table.row_type_name))
        .collect::<Vec<_>>();
    let mut shape = NativeTableFamilyOwnedStringCrcIndexManager::new(
        ident(spec.module),
        spec.tables
            .into_iter()
            .map(|table| {
                NativeTableFamilyTable::new(
                    ident(table.variant),
                    game_table(table.table_name),
                    game_row_type(table.row_type_name),
                )
            })
            .collect(),
        ident(spec.table_type),
        ident(spec.handle_type),
        ident(spec.row_alias),
        ident(spec.tables_field),
        ident(spec.indexes_field),
        ident(spec.indexed_type),
        ident(spec.indexes_type),
        ident(spec.indexed_key_field),
        ident(spec.indexed_source_field),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.skip_empty_key,
        spec.ascii_case_insensitive,
        ident(spec.duplicate_manager_label),
        ident(spec.duplicate_key_label),
        spec.lookup_methods,
    )
    .expect("validated table-family owned-string CRC manager shape")
    .with_duplicate_key_policy(spec.duplicate_key_policy);

    if let Some((method, parameter)) = spec.source_handle_method {
        shape = shape.with_source_handle_method(ident(method), ident(parameter));
    }
    if let Some(method) = spec.ids_method {
        shape = shape.with_ids_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    NativeManagerSpec::new(
        GhidraClassPath::new(spec.ghidra_class).expect("validated Ghidra class"),
        rust_type(spec.rust_type),
        input_tables,
        spec.ghidra_functions
            .into_iter()
            .map(|function| GhidraFunctionPath::new(function).expect("validated Ghidra function"))
            .collect(),
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::table_family_owned_string_crc_index(
        shape,
    ))
}

fn crc_projection(spec: CrcProjectionSpec) -> NativeManagerSpec {
    crc_projection_with_options(spec, NativeCrcHashPolicy::Lowercase, None)
}

fn crc_projection_with_foreign_key_target_inputs(spec: CrcProjectionSpec) -> NativeManagerSpec {
    crc_projection_with_options_schema_dependency_methods_and_inputs(
        spec,
        NativeCrcHashPolicy::Lowercase,
        None,
        None,
        None,
        Vec::new(),
        true,
    )
}

fn crc_projection_with_dependency_lookup_methods(
    spec: CrcProjectionSpec,
    dependency_lookup_methods: Vec<NativeCrcProjectionDependencyLookupMethod>,
) -> NativeManagerSpec {
    crc_projection_with_options_schema_dependency_methods_and_inputs(
        spec,
        NativeCrcHashPolicy::Lowercase,
        None,
        None,
        None,
        dependency_lookup_methods,
        false,
    )
}

fn schema_crc_projection(
    spec: CrcProjectionSpec,
    schema_fields: NativeSchemaProjectionFields,
) -> NativeManagerSpec {
    crc_projection_with_options_and_schema_fields(
        spec,
        NativeCrcHashPolicy::Lowercase,
        None,
        Some(schema_fields),
    )
}

fn validation_crc_projection(
    spec: CrcProjectionSpec,
    schema_validation_fields: NativeSchemaProjectionFields,
) -> NativeManagerSpec {
    crc_projection_with_options_schema_dependency_methods_and_inputs(
        spec,
        NativeCrcHashPolicy::Lowercase,
        None,
        None,
        Some(schema_validation_fields),
        Vec::new(),
        false,
    )
}

fn crc_projection_with_options(
    spec: CrcProjectionSpec,
    hash_policy: NativeCrcHashPolicy,
    source_handle_type: Option<&'static str>,
) -> NativeManagerSpec {
    crc_projection_with_options_and_schema_fields(spec, hash_policy, source_handle_type, None)
}

fn crc_projection_with_options_and_schema_fields(
    spec: CrcProjectionSpec,
    hash_policy: NativeCrcHashPolicy,
    source_handle_type: Option<&'static str>,
    schema_fields: Option<NativeSchemaProjectionFields>,
) -> NativeManagerSpec {
    crc_projection_with_options_schema_dependency_methods_and_inputs(
        spec,
        hash_policy,
        source_handle_type,
        schema_fields,
        None,
        Vec::new(),
        false,
    )
}

fn crc_projection_with_options_schema_dependency_methods_and_inputs(
    spec: CrcProjectionSpec,
    hash_policy: NativeCrcHashPolicy,
    source_handle_type: Option<&'static str>,
    schema_fields: Option<NativeSchemaProjectionFields>,
    schema_validation_fields: Option<NativeSchemaProjectionFields>,
    dependency_lookup_methods: Vec<NativeCrcProjectionDependencyLookupMethod>,
    include_foreign_key_target_inputs: bool,
) -> NativeManagerSpec {
    let mut inputs = vec![table_input(spec.table_name, spec.row_type_name)];
    if include_foreign_key_target_inputs {
        inputs.extend(projection_foreign_key_target_table_inputs(&spec.fields));
    }

    let mut shape = NativeOneTableCrcKeyProjectionManager::new(
        ident(spec.module),
        game_table(spec.table_name),
        game_row_type(spec.row_type_name),
        ident(spec.data_type),
        ident(spec.entries_field),
        ident(spec.index_field),
        ident(spec.key_field),
        ident(spec.crc_field),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.skip_empty_key,
        spec.trim_key,
        spec.reject_zero_crc,
        spec.duplicate_key_policy,
        spec.fields,
        spec.lookup_methods,
    )
    .expect("validated one-table CRC-key projection manager shape");
    shape = shape.with_hash_policy(hash_policy);
    if let Some(schema_fields) = schema_fields {
        shape = shape.with_schema_fields(schema_fields);
    }
    if let Some(schema_validation_fields) = schema_validation_fields {
        shape = shape.with_schema_validation_fields(schema_validation_fields);
    }

    for filter in spec.row_filters {
        shape = shape.with_row_filter(filter);
    }
    for index in spec.secondary_indexes {
        shape = shape.with_secondary_index(index);
    }
    for method in dependency_lookup_methods {
        shape = shape.with_dependency_lookup_method(method);
    }
    if let Some(field) = spec.source_row_field {
        shape = shape.with_source_row_field(ident(field));
    }
    if let Some(method) = spec.source_row_method {
        shape = shape.with_source_row_method(ident(method));
    }
    if let Some(handle_type) = source_handle_type {
        shape = shape.with_source_handle_type(ident(handle_type));
    }
    if let Some(method) = spec.ids_method {
        shape = shape.with_ids_method(ident(method));
    }
    if let Some(method) = spec.rows_method {
        shape = shape.with_rows_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    let manager = if spec.ghidra_functions.is_empty() {
        let inputs = if include_foreign_key_target_inputs {
            inputs
        } else {
            inputs::manager_inputs_for_manifest(spec.rust_type, inputs)
        };
        NativeManagerSpec::runtime_manifest(
            GhidraClassPath::new(spec.ghidra_class).expect("validated Ghidra class"),
            rust_type(spec.rust_type),
            inputs,
        )
        .expect("validated CRC-key projection runtime manifest")
    } else if include_foreign_key_target_inputs {
        manager_spec_with_exact_inputs(
            spec.ghidra_class,
            spec.rust_type,
            inputs,
            spec.ghidra_functions,
        )
    } else {
        manager_spec(
            spec.ghidra_class,
            spec.rust_type,
            spec.table_name,
            spec.row_type_name,
            spec.ghidra_functions,
        )
    };

    manager.with_shape(NativeManagerShape::one_table_crc_key_projection(shape))
}

fn projection_foreign_key_target_table_inputs(
    fields: &[NativeProjectionField],
) -> Vec<NativeManagerInput> {
    let mut inputs = Vec::new();
    for field in fields.iter().filter(|field| {
        matches!(
            field.transform(),
            NativeProjectionTransform::ForeignKeyTargetKey
                | NativeProjectionTransform::ForeignKeyTargetLowercaseCrc
        )
    }) {
        let Some(target) = field.foreign_key_target() else {
            continue;
        };
        let input =
            NativeManagerInput::table(target.table_name().clone(), target.row_type_name().clone());
        if !inputs.contains(&input) {
            inputs.push(input);
        }
    }
    inputs
}

fn table_family_crc_projection(spec: TableFamilyCrcProjectionSpec) -> NativeManagerSpec {
    let input_tables = spec
        .tables
        .iter()
        .map(|table| table_input(table.table_name, table.row_type_name))
        .collect::<Vec<_>>();
    let mut shape = NativeTableFamilyCrcKeyProjectionManager::new(
        ident(spec.module),
        spec.tables
            .into_iter()
            .map(|table| {
                NativeTableFamilyTable::new(
                    ident(table.variant),
                    game_table(table.table_name),
                    game_row_type(table.row_type_name),
                )
            })
            .collect(),
        ident(spec.tables_type),
        ident(spec.table_type),
        ident(spec.handle_type),
        ident(spec.row_alias),
        ident(spec.data_type),
        ident(spec.entries_field),
        ident(spec.index_field),
        ident(spec.key_field),
        ident(spec.crc_field),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.skip_empty_key,
        spec.trim_key,
        spec.reject_zero_crc,
        spec.duplicate_key_policy,
        spec.fields,
        spec.lookup_methods,
    )
    .expect("validated table-family CRC-key projection manager shape");

    for filter in spec.row_filters {
        shape = shape.with_row_filter(filter);
    }
    for index in spec.table_indexes {
        shape = shape.with_table_index(
            NativeTableFamilyCrcTableIndex::new(
                ident(index.index_field),
                ident(index.table_variant),
                index.duplicate_key_policy,
                index.lookup_methods,
            )
            .expect("validated table-family CRC table index"),
        );
    }
    for method in spec.field_lookup_methods {
        shape = shape.with_field_lookup_method(method);
    }
    if let Some(module) = spec.table_module {
        shape = shape.with_table_module(ident(module));
    }
    if !spec.store_key_text {
        shape = shape.without_key_text();
    }
    if let Some(field) = spec.source_handle_field {
        shape = shape.with_source_handle_field(ident(field));
    }
    if let Some((method, parameter)) = spec.source_handle_method {
        shape = shape.with_source_handle_method(ident(method), ident(parameter));
    }
    if let Some(method) = spec.ids_method {
        shape = shape.with_ids_method(ident(method));
    }
    if let Some(method) = spec.rows_method {
        shape = shape.with_rows_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    NativeManagerSpec::new(
        GhidraClassPath::new(spec.ghidra_class).expect("validated Ghidra class"),
        rust_type(spec.rust_type),
        input_tables,
        spec.ghidra_functions
            .into_iter()
            .map(|function| GhidraFunctionPath::new(function).expect("validated Ghidra function"))
            .collect(),
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::table_family_crc_key_projection(shape))
}

fn table_family_partitioned_crc_projection(
    spec: TableFamilyPartitionedCrcProjectionSpec,
) -> NativeManagerSpec {
    let input_tables = spec
        .tables
        .iter()
        .map(|table| table_input(table.table_name, table.row_type_name))
        .collect::<Vec<_>>();
    let table_indexes = spec
        .table_indexes
        .into_iter()
        .map(|index| {
            if index.lookup_methods.is_empty() {
                NativeTableFamilyCrcTableIndex::private(
                    ident(index.index_field),
                    ident(index.table_variant),
                    index.duplicate_key_policy,
                )
            } else {
                NativeTableFamilyCrcTableIndex::new(
                    ident(index.index_field),
                    ident(index.table_variant),
                    index.duplicate_key_policy,
                    index.lookup_methods,
                )
                .expect("validated table-family CRC table index")
            }
        })
        .collect::<Vec<_>>();
    let mut shape = NativeTableFamilyPartitionedCrcKeyProjectionManager::new(
        ident(spec.module),
        spec.tables
            .into_iter()
            .map(|table| {
                NativeTableFamilyTable::new(
                    ident(table.variant),
                    game_table(table.table_name),
                    game_row_type(table.row_type_name),
                )
            })
            .collect(),
        ident(spec.tables_type),
        ident(spec.table_type),
        ident(spec.data_type),
        ident(spec.entries_field),
        ident(spec.key_field),
        ident(spec.crc_field),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.skip_empty_key,
        spec.trim_key,
        spec.reject_zero_crc,
        table_indexes,
    )
    .expect("validated table-family partitioned CRC-key projection manager shape");

    for field in spec.fields {
        shape = shape.with_field(field);
    }
    for field in spec.vec3_fields {
        shape = shape.with_vec3_field(NativeVec3ProjectionField::new(
            ident(field.field),
            column(field.x_column),
            ident(field.x_getter),
            column(field.y_column),
            ident(field.y_getter),
            column(field.z_column),
            ident(field.z_getter),
        ));
    }
    if let Some(index) = spec.global_index {
        shape = shape.with_global_index(
            NativePartitionedCrcGlobalIndex::new(
                ident(index.index_field),
                index.duplicate_key_policy,
                index.lookup_methods,
            )
            .expect("validated partitioned CRC global index"),
        );
    }
    if !spec.store_key_text {
        shape = shape.without_key_text();
    }
    if let Some(method) = spec.rows_method {
        shape = shape.with_rows_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    NativeManagerSpec::new(
        GhidraClassPath::new(spec.ghidra_class).expect("validated Ghidra class"),
        rust_type(spec.rust_type),
        input_tables,
        spec.ghidra_functions
            .into_iter()
            .map(|function| GhidraFunctionPath::new(function).expect("validated Ghidra function"))
            .collect(),
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::table_family_partitioned_crc_key_projection(shape))
}

fn numeric_projection(spec: NumericProjectionSpec) -> NativeManagerSpec {
    let mut shape = NativeOneTableNumericKeyProjectionManager::new(
        ident(spec.module),
        game_table(spec.table_name),
        game_row_type(spec.row_type_name),
        ident(spec.data_type),
        ident(spec.entries_field),
        ident(spec.index_field),
        ident(spec.key_field),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.key_type,
        spec.duplicate_key_policy,
        spec.fields,
        spec.lookup_methods,
    )
    .expect("validated one-table numeric-key projection manager shape");

    if let Some(field) = spec.source_row_field {
        shape = shape.with_source_row_field(ident(field));
    }
    if let Some(method) = spec.source_row_method {
        shape = shape.with_source_row_method(ident(method));
    }
    if let Some(method) = spec.rows_method {
        shape = shape.with_rows_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    manager_spec(
        spec.ghidra_class,
        spec.rust_type,
        spec.table_name,
        spec.row_type_name,
        spec.ghidra_functions,
    )
    .with_shape(NativeManagerShape::one_table_numeric_key_projection(shape))
}

fn table_family_numeric_projection(spec: TableFamilyNumericProjectionSpec) -> NativeManagerSpec {
    let input_tables = spec
        .tables
        .iter()
        .map(|table| table_input(table.table_name, table.row_type_name))
        .collect::<Vec<_>>();
    let mut shape = NativeTableFamilyNumericKeyProjectionManager::new(
        ident(spec.module),
        spec.tables
            .into_iter()
            .map(|table| {
                NativeTableFamilyTable::new(
                    ident(table.variant),
                    game_table(table.table_name),
                    game_row_type(table.row_type_name),
                )
            })
            .collect(),
        ident(spec.tables_type),
        ident(spec.table_type),
        ident(spec.handle_type),
        ident(spec.row_alias),
        ident(spec.data_type),
        ident(spec.entries_field),
        ident(spec.index_field),
        ident(spec.key_field),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.key_type,
        spec.duplicate_key_policy,
        spec.fields,
        spec.lookup_methods,
    )
    .expect("validated table-family numeric-key projection manager shape");

    if let Some(field) = spec.source_handle_field {
        shape = shape.with_source_handle_field(ident(field));
    }
    if let Some((method, parameter)) = spec.source_handle_method {
        shape = shape.with_source_handle_method(ident(method), ident(parameter));
    }
    if let Some(method) = spec.rows_method {
        shape = shape.with_rows_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    NativeManagerSpec::new(
        GhidraClassPath::new(spec.ghidra_class).expect("validated Ghidra class"),
        rust_type(spec.rust_type),
        input_tables,
        spec.ghidra_functions
            .into_iter()
            .map(|function| GhidraFunctionPath::new(function).expect("validated Ghidra function"))
            .collect(),
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::table_family_numeric_key_projection(
        shape,
    ))
}

fn enum_projection(spec: EnumProjectionSpec) -> NativeManagerSpec {
    let mut shape = NativeOneTableEnumKeyProjectionManager::new(
        ident(spec.module),
        game_table(spec.table_name),
        game_row_type(spec.row_type_name),
        ident(spec.data_type),
        ident(spec.entries_field),
        ident(spec.index_field),
        ident(spec.key_field),
        column(spec.key_column),
        ident(spec.key_getter),
        rust_type(spec.key_type),
        spec.invalid_key_variants.into_iter().map(ident).collect(),
        spec.skip_empty_key,
        spec.trim_key,
        spec.duplicate_key_policy,
        spec.fields,
        spec.lookup_methods,
    )
    .expect("validated one-table enum-key projection manager shape");

    if let Some(alias) = spec.key_type_alias {
        shape = shape.with_key_type_alias(ident(alias));
    }
    if let Some(alias) = spec.table_view_alias {
        shape = shape.with_table_view_alias(ident(alias));
    }
    if spec.expose_table_constructor {
        shape = shape.with_exposed_table_constructor();
    }
    if let Some(index) = spec.secondary_crc_index {
        shape = shape.with_secondary_crc_index(NativeEnumProjectionCrcIndex::new(
            ident(index.index_field),
            ident(index.crc_field),
            index.lookup_methods,
        ));
    }
    if let Some(field) = spec.source_row_field {
        shape = shape.with_source_row_field(ident(field));
    }
    if let Some(method) = spec.source_row_method {
        shape = shape.with_source_row_method(ident(method));
    }
    if let Some(method) = spec.ids_method {
        shape = shape.with_ids_method(ident(method));
    }
    if let Some(method) = spec.rows_method {
        shape = shape.with_rows_method(ident(method));
    }
    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    manager_spec(
        spec.ghidra_class,
        spec.rust_type,
        spec.table_name,
        spec.row_type_name,
        spec.ghidra_functions,
    )
    .with_shape(NativeManagerShape::one_table_enum_key_projection(shape))
}

fn one_table_string_projection(spec: StringProjectionSpec) -> NativeManagerSpec {
    let mut shape = NativeOneTableStringKeyProjectionManager::new(
        ident(spec.module),
        game_table(spec.table_name),
        game_row_type(spec.row_type_name),
        ident(spec.data_type),
        ident(spec.map_field),
        ident(spec.key_field),
        column(spec.key_column),
        ident(spec.key_getter),
        spec.skip_empty_key,
        spec.duplicate_key_policy,
        spec.fields,
        spec.lookup_methods,
    )
    .expect("validated one-table string-key projection manager shape");

    if let Some(method) = spec.len_method {
        shape = shape.with_len_method(ident(method));
    }
    if let Some(method) = spec.is_empty_method {
        shape = shape.with_is_empty_method(ident(method));
    }

    manager_spec(
        spec.ghidra_class,
        spec.rust_type,
        spec.table_name,
        spec.row_type_name,
        spec.ghidra_functions,
    )
    .with_shape(NativeManagerShape::one_table_string_key_projection(shape))
}

fn manager_spec(
    ghidra_class: &'static str,
    rust_type_path: &'static str,
    table_name: &'static str,
    row_type_name: &'static str,
    ghidra_functions: Vec<&'static str>,
) -> NativeManagerSpec {
    manager_spec_with_inputs(
        ghidra_class,
        rust_type_path,
        vec![table_input(table_name, row_type_name)],
        ghidra_functions,
    )
}

fn table_input(table_name: &'static str, row_type_name: &'static str) -> NativeManagerInput {
    NativeManagerInput::table(game_table(table_name), game_row_type(row_type_name))
}

fn product_asset_resource_manager_spec(
    ghidra_class: &'static str,
    rust_type_path: &'static str,
    products: Vec<ProductAssetResourceSpec>,
    ghidra_functions: Vec<&'static str>,
) -> NativeManagerSpec {
    product_asset_resource_manager_spec_with_format(
        ghidra_class,
        rust_type_path,
        NativeManagerProductFormat::ObjectStream,
        products,
        ghidra_functions,
    )
}

fn xml_product_asset_resource_manager_spec(
    ghidra_class: &'static str,
    rust_type_path: &'static str,
    products: Vec<ProductAssetResourceSpec>,
    ghidra_functions: Vec<&'static str>,
) -> NativeManagerSpec {
    product_asset_resource_manager_spec_with_format(
        ghidra_class,
        rust_type_path,
        NativeManagerProductFormat::Xml,
        products,
        ghidra_functions,
    )
}

fn product_asset_resource_manager_spec_with_format(
    ghidra_class: &'static str,
    rust_type_path: &'static str,
    format: NativeManagerProductFormat,
    products: Vec<ProductAssetResourceSpec>,
    ghidra_functions: Vec<&'static str>,
) -> NativeManagerSpec {
    let product_inputs = products
        .iter()
        .map(|product| product.product)
        .collect::<Vec<_>>();
    product_manager_spec_with_format(
        ghidra_class,
        rust_type_path,
        format,
        product_inputs,
        ghidra_functions,
    )
    .with_shape(NativeManagerShape::product_asset_resource(
        NativeProductAssetResourceManager::new(
            rust_type(rust_type_path),
            ident("new"),
            products
                .into_iter()
                .map(|product| {
                    NativeProductAssetResource::new(
                        rust_type(product.product.rust_type),
                        rust_type(product.product.rust_type),
                        ident(product.handle_getter),
                        ident(product.asset_getter),
                        ident(product.manager_getter),
                    )
                })
                .collect(),
        )
        .expect("validated product asset resource manager shape"),
    ))
}

fn product_manager_spec_with_format(
    ghidra_class: &'static str,
    rust_type_path: &'static str,
    format: NativeManagerProductFormat,
    products: Vec<AssetProductSpec>,
    ghidra_functions: Vec<&'static str>,
) -> NativeManagerSpec {
    manager_spec_with_inputs(
        ghidra_class,
        rust_type_path,
        products
            .into_iter()
            .map(|product| {
                NativeManagerInput::product(NativeManagerProductInput::new(
                    format,
                    asset_path(product.asset_path),
                    rust_type(product.rust_type),
                ))
            })
            .collect(),
        ghidra_functions,
    )
}

fn manager_spec_with_inputs(
    ghidra_class: &'static str,
    rust_type_path: &'static str,
    inputs: Vec<NativeManagerInput>,
    ghidra_functions: Vec<&'static str>,
) -> NativeManagerSpec {
    NativeManagerSpec::new(
        GhidraClassPath::new(ghidra_class).expect("validated Ghidra class"),
        rust_type(rust_type_path),
        inputs::manager_inputs_for_manifest(rust_type_path, inputs),
        ghidra_functions
            .into_iter()
            .map(|function| GhidraFunctionPath::new(function).expect("validated Ghidra function"))
            .collect(),
    )
    .expect("validated native manager spec")
}

fn manager_spec_with_class_evidence(
    ghidra_class: &'static str,
    rust_type_path: &'static str,
    inputs: Vec<NativeManagerInput>,
) -> NativeManagerSpec {
    NativeManagerSpec::class_evidence(
        GhidraClassPath::new(ghidra_class).expect("validated Ghidra class"),
        rust_type(rust_type_path),
        inputs::manager_inputs_for_manifest(rust_type_path, inputs),
    )
    .expect("validated native manager spec")
}

fn manager_spec_with_exact_inputs(
    ghidra_class: &'static str,
    rust_type_path: &'static str,
    inputs: Vec<NativeManagerInput>,
    ghidra_functions: Vec<&'static str>,
) -> NativeManagerSpec {
    NativeManagerSpec::new(
        GhidraClassPath::new(ghidra_class).expect("validated Ghidra class"),
        rust_type(rust_type_path),
        inputs,
        ghidra_functions
            .into_iter()
            .map(|function| GhidraFunctionPath::new(function).expect("validated Ghidra function"))
            .collect(),
    )
    .expect("validated native manager spec")
}

fn asset_product(asset_path: &'static str, kind: NativeManagerProductKind) -> AssetProductSpec {
    AssetProductSpec {
        asset_path,
        rust_type: kind.canonical_type_path(),
    }
}

fn product_asset_resource(
    asset_path: &'static str,
    kind: NativeManagerProductKind,
    handle_getter: &'static str,
    asset_getter: &'static str,
    manager_getter: &'static str,
) -> ProductAssetResourceSpec {
    ProductAssetResourceSpec {
        product: asset_product(asset_path, kind),
        handle_getter,
        asset_getter,
        manager_getter,
    }
}

fn native_product_asset_resource(product: ProductAssetResourceSpec) -> NativeProductAssetResource {
    NativeProductAssetResource::new(
        rust_type(product.product.rust_type),
        rust_type(product.product.rust_type),
        ident(product.handle_getter),
        ident(product.asset_getter),
        ident(product.manager_getter),
    )
}

fn ident(value: impl Into<String>) -> RustIdentifier {
    RustIdentifier::new(value).expect("validated Rust identifier")
}

fn rust_type(value: &'static str) -> RustTypePath {
    RustTypePath::new(value).expect("validated Rust type")
}

fn rust_path(value: &'static str) -> RustPath {
    RustPath::new(value).expect("validated Rust path")
}

fn game_table(value: &'static str) -> GameDataTableName {
    GameDataTableName::new(value).expect("validated GameData table name")
}

fn asset_path(value: &'static str) -> GameAssetPath {
    GameAssetPath::new(value).expect("validated game asset path")
}

fn game_row_type(value: &'static str) -> GameDataRowTypeName {
    GameDataRowTypeName::new(value).expect("validated GameData row type name")
}

fn column(value: &'static str) -> GameDataColumnName {
    GameDataColumnName::new(value).expect("validated GameData column name")
}

fn categorical_progression_rank_data_tables() -> Vec<TableFamilyTableSpec> {
    vec![
        rank_table("AzothCurrency", "AzothCurrency"),
        rank_table("AzothSaltCurrency", "AzothSaltCurrency"),
        rank_table("BattleToken", "BattleToken"),
        rank_table("BerlaurCurrency", "BerlaurCurrency"),
        rank_table("BerlaurRep", "BerlaurRep"),
        rank_table("BountyGuild", "BountyGuild"),
        rank_table("Camping", "Camping"),
        rank_table("CatacombsCurrencyCrowns", "CatacombsCurrencyCrowns"),
        rank_table(
            "CatacombsCurrencyCursedCrowns",
            "CatacombsCurrencyCursedCrowns",
        ),
        rank_table(
            "CatacombsCurrencyCursedSilvers",
            "CatacombsCurrencyCursedSilvers",
        ),
        rank_table("CatacombsCurrencySilvers", "CatacombsCurrencySilvers"),
        rank_table("CatacombsShop", "CatacombsShop"),
        rank_table("CollectiblesRankData", "CollectiblesRankData"),
        rank_table("CovenantTokens", "CovenantTokens"),
        rank_table("EventRanks", "EventRanks"),
        rank_table("ExplorerGuild", "ExplorerGuild"),
        rank_table("HalloweenEventRanks", "HalloweenEventRanks"),
        rank_table("HouseBonus", "HouseBonus"),
        rank_table("MarauderTokens", "MarauderTokens"),
        rank_table("MerchantGuild", "MerchantGuild"),
        rank_table("MutatorRankData", "MutatorRankData"),
        rank_table("ProcurerGuild", "ProcurerGuild"),
        rank_table("RepairT1", "Repair_T1"),
        rank_table("RepairT2", "Repair_T2"),
        rank_table("RepairT3", "Repair_T3"),
        rank_table("RepairT4", "Repair_T4"),
        rank_table("RepairT5", "Repair_T5"),
        rank_table("SpringEventRanks", "SpringEventRanks"),
        rank_table("SummerEventRanks", "SummerEventRanks"),
        rank_table("SyndicateTokens", "SyndicateTokens"),
        rank_table("TerritoryStanding", "Territory_Standing"),
        rank_table("UmbralCurrency", "UmbralCurrency"),
        rank_table("UpyrCurrency", "UpyrCurrency"),
        rank_table("UpyrRep", "UpyrRep"),
        rank_table("WeaponMastery", "WeaponMastery"),
    ]
}

fn rank_table(variant: &'static str, table_name: &'static str) -> TableFamilyTableSpec {
    TableFamilyTableSpec {
        variant: variant.to_owned(),
        table_name,
        row_type_name: "CategoricalProgressionRankData",
    }
}

fn vitals_base_data_tables() -> Vec<TableFamilyTableSpec> {
    vec![
        vitals_base_data_table("BaseVitalsCatacombs", "BaseVitals_Catacombs"),
        vitals_base_data_table("BaseVitalsCommon", "BaseVitals_Common"),
        vitals_base_data_table("BaseVitalsCutlassKeys", "BaseVitals_CutlassKeys"),
        vitals_base_data_table("BaseVitalsDunwood", "BaseVitals_Dunwood"),
        vitals_base_data_table("BaseVitalsFirstLight", "BaseVitals_FirstLight"),
        vitals_base_data_table("BaseVitalsIsleOfNight", "BaseVitals_IsleOfNight"),
        vitals_base_data_table("BaseVitalsPlayer", "BaseVitals_Player"),
        vitals_base_data_table("BaseVitalsRaidCutlassKeys", "BaseVitals_Raid_CutlassKeys"),
        vitals_base_data_table("BaseVitalsWorldBoss", "BaseVitals_WorldBoss"),
    ]
}

fn vitals_base_data_table(variant: &'static str, table_name: &'static str) -> TableFamilyTableSpec {
    TableFamilyTableSpec {
        variant: variant.to_owned(),
        table_name,
        row_type_name: "VitalsBaseData",
    }
}

fn perk_data_tables() -> Vec<TableFamilyTableSpec> {
    vec![
        TableFamilyTableSpec {
            variant: "ItemPerks".to_owned(),
            table_name: "ItemPerks",
            row_type_name: "PerkData",
        },
        TableFamilyTableSpec {
            variant: "ItemPerks2025".to_owned(),
            table_name: "ItemPerks_2025",
            row_type_name: "PerkData",
        },
        TableFamilyTableSpec {
            variant: "ItemPerksArtifacts".to_owned(),
            table_name: "ItemPerks_Artifacts",
            row_type_name: "PerkData",
        },
        TableFamilyTableSpec {
            variant: "ItemPerksDeprecated".to_owned(),
            table_name: "ItemPerks_Deprecated",
            row_type_name: "PerkData",
        },
        TableFamilyTableSpec {
            variant: "ItemPerksEquipmentSetBonuses".to_owned(),
            table_name: "ItemPerks_EquipmentSetBonuses",
            row_type_name: "PerkData",
        },
        TableFamilyTableSpec {
            variant: "ItemPerksGems".to_owned(),
            table_name: "ItemPerks_Gems",
            row_type_name: "PerkData",
        },
        TableFamilyTableSpec {
            variant: "ItemPerksInfix".to_owned(),
            table_name: "ItemPerks_Infix",
            row_type_name: "PerkData",
        },
    ]
}
