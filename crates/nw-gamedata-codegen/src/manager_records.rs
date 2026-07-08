use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use nw_serialize_codegen::{
    ReflectedTypeRole, ResolvedType, ScalarType, SequenceKind, SerializeCodegenField,
    SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::compiler::GameDataCompileUnit;
use crate::manager::{
    ManagerContractInput, NativeComposedResourceArgument, NativeComposedResourceManager,
    NativeCrcIndexLookupMethod, NativeCrcIndexLookupParameterKind, NativeCrcProjectionRowFilter,
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
    Semantic(SemanticManagerRecord),
    ItemData(ItemDataManagerSurface),
    ProductBacked(DirectManagerSurface),
}

pub(crate) fn manager_surface_name(surface: &ManagerSurface) -> &str {
    match surface {
        ManagerSurface::Direct(manager) | ManagerSurface::ProductBacked(manager) => {
            &manager.manager_name
        }
        ManagerSurface::Semantic(manager) => &manager.manager_name,
        ManagerSurface::ItemData(manager) => &manager.manager_name,
    }
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
pub(crate) enum ManagerSurfaceDependency {
    Table { name: String, row: String },
    Asset { path: String },
    Manager { name: String },
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticProjectionTransform {
    String,
    StringDefaultEmpty,
    PlusJoinedList,
    OptionalString,
    StringList,
    NonEmptyStringList,
    OptionalStringList,
    Bool,
    OptionalBool,
    U8,
    U16,
    U32,
    OptionalU32,
    I32,
    F32,
    OptionalF32,
    F32List,
    I32List,
    Crc32,
    Crc32List,
    OptionalLowercaseCrcString,
    LowercaseCrcStringList,
    RowIndex,
    OptionalRowIndex,
    RowIndexList,
    F32RangeInclusive,
    U32RangeInclusive,
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
    manager_surfaces_from_managers(unit.codegen_plan_ref().managers().managers())
}

pub(crate) fn manager_surfaces_from_managers(
    managers: &[NativeManagerSpec],
) -> Result<Vec<ManagerSurface>> {
    managers
        .iter()
        .filter_map(|manager| manager_surface(manager).transpose())
        .collect()
}

pub(crate) fn manager_surface_dependencies(
    surface: &ManagerSurface,
    inputs: &[ManagerContractInput<'_>],
) -> Vec<ManagerSurfaceDependency> {
    let table_keys = surface_table_dependencies(surface);
    let table_names = surface_table_name_dependencies(surface);
    let asset_paths = surface_asset_dependencies(surface);

    inputs
        .iter()
        .filter_map(|input| match input {
            ManagerContractInput::Table {
                name,
                row,
                product_path: _,
            } => {
                let name = name.as_str();
                let row = row.as_str();
                (table_keys.contains(&(name.to_owned(), row.to_owned()))
                    || table_names.contains(name))
                .then(|| ManagerSurfaceDependency::Table {
                    name: name.to_owned(),
                    row: row.to_owned(),
                })
            }
            ManagerContractInput::Asset {
                path,
                asset_type: _,
            } => {
                let path = path.as_str();
                asset_paths
                    .contains(path)
                    .then(|| ManagerSurfaceDependency::Asset {
                        path: path.to_owned(),
                    })
            }
            ManagerContractInput::Manager { manager } => {
                let manager = semantic_type_name(manager.as_str()).to_owned();
                Some(ManagerSurfaceDependency::Manager { name: manager })
            }
        })
        .collect()
}

fn surface_table_dependencies(surface: &ManagerSurface) -> BTreeSet<(String, String)> {
    match surface {
        ManagerSurface::Direct(manager) => manager
            .tables
            .iter()
            .map(|table| (table.table_name.clone(), table.row_type_name.clone()))
            .collect(),
        ManagerSurface::Semantic(_)
        | ManagerSurface::ItemData(_)
        | ManagerSurface::ProductBacked(_) => BTreeSet::new(),
    }
}

fn surface_table_name_dependencies(surface: &ManagerSurface) -> BTreeSet<&str> {
    match surface {
        ManagerSurface::Semantic(record) => record
            .tables
            .iter()
            .map(|table| table.table_name.as_str())
            .collect(),
        ManagerSurface::ItemData(manager) => manager
            .tables
            .iter()
            .map(|table| table.table_name.as_str())
            .collect(),
        ManagerSurface::Direct(_) | ManagerSurface::ProductBacked(_) => BTreeSet::new(),
    }
}

fn surface_asset_dependencies(surface: &ManagerSurface) -> BTreeSet<&str> {
    match surface {
        ManagerSurface::Direct(manager) | ManagerSurface::ProductBacked(manager) => manager
            .products
            .iter()
            .map(|product| product.path.as_str())
            .collect(),
        ManagerSurface::Semantic(_) | ManagerSurface::ItemData(_) => BTreeSet::new(),
    }
}

pub(crate) fn semantic_manager_record_unit(
    records: &[SemanticManagerRecord],
) -> SerializeCodegenUnit {
    SerializeCodegenUnit {
        items: records.iter().map(record_codegen_item).collect(),
    }
}

pub(crate) fn ts_field_name(source_name: &str) -> String {
    lower_camel(source_name)
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
    let ident = upper_camel(source_name);
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
        return Ok(None);
    };
    if let NativeManagerShape::ItemData(shape) = shape {
        return Ok(Some(ManagerSurface::ItemData(item_data_manager_surface(
            manager_name,
            shape,
        ))));
    }
    if let Some(record) = semantic_manager_record(&manager_name, manager, shape) {
        return Ok(Some(ManagerSurface::Semantic(record?)));
    }
    let products = direct_products(manager, shape)?;
    if is_product_backed_surface(shape, &products) {
        return Ok(Some(ManagerSurface::ProductBacked(
            product_backed_manager_surface(manager_name, products),
        )));
    }
    let direct = direct_manager_surface(manager_name, manager, products);
    if direct.tables.is_empty() && direct.products.is_empty() {
        return Ok(None);
    }
    Ok(Some(ManagerSurface::Direct(direct)))
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
        .filter(|product| supported_product_value_type(product.value_type().as_str()))
        .map(|product| direct_product(manager, product))
        .collect()
}

fn supported_product_value_type(value_type: &str) -> bool {
    matches!(
        value_type,
        "newworld_plugin::assets::armor_offset_database::ArmorOffsetDatabase"
            | "newworld_plugin::assets::equip_types_database::EquipTypesDatabase"
            | "newworld_plugin::assets::game_debug_settings::GameDebugSettings"
            | "newworld_plugin::assets::player_base_attributes::PlayerBaseAttributes"
            | "newworld_plugin::assets::settlement_progression_data::SettlementProgressionData"
            | "newworld_plugin::assets::ui_database::UiDatabase"
            | "newworld_plugin::assets::camera_settings::GameCameraSettings"
            | "newworld_plugin::assets::gathering_database::GatheringDatabase"
            | "newworld_plugin::assets::gathering_database::GatheringActionDatabase"
            | "newworld_plugin::assets::crafting_station_database::CraftingStationDatabase"
            | "newworld_plugin::assets::rank_database::SocialRankDatabase"
    )
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

fn one_table_crc_index_record(
    manager_name: &str,
    shape: &NativeOneTableCrcIndexManager,
) -> Result<SemanticManagerRecord> {
    Ok(SemanticManagerRecord {
        manager_name: manager_name.to_owned(),
        manager_class_name: manager_name.to_owned(),
        record_type_name: shape.row_alias().as_str().to_owned(),
        tables: one_table(shape.table_name().as_str()),
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
        tables: one_table(shape.table_name().as_str()),
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
        tables: one_table(shape.table_name().as_str()),
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
        tables: one_table(shape.table_name().as_str()),
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
        tables: one_table(shape.table_name().as_str()),
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
        tables: one_table(shape.table_name().as_str()),
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
        tables: one_table(shape.table_name().as_str()),
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

fn one_table(table_name: &str) -> Vec<SemanticManagerTable> {
    vec![SemanticManagerTable {
        table_name: table_name.to_owned(),
    }]
}

fn family_tables(tables: &[NativeTableFamilyTable]) -> Vec<SemanticManagerTable> {
    tables
        .iter()
        .map(|table| SemanticManagerTable {
            table_name: table.table_name().as_str().to_owned(),
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
            Ok(SemanticRecordField {
                name: field.field().as_str().to_owned(),
                column: field.column().as_str().to_owned(),
                transform: semantic_transform(field.transform()).with_context(|| {
                    format!(
                        "field `{}` column `{}`",
                        field.field().as_str(),
                        field.column().as_str()
                    )
                })?,
            })
        })
        .collect()
}

fn semantic_transform(transform: NativeProjectionTransform) -> Result<SemanticProjectionTransform> {
    Ok(match transform {
        NativeProjectionTransform::String
        | NativeProjectionTransform::NonEmptyString
        | NativeProjectionTransform::EnumString
        | NativeProjectionTransform::EnumStringRejectDefault
        | NativeProjectionTransform::TypedCell
        | NativeProjectionTransform::OptionalTypedCellDefaultValue
        | NativeProjectionTransform::ForeignKeyTargetKey => SemanticProjectionTransform::String,
        NativeProjectionTransform::OptionalStringDefaultEmpty => {
            SemanticProjectionTransform::StringDefaultEmpty
        }
        NativeProjectionTransform::PlusJoinedList => SemanticProjectionTransform::PlusJoinedList,
        NativeProjectionTransform::OptionalString
        | NativeProjectionTransform::OptionalFirstString => {
            SemanticProjectionTransform::OptionalString
        }
        NativeProjectionTransform::StringList => SemanticProjectionTransform::StringList,
        NativeProjectionTransform::NonEmptyStringList => {
            SemanticProjectionTransform::NonEmptyStringList
        }
        NativeProjectionTransform::OptionalStringList => {
            SemanticProjectionTransform::OptionalStringList
        }
        NativeProjectionTransform::Bool
        | NativeProjectionTransform::OptionalBoolDefaultFalse
        | NativeProjectionTransform::Crc32NonZeroBool => SemanticProjectionTransform::Bool,
        NativeProjectionTransform::OptionalBool => SemanticProjectionTransform::OptionalBool,
        NativeProjectionTransform::U8
        | NativeProjectionTransform::OptionalU8DefaultZero
        | NativeProjectionTransform::OptionalU8DefaultMax
        | NativeProjectionTransform::U8Enum
        | NativeProjectionTransform::OptionalU8EnumDefaultValue => SemanticProjectionTransform::U8,
        NativeProjectionTransform::U32ToU16BelowMax => SemanticProjectionTransform::U16,
        NativeProjectionTransform::U32
        | NativeProjectionTransform::OptionalU32DefaultZero
        | NativeProjectionTransform::NonZeroU32 => SemanticProjectionTransform::U32,
        NativeProjectionTransform::OptionalU32 | NativeProjectionTransform::OptionalNonZeroU32 => {
            SemanticProjectionTransform::OptionalU32
        }
        NativeProjectionTransform::I32 => SemanticProjectionTransform::I32,
        NativeProjectionTransform::F32
        | NativeProjectionTransform::F32MinutesToSeconds
        | NativeProjectionTransform::F32UpperBound10000ZeroIsDefault
        | NativeProjectionTransform::F32LowerBound10000CappedToField => {
            SemanticProjectionTransform::F32
        }
        NativeProjectionTransform::OptionalF32 => SemanticProjectionTransform::OptionalF32,
        NativeProjectionTransform::F32ListDefaultEmpty => SemanticProjectionTransform::F32List,
        NativeProjectionTransform::I32ListDefaultEmpty => SemanticProjectionTransform::I32List,
        NativeProjectionTransform::Crc32
        | NativeProjectionTransform::LowercaseCrcString
        | NativeProjectionTransform::ForeignKeyTargetLowercaseCrc
        | NativeProjectionTransform::OptionalLowercaseCrcStringDefaultZero
        | NativeProjectionTransform::OptionalFirstLowercaseCrcStringDefaultZero
        | NativeProjectionTransform::OptionalTrimmedLowercaseCrcStringDefaultZero => {
            SemanticProjectionTransform::Crc32
        }
        NativeProjectionTransform::OptionalCrc32
        | NativeProjectionTransform::OptionalCrc32ZeroAsNone
        | NativeProjectionTransform::OptionalLowercaseCrcString
        | NativeProjectionTransform::OptionalTrimmedLowercaseCrcString => {
            SemanticProjectionTransform::OptionalLowercaseCrcString
        }
        NativeProjectionTransform::CrcList
        | NativeProjectionTransform::OptionalCrcListDefaultEmpty => {
            SemanticProjectionTransform::Crc32List
        }
        NativeProjectionTransform::LowercaseCrcStringList
        | NativeProjectionTransform::TrimmedLowercaseCrcStringList => {
            SemanticProjectionTransform::LowercaseCrcStringList
        }
        NativeProjectionTransform::ForeignKeyRow => SemanticProjectionTransform::RowIndex,
        NativeProjectionTransform::OptionalForeignKeyRow => {
            SemanticProjectionTransform::OptionalRowIndex
        }
        NativeProjectionTransform::ForeignKeyRowList
        | NativeProjectionTransform::OptionalForeignKeyRowListDefaultEmpty => {
            SemanticProjectionTransform::RowIndexList
        }
        NativeProjectionTransform::F32RangeInclusive => {
            SemanticProjectionTransform::F32RangeInclusive
        }
        NativeProjectionTransform::U32RangeInclusive => {
            SemanticProjectionTransform::U32RangeInclusive
        }
    })
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
        record_codegen_field(
            &source_name,
            &field.name,
            resolved_type_for_transform(field.transform),
        )
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
                ResolvedType::Scalar(ScalarType::U32),
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
                ResolvedType::Scalar(ScalarType::U32),
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

fn resolved_type_for_transform(transform: SemanticProjectionTransform) -> ResolvedType {
    match transform {
        SemanticProjectionTransform::String
        | SemanticProjectionTransform::StringDefaultEmpty
        | SemanticProjectionTransform::PlusJoinedList => ResolvedType::Scalar(ScalarType::String),
        SemanticProjectionTransform::OptionalString => optional(ScalarType::String),
        SemanticProjectionTransform::StringList
        | SemanticProjectionTransform::NonEmptyStringList => vector(ScalarType::String),
        SemanticProjectionTransform::OptionalStringList => ResolvedType::Optional {
            value: Box::new(vector(ScalarType::String)),
        },
        SemanticProjectionTransform::Bool => ResolvedType::Scalar(ScalarType::Bool),
        SemanticProjectionTransform::OptionalBool => optional(ScalarType::Bool),
        SemanticProjectionTransform::U8 => ResolvedType::Scalar(ScalarType::U8),
        SemanticProjectionTransform::U16 => ResolvedType::Scalar(ScalarType::U16),
        SemanticProjectionTransform::U32
        | SemanticProjectionTransform::Crc32
        | SemanticProjectionTransform::RowIndex => ResolvedType::Scalar(ScalarType::U32),
        SemanticProjectionTransform::OptionalU32
        | SemanticProjectionTransform::OptionalLowercaseCrcString
        | SemanticProjectionTransform::OptionalRowIndex => optional(ScalarType::U32),
        SemanticProjectionTransform::I32 => ResolvedType::Scalar(ScalarType::I32),
        SemanticProjectionTransform::F32 => ResolvedType::Scalar(ScalarType::F32),
        SemanticProjectionTransform::OptionalF32 => optional(ScalarType::F32),
        SemanticProjectionTransform::F32List => vector(ScalarType::F32),
        SemanticProjectionTransform::I32List => vector(ScalarType::I32),
        SemanticProjectionTransform::Crc32List
        | SemanticProjectionTransform::LowercaseCrcStringList
        | SemanticProjectionTransform::RowIndexList => vector(ScalarType::U32),
        SemanticProjectionTransform::F32RangeInclusive => pair(ScalarType::F32),
        SemanticProjectionTransform::U32RangeInclusive => pair(ScalarType::U32),
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

    #[test]
    fn manager_surfaces_emit_direct_or_implemented_semantic_apis_without_selection_lists() {
        let managers = validated_native_manager_specs();
        let surfaces = manager_surfaces_from_managers(&managers).expect("manager surfaces");

        let emitted_names = surfaces
            .iter()
            .map(|surface| match surface {
                ManagerSurface::Direct(manager) => manager.manager_name.clone(),
                ManagerSurface::Semantic(manager) => manager.manager_name.clone(),
                ManagerSurface::ItemData(manager) => manager.manager_name.clone(),
                ManagerSurface::ProductBacked(manager) => manager.manager_name.clone(),
            })
            .collect::<BTreeSet<_>>();
        assert!(!emitted_names.is_empty());
        assert!(
            surfaces
                .iter()
                .any(|surface| matches!(surface, ManagerSurface::Direct(_)))
        );
        for surface in surfaces.iter().filter_map(|surface| match surface {
            ManagerSurface::Direct(surface) | ManagerSurface::ProductBacked(surface) => {
                Some(surface)
            }
            ManagerSurface::Semantic(_) | ManagerSurface::ItemData(_) => None,
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
            | ManagerSurface::Semantic(_)
            | ManagerSurface::ItemData(_) => None,
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
            let dependencies = manager_surface_dependencies(
                &ManagerSurface::ProductBacked(surface.clone()),
                &manager.contract().inputs(),
            );
            assert!(
                dependencies.iter().all(|dependency| !matches!(
                    dependency,
                    ManagerSurfaceDependency::Table { .. }
                )),
                "`{}` product-backed surface must not load table dependencies",
                surface.manager_name
            );
        }
        let product_backed_names = surfaces
            .iter()
            .filter_map(|surface| match surface {
                ManagerSurface::ProductBacked(surface) => Some(surface.manager_name.clone()),
                ManagerSurface::Direct(_)
                | ManagerSurface::Semantic(_)
                | ManagerSurface::ItemData(_) => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            product_backed_names,
            BTreeSet::from([
                "ArmorOffsetDataManager".to_owned(),
                "CameraSettingsDataManager".to_owned(),
                "EquipTypesDataManager".to_owned(),
                "GameDebugSettingsManager".to_owned(),
                "GatherableDataManager".to_owned(),
                "PlayerDataManager".to_owned(),
                "RecipeDataManager".to_owned(),
                "SocialDataManager".to_owned(),
                "UiDataManager".to_owned(),
            ]),
            "standalone product-backed manager set should match the documented parsed product surfaces"
        );

        assert!(
            !emitted_names.contains("CurrencyExchangeMappingManager"),
            "manager-composition algorithms without generated semantic methods must not emit empty direct surfaces"
        );

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
    }
}
