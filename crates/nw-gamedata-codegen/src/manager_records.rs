use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use nw_datasheet::{ColumnType, game_system::Crc32};
use nw_serialize_codegen::go::{go_exported_identifier, go_initialism};
use nw_serialize_codegen::{
    ReflectedTypeRole, ResolvedType, ScalarType, SequenceKind, SerializeCodegenField,
    SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit, SerializeCodegenVariant,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::compiler::GameDataCompileUnit;
use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemDataTablesSchemaReport,
    GameSystemEnumRepresentation, GameSystemEnumShape, GameSystemListAtomShape,
    GameSystemListElementShape,
};
use crate::manager::{
    NativeComposedResourceArgument, NativeComposedResourceManager, NativeCrcIndexLookupMethod,
    NativeCrcIndexLookupParameterKind, NativeCrcProjectionRowFilter,
    NativeCrcProjectionRowFilterPredicate, NativeDuplicateKeyPolicy, NativeGatherableDataManager,
    NativeItemDataManager, NativeManagerInput, NativeManagerShape, NativeManagerSpec,
    NativeNumericKeyType, NativeNumericLookupMethod, NativeNumericLookupParameterKind,
    NativeOneTableCrcIndexManager, NativeOneTableCrcKeyProjectionManager,
    NativeOneTableEnumKeyProjectionManager, NativeOneTableNumericKeyProjectionManager,
    NativeOneTableOwnedStringCrcIndexManager, NativeOneTableRowProjectionManager,
    NativeOneTableStringKeyProjectionManager, NativePlayerDataManager, NativeProductAssetResource,
    NativeProjectionField, NativeProjectionTransform, NativeRecipeDataManager,
    NativeSocialDataManager, NativeStringLookupMethod, NativeTableFamilyCrcIndexManager,
    NativeTableFamilyCrcKeyProjectionManager, NativeTableFamilyFallbackCrcKeyProjectionManager,
    NativeTableFamilyNumericKeyProjectionManager, NativeTableFamilyOwnedStringCrcIndexManager,
    NativeTableFamilyPartitionedCrcKeyProjectionManager, NativeTableFamilyTable,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ManagerSurface {
    Direct(DirectManagerSurface),
    Native {
        manager: DirectManagerSurface,
        shape: NativeManagerShape,
        dependencies: Vec<String>,
        semantic_projections: Vec<SemanticManagerRecord>,
    },
    Semantic(SemanticManagerRecord),
    ItemData(ItemDataManagerSurface),
    Composition(CompositionManagerSurface),
    ProductBacked(DirectManagerSurface),
}

pub(crate) fn manager_surface_name(surface: &ManagerSurface) -> &str {
    match surface {
        ManagerSurface::Direct(manager) | ManagerSurface::ProductBacked(manager) => {
            &manager.manager_name
        }
        ManagerSurface::Native { manager, .. } => &manager.manager_name,
        ManagerSurface::Semantic(manager) => &manager.manager_name,
        ManagerSurface::ItemData(manager) => &manager.manager_name,
        ManagerSurface::Composition(manager) => &manager.manager_name,
    }
}

/// Domain noun used by generated facade accessors.
///
/// The facade already supplies the manager context, so native-facing framing
/// such as `Static`, `Data`, and `Manager` is redundant at call sites.
pub(crate) fn manager_accessor_domain(manager_name: &str) -> &str {
    let without_manager = manager_name.strip_suffix("Manager").unwrap_or(manager_name);
    let without_static = without_manager
        .strip_prefix("Static")
        .filter(|name| !name.is_empty())
        .unwrap_or(without_manager);
    without_static
        .strip_suffix("Data")
        .filter(|name| !name.is_empty())
        .unwrap_or(without_static)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompositionManagerKind {
    CurrencyExchangeMapping,
    ReplicationData,
    StaticTradeskillRankDataMapping,
    VitalsModifierMapping,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompositionManagerSurface {
    pub manager_name: String,
    pub manager_class_name: String,
    pub kind: CompositionManagerKind,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirectManagerSurface {
    pub manager_name: String,
    pub manager_class_name: String,
    pub tables: Vec<DirectManagerTable>,
    pub products: Vec<DirectProductAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirectManagerTable {
    pub table_name: String,
    pub row_type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirectProductAsset {
    pub path: String,
    pub product_type: String,
    pub value_type: String,
    pub manager_getter: String,
}

pub(crate) fn default_direct_manager_row_type<'a>(
    manager_name: &str,
    row_types: &'a [String],
) -> Option<&'a str> {
    let manager_base = manager_name.strip_suffix("Manager").unwrap_or(manager_name);
    row_types
        .iter()
        .find(|row_type| row_type.as_str() == manager_base)
        .or_else(|| (row_types.len() == 1).then(|| &row_types[0]))
        .map(String::as_str)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemDataManagerSurface {
    pub manager_name: String,
    pub manager_class_name: String,
    pub table_type_name: String,
    pub handle_type_name: String,
    pub data_type_name: String,
    pub tables: Vec<ItemDataManagerTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemDataManagerTable {
    pub variant_name: String,
    pub table_name: String,
    pub row_type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticManagerRecord {
    pub manager_name: String,
    pub manager_class_name: String,
    pub record_type_name: String,
    pub tables: Vec<SemanticManagerTable>,
    pub key: Option<SemanticManagerKey>,
    pub source_row_field: Option<String>,
    pub source_row_method: Option<String>,
    pub row_filters: Vec<SemanticRowFilter>,
    pub fields: Vec<SemanticRecordField>,
    pub lookup_methods: Vec<SemanticLookupMethod>,
    pub ids_method: Option<String>,
    pub rows_method: Option<String>,
    pub len_method: Option<String>,
    pub is_empty_method: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticManagerTable {
    pub table_name: String,
    pub row_type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SemanticManagerKey {
    Crc {
        key_field: String,
        crc_field: String,
        key_column: String,
        skip_empty_key: bool,
        trim_key: bool,
        reject_zero_crc: bool,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
    },
    FallbackCrc {
        key_kind_field: String,
        primary_key_kind: String,
        fallback_key_kind: String,
        key_field: String,
        crc_field: String,
        primary_key_column: String,
        fallback_key_column: String,
        skip_empty_key: bool,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
    },
    Numeric {
        key_field: String,
        key_column: String,
        key_type: SemanticNumericKeyType,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
    },
    EnumString {
        key_field: String,
        key_column: String,
        skip_empty_key: bool,
        trim_key: bool,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
    },
    String {
        key_field: String,
        key_column: String,
        skip_empty_key: bool,
        duplicate_key_policy: NativeDuplicateKeyPolicy,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticNumericKeyType {
    U8,
    U16,
    U32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticRecordField {
    pub name: String,
    pub column: String,
    pub transform: SemanticProjectionTransform,
    pub value_type: Option<String>,
    pub default_value: Option<String>,
    pub reference_field: Option<String>,
    pub u16_max_exclusive: Option<u32>,
    pub enum_shape: Option<GameSystemEnumShape>,
    pub pair_first_enum_shape: Option<GameSystemEnumShape>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticProjectionTransform {
    String,
    NonEmptyString,
    StringDefaultEmpty,
    PlusJoinedList,
    OptionalString,
    OptionalFirstString,
    EnumString,
    EnumStringSkipInvalid,
    EnumStringRejectDefault,
    EnumDefault,
    StringList,
    NonEmptyStringList,
    OptionalStringList,
    Bool,
    OptionalBool,
    BoolDefaultFalse,
    Crc32NonZeroBool,
    U8,
    NonZeroU8,
    U8DefaultZero,
    U8DefaultMax,
    U16,
    NonZeroU16,
    U16BelowMax,
    U32,
    OptionalU32,
    U32DefaultZero,
    NonZeroU32,
    OptionalNonZeroU32,
    I32,
    F32,
    OptionalF32,
    F32MinutesToSeconds,
    F32UpperBound10000ZeroIsDefault,
    F32LowerBound10000CappedToField,
    F32List,
    I32List,
    Crc32,
    LowercaseCrcString,
    LowercaseCrcStringDefaultZero,
    FirstLowercaseCrcStringDefaultZero,
    OptionalCrc32,
    OptionalCrc32ZeroAsNone,
    Crc32List,
    OptionalLowercaseCrcString,
    OptionalTrimmedLowercaseCrcString,
    TrimmedLowercaseCrcStringDefaultZero,
    LowercaseCrcStringList,
    ForeignKey,
    OptionalForeignKey,
    ForeignKeyList,
    F32RangeInclusive,
    U32RangeInclusive,
    OptionalCrc32F32PairList,
    OptionalU8F32PairList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticRowFilter {
    pub column: String,
    pub predicate: SemanticRowFilterPredicate,
    pub compare_column: Option<String>,
    pub extra_columns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticRowFilterPredicate {
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
pub(crate) struct SemanticLookupMethod {
    pub name: String,
    pub parameter: String,
    pub kind: SemanticLookupKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticLookupKind {
    CrcStringKey,
    CrcKey,
    IntoCrcKey,
    NumericKey(SemanticNumericKeyType),
    StringKey,
}

pub(crate) fn manager_surfaces(unit: &GameDataCompileUnit) -> Result<Vec<ManagerSurface>> {
    manager_surfaces_for_schema(
        unit.codegen_plan_ref().managers().managers(),
        unit.strict_schema_report(),
    )
}

pub(crate) fn manager_surfaces_for_schema(
    managers: &[NativeManagerSpec],
    schema: &GameSystemDataTablesSchemaReport,
) -> Result<Vec<ManagerSurface>> {
    let mut surfaces = manager_surfaces_from_managers(managers)?;
    validate_direct_manager_row_families(&surfaces, schema)?;
    reconcile_semantic_records_with_source_schema(&mut surfaces, schema)?;
    Ok(surfaces)
}

fn validate_direct_manager_row_families(
    surfaces: &[ManagerSurface],
    schema: &GameSystemDataTablesSchemaReport,
) -> Result<()> {
    let mut diagnostics = Vec::new();
    for manager in surfaces.iter().filter_map(|surface| match surface {
        ManagerSurface::Direct(manager) | ManagerSurface::Native { manager, .. } => Some(manager),
        ManagerSurface::Semantic(_)
        | ManagerSurface::ItemData(_)
        | ManagerSurface::Composition(_)
        | ManagerSurface::ProductBacked(_) => None,
    }) {
        let mut declared_by_row = BTreeMap::<&str, BTreeSet<&str>>::new();
        for table in &manager.tables {
            declared_by_row
                .entry(&table.row_type_name)
                .or_default()
                .insert(&table.table_name);
        }
        for (row_type, declared) in declared_by_row {
            let available = schema
                .tables
                .iter()
                .filter(|table| table.row_type_name == row_type)
                .map(|table| table.table_name.as_str())
                .collect::<BTreeSet<_>>();
            let unknown = declared.difference(&available).copied().collect::<Vec<_>>();
            let uncovered = available.difference(&declared).copied().collect::<Vec<_>>();
            if unknown.is_empty() && uncovered.is_empty() {
                continue;
            }
            diagnostics.push(format!(
                "manager `{}` row family `{row_type}` does not match the source schema (unknown declarations: [{}]; uncovered tables: [{}])",
                manager.manager_name,
                unknown.join(", "),
                uncovered.join(", ")
            ));
        }
    }
    if !diagnostics.is_empty() {
        bail!(
            "standalone direct-manager/schema reconciliation failed:\n{}",
            diagnostics.join("\n")
        );
    }
    Ok(())
}

fn reconcile_semantic_records_with_source_schema(
    surfaces: &mut [ManagerSurface],
    schema: &GameSystemDataTablesSchemaReport,
) -> Result<()> {
    let mut diagnostics = Vec::new();
    for record in surfaces.iter_mut().filter_map(|surface| match surface {
        ManagerSurface::Semantic(record) => Some(record),
        ManagerSurface::Direct(_)
        | ManagerSurface::Native { .. }
        | ManagerSurface::ItemData(_)
        | ManagerSurface::Composition(_)
        | ManagerSurface::ProductBacked(_) => None,
    }) {
        let table_keys = record
            .tables
            .iter()
            .map(|table| (table.table_name.as_str(), table.row_type_name.as_str()))
            .collect::<BTreeSet<_>>();
        for field in &mut record.fields {
            let column_crc = Crc32::from_str_lower(&field.column).value();
            let source_columns = schema
                .tables
                .iter()
                .filter(|table| {
                    table_keys.contains(&(table.table_name.as_str(), table.row_type_name.as_str()))
                })
                .filter_map(|table| table.columns.iter().find(|column| column.crc == column_crc))
                .collect::<Vec<_>>();
            let Some(first) = source_columns.first() else {
                diagnostics.push(format!(
                    "manager `{}` field `{}` references missing column `{}` in tables {}",
                    record.manager_name,
                    field.name,
                    field.column,
                    table_keys
                        .iter()
                        .map(|(table, row)| format!("{table}:{row}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                continue;
            };
            field.column = first.name.clone();

            if semantic_transform_is_enum(field.transform) {
                let enum_shape =
                    match merged_enum_shape(&record.manager_name, &field.name, &source_columns) {
                        Ok(enum_shape) => enum_shape,
                        Err(error) => {
                            diagnostics.push(error.to_string());
                            continue;
                        }
                    };
                if let Some(value_type) = field.value_type.as_deref() {
                    let expected = semantic_type_name(value_type);
                    if enum_shape.name != expected {
                        diagnostics.push(format!(
                            "manager `{}` field `{}` expects enum `{expected}`, but column `{}` is `{}`",
                            record.manager_name,
                            field.name,
                            field.column,
                            enum_shape.name
                        ));
                        continue;
                    }
                }
                field.enum_shape = Some(enum_shape);
            }
            if field.transform == SemanticProjectionTransform::OptionalU8F32PairList {
                match merged_pair_first_enum_shape(
                    &record.manager_name,
                    &field.name,
                    &source_columns,
                ) {
                    Ok(enum_shape) => field.pair_first_enum_shape = Some(enum_shape),
                    Err(error) => diagnostics.push(error.to_string()),
                }
            }
        }
        append_unrepresented_schema_fields(record, schema);
    }
    if !diagnostics.is_empty() {
        bail!(
            "standalone manager/schema reconciliation failed:\n{}",
            diagnostics.join("\n")
        );
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct MergedSemanticSourceColumn {
    name: String,
    declared_type: ColumnType,
    row_key: bool,
}

fn append_unrepresented_schema_fields(
    record: &mut SemanticManagerRecord,
    schema: &GameSystemDataTablesSchemaReport,
) {
    let table_keys = record
        .tables
        .iter()
        .map(|table| (table.table_name.as_str(), table.row_type_name.as_str()))
        .collect::<BTreeSet<_>>();
    let mut columns = BTreeMap::<u32, MergedSemanticSourceColumn>::new();
    for column in schema
        .tables
        .iter()
        .filter(|table| {
            table_keys.contains(&(table.table_name.as_str(), table.row_type_name.as_str()))
        })
        .flat_map(|table| &table.columns)
    {
        columns
            .entry(column.crc)
            .and_modify(|merged| {
                merged.declared_type =
                    merge_semantic_column_type(merged.declared_type, column.declared_type);
                merged.row_key |= column.row_key;
            })
            .or_insert_with(|| MergedSemanticSourceColumn {
                name: column.name.clone(),
                declared_type: column.declared_type,
                row_key: column.row_key,
            });
    }

    let key_columns = semantic_key_columns(record)
        .into_iter()
        .map(|column| Crc32::from_str_lower(column).value())
        .collect::<BTreeSet<_>>();
    let mut occupied_names = semantic_record_field_names(record);
    for (column_crc, column) in columns {
        if key_columns.contains(&column_crc)
            || record.fields.iter().any(|field| {
                Crc32::from_str_lower(&field.column).value() == column_crc
                    && semantic_transform_preserves_source_cell(
                        field.transform,
                        column.declared_type,
                        column.row_key,
                    )
            })
        {
            continue;
        }

        let base_name = crate::naming::to_snake_ident(&column.name, "field");
        let name = unique_semantic_source_field_name(&base_name, &mut occupied_names);
        record.fields.push(SemanticRecordField {
            name,
            column: column.name,
            transform: semantic_source_cell_transform(column.declared_type, column.row_key),
            value_type: None,
            default_value: None,
            reference_field: None,
            u16_max_exclusive: None,
            enum_shape: None,
            pair_first_enum_shape: None,
        });
    }
}

fn merge_semantic_column_type(left: ColumnType, right: ColumnType) -> ColumnType {
    match (left, right) {
        (ColumnType::String, _) | (_, ColumnType::String) => ColumnType::String,
        (ColumnType::Number, _) | (_, ColumnType::Number) => ColumnType::Number,
        (ColumnType::Boolean, ColumnType::Boolean) => ColumnType::Boolean,
    }
}

fn semantic_key_columns(record: &SemanticManagerRecord) -> Vec<&str> {
    match record.key.as_ref() {
        Some(SemanticManagerKey::Crc { key_column, .. })
        | Some(SemanticManagerKey::Numeric { key_column, .. })
        | Some(SemanticManagerKey::EnumString { key_column, .. })
        | Some(SemanticManagerKey::String { key_column, .. }) => vec![key_column],
        Some(SemanticManagerKey::FallbackCrc {
            primary_key_column,
            fallback_key_column,
            ..
        }) => vec![primary_key_column, fallback_key_column],
        None => Vec::new(),
    }
}

fn semantic_record_field_names(record: &SemanticManagerRecord) -> BTreeSet<String> {
    let mut names = record
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<BTreeSet<_>>();
    if let Some(source_row_field) = &record.source_row_field {
        names.insert(source_row_field.clone());
    }
    match record.key.as_ref() {
        Some(SemanticManagerKey::Crc {
            key_field,
            crc_field,
            ..
        })
        | Some(SemanticManagerKey::FallbackCrc {
            key_field,
            crc_field,
            ..
        }) => {
            names.insert(key_field.clone());
            names.insert(crc_field.clone());
        }
        Some(SemanticManagerKey::Numeric { key_field, .. })
        | Some(SemanticManagerKey::EnumString { key_field, .. })
        | Some(SemanticManagerKey::String { key_field, .. }) => {
            names.insert(key_field.clone());
        }
        None => {}
    }
    names
}

fn unique_semantic_source_field_name(
    base_name: &str,
    occupied_names: &mut BTreeSet<String>,
) -> String {
    if occupied_names.insert(base_name.to_owned()) {
        return base_name.to_owned();
    }
    let source_name = format!("{base_name}_source");
    if occupied_names.insert(source_name.clone()) {
        return source_name;
    }
    let mut suffix = 2_u32;
    loop {
        let candidate = format!("{source_name}_{suffix}");
        if occupied_names.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn semantic_source_cell_transform(
    declared_type: ColumnType,
    row_key: bool,
) -> SemanticProjectionTransform {
    match (declared_type, row_key) {
        (ColumnType::String, true) => SemanticProjectionTransform::String,
        (ColumnType::String, false) => SemanticProjectionTransform::OptionalString,
        (ColumnType::Number, true) => SemanticProjectionTransform::F32,
        (ColumnType::Number, false) => SemanticProjectionTransform::OptionalF32,
        (ColumnType::Boolean, true) => SemanticProjectionTransform::Bool,
        (ColumnType::Boolean, false) => SemanticProjectionTransform::OptionalBool,
    }
}

fn semantic_transform_preserves_source_cell(
    transform: SemanticProjectionTransform,
    declared_type: ColumnType,
    row_key: bool,
) -> bool {
    transform == semantic_source_cell_transform(declared_type, row_key)
}

fn semantic_transform_is_enum(transform: SemanticProjectionTransform) -> bool {
    matches!(
        transform,
        SemanticProjectionTransform::EnumString
            | SemanticProjectionTransform::EnumStringSkipInvalid
            | SemanticProjectionTransform::EnumStringRejectDefault
            | SemanticProjectionTransform::EnumDefault
    )
}

pub(crate) fn semantic_enum_type_name(field: &SemanticRecordField) -> &str {
    &field
        .enum_shape
        .as_ref()
        .expect("enum projection fields have reconciled enum schemas")
        .name
}

pub(crate) fn semantic_enum_default_variant(field: &SemanticRecordField) -> &str {
    if let Some(default) = field.default_value.as_deref() {
        return semantic_type_name(default);
    }
    field
        .enum_shape
        .as_ref()
        .and_then(|shape| {
            shape
                .variants
                .iter()
                .find(|variant| variant.discriminant == 0)
        })
        .map(|variant| variant.name.as_str())
        .expect("defaulted enum projections have a zero-discriminant variant")
}

fn merged_enum_shape(
    manager_name: &str,
    field_name: &str,
    columns: &[&GameSystemColumnSchema],
) -> Result<GameSystemEnumShape> {
    let mut shapes = columns
        .iter()
        .filter_map(|column| match &column.value_shape {
            GameSystemColumnValueShape::Enum { enum_shape } => Some(enum_shape),
            _ => None,
        });
    let Some(first) = shapes.next() else {
        bail!(
            "manager `{manager_name}` field `{field_name}` is an enum, but its source column has no enum schema"
        );
    };
    for shape in shapes {
        if shape != first {
            bail!(
                "manager `{manager_name}` field `{field_name}` resolves to conflicting enum schemas `{}` and `{}`",
                first.name,
                shape.name
            );
        }
    }
    Ok(first.clone())
}

fn merged_pair_first_enum_shape(
    manager_name: &str,
    field_name: &str,
    columns: &[&GameSystemColumnSchema],
) -> Result<GameSystemEnumShape> {
    let mut shapes = columns.iter().filter_map(|column| {
        let GameSystemColumnValueShape::String {
            list: Some(list), ..
        } = &column.value_shape
        else {
            return None;
        };
        let Some(GameSystemListElementShape::Pair {
            first: GameSystemListAtomShape::Enum { enum_shape },
            ..
        }) = list.element_shape.as_ref()
        else {
            return None;
        };
        Some(enum_shape)
    });
    let Some(first) = shapes.next() else {
        bail!(
            "manager `{manager_name}` field `{field_name}` expects an enum-keyed pair list, but its source column has no pair enum schema"
        );
    };
    if first.representation != GameSystemEnumRepresentation::U8 {
        bail!(
            "manager `{manager_name}` field `{field_name}` pair enum `{}` is not u8-backed",
            first.name
        );
    }
    for shape in shapes {
        if shape != first {
            bail!(
                "manager `{manager_name}` field `{field_name}` resolves to conflicting pair enum schemas `{}` and `{}`",
                first.name,
                shape.name
            );
        }
    }
    Ok(first.clone())
}

pub(crate) fn manager_surfaces_from_managers(
    managers: &[NativeManagerSpec],
) -> Result<Vec<ManagerSurface>> {
    let surfaces = managers
        .iter()
        .filter_map(|manager| manager_surface(manager).transpose())
        .collect::<Result<Vec<_>>>()?;
    topologically_order_manager_surfaces(surfaces)
}

fn topologically_order_manager_surfaces(
    surfaces: Vec<ManagerSurface>,
) -> Result<Vec<ManagerSurface>> {
    let names = surfaces
        .iter()
        .map(|surface| manager_surface_name(surface).to_owned())
        .collect::<BTreeSet<_>>();
    let missing_dependencies = surfaces
        .iter()
        .flat_map(|surface| {
            surface_manager_dependencies(surface)
                .iter()
                .filter(|dependency| !names.contains(*dependency))
                .map(move |dependency| format!("{} <- {dependency}", manager_surface_name(surface)))
        })
        .collect::<Vec<_>>();
    if !missing_dependencies.is_empty() {
        bail!(
            "manager dependencies without generated surfaces: {}",
            missing_dependencies.join("; ")
        );
    }
    let mut remaining = surfaces;
    let mut emitted = BTreeSet::new();
    let mut ordered = Vec::with_capacity(remaining.len());

    while !remaining.is_empty() {
        let Some(index) = remaining.iter().position(|surface| {
            surface_manager_dependencies(surface)
                .iter()
                .all(|dependency| emitted.contains(dependency))
        }) else {
            let blocked = remaining
                .iter()
                .map(|surface| {
                    let waiting = surface_manager_dependencies(surface)
                        .iter()
                        .filter(|dependency| !emitted.contains(*dependency))
                        .cloned()
                        .collect::<Vec<_>>();
                    format!(
                        "{} <- [{}]",
                        manager_surface_name(surface),
                        waiting.join(", ")
                    )
                })
                .collect::<Vec<_>>();
            bail!("manager dependency cycle: {}", blocked.join("; "));
        };
        let surface = remaining.remove(index);
        emitted.insert(manager_surface_name(&surface).to_owned());
        ordered.push(surface);
    }
    Ok(ordered)
}

fn surface_manager_dependencies(surface: &ManagerSurface) -> &[String] {
    match surface {
        ManagerSurface::Native { dependencies, .. } => dependencies,
        ManagerSurface::Composition(manager) => &manager.dependencies,
        ManagerSurface::Direct(_)
        | ManagerSurface::Semantic(_)
        | ManagerSurface::ItemData(_)
        | ManagerSurface::ProductBacked(_) => &[],
    }
}

pub(crate) fn semantic_manager_record_unit(
    records: &[SemanticManagerRecord],
) -> SerializeCodegenUnit {
    let mut enum_shapes = BTreeMap::<String, GameSystemEnumShape>::new();
    for shape in records
        .iter()
        .flat_map(|record| record.fields.iter())
        .filter_map(|field| field.enum_shape.as_ref())
    {
        match enum_shapes.get(&shape.name) {
            Some(existing) => debug_assert_eq!(existing, shape),
            None => {
                enum_shapes.insert(shape.name.clone(), shape.clone());
            }
        }
    }
    let mut items = enum_shapes
        .values()
        .map(enum_codegen_item)
        .collect::<Vec<_>>();
    items.extend(records.iter().map(record_codegen_item));
    SerializeCodegenUnit { items }
}

pub(crate) fn ts_field_name(source_name: &str) -> String {
    nw_serialize_codegen::typescript::source::typescript_field_name(source_name)
}

pub(crate) fn ts_method_name(source_name: &str) -> String {
    lower_camel(source_name)
}

pub(crate) fn go_field_name(source_name: &str) -> String {
    exported_identifier(source_name, "Field")
}

pub(crate) fn go_method_name(source_name: &str) -> String {
    exported_identifier(source_name, "Method")
}

pub(crate) fn go_local_name(source_name: &str) -> String {
    let snake = crate::naming::to_snake_ident(source_name, "value");
    let mut parts = snake.split('_').filter(|part| !part.is_empty());
    let Some(first) = parts.next() else {
        return "value".to_owned();
    };
    let mut out = go_initialism(first)
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| first.to_owned());
    for part in parts {
        if let Some(initialism) = go_initialism(part) {
            out.push_str(initialism);
        } else {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                out.push(first.to_ascii_uppercase());
                out.extend(chars);
            }
        }
    }
    out
}

pub(crate) fn lower_camel(source_name: &str) -> String {
    let snake = crate::naming::to_snake_ident(source_name, "field");
    let mut parts = snake.split('_');
    let Some(first) = parts.next() else {
        return "field".to_owned();
    };
    let mut out = first.to_owned();
    for part in parts.filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    if is_identifier(&out) {
        out
    } else {
        format!("field{}", upper_camel(source_name))
    }
}

pub(crate) fn upper_camel(source_name: &str) -> String {
    let snake = crate::naming::to_snake_ident(source_name, "field");
    snake
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut out = String::new();
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
            out
        })
        .collect()
}

fn exported_identifier(source_name: &str, fallback_prefix: &str) -> String {
    let ident = go_exported_identifier(source_name);
    if is_identifier(&ident) {
        ident
    } else {
        format!("{fallback_prefix}{ident}")
    }
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn manager_surface(manager: &NativeManagerSpec) -> Result<Option<ManagerSurface>> {
    let manager_name = semantic_type_name(manager.rust_type().as_str()).to_owned();
    let Some(shape) = manager.shape() else {
        bail!("validated manager `{manager_name}` has no generated surface shape");
    };
    if let NativeManagerShape::ItemData(shape) = shape {
        return Ok(Some(ManagerSurface::ItemData(item_data_manager_surface(
            manager_name,
            shape,
        ))));
    }
    if let Some(kind) = composition_manager_kind(shape) {
        return Ok(Some(ManagerSurface::Composition(
            CompositionManagerSurface {
                manager_class_name: manager_name.clone(),
                manager_name,
                kind,
                dependencies: manager_dependency_names(manager),
            },
        )));
    }
    if let Some(record) = semantic_manager_record(&manager_name, manager, shape) {
        return Ok(Some(ManagerSurface::Semantic(record?)));
    }
    let products = direct_products(manager, shape)?;
    if is_product_backed_surface(shape, &products) && !shape.exposes_native_api() {
        return Ok(Some(ManagerSurface::ProductBacked(
            product_backed_manager_surface(manager_name, products),
        )));
    }
    let semantic_projections = if shape.exposes_native_api() {
        semantic_projection_records(&manager_name, manager, shape)
            .transpose()?
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let direct = direct_manager_surface(manager_name, manager, products);
    if direct.tables.is_empty() && direct.products.is_empty() {
        bail!(
            "validated manager `{}` has neither a table nor asset-backed generated surface",
            direct.manager_name
        );
    }
    if shape.exposes_native_api() {
        Ok(Some(ManagerSurface::Native {
            manager: direct,
            shape: shape.clone(),
            dependencies: manager_dependency_names(manager),
            semantic_projections,
        }))
    } else {
        Ok(Some(ManagerSurface::Direct(direct)))
    }
}

fn manager_dependency_names(manager: &NativeManagerSpec) -> Vec<String> {
    manager
        .inputs()
        .iter()
        .filter_map(|input| match input {
            NativeManagerInput::Manager(manager) => {
                Some(semantic_type_name(manager.as_str()).to_owned())
            }
            NativeManagerInput::Table(_) | NativeManagerInput::Product(_) => None,
        })
        .collect()
}

fn composition_manager_kind(shape: &NativeManagerShape) -> Option<CompositionManagerKind> {
    match shape {
        NativeManagerShape::CurrencyExchangeMapping(_) => {
            Some(CompositionManagerKind::CurrencyExchangeMapping)
        }
        NativeManagerShape::ReplicationData(_) => Some(CompositionManagerKind::ReplicationData),
        NativeManagerShape::StaticTradeskillRankDataMapping(_) => {
            Some(CompositionManagerKind::StaticTradeskillRankDataMapping)
        }
        NativeManagerShape::VitalsModifierMapping(_) => {
            Some(CompositionManagerKind::VitalsModifierMapping)
        }
        _ => None,
    }
}

fn is_product_backed_surface(shape: &NativeManagerShape, products: &[DirectProductAsset]) -> bool {
    !products.is_empty()
        && matches!(
            shape,
            NativeManagerShape::ProductAssetResource(_)
                | NativeManagerShape::RecipeData(_)
                | NativeManagerShape::GatherableData(_)
                | NativeManagerShape::SocialData(_)
                | NativeManagerShape::PlayerData(_)
        )
}

fn product_backed_manager_surface(
    manager_name: String,
    products: Vec<DirectProductAsset>,
) -> DirectManagerSurface {
    DirectManagerSurface {
        manager_class_name: manager_name.clone(),
        manager_name,
        tables: Vec::new(),
        products,
    }
}

fn direct_manager_surface(
    manager_name: String,
    manager: &NativeManagerSpec,
    products: Vec<DirectProductAsset>,
) -> DirectManagerSurface {
    DirectManagerSurface {
        manager_class_name: manager_name.clone(),
        manager_name,
        tables: direct_manager_tables(manager),
        products,
    }
}

fn item_data_manager_surface(
    manager_name: String,
    shape: &NativeItemDataManager,
) -> ItemDataManagerSurface {
    ItemDataManagerSurface {
        manager_class_name: manager_name.clone(),
        manager_name,
        table_type_name: shape.table_type().as_str().to_owned(),
        handle_type_name: shape.handle_type().as_str().to_owned(),
        data_type_name: shape.data_type().as_str().to_owned(),
        tables: shape.tables().iter().map(item_data_manager_table).collect(),
    }
}

fn item_data_manager_table(table: &NativeTableFamilyTable) -> ItemDataManagerTable {
    ItemDataManagerTable {
        variant_name: table.variant().as_str().to_owned(),
        table_name: table.table_name().as_str().to_owned(),
        row_type_name: table.row_type_name().as_str().to_owned(),
    }
}

fn direct_products(
    manager: &NativeManagerSpec,
    shape: &NativeManagerShape,
) -> Result<Vec<DirectProductAsset>> {
    let products = match shape {
        NativeManagerShape::ProductAssetResource(shape) => {
            shape.products().iter().collect::<Vec<_>>()
        }
        NativeManagerShape::RecipeData(shape) => recipe_data_products(shape),
        NativeManagerShape::GatherableData(shape) => gatherable_data_products(shape),
        NativeManagerShape::SocialData(shape) => social_data_products(shape),
        NativeManagerShape::PlayerData(shape) => player_data_products(shape),
        NativeManagerShape::ComposedResource(shape) => composed_resource_products(shape),
        _ => return Ok(Vec::new()),
    };
    products
        .into_iter()
        .map(|product| direct_product(manager, product))
        .collect()
}

fn recipe_data_products(shape: &NativeRecipeDataManager) -> Vec<&NativeProductAssetResource> {
    vec![shape.product()]
}

fn gatherable_data_products(
    shape: &NativeGatherableDataManager,
) -> Vec<&NativeProductAssetResource> {
    vec![
        shape.gathering_database(),
        shape.gathering_action_database(),
    ]
}

fn social_data_products(shape: &NativeSocialDataManager) -> Vec<&NativeProductAssetResource> {
    vec![shape.rank_database()]
}

fn player_data_products(shape: &NativePlayerDataManager) -> Vec<&NativeProductAssetResource> {
    shape.product_assets().products().iter().collect()
}

fn composed_resource_products(
    shape: &NativeComposedResourceManager,
) -> Vec<&NativeProductAssetResource> {
    shape
        .arguments()
        .iter()
        .filter_map(|argument| match argument {
            NativeComposedResourceArgument::Product(product) => Some(product),
            NativeComposedResourceArgument::Tables | NativeComposedResourceArgument::Manager(_) => {
                None
            }
        })
        .collect()
}

fn direct_product(
    manager: &NativeManagerSpec,
    product: &NativeProductAssetResource,
) -> Result<DirectProductAsset> {
    let input = manager.inputs().iter().find_map(|input| match input {
        NativeManagerInput::Product(input)
            if input.rust_type().as_str() == product.product_type().as_str() =>
        {
            Some(input)
        }
        NativeManagerInput::Table(_)
        | NativeManagerInput::Product(_)
        | NativeManagerInput::Manager(_) => None,
    });
    let Some(input) = input else {
        bail!(
            "manager `{}` product `{}` has no matching product input",
            manager.rust_type().as_str(),
            product.product_type().as_str()
        );
    };
    Ok(DirectProductAsset {
        path: input.asset_path().as_str().to_owned(),
        product_type: product.product_type().as_str().to_owned(),
        value_type: product.value_type().as_str().to_owned(),
        manager_getter: product.manager_getter().as_str().to_owned(),
    })
}

fn semantic_manager_record(
    manager_name: &str,
    manager: &NativeManagerSpec,
    shape: &NativeManagerShape,
) -> Option<Result<SemanticManagerRecord>> {
    match shape {
        NativeManagerShape::OneTableCrcIndex(shape) => {
            Some(one_table_crc_index_record(manager_name, shape))
        }
        NativeManagerShape::TableFamilyCrcIndex(shape) => {
            Some(table_family_crc_index_record(manager_name, manager, shape))
        }
        NativeManagerShape::OneTableOwnedStringCrcIndex(shape) => {
            Some(one_table_owned_string_crc_index_record(manager_name, shape))
        }
        NativeManagerShape::TableFamilyOwnedStringCrcIndex(shape) => Some(
            table_family_owned_string_crc_index_record(manager_name, shape),
        ),
        NativeManagerShape::OneTableCrcKeyProjection(shape) => {
            Some(one_table_crc_record(manager_name, shape))
        }
        NativeManagerShape::TableFamilyCrcKeyProjection(shape) => {
            Some(table_family_crc_record(manager_name, shape))
        }
        NativeManagerShape::TableFamilyPartitionedCrcKeyProjection(shape) => {
            Some(table_family_partitioned_crc_record(manager_name, shape))
        }
        NativeManagerShape::TableFamilyFallbackCrcKeyProjection(shape) => {
            Some(table_family_fallback_crc_record(manager_name, shape))
        }
        NativeManagerShape::OneTableNumericKeyProjection(shape) => {
            Some(one_table_numeric_record(manager_name, shape))
        }
        NativeManagerShape::TableFamilyNumericKeyProjection(shape) => {
            Some(table_family_numeric_record(manager_name, shape))
        }
        NativeManagerShape::OneTableEnumKeyProjection(shape) => {
            Some(one_table_enum_record(manager_name, shape))
        }
        NativeManagerShape::OneTableStringKeyProjection(shape) => {
            Some(one_table_string_record(manager_name, shape))
        }
        NativeManagerShape::OneTableRowProjection(shape) => {
            Some(one_table_row_record(manager_name, shape))
        }
        _ => None,
    }
}

pub(crate) fn semantic_projection_records(
    manager_name: &str,
    manager: &NativeManagerSpec,
    shape: &NativeManagerShape,
) -> Option<Result<Vec<SemanticManagerRecord>>> {
    match shape {
        NativeManagerShape::MultiTableCrcKeyProjection(shape) => Some(
            shape
                .projections()
                .iter()
                .map(|projection| one_table_crc_record(manager_name, projection))
                .collect(),
        ),
        _ => semantic_manager_record(manager_name, manager, shape)
            .map(|record| record.map(|record| vec![record])),
    }
}

fn one_table_crc_index_record(
    manager_name: &str,
    shape: &NativeOneTableCrcIndexManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.row_alias().as_str().to_owned(),
        tables: one_table(shape.table_name().as_str(), shape.row_type_name().as_str()),
        key: Some(SemanticManagerKey::Crc {
            key_field: shape.row_key_method().as_str().to_owned(),
            crc_field: crc_index_field_name(shape.row_crc_method()),
            key_column: shape.key_column().as_str().to_owned(),
            skip_empty_key: true,
            trim_key: false,
            reject_zero_crc: shape.reject_zero_crc(),
            duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        }),
        source_row_field: shape.source_row_method().map(|_| "source_row".to_owned()),
        source_row_method: shape
            .source_row_method()
            .map(|method| method.as_str().to_owned()),
        row_filters: Vec::new(),
        fields: Vec::new(),
        lookup_methods: crc_lookup_methods(shape.methods()),
        ids_method: None,
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn table_family_crc_index_record(
    manager_name: &str,
    manager: &NativeManagerSpec,
    shape: &NativeTableFamilyCrcIndexManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.row_alias().as_str().to_owned(),
        tables: manager_input_tables(manager),
        key: Some(SemanticManagerKey::Crc {
            key_field: shape.row_key_method().as_str().to_owned(),
            crc_field: crc_index_field_name(shape.row_crc_method()),
            key_column: shape.key_column().as_str().to_owned(),
            skip_empty_key: true,
            trim_key: false,
            reject_zero_crc: shape.reject_zero_crc(),
            duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        }),
        source_row_field: None,
        source_row_method: None,
        row_filters: Vec::new(),
        fields: Vec::new(),
        lookup_methods: crc_lookup_methods(shape.methods()),
        ids_method: None,
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn one_table_owned_string_crc_index_record(
    manager_name: &str,
    shape: &NativeOneTableOwnedStringCrcIndexManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.indexed_type().as_str().to_owned(),
        tables: one_table(shape.table_name().as_str(), shape.row_type_name().as_str()),
        key: Some(SemanticManagerKey::Crc {
            key_field: shape.indexed_key_field().as_str().to_owned(),
            crc_field: "crc".to_owned(),
            key_column: shape.key_column().as_str().to_owned(),
            skip_empty_key: shape.skip_empty_key(),
            trim_key: false,
            reject_zero_crc: false,
            duplicate_key_policy: shape.duplicate_key_policy(),
        }),
        source_row_field: shape.source_row_method().map(|_| "source_row".to_owned()),
        source_row_method: shape
            .source_row_method()
            .map(|method| method.as_str().to_owned()),
        row_filters: Vec::new(),
        fields: Vec::new(),
        lookup_methods: crc_lookup_methods(shape.methods()),
        ids_method: shape.ids_method().map(|method| method.as_str().to_owned()),
        rows_method: None,
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn table_family_owned_string_crc_index_record(
    manager_name: &str,
    shape: &NativeTableFamilyOwnedStringCrcIndexManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.indexed_type().as_str().to_owned(),
        tables: family_tables(shape.tables()),
        key: Some(SemanticManagerKey::Crc {
            key_field: shape.indexed_key_field().as_str().to_owned(),
            crc_field: "crc".to_owned(),
            key_column: shape.key_column().as_str().to_owned(),
            skip_empty_key: shape.skip_empty_key(),
            trim_key: false,
            reject_zero_crc: false,
            duplicate_key_policy: shape.duplicate_key_policy(),
        }),
        source_row_field: None,
        source_row_method: None,
        row_filters: Vec::new(),
        fields: Vec::new(),
        lookup_methods: crc_lookup_methods(shape.methods()),
        ids_method: shape.ids_method().map(|method| method.as_str().to_owned()),
        rows_method: None,
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn one_table_crc_record(
    manager_name: &str,
    shape: &NativeOneTableCrcKeyProjectionManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.data_type().as_str().to_owned(),
        tables: one_table(shape.table_name().as_str(), shape.row_type_name().as_str()),
        key: Some(SemanticManagerKey::Crc {
            key_field: shape.key_field().as_str().to_owned(),
            crc_field: shape.crc_field().as_str().to_owned(),
            key_column: shape.key_column().as_str().to_owned(),
            skip_empty_key: shape.skip_empty_key(),
            trim_key: shape.trim_key(),
            reject_zero_crc: shape.reject_zero_crc(),
            duplicate_key_policy: shape.duplicate_key_policy(),
        }),
        source_row_field: shape
            .source_row_field()
            .map(|field| field.as_str().to_owned()),
        source_row_method: shape
            .source_row_method()
            .map(|method| method.as_str().to_owned()),
        row_filters: row_filters(shape.row_filters(), shape.fields())?,
        fields: record_fields(shape.fields())?,
        lookup_methods: crc_lookup_methods(shape.methods()),
        ids_method: shape.ids_method().map(|method| method.as_str().to_owned()),
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn table_family_crc_record(
    manager_name: &str,
    shape: &NativeTableFamilyCrcKeyProjectionManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.data_type().as_str().to_owned(),
        tables: family_tables(shape.tables()),
        key: Some(SemanticManagerKey::Crc {
            key_field: shape.key_field().as_str().to_owned(),
            crc_field: shape.crc_field().as_str().to_owned(),
            key_column: shape.key_column().as_str().to_owned(),
            skip_empty_key: shape.skip_empty_key(),
            trim_key: shape.trim_key(),
            reject_zero_crc: shape.reject_zero_crc(),
            duplicate_key_policy: shape.duplicate_key_policy(),
        }),
        source_row_field: None,
        source_row_method: None,
        row_filters: row_filters(shape.row_filters(), shape.fields())?,
        fields: record_fields(shape.fields())?,
        lookup_methods: crc_lookup_methods(shape.methods()),
        ids_method: shape.ids_method().map(|method| method.as_str().to_owned()),
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn table_family_partitioned_crc_record(
    manager_name: &str,
    shape: &NativeTableFamilyPartitionedCrcKeyProjectionManager,
) -> Result<SemanticManagerRecord> {
    let lookup_methods = shape
        .global_index()
        .map(|index| crc_lookup_methods(index.methods()))
        .unwrap_or_default();
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.data_type().as_str().to_owned(),
        tables: family_tables(shape.tables()),
        key: Some(SemanticManagerKey::Crc {
            key_field: shape.key_field().as_str().to_owned(),
            crc_field: shape.crc_field().as_str().to_owned(),
            key_column: shape.key_column().as_str().to_owned(),
            skip_empty_key: shape.skip_empty_key(),
            trim_key: shape.trim_key(),
            reject_zero_crc: shape.reject_zero_crc(),
            duplicate_key_policy: shape
                .global_index()
                .map(|index| index.duplicate_key_policy())
                .unwrap_or(NativeDuplicateKeyPolicy::Overwrite),
        }),
        source_row_field: None,
        source_row_method: None,
        row_filters: Vec::new(),
        fields: record_fields(shape.fields())?,
        lookup_methods,
        ids_method: None,
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn table_family_fallback_crc_record(
    manager_name: &str,
    shape: &NativeTableFamilyFallbackCrcKeyProjectionManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.data_type().as_str().to_owned(),
        tables: family_tables(shape.tables()),
        key: Some(SemanticManagerKey::FallbackCrc {
            key_kind_field: shape.key_kind_field().as_str().to_owned(),
            primary_key_kind: shape.primary_key_kind().as_str().to_owned(),
            fallback_key_kind: shape.fallback_key_kind().as_str().to_owned(),
            key_field: shape.key_field().as_str().to_owned(),
            crc_field: shape.crc_field().as_str().to_owned(),
            primary_key_column: shape.primary_key_column().as_str().to_owned(),
            fallback_key_column: shape.fallback_key_column().as_str().to_owned(),
            skip_empty_key: shape.skip_empty_key(),
            duplicate_key_policy: shape.duplicate_key_policy(),
        }),
        source_row_field: None,
        source_row_method: None,
        row_filters: Vec::new(),
        fields: Vec::new(),
        lookup_methods: crc_lookup_methods(shape.methods()),
        ids_method: None,
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn one_table_numeric_record(
    manager_name: &str,
    shape: &NativeOneTableNumericKeyProjectionManager,
) -> Result<SemanticManagerRecord> {
    let key_type = semantic_numeric_key_type(shape.key_type());
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.data_type().as_str().to_owned(),
        tables: one_table(shape.table_name().as_str(), shape.row_type_name().as_str()),
        key: Some(SemanticManagerKey::Numeric {
            key_field: shape.key_field().as_str().to_owned(),
            key_column: shape.key_column().as_str().to_owned(),
            key_type,
            duplicate_key_policy: shape.duplicate_key_policy(),
        }),
        source_row_field: shape
            .source_row_field()
            .map(|field| field.as_str().to_owned()),
        source_row_method: shape
            .source_row_method()
            .map(|method| method.as_str().to_owned()),
        row_filters: Vec::new(),
        fields: record_fields(shape.fields())?,
        lookup_methods: numeric_lookup_methods(shape.methods()),
        ids_method: None,
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn table_family_numeric_record(
    manager_name: &str,
    shape: &NativeTableFamilyNumericKeyProjectionManager,
) -> Result<SemanticManagerRecord> {
    let key_type = semantic_numeric_key_type(shape.key_type());
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.data_type().as_str().to_owned(),
        tables: family_tables(shape.tables()),
        key: Some(SemanticManagerKey::Numeric {
            key_field: shape.key_field().as_str().to_owned(),
            key_column: shape.key_column().as_str().to_owned(),
            key_type,
            duplicate_key_policy: shape.duplicate_key_policy(),
        }),
        source_row_field: None,
        source_row_method: None,
        row_filters: Vec::new(),
        fields: record_fields(shape.fields())?,
        lookup_methods: numeric_lookup_methods(shape.methods()),
        ids_method: None,
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn one_table_enum_record(
    manager_name: &str,
    shape: &NativeOneTableEnumKeyProjectionManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.data_type().as_str().to_owned(),
        tables: one_table(shape.table_name().as_str(), shape.row_type_name().as_str()),
        key: Some(SemanticManagerKey::EnumString {
            key_field: shape.key_field().as_str().to_owned(),
            key_column: shape.key_column().as_str().to_owned(),
            skip_empty_key: shape.skip_empty_key(),
            trim_key: shape.trim_key(),
            duplicate_key_policy: shape.duplicate_key_policy(),
        }),
        source_row_field: shape
            .source_row_field()
            .map(|field| field.as_str().to_owned()),
        source_row_method: shape
            .source_row_method()
            .map(|method| method.as_str().to_owned()),
        row_filters: Vec::new(),
        fields: record_fields(shape.fields())?,
        lookup_methods: shape
            .methods()
            .iter()
            .map(|method| SemanticLookupMethod {
                name: method.name().as_str().to_owned(),
                parameter: method.parameter().as_str().to_owned(),
                kind: SemanticLookupKind::StringKey,
            })
            .collect(),
        ids_method: shape.ids_method().map(|method| method.as_str().to_owned()),
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn one_table_string_record(
    manager_name: &str,
    shape: &NativeOneTableStringKeyProjectionManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.data_type().as_str().to_owned(),
        tables: one_table(shape.table_name().as_str(), shape.row_type_name().as_str()),
        key: Some(SemanticManagerKey::String {
            key_field: shape.key_field().as_str().to_owned(),
            key_column: shape.key_column().as_str().to_owned(),
            skip_empty_key: shape.skip_empty_key(),
            duplicate_key_policy: shape.duplicate_key_policy(),
        }),
        source_row_field: None,
        source_row_method: None,
        row_filters: Vec::new(),
        fields: record_fields(shape.fields())?,
        lookup_methods: string_lookup_methods(shape.methods()),
        ids_method: None,
        rows_method: None,
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn one_table_row_record(
    manager_name: &str,
    shape: &NativeOneTableRowProjectionManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.data_type().as_str().to_owned(),
        tables: one_table(shape.table_name().as_str(), shape.row_type_name().as_str()),
        key: None,
        source_row_field: shape
            .source_row_field()
            .map(|field| field.as_str().to_owned()),
        source_row_method: shape
            .source_row_method()
            .map(|method| method.as_str().to_owned()),
        row_filters: Vec::new(),
        fields: record_fields(shape.fields())?,
        lookup_methods: Vec::new(),
        ids_method: None,
        rows_method: shape.rows_method().map(|method| method.as_str().to_owned()),
        len_method: shape.len_method().map(|method| method.as_str().to_owned()),
        is_empty_method: shape
            .is_empty_method()
            .map(|method| method.as_str().to_owned()),
    })
}

fn one_table(table_name: &str, row_type_name: &str) -> Vec<SemanticManagerTable> {
    vec![SemanticManagerTable {
        table_name: table_name.to_owned(),
        row_type_name: row_type_name.to_owned(),
    }]
}

fn family_tables(tables: &[NativeTableFamilyTable]) -> Vec<SemanticManagerTable> {
    tables
        .iter()
        .map(|table| SemanticManagerTable {
            table_name: table.table_name().as_str().to_owned(),
            row_type_name: table.row_type_name().as_str().to_owned(),
        })
        .collect()
}

fn manager_input_tables(manager: &NativeManagerSpec) -> Vec<SemanticManagerTable> {
    manager
        .inputs()
        .iter()
        .filter_map(|input| match input {
            NativeManagerInput::Table(table) => Some(SemanticManagerTable {
                table_name: table.table_name().as_str().to_owned(),
                row_type_name: table.row_type_name().as_str().to_owned(),
            }),
            NativeManagerInput::Product(_) | NativeManagerInput::Manager(_) => None,
        })
        .collect()
}

fn direct_manager_tables(manager: &NativeManagerSpec) -> Vec<DirectManagerTable> {
    manager
        .inputs()
        .iter()
        .filter_map(|input| match input {
            NativeManagerInput::Table(table) => Some(DirectManagerTable {
                table_name: table.table_name().as_str().to_owned(),
                row_type_name: table.row_type_name().as_str().to_owned(),
            }),
            NativeManagerInput::Product(_) | NativeManagerInput::Manager(_) => None,
        })
        .collect()
}

fn crc_index_field_name(method: Option<&crate::symbols::RustIdentifier>) -> String {
    method
        .map(|method| method.as_str().to_owned())
        .unwrap_or_else(|| "crc".to_owned())
}

fn semantic_numeric_key_type(key_type: NativeNumericKeyType) -> SemanticNumericKeyType {
    match key_type {
        NativeNumericKeyType::NonZeroU8 => SemanticNumericKeyType::U8,
        NativeNumericKeyType::U16 | NativeNumericKeyType::U16FromNonZeroU32 => {
            SemanticNumericKeyType::U16
        }
        NativeNumericKeyType::NonZeroU32 => SemanticNumericKeyType::U32,
    }
}

fn row_filters(
    filters: &[NativeCrcProjectionRowFilter],
    fields: &[NativeProjectionField],
) -> Result<Vec<SemanticRowFilter>> {
    filters
        .iter()
        .map(|filter| {
            let predicate = match filter.predicate() {
                NativeCrcProjectionRowFilterPredicate::BoolTrueWhenPresent => {
                    SemanticRowFilterPredicate::BoolTrueWhenPresent
                }
                NativeCrcProjectionRowFilterPredicate::BoolMustBeTrue => {
                    SemanticRowFilterPredicate::BoolMustBeTrue
                }
                NativeCrcProjectionRowFilterPredicate::F32GreaterThanOrEqualZero => {
                    SemanticRowFilterPredicate::F32GreaterThanOrEqualZero
                }
                NativeCrcProjectionRowFilterPredicate::F32LessThanOrEqualZero => {
                    SemanticRowFilterPredicate::F32LessThanOrEqualZero
                }
                NativeCrcProjectionRowFilterPredicate::F32AnyGreaterThanZero => {
                    SemanticRowFilterPredicate::F32AnyGreaterThanZero
                }
                NativeCrcProjectionRowFilterPredicate::I32LessThanOrEqualZero => {
                    SemanticRowFilterPredicate::I32LessThanOrEqualZero
                }
                NativeCrcProjectionRowFilterPredicate::LowercaseCrcStringNonZero => {
                    SemanticRowFilterPredicate::LowercaseCrcStringNonZero
                }
                NativeCrcProjectionRowFilterPredicate::StringNotEqualToColumn => {
                    SemanticRowFilterPredicate::StringNotEqualToColumn
                }
            };
            let compare_column = filter
                .compare_getter()
                .map(|getter| column_for_getter(fields, getter.as_str()))
                .transpose()
                .with_context(|| {
                    format!("row filter `{}` compare getter", filter.column().as_str())
                })?;
            let extra_columns = filter
                .extra_getters()
                .iter()
                .map(|getter| column_for_getter(fields, getter.as_str()))
                .collect::<Result<Vec<_>>>()
                .with_context(|| {
                    format!("row filter `{}` extra getters", filter.column().as_str())
                })?;
            if matches!(
                predicate,
                SemanticRowFilterPredicate::StringNotEqualToColumn
            ) && compare_column.is_none()
            {
                bail!(
                    "row filter `{}` needs a compare getter",
                    filter.column().as_str()
                );
            }
            Ok(SemanticRowFilter {
                column: filter.column().as_str().to_owned(),
                predicate,
                compare_column,
                extra_columns,
            })
        })
        .collect()
}

fn column_for_getter(fields: &[NativeProjectionField], getter: &str) -> Result<String> {
    fields
        .iter()
        .find(|field| field.getter().as_str() == getter)
        .map(|field| field.column().as_str().to_owned())
        .with_context(|| format!("projection getter `{getter}` has no source column"))
}

fn record_fields(fields: &[NativeProjectionField]) -> Result<Vec<SemanticRecordField>> {
    fields
        .iter()
        .map(|field| {
            let transform = semantic_transform(field).with_context(|| {
                format!(
                    "field `{}` column `{}`",
                    field.field().as_str(),
                    field.column().as_str()
                )
            })?;
            Ok(SemanticRecordField {
                name: standalone_projection_field_name(field),
                column: field.column().as_str().to_owned(),
                transform,
                value_type: field.value_type().map(|value| value.as_str().to_owned()),
                default_value: field.default_value().map(|value| value.as_str().to_owned()),
                reference_field: field
                    .reference_field()
                    .map(|value| value.as_str().to_owned()),
                u16_max_exclusive: field.u16_max_exclusive(),
                enum_shape: None,
                pair_first_enum_shape: None,
            })
        })
        .collect()
}

fn standalone_projection_field_name(field: &NativeProjectionField) -> String {
    let name = field.field().as_str();
    match field.transform() {
        NativeProjectionTransform::ForeignKeyRow
        | NativeProjectionTransform::OptionalForeignKeyRow => name
            .strip_suffix("_row")
            .map_or_else(|| name.to_owned(), |prefix| format!("{prefix}_key")),
        NativeProjectionTransform::ForeignKeyRowList
        | NativeProjectionTransform::OptionalForeignKeyRowListDefaultEmpty => name
            .strip_suffix("_rows")
            .map_or_else(|| name.to_owned(), |prefix| format!("{prefix}_keys")),
        _ => name.to_owned(),
    }
}

fn semantic_transform(field: &NativeProjectionField) -> Result<SemanticProjectionTransform> {
    let transform = field.transform();
    Ok(match transform {
        NativeProjectionTransform::String | NativeProjectionTransform::ForeignKeyTargetKey => {
            SemanticProjectionTransform::String
        }
        NativeProjectionTransform::NonEmptyString => SemanticProjectionTransform::NonEmptyString,
        NativeProjectionTransform::EnumString => SemanticProjectionTransform::EnumString,
        NativeProjectionTransform::EnumStringSkipInvalid => {
            SemanticProjectionTransform::EnumStringSkipInvalid
        }
        NativeProjectionTransform::EnumStringRejectDefault => {
            SemanticProjectionTransform::EnumStringRejectDefault
        }
        NativeProjectionTransform::TypedCell
        | NativeProjectionTransform::OptionalTypedCellDefaultValue => {
            semantic_typed_cell_transform(field)?
        }
        NativeProjectionTransform::OptionalTypedCell => {
            semantic_optional_typed_cell_transform(field)?
        }
        NativeProjectionTransform::U8Enum => SemanticProjectionTransform::EnumString,
        NativeProjectionTransform::OptionalU8EnumDefaultValue => {
            SemanticProjectionTransform::EnumDefault
        }
        NativeProjectionTransform::OptionalStringDefaultEmpty => {
            SemanticProjectionTransform::StringDefaultEmpty
        }
        NativeProjectionTransform::PlusJoinedList => SemanticProjectionTransform::PlusJoinedList,
        NativeProjectionTransform::OptionalString => SemanticProjectionTransform::OptionalString,
        NativeProjectionTransform::OptionalFirstString => {
            SemanticProjectionTransform::OptionalFirstString
        }
        NativeProjectionTransform::StringList => SemanticProjectionTransform::StringList,
        NativeProjectionTransform::NonEmptyStringList => {
            SemanticProjectionTransform::NonEmptyStringList
        }
        NativeProjectionTransform::OptionalStringList => {
            SemanticProjectionTransform::OptionalStringList
        }
        NativeProjectionTransform::Bool => SemanticProjectionTransform::Bool,
        NativeProjectionTransform::OptionalBool => SemanticProjectionTransform::OptionalBool,
        NativeProjectionTransform::OptionalBoolDefaultFalse => {
            SemanticProjectionTransform::BoolDefaultFalse
        }
        NativeProjectionTransform::Crc32NonZeroBool => {
            SemanticProjectionTransform::Crc32NonZeroBool
        }
        NativeProjectionTransform::U8 => SemanticProjectionTransform::U8,
        NativeProjectionTransform::OptionalU8DefaultZero => {
            SemanticProjectionTransform::U8DefaultZero
        }
        NativeProjectionTransform::OptionalU8DefaultMax => {
            SemanticProjectionTransform::U8DefaultMax
        }
        NativeProjectionTransform::U32ToU16BelowMax => SemanticProjectionTransform::U16BelowMax,
        NativeProjectionTransform::U32 => SemanticProjectionTransform::U32,
        NativeProjectionTransform::OptionalU32 => SemanticProjectionTransform::OptionalU32,
        NativeProjectionTransform::OptionalU32DefaultZero => {
            SemanticProjectionTransform::U32DefaultZero
        }
        NativeProjectionTransform::NonZeroU32 => SemanticProjectionTransform::NonZeroU32,
        NativeProjectionTransform::OptionalNonZeroU32 => {
            SemanticProjectionTransform::OptionalNonZeroU32
        }
        NativeProjectionTransform::I32 => SemanticProjectionTransform::I32,
        NativeProjectionTransform::F32 => SemanticProjectionTransform::F32,
        NativeProjectionTransform::F32MinutesToSeconds => {
            SemanticProjectionTransform::F32MinutesToSeconds
        }
        NativeProjectionTransform::F32UpperBound10000ZeroIsDefault => {
            SemanticProjectionTransform::F32UpperBound10000ZeroIsDefault
        }
        NativeProjectionTransform::F32LowerBound10000CappedToField => {
            SemanticProjectionTransform::F32LowerBound10000CappedToField
        }
        NativeProjectionTransform::OptionalF32 => SemanticProjectionTransform::OptionalF32,
        NativeProjectionTransform::F32ListDefaultEmpty => SemanticProjectionTransform::F32List,
        NativeProjectionTransform::I32ListDefaultEmpty => SemanticProjectionTransform::I32List,
        NativeProjectionTransform::Crc32 => SemanticProjectionTransform::Crc32,
        NativeProjectionTransform::LowercaseCrcString
        | NativeProjectionTransform::ForeignKeyTargetLowercaseCrc => {
            SemanticProjectionTransform::LowercaseCrcString
        }
        NativeProjectionTransform::OptionalLowercaseCrcStringDefaultZero => {
            SemanticProjectionTransform::LowercaseCrcStringDefaultZero
        }
        NativeProjectionTransform::OptionalFirstLowercaseCrcStringDefaultZero => {
            SemanticProjectionTransform::FirstLowercaseCrcStringDefaultZero
        }
        NativeProjectionTransform::OptionalTrimmedLowercaseCrcStringDefaultZero => {
            SemanticProjectionTransform::TrimmedLowercaseCrcStringDefaultZero
        }
        NativeProjectionTransform::OptionalCrc32 => SemanticProjectionTransform::OptionalCrc32,
        NativeProjectionTransform::OptionalCrc32ZeroAsNone => {
            SemanticProjectionTransform::OptionalCrc32ZeroAsNone
        }
        NativeProjectionTransform::OptionalLowercaseCrcString => {
            SemanticProjectionTransform::OptionalLowercaseCrcString
        }
        NativeProjectionTransform::OptionalTrimmedLowercaseCrcString => {
            SemanticProjectionTransform::OptionalTrimmedLowercaseCrcString
        }
        NativeProjectionTransform::CrcList
        | NativeProjectionTransform::OptionalCrcListDefaultEmpty => {
            SemanticProjectionTransform::Crc32List
        }
        NativeProjectionTransform::LowercaseCrcStringList
        | NativeProjectionTransform::TrimmedLowercaseCrcStringList => {
            SemanticProjectionTransform::LowercaseCrcStringList
        }
        NativeProjectionTransform::ForeignKeyRow => SemanticProjectionTransform::ForeignKey,
        NativeProjectionTransform::OptionalForeignKeyRow => {
            SemanticProjectionTransform::OptionalForeignKey
        }
        NativeProjectionTransform::ForeignKeyRowList
        | NativeProjectionTransform::OptionalForeignKeyRowListDefaultEmpty => {
            SemanticProjectionTransform::ForeignKeyList
        }
        NativeProjectionTransform::F32RangeInclusive => {
            SemanticProjectionTransform::F32RangeInclusive
        }
        NativeProjectionTransform::U32RangeInclusive => {
            SemanticProjectionTransform::U32RangeInclusive
        }
    })
}

fn semantic_typed_cell_transform(
    field: &NativeProjectionField,
) -> Result<SemanticProjectionTransform> {
    let value_type = field
        .value_type()
        .context("typed-cell transform has no value type")?
        .as_str();
    let leaf = value_type.rsplit("::").next().unwrap_or(value_type);
    match leaf {
        "u8" => Ok(SemanticProjectionTransform::U8),
        "NonZeroU8" => Ok(SemanticProjectionTransform::NonZeroU8),
        "u16" => Ok(SemanticProjectionTransform::U16),
        "NonZeroU16" => Ok(SemanticProjectionTransform::NonZeroU16),
        "u32" => Ok(SemanticProjectionTransform::U32),
        "NonZeroU32" => Ok(SemanticProjectionTransform::NonZeroU32),
        "i32" => Ok(SemanticProjectionTransform::I32),
        "f32" => Ok(SemanticProjectionTransform::F32),
        "String" => Ok(SemanticProjectionTransform::String),
        _ => bail!("unsupported standalone typed-cell value type `{value_type}`"),
    }
}

fn semantic_optional_typed_cell_transform(
    field: &NativeProjectionField,
) -> Result<SemanticProjectionTransform> {
    let value_type = field
        .value_type()
        .context("optional typed-cell transform has no value type")?
        .as_str();
    match value_type {
        "Vec<(az_core::crc::Crc32, f32)>" => {
            Ok(SemanticProjectionTransform::OptionalCrc32F32PairList)
        }
        "Vec<(u8, f32)>" => Ok(SemanticProjectionTransform::OptionalU8F32PairList),
        _ => bail!("unsupported standalone optional typed-cell value type `{value_type}`"),
    }
}

fn crc_lookup_methods(methods: &[NativeCrcIndexLookupMethod]) -> Vec<SemanticLookupMethod> {
    methods
        .iter()
        .map(|method| {
            let parameter = method.parameter();
            let kind = match parameter.kind() {
                NativeCrcIndexLookupParameterKind::StrRef
                | NativeCrcIndexLookupParameterKind::AsRefStr => SemanticLookupKind::CrcStringKey,
                NativeCrcIndexLookupParameterKind::Crc32 => SemanticLookupKind::CrcKey,
                NativeCrcIndexLookupParameterKind::IntoCrc32 => SemanticLookupKind::IntoCrcKey,
            };
            SemanticLookupMethod {
                name: method.name().as_str().to_owned(),
                parameter: parameter.name().as_str().to_owned(),
                kind,
            }
        })
        .collect()
}

fn numeric_lookup_methods(methods: &[NativeNumericLookupMethod]) -> Vec<SemanticLookupMethod> {
    methods
        .iter()
        .map(|method| {
            let kind = match method.parameter_kind() {
                NativeNumericLookupParameterKind::NonZeroU8 => {
                    SemanticLookupKind::NumericKey(SemanticNumericKeyType::U8)
                }
                NativeNumericLookupParameterKind::U16 => {
                    SemanticLookupKind::NumericKey(SemanticNumericKeyType::U16)
                }
                NativeNumericLookupParameterKind::NonZeroU32 => {
                    SemanticLookupKind::NumericKey(SemanticNumericKeyType::U32)
                }
            };
            SemanticLookupMethod {
                name: method.name().as_str().to_owned(),
                parameter: method.parameter().as_str().to_owned(),
                kind,
            }
        })
        .collect()
}

fn string_lookup_methods(methods: &[NativeStringLookupMethod]) -> Vec<SemanticLookupMethod> {
    methods
        .iter()
        .map(|method| SemanticLookupMethod {
            name: method.name().as_str().to_owned(),
            parameter: method.parameter().as_str().to_owned(),
            kind: SemanticLookupKind::StringKey,
        })
        .collect()
}

fn record_codegen_item(record: &SemanticManagerRecord) -> SerializeCodegenItem {
    let source_name = format!("NewWorld::GameData::{}", record.record_type_name);
    let type_id = deterministic_uuid(&source_name);
    let mut fields = Vec::new();

    if let Some(source_row_field) = &record.source_row_field {
        fields.push(record_codegen_field(
            &source_name,
            source_row_field,
            ResolvedType::Scalar(ScalarType::U32),
        ));
    }
    if let Some(key) = &record.key {
        push_key_codegen_fields(&source_name, key, &mut fields);
    }
    fields.extend(record.fields.iter().map(|field| {
        record_codegen_field(&source_name, &field.name, resolved_type_for_field(field))
    }));

    SerializeCodegenItem {
        source_type_id: type_id,
        source_name,
        role: ReflectedTypeRole::SupportType,
        is_reflection_marker: false,
        is_abstract: Some(false),
        factory: None,
        rtti_base_chain: Vec::new(),
        kind: SerializeCodegenItemKind::Struct,
        enum_underlying_type: None,
        fields,
        variants: Vec::new(),
    }
}

fn enum_codegen_item(shape: &GameSystemEnumShape) -> SerializeCodegenItem {
    SerializeCodegenItem {
        source_type_id: semantic_enum_type_id(&shape.name),
        source_name: shape.name.clone(),
        role: ReflectedTypeRole::SupportType,
        is_reflection_marker: false,
        is_abstract: Some(false),
        factory: None,
        rtti_base_chain: Vec::new(),
        kind: SerializeCodegenItemKind::Enum,
        enum_underlying_type: Some(ResolvedType::Scalar(match shape.representation {
            GameSystemEnumRepresentation::U8 => ScalarType::U8,
            GameSystemEnumRepresentation::I32 => ScalarType::I32,
            GameSystemEnumRepresentation::U32 | GameSystemEnumRepresentation::Crc32 => {
                ScalarType::U32
            }
        })),
        fields: Vec::new(),
        variants: shape
            .variants
            .iter()
            .map(|variant| SerializeCodegenVariant {
                source_name: variant.name.clone(),
                value_u64: u64::try_from(variant.discriminant).ok(),
                value_u32: u32::try_from(variant.discriminant).ok(),
                value_i32: i32::try_from(variant.discriminant).ok(),
            })
            .collect(),
    }
}

fn semantic_enum_type_id(name: &str) -> Uuid {
    deterministic_uuid(&format!("NewWorld::GameData::Enum::{name}"))
}

fn push_key_codegen_fields(
    source_name: &str,
    key: &SemanticManagerKey,
    fields: &mut Vec<SerializeCodegenField>,
) {
    match key {
        SemanticManagerKey::Crc {
            key_field,
            crc_field,
            ..
        } => {
            fields.push(record_codegen_field(
                source_name,
                key_field,
                ResolvedType::Scalar(ScalarType::String),
            ));
            fields.push(record_codegen_field(
                source_name,
                crc_field,
                ResolvedType::Scalar(ScalarType::Crc32),
            ));
        }
        SemanticManagerKey::FallbackCrc {
            key_kind_field,
            key_field,
            crc_field,
            ..
        } => {
            fields.push(record_codegen_field(
                source_name,
                key_kind_field,
                ResolvedType::Scalar(ScalarType::String),
            ));
            fields.push(record_codegen_field(
                source_name,
                key_field,
                ResolvedType::Scalar(ScalarType::String),
            ));
            fields.push(record_codegen_field(
                source_name,
                crc_field,
                ResolvedType::Scalar(ScalarType::Crc32),
            ));
        }
        SemanticManagerKey::Numeric {
            key_field,
            key_type,
            ..
        } => {
            fields.push(record_codegen_field(
                source_name,
                key_field,
                resolved_type_for_numeric_key(*key_type),
            ));
        }
        SemanticManagerKey::EnumString { key_field, .. }
        | SemanticManagerKey::String { key_field, .. } => {
            fields.push(record_codegen_field(
                source_name,
                key_field,
                ResolvedType::Scalar(ScalarType::String),
            ));
        }
    }
}

fn record_codegen_field(
    owner_source_name: &str,
    source_name: &str,
    resolved_type: ResolvedType,
) -> SerializeCodegenField {
    SerializeCodegenField {
        source_name: source_name.to_owned(),
        source_type_id: deterministic_uuid(&format!("{owner_source_name}::{source_name}")),
        resolved_type,
        data_size: None,
        offset: None,
        flags: None,
        is_base_class: false,
        is_pointer: false,
        is_dynamic_field: false,
    }
}

fn resolved_type_for_field(field: &SemanticRecordField) -> ResolvedType {
    let transform = field.transform;
    match transform {
        SemanticProjectionTransform::String
        | SemanticProjectionTransform::NonEmptyString
        | SemanticProjectionTransform::StringDefaultEmpty
        | SemanticProjectionTransform::PlusJoinedList
        | SemanticProjectionTransform::ForeignKey => ResolvedType::Scalar(ScalarType::String),
        SemanticProjectionTransform::EnumString
        | SemanticProjectionTransform::EnumStringSkipInvalid
        | SemanticProjectionTransform::EnumStringRejectDefault
        | SemanticProjectionTransform::EnumDefault => {
            let shape = field
                .enum_shape
                .as_ref()
                .expect("enum projection fields have reconciled enum schemas");
            ResolvedType::Named {
                type_id: semantic_enum_type_id(&shape.name),
                source_name: shape.name.clone(),
            }
        }
        SemanticProjectionTransform::OptionalString
        | SemanticProjectionTransform::OptionalFirstString
        | SemanticProjectionTransform::OptionalForeignKey => optional(ScalarType::String),
        SemanticProjectionTransform::StringList
        | SemanticProjectionTransform::NonEmptyStringList
        | SemanticProjectionTransform::ForeignKeyList => vector(ScalarType::String),
        SemanticProjectionTransform::OptionalStringList => ResolvedType::Optional {
            value: Box::new(vector(ScalarType::String)),
        },
        SemanticProjectionTransform::Bool
        | SemanticProjectionTransform::BoolDefaultFalse
        | SemanticProjectionTransform::Crc32NonZeroBool => ResolvedType::Scalar(ScalarType::Bool),
        SemanticProjectionTransform::OptionalBool => optional(ScalarType::Bool),
        SemanticProjectionTransform::U8
        | SemanticProjectionTransform::NonZeroU8
        | SemanticProjectionTransform::U8DefaultZero
        | SemanticProjectionTransform::U8DefaultMax => ResolvedType::Scalar(ScalarType::U8),
        SemanticProjectionTransform::U16
        | SemanticProjectionTransform::NonZeroU16
        | SemanticProjectionTransform::U16BelowMax => ResolvedType::Scalar(ScalarType::U16),
        SemanticProjectionTransform::U32
        | SemanticProjectionTransform::U32DefaultZero
        | SemanticProjectionTransform::NonZeroU32 => ResolvedType::Scalar(ScalarType::U32),
        SemanticProjectionTransform::OptionalU32
        | SemanticProjectionTransform::OptionalNonZeroU32 => optional(ScalarType::U32),
        SemanticProjectionTransform::Crc32
        | SemanticProjectionTransform::LowercaseCrcString
        | SemanticProjectionTransform::LowercaseCrcStringDefaultZero
        | SemanticProjectionTransform::FirstLowercaseCrcStringDefaultZero
        | SemanticProjectionTransform::TrimmedLowercaseCrcStringDefaultZero => {
            ResolvedType::Scalar(ScalarType::Crc32)
        }
        SemanticProjectionTransform::OptionalCrc32
        | SemanticProjectionTransform::OptionalCrc32ZeroAsNone
        | SemanticProjectionTransform::OptionalLowercaseCrcString
        | SemanticProjectionTransform::OptionalTrimmedLowercaseCrcString => {
            optional(ScalarType::Crc32)
        }
        SemanticProjectionTransform::I32 => ResolvedType::Scalar(ScalarType::I32),
        SemanticProjectionTransform::F32
        | SemanticProjectionTransform::F32MinutesToSeconds
        | SemanticProjectionTransform::F32UpperBound10000ZeroIsDefault
        | SemanticProjectionTransform::F32LowerBound10000CappedToField => {
            ResolvedType::Scalar(ScalarType::F32)
        }
        SemanticProjectionTransform::OptionalF32 => optional(ScalarType::F32),
        SemanticProjectionTransform::F32List => vector(ScalarType::F32),
        SemanticProjectionTransform::I32List => vector(ScalarType::I32),
        SemanticProjectionTransform::Crc32List
        | SemanticProjectionTransform::LowercaseCrcStringList => vector(ScalarType::Crc32),
        SemanticProjectionTransform::F32RangeInclusive => pair(ScalarType::F32),
        SemanticProjectionTransform::U32RangeInclusive => pair(ScalarType::U32),
        SemanticProjectionTransform::OptionalCrc32F32PairList => {
            optional_pair_list(ScalarType::Crc32, ScalarType::F32)
        }
        SemanticProjectionTransform::OptionalU8F32PairList => {
            optional_pair_list(ScalarType::U8, ScalarType::F32)
        }
    }
}

fn resolved_type_for_numeric_key(key_type: SemanticNumericKeyType) -> ResolvedType {
    match key_type {
        SemanticNumericKeyType::U8 => ResolvedType::Scalar(ScalarType::U8),
        SemanticNumericKeyType::U16 => ResolvedType::Scalar(ScalarType::U16),
        SemanticNumericKeyType::U32 => ResolvedType::Scalar(ScalarType::U32),
    }
}

fn optional(scalar: ScalarType) -> ResolvedType {
    ResolvedType::Optional {
        value: Box::new(ResolvedType::Scalar(scalar)),
    }
}

fn vector(scalar: ScalarType) -> ResolvedType {
    ResolvedType::Sequence {
        kind: SequenceKind::Vector,
        element: Box::new(ResolvedType::Scalar(scalar)),
        capacity: None,
    }
}

fn pair(scalar: ScalarType) -> ResolvedType {
    ResolvedType::Pair {
        first: Box::new(ResolvedType::Scalar(scalar)),
        second: Box::new(ResolvedType::Scalar(scalar)),
    }
}

fn optional_pair_list(first: ScalarType, second: ScalarType) -> ResolvedType {
    ResolvedType::Optional {
        value: Box::new(ResolvedType::Sequence {
            kind: SequenceKind::Vector,
            element: Box::new(ResolvedType::Pair {
                first: Box::new(ResolvedType::Scalar(first)),
                second: Box::new(ResolvedType::Scalar(second)),
            }),
            capacity: None,
        }),
    }
}

fn deterministic_uuid(value: &str) -> Uuid {
    let digest = Sha256::digest(value.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn semantic_type_name(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::manager::validated_native_manager_specs;

    use super::*;

    fn composition_surface(name: &str, dependencies: &[&str]) -> ManagerSurface {
        ManagerSurface::Composition(CompositionManagerSurface {
            manager_name: name.to_owned(),
            manager_class_name: name.to_owned(),
            kind: CompositionManagerKind::ReplicationData,
            dependencies: dependencies
                .iter()
                .map(|dependency| (*dependency).to_owned())
                .collect(),
        })
    }

    #[test]
    fn manager_dependencies_require_generated_surfaces() {
        let error = topologically_order_manager_surfaces(vec![composition_surface(
            "DependentManager",
            &["MissingManager"],
        )])
        .expect_err("missing manager dependency must fail generation");

        assert!(
            error
                .to_string()
                .contains("DependentManager <- MissingManager")
        );
    }

    #[test]
    fn manager_dependencies_are_emitted_before_consumers() {
        let ordered = topologically_order_manager_surfaces(vec![
            composition_surface("DependentManager", &["DependencyManager"]),
            composition_surface("DependencyManager", &[]),
        ])
        .expect("acyclic manager dependencies");

        assert_eq!(manager_surface_name(&ordered[0]), "DependencyManager");
        assert_eq!(manager_surface_name(&ordered[1]), "DependentManager");
    }

    #[test]
    fn manager_dependency_cycles_fail_generation() {
        let error = topologically_order_manager_surfaces(vec![
            composition_surface("FirstManager", &["SecondManager"]),
            composition_surface("SecondManager", &["FirstManager"]),
        ])
        .expect_err("manager dependency cycle must fail generation");

        assert!(error.to_string().contains("manager dependency cycle"));
    }

    #[test]
    fn every_declared_product_is_present_on_its_generated_manager_surface() {
        let managers = validated_native_manager_specs();
        let surfaces = manager_surfaces_from_managers(&managers).expect("manager surfaces");

        for manager in &managers {
            let expected = manager
                .inputs()
                .iter()
                .filter(|input| matches!(input, NativeManagerInput::Product(_)))
                .count();
            if expected == 0 {
                continue;
            }
            let name = semantic_type_name(manager.rust_type().as_str());
            let actual = surfaces
                .iter()
                .find(|surface| manager_surface_name(surface) == name)
                .map(|surface| match surface {
                    ManagerSurface::Direct(manager)
                    | ManagerSurface::Native { manager, .. }
                    | ManagerSurface::ProductBacked(manager) => manager.products.len(),
                    ManagerSurface::Semantic(_)
                    | ManagerSurface::ItemData(_)
                    | ManagerSurface::Composition(_) => 0,
                })
                .expect("validated managers have generated surfaces");
            assert_eq!(
                actual, expected,
                "manager `{name}` dropped declared product inputs"
            );
        }
    }

    #[test]
    fn go_names_use_the_serialize_emitter_initialism_policy() {
        assert_eq!(go_field_name("event_id_crc32"), "EventIDCRC32");
        assert_eq!(go_field_name("asset_uuid"), "AssetUUID");
        assert_eq!(go_method_name("ui_data_by_id"), "UIDataByID");
    }

    #[test]
    fn standalone_projection_ir_preserves_manager_semantics() {
        let managers = validated_native_manager_specs();
        let surfaces = manager_surfaces_from_managers(&managers).expect("manager surfaces");
        let leaderboard_rewards = surfaces
            .iter()
            .find_map(|surface| match surface {
                ManagerSurface::Semantic(record)
                    if record.manager_name == "LeaderboardRewardsDataManager" =>
                {
                    Some(record)
                }
                _ => None,
            })
            .expect("LeaderboardRewardsDataManager semantic surface");
        assert_eq!(
            leaderboard_rewards
                .fields
                .iter()
                .find(|field| field.column == "Rotation")
                .expect("Rotation projection")
                .transform,
            SemanticProjectionTransform::EnumString
        );

        let mission_weights = surfaces
            .iter()
            .find_map(|surface| match surface {
                ManagerSurface::Semantic(record)
                    if record.manager_name == "MissionWeightsDataManager" =>
                {
                    Some(record)
                }
                _ => None,
            })
            .expect("MissionWeightsDataManager semantic surface");
        assert_eq!(
            mission_weights
                .fields
                .iter()
                .find(|field| field.column == "MissionGoalType")
                .expect("MissionGoalType projection")
                .transform,
            SemanticProjectionTransform::EnumStringSkipInvalid
        );
    }

    #[test]
    fn semantic_records_include_every_unrepresented_source_column() {
        let mut record = SemanticManagerRecord {
            manager_name: "ExampleDataManager".to_owned(),
            manager_class_name: "ExampleDataManager".to_owned(),
            record_type_name: "ExampleData".to_owned(),
            tables: one_table("ExampleData", "ExampleData"),
            key: Some(SemanticManagerKey::Crc {
                key_field: "item_key".to_owned(),
                crc_field: "item_id".to_owned(),
                key_column: "ItemID".to_owned(),
                skip_empty_key: true,
                trim_key: false,
                reject_zero_crc: false,
                duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
            }),
            source_row_field: None,
            source_row_method: None,
            row_filters: Vec::new(),
            fields: vec![SemanticRecordField {
                name: "name".to_owned(),
                column: "Name".to_owned(),
                transform: SemanticProjectionTransform::OptionalString,
                value_type: None,
                default_value: None,
                reference_field: None,
                u16_max_exclusive: None,
                enum_shape: None,
                pair_first_enum_shape: None,
            }],
            lookup_methods: Vec::new(),
            ids_method: None,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        };
        let schema = GameSystemDataTablesSchemaReport {
            tables: vec![crate::game_system_schema::GameSystemTableSchema {
                table_name: "ExampleData".to_owned(),
                table_name_crc: Crc32::from_str_lower("ExampleData").value(),
                row_type_name: "ExampleData".to_owned(),
                row_type_crc: Crc32::from_str_lower("ExampleData").value(),
                row_count: 1,
                sources: vec!["example.datasheet".to_owned()],
                columns: vec![
                    test_source_column("ItemID", ColumnType::String, true),
                    test_source_column("Name", ColumnType::String, false),
                    test_source_column("Hidden", ColumnType::Boolean, false),
                    test_source_column("BaseDamage", ColumnType::Number, false),
                ],
            }],
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        };

        append_unrepresented_schema_fields(&mut record, &schema);

        assert_eq!(
            record
                .fields
                .iter()
                .filter(|field| field.column == "Name")
                .count(),
            1
        );
        assert!(record.fields.iter().all(|field| field.column != "ItemID"));
        assert!(record.fields.iter().any(|field| {
            field.name == "hidden" && field.transform == SemanticProjectionTransform::OptionalBool
        }));
        assert!(record.fields.iter().any(|field| {
            field.name == "base_damage"
                && field.transform == SemanticProjectionTransform::OptionalF32
        }));
    }

    fn test_source_column(
        name: &str,
        declared_type: ColumnType,
        row_key: bool,
    ) -> GameSystemColumnSchema {
        GameSystemColumnSchema {
            name: name.to_owned(),
            crc: Crc32::from_str_lower(name).value(),
            declared_type,
            row_key,
            required: row_key,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: true,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: None,
                foreign_keys: Vec::new(),
            },
        }
    }

    #[test]
    fn manager_surfaces_emit_direct_or_implemented_semantic_apis_without_selection_lists() {
        let managers = validated_native_manager_specs();
        let surfaces = manager_surfaces_from_managers(&managers).expect("manager surfaces");

        let emitted_names = surfaces
            .iter()
            .map(|surface| match surface {
                ManagerSurface::Direct(manager) => manager.manager_name.clone(),
                ManagerSurface::Native { manager, .. } => manager.manager_name.clone(),
                ManagerSurface::Semantic(manager) => manager.manager_name.clone(),
                ManagerSurface::ItemData(manager) => manager.manager_name.clone(),
                ManagerSurface::Composition(manager) => manager.manager_name.clone(),
                ManagerSurface::ProductBacked(manager) => manager.manager_name.clone(),
            })
            .collect::<BTreeSet<_>>();
        let planned_names = managers
            .iter()
            .map(|manager| semantic_type_name(manager.rust_type().as_str()).to_owned())
            .collect::<BTreeSet<_>>();
        let missing_names = planned_names
            .difference(&emitted_names)
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            missing_names.is_empty(),
            "validated manager shapes without generated surfaces: {}",
            missing_names.join(", ")
        );
        let degraded_native_apis = surfaces
            .iter()
            .filter_map(|surface| match surface {
                ManagerSurface::Direct(surface) => managers
                    .iter()
                    .find(|manager| {
                        semantic_type_name(manager.rust_type().as_str()) == surface.manager_name
                    })
                    .and_then(|manager| manager.shape())
                    .filter(|shape| shape.exposes_native_api())
                    .map(|_| surface.manager_name.clone()),
                ManagerSurface::Semantic(_)
                | ManagerSurface::Native { .. }
                | ManagerSurface::ItemData(_)
                | ManagerSurface::Composition(_)
                | ManagerSurface::ProductBacked(_) => None,
            })
            .collect::<Vec<_>>();
        assert!(
            degraded_native_apis.is_empty(),
            "native API managers degraded to direct table wrappers: {}",
            degraded_native_apis.join(", ")
        );
        assert!(!emitted_names.is_empty());
        for surface in surfaces.iter().filter_map(|surface| match surface {
            ManagerSurface::Direct(surface)
            | ManagerSurface::Native {
                manager: surface, ..
            }
            | ManagerSurface::ProductBacked(surface) => Some(surface),
            ManagerSurface::Semantic(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_) => None,
        }) {
            assert!(
                !surface.tables.is_empty() || !surface.products.is_empty(),
                "`{}` emitted an empty manager surface",
                surface.manager_name
            );
        }

        for surface in surfaces.iter().filter_map(|surface| match surface {
            ManagerSurface::ProductBacked(surface) => Some(surface),
            ManagerSurface::Direct(_)
            | ManagerSurface::Native { .. }
            | ManagerSurface::Semantic(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_) => None,
        }) {
            let manager = managers
                .iter()
                .find(|manager| {
                    semantic_type_name(manager.rust_type().as_str()) == surface.manager_name
                })
                .expect("product-backed surface manager exists in plan");
            assert!(
                manager
                    .shape()
                    .is_some_and(|shape| is_product_backed_surface(shape, &surface.products)),
                "`{}` product-backed surface should only be used for validated product-backed manager shapes",
                surface.manager_name
            );
        }
        let backstory = surfaces
            .iter()
            .find_map(|surface| match surface {
                ManagerSurface::Semantic(record)
                    if record.manager_name == "StaticBackstoryDataManager" =>
                {
                    Some(record)
                }
                _ => None,
            })
            .expect("StaticBackstoryDataManager semantic surface");
        let backstory_lookup = backstory
            .lookup_methods
            .iter()
            .find(|method| method.name == "backstory")
            .expect("backstory lookup");
        assert_eq!(backstory_lookup.kind, SemanticLookupKind::IntoCrcKey);
        let backstory_by_key_lookup = backstory
            .lookup_methods
            .iter()
            .find(|method| method.name == "backstory_by_key")
            .expect("backstory_by_key lookup");
        assert_eq!(
            backstory_by_key_lookup.kind,
            SemanticLookupKind::CrcStringKey
        );

        let mutation_perks = surfaces
            .iter()
            .find_map(|surface| match surface {
                ManagerSurface::Semantic(record)
                    if record.manager_name == "ElementalMutationPerksStaticDataManager" =>
                {
                    Some(record)
                }
                _ => None,
            })
            .expect("ElementalMutationPerksStaticDataManager semantic surface");
        let bucket2 = mutation_perks
            .fields
            .iter()
            .find(|field| field.column == "InjectedPerkBucket2")
            .expect("InjectedPerkBucket2 projection");
        assert_eq!(bucket2.name, "injected_perk_bucket2_key");
        assert_eq!(bucket2.transform, SemanticProjectionTransform::ForeignKey);
        assert!(
            mutation_perks
                .fields
                .iter()
                .all(|field| !field.name.ends_with("_row"))
        );

        let leaderboard_rewards = surfaces
            .iter()
            .find_map(|surface| match surface {
                ManagerSurface::Semantic(record)
                    if record.manager_name == "LeaderboardRewardsDataManager" =>
                {
                    Some(record)
                }
                _ => None,
            })
            .expect("LeaderboardRewardsDataManager semantic surface");
        assert_eq!(
            leaderboard_rewards
                .fields
                .iter()
                .find(|field| field.column == "Rotation")
                .expect("Rotation projection")
                .transform,
            SemanticProjectionTransform::EnumString
        );

        let mutation_difficulty = surfaces
            .iter()
            .find_map(|surface| match surface {
                ManagerSurface::Semantic(record)
                    if record.manager_name == "MutationDifficultyStaticDataManager" =>
                {
                    Some(record)
                }
                _ => None,
            })
            .expect("MutationDifficultyStaticDataManager semantic surface");
        assert_eq!(
            mutation_difficulty
                .fields
                .iter()
                .find(|field| field.column == "DifficultyTier")
                .expect("DifficultyTier projection")
                .transform,
            SemanticProjectionTransform::NonZeroU8
        );
    }
}
