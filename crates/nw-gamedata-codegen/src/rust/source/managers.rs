use anyhow::{Context, Result};
use nw_datasheet::ColumnType;
use quote::ToTokens;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

use crate::game_system_schema::GameSystemDataTablesSchemaReport;
use crate::manager::*;
use crate::manager_records::{
    CompositionManagerKind, CompositionManagerSurface, DirectManagerSurface,
    ItemDataManagerSurface, ManagerSurface, SemanticLookupKind, SemanticManagerKey,
    SemanticManagerRecord, SemanticNumericKeyType, SemanticProjectionTransform,
    SemanticRecordField, SemanticRowFilterPredicate, default_direct_manager_row_type,
    manager_accessor_domain, manager_surface_name, manager_surfaces_for_schema,
    semantic_enum_default_variant, semantic_enum_type_name,
};
use crate::naming::{to_snake_ident, to_upper_camel_ident};
use crate::native::NativeCodegenFile;
use crate::target::GameDataTargetLanguage;
use nw_serialize_codegen::rust_field_ident as serialize_rust_field_ident;

use super::format_rust_source;

mod native_standalone;
use native_standalone::{RustNativeManagerAugmentation, augment_native_manager};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RustManagerSourceEmitter;

impl ManagerEmitter for RustManagerSourceEmitter {
    fn target_language(&self) -> GameDataTargetLanguage {
        GameDataTargetLanguage::Rust
    }

    fn emit_managers(&self, context: ManagerEmissionContext<'_>) -> Result<ManagerCodegenOutput> {
        self.emit_managers_with_schema_report(
            context,
            &GameSystemDataTablesSchemaReport {
                tables: Vec::new(),
                diagnostics: Vec::new(),
                type_affinities: Vec::new(),
            },
        )
    }
}

impl RustManagerSourceEmitter {
    pub(crate) fn emit_managers_with_schema_report(
        &self,
        context: ManagerEmissionContext<'_>,
        schema_report: &GameSystemDataTablesSchemaReport,
    ) -> Result<ManagerCodegenOutput> {
        Ok(ManagerCodegenOutput::new(
            GameDataTargetLanguage::Rust,
            render_standalone_dynamic_manager_files(context, schema_report)?,
        ))
    }
}
fn render_standalone_dynamic_manager_files(
    context: ManagerEmissionContext<'_>,
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Result<Vec<NativeCodegenFile>> {
    let surfaces = manager_surfaces_for_schema(context.plan().managers(), schema_report)?;
    let records = rust_semantic_records(&surfaces);
    let runtime_source = String::from(
        r#"
use super::*;

#[derive(Debug, Clone)]
enum DatasheetCellValue {
    String(String),
    Number(f32),
    Boolean(bool),
}

#[derive(Debug, Clone)]
struct DynamicTableRow {
    source_path: String,
    row_index: usize,
    key: String,
    cells: Vec<DatasheetCellValue>,
    column_slots: Arc<HashMap<u32, usize>>,
}

#[derive(Debug, Clone)]
struct DynamicTable {
    schema: &'static TableDescriptor,
    rows: Vec<DynamicTableRow>,
    column_crcs: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
pub struct RowRef<Table, Row> {
    table: Table,
    key: String,
    marker: PhantomData<fn() -> Row>,
}

impl<Table: PartialEq, Row> PartialEq for RowRef<Table, Row> {
    fn eq(&self, other: &Self) -> bool {
        self.table == other.table && self.key == other.key
    }
}

impl<Table: Eq, Row> Eq for RowRef<Table, Row> {}

impl<Table: std::hash::Hash, Row> std::hash::Hash for RowRef<Table, Row> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.table, state);
        std::hash::Hash::hash(&self.key, state);
    }
}

impl<Table, Row> RowRef<Table, Row> {
    pub(in crate::managers) fn new(table: Table, key: impl Into<String>) -> Self {
        Self {
            table,
            key: key.into(),
            marker: PhantomData,
        }
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, Clone)]
pub struct RowSlot<Table, Row> {
    table: Table,
    row_index: usize,
    marker: PhantomData<fn() -> Row>,
}

impl<Table: PartialEq, Row> PartialEq for RowSlot<Table, Row> {
    fn eq(&self, other: &Self) -> bool {
        self.table == other.table && self.row_index == other.row_index
    }
}

impl<Table: Eq, Row> Eq for RowSlot<Table, Row> {}

impl<Table: std::hash::Hash, Row> std::hash::Hash for RowSlot<Table, Row> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.table, state);
        std::hash::Hash::hash(&self.row_index, state);
    }
}

impl<Table, Row> RowSlot<Table, Row> {
    pub(in crate::managers) fn new(table: Table, row_index: usize) -> Self {
        Self {
            table,
            row_index,
            marker: PhantomData,
        }
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    pub fn row_index(&self) -> usize {
        self.row_index
    }
}

#[derive(Debug, Clone)]
pub struct RowEntry<Table, Row> {
    pub reference: RowRef<Table, Row>,
    pub slot: RowSlot<Table, Row>,
    pub row: Row,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableReference<'a> {
    path: &'a str,
    key: &'a str,
}

impl<'a> TableReference<'a> {
    pub const fn new(path: &'a str, key: &'a str) -> Self {
        Self { path, key }
    }

    pub const fn path(self) -> &'a str {
        self.path
    }

    pub const fn key(self) -> &'a str {
        self.key
    }
}

#[derive(Debug, Clone)]
pub struct RowCollection<Table, Row> {
    entries: Arc<[RowEntry<Table, Row>]>,
    table_indexes: Arc<HashMap<String, RowTableIndex>>,
    table_order: Arc<[String]>,
    catalog_name: fn(Table) -> &'static str,
}

#[derive(Debug, Clone, Default)]
struct RowTableIndex {
    entries: Vec<usize>,
    by_key: HashMap<String, usize>,
    by_row_index: HashMap<usize, usize>,
}

impl<Table: Copy + Eq + std::hash::Hash, Row> RowCollection<Table, Row> {
    fn new(
        entries: Vec<RowEntry<Table, Row>>,
        catalog_name: fn(Table) -> &'static str,
    ) -> Self {
        let mut table_indexes = HashMap::<String, RowTableIndex>::new();
        let mut table_order = Vec::new();
        for (entry_index, entry) in entries.iter().enumerate() {
            let table = normalize_data_path(catalog_name(entry.reference.table));
            if !table_indexes.contains_key(&table) {
                table_order.push(table.clone());
            }
            let index = table_indexes.entry(table).or_default();
            index.entries.push(entry_index);
            index
                .by_key
                .entry(normalize_lookup_key(entry.reference.key()))
                .or_insert(entry_index);
            index
                .by_row_index
                .entry(entry.slot.row_index())
                .or_insert(entry_index);
        }
        Self {
            entries: entries.into(),
            table_indexes: Arc::new(table_indexes),
            table_order: table_order.into(),
            catalog_name,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn rows(&self) -> std::slice::Iter<'_, RowEntry<Table, Row>> {
        self.entries.iter()
    }

    pub(in crate::managers) fn table(&self, table: Table) -> TableRows<'_, Table, Row> {
        TableRows { rows: self, table }
    }

    pub fn get(&self, reference: &RowRef<Table, Row>) -> Option<&Row> {
        let entry_index = self
            .table_index((self.catalog_name)(reference.table))?
            .by_key
            .get(&normalize_lookup_key(reference.key()))?;
        self.entries
            .get(*entry_index)
            .map(|entry| &entry.row)
    }

    pub fn row_by_index(&self, slot: &RowSlot<Table, Row>) -> Option<&Row> {
        let entry_index = self
            .table_index((self.catalog_name)(slot.table))?
            .by_row_index
            .get(&slot.row_index())?;
        self.entries
            .get(*entry_index)
            .map(|entry| &entry.row)
    }

    pub fn row_key_by_index(&self, slot: &RowSlot<Table, Row>) -> Option<&str> {
        let entry_index = self
            .table_index((self.catalog_name)(slot.table))?
            .by_row_index
            .get(&slot.row_index())?;
        self.entries
            .get(*entry_index)
            .map(|entry| entry.reference.key())
    }

    fn table_index(&self, table: &str) -> Option<&RowTableIndex> {
        let normalized = normalize_data_path(table);
        self.table_indexes.get(&normalized).or_else(|| {
            self.table_order.iter().find_map(|candidate| {
                if table_path_matches(candidate, &normalized) {
                    self.table_indexes.get(candidate)
                } else {
                    None
                }
            })
        })
    }
}

pub struct TableRows<'a, Table, Row> {
    rows: &'a RowCollection<Table, Row>,
    table: Table,
}

impl<'a, Table: Copy + Eq + std::hash::Hash, Row> TableRows<'a, Table, Row> {
    pub fn table(&self) -> &Table {
        &self.table
    }

    pub fn get(&self, key: impl AsRef<str>) -> Option<&'a Row> {
        let entry_index = self
            .rows
            .table_index((self.rows.catalog_name)(self.table))?
            .by_key
            .get(&normalize_lookup_key(key.as_ref()))?;
        self.rows.entries.get(*entry_index).map(|entry| &entry.row)
    }

    pub fn row_by_index(&self, row_index: usize) -> Option<&'a Row> {
        let entry_index = self
            .rows
            .table_index((self.rows.catalog_name)(self.table))?
            .by_row_index
            .get(&row_index)?;
        self.rows.entries.get(*entry_index).map(|entry| &entry.row)
    }

    pub fn row_key_by_index(&self, row_index: usize) -> Option<&'a str> {
        let entry_index = self
            .rows
            .table_index((self.rows.catalog_name)(self.table))?
            .by_row_index
            .get(&row_index)?;
        self.rows
            .entries
            .get(*entry_index)
            .map(|entry| entry.reference.key())
    }

    pub fn rows(&self) -> impl Iterator<Item = &'a RowEntry<Table, Row>> {
        self.rows
            .table_index((self.rows.catalog_name)(self.table))
            .into_iter()
            .flat_map(|index| index.entries.iter())
            .filter_map(|entry_index| self.rows.entries.get(*entry_index))
    }
}

pub trait Rows {
    type Row;

    fn rows(&self) -> impl Iterator<Item = &Self::Row>;
}

impl<Table: Copy + Eq + std::hash::Hash, Row> Rows for RowCollection<Table, Row> {
    type Row = RowEntry<Table, Row>;

    fn rows(&self) -> impl Iterator<Item = &Self::Row> {
        RowCollection::rows(self)
    }
}

impl<Table: Copy + Eq + std::hash::Hash, Row> Rows for TableRows<'_, Table, Row> {
    type Row = RowEntry<Table, Row>;

    fn rows(&self) -> impl Iterator<Item = &Self::Row> {
        TableRows::rows(self)
    }
}

pub trait IntoCrc32Key {
    fn into_crc32_key(self) -> Crc32;
}

impl IntoCrc32Key for Crc32 {
    fn into_crc32_key(self) -> Crc32 {
        self
    }
}

impl IntoCrc32Key for &str {
    fn into_crc32_key(self) -> Crc32 {
        Crc32::from_str_lower(self)
    }
}

impl IntoCrc32Key for &String {
    fn into_crc32_key(self) -> Crc32 {
        Crc32::from_str_lower(self)
    }
}

impl IntoCrc32Key for String {
    fn into_crc32_key(self) -> Crc32 {
        Crc32::from_str_lower(&self)
    }
}

fn normalize_lookup_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

fn normalize_data_path(path: &str) -> String {
    path.replace('\\', "/").replace("//", "/").to_ascii_lowercase()
}

fn table_path_matches(left: &str, right: &str) -> bool {
    let left = normalize_data_path(left);
    let right = normalize_data_path(right);
    left == right
        || left.ends_with(&format!("/{right}"))
        || right.ends_with(&format!("/{left}"))
}

"#,
    );
    let datasheet_source = format!(
        "use super::*;\nuse super::products::crc32_lowercase;\nuse super::runtime::*;\n\n{RUST_STANDALONE_DYNAMIC_MANAGER_RUNTIME}",
    );

    let module_source = r#"
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use nw_objectstream::{asset_reference, value, Element, ObjectStream};

use crate::datasheet_catalog::{TableDescriptor, TABLES};
use crate::{AssetReference, Crc32, Vec3};

mod datasheets;
mod facade;
mod products;
mod rows;
mod runtime;
mod serialized;
mod surfaces;
mod values;

pub use facade::Managers;
pub use products::*;
pub use rows::*;
pub use runtime::{
    IntoCrc32Key, RowCollection, RowEntry, RowRef, RowSlot, Rows, TableReference, TableRows,
};
pub use serialized::*;
pub use surfaces::*;
pub use values::*;
"#;
    let mut facade_source =
        String::from("use super::*;\nuse super::datasheets::*;\n\nmod accessors;\n\n");
    let mut accessor_source = String::from("use super::*;\n\n");
    push_rust_managers_facade(&mut facade_source, &mut accessor_source, &surfaces);
    let (product_types, product_decoder_tail) = RUST_STANDALONE_PRODUCT_MANAGER_RUNTIME
        .split_once("fn parse_armor_offset_database")
        .context("split Rust product DTOs from decoders")?;
    let product_decoders = format!("fn parse_armor_offset_database{product_decoder_tail}");
    let (product_object_stream, product_xml_tail) = product_decoders
        .split_once("fn xml_fields")
        .context("split Rust ObjectStream and XML product decoders")?;
    let products_source = format!(
        "use super::*;\n\nmod decode;\nmod xml;\n\npub(super) use decode::*;\npub(super) use xml::crc32_lowercase;\n\n{product_types}"
    );
    let product_decoder_source =
        format!("use super::*;\nuse super::xml::*;\n\n{product_object_stream}");
    let product_xml_source = format!("use super::*;\n\nfn xml_fields{product_xml_tail}");

    let mut files = vec![
        NativeCodegenFile::new(
            "src/managers/mod.rs",
            format_rust_source(module_source).context("format Rust manager module")?,
        ),
        NativeCodegenFile::new(
            "src/managers/datasheets.rs",
            format_rust_source(&expose_rust_module_internals(&datasheet_source)?)
                .context("format Rust manager datasheet decoder")?,
        ),
        NativeCodegenFile::new(
            "src/managers/facade.rs",
            format_rust_source(&facade_source).context("format Rust manager facade")?,
        ),
        NativeCodegenFile::new(
            "src/managers/facade/accessors.rs",
            format_rust_source(&accessor_source).context("format Rust manager accessors")?,
        ),
        NativeCodegenFile::new(
            "src/managers/products/mod.rs",
            format_rust_source(&expose_rust_module_internals(&products_source)?)
                .context("format Rust manager product types")?,
        ),
        NativeCodegenFile::new(
            "src/managers/products/decode.rs",
            format_rust_source(&expose_rust_nested_manager_internals(
                &product_decoder_source,
            )?)
            .context("format Rust ObjectStream product decoders")?,
        ),
        NativeCodegenFile::new(
            "src/managers/products/xml.rs",
            format_rust_source(&expose_rust_nested_manager_internals(&product_xml_source)?)
                .context("format Rust XML product decoders")?,
        ),
        NativeCodegenFile::new(
            "src/managers/runtime.rs",
            format_rust_source(&expose_rust_module_internals(&runtime_source)?)
                .context("format Rust manager runtime")?,
        ),
        NativeCodegenFile::new(
            "src/managers/serialized.rs",
            format_rust_source(RUST_STANDALONE_SERIALIZED_TYPES)
                .context("format Rust serialized manager types")?,
        ),
        NativeCodegenFile::new(
            "src/managers/values.rs",
            format_rust_source(RUST_STANDALONE_VALUE_TYPES)
                .context("format Rust manager value types")?,
        ),
    ];
    files.extend(render_rust_row_modules(
        schema_report,
        &rust_direct_schema_row_types(&surfaces),
        &records,
    )?);
    files.extend(render_rust_surface_modules(&surfaces, schema_report)?);
    Ok(files)
}

fn expose_rust_module_internals(source: &str) -> Result<String> {
    expose_rust_internals(source, syn::parse_quote!(pub(super)))
}

fn expose_rust_nested_manager_internals(source: &str) -> Result<String> {
    expose_rust_internals(source, syn::parse_quote!(pub(in crate::managers)))
}

fn expose_rust_internals(source: &str, target: syn::Visibility) -> Result<String> {
    let mut file = syn::parse_file(source).context("parse generated Rust manager module")?;
    for item in &mut file.items {
        match item {
            syn::Item::Const(item) => widen_rust_visibility(&mut item.vis, &target),
            syn::Item::Enum(item) => {
                widen_rust_visibility(&mut item.vis, &target);
            }
            syn::Item::Fn(item) => widen_rust_visibility(&mut item.vis, &target),
            syn::Item::Impl(item) if item.trait_.is_none() => {
                for impl_item in &mut item.items {
                    match impl_item {
                        syn::ImplItem::Const(item) => widen_rust_visibility(&mut item.vis, &target),
                        syn::ImplItem::Fn(item) => widen_rust_visibility(&mut item.vis, &target),
                        syn::ImplItem::Type(item) => widen_rust_visibility(&mut item.vis, &target),
                        _ => {}
                    }
                }
            }
            syn::Item::Static(item) => widen_rust_visibility(&mut item.vis, &target),
            syn::Item::Struct(item) => {
                widen_rust_visibility(&mut item.vis, &target);
                widen_rust_fields(&mut item.fields, &target);
            }
            syn::Item::Type(item) => widen_rust_visibility(&mut item.vis, &target),
            _ => {}
        }
    }
    Ok(prettyplease::unparse(&file))
}

fn widen_rust_fields<'a>(
    fields: impl IntoIterator<Item = &'a mut syn::Field>,
    target: &syn::Visibility,
) {
    for field in fields {
        widen_rust_visibility(&mut field.vis, target);
    }
}

fn widen_rust_visibility(visibility: &mut syn::Visibility, target: &syn::Visibility) {
    if matches!(visibility, syn::Visibility::Inherited) {
        *visibility = target.clone();
    }
}

fn render_rust_row_modules(
    schema_report: &GameSystemDataTablesSchemaReport,
    readable_row_types: &BTreeSet<String>,
    records: &[SemanticManagerRecord],
) -> Result<Vec<NativeCodegenFile>> {
    let mut modules = Vec::new();
    let mut index = String::new();

    for row in rust_standalone_schema_rows(schema_report) {
        if !readable_row_types.contains(&row.source_row_type) {
            continue;
        }
        let module = format!(
            "schema_{}",
            to_snake_ident(&row.source_row_type, "schema_row")
        );
        let mut source = String::from(
            "use super::super::*;\nuse super::super::datasheets::*;\nuse super::super::runtime::*;\n\n",
        );
        if row.source_row_type == "LootBucketData" {
            push_rust_loot_bucket_schema_row(&mut source);
            index.push_str(&format!(
                "mod {module};\npub use {module}::{{LootBucketBiasingDisabled, LootBucketDataSchemaRow, LootBucketDataSlotEntry}};\npub(in crate::managers) use {module}::read_loot_bucket_data;\n"
            ));
        } else {
            push_rust_standalone_schema_row(&mut source, &row);
            let reader = rust_standalone_schema_reader_name(&row.source_row_type);
            index.push_str(&format!(
                "mod {module};\npub use {module}::{};\npub(in crate::managers) use {module}::{reader};\n",
                row.type_name
            ));
        }
        modules.push((
            format!("src/managers/rows/{module}.rs"),
            source,
            format!("format Rust schema row {}", row.type_name),
        ));
    }

    let enum_shapes = rust_semantic_enum_shapes(records);
    if !enum_shapes.is_empty() {
        let mut source = String::new();
        push_rust_semantic_enum_types(&mut source, &enum_shapes);
        let names = enum_shapes
            .iter()
            .map(|shape| shape.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        index.push_str(&format!(
            "mod semantic_enums;\npub use semantic_enums::{{{names}}};\n"
        ));
        modules.push((
            "src/managers/rows/semantic_enums.rs".to_owned(),
            source,
            "format Rust semantic manager enums".to_owned(),
        ));
    }

    for record in records {
        let module = format!(
            "record_{}",
            to_snake_ident(&record.record_type_name, "manager_record")
        );
        let mut source = String::new();
        if rust_semantic_record_fields(record)
            .iter()
            .any(|(_, field_type)| field_type.contains("Crc32"))
        {
            source.push_str("use crate::Crc32;\n");
        }
        let enum_names = record
            .fields
            .iter()
            .filter_map(|field| field.enum_shape.as_ref().map(|shape| shape.name.as_str()))
            .collect::<BTreeSet<_>>();
        if !enum_names.is_empty() {
            source.push_str(&format!(
                "use super::{{{}}};\n",
                enum_names.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
        if !source.is_empty() {
            source.push('\n');
        }
        push_rust_semantic_record_type(&mut source, record);
        index.push_str(&format!(
            "mod {module};\npub use {module}::{};\n",
            record.record_type_name
        ));
        modules.push((
            format!("src/managers/rows/{module}.rs"),
            source,
            format!("format Rust semantic record {}", record.record_type_name),
        ));
    }

    modules.push((
        "src/managers/rows/mod.rs".to_owned(),
        index,
        "format Rust manager row module".to_owned(),
    ));
    format_rust_modules(modules)
}

fn render_rust_surface_modules(
    surfaces: &[ManagerSurface],
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Result<Vec<NativeCodegenFile>> {
    let mut modules = Vec::new();
    let mut index = String::new();
    let mut seen = BTreeSet::new();
    for surface in surfaces {
        let manager_type = match surface {
            ManagerSurface::Direct(manager) | ManagerSurface::ProductBacked(manager) => {
                manager.manager_class_name.as_str()
            }
            ManagerSurface::Native { manager, .. } => manager.manager_class_name.as_str(),
            ManagerSurface::Semantic(record) => record.manager_class_name.as_str(),
            ManagerSurface::ItemData(manager) => manager.manager_class_name.as_str(),
            ManagerSurface::Composition(manager) => manager.manager_class_name.as_str(),
        };
        if !seen.insert(manager_type) {
            continue;
        }
        let module = to_snake_ident(manager_type, "manager");
        let mut source = String::new();
        if let ManagerSurface::Semantic(record) = surface {
            push_rust_enum_parsers(&mut source, std::slice::from_ref(record));
        }
        if matches!(
            surface,
            ManagerSurface::Direct(_) | ManagerSurface::Native { .. }
        ) {
            push_rust_direct_row_family_types(
                &mut source,
                std::slice::from_ref(surface),
                schema_report,
            );
        }
        push_rust_standalone_manager_surfaces(
            &mut source,
            std::slice::from_ref(surface),
            schema_report,
        )?;
        source = prune_unused_generated_helpers(&source)?;
        let mut imports = String::from("use super::super::*;\n");
        if source.contains("ManagerResources")
            || source.contains("ManagerCache")
            || source.contains("split_designer_list(")
        {
            imports.push_str("use super::super::datasheets::*;\n");
        }
        if source.contains("DynamicTable")
            || source.contains("DynamicTableRow")
            || source.contains("table_path_matches(")
            || source.contains("normalize_lookup_key(")
            || source.contains("normalize_data_path(")
        {
            imports.push_str("use super::super::runtime::*;\n");
        }
        imports.push('\n');
        source.insert_str(0, &imports);
        index.push_str(&format!("mod {module};\npub use {module}::*;\n"));
        modules.push((
            format!("src/managers/surfaces/{module}.rs"),
            source,
            format!("format Rust manager {manager_type}"),
        ));
    }
    modules.push((
        "src/managers/surfaces/mod.rs".to_owned(),
        index,
        "format Rust manager surface module".to_owned(),
    ));
    format_rust_modules(modules)
}

const GENERATED_PRIVATE_HELPERS: &[&str] = &[
    "pvp_balance_f32",
    "pvp_balance_number",
    "pvp_balance_number_text",
    "pvp_balance_text",
    "season_bool",
    "season_crc",
    "season_crc_list",
    "season_exact_nonzero_u32",
    "season_exact_u32",
    "season_finite_f32",
    "season_id_from_table_name",
    "season_owned_text",
    "season_row_index",
];

fn prune_unused_generated_helpers(source: &str) -> Result<String> {
    let mut file = syn::parse_file(source).context("parse Rust manager before helper pruning")?;
    loop {
        let removable = file
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let syn::Item::Fn(function) = item else {
                    return None;
                };
                let name = function.sig.ident.to_string();
                if !GENERATED_PRIVATE_HELPERS.contains(&name.as_str()) {
                    return None;
                }
                let referenced = file.items.iter().enumerate().any(|(other_index, other)| {
                    other_index != index && token_stream_mentions(other.to_token_stream(), &name)
                });
                (!referenced).then_some(index)
            })
            .collect::<BTreeSet<_>>();
        if removable.is_empty() {
            break;
        }
        file.items = file
            .items
            .into_iter()
            .enumerate()
            .filter_map(|(index, item)| (!removable.contains(&index)).then_some(item))
            .collect();
    }
    Ok(prettyplease::unparse(&file))
}

fn token_stream_mentions(stream: proc_macro2::TokenStream, name: &str) -> bool {
    stream.into_iter().any(|token| match token {
        proc_macro2::TokenTree::Ident(identifier) => identifier == name,
        proc_macro2::TokenTree::Group(group) => token_stream_mentions(group.stream(), name),
        _ => false,
    })
}

fn format_rust_modules(modules: Vec<(String, String, String)>) -> Result<Vec<NativeCodegenFile>> {
    let worker_count = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count)
        .thread_name(|index| format!("gamedata-rustfmt-{index}"))
        .build()
        .context("build Rust manager formatting pool")?;
    pool.install(|| {
        modules
            .into_par_iter()
            .map(|(path, source, context)| {
                let source = format_rust_source(&source).with_context(|| context)?;
                Ok(NativeCodegenFile::new(path, source))
            })
            .collect()
    })
}

fn rust_semantic_records(surfaces: &[ManagerSurface]) -> Vec<SemanticManagerRecord> {
    surfaces
        .iter()
        .filter_map(|surface| match surface {
            ManagerSurface::Semantic(record) => Some(record.clone()),
            ManagerSurface::Direct(_)
            | ManagerSurface::Native { .. }
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_)
            | ManagerSurface::ProductBacked(_) => None,
        })
        .collect()
}

fn rust_direct_schema_row_types(surfaces: &[ManagerSurface]) -> BTreeSet<String> {
    let mut row_types = BTreeSet::new();
    for surface in surfaces {
        let manager = match surface {
            ManagerSurface::Direct(manager) | ManagerSurface::Native { manager, .. } => manager,
            ManagerSurface::Semantic(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_)
            | ManagerSurface::ProductBacked(_) => continue,
        };
        row_types.extend(
            manager
                .tables
                .iter()
                .map(|table| table.row_type_name.clone()),
        );
    }
    row_types
}

fn rust_manager_accessor_name(manager_name: &str) -> String {
    to_snake_ident(manager_accessor_domain(manager_name), "manager")
}

fn rust_manager_dependency_name(manager_name: &str) -> String {
    to_snake_ident(
        manager_name.strip_suffix("Manager").unwrap_or(manager_name),
        "manager",
    )
}

fn rust_manager_resources_expression<'a>(
    manager_name: &str,
    tables: impl IntoIterator<Item = (&'a str, &'a str)>,
    asset_paths: impl IntoIterator<Item = &'a str>,
) -> String {
    format!(
        "cache.resources_for_tables({}, {}, {})",
        rust_string_literal(manager_name),
        rust_table_selector_slice(tables),
        rust_string_slice(asset_paths)
    )
}

fn rust_table_selector_slice<'a>(tables: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let tables = tables
        .into_iter()
        .map(|(name, row_type)| {
            format!(
                "TableSelector::new({}, {})",
                rust_string_literal(name),
                rust_string_literal(row_type)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[{tables}]")
}

fn rust_direct_manager_resources_expression(manager: &DirectManagerSurface) -> String {
    let row_types = manager
        .tables
        .iter()
        .map(|table| table.row_type_name.as_str())
        .collect::<BTreeSet<_>>();
    format!(
        "cache.resources_for_rows({}, {}, {})",
        rust_string_literal(&manager.manager_name),
        rust_string_slice(row_types),
        rust_string_slice(manager.products.iter().map(|product| product.path.as_str()))
    )
}

fn rust_string_slice<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    let values = values
        .into_iter()
        .map(rust_string_literal)
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[{values}]")
}

#[derive(Debug, Clone)]
struct RustStandaloneSchemaRow {
    type_name: String,
    source_row_type: String,
    fields: Vec<RustStandaloneSchemaField>,
}

#[derive(Debug, Clone)]
struct RustStandaloneSchemaField {
    source_name: String,
    field_name: String,
    column_type: ColumnType,
    required: bool,
    row_key: bool,
}

fn push_rust_standalone_schema_row(source: &mut String, row: &RustStandaloneSchemaRow) {
    source.push_str(&format!(
        "#[derive(Debug, Clone, PartialEq)]\npub struct {} {{\n",
        row.type_name
    ));
    for field in &row.fields {
        source.push_str(&format!(
            "    pub {}: {},\n",
            field.field_name,
            rust_standalone_schema_field_type(field.column_type, field.required)
        ));
    }
    source.push_str("}\n\n");
    source.push_str(&format!(
        "pub(in crate::managers) fn {}(table: &DynamicTable, row: &DynamicTableRow) -> Result<{}> {{\n",
        rust_standalone_schema_reader_name(&row.source_row_type),
        row.type_name
    ));
    source.push_str(&format!("    Ok({} {{\n", row.type_name));
    for field in &row.fields {
        source.push_str(&format!(
            "        {}: {},\n",
            field.field_name,
            rust_standalone_schema_field_read_expression(field)
        ));
    }
    source.push_str("    })\n");
    source.push_str("}\n\n");
}

fn push_rust_loot_bucket_schema_row(source: &mut String) {
    source.push_str(
        r#"
#[derive(Debug, Clone)]
pub struct LootBucketDataSchemaRow {
    pub row_placeholders: String,
    pub entries: Vec<LootBucketDataSlotEntry>,
    pub loot_biasing_disabled: Vec<LootBucketBiasingDisabled>,
}

#[derive(Debug, Clone)]
pub struct LootBucketDataSlotEntry {
    pub slot: u16,
    pub loot_bucket: Option<String>,
    pub tags: Option<String>,
    pub match_one: Option<String>,
    pub item: Option<String>,
    pub quantity: Option<String>,
    pub odds: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LootBucketBiasingDisabled {
    pub slot: u16,
    pub disabled: bool,
}

pub(in crate::managers) fn read_loot_bucket_data(
    table: &DynamicTable,
    row: &DynamicTableRow,
) -> Result<LootBucketDataSchemaRow> {
    let row_placeholders = required_string_cell(table, row, "RowPlaceholders")?.to_owned();
    let mut entries = Vec::new();
    for slot in numbered_column_slots(
        table,
        &["LootBucket", "Tags", "MatchOne", "Item", "Quantity", "Odds"],
    ) {
        let loot_bucket = optional_cell_text(table, row, &numbered_column_name("LootBucket", slot))?;
        let tags = optional_cell_text(table, row, &numbered_column_name("Tags", slot))?;
        let match_one = optional_cell_text(table, row, &numbered_column_name("MatchOne", slot))?;
        let item = optional_cell_text(table, row, &numbered_column_name("Item", slot))?;
        let quantity = optional_cell_text(table, row, &numbered_column_name("Quantity", slot))?;
        let odds = optional_cell_text(table, row, &numbered_column_name("Odds", slot))?;
        if loot_bucket.is_some()
            || tags.is_some()
            || match_one.is_some()
            || item.is_some()
            || quantity.is_some()
            || odds.is_some()
        {
            entries.push(LootBucketDataSlotEntry {
                slot,
                loot_bucket,
                tags,
                match_one,
                item,
                quantity,
                odds,
            });
        }
    }

    let mut loot_biasing_disabled = Vec::new();
    for slot in numbered_column_slots(table, &["LootBiasingDisabled"]) {
        if let Some(disabled) =
            optional_cell_bool_text(table, row, &numbered_column_name("LootBiasingDisabled", slot))?
        {
            loot_biasing_disabled.push(LootBucketBiasingDisabled { slot, disabled });
        }
    }

    Ok(LootBucketDataSchemaRow {
        row_placeholders,
        entries,
        loot_biasing_disabled,
    })
}

fn numbered_column_slots(table: &DynamicTable, prefixes: &[&str]) -> Vec<u16> {
    let mut slots = Vec::new();
    for column in &table.schema.columns {
        for prefix in prefixes {
            if let Some(slot) = numbered_column_slot(&column.name, prefix) {
                slots.push(slot);
            }
        }
    }
    slots.sort_unstable();
    slots.dedup();
    slots
}

fn numbered_column_slot(name: &str, prefix: &str) -> Option<u16> {
    let suffix = name.strip_prefix(prefix)?;
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok()
}

fn numbered_column_name(prefix: &str, slot: u16) -> String {
    format!("{prefix}{slot}")
}

fn optional_cell_text(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<String>> {
    match row_cell(table, row, column_name) {
        None => Ok(None),
        Some(DatasheetCellValue::String(value)) if value.is_empty() => Ok(None),
        Some(DatasheetCellValue::String(value)) => Ok(Some(value.to_owned())),
        Some(DatasheetCellValue::Number(value)) => Ok(Some(value.to_string())),
        Some(DatasheetCellValue::Boolean(value)) => Ok(Some(value.to_string())),
    }
}

fn optional_cell_bool_text(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<bool>> {
    match row_cell(table, row, column_name) {
        None => Ok(None),
        Some(DatasheetCellValue::Boolean(value)) => Ok(Some(*value)),
        Some(DatasheetCellValue::Number(value)) => Ok(Some(*value != 0.0)),
        Some(DatasheetCellValue::String(value)) if value.is_empty() => Ok(None),
        Some(DatasheetCellValue::String(value)) => match value.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(Some(true)),
            "false" | "0" | "no" => Ok(Some(false)),
            _ => bail!(
                "row {}:{} has non-bool {column_name}",
                row.source_path,
                row.row_index + 1
            ),
        },
    }
}

"#,
    );
}

fn rust_standalone_schema_rows(
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Vec<RustStandaloneSchemaRow> {
    let mut rows = BTreeMap::<String, Vec<RustStandaloneSchemaField>>::new();
    for table in &schema_report.tables {
        let row_type = table.row_type_name.clone();
        let fields = rows.entry(row_type.clone()).or_default();
        for column in &table.columns {
            let field_name = to_snake_ident(&column.name, "field");
            if let Some(existing) = fields
                .iter_mut()
                .find(|field| field.field_name == field_name)
            {
                existing.row_key |= column.row_key;
                existing.required = existing.row_key;
                existing.column_type =
                    merge_schema_column_type(existing.column_type, column.declared_type);
                continue;
            }
            fields.push(RustStandaloneSchemaField {
                source_name: column.name.clone(),
                field_name,
                column_type: column.declared_type,
                required: column.row_key,
                row_key: column.row_key,
            });
        }
    }
    rows.into_iter()
        .map(|(source_row_type, fields)| RustStandaloneSchemaRow {
            type_name: rust_standalone_schema_row_type_name(&source_row_type),
            source_row_type,
            fields,
        })
        .collect()
}

fn merge_schema_column_type(left: ColumnType, right: ColumnType) -> ColumnType {
    match (left, right) {
        (ColumnType::String, _) | (_, ColumnType::String) => ColumnType::String,
        (ColumnType::Number, _) | (_, ColumnType::Number) => ColumnType::Number,
        (ColumnType::Boolean, ColumnType::Boolean) => ColumnType::Boolean,
    }
}

fn rust_standalone_schema_row_type_name(row_type: &str) -> String {
    format!("{}SchemaRow", to_upper_camel_ident(row_type, "Schema"))
}

// Tests exercise source renderers before the remaining production helpers below.
#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use nw_datasheet::game_system::Crc32;

    use crate::game_system_schema::{
        GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemDataTablesSchemaReport,
        GameSystemTableSchema,
    };
    use crate::manager_records::{DirectManagerTable, ItemDataManagerTable, SemanticLookupMethod};

    use super::*;

    #[test]
    fn semantic_resources_use_exact_table_schema_identity() {
        let expression = rust_manager_resources_expression(
            "ExampleManager",
            [("SharedTable", "ExampleRow")],
            std::iter::empty(),
        );

        assert!(expression.contains("cache.resources_for_tables("));
        assert!(expression.contains("TableSelector::new(\"SharedTable\", \"ExampleRow\")"));
        assert!(!expression.contains("cache.resources("));
    }

    #[test]
    fn costume_change_shipping_slots_emit_schema_backed_semantic_rows() {
        let specs = validated_native_manager_specs();
        let surface = crate::manager_records::manager_surfaces_from_managers(&specs)
            .unwrap()
            .into_iter()
            .find(|surface| manager_surface_name(surface) == "CostumeChangeDataManager")
            .expect("CostumeChangeDataManager surface");
        let mut columns = vec![
            schema_column("CostumeChangeId", ColumnType::String, true),
            schema_column("CostumeChangeMesh", ColumnType::String, false),
            schema_column("MatchesPlayerSkeleton", ColumnType::Boolean, false),
            schema_column("MeshRenderZPosOffset", ColumnType::Number, false),
        ];
        for slot in ["HEAD", "CHEST", "HANDS", "LEGS", "FEET"] {
            columns.push(schema_column(
                &format!("{slot}_SLOT_Left"),
                ColumnType::String,
                false,
            ));
            columns.push(schema_column(
                &format!("{slot}_SLOT_Right"),
                ColumnType::String,
                false,
            ));
        }
        let schema = GameSystemDataTablesSchemaReport {
            tables: vec![schema_table("CostumeChanges", "CostumeChangeData", columns)],
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        };
        let files = render_rust_surface_modules(std::slice::from_ref(&surface), &schema).unwrap();
        let source = files
            .iter()
            .find(|file| file.path() == "src/managers/surfaces/costume_change_data_manager.rs")
            .expect("CostumeChangeDataManager module")
            .contents();

        assert!(source.contains("Head = 0"));
        assert!(source.contains("Chest = 1"));
        assert!(source.contains("Hands = 2"));
        assert!(source.contains("Legs = 3"));
        assert!(source.contains("Feet = 4"));
        let compact = source.split_whitespace().collect::<String>();
        assert!(
            compact.contains("audio_overrides:[CostumeAudioDataOverride;5]"),
            "{source}"
        );
        assert_eq!(compact.matches("pubfnrows(&self)").count(), 1, "{source}");
        assert!(source.contains("type Row = CostumeChangeData;"));
        assert!(source.contains("pub fn table("));
        assert!(!source.contains("from_loaded_tables"));
        assert!(!source.contains("//!"));
    }

    #[test]
    fn skip_empty_semantic_keys_accept_missing_cells() {
        let source = rust_semantic_key_materializer(&semantic_lookup_record());

        assert!(source.contains("optional_string_cell"));
        assert!(source.contains("let Some(key_text)"));
        assert!(!source.contains("required_string_cell"));
    }

    #[test]
    fn merged_schema_column_type_is_lossless_for_mixed_source_columns() {
        assert_eq!(
            merge_schema_column_type(ColumnType::Number, ColumnType::String),
            ColumnType::String
        );
        assert_eq!(
            merge_schema_column_type(ColumnType::Boolean, ColumnType::String),
            ColumnType::String
        );
        assert_eq!(
            merge_schema_column_type(ColumnType::Boolean, ColumnType::Number),
            ColumnType::Number
        );
    }

    #[test]
    fn direct_schema_manager_uses_rows_contract_for_primary_row_type() {
        let schema_report = damage_schema_report();
        let manager = damage_manager_surface();
        let methods = rust_direct_schema_methods(&manager, &schema_report, true);
        let rows_trait_impl = rust_direct_rows_trait_impl(&manager, &schema_report);
        let resources = rust_direct_manager_resources_expression(&manager);

        assert!(resources.contains("cache.resources_for_rows"));
        assert!(resources.contains("\"AfflictionData\""));
        assert!(resources.contains("\"DamageTypeData\""));
        assert!(
            methods.contains(
                "pub fn rows(&self) -> std::slice::Iter<'_, RowEntry<DamageDataTable, DamageDataSchemaRow>>"
            )
        );
        assert!(methods.contains(
            "pub fn table(&self, table: DamageDataTable) -> TableRows<'_, DamageDataTable, DamageDataSchemaRow>"
        ));
        assert!(methods.contains(
            "pub fn row_ref(&self, table: DamageDataTable, key: impl Into<String>) -> RowRef<DamageDataTable, DamageDataSchemaRow>"
        ));
        assert!(!methods.contains("table: impl Into<String>"));
        assert!(methods.contains(
            "pub fn row(&self, reference: &RowRef<DamageDataTable, DamageDataSchemaRow>) -> Option<&DamageDataSchemaRow>"
        ));
        assert!(methods.contains(
            "pub fn row_by_index(&self, slot: &RowSlot<DamageDataTable, DamageDataSchemaRow>) -> Option<&DamageDataSchemaRow>"
        ));
        assert!(!methods.contains("pub fn iter"));
        assert!(
            methods.contains(
                "pub fn affliction_data_rows(&self) -> &RowCollection<DamageDataAfflictionDataTable, AfflictionDataSchemaRow>"
            )
        );
        assert!(
            methods.contains(
                "pub fn damage_type_data_rows(&self) -> &RowCollection<DamageDataDamageTypeDataTable, DamageTypeDataSchemaRow>"
            )
        );
        assert!(methods.contains(
            "pub fn damage_type_data_rows_table(&self, table: DamageDataDamageTypeDataTable)"
        ));
        assert!(!methods.contains("pub fn affliction_data(&self) -> &RowCollection"));
        assert!(!methods.contains("pub fn damage_type_data(&self) -> &RowCollection"));
        assert!(!methods.contains(
            "pub fn affliction_data(&self, key: impl ToString) -> Result<Option<AfflictionDataSchemaRow>>"
        ));
        assert!(!methods.contains(
            "pub fn row(&self, key: impl ToString) -> Result<Option<DamageDataSchemaRow>>"
        ));
        assert!(!methods.contains("pub fn damage_data_rows"));
        assert!(rows_trait_impl.contains("impl Rows for DamageDataManager"));
        assert!(
            rows_trait_impl.contains("type Row = RowEntry<DamageDataTable, DamageDataSchemaRow>")
        );
    }

    #[test]
    fn generic_direct_manager_uses_a_typed_table_identifier() {
        let schema_report = GameSystemDataTablesSchemaReport {
            tables: vec![schema_table(
                "GenericRows",
                "GenericData",
                vec![schema_column("Id", ColumnType::String, true)],
            )],
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        };
        let manager = DirectManagerSurface {
            manager_name: "GenericDataManager".to_owned(),
            manager_class_name: "GenericDataManager".to_owned(),
            tables: vec![DirectManagerTable {
                table_name: "GenericRows".to_owned(),
                row_type_name: "GenericData".to_owned(),
            }],
            products: Vec::new(),
        };
        let mut source = String::new();
        push_rust_direct_manager_wrapper(&mut source, &manager, &schema_report).unwrap();

        assert!(source.contains("pub enum GenericDataTable"));
        assert!(source.contains(
            "pub fn table(&self, table: GenericDataTable) -> TableRows<'_, GenericDataTable, GenericDataSchemaRow>"
        ));
        assert!(source.contains("pub fn row_ref(&self, table: GenericDataTable"));
        assert!(source.contains("RowRef<GenericDataTable, GenericDataSchemaRow>"));
        assert!(!source.contains("pub const fn table_name"));
        assert!(!source.contains("table: impl Into<String>"));
    }

    #[test]
    fn replication_composition_precomputes_reverse_indexes() {
        let mut source = String::new();
        push_rust_composition_manager_wrapper(
            &mut source,
            &composition_surface(CompositionManagerKind::ReplicationData),
        );

        assert!(source.contains("indexes_by_id: HashMap<Crc32, u16>"));
        assert!(source.contains("self.indexes_by_id.get(&id).copied().unwrap_or(0)"));
        assert!(!source.contains(".position("));
    }

    #[test]
    fn static_tradeskill_composition_materializes_typed_cached_mappings() {
        let mut source = String::new();
        push_rust_composition_manager_wrapper(
            &mut source,
            &composition_surface(CompositionManagerKind::StaticTradeskillRankDataMapping),
        );

        assert!(source.contains("pub struct StaticTradeskillRankDataMapping"));
        assert!(source.contains("table: TradeskillRankDataTable"));
        assert!(source.contains("tradeskill_ranks: HashMap<Crc32"));
        assert!(source.contains("for entry in tradeskill_rank_data.rows()"));
        assert!(source.contains("entry.source.table().catalog_name()"));
        assert!(source.contains("entry.rank.value()"));
        assert!(source.contains("entry.display_name.as_deref()"));
        assert!(!source.contains("pub struct TradeskillRank("));
        assert!(!source.contains("pub struct TradeskillRankDataTable("));
        assert!(!source.contains("pub struct StaticTradeskillRankDataMappingManager;"));
    }

    #[test]
    fn item_data_manager_uses_rows_contract() {
        let mut source = String::new();
        push_rust_item_data_manager_wrapper(&mut source, &item_data_manager_surface());

        assert!(source.contains("pub fn rows(&self) -> std::slice::Iter<'_, ItemData>"));
        assert!(!source.contains("pub fn iter"));
        assert!(source.contains("impl Rows for ItemDataManager"));
        assert!(source.contains("type Row = ItemData"));
    }

    #[test]
    fn semantic_into_crc_lookup_accepts_string_or_crc_key() {
        let methods = rust_semantic_lookup_methods(&semantic_lookup_record());

        assert!(methods.contains(
            "pub fn backstory(&self, backstory_id: impl IntoCrc32Key) -> Option<&StaticBackstoryData>"
        ));
        assert!(methods.contains("let key = backstory_id.into_crc32_key();"));
        assert!(methods.contains(
            "pub fn backstory_by_key(&self, backstory_key: impl AsRef<str>) -> Option<&StaticBackstoryData>"
        ));
    }

    #[test]
    fn source_foreign_keys_materialize_owned_strings() {
        let required = SemanticRecordField {
            name: "project_key".to_owned(),
            column: "ProjectID".to_owned(),
            transform: SemanticProjectionTransform::ForeignKey,
            value_type: None,
            default_value: None,
            reference_field: None,
            u16_max_exclusive: None,
            enum_shape: None,
            pair_first_enum_shape: None,
        };
        let optional = SemanticRecordField {
            name: "previous_project_key".to_owned(),
            column: "PreviousProjectID".to_owned(),
            transform: SemanticProjectionTransform::OptionalForeignKey,
            value_type: None,
            default_value: None,
            reference_field: None,
            u16_max_exclusive: None,
            enum_shape: None,
            pair_first_enum_shape: None,
        };

        assert_eq!(
            rust_projection_value(&required),
            "required_string_cell(table, source_row, \"ProjectID\")?.to_owned()"
        );
        assert_eq!(
            rust_projection_value(&optional),
            "optional_string_cell(table, source_row, \"PreviousProjectID\")?.map(str::to_owned)"
        );
    }

    #[test]
    fn source_string_projections_accept_mixed_physical_cell_types() {
        let optional = SemanticRecordField {
            name: "influence_cost".to_owned(),
            column: "InfluenceCost".to_owned(),
            transform: SemanticProjectionTransform::OptionalString,
            value_type: None,
            default_value: None,
            reference_field: None,
            u16_max_exclusive: None,
            enum_shape: None,
            pair_first_enum_shape: None,
        };

        assert_eq!(
            rust_projection_value(&optional),
            "optional_schema_string_cell(table, source_row, \"InfluenceCost\")?"
        );
    }

    #[test]
    fn semantic_managers_emit_only_consumed_indexes() {
        let mut record = semantic_lookup_record();
        record.lookup_methods.clear();
        let mut source = String::new();

        push_rust_semantic_manager_wrapper(&mut source, &record);

        assert!(!source.contains("entries_by_key"));
        assert!(!source.contains("entries_by_source_row"));
    }

    #[test]
    fn skip_invalid_enum_projection_continues_without_fabricating_a_variant() {
        let mut record = semantic_lookup_record();
        record.fields.push(skip_invalid_enum_field());
        let mut source = String::new();

        push_rust_semantic_materializer(&mut source, &record);

        assert!(source.contains("let Ok(projected_mission_goal_type) = parse_mission_goal_type"));
        assert!(source.contains("continue;"));
        assert!(!source.contains("MissionGoalType::Invalid"));
    }

    #[test]
    fn numeric_key_conversion_preserves_u32_values() {
        assert_eq!(
            rust_numeric_key_as_u32("row.level", SemanticNumericKeyType::U8),
            "row.level as u32"
        );
        assert_eq!(
            rust_numeric_key_as_u32("row.level", SemanticNumericKeyType::U32),
            "row.level"
        );
    }

    #[test]
    fn rust_field_initializers_use_shorthand_when_names_match() {
        assert_eq!(
            rust_field_initializer("key_kind", "key_kind"),
            "                key_kind,\n"
        );
        assert_eq!(
            rust_field_initializer("item_id", "key_value"),
            "                item_id: key_value,\n"
        );
    }

    fn damage_manager_surface() -> DirectManagerSurface {
        DirectManagerSurface {
            manager_name: "DamageDataManager".to_owned(),
            manager_class_name: "DamageDataManager".to_owned(),
            tables: vec![
                DirectManagerTable {
                    table_name: "DamageData".to_owned(),
                    row_type_name: "DamageData".to_owned(),
                },
                DirectManagerTable {
                    table_name: "AfflictionData".to_owned(),
                    row_type_name: "AfflictionData".to_owned(),
                },
                DirectManagerTable {
                    table_name: "DamageTypeData".to_owned(),
                    row_type_name: "DamageTypeData".to_owned(),
                },
            ],
            products: Vec::new(),
        }
    }

    fn item_data_manager_surface() -> ItemDataManagerSurface {
        ItemDataManagerSurface {
            manager_name: "ItemDataManager".to_owned(),
            manager_class_name: "ItemDataManager".to_owned(),
            table_type_name: "ItemDataTable".to_owned(),
            handle_type_name: "ItemDataHandle".to_owned(),
            data_type_name: "ItemData".to_owned(),
            tables: vec![ItemDataManagerTable {
                variant_name: "Master".to_owned(),
                table_name: "MasterItemDefinitions".to_owned(),
                row_type_name: "MasterItemDefinitions".to_owned(),
            }],
        }
    }

    fn composition_surface(kind: CompositionManagerKind) -> CompositionManagerSurface {
        CompositionManagerSurface {
            manager_name: "TestManager".to_owned(),
            manager_class_name: "TestManager".to_owned(),
            kind,
            dependencies: Vec::new(),
        }
    }

    fn semantic_lookup_record() -> SemanticManagerRecord {
        SemanticManagerRecord {
            manager_name: "StaticBackstoryDataManager".to_owned(),
            manager_class_name: "StaticBackstoryDataManager".to_owned(),
            record_type_name: "StaticBackstoryData".to_owned(),
            tables: Vec::new(),
            key: Some(SemanticManagerKey::Crc {
                key_field: "backstory_id".to_owned(),
                crc_field: "backstory_crc".to_owned(),
                key_column: "BackstoryID".to_owned(),
                skip_empty_key: true,
                trim_key: true,
                reject_zero_crc: true,
                duplicate_key_policy: crate::manager::NativeDuplicateKeyPolicy::FirstWins,
            }),
            source_row_field: None,
            source_row_method: None,
            row_filters: Vec::new(),
            fields: Vec::new(),
            lookup_methods: vec![
                SemanticLookupMethod {
                    name: "backstory".to_owned(),
                    parameter: "backstory_id".to_owned(),
                    kind: SemanticLookupKind::IntoCrc,
                },
                SemanticLookupMethod {
                    name: "backstory_by_key".to_owned(),
                    parameter: "backstory_key".to_owned(),
                    kind: SemanticLookupKind::CrcString,
                },
            ],
            ids_method: None,
            rows_method: None,
            len_method: None,
            is_empty_method: None,
        }
    }

    fn skip_invalid_enum_field() -> SemanticRecordField {
        SemanticRecordField {
            name: "mission_goal_type".to_owned(),
            column: "MissionGoalType".to_owned(),
            transform: SemanticProjectionTransform::EnumStringSkipInvalid,
            value_type: Some("MissionGoalType".to_owned()),
            default_value: None,
            reference_field: None,
            u16_max_exclusive: None,
            enum_shape: Some(crate::game_system_schema::GameSystemEnumShape {
                name: "MissionGoalType".to_owned(),
                representation: crate::game_system_schema::GameSystemEnumRepresentation::U8,
                variants: Vec::new(),
            }),
            pair_first_enum_shape: None,
        }
    }

    fn damage_schema_report() -> GameSystemDataTablesSchemaReport {
        GameSystemDataTablesSchemaReport {
            tables: vec![
                schema_table(
                    "DamageData",
                    "DamageData",
                    vec![
                        schema_column("DamageID", ColumnType::String, true),
                        schema_column("BaseDamage", ColumnType::Number, false),
                    ],
                ),
                schema_table(
                    "AfflictionData",
                    "AfflictionData",
                    vec![
                        schema_column("AfflictionID", ColumnType::String, true),
                        schema_column("DisplayName", ColumnType::String, false),
                    ],
                ),
                schema_table(
                    "DamageTypeData",
                    "DamageTypeData",
                    vec![
                        schema_column("DamageTypeID", ColumnType::String, true),
                        schema_column("IsElemental", ColumnType::Boolean, false),
                    ],
                ),
            ],
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        }
    }

    fn schema_table(
        table_name: &str,
        row_type_name: &str,
        columns: Vec<GameSystemColumnSchema>,
    ) -> GameSystemTableSchema {
        GameSystemTableSchema {
            table_name: table_name.to_owned(),
            table_name_crc: Crc32::from_str_lower(table_name).value(),
            row_type_name: row_type_name.to_owned(),
            row_type_crc: Crc32::from_str_lower(row_type_name).value(),
            row_count: 1,
            sources: vec![format!("{table_name}.datasheet")],
            columns,
        }
    }

    fn schema_column(
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
            non_empty_rows: usize::from(row_key),
            empty_rows: usize::from(!row_key),
            distinct_values: usize::from(row_key),
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: true,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                qualified_reference_like: false,
                list: None,
                foreign_keys: Vec::new(),
            },
        }
    }
}

fn rust_standalone_schema_field_type(column_type: ColumnType, required: bool) -> &'static str {
    match (column_type, required) {
        (ColumnType::String, true) => "String",
        (ColumnType::String, false) => "Option<String>",
        (ColumnType::Number, true) => "f32",
        (ColumnType::Number, false) => "Option<f32>",
        (ColumnType::Boolean, true) => "bool",
        (ColumnType::Boolean, false) => "Option<bool>",
    }
}

fn rust_standalone_schema_field_read_expression(field: &RustStandaloneSchemaField) -> String {
    let column = rust_string_literal(&field.source_name);
    match (field.column_type, field.required) {
        (ColumnType::String, true) => {
            format!("required_schema_string_cell(table, row, {column})?")
        }
        (ColumnType::String, false) => {
            format!("optional_schema_string_cell(table, row, {column})?")
        }
        (ColumnType::Number, true) => format!("required_number_cell(table, row, {column})?"),
        (ColumnType::Number, false) => format!("optional_number_cell(table, row, {column})?"),
        (ColumnType::Boolean, true) => format!("required_bool_cell(table, row, {column})?"),
        (ColumnType::Boolean, false) => format!("optional_bool_cell(table, row, {column})?"),
    }
}

fn rust_standalone_schema_reader_name(row_type: &str) -> String {
    format!("read_{}", to_snake_ident(row_type, "row"))
}

fn push_rust_standalone_manager_surfaces(
    source: &mut String,
    surfaces: &[ManagerSurface],
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Result<()> {
    for surface in surfaces {
        match surface {
            ManagerSurface::Direct(manager) => {
                push_rust_direct_manager_wrapper(source, manager, schema_report)?;
            }
            ManagerSurface::Native {
                manager,
                shape,
                dependencies,
                ..
            } => {
                push_rust_native_manager_wrapper(
                    source,
                    manager,
                    shape,
                    dependencies,
                    schema_report,
                )?;
            }
            ManagerSurface::Semantic(record) => push_rust_semantic_manager_wrapper(source, record),
            ManagerSurface::ItemData(manager) => {
                push_rust_item_data_manager_wrapper(source, manager)
            }
            ManagerSurface::Composition(manager) => {
                push_rust_composition_manager_wrapper(source, manager)
            }
            ManagerSurface::ProductBacked(manager) => {
                push_rust_direct_manager_wrapper(source, manager, schema_report)?;
            }
        }
    }
    Ok(())
}

fn push_rust_managers_facade(
    source: &mut String,
    accessors: &mut String,
    surfaces: &[ManagerSurface],
) {
    let mut fields = String::new();
    let mut field_values = String::new();
    let mut methods = String::new();
    let mut seen = BTreeSet::new();
    for surface in surfaces {
        let manager_name = manager_surface_name(surface);
        if !seen.insert(manager_name) {
            continue;
        }
        let manager_type = match surface {
            ManagerSurface::Direct(manager) | ManagerSurface::ProductBacked(manager) => {
                manager.manager_class_name.as_str()
            }
            ManagerSurface::Native { manager, .. } => manager.manager_class_name.as_str(),
            ManagerSurface::Semantic(record) => record.manager_class_name.as_str(),
            ManagerSurface::ItemData(manager) => manager.manager_class_name.as_str(),
            ManagerSurface::Composition(manager) => manager.manager_class_name.as_str(),
        };
        let accessor = rust_manager_accessor_name(manager_name);
        fields.push_str(&format!(
            "    {accessor}: once_cell::sync::OnceCell<{manager_type}>,\n"
        ));
        let build = match surface {
            ManagerSurface::Composition(manager) => {
                let dependencies = manager
                    .dependencies
                    .iter()
                    .map(|dependency| format!("self.{}()?", rust_manager_accessor_name(dependency)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("surfaces::{manager_type}::from_managers({dependencies})")
            }
            ManagerSurface::Native { dependencies, .. } => {
                let dependencies = dependencies
                    .iter()
                    .map(|dependency| format!("self.{}()?", rust_manager_accessor_name(dependency)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let arguments = if dependencies.is_empty() {
                    "&mut cache".to_owned()
                } else {
                    format!("&mut cache, {dependencies}")
                };
                format!(
                    "let mut cache = self.lock_cache({manager_name:?})?;\n            surfaces::{manager_type}::build({arguments})"
                )
            }
            ManagerSurface::Direct(_)
            | ManagerSurface::Semantic(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::ProductBacked(_) => format!(
                "let mut cache = self.lock_cache({manager_name:?})?;\n            surfaces::{manager_type}::build(&mut cache)"
            ),
        };
        field_values.push_str(&format!(
            "            {accessor}: once_cell::sync::OnceCell::new(),\n"
        ));
        methods.push_str(&format!(
            r#"    pub fn {accessor}(&self) -> ManagerResult<&{manager_type}> {{
        self.{accessor}.get_or_try_init(|| {{
            let result = {{
                {build}
            }};
            result.map_err(|source| ManagerLoadError::new({manager_name:?}, source))
        }})
    }}

"#
        ));
    }
    source.push_str(&format!(
        r#"
#[derive(Debug)]
pub struct ManagerLoadError {{
    manager: &'static str,
    source: anyhow::Error,
}}

impl ManagerLoadError {{
    fn new(manager: &'static str, source: impl Into<anyhow::Error>) -> Self {{
        Self {{ manager, source: source.into() }}
    }}

    pub const fn manager(&self) -> &'static str {{
        self.manager
    }}
}}

impl std::fmt::Display for ManagerLoadError {{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        write!(formatter, "load {{}}: {{}}", self.manager, self.source)
    }}
}}

impl std::error::Error for ManagerLoadError {{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {{
        Some(self.source.as_ref())
    }}
}}

pub type ManagerResult<T> = std::result::Result<T, ManagerLoadError>;

pub struct Managers {{
    cache: std::sync::Mutex<ManagerCache>,
{fields}}}

impl Managers {{
    pub fn new(loader: &crate::assets::AssetLoader) -> Self {{
        Self {{
            cache: std::sync::Mutex::new(ManagerCache::new(loader.clone())),
{field_values}        }}
    }}

    fn lock_cache(
        &self,
        manager: &'static str,
    ) -> ManagerResult<std::sync::MutexGuard<'_, ManagerCache>> {{
        self.cache.lock().map_err(|_| {{
            ManagerLoadError::new(manager, anyhow::anyhow!("manager asset cache lock poisoned"))
        }})
    }}
}}

"#
    ));
    accessors.push_str(&format!("impl Managers {{\n{methods}}}\n"));
}

fn push_rust_composition_manager_wrapper(source: &mut String, manager: &CompositionManagerSurface) {
    match manager.kind {
        CompositionManagerKind::ReplicationData => source.push_str(
            r#"
#[derive(Debug, Clone)]
pub struct ReplicationDataManager {
    ids: Vec<Crc32>,
    indexes_by_id: HashMap<Crc32, u16>,
}

impl ReplicationDataManager {
    pub(in crate::managers) fn from_managers(perk_data: &PerkDataManager) -> Result<Self> {
        let mut ids = Vec::new();
        ids.push(Crc32::ZERO);
        ids.extend(perk_data.perk_ids());
        let indexes_by_id = ids
            .iter()
            .copied()
            .enumerate()
            .map(|(index, id)| {
                u16::try_from(index)
                    .map(|index| (id, index))
                    .context("replication id table exceeds u16 index range")
            })
            .collect::<Result<HashMap<_, _>>>()?;
        Ok(Self { ids, indexes_by_id })
    }

    pub fn id_at(&self, index: u16) -> Crc32 {
        self.ids.get(usize::from(index)).copied().unwrap_or(Crc32::ZERO)
    }

    pub fn index_of(&self, id: impl IntoCrc32Key) -> u16 {
        let id = id.into_crc32_key();
        if id == Crc32::ZERO {
            return 0;
        }
        self.indexes_by_id.get(&id).copied().unwrap_or(0)
    }

    pub fn ids(&self) -> &[Crc32] {
        &self.ids
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}
"#,
        ),
        CompositionManagerKind::CurrencyExchangeMapping => source.push_str(
            r#"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CurrencyExchangeEndpoint {
    NonCategoricalCurrency,
    CategoricalProgression(Crc32),
}

impl CurrencyExchangeEndpoint {
    pub const fn categorical_progression_id(self) -> Option<Crc32> {
        match self {
            Self::NonCategoricalCurrency => None,
            Self::CategoricalProgression(id) => Some(id),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurrencyExchangeMapping {
    source: CurrencyExchangeEndpoint,
    target: CurrencyExchangeEndpoint,
    exchange: CurrencyExchangeData,
}

impl CurrencyExchangeMapping {
    pub const fn source(&self) -> CurrencyExchangeEndpoint { self.source }
    pub const fn target(&self) -> CurrencyExchangeEndpoint { self.target }
    pub const fn exchange(&self) -> &CurrencyExchangeData { &self.exchange }
}

#[derive(Debug, Clone)]
pub struct CurrencyExchangeMappingManager {
    mappings: Vec<CurrencyExchangeMapping>,
    mappings_by_endpoint: HashMap<(CurrencyExchangeEndpoint, CurrencyExchangeEndpoint), usize>,
}

impl CurrencyExchangeMappingManager {
    pub(in crate::managers) fn from_managers(
        currency_exchange_data: &CurrencyExchangeDataManager,
        categorical_progression_data: &CategoricalProgressionDataManager,
    ) -> Result<Self> {
        let mut manager = Self {
            mappings: Vec::new(),
            mappings_by_endpoint: HashMap::new(),
        };
        for exchange in currency_exchange_data.rows() {
            let Some(source) = currency_exchange_endpoint(
                exchange.from_currency_crc,
                exchange.from_currency_is_categorical_progression,
                categorical_progression_data,
            ) else { continue };
            let Some(target) = currency_exchange_endpoint(
                exchange.to_currency_crc,
                exchange.to_currency_is_categorical_progression,
                categorical_progression_data,
            ) else { continue };
            if matches!((source, target),
                (CurrencyExchangeEndpoint::CategoricalProgression(source),
                 CurrencyExchangeEndpoint::CategoricalProgression(target)) if source == target)
            {
                continue;
            }
            let key = (source, target);
            if manager.mappings_by_endpoint.contains_key(&key) {
                continue;
            }
            let index = manager.mappings.len();
            manager.mappings_by_endpoint.insert(key, index);
            manager.mappings.push(CurrencyExchangeMapping {
                source,
                target,
                exchange: exchange.clone(),
            });
        }
        Ok(manager)
    }

    pub fn mapping(
        &self,
        source: CurrencyExchangeEndpoint,
        target: CurrencyExchangeEndpoint,
    ) -> Option<&CurrencyExchangeMapping> {
        self.mappings.get(*self.mappings_by_endpoint.get(&(source, target))?)
    }

    pub fn currency_exchange(
        &self,
        source: CurrencyExchangeEndpoint,
        target: CurrencyExchangeEndpoint,
    ) -> Option<&CurrencyExchangeData> {
        self.mapping(source, target).map(CurrencyExchangeMapping::exchange)
    }

    pub fn conversion_id(
        &self,
        source: CurrencyExchangeEndpoint,
        target: CurrencyExchangeEndpoint,
    ) -> Option<Crc32> {
        self.currency_exchange(source, target).map(|exchange| exchange.conversion_crc)
    }

    pub fn mappings(&self) -> impl ExactSizeIterator<Item = &CurrencyExchangeMapping> + '_ {
        self.mappings.iter()
    }

    pub fn len(&self) -> usize { self.mappings.len() }
    pub fn is_empty(&self) -> bool { self.mappings.is_empty() }
}

fn currency_exchange_endpoint(
    currency_id: Crc32,
    is_categorical_progression: bool,
    categorical_progression_data: &CategoricalProgressionDataManager,
) -> Option<CurrencyExchangeEndpoint> {
    if !is_categorical_progression {
        return Some(CurrencyExchangeEndpoint::NonCategoricalCurrency);
    }
    let progression = categorical_progression_data
        .categorical_progression_data_from_id(currency_id)?;
    Some(CurrencyExchangeEndpoint::CategoricalProgression(
        progression.categorical_progression_id_crc,
    ))
}
"#,
        ),
        CompositionManagerKind::VitalsModifierMapping => source.push_str(
            r#"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VitalsModifierMapping {
    key: String,
    id: Crc32,
}

impl VitalsModifierMapping {
    pub fn key(&self) -> &str { &self.key }
    pub const fn id(&self) -> Crc32 { self.id }
}

#[derive(Debug, Clone)]
pub struct VitalsModifierMappingManager {
    entries: Vec<VitalsModifierMapping>,
    entries_by_id: HashMap<Crc32, usize>,
}

impl VitalsModifierMappingManager {
    pub(in crate::managers) fn from_managers(
        vitals_data: &VitalsDataManager,
        damage_data: &DamageDataManager,
        item_data: &ItemDataManager,
    ) -> Result<Self> {
        let mut manager = Self { entries: Vec::new(), entries_by_id: HashMap::new() };
        for entry in vitals_data.rows() {
            manager.insert_lowercase(&entry.key);
        }
        for entry in damage_data.damage_types() {
            manager.insert_lowercase(&entry.key);
        }
        for entry in damage_data.rows() {
            let category = normalize_weapon_category(&entry.weapon_category);
            manager.insert_lowercase(category);
        }
        manager.insert_lowercase("Physical");
        manager.insert_lowercase("Elemental");
        for item in item_data.rows() {
            manager.insert_item_aliases(item.item_id(), item.item_id_crc());
        }
        Ok(manager)
    }

    pub fn get(&self, id: impl IntoCrc32Key) -> Option<&VitalsModifierMapping> {
        self.entries.get(*self.entries_by_id.get(&id.into_crc32_key())?)
    }
    pub fn by_key(&self, key: impl AsRef<str>) -> Option<&VitalsModifierMapping> {
        self.get(Crc32::from_str_lower(key.as_ref()))
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &VitalsModifierMapping> + '_ {
        self.entries.iter()
    }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    fn insert_lowercase(&mut self, key: &str) {
        let key = key.trim();
        if key.is_empty() { return; }
        self.insert_with_id(key, Crc32::from_str_lower(key));
    }
    fn insert_item_aliases(&mut self, key: &str, id: Crc32) {
        let key = key.trim();
        if key.is_empty() || id == Crc32::ZERO { return; }
        let index = self.insert_with_id(key, id);
        let lowercase_id = Crc32::from_str_lower(key);
        if lowercase_id != Crc32::ZERO {
            self.entries_by_id.entry(lowercase_id).or_insert(index);
        }
    }
    fn insert_with_id(&mut self, key: &str, id: Crc32) -> usize {
        if id == Crc32::ZERO { return 0; }
        if let Some(index) = self.entries_by_id.get(&id).copied() { return index; }
        let index = self.entries.len();
        self.entries_by_id.insert(id, index);
        self.entries.push(VitalsModifierMapping { key: key.to_owned(), id });
        index
    }
}

fn normalize_weapon_category(value: &str) -> &str {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") { "Default" } else { value }
}
"#,
        ),
        CompositionManagerKind::StaticTradeskillRankDataMapping => source.push_str(
            r#"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticTradeskillRankDataMapping {
    categorical_progression_id: Crc32,
    table: TradeskillRankDataTable,
    rank: TradeskillRank,
}

impl StaticTradeskillRankDataMapping {
    pub const fn categorical_progression_id(&self) -> Crc32 {
        self.categorical_progression_id
    }
    pub const fn table(&self) -> TradeskillRankDataTable { self.table }
    pub const fn rank(&self) -> TradeskillRank { self.rank }
}

#[derive(Debug, Clone)]
pub struct StaticTradeskillRankDataMappingManager {
    player_levels: HashMap<Crc32, TradeskillRank>,
    tradeskill_ranks: HashMap<Crc32, StaticTradeskillRankDataMapping>,
}

impl StaticTradeskillRankDataMappingManager {
    pub(in crate::managers) fn from_managers(
        experience_data: &ExperienceDataManager,
        player_data: &PlayerDataManager,
        categorical_progression_data: &CategoricalProgressionDataManager,
        tradeskill_rank_data: &TradeskillRankDataManager,
    ) -> Result<Self> {
        let max_player_level = experience_data
            .rows()
            .filter_map(|entry| f32_to_u16(entry.row.level_number).ok())
            .max()
            .unwrap_or(0);
        let mut manager = Self {
            player_levels: HashMap::new(),
            tradeskill_ranks: HashMap::new(),
        };

        // XPLevels has no authored display-name column in this build, so the native
        // player-level display-name map is empty. Keep the max-level validation here
        // so malformed XP levels still fail construction instead of being truncated.
        for entry in experience_data.rows() {
            let level = f32_to_u16(entry.row.level_number)?;
            if level > max_player_level {
                bail!("player level {level} exceeds computed maximum {max_player_level}");
            }
        }

        let mut progressions_by_table = HashMap::new();
        for tradeskill in TRADESKILL_NAMES {
            let Some(progression_id) = player_data.categorical_progression_id(tradeskill) else {
                continue;
            };
            let Some(progression) = categorical_progression_data
                .categorical_progression_data_from_id(progression_id)
            else { continue };
            let Some(table) = progression.rank_table_id.as_deref() else { continue };
            progressions_by_table.entry(normalize_data_path(table)).or_insert((
                progression.categorical_progression_id_crc,
                progression.max_level,
            ));
        }

        for entry in tradeskill_rank_data.rows() {
            let Some((progression_id, max_level)) = progressions_by_table
                .get(&normalize_data_path(entry.source.table().catalog_name()))
            else { continue };
            if u32::from(entry.rank.value()) > *max_level { continue; }
            let Some(display_name) = entry.display_name.as_deref() else { continue };
            let display_name = display_name.trim();
            if display_name.is_empty() { continue; }
            let display_name_id = entry.display_name_id;
            if display_name_id == Crc32::ZERO { continue; }
            manager.tradeskill_ranks.entry(display_name_id).or_insert_with(|| {
                StaticTradeskillRankDataMapping {
                    categorical_progression_id: *progression_id,
                    table: entry.table,
                    rank: entry.rank,
                }
            });
        }
        Ok(manager)
    }

    pub fn player_level_for_display_name(
        &self,
        display_name: impl IntoCrc32Key,
    ) -> Option<TradeskillRank> {
        self.player_levels.get(&display_name.into_crc32_key()).copied()
    }

    pub fn tradeskill_rank_for_display_name(
        &self,
        display_name: impl IntoCrc32Key,
    ) -> Option<&StaticTradeskillRankDataMapping> {
        self.tradeskill_ranks.get(&display_name.into_crc32_key())
    }

    pub fn player_levels(
        &self,
    ) -> impl ExactSizeIterator<Item = (Crc32, TradeskillRank)> + '_ {
        self.player_levels.iter().map(|(id, rank)| (*id, *rank))
    }

    pub fn tradeskill_ranks(
        &self,
    ) -> impl ExactSizeIterator<Item = &StaticTradeskillRankDataMapping> + '_ {
        self.tradeskill_ranks.values()
    }

    pub fn len(&self) -> usize { self.player_levels.len() + self.tradeskill_ranks.len() }
    pub fn is_empty(&self) -> bool {
        self.player_levels.is_empty() && self.tradeskill_ranks.is_empty()
    }
}

const TRADESKILL_NAMES: &[&str] = &[
    "Arcana", "Armoring", "Cooking", "Engineering", "Fishing", "Furnishing",
    "Harvesting", "Jewelcrafting", "Leatherworking", "Logging", "Mining", "Musician",
    "Riding", "Skinning", "Smelting", "Stonecutting", "Weaponsmithing", "Weaving",
    "Woodworking",
];

fn f32_to_u16(value: f32) -> Result<u16> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > f32::from(u16::MAX) {
        bail!("rank value {value} is not an exact u16")
    }
    Ok(value as u16)
}
"#,
        ),
    }
}

fn push_rust_direct_manager_wrapper(
    source: &mut String,
    manager: &DirectManagerSurface,
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Result<()> {
    push_rust_direct_manager_wrapper_with_dependencies(
        source,
        manager,
        &[],
        schema_report,
        RustNativeManagerAugmentation::default(),
    )
}

fn push_rust_direct_manager_wrapper_with_dependencies(
    source: &mut String,
    manager: &DirectManagerSurface,
    dependencies: &[String],
    schema_report: &GameSystemDataTablesSchemaReport,
    augmentation: RustNativeManagerAugmentation,
) -> Result<()> {
    let manager_name = &manager.manager_class_name;
    let manager_resources = rust_direct_manager_resources_expression(manager);
    let table_types = rust_direct_table_types(manager, schema_report);
    let mut product_methods = rust_direct_product_methods(manager)?;
    product_methods.push_str(rust_standalone_special_manager_extra_methods(manager_name));
    let has_semantic_rows = !augmentation.rows_type.is_empty();
    let row_methods = rust_direct_schema_methods(manager, schema_report, !has_semantic_rows);
    let rows_trait_impl = if has_semantic_rows {
        rust_semantic_rows_trait_impl(manager, &augmentation.rows_type, &augmentation.rows_method)
    } else {
        rust_direct_rows_trait_impl(manager, schema_report)
    };
    let row_specs = rust_direct_row_specs(manager, schema_report);
    let row_fields = row_specs
        .iter()
        .map(|row| {
            let table_type = rust_direct_table_type_for_row(manager, schema_report, row);
            format!(
                "    {}: RowCollection<{table_type}, {}>,\n",
                rust_direct_row_field_name(&row.source_row_type),
                row.type_name
            )
        })
        .collect::<String>();
    let row_initializers = row_specs
        .iter()
        .map(|row| {
            let field = rust_direct_row_field_name(&row.source_row_type);
            let reader = rust_standalone_schema_reader_name(&row.source_row_type);
            let table_type = rust_direct_table_type_for_row(manager, schema_report, row);
            format!(
                "        let {field} = RowCollection::new(resources.schema_family_entries({:?}, {table_type}::from_path, {reader})?, {table_type}::catalog_name);\n",
                row.source_row_type
            )
        })
        .collect::<String>();
    let row_field_values = row_specs
        .iter()
        .map(|row| {
            format!(
                "            {},\n",
                rust_direct_row_field_name(&row.source_row_type)
            )
        })
        .collect::<String>();
    let (product_fields, product_initializers, product_field_values) =
        rust_product_storage(manager)?;
    let dependency_parameters = dependencies
        .iter()
        .map(|dependency| {
            format!(
                ", _{}: &{}",
                rust_manager_dependency_name(dependency),
                dependency
            )
        })
        .collect::<String>();
    source.push_str(&format!(
        r#"
{declarations}
{table_types}
#[derive(Debug, Clone)]
pub struct {manager_name} {{
{row_fields}
{product_fields}
{augmentation_fields}
}}

impl {manager_name} {{
    pub(in crate::managers) fn build(cache: &mut ManagerCache{dependency_parameters}) -> Result<Self> {{
        let resources = {manager_resources}?;
{row_initializers}
{product_initializers}
{augmentation_initializers}
        Ok(Self {{
{row_field_values}
{product_field_values}
{augmentation_field_values}
        }})
    }}

{row_methods}
{product_methods}
{augmentation_methods}
}}
{rows_trait_impl}
"#
        ,
        declarations = augmentation.declarations,
        augmentation_fields = augmentation.fields,
        augmentation_initializers = augmentation.initializers,
        augmentation_field_values = augmentation.field_values,
        augmentation_methods = augmentation.methods,
    ));
    Ok(())
}

fn push_rust_native_manager_wrapper(
    source: &mut String,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
    dependencies: &[String],
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Result<()> {
    let effective = rust_effective_native_manager_surface(manager, shape);
    let augmentation = augment_native_manager(&effective, shape, schema_report)?;
    push_rust_direct_manager_wrapper_with_dependencies(
        source,
        &effective,
        dependencies,
        schema_report,
        augmentation,
    )
}

fn rust_effective_native_manager_surface(
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> DirectManagerSurface {
    let mut effective = manager.clone();
    if let NativeManagerShape::RecipeData(shape) = shape {
        for table in shape.tables() {
            let candidate = crate::manager_records::DirectManagerTable {
                table_name: table.table_name().as_str().to_owned(),
                row_type_name: table.row_type_name().as_str().to_owned(),
            };
            if !effective.tables.contains(&candidate) {
                effective.tables.push(candidate);
            }
        }
    }
    effective
}

fn rust_direct_rows_trait_impl(
    manager: &DirectManagerSurface,
    schema_report: &GameSystemDataTablesSchemaReport,
) -> String {
    let Some(row) = rust_direct_default_row_spec(manager, schema_report) else {
        return String::new();
    };
    let manager_name = &manager.manager_class_name;
    let table_type = rust_direct_table_type_for_row(manager, schema_report, &row);
    let row_type = &row.type_name;
    format!(
        r#"
impl Rows for {manager_name} {{
    type Row = RowEntry<{table_type}, {row_type}>;

    fn rows(&self) -> impl Iterator<Item = &Self::Row> {{
        {manager_name}::rows(self)
    }}
}}
"#,
    )
}

fn rust_semantic_rows_trait_impl(
    manager: &DirectManagerSurface,
    row_type: &str,
    rows_method: &str,
) -> String {
    let manager_name = &manager.manager_class_name;
    format!(
        r#"
impl Rows for {manager_name} {{
    type Row = {row_type};

    fn rows(&self) -> impl Iterator<Item = &Self::Row> {{
        {manager_name}::{rows_method}(self)
    }}
}}
"#,
    )
}

fn push_rust_item_data_manager_wrapper(source: &mut String, manager: &ItemDataManagerSurface) {
    let manager_name = &manager.manager_class_name;
    let factory = to_snake_ident(manager_name, "manager");
    let table_type = &manager.table_type_name;
    let handle_type = &manager.handle_type_name;
    let data_type = &manager.data_type_name;
    let table_variants = manager
        .tables
        .iter()
        .map(|table| format!("    {},\n", table.variant_name))
        .collect::<String>();
    let table_name_arms = manager
        .tables
        .iter()
        .map(|table| {
            format!(
                "            Self::{} => {},\n",
                table.variant_name,
                rust_string_literal(&table.table_name)
            )
        })
        .collect::<String>();
    let table_selector_arms = manager
        .tables
        .iter()
        .map(|table| {
            format!(
                "            Self::{} => TableSelector::new({}, {}),\n",
                table.variant_name,
                rust_string_literal(&table.table_name),
                rust_string_literal(&table.row_type_name)
            )
        })
        .collect::<String>();
    let table_list = manager
        .tables
        .iter()
        .map(|table| format!("    {table_type}::{},\n", table.variant_name))
        .collect::<String>();
    let manager_resources = rust_manager_resources_expression(
        &manager.manager_name,
        manager
            .tables
            .iter()
            .map(|table| (table.table_name.as_str(), table.row_type_name.as_str())),
        std::iter::empty(),
    );

    source.push_str(&format!(
        r#"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum {table_type} {{
{table_variants}}}

impl {table_type} {{
    pub(in crate::managers) const fn catalog_name(self) -> &'static str {{
        match self {{
{table_name_arms}        }}
    }}

    const fn selector(self) -> TableSelector {{
        match self {{
{table_selector_arms}        }}
    }}
}}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct {handle_type} {{
    table: {table_type},
    row: u32,
}}

impl {handle_type} {{
    #[must_use]
    pub const fn new(table: {table_type}, row: u32) -> Self {{
        Self {{ table, row }}
    }}

    #[must_use]
    pub const fn table(self) -> {table_type} {{
        self.table
    }}

    #[must_use]
    pub const fn row(self) -> u32 {{
        self.row
    }}

    #[must_use]
    pub const fn row_index(self) -> u32 {{
        self.row
    }}
}}

#[derive(Debug, Clone, PartialEq)]
pub struct {data_type} {{
    source_handle: {handle_type},
    definition: MasterItemDefinitionsSchemaRow,
    item_id: String,
    item_id_crc: Crc32,
    name: Option<String>,
    description: Option<String>,
    item_type: Option<String>,
    item_type_display_name: Option<String>,
    ui_item_class: Option<String>,
    heartgem_rune_tooltip_title: Option<String>,
    confirm_before_use: bool,
    consume_on_use: bool,
    bind_on_pickup: bool,
    death_drop_percentage: f32,
}}

impl {data_type} {{
    #[must_use]
    pub const fn definition(&self) -> &MasterItemDefinitionsSchemaRow {{
        &self.definition
    }}

    #[must_use]
    pub const fn source_handle(&self) -> {handle_type} {{
        self.source_handle
    }}

    #[must_use]
    pub const fn source_table(&self) -> {table_type} {{
        self.source_handle.table()
    }}

    #[must_use]
    pub const fn source_row(&self) -> u32 {{
        self.source_handle.row()
    }}

    #[must_use]
    pub fn item_id(&self) -> &str {{
        &self.item_id
    }}

    #[must_use]
    pub const fn item_id_crc(&self) -> Crc32 {{
        self.item_id_crc
    }}

    #[must_use]
    pub fn name(&self) -> Option<&str> {{
        self.name.as_deref()
    }}

    #[must_use]
    pub fn description(&self) -> Option<&str> {{
        self.description.as_deref()
    }}

    #[must_use]
    pub fn item_type(&self) -> Option<&str> {{
        self.item_type.as_deref()
    }}

    #[must_use]
    pub fn item_type_display_name(&self) -> Option<&str> {{
        self.item_type_display_name.as_deref()
    }}

    #[must_use]
    pub fn ui_item_class(&self) -> Option<&str> {{
        self.ui_item_class.as_deref()
    }}

    #[must_use]
    pub fn heartgem_rune_tooltip_title(&self) -> Option<&str> {{
        self.heartgem_rune_tooltip_title.as_deref()
    }}

    #[must_use]
    pub const fn confirm_before_use(&self) -> bool {{
        self.confirm_before_use
    }}

    #[must_use]
    pub const fn consume_on_use(&self) -> bool {{
        self.consume_on_use
    }}

    #[must_use]
    pub const fn bind_on_pickup(&self) -> bool {{
        self.bind_on_pickup
    }}

    #[must_use]
    pub const fn death_drop_percentage(&self) -> f32 {{
        self.death_drop_percentage
    }}
}}

const ITEM_DATA_MANAGER_TABLES: &[{table_type}] = &[
{table_list}];

#[derive(Debug, Clone)]
pub struct {manager_name} {{
    items: Arc<Vec<{data_type}>>,
    items_by_id: Arc<HashMap<Crc32, usize>>,
}}

impl {manager_name} {{
    pub(in crate::managers) fn build(cache: &mut ManagerCache) -> Result<Self> {{
        let resources = {manager_resources}?;
        let items = materialize_{factory}(&resources)?;
        let mut items_by_id = HashMap::new();
        for (index, item) in items.iter().enumerate() {{
            items_by_id.insert(item.item_id_crc, index);
        }}
        Ok(Self {{
            items: Arc::new(items),
            items_by_id: Arc::new(items_by_id),
        }})
    }}

    #[must_use]
    pub fn get(&self, item_id: impl AsRef<str>) -> Option<&{data_type}> {{
        self.get_from_id(Crc32::from_str_lower(item_id.as_ref()))
    }}

    #[must_use]
    pub fn get_from_id(&self, item_id: Crc32) -> Option<&{data_type}> {{
        self.items.get(*self.items_by_id.get(&item_id)?)
    }}

    #[must_use]
    pub fn by_index(&self, index: std::num::NonZeroU32) -> Option<&{data_type}> {{
        let zero_based = usize::try_from(index.get() - 1).ok()?;
        self.items.get(zero_based)
    }}

    pub fn rows(&self) -> std::slice::Iter<'_, {data_type}> {{
        self.items.iter()
    }}

    #[must_use]
    pub fn len(&self) -> usize {{
        self.items.len()
    }}

    #[must_use]
    pub fn is_empty(&self) -> bool {{
        self.items.is_empty()
    }}
}}

impl Rows for {manager_name} {{
    type Row = {data_type};

    fn rows(&self) -> impl Iterator<Item = &Self::Row> {{
        {manager_name}::rows(self)
    }}
}}

fn materialize_{factory}(resources: &ManagerResources) -> Result<Vec<{data_type}>> {{
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for table_id in ITEM_DATA_MANAGER_TABLES {{
        let table = resources.table(table_id.selector()).with_context(|| {{
            format!(
                "manager {{}} table {{}} was not loaded",
                resources.manager_name,
                table_id.catalog_name()
            )
        }})?;
        cache_item_data_rows(&mut items, &mut seen, *table_id, table)?;
    }}
    Ok(items)
}}

fn cache_item_data_rows(
    items: &mut Vec<{data_type}>,
    seen: &mut HashSet<Crc32>,
    table_id: {table_type},
    table: &DynamicTable,
) -> Result<()> {{
    for source_row in &table.rows {{
        let definition = read_master_item_definitions(table, source_row)?;
        let item_id = definition.item_id.trim().to_owned();
        if item_id.is_empty() {{
            continue;
        }}
        let item_id_crc = Crc32::from_str_lower(&item_id);
        if item_id_crc == Crc32::ZERO || !seen.insert(item_id_crc) {{
            continue;
        }}
        let row = u32::try_from(source_row.row_index + 1).with_context(|| {{
            format!(
                "row {{}}:{{}} source row exceeds u32",
                source_row.source_path,
                source_row.row_index + 1
            )
        }})?;
        items.push({data_type} {{
            source_handle: {handle_type}::new(table_id, row),
            definition,
            item_id,
            item_id_crc,
            name: optional_string_cell(table, source_row, "Name")?.map(str::to_owned),
            description: optional_string_cell(table, source_row, "Description")?.map(str::to_owned),
            item_type: optional_string_cell(table, source_row, "ItemType")?.map(str::to_owned),
            item_type_display_name: optional_string_cell(table, source_row, "ItemTypeDisplayName")?.map(str::to_owned),
            ui_item_class: optional_string_cell(table, source_row, "UiItemClass")?.map(str::to_owned),
            heartgem_rune_tooltip_title: optional_string_cell(table, source_row, "HeartgemRuneTooltipTitle")?.map(str::to_owned),
            confirm_before_use: optional_bool_cell(table, source_row, "ConfirmBeforeUse")?.unwrap_or(false),
            consume_on_use: optional_bool_cell(table, source_row, "ConsumeOnUse")?.unwrap_or(false),
            bind_on_pickup: optional_bool_cell(table, source_row, "BindOnPickup")?.unwrap_or(false),
            death_drop_percentage: optional_number_cell(table, source_row, "DeathDropPercentage")?.unwrap_or(0.0),
        }});
    }}
    Ok(())
}}

"#
    ));
}

fn rust_direct_schema_methods(
    manager: &DirectManagerSurface,
    schema_report: &GameSystemDataTablesSchemaReport,
    include_primary_rows: bool,
) -> String {
    let default_row_type =
        rust_direct_default_row_spec(manager, schema_report).map(|row| row.source_row_type);
    let mut source = String::new();
    for row_spec in rust_direct_row_specs(manager, schema_report) {
        let source_row_type = &row_spec.source_row_type;
        let is_default_row_type = default_row_type.as_deref() == Some(source_row_type.as_str());
        let table_type = rust_direct_table_type_name(manager, source_row_type, is_default_row_type);
        if is_default_row_type {
            source.push_str(&rust_direct_primary_row_family_methods(
                &row_spec,
                &table_type,
                include_primary_rows,
            ));
        } else {
            let accessor = format!("{}_rows", to_snake_ident(source_row_type, "rows"));
            let row_type = &row_spec.type_name;
            let field = rust_direct_row_field_name(source_row_type);
            let table_method = format!("{accessor}_table");
            let reference_method = format!("{accessor}_ref");
            let slot_method = format!("{accessor}_slot");
            source.push_str(&format!(
                r#"    pub fn {accessor}(&self) -> &RowCollection<{table_type}, {row_type}> {{
        &self.{field}
    }}

    pub fn {table_method}(&self, table: {table_type}) -> TableRows<'_, {table_type}, {row_type}> {{
        self.{field}.table(table)
    }}

    pub fn {reference_method}(
        &self,
        table: {table_type},
        key: impl Into<String>,
    ) -> RowRef<{table_type}, {row_type}> {{
        RowRef::new(table, key)
    }}

    pub fn {slot_method}(
        &self,
        table: {table_type},
        row_index: usize,
    ) -> RowSlot<{table_type}, {row_type}> {{
        RowSlot::new(table, row_index)
    }}

"#
            ));
        }
    }
    source
}

fn rust_direct_row_specs(
    manager: &DirectManagerSurface,
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Vec<RustStandaloneSchemaRow> {
    let row_specs = rust_standalone_schema_rows(schema_report);
    let mut seen = BTreeSet::new();
    manager
        .tables
        .iter()
        .filter_map(|table| {
            seen.insert(table.row_type_name.clone())
                .then_some(table.row_type_name.as_str())
        })
        .filter_map(|row_type| {
            row_specs
                .iter()
                .find(|row| row.source_row_type == row_type)
                .cloned()
        })
        .collect()
}

fn rust_direct_default_row_spec(
    manager: &DirectManagerSurface,
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Option<RustStandaloneSchemaRow> {
    let row_specs = rust_direct_row_specs(manager, schema_report);
    let row_types = row_specs
        .iter()
        .map(|row| row.source_row_type.clone())
        .collect::<Vec<_>>();
    let default_row_type = default_direct_manager_row_type(&manager.manager_name, &row_types)?;
    row_specs
        .into_iter()
        .find(|row| row.source_row_type == default_row_type)
}

fn push_rust_direct_row_family_types(
    _source: &mut String,
    _surfaces: &[ManagerSurface],
    _schema_report: &GameSystemDataTablesSchemaReport,
) {
}

fn rust_direct_table_types(
    manager: &DirectManagerSurface,
    schema_report: &GameSystemDataTablesSchemaReport,
) -> String {
    let default_row_type =
        rust_direct_default_row_spec(manager, schema_report).map(|row| row.source_row_type);
    rust_direct_row_specs(manager, schema_report)
        .into_iter()
        .map(|row| {
            let is_default = default_row_type.as_deref() == Some(row.source_row_type.as_str());
            let type_name = rust_direct_table_type_name(manager, &row.source_row_type, is_default);
            let tables = rust_direct_family_tables(manager, &row.source_row_type);
            let variants = tables
                .iter()
                .map(|(variant, _)| format!("    {variant},\n"))
                .collect::<String>();
            let table_name_arms = tables
                .iter()
                .map(|(variant, table)| {
                    format!(
                        "            Self::{variant} => {},\n",
                        rust_string_literal(table)
                    )
                })
                .collect::<String>();
            let from_catalog_checks = tables
                .iter()
                .map(|(variant, table)| {
                    format!(
                        "        if table_path_matches(name, {}) {{ return Some(Self::{variant}); }}\n",
                        rust_string_literal(table)
                    )
                })
                .collect::<String>();
            format!(
                r#"#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum {type_name} {{
{variants}}}

impl {type_name} {{
    pub(in crate::managers) const fn catalog_name(self) -> &'static str {{
        match self {{
{table_name_arms}        }}
    }}

    pub fn from_path(name: &str) -> Option<Self> {{
{from_catalog_checks}        None
    }}
}}

"#
            )
        })
        .collect()
}

fn rust_direct_table_type_for_row(
    manager: &DirectManagerSurface,
    schema_report: &GameSystemDataTablesSchemaReport,
    row: &RustStandaloneSchemaRow,
) -> String {
    let is_default = rust_direct_default_row_spec(manager, schema_report)
        .is_some_and(|default| default.source_row_type == row.source_row_type);
    rust_direct_table_type_name(manager, &row.source_row_type, is_default)
}

fn rust_direct_table_type_name(
    manager: &DirectManagerSurface,
    source_row_type: &str,
    is_default: bool,
) -> String {
    let manager_base = manager
        .manager_class_name
        .strip_suffix("Manager")
        .unwrap_or(&manager.manager_class_name);
    if is_default {
        format!("{manager_base}Table")
    } else {
        format!(
            "{manager_base}{}Table",
            to_upper_camel_ident(source_row_type, "Rows")
        )
    }
}

fn rust_direct_family_tables(
    manager: &DirectManagerSurface,
    source_row_type: &str,
) -> Vec<(String, String)> {
    let mut variants = BTreeMap::<String, usize>::new();
    manager
        .tables
        .iter()
        .filter(|table| table.row_type_name == source_row_type)
        .map(|table| {
            let base = to_upper_camel_ident(&table.table_name, "Table");
            let count = variants.entry(base.clone()).or_default();
            *count += 1;
            let variant = if *count == 1 {
                base
            } else {
                format!("{base}{}", *count)
            };
            (variant, table.table_name.clone())
        })
        .collect()
}

fn rust_direct_primary_row_family_methods(
    row_spec: &RustStandaloneSchemaRow,
    table_type: &str,
    include_rows: bool,
) -> String {
    let source_row_type = &row_spec.source_row_type;
    let row_type = &row_spec.type_name;
    let field = rust_direct_row_field_name(source_row_type);
    let rows = if include_rows {
        format!(
            r#"    pub fn rows(&self) -> std::slice::Iter<'_, RowEntry<{table_type}, {row_type}>> {{
        self.{field}.rows()
    }}

"#,
        )
    } else {
        String::new()
    };
    format!(
        r#"{rows}    pub fn table(&self, table: {table_type}) -> TableRows<'_, {table_type}, {row_type}> {{
        self.{field}.table(table)
    }}

    pub fn resolve_row(&self, reference: TableReference<'_>) -> Option<&{row_type}> {{
        self.table({table_type}::from_path(reference.path())?)
            .get(reference.key())
    }}

    pub fn row_ref(&self, table: {table_type}, key: impl Into<String>) -> RowRef<{table_type}, {row_type}> {{
        RowRef::new(table, key)
    }}

    pub fn row_slot(&self, table: {table_type}, row_index: usize) -> RowSlot<{table_type}, {row_type}> {{
        RowSlot::new(table, row_index)
    }}

    pub fn row(&self, reference: &RowRef<{table_type}, {row_type}>) -> Option<&{row_type}> {{
        self.{field}.get(reference)
    }}

    pub fn row_by_index(&self, slot: &RowSlot<{table_type}, {row_type}>) -> Option<&{row_type}> {{
        self.{field}.row_by_index(slot)
    }}

    pub fn row_key_by_index(&self, slot: &RowSlot<{table_type}, {row_type}>) -> Option<&str> {{
        self.{field}.row_key_by_index(slot)
    }}

"#
    )
}

fn rust_direct_row_field_name(source_row_type: &str) -> String {
    format!("{}_rows", to_snake_ident(source_row_type, "rows"))
}

fn rust_product_info(value_type: &str) -> Option<(&'static str, &'static str)> {
    let kind = NativeManagerProductKind::from_canonical_type_path(value_type)?;
    let info = match kind {
        NativeManagerProductKind::ArmorOffsetDatabase => {
            ("ArmorOffsetDatabase", "parse_armor_offset_database")
        }
        NativeManagerProductKind::EquipTypesDatabase => {
            ("EquipTypesDatabase", "parse_equip_types_database")
        }
        NativeManagerProductKind::GameDebugSettings => {
            ("GameDebugSettings", "parse_game_debug_settings")
        }
        NativeManagerProductKind::PlayerBaseAttributes => {
            ("PlayerBaseAttributes", "parse_player_base_attributes")
        }
        NativeManagerProductKind::SettlementProgressionData => (
            "SettlementProgressionData",
            "parse_settlement_progression_data",
        ),
        NativeManagerProductKind::UiDatabase => ("UiDatabase", "parse_ui_database"),
        NativeManagerProductKind::GameCameraSettings => {
            ("GameCameraSettings", "parse_game_camera_settings")
        }
        NativeManagerProductKind::GatheringDatabase => {
            ("GatheringDatabase", "parse_gathering_database")
        }
        NativeManagerProductKind::GatheringActionDatabase => {
            ("GatheringActionDatabase", "parse_gathering_action_database")
        }
        NativeManagerProductKind::CraftingStationDatabase => {
            ("CraftingStationDatabase", "parse_crafting_station_database")
        }
        NativeManagerProductKind::SocialRankDatabase => {
            ("SocialRankDatabase", "parse_social_rank_database")
        }
    };
    Some(info)
}

fn rust_product_storage(manager: &DirectManagerSurface) -> Result<(String, String, String)> {
    let mut fields = String::new();
    let mut initializers = String::new();
    let mut field_values = String::new();
    let mut seen = BTreeSet::new();
    for product in &manager.products {
        let (type_name, parser) = rust_product_info(&product.value_type).with_context(|| {
            format!(
                "manager {} product {} declares unsupported Rust value type {}",
                manager.manager_name, product.path, product.value_type
            )
        })?;
        let field = to_snake_ident(type_name, "product");
        if !seen.insert(field.clone()) {
            continue;
        }
        fields.push_str(&format!("    {field}: {type_name},\n"));
        initializers.push_str(&format!(
            "        let {field} = {parser}(resources.required_asset_bytes({})?)?;\n",
            rust_string_literal(&product.path)
        ));
        field_values.push_str(&format!("            {field},\n"));
    }
    Ok((fields, initializers, field_values))
}

fn rust_direct_product_methods(manager: &DirectManagerSurface) -> Result<String> {
    let mut source = String::new();
    for product in &manager.products {
        let getter = to_snake_ident(&product.manager_getter, "asset");
        let kind = NativeManagerProductKind::from_canonical_type_path(&product.value_type)
            .with_context(|| {
                format!(
                    "manager {} product {} declares unsupported Rust value type {}",
                    manager.manager_name, product.path, product.value_type
                )
            })?;
        let (type_name, _) = rust_product_info(&product.value_type).with_context(|| {
            format!(
                "manager {} product {} declares unsupported Rust value type {}",
                manager.manager_name, product.path, product.value_type
            )
        })?;
        let field = to_snake_ident(type_name, "product");
        match kind {
            NativeManagerProductKind::ArmorOffsetDatabase => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &ArmorOffsetDatabase {{
        &self.{field}
    }}

    pub fn armor_offset(&self, name: &str) -> Option<&ArmorOffsetData> {{
        armor_offset_by_name(self.{getter}(), name)
    }}

    pub fn furthest_attachment_offset(
        &self,
        armor_offset_names: &[String],
        attachment_name: &str,
        current_position: Vec3,
    ) -> Option<&AttachmentOffsetData> {{
        furthest_armor_attachment_offset(
            self.{getter}(),
            armor_offset_names,
            attachment_name,
            current_position,
        )
    }}

"#
                ));
            }
            NativeManagerProductKind::EquipTypesDatabase => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &EquipTypesDatabase {{
        &self.{field}
    }}

    pub fn equip_types(&self) -> &[EquipTypeData] {{
        &self.{getter}().equip_types
    }}

"#
                ));
            }
            NativeManagerProductKind::GameDebugSettings => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &GameDebugSettings {{
        &self.{field}
    }}

    pub fn combat(&self) -> &CombatDebugSettings {{
        &self.{getter}().combat_settings
    }}

    pub fn disabled_combat_toggle_count(&self) -> usize {{
        disabled_combat_toggle_count(self.combat())
    }}

"#
                ));
            }
            NativeManagerProductKind::PlayerBaseAttributes => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &PlayerBaseAttributes {{
        &self.{field}
    }}

    pub fn player_attribute_data(&self) -> &PlayerAttributeData {{
        &self.{getter}().player_attribute_data
    }}

    pub fn max_perks(&self, rarity_level: usize) -> Option<i32> {{
        self.player_attribute_data()
            .item_rarity_data
            .get(rarity_level)
            .map(|rarity| rarity.max_perk_count)
    }}

"#
                ));
            }
            NativeManagerProductKind::SettlementProgressionData => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &SettlementProgressionData {{
        &self.{field}
    }}

    pub fn settlement_progression_categories(&self) -> &[ProgressionCategoryEntry] {{
        &self.{getter}().settlement_progression_categories
    }}

"#
                ));
            }
            NativeManagerProductKind::UiDatabase => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &UiDatabase {{
        &self.{field}
    }}

    pub fn interact_options(&self) -> &[InteractOptionData] {{
        &self.{getter}().unified_interact_data.interact_options
    }}

    pub fn interact_option(&self, id: Crc32) -> Option<&InteractOptionData> {{
        interact_option_by_crc(self.interact_options(), id)
    }}

    pub fn interact_option_by_name(&self, name: &str) -> Option<&InteractOptionData> {{
        self.interact_option(Crc32::from_str_lower(name))
    }}

    pub fn interact_options_by_category(
        &self,
        category: i32,
    ) -> impl Iterator<Item = &InteractOptionData> {{
        interact_options_by_category(self.interact_options(), category)
    }}

"#
                ));
            }
            NativeManagerProductKind::GameCameraSettings => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &GameCameraSettings {{
        &self.{field}
    }}

    pub fn camera_states(&self) -> &[CameraStateSettings] {{
        &self.{getter}().camera_states
    }}

"#
                ));
            }
            NativeManagerProductKind::GatheringDatabase => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &GatheringDatabase {{
        &self.{field}
    }}

    pub fn gathering_data(&self) -> &GatheringData {{
        &self.{getter}().gathering_data
    }}

    pub fn gathering_types(&self) -> &[GatheringTypeData] {{
        &self.gathering_data().gathering_types
    }}

    pub fn gathering_actions(&self) -> &[GatheringAction] {{
        &self.gathering_data().gathering_actions
    }}

"#
                ));
            }
            NativeManagerProductKind::GatheringActionDatabase => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &GatheringActionDatabase {{
        &self.{field}
    }}

    pub fn gathering_action_data(&self) -> &[GatheringActionData] {{
        &self.{getter}().gathering_actions
    }}

"#
                ));
            }
            NativeManagerProductKind::CraftingStationDatabase => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &CraftingStationDatabase {{
        &self.{field}
    }}

    pub fn crafting_stations(&self) -> &[CraftingStationData] {{
        &self.{getter}().crafting_stations
    }}

"#
                ));
            }
            NativeManagerProductKind::SocialRankDatabase => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> &SocialRankDatabase {{
        &self.{field}
    }}

    pub fn ranks(&self) -> &[SocialRankData] {{
        &self.{getter}().ranks
    }}

"#
                ));
            }
        }
    }
    Ok(source)
}

fn push_rust_semantic_enum_types(
    source: &mut String,
    shapes: &[crate::game_system_schema::GameSystemEnumShape],
) {
    for shape in shapes {
        let repr = match shape.representation {
            crate::game_system_schema::GameSystemEnumRepresentation::U8 => "u8",
            crate::game_system_schema::GameSystemEnumRepresentation::I32 => "i32",
            crate::game_system_schema::GameSystemEnumRepresentation::U32
            | crate::game_system_schema::GameSystemEnumRepresentation::Crc32 => "u32",
        };
        source.push_str(&format!(
            "#[repr({repr})]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum {} {{\n",
            shape.name
        ));
        for variant in &shape.variants {
            source.push_str(&format!(
                "    {} = {},\n",
                to_upper_camel_ident(&variant.name, "Variant"),
                variant.discriminant
            ));
        }
        source.push_str("}\n\n");
    }
}

fn push_rust_semantic_record_type(source: &mut String, record: &SemanticManagerRecord) {
    source.push_str("#[derive(Debug, Clone)]\n");
    source.push_str(&format!("pub struct {} {{\n", record.record_type_name));
    for (field_name, field_type) in rust_semantic_record_fields(record) {
        source.push_str(&format!("    pub {field_name}: {field_type},\n"));
    }
    source.push_str("}\n\n");
}

fn rust_semantic_enum_shapes(
    records: &[SemanticManagerRecord],
) -> Vec<crate::game_system_schema::GameSystemEnumShape> {
    let mut shapes = BTreeMap::new();
    for shape in records
        .iter()
        .flat_map(|record| record.fields.iter())
        .filter_map(|field| field.enum_shape.as_ref())
    {
        shapes
            .entry(shape.name.clone())
            .or_insert_with(|| shape.clone());
    }
    shapes.into_values().collect()
}

fn rust_semantic_pair_first_enum_shapes(
    records: &[SemanticManagerRecord],
) -> Vec<crate::game_system_schema::GameSystemEnumShape> {
    let mut shapes = BTreeMap::new();
    for shape in records
        .iter()
        .flat_map(|record| record.fields.iter())
        .filter_map(|field| field.pair_first_enum_shape.as_ref())
    {
        shapes
            .entry(shape.name.clone())
            .or_insert_with(|| shape.clone());
    }
    shapes.into_values().collect()
}

fn push_rust_enum_parsers(source: &mut String, records: &[SemanticManagerRecord]) {
    for shape in rust_semantic_enum_shapes(records) {
        let parser = rust_enum_parser_name(&shape.name);
        source.push_str(&format!(
            "fn {parser}(source: &str) -> Result<{}> {{\n    match source.trim() {{\n",
            shape.name
        ));
        let mut tokens = BTreeMap::<String, String>::new();
        for variant in &shape.variants {
            let variant_name = to_upper_camel_ident(&variant.name, "Variant");
            tokens
                .entry(variant.name.clone())
                .or_insert_with(|| variant_name.clone());
            for token in &variant.source_tokens {
                tokens
                    .entry(token.clone())
                    .or_insert_with(|| variant_name.clone());
            }
        }
        for (token, variant) in tokens {
            source.push_str(&format!(
                "        {token:?} => Ok({}::{}),\n",
                shape.name, variant
            ));
        }
        source.push_str(&format!(
            "        value => bail!(\"unknown {} value `{{value}}`\"),\n    }}\n}}\n\n",
            shape.name
        ));
    }
    for shape in rust_semantic_pair_first_enum_shapes(records) {
        let parser = rust_pair_enum_parser_name(&shape.name);
        source.push_str(&format!(
            "fn {parser}(source: &str) -> Result<u8> {{\n    match source.trim() {{\n"
        ));
        let mut tokens = BTreeMap::<String, i64>::new();
        for variant in &shape.variants {
            tokens
                .entry(variant.name.clone())
                .or_insert(variant.discriminant);
            for token in &variant.source_tokens {
                tokens.entry(token.clone()).or_insert(variant.discriminant);
            }
        }
        for (token, discriminant) in tokens {
            source.push_str(&format!("        {token:?} => Ok({discriminant}u8),\n"));
        }
        source.push_str(&format!(
            "        value => value.parse::<u8>().with_context(|| format!(\"unknown {} value `{{value}}`\")),\n    }}\n}}\n\n",
            shape.name
        ));
    }
}

fn rust_enum_parser_name(enum_name: &str) -> String {
    to_snake_ident(&format!("parse_{enum_name}"), "parse_enum")
}

fn rust_pair_enum_parser_name(enum_name: &str) -> String {
    to_snake_ident(
        &format!("parse_{enum_name}_discriminant"),
        "parse_enum_discriminant",
    )
}

fn rust_semantic_record_fields(record: &SemanticManagerRecord) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(field) = &record.source_row_field {
        push_rust_semantic_record_field(&mut fields, &mut seen, field, "u32");
    }
    if let Some(key) = &record.key {
        match key {
            SemanticManagerKey::Crc {
                key_field,
                crc_field,
                ..
            } => {
                push_rust_semantic_record_field(&mut fields, &mut seen, key_field, "String");
                push_rust_semantic_record_field(&mut fields, &mut seen, crc_field, "Crc32");
            }
            SemanticManagerKey::FallbackCrc {
                key_kind_field,
                key_field,
                crc_field,
                ..
            } => {
                push_rust_semantic_record_field(&mut fields, &mut seen, key_kind_field, "String");
                push_rust_semantic_record_field(&mut fields, &mut seen, key_field, "String");
                push_rust_semantic_record_field(&mut fields, &mut seen, crc_field, "Crc32");
            }
            SemanticManagerKey::Numeric {
                key_field,
                key_type,
                ..
            } => push_rust_semantic_record_field(
                &mut fields,
                &mut seen,
                key_field,
                rust_numeric_key_type(*key_type),
            ),
            SemanticManagerKey::EnumString { key_field, .. }
            | SemanticManagerKey::String { key_field, .. } => {
                push_rust_semantic_record_field(&mut fields, &mut seen, key_field, "String");
            }
        }
    }
    for field in &record.fields {
        push_rust_semantic_record_field(
            &mut fields,
            &mut seen,
            &field.name,
            &rust_projection_type(field),
        );
    }
    fields
}

fn push_rust_semantic_record_field(
    fields: &mut Vec<(String, String)>,
    seen: &mut BTreeSet<String>,
    source_name: &str,
    field_type: &str,
) {
    let field_name = rust_semantic_field_name(source_name);
    if seen.insert(field_name.clone()) {
        fields.push((field_name, field_type.to_owned()));
    }
}

fn push_rust_semantic_manager_wrapper(source: &mut String, record: &SemanticManagerRecord) {
    let manager_name = &record.manager_class_name;
    let record_type = &record.record_type_name;
    let factory = to_snake_ident(manager_name, "manager");
    let manager_resources = rust_manager_resources_expression(
        &record.manager_name,
        record
            .tables
            .iter()
            .map(|table| (table.table_name.as_str(), table.row_type_name.as_str())),
        std::iter::empty(),
    );
    let lookup_methods = rust_semantic_lookup_methods(record);
    let source_row_method = rust_semantic_source_row_method(record);
    let ids_method = rust_semantic_ids_method(record);
    let rows_method = rust_semantic_rows_method(record);
    let len_method = rust_semantic_len_method(record);
    let is_empty_method = rust_semantic_is_empty_method(record);
    let special_methods = rust_standalone_special_manager_extra_methods(manager_name);
    let has_key_index = !record.lookup_methods.is_empty();
    let has_source_row_index = record.source_row_method.is_some();
    assert!(
        !has_key_index || record.key.is_some(),
        "{manager_name} exposes key lookups without a semantic key"
    );
    assert!(
        !has_source_row_index || record.source_row_field.is_some(),
        "{manager_name} exposes a source-row lookup without a source-row field"
    );

    let key_index_field = if has_key_index {
        format!(
            "    entries_by_key: Arc<HashMap<{}, usize>>,\n",
            rust_key_map_type(record)
        )
    } else {
        String::new()
    };
    let source_row_index_field = if has_source_row_index {
        "    entries_by_source_row: Arc<HashMap<u32, usize>>,\n"
    } else {
        ""
    };
    let mut index_build = String::new();
    if has_key_index {
        index_build.push_str("        let mut entries_by_key = HashMap::new();\n");
    }
    if has_source_row_index {
        index_build.push_str("        let mut entries_by_source_row = HashMap::new();\n");
    }
    if has_key_index || has_source_row_index {
        index_build.push_str("        for (index, row) in entries.iter().enumerate() {\n");
        if has_key_index {
            index_build.push_str(&rust_semantic_key_index_insert(record));
        }
        if has_source_row_index {
            index_build.push_str(&rust_semantic_source_row_index_insert(record));
        }
        index_build.push_str("        }\n");
    }
    let key_index_initializer = if has_key_index {
        "            entries_by_key: Arc::new(entries_by_key),\n"
    } else {
        ""
    };
    let source_row_index_initializer = if has_source_row_index {
        "            entries_by_source_row: Arc::new(entries_by_source_row),\n"
    } else {
        ""
    };

    source.push_str(&format!(
        r#"
#[derive(Debug, Clone)]
pub struct {manager_name} {{
    entries: Arc<Vec<{record_type}>>,
{key_index_field}{source_row_index_field}
}}

impl {manager_name} {{
    pub(in crate::managers) fn build(cache: &mut ManagerCache) -> Result<Self> {{
        let resources = {manager_resources}?;
        let entries = materialize_{factory}(&resources)?;
{index_build}
        Ok(Self {{
            entries: Arc::new(entries),
{key_index_initializer}{source_row_index_initializer}
        }})
    }}

{lookup_methods}{source_row_method}{ids_method}{rows_method}{len_method}{is_empty_method}{special_methods}
}}

impl Rows for {manager_name} {{
    type Row = {record_type};

    fn rows(&self) -> impl Iterator<Item = &Self::Row> {{
        {manager_name}::rows(self)
    }}
}}

"#
    ));
    push_rust_semantic_materializer(source, record);
}

fn rust_semantic_field_name(source_name: &str) -> String {
    serialize_rust_field_ident(source_name)
}

fn rust_numeric_key_type(key_type: SemanticNumericKeyType) -> &'static str {
    match key_type {
        SemanticNumericKeyType::U8 => "u8",
        SemanticNumericKeyType::U16 => "u16",
        SemanticNumericKeyType::U32 => "u32",
    }
}

fn rust_projection_type(field: &SemanticRecordField) -> String {
    match field.transform {
        SemanticProjectionTransform::String
        | SemanticProjectionTransform::NonEmptyString
        | SemanticProjectionTransform::StringDefaultEmpty
        | SemanticProjectionTransform::PlusJoinedList
        | SemanticProjectionTransform::ForeignKey => "String".to_owned(),
        SemanticProjectionTransform::EnumString
        | SemanticProjectionTransform::EnumStringSkipInvalid
        | SemanticProjectionTransform::EnumStringRejectDefault
        | SemanticProjectionTransform::EnumDefault => semantic_enum_type_name(field).to_owned(),
        SemanticProjectionTransform::OptionalString
        | SemanticProjectionTransform::OptionalFirstString
        | SemanticProjectionTransform::OptionalForeignKey => "Option<String>".to_owned(),
        SemanticProjectionTransform::StringList
        | SemanticProjectionTransform::NonEmptyStringList
        | SemanticProjectionTransform::ForeignKeyList => "Vec<String>".to_owned(),
        SemanticProjectionTransform::OptionalStringList => "Option<Vec<String>>".to_owned(),
        SemanticProjectionTransform::Bool
        | SemanticProjectionTransform::BoolDefaultFalse
        | SemanticProjectionTransform::Crc32NonZeroBool => "bool".to_owned(),
        SemanticProjectionTransform::OptionalBool => "Option<bool>".to_owned(),
        SemanticProjectionTransform::U8
        | SemanticProjectionTransform::NonZeroU8
        | SemanticProjectionTransform::U8DefaultZero
        | SemanticProjectionTransform::U8DefaultMax => "u8".to_owned(),
        SemanticProjectionTransform::U16
        | SemanticProjectionTransform::NonZeroU16
        | SemanticProjectionTransform::U16BelowMax => "u16".to_owned(),
        SemanticProjectionTransform::U32
        | SemanticProjectionTransform::U32DefaultZero
        | SemanticProjectionTransform::NonZeroU32 => "u32".to_owned(),
        SemanticProjectionTransform::OptionalU32
        | SemanticProjectionTransform::OptionalNonZeroU32 => "Option<u32>".to_owned(),
        SemanticProjectionTransform::Crc32
        | SemanticProjectionTransform::LowercaseCrcString
        | SemanticProjectionTransform::LowercaseCrcStringDefaultZero
        | SemanticProjectionTransform::FirstLowercaseCrcStringDefaultZero
        | SemanticProjectionTransform::TrimmedLowercaseCrcStringDefaultZero => "Crc32".to_owned(),
        SemanticProjectionTransform::OptionalCrc32
        | SemanticProjectionTransform::OptionalCrc32ZeroAsNone
        | SemanticProjectionTransform::OptionalLowercaseCrcString
        | SemanticProjectionTransform::OptionalQualifiedLowercaseCrcString
        | SemanticProjectionTransform::OptionalTrimmedLowercaseCrcString => {
            "Option<Crc32>".to_owned()
        }
        SemanticProjectionTransform::OptionalQualifiedU16 => "Option<u16>".to_owned(),
        SemanticProjectionTransform::I32 => "i32".to_owned(),
        SemanticProjectionTransform::F32
        | SemanticProjectionTransform::F32MinutesToSeconds
        | SemanticProjectionTransform::F32UpperBound10000ZeroIsDefault
        | SemanticProjectionTransform::F32LowerBound10000CappedToField => "f32".to_owned(),
        SemanticProjectionTransform::OptionalF32 => "Option<f32>".to_owned(),
        SemanticProjectionTransform::F32List => "Vec<f32>".to_owned(),
        SemanticProjectionTransform::I32List => "Vec<i32>".to_owned(),
        SemanticProjectionTransform::Crc32List
        | SemanticProjectionTransform::LowercaseCrcStringList => "Vec<Crc32>".to_owned(),
        SemanticProjectionTransform::F32RangeInclusive => "(f32, f32)".to_owned(),
        SemanticProjectionTransform::U32RangeInclusive => "(u32, u32)".to_owned(),
        SemanticProjectionTransform::OptionalCrc32F32PairList => {
            "Option<Vec<(Crc32, f32)>>".to_owned()
        }
        SemanticProjectionTransform::OptionalU8F32PairList => "Option<Vec<(u8, f32)>>".to_owned(),
    }
}

fn rust_key_map_type(record: &SemanticManagerRecord) -> &'static str {
    match record.key {
        Some(SemanticManagerKey::String { .. } | SemanticManagerKey::EnumString { .. }) => "String",
        Some(SemanticManagerKey::Crc { .. } | SemanticManagerKey::FallbackCrc { .. }) => "Crc32",
        Some(SemanticManagerKey::Numeric { .. }) | None => "u32",
    }
}

fn rust_semantic_lookup_methods(record: &SemanticManagerRecord) -> String {
    let mut source = String::new();
    let record_type = &record.record_type_name;
    for method in &record.lookup_methods {
        let method_name = to_snake_ident(&method.name, "method");
        let parameter_name = to_snake_ident(&method.parameter, "key");
        match method.kind {
            SemanticLookupKind::CrcString => source.push_str(&format!(
                r#"    pub fn {method_name}(&self, {parameter_name}: impl AsRef<str>) -> Option<&{record_type}> {{
        let key = Crc32::from_str_lower({parameter_name}.as_ref());
        self.entries_by_key.get(&key).map(|index| &self.entries[*index])
    }}

"#
            )),
            SemanticLookupKind::Crc => source.push_str(&format!(
                r#"    pub fn {method_name}(&self, {parameter_name}: Crc32) -> Option<&{record_type}> {{
        self.entries_by_key.get(&{parameter_name}).map(|index| &self.entries[*index])
    }}

"#
            )),
            SemanticLookupKind::IntoCrc => source.push_str(&format!(
                r#"    pub fn {method_name}(&self, {parameter_name}: impl IntoCrc32Key) -> Option<&{record_type}> {{
        let key = {parameter_name}.into_crc32_key();
        self.entries_by_key.get(&key).map(|index| &self.entries[*index])
    }}

"#
            )),
            SemanticLookupKind::Numeric(key_type) => {
                let parameter_type = rust_numeric_key_type(key_type);
                let key_value = rust_numeric_key_as_u32(&parameter_name, key_type);
                source.push_str(&format!(
                    r#"    pub fn {method_name}(&self, {parameter_name}: {parameter_type}) -> Option<&{record_type}> {{
        let key = {key_value};
        self.entries_by_key.get(&key).map(|index| &self.entries[*index])
    }}

"#
                ));
            }
            SemanticLookupKind::String => source.push_str(&format!(
                r#"    pub fn {method_name}(&self, {parameter_name}: impl AsRef<str>) -> Option<&{record_type}> {{
        let key = normalize_lookup_key({parameter_name}.as_ref());
        self.entries_by_key.get(&key).map(|index| &self.entries[*index])
    }}

"#
            )),
        }
    }
    source
}

fn rust_semantic_source_row_method(record: &SemanticManagerRecord) -> String {
    let Some(method) = &record.source_row_method else {
        return String::new();
    };
    let method_name = to_snake_ident(method, "source_row");
    let record_type = &record.record_type_name;
    format!(
        r#"    pub fn {method_name}(&self, row: u32) -> Option<&{record_type}> {{
        self.entries_by_source_row
            .get(&row)
            .map(|index| &self.entries[*index])
    }}

"#
    )
}

fn rust_semantic_ids_method(record: &SemanticManagerRecord) -> String {
    let Some(method) = &record.ids_method else {
        return String::new();
    };
    let method_name = to_snake_ident(method, "ids");
    let id_type = rust_ids_type(record);
    let id_expr = rust_ids_expression(record);
    format!(
        r#"    pub fn {method_name}(&self) -> impl Iterator<Item = {id_type}> + '_ {{
        self.entries.iter().map(|row| {id_expr})
    }}

"#
    )
}

fn rust_semantic_rows_method(record: &SemanticManagerRecord) -> String {
    let method_name = record
        .rows_method
        .as_deref()
        .map(|method| to_snake_ident(method, "rows"))
        .unwrap_or_else(|| "rows".to_owned());
    let record_type = &record.record_type_name;
    let alias_method = if method_name == "rows" {
        String::new()
    } else {
        format!(
            r#"    pub fn {method_name}(&self) -> std::slice::Iter<'_, {record_type}> {{
        self.entries.iter()
    }}

"#
        )
    };
    format!(
        r#"    pub fn rows(&self) -> std::slice::Iter<'_, {record_type}> {{
        self.entries.iter()
    }}

{alias_method}"#
    )
}

fn rust_semantic_len_method(record: &SemanticManagerRecord) -> String {
    let Some(method) = &record.len_method else {
        return String::new();
    };
    let method_name = to_snake_ident(method, "len");
    format!(
        r#"    pub fn {method_name}(&self) -> usize {{
        self.entries.len()
    }}

"#
    )
}

fn rust_semantic_is_empty_method(record: &SemanticManagerRecord) -> String {
    let Some(method) = &record.is_empty_method else {
        return String::new();
    };
    let method_name = to_snake_ident(method, "is_empty");
    format!(
        r#"    pub fn {method_name}(&self) -> bool {{
        self.entries.is_empty()
    }}

"#
    )
}

fn rust_ids_type(record: &SemanticManagerRecord) -> &'static str {
    match record.key {
        Some(SemanticManagerKey::String { .. } | SemanticManagerKey::EnumString { .. }) => "String",
        Some(SemanticManagerKey::Numeric { key_type, .. }) => rust_numeric_key_type(key_type),
        Some(SemanticManagerKey::Crc { .. } | SemanticManagerKey::FallbackCrc { .. }) => "Crc32",
        None => "u32",
    }
}

fn rust_ids_expression(record: &SemanticManagerRecord) -> String {
    match record.key.as_ref() {
        Some(SemanticManagerKey::Crc { crc_field, .. })
        | Some(SemanticManagerKey::FallbackCrc { crc_field, .. }) => {
            format!("row.{}", rust_semantic_field_name(crc_field))
        }
        Some(SemanticManagerKey::Numeric { key_field, .. }) => {
            format!("row.{}", rust_semantic_field_name(key_field))
        }
        Some(SemanticManagerKey::EnumString { key_field, .. })
        | Some(SemanticManagerKey::String { key_field, .. }) => {
            format!("row.{}.clone()", rust_semantic_field_name(key_field))
        }
        None => "0".to_owned(),
    }
}

fn rust_semantic_key_index_insert(record: &SemanticManagerRecord) -> String {
    let Some(key) = &record.key else {
        return String::new();
    };
    let expression = match key {
        SemanticManagerKey::Crc { crc_field, .. }
        | SemanticManagerKey::FallbackCrc { crc_field, .. } => {
            format!("row.{}", rust_semantic_field_name(crc_field))
        }
        SemanticManagerKey::Numeric {
            key_field,
            key_type,
            ..
        } => {
            let field = format!("row.{}", rust_semantic_field_name(key_field));
            rust_numeric_key_as_u32(&field, *key_type)
        }
        SemanticManagerKey::EnumString { key_field, .. }
        | SemanticManagerKey::String { key_field, .. } => {
            format!(
                "normalize_lookup_key(&row.{})",
                rust_semantic_field_name(key_field)
            )
        }
    };
    format!("            entries_by_key.insert({expression}, index);\n")
}

fn rust_semantic_source_row_index_insert(record: &SemanticManagerRecord) -> String {
    let Some(field) = &record.source_row_field else {
        return String::new();
    };
    format!(
        "            entries_by_source_row.insert(row.{}, index);\n",
        rust_semantic_field_name(field)
    )
}

fn push_rust_semantic_materializer(source: &mut String, record: &SemanticManagerRecord) {
    let manager_factory = to_snake_ident(&record.manager_class_name, "manager");
    let record_type = &record.record_type_name;
    source.push_str(&format!(
        r#"fn materialize_{manager_factory}(resources: &ManagerResources) -> Result<Vec<{record_type}>> {{
    let mut rows = Vec::new();
"#
    ));
    if record.key.is_some() {
        source.push_str("    let mut seen = HashSet::new();\n");
    }
    source.push_str(
        "    for table in &resources.table_order {\n        for source_row in &table.rows {\n",
    );
    source.push_str(&rust_semantic_key_materializer(record));
    source.push_str(&rust_semantic_row_filters(record));
    for field in &record.fields {
        let local = rust_projection_local_name(&field.name);
        if matches!(
            field.transform,
            SemanticProjectionTransform::EnumStringSkipInvalid
                | SemanticProjectionTransform::EnumStringRejectDefault
        ) {
            let parser = rust_enum_parser_name(semantic_enum_type_name(field));
            let column = rust_string_literal(&field.column);
            source.push_str(&format!(
                "            let Ok({local}) = {parser}(required_string_cell(table, source_row, {column})?) else {{\n                continue;\n            }};\n"
            ));
        } else {
            source.push_str(&format!(
                "            let {local} = {};\n",
                rust_projection_value(field)
            ));
        }
    }
    for field in &record.fields {
        let local = rust_projection_local_name(&field.name);
        match field.transform {
            SemanticProjectionTransform::NonEmptyString
            | SemanticProjectionTransform::NonEmptyStringList => source.push_str(&format!(
                "            if {local}.is_empty() {{\n                continue;\n            }}\n"
            )),
            SemanticProjectionTransform::EnumStringRejectDefault => {
                let enum_type = semantic_enum_type_name(field);
                let default = to_upper_camel_ident(semantic_enum_default_variant(field), "Variant");
                source.push_str(&format!(
                    "            if {local} == {enum_type}::{default} {{\n                continue;\n            }}\n"
                ));
            }
            _ => {}
        }
    }
    source.push_str(&rust_semantic_duplicate_key_policy(record));
    source.push_str(&format!("            let row = {record_type} {{\n"));
    if let Some(field) = &record.source_row_field {
        source.push_str(&format!(
            "                {}: u32::try_from(source_row.row_index + 1).context(\"source row index exceeds u32\")?,\n",
            rust_semantic_field_name(field)
        ));
    }
    source.push_str(&rust_semantic_key_row_fields(record));
    for field in &record.fields {
        source.push_str(&format!(
            "                {}: {},\n",
            rust_semantic_field_name(&field.name),
            rust_projection_local_name(&field.name)
        ));
    }
    source.push_str("            };\n            rows.push(row);\n");
    if record.key.is_some() {
        source.push_str("            seen.insert(seen_key);\n");
    }
    source.push_str(
        r#"        }
    }
    Ok(rows)
}

"#,
    );
}

fn rust_projection_local_name(field_name: &str) -> String {
    format!("projected_{}", rust_semantic_field_name(field_name))
}

fn rust_semantic_key_materializer(record: &SemanticManagerRecord) -> String {
    let Some(key) = &record.key else {
        return String::new();
    };
    match key {
        SemanticManagerKey::Crc {
            key_column,
            skip_empty_key,
            trim_key,
            reject_zero_crc,
            ..
        } => {
            let column = rust_string_literal(key_column);
            let mut source = if *skip_empty_key {
                format!(
                    "            let Some(key_text) = optional_string_cell(table, source_row, {column})? else {{\n                continue;\n            }};\n"
                )
            } else {
                format!(
                    "            let key_text = required_string_cell(table, source_row, {column})?;\n"
                )
            };
            if *trim_key {
                source.push_str("            let key_value = key_text.trim().to_owned();\n");
            } else {
                source.push_str("            let key_value = key_text.to_owned();\n");
            }
            if *skip_empty_key {
                source.push_str(
                    r#"            if key_value.is_empty() {
                continue;
            }
"#,
                );
            }
            source.push_str("            let key_crc = Crc32::from_str_lower(&key_value);\n");
            if *reject_zero_crc {
                source.push_str(
                    r#"            if key_crc == Crc32::ZERO {
                continue;
            }
"#,
                );
            }
            source.push_str("            let seen_key = key_crc;\n");
            source
        }
        SemanticManagerKey::FallbackCrc {
            primary_key_kind,
            fallback_key_kind,
            primary_key_column,
            fallback_key_column,
            skip_empty_key,
            ..
        } => {
            let primary_column = rust_string_literal(primary_key_column);
            let fallback_column = rust_string_literal(fallback_key_column);
            let primary_kind = rust_string_literal(primary_key_kind);
            let fallback_kind = rust_string_literal(fallback_key_kind);
            let mut source = format!(
                r#"            let primary_key_value = optional_string_cell(table, source_row, {primary_column})?;
            let fallback_key_value = optional_string_cell(table, source_row, {fallback_column})?;
            let (key_kind, key_value) = if let Some(primary) = primary_key_value.filter(|value| !value.is_empty()) {{
                ({primary_kind}.to_owned(), primary.to_owned())
            }} else {{
                ({fallback_kind}.to_owned(), fallback_key_value.unwrap_or("").to_owned())
            }};
"#
            );
            if *skip_empty_key {
                source.push_str(
                    r#"            if key_value.is_empty() {
                continue;
            }
"#,
                );
            }
            source.push_str(
                r#"            let key_crc = Crc32::from_str_lower(&key_value);
            let seen_key = key_crc;
"#,
            );
            source
        }
        SemanticManagerKey::Numeric {
            key_column,
            key_type,
            ..
        } => {
            let key_value = rust_numeric_key_value("table", "source_row", key_column, *key_type);
            format!(
                r#"            let key_value = {key_value};
            let seen_key = key_value as u32;
"#
            )
        }
        SemanticManagerKey::EnumString {
            key_column,
            skip_empty_key,
            trim_key,
            ..
        } => {
            let column = rust_string_literal(key_column);
            let mut source = if *skip_empty_key {
                format!(
                    "            let Some(key_text) = optional_string_cell(table, source_row, {column})? else {{\n                continue;\n            }};\n"
                )
            } else {
                format!(
                    "            let key_text = required_string_cell(table, source_row, {column})?;\n"
                )
            };
            if *trim_key {
                source.push_str("            let key_value = key_text.trim().to_owned();\n");
            } else {
                source.push_str("            let key_value = key_text.to_owned();\n");
            }
            if *skip_empty_key {
                source.push_str(
                    r#"            if key_value.is_empty() {
                continue;
            }
"#,
                );
            }
            source.push_str("            let seen_key = normalize_lookup_key(&key_value);\n");
            source
        }
        SemanticManagerKey::String {
            key_column,
            skip_empty_key,
            ..
        } => {
            let column = rust_string_literal(key_column);
            let mut source = if *skip_empty_key {
                format!(
                    "            let Some(key_value) = optional_string_cell(table, source_row, {column})? else {{\n                continue;\n            }};\n            let key_value = key_value.to_owned();\n"
                )
            } else {
                format!(
                    "            let key_value = required_string_cell(table, source_row, {column})?.to_owned();\n"
                )
            };
            if *skip_empty_key {
                source.push_str(
                    r#"            if key_value.is_empty() {
                continue;
            }
"#,
                );
            }
            source.push_str("            let seen_key = normalize_lookup_key(&key_value);\n");
            source
        }
    }
}

fn rust_semantic_row_filters(record: &SemanticManagerRecord) -> String {
    let mut source = String::new();
    for filter in &record.row_filters {
        let column = rust_string_literal(&filter.column);
        match filter.predicate {
            SemanticRowFilterPredicate::BoolTrueWhenPresent => source.push_str(&format!(
                r#"            if optional_bool_cell(table, source_row, {column})?.unwrap_or(false) {{
                continue;
            }}
"#
            )),
            SemanticRowFilterPredicate::BoolMustBeTrue => source.push_str(&format!(
                r#"            if optional_bool_cell(table, source_row, {column})? != Some(true) {{
                continue;
            }}
"#
            )),
            SemanticRowFilterPredicate::F32GreaterThanOrEqualZero => source.push_str(&format!(
                r#"            if required_number_cell(table, source_row, {column})? < 0.0 {{
                continue;
            }}
"#
            )),
            SemanticRowFilterPredicate::F32LessThanOrEqualZero => source.push_str(&format!(
                r#"            if required_number_cell(table, source_row, {column})? > 0.0 {{
                continue;
            }}
"#
            )),
            SemanticRowFilterPredicate::F32AnyGreaterThanZero => {
                let checks = std::iter::once(filter.column.as_str())
                    .chain(filter.extra_columns.iter().map(String::as_str))
                    .map(|column| {
                        format!(
                            "required_number_cell(table, source_row, {})? > 0.0",
                            rust_string_literal(column)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" || ");
                source.push_str(&format!(
                    r#"            if !({checks}) {{
                continue;
            }}
"#
                ));
            }
            SemanticRowFilterPredicate::I32LessThanOrEqualZero => source.push_str(&format!(
                r#"            if required_i32_cell(table, source_row, {column})? > 0 {{
                continue;
            }}
"#
            )),
            SemanticRowFilterPredicate::LowercaseCrcStringNonZero => source.push_str(&format!(
                r#"            if Crc32::from_str_lower(required_string_cell(table, source_row, {column})?) == Crc32::ZERO {{
                continue;
            }}
"#
            )),
            SemanticRowFilterPredicate::StringNotEqualToColumn => {
                let compare_column = rust_string_literal(
                    filter
                        .compare_column
                        .as_deref()
                        .expect("string comparison row filters have compare columns"),
                );
                source.push_str(&format!(
                    r#"            if required_string_cell(table, source_row, {column})? == required_string_cell(table, source_row, {compare_column})? {{
                continue;
            }}
"#
                ));
            }
        }
    }
    source
}

fn rust_semantic_duplicate_key_policy(record: &SemanticManagerRecord) -> String {
    let Some(policy) = record.key.as_ref().map(rust_semantic_key_duplicate_policy) else {
        return String::new();
    };
    match policy {
        NativeDuplicateKeyPolicy::FirstWins => r#"            if seen.contains(&seen_key) {
                continue;
            }
"#
        .to_owned(),
        NativeDuplicateKeyPolicy::Error => format!(
            r#"            if seen.contains(&seen_key) {{
                bail!("manager {} duplicate key {{seen_key:?}}");
            }}
"#,
            record.manager_name
        ),
        NativeDuplicateKeyPolicy::Overwrite => String::new(),
    }
}

fn rust_semantic_key_duplicate_policy(key: &SemanticManagerKey) -> NativeDuplicateKeyPolicy {
    match key {
        SemanticManagerKey::Crc {
            duplicate_key_policy,
            ..
        }
        | SemanticManagerKey::FallbackCrc {
            duplicate_key_policy,
            ..
        }
        | SemanticManagerKey::Numeric {
            duplicate_key_policy,
            ..
        }
        | SemanticManagerKey::EnumString {
            duplicate_key_policy,
            ..
        }
        | SemanticManagerKey::String {
            duplicate_key_policy,
            ..
        } => *duplicate_key_policy,
    }
}

fn rust_semantic_key_row_fields(record: &SemanticManagerRecord) -> String {
    let Some(key) = &record.key else {
        return String::new();
    };
    match key {
        SemanticManagerKey::Crc {
            key_field,
            crc_field,
            ..
        } => [
            rust_field_initializer(key_field, "key_value"),
            rust_field_initializer(crc_field, "key_crc"),
        ]
        .concat(),
        SemanticManagerKey::FallbackCrc {
            key_kind_field,
            key_field,
            crc_field,
            ..
        } => [
            rust_field_initializer(key_kind_field, "key_kind"),
            rust_field_initializer(key_field, "key_value"),
            rust_field_initializer(crc_field, "key_crc"),
        ]
        .concat(),
        SemanticManagerKey::Numeric { key_field, .. }
        | SemanticManagerKey::EnumString { key_field, .. }
        | SemanticManagerKey::String { key_field, .. } => {
            rust_field_initializer(key_field, "key_value")
        }
    }
}

fn rust_field_initializer(field: &str, value: &str) -> String {
    let field = rust_semantic_field_name(field);
    if field == value {
        format!("                {field},\n")
    } else {
        format!("                {field}: {value},\n")
    }
}

fn rust_numeric_key_as_u32(value: &str, key_type: SemanticNumericKeyType) -> String {
    match key_type {
        SemanticNumericKeyType::U8 | SemanticNumericKeyType::U16 => {
            format!("{value} as u32")
        }
        SemanticNumericKeyType::U32 => value.to_owned(),
    }
}

fn rust_numeric_key_value(
    table: &str,
    row: &str,
    column: &str,
    key_type: SemanticNumericKeyType,
) -> String {
    let column = rust_string_literal(column);
    match key_type {
        SemanticNumericKeyType::U8 => format!("required_u8_cell({table}, {row}, {column})?"),
        SemanticNumericKeyType::U16 => format!("required_u16_cell({table}, {row}, {column})?"),
        SemanticNumericKeyType::U32 => format!("required_u32_cell({table}, {row}, {column})?"),
    }
}

fn rust_projection_value(field: &SemanticRecordField) -> String {
    let column = rust_string_literal(&field.column);
    match field.transform {
        SemanticProjectionTransform::String => {
            format!("required_schema_string_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::NonEmptyString => {
            format!("required_string_cell(table, source_row, {column})?.to_owned()")
        }
        SemanticProjectionTransform::EnumString
        | SemanticProjectionTransform::EnumStringSkipInvalid
        | SemanticProjectionTransform::EnumStringRejectDefault => {
            let parser = rust_enum_parser_name(semantic_enum_type_name(field));
            format!("{parser}(required_string_cell(table, source_row, {column})?)?")
        }
        SemanticProjectionTransform::EnumDefault => {
            let enum_type = semantic_enum_type_name(field);
            let default = to_upper_camel_ident(semantic_enum_default_variant(field), "Variant");
            let parser = rust_enum_parser_name(enum_type);
            format!(
                "match optional_string_cell(table, source_row, {column})? {{ Some(value) => {parser}(value)?, None => {enum_type}::{default} }}"
            )
        }
        SemanticProjectionTransform::StringDefaultEmpty => {
            format!("optional_string_cell(table, source_row, {column})?.unwrap_or(\"\").to_owned()")
        }
        SemanticProjectionTransform::PlusJoinedList => {
            format!("string_list_cell(table, source_row, {column})?.join(\"+\")")
        }
        SemanticProjectionTransform::OptionalString => {
            format!("optional_schema_string_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalFirstString => {
            format!(
                "optional_string_list_cell(table, source_row, {column})?.and_then(|values| values.into_iter().next())"
            )
        }
        SemanticProjectionTransform::StringList => {
            format!("string_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::NonEmptyStringList => {
            format!("non_empty_string_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalStringList => {
            format!("optional_string_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::Bool => {
            format!("required_bool_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalBool => {
            format!("optional_bool_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::BoolDefaultFalse => {
            format!("optional_bool_cell(table, source_row, {column})?.unwrap_or(false)")
        }
        SemanticProjectionTransform::Crc32NonZeroBool => {
            let reference = field
                .reference_field
                .as_deref()
                .expect("CRC presence projections have reference fields");
            format!("{} != Crc32::ZERO", rust_projection_local_name(reference))
        }
        SemanticProjectionTransform::U8 => {
            format!("required_u8_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::NonZeroU8 => {
            format!("required_non_zero_u8_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::U8DefaultZero => {
            format!("optional_u8_cell(table, source_row, {column})?.unwrap_or(0)")
        }
        SemanticProjectionTransform::U8DefaultMax => {
            format!("optional_u8_cell(table, source_row, {column})?.unwrap_or(u8::MAX)")
        }
        SemanticProjectionTransform::U16 => {
            format!("required_u16_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::NonZeroU16 => {
            format!("required_non_zero_u16_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::U16BelowMax => {
            let max = field
                .u16_max_exclusive
                .expect("capped u16 projections have a maximum");
            format!(
                "{{ let value = required_u16_cell(table, source_row, {column})?; if u32::from(value) >= {max} {{ bail!(\"row {{}}:{{}} {{}} exceeds supported cap {max}\", source_row.source_path, source_row.row_index + 1, {column}); }} value }}"
            )
        }
        SemanticProjectionTransform::U32 => {
            format!("required_u32_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalU32 => {
            format!("optional_u32_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::U32DefaultZero => {
            format!("optional_u32_cell(table, source_row, {column})?.unwrap_or(0)")
        }
        SemanticProjectionTransform::NonZeroU32 => {
            format!("required_non_zero_u32_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalNonZeroU32 => {
            format!(
                "optional_u32_cell(table, source_row, {column})?.map(|value| require_non_zero_u32(value, source_row, {column})).transpose()?"
            )
        }
        SemanticProjectionTransform::I32 => {
            format!("required_i32_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::F32 => {
            format!("required_number_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalF32 => {
            format!("optional_number_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::F32MinutesToSeconds => {
            format!("required_number_cell(table, source_row, {column})? * 60.0")
        }
        SemanticProjectionTransform::F32UpperBound10000ZeroIsDefault => {
            format!("upper_bound_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::F32LowerBound10000CappedToField => {
            let reference = field
                .reference_field
                .as_deref()
                .expect("lower-bound projections have reference fields");
            format!(
                "lower_bound_cell(table, source_row, {column}, {})?",
                rust_projection_local_name(reference)
            )
        }
        SemanticProjectionTransform::F32List => {
            format!("f32_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::I32List => {
            format!("i32_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::Crc32 => {
            format!("Crc32::new(required_crc32_cell(table, source_row, {column})?)")
        }
        SemanticProjectionTransform::LowercaseCrcString => {
            format!("Crc32::from_str_lower(required_string_cell(table, source_row, {column})?)")
        }
        SemanticProjectionTransform::LowercaseCrcStringDefaultZero => {
            format!(
                "Crc32::from_str_lower(optional_string_cell(table, source_row, {column})?.unwrap_or(\"\"))"
            )
        }
        SemanticProjectionTransform::FirstLowercaseCrcStringDefaultZero => {
            format!(
                "optional_string_list_cell(table, source_row, {column})?.and_then(|values| values.into_iter().next()).map_or(Crc32::ZERO, |value| Crc32::from_str_lower(&value))"
            )
        }
        SemanticProjectionTransform::TrimmedLowercaseCrcStringDefaultZero => {
            format!(
                "Crc32::from_str_lower(optional_string_cell(table, source_row, {column})?.unwrap_or(\"\").trim_ascii())"
            )
        }
        SemanticProjectionTransform::OptionalCrc32 => {
            format!("optional_crc32_cell(table, source_row, {column}, false)?.map(Crc32::new)")
        }
        SemanticProjectionTransform::OptionalCrc32ZeroAsNone => {
            format!("optional_crc32_cell(table, source_row, {column}, true)?.map(Crc32::new)")
        }
        SemanticProjectionTransform::Crc32List => {
            format!(
                "crc32_list_cell(table, source_row, {column})?.into_iter().map(Crc32::new).collect()"
            )
        }
        SemanticProjectionTransform::OptionalLowercaseCrcString => {
            format!(
                "optional_lowercase_crc_string_cell(table, source_row, {column})?.map(Crc32::new)"
            )
        }
        SemanticProjectionTransform::OptionalQualifiedLowercaseCrcString => {
            format!(
                "optional_qualified_crc_u16_cell(table, source_row, {column})?.map(|(id, _)| Crc32::new(id))"
            )
        }
        SemanticProjectionTransform::OptionalQualifiedU16 => {
            format!(
                "optional_qualified_crc_u16_cell(table, source_row, {column})?.map(|(_, rank)| rank)"
            )
        }
        SemanticProjectionTransform::OptionalTrimmedLowercaseCrcString => {
            format!(
                "optional_trimmed_lowercase_crc_string_cell(table, source_row, {column})?.map(Crc32::new)"
            )
        }
        SemanticProjectionTransform::LowercaseCrcStringList => {
            format!(
                "lowercase_crc_string_list_cell(table, source_row, {column})?.into_iter().map(Crc32::new).collect()"
            )
        }
        SemanticProjectionTransform::ForeignKey => {
            format!("required_string_cell(table, source_row, {column})?.to_owned()")
        }
        SemanticProjectionTransform::OptionalForeignKey => {
            format!("optional_string_cell(table, source_row, {column})?.map(str::to_owned)")
        }
        SemanticProjectionTransform::ForeignKeyList => {
            format!("string_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::F32RangeInclusive => {
            format!("f32_range_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::U32RangeInclusive => {
            format!("u32_range_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalCrc32F32PairList => {
            format!("optional_crc32_f32_pair_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalU8F32PairList => {
            let enum_shape = field
                .pair_first_enum_shape
                .as_ref()
                .expect("u8 pair-list projections have a reconciled enum schema");
            let parser = rust_pair_enum_parser_name(&enum_shape.name);
            format!("optional_u8_f32_pair_list_cell(table, source_row, {column}, {parser})?")
        }
    }
}

fn rust_standalone_special_manager_extra_methods(manager_name: &str) -> &'static str {
    match manager_name {
        "PlayerDataManager" => {
            r#"    pub fn categorical_progression_id(&self, tradeskill: impl ToString) -> Option<Crc32> {
        let normalized = tradeskill.to_string();
        if normalized == "None" || normalized == "WildernessSurvival" {
            return None;
        }
        Some(Crc32::from_str_lower(&normalized))
    }
"#
        }
        _ => "",
    }
}

fn rust_string_literal(value: &str) -> String {
    format!("{value:?}")
}

const RUST_STANDALONE_VALUE_TYPES: &str = r#"
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TradeskillRank(u16);

impl TradeskillRank {
    #[must_use]
    pub const fn new(value: u16) -> Self { Self(value) }

    #[must_use]
    pub const fn value(self) -> u16 { self.0 }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Srgba {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Srgba {
    #[must_use]
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self { red, green, blue, alpha }
    }
}
"#;

const RUST_STANDALONE_SERIALIZED_TYPES: &str = r#"
/// Serialize-context enum `EA27A445-C5F3-42AB-8BA0-8F617A19DC38`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ContributionType {
    DamageDealt = 0,
    DamageReceived = 1,
    Support = 2,
    Killed = 3,
    SpawnKilled = 4,
    SpawnInteracted = 5,
    WaveStarted = 6,
    EncounterActivated = 7,
    NumTypes = 8,
}

impl TryFrom<&str> for ContributionType {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Damage_Dealt" => Ok(Self::DamageDealt),
            "Damage_Received" => Ok(Self::DamageReceived),
            "Support" => Ok(Self::Support),
            "Killed" => Ok(Self::Killed),
            "Spawn_Killed" => Ok(Self::SpawnKilled),
            "Spawn_Interacted" => Ok(Self::SpawnInteracted),
            "Wave_Started" => Ok(Self::WaveStarted),
            "Encounter_Activated" => Ok(Self::EncounterActivated),
            "Num_Types" => Ok(Self::NumTypes),
            _ => Err(()),
        }
    }
}

/// Serialize-context enum `823EB577-91B2-463C-B7D7-D9BCE94E9309`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DarknessThreshold {
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
}

/// Serialize-context type `B9D7F0E8-6518-48C0-88E1-9D084C03259A`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DarknessLevel {
    pub threshold: DarknessThreshold,
    pub percentage: u32,
}
"#;

const RUST_STANDALONE_PRODUCT_MANAGER_RUNTIME: &str = r#"
pub const ALL_INTERACT_OPTIONS_CATEGORY: i32 = 0x15;

#[derive(Debug, Clone, PartialEq)]
pub struct ArmorOffsetDatabase {
    pub offsets: Vec<ArmorOffsetData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArmorOffsetData {
    pub name: String,
    pub attachments: Vec<AttachmentOffsetData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentOffsetData {
    pub attachment: String,
    pub position: Vec3,
    pub rotation_degrees: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EquipTypesDatabase {
    pub equip_types: Vec<EquipTypeData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EquipTypeData {
    pub name: String,
    pub attachment: String,
    pub attachment_offset_position: Vec3,
    pub attachment_offset_rotation_degrees: Vec3,
    pub sheath_data: String,
    pub sheath_offset_position: Vec3,
    pub sheath_offset_rotation_degrees: Vec3,
    pub off_hand_attachment: String,
    pub off_hand_attachment_offset_position: Vec3,
    pub off_hand_attachment_offset_rotation_degrees: Vec3,
    pub off_hand_sheath_data: String,
    pub off_hand_sheath_offset_position: Vec3,
    pub off_hand_sheath_offset_rotation_degrees: Vec3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDebugSettings {
    pub combat_settings: CombatDebugSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatDebugSettings {
    pub disable_player_loot_drop_on_death: bool,
    pub disable_weapon_durability: bool,
    pub disable_item_durability: bool,
    pub disable_durability_penalty_on_death: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiDatabase {
    pub unified_interact_data: UnifiedInteractData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnifiedInteractData {
    pub interact_options: Vec<InteractOptionData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DelayedInteractionData {
    pub delay_time: f32,
    pub delay_mannequin_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectData {
    pub effect_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractOptionData {
    pub name: String,
    pub display_name: String,
    pub interact_input_type: i32,
    pub ui_interact_action: u8,
    pub additional_info_type: i32,
    pub interact_option_category: i32,
    pub delayed_interaction_data: DelayedInteractionData,
    pub interact_privilege_ids: Vec<u32>,
    pub blueprint_privilege_id: u32,
    pub requires_confirmation: bool,
    pub is_committed_interaction: bool,
    pub is_instant_cancel: bool,
    pub close_prompt_on_interaction: bool,
    pub force_secondary_interact: bool,
    pub only_show_if_bound_to_camp: bool,
    pub display_priority: i32,
    pub interact_option_icon: String,
    pub ui_additional_info_slice_path: String,
    pub requires_security_level_validation: bool,
    pub mannequin_fragment: String,
    pub mannequin_tag: String,
    pub align_to_interaction: bool,
    pub hold_action_press_time: f32,
    pub cooldown_time: i32,
    pub set_ownership_on_interact: bool,
    pub required_item_name: String,
    pub required_item_count: i32,
    pub required_currency: i32,
    pub availability: i32,
    pub siege_warfare_game_event_name: String,
    pub added_status_effects: Vec<EffectData>,
    pub required_status_effects: Vec<EffectData>,
    pub remove_status_effects: Vec<EffectData>,
    pub excluded_status_effects: Vec<EffectData>,
    pub delay_before_adding_removing_effect: f32,
    pub remove_added_effects_on_interaction_end: bool,
    pub check_pvp_flag_is_set: bool,
    pub faction_required: bool,
    pub show_instanced_loot_item_count: bool,
    pub required_achievement_name: String,
    pub required_level: u32,
    pub committed_interaction_max_usage_timeout: f32,
    pub committed_interaction_max_usage_timeout_notification: String,
    pub committed_interaction_inactive_timeout: f32,
    pub committed_interaction_inactive_timeout_notification: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameCameraSettings {
    pub default_state_name: String,
    pub fields: HashMap<String, String>,
    pub camera_states: Vec<CameraStateSettings>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraStateSettings {
    pub name: String,
    pub include: Option<String>,
    pub fields: HashMap<String, String>,
    pub from_transitions: Vec<CameraStateTransition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraStateTransition {
    pub from_camera: String,
    pub smooth_time: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleAssetReferenceTextureAsset {
    pub asset_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditCrc {
    pub value_str: String,
    pub value_crc: Crc32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntRange {
    pub min: i32,
    pub max: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerBaseAttributes {
    pub player_attribute_data: PlayerAttributeData,
    pub guild_siege_window_region_data: HashMap<String, GuildSiegeWindowRegionData>,
    pub faction_influence_config_data: FactionInfluenceConfigData,
    pub valid_group_data: ValidGroupData,
    pub war_data: WarData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAttributeData {
    pub base_deployable_limit: i32,
    pub player_display_level_unlock_free_gear_sets: i32,
    pub item_rarity_data: Vec<ItemRarityData>,
    pub perk_generation_data: PerkGenerationData,
    pub perk_chance_item_id: String,
    pub ability_points_required_in_tree_to_unlock_final_row: i32,
    pub perk_chance_modifier: f32,
    pub attribute_chance_modifier: f32,
    pub gem_slot_chance_modifier: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRarityData {
    pub rarity_level_loc_string: String,
    pub max_perk_count: i32,
    pub level_requirement_modifier: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PerkGenerationData {
    pub perk_data_per_tier: Vec<PerkTierData>,
    pub crafting_result_loot_bucket_id: Crc32,
    pub crafting_result_loot_bucket: String,
    pub roll_perk_on_upgrade_gs: i32,
    pub roll_perk_on_upgrade_tier: i32,
    pub roll_perk_on_upgrade_perk_count: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PerkTierData {
    pub max_perk_channel: i32,
    pub gem_slot_probability: f32,
    pub attribute_perk_probability: f32,
    pub general_gear_score_perk_count: HashMap<i32, Vec<IntRange>>,
    pub crafting_gear_score_perk_count: HashMap<i32, Vec<IntRange>>,
    pub attribute_perk_bucket: String,
    pub attribute_perk_bucket_id: Crc32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildSiegeWindowRegionData {
    pub start_hour: u32,
    pub end_hour: u32,
    pub utc_offset: i32,
    pub dst_rule_id: Crc32,
    pub dst_rule: String,
    pub observes_dst: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FactionInfluenceConfigData {
    pub max_influence: f32,
    pub decrement_rate: f32,
    pub increment_rate: f32,
    pub max_increment_time_modifier: f32,
    pub max_decrement_time_modifier: f32,
    pub minimum_time_since_last_war: f32,
    pub min_territory_diff_to_apply_ud_mechanics: i32,
    pub min_time_to_apply_ud_mechanics: i32,
    pub under_dog_mission_influence_gain: f32,
    pub under_dog_mission_influence_gain_cap: f32,
    pub uder_dog_faction_rep_gain: f32,
    pub under_dog_faction_rep_gain_cap: f32,
    pub under_dog_pvp_influence_gain: f32,
    pub under_dog_pvp_influence_gain_cap: f32,
    pub minimum_influence_threshold_for_war: f32,
    pub influence_race_attacker_win_game_event_id: EditCrc,
    pub influence_race_defender_win_game_event_id: EditCrc,
    pub influence_race_lose_game_event_id: EditCrc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidGroupData {
    pub names: Vec<String>,
    pub objectives: Vec<String>,
    pub icon_paths: Vec<String>,
    pub colors: Vec<ColorRgba>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarData {
    pub deployable_limits: HashMap<Crc32, WarDeployableLimitData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarDeployableLimitData {
    pub id: Crc32,
    pub display_name: String,
    pub buildable_names: Vec<String>,
    pub buildable_ids: Vec<Crc32>,
    pub attacker_limits: [i32; 3],
    pub defender_limit: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementProgressionData {
    pub settlement_progression_categories: Vec<ProgressionCategoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressionCategoryEntry {
    pub settlement_progression_category: String,
    pub settlement_progression_entries: Vec<ProgressionSpawnerEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressionSpawnerEntry {
    pub settlement_progression_category_level: i32,
    pub slice: AssetReference,
    pub alternate_slice: AssetReference,
    pub display_loc_string: String,
    pub icon: SimpleAssetReferenceTextureAsset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheringDatabase {
    pub gathering_data: GatheringData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheringData {
    pub gathering_types: Vec<GatheringTypeData>,
    pub gathering_actions: Vec<GatheringAction>,
    pub required_water_gathering_type: String,
    pub none_gathering_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheringTypeData {
    pub gathering_type: String,
    pub ui_icon: SimpleAssetReferenceTextureAsset,
    pub requirement_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheringAction {
    pub name: String,
    pub mannequin_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheringActionDatabase {
    pub gathering_actions: Vec<GatheringActionData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheringActionData {
    pub name: String,
    pub mannequin_tag: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CraftingStationDatabase {
    pub crafting_stations: Vec<CraftingStationData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CraftingStationData {
    pub name: String,
    pub crafting_types: Vec<String>,
    pub mannequin_tag: String,
    pub azoth_discount_percent: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocialRankDatabase {
    pub ranks: Vec<SocialRankData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocialRankData {
    pub guild_rank_data: SocialGuildRankData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocialGuildRankData {
    pub name: String,
    pub security_level: u32,
    pub all_privileges: bool,
    pub privilege_ids: Vec<u32>,
}

const AZSTD_STRING_TYPE_ID: &str = "03aaab3f-5c47-5a66-9ebc-d5fa4db353c9";
const VECTOR3_TYPE_ID: &str = "8379eb7d-01fa-4538-b64b-a6543b4be73d";
const BOOL_TYPE_ID: &str = "a0ca880c-afe4-43cb-926c-59ac48496112";
const U8_TYPE_ID: &str = "72b9409a-7d1a-4831-9cfe-fcb3fadd3426";
const U32_TYPE_ID: &str = "43da906b-7def-4ca8-9790-854106d3f983";
const INT_TYPE_ID: &str = "72039442-eb38-4d42-a1ad-cb68f7e0eef6";
const FLOAT_TYPE_ID: &str = "ea2c3e90-afbe-44d4-a90d-faaf79baf93d";
const ASSET_TYPE_ID: &str = "77a19d40-8731-4d3c-9041-1b43047366a4";
const EDIT_CRC_TYPE_ID: &str = "9a339de9-0d6e-4708-922f-f46af04370e9";
const SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID: &str = "68e92460-5c0c-4031-9620-6f1a08763243";

const ARMOR_OFFSET_DATABASE_TYPE_ID: &str = "8c1fa8a8-2e58-4791-acda-2c3625318832";
const ARMOR_OFFSET_VECTOR_TYPE_ID: &str = "d276dfb3-a8ec-58c2-96e2-145bc5a6ee3d";
const ARMOR_OFFSET_DATA_TYPE_ID: &str = "13b87761-89ab-4a4b-a370-dad3875380da";
const ATTACHMENT_OFFSET_VECTOR_TYPE_ID: &str = "8b83aa0c-520e-5074-8c4e-5ad60c3d70fe";
const ATTACHMENT_OFFSET_DATA_TYPE_ID: &str = "fc296230-5f66-473e-90c8-66ad7028fd07";
const ARMOR_OFFSETS_FIELD_CRC: u32 = 2_282_200_990;
const ARMOR_OFFSET_NAME_FIELD_CRC: u32 = 1_579_384_326;
const ARMOR_OFFSET_ATTACHMENTS_FIELD_CRC: u32 = 1_204_091_606;
const ATTACHMENT_NAME_FIELD_CRC: u32 = 2_036_324_795;
const ATTACHMENT_OFFSET_POSITION_FIELD_CRC: u32 = 379_390_882;
const ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC: u32 = 581_018_980;

const EQUIP_TYPES_DATABASE_TYPE_ID: &str = "f937c753-ffc0-4f9c-a234-7c71c9a5bdb3";
const EQUIP_TYPE_DATA_VECTOR_TYPE_ID: &str = "53de1751-3981-5da4-8f72-f9e5712b3127";
const EQUIP_TYPE_DATA_TYPE_ID: &str = "0386d9b0-3e95-411f-823f-4a800c74f7ed";
const EQUIP_TYPES_FIELD_CRC: u32 = 1_388_966_666;
const EQUIP_NAME_FIELD_CRC: u32 = 1_579_384_326;
const EQUIP_ATTACHMENT_FIELD_CRC: u32 = 2_036_324_795;
const EQUIP_ATTACHMENT_OFFSET_POSITION_FIELD_CRC: u32 = 379_390_882;
const EQUIP_ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC: u32 = 581_018_980;
const EQUIP_SHEATH_DATA_FIELD_CRC: u32 = 1_966_399_264;
const EQUIP_SHEATH_OFFSET_POSITION_FIELD_CRC: u32 = 619_916_990;
const EQUIP_SHEATH_OFFSET_ROTATION_DEGREES_FIELD_CRC: u32 = 768_083_228;
const EQUIP_OFF_HAND_ATTACHMENT_FIELD_CRC: u32 = 2_388_996_306;
const EQUIP_OFF_HAND_ATTACHMENT_OFFSET_POSITION_FIELD_CRC: u32 = 2_522_934_056;
const EQUIP_OFF_HAND_ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC: u32 = 97_015_342;
const EQUIP_OFF_HAND_SHEATH_DATA_FIELD_CRC: u32 = 1_101_872_181;
const EQUIP_OFF_HAND_SHEATH_OFFSET_POSITION_FIELD_CRC: u32 = 1_077_303_719;
const EQUIP_OFF_HAND_SHEATH_OFFSET_ROTATION_DEGREES_FIELD_CRC: u32 = 789_454_983;

const GAME_DEBUG_SETTINGS_TYPE_ID: &str = "3e5db037-ae49-43e4-8bcc-67f8c511a091";
const COMBAT_DEBUG_SETTINGS_TYPE_ID: &str = "3c0e5dc7-06b9-4411-893e-daac101731d3";
const COMBAT_SETTINGS_FIELD_CRC: u32 = 3_204_566_528;
const DISABLE_PLAYER_LOOT_DROP_ON_DEATH_FIELD_CRC: u32 = 76_657_494;
const DISABLE_WEAPON_DURABILITY_FIELD_CRC: u32 = 2_559_298_940;
const DISABLE_ITEM_DURABILITY_FIELD_CRC: u32 = 880_532_799;
const DISABLE_DURABILITY_PENALTY_ON_DEATH_FIELD_CRC: u32 = 429_903_575;

const UI_DATABASE_TYPE_ID: &str = "7cc2b992-1c5b-4b27-bcb9-790175f09da6";
const UNIFIED_INTERACT_DATA_TYPE_ID: &str = "ebc0595e-4adb-4323-9527-82d07e30908c";
const INTERACT_OPTION_VECTOR_TYPE_ID: &str = "33d6e083-a124-527f-baac-824deb5cd6e8";
const INTERACT_OPTION_DATA_TYPE_ID: &str = "f0887e97-5084-413c-bce7-5c24cecb03c0";

const PLAYER_BASE_ATTRIBUTES_TYPE_ID: &str = "0f40ecc6-ace9-476a-9a5c-b83be6129a4b";
const PLAYER_ATTRIBUTE_DATA_TYPE_ID: &str = "46113bed-540d-4584-92aa-b9223d83875a";
const GUILD_SIEGE_WINDOW_REGION_DATA_TYPE_ID: &str = "da0aab24-92a0-5ea4-9a1a-5cef4e8c3bf9";
const FACTION_INFLUENCE_CONFIG_DATA_TYPE_ID: &str = "8ed959c4-b0e3-4d45-84d1-fcaec1c7d1a4";
const VALID_GROUP_DATA_TYPE_ID: &str = "4f986681-3060-4a47-9a45-694a027e5f46";
const WAR_DATA_TYPE_ID: &str = "4febcf31-140c-4ef1-8c53-814daa4426ac";

const SETTLEMENT_PROGRESSION_DATA_TYPE_ID: &str = "0543759c-4cf0-4eba-b0dd-f0f020b480b3";
const PROGRESSION_CATEGORY_ENTRY_TYPE_ID: &str = "e1766b2b-75fd-4eb2-ab13-0e5f343b7e68";
const PROGRESSION_SPAWNER_ENTRY_TYPE_ID: &str = "d91778a1-a110-46e4-8b9a-30402d8996d6";
const SETTLEMENT_PROGRESSION_CATEGORY_VECTOR_TYPE_ID: &str = "2d93cc0a-78e0-5fdf-af40-c2f0491facd0";
const PROGRESSION_SPAWNER_ENTRY_VECTOR_TYPE_ID: &str = "3999d332-be04-5382-9e40-a2bf965e61eb";
const SETTLEMENT_PROGRESSION_CATEGORIES_FIELD_CRC: u32 = 2_439_926_458;
const SETTLEMENT_PROGRESSION_CATEGORY_FIELD_CRC: u32 = 1_188_522_208;
const SETTLEMENT_PROGRESSION_ENTRIES_FIELD_CRC: u32 = 1_770_189_871;
const SETTLEMENT_PROGRESSION_CATEGORY_LEVEL_FIELD_CRC: u32 = 2_587_150_535;
const SLICE_FIELD_CRC: u32 = 1_034_844_325;
const ALTERNATE_SLICE_FIELD_CRC: u32 = 1_867_428_434;
const DISPLAY_LOC_STRING_FIELD_CRC: u32 = 457_484_292;
const ICON_FIELD_CRC: u32 = 1_704_208_859;

const GATHERING_DATABASE_TYPE_ID: &str = "1ef311cc-a16e-426d-9763-a40473495330";
const GATHERING_DATA_TYPE_ID: &str = "579abcc6-ec1e-4157-abc5-2569c7624b0a";
const GATHERING_ACTION_DATABASE_TYPE_ID: &str = "9ac82655-bc8f-4165-ae2f-6d6f3d543d9a";
const GATHERING_ACTION_DATA_TYPE_ID: &str = "a6b5258c-2984-4225-88e9-b66813457286";
const GATHERING_ACTION_TYPE_ID: &str = "5cfd353d-418d-4421-a207-2c748cfbdd16";
const GATHERING_TYPE_DATA_TYPE_ID: &str = "3266a19a-6bac-4703-b663-9f6ed48f1d76";
const GATHERING_TYPE_DATA_VECTOR_TYPE_ID: &str = "779755e7-d85d-5d47-91d5-5fdbb851da57";
const GATHERING_ACTION_VECTOR_TYPE_ID: &str = "0c5b29e6-74c4-5adf-8fcf-c3204a7e4662";
const GATHERING_ACTION_DATA_VECTOR_TYPE_ID: &str = "ceef81af-b476-5463-af1e-b7ec9f65c02f";
const GATHERING_DATA_FIELD_CRC: u32 = 2_208_564_949;
const GATHERING_TYPES_FIELD_CRC: u32 = 2_065_483_900;
const GATHERING_ACTIONS_FIELD_CRC: u32 = 1_482_662_604;
const REQUIRED_WATER_GATHERING_TYPE_FIELD_CRC: u32 = 674_599_067;
const NONE_GATHERING_TYPE_FIELD_CRC: u32 = 3_194_172_210;
const TYPE_FIELD_CRC: u32 = 2_363_381_545;
const UI_ICON_FIELD_CRC: u32 = 2_312_546_211;
const REQUIREMENT_TEXT_FIELD_CRC: u32 = 2_484_547_296;
const NAME_FIELD_CRC: u32 = 1_579_384_326;
const MANNEQUIN_TAG_FIELD_CRC: u32 = 2_777_524_544;

const CRAFTING_STATION_DATABASE_TYPE_ID: &str = "72175d3e-9370-4b21-970f-dc2adc11e52b";
const CRAFTING_STATION_DATA_VECTOR_TYPE_ID: &str = "866eb75e-8cfd-511b-a4f0-b8dfa17138bd";
const CRAFTING_STATION_DATA_TYPE_ID: &str = "75cfb9e3-fe11-4d1d-ac0a-44916a5c27a0";
const CRAFTING_TYPE_STRING_VECTOR_TYPE_ID: &str = "99dad0bc-740e-5e82-826b-8fc7968cc02c";
const CRAFTING_STATIONS_FIELD_CRC: u32 = 2_156_395_334;
const CRAFTING_TYPES_FIELD_CRC: u32 = 169_774_472;
const CRAFTING_MANNEQUIN_TAG_FIELD_CRC: u32 = 1_024_826_923;
const AZOTH_DISCOUNT_PERCENT_FIELD_CRC: u32 = 757_151_162;

const SOCIAL_RANK_DATABASE_TYPE_ID: &str = "b0024f1f-651d-48a5-a56a-9dea80cb487e";
const SOCIAL_RANK_DATA_VECTOR_TYPE_ID: &str = "1297b8af-3355-5871-914e-82178f34b16e";
const SOCIAL_RANK_DATA_TYPE_ID: &str = "2f2c2714-e932-43bf-a702-cacd8c9ae544";
const SOCIAL_GUILD_RANK_DATA_TYPE_ID: &str = "e756a995-93ed-f487-1a76-23b1ad74df11";
const SOCIAL_PRIVILEGE_ID_SET_TYPE_ID: &str = "4c9c7f67-4b86-58af-b45a-abf4d2eae45f";
const SOCIAL_RANKS_FIELD_CRC: u32 = 3_420_889_108;
const SOCIAL_GUILD_RANK_DATA_FIELD_CRC: u32 = 2_999_919_934;
const SOCIAL_GUILD_RANK_NAME_FIELD_CRC: u32 = 3_230_417_959;
const SOCIAL_GUILD_RANK_SECURITY_LEVEL_FIELD_CRC: u32 = 265_698_600;
const SOCIAL_GUILD_RANK_ALL_PRIVILEGES_FIELD_CRC: u32 = 928_054_442;
const SOCIAL_GUILD_RANK_PRIVILEGE_IDS_FIELD_CRC: u32 = 2_614_315_740;

fn parse_armor_offset_database(bytes: &[u8]) -> Result<ArmorOffsetDatabase> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, ARMOR_OFFSET_DATABASE_TYPE_ID)?;
    let offsets = required_typed_child(root, ARMOR_OFFSETS_FIELD_CRC, ARMOR_OFFSET_VECTOR_TYPE_ID)?;
    Ok(ArmorOffsetDatabase {
        offsets: offsets
            .children()
            .iter()
            .map(parse_armor_offset_data)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn parse_armor_offset_data(element: &Element) -> Result<ArmorOffsetData> {
    require_type(element, ARMOR_OFFSET_DATA_TYPE_ID)?;
    let attachments = required_typed_child(
        element,
        ARMOR_OFFSET_ATTACHMENTS_FIELD_CRC,
        ATTACHMENT_OFFSET_VECTOR_TYPE_ID,
    )?;
    Ok(ArmorOffsetData {
        name: required_string_field(element, ARMOR_OFFSET_NAME_FIELD_CRC)?,
        attachments: attachments
            .children()
            .iter()
            .map(parse_attachment_offset_data)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn parse_attachment_offset_data(element: &Element) -> Result<AttachmentOffsetData> {
    require_type(element, ATTACHMENT_OFFSET_DATA_TYPE_ID)?;
    Ok(AttachmentOffsetData {
        attachment: required_string_field(element, ATTACHMENT_NAME_FIELD_CRC)?,
        position: required_vec3_field(element, ATTACHMENT_OFFSET_POSITION_FIELD_CRC)?,
        rotation_degrees: required_vec3_field(
            element,
            ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC,
        )?,
    })
}

fn armor_offset_by_name<'a>(
    database: &'a ArmorOffsetDatabase,
    name: &str,
) -> Option<&'a ArmorOffsetData> {
    database.offsets.iter().find(|offset| offset.name == name)
}

fn furthest_armor_attachment_offset<'a>(
    database: &'a ArmorOffsetDatabase,
    armor_offset_names: &[String],
    attachment_name: &str,
    current_position: Vec3,
) -> Option<&'a AttachmentOffsetData> {
    let mut best = None;
    let mut best_length = current_position.length();
    for offset_name in armor_offset_names {
        let Some(offset) = armor_offset_by_name(database, offset_name) else {
            continue;
        };
        for attachment in &offset.attachments {
            if attachment.attachment != attachment_name {
                continue;
            }
            let length = attachment.position.length();
            if length > best_length {
                best_length = length;
                best = Some(attachment);
            }
        }
    }
    best
}

fn parse_equip_types_database(bytes: &[u8]) -> Result<EquipTypesDatabase> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, EQUIP_TYPES_DATABASE_TYPE_ID)?;
    let equip_types =
        required_typed_child(root, EQUIP_TYPES_FIELD_CRC, EQUIP_TYPE_DATA_VECTOR_TYPE_ID)?;
    Ok(EquipTypesDatabase {
        equip_types: equip_types
            .children()
            .iter()
            .map(parse_equip_type_data)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn parse_equip_type_data(element: &Element) -> Result<EquipTypeData> {
    require_type(element, EQUIP_TYPE_DATA_TYPE_ID)?;
    Ok(EquipTypeData {
        name: required_string_field(element, EQUIP_NAME_FIELD_CRC)?,
        attachment: required_string_field(element, EQUIP_ATTACHMENT_FIELD_CRC)?,
        attachment_offset_position: required_vec3_field(
            element,
            EQUIP_ATTACHMENT_OFFSET_POSITION_FIELD_CRC,
        )?,
        attachment_offset_rotation_degrees: required_vec3_field(
            element,
            EQUIP_ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC,
        )?,
        sheath_data: required_string_field(element, EQUIP_SHEATH_DATA_FIELD_CRC)?,
        sheath_offset_position: required_vec3_field(
            element,
            EQUIP_SHEATH_OFFSET_POSITION_FIELD_CRC,
        )?,
        sheath_offset_rotation_degrees: required_vec3_field(
            element,
            EQUIP_SHEATH_OFFSET_ROTATION_DEGREES_FIELD_CRC,
        )?,
        off_hand_attachment: required_string_field(element, EQUIP_OFF_HAND_ATTACHMENT_FIELD_CRC)?,
        off_hand_attachment_offset_position: required_vec3_field(
            element,
            EQUIP_OFF_HAND_ATTACHMENT_OFFSET_POSITION_FIELD_CRC,
        )?,
        off_hand_attachment_offset_rotation_degrees: required_vec3_field(
            element,
            EQUIP_OFF_HAND_ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC,
        )?,
        off_hand_sheath_data: required_string_field(element, EQUIP_OFF_HAND_SHEATH_DATA_FIELD_CRC)?,
        off_hand_sheath_offset_position: required_vec3_field(
            element,
            EQUIP_OFF_HAND_SHEATH_OFFSET_POSITION_FIELD_CRC,
        )?,
        off_hand_sheath_offset_rotation_degrees: required_vec3_field(
            element,
            EQUIP_OFF_HAND_SHEATH_OFFSET_ROTATION_DEGREES_FIELD_CRC,
        )?,
    })
}

fn parse_game_debug_settings(bytes: &[u8]) -> Result<GameDebugSettings> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, GAME_DEBUG_SETTINGS_TYPE_ID)?;
    let combat =
        required_typed_child(root, COMBAT_SETTINGS_FIELD_CRC, COMBAT_DEBUG_SETTINGS_TYPE_ID)?;
    Ok(GameDebugSettings {
        combat_settings: CombatDebugSettings {
            disable_player_loot_drop_on_death: required_bool_field(
                combat,
                DISABLE_PLAYER_LOOT_DROP_ON_DEATH_FIELD_CRC,
            )?,
            disable_weapon_durability: required_bool_field(
                combat,
                DISABLE_WEAPON_DURABILITY_FIELD_CRC,
            )?,
            disable_item_durability: required_bool_field(
                combat,
                DISABLE_ITEM_DURABILITY_FIELD_CRC,
            )?,
            disable_durability_penalty_on_death: required_bool_field(
                combat,
                DISABLE_DURABILITY_PENALTY_ON_DEATH_FIELD_CRC,
            )?,
        },
    })
}

fn disabled_combat_toggle_count(combat: &CombatDebugSettings) -> usize {
    [
        combat.disable_player_loot_drop_on_death,
        combat.disable_weapon_durability,
        combat.disable_item_durability,
        combat.disable_durability_penalty_on_death,
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count()
}

fn parse_ui_database(bytes: &[u8]) -> Result<UiDatabase> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, UI_DATABASE_TYPE_ID)?;
    let unified = child_at(root, 0, UNIFIED_INTERACT_DATA_TYPE_ID)?;
    let options = child_at(unified, 0, INTERACT_OPTION_VECTOR_TYPE_ID)?;
    Ok(UiDatabase {
        unified_interact_data: UnifiedInteractData {
            interact_options: options
                .children()
                .iter()
                .map(parse_interact_option_data)
                .collect::<Result<Vec<_>>>()?,
        },
    })
}

fn parse_interact_option_data(element: &Element) -> Result<InteractOptionData> {
    require_type(element, INTERACT_OPTION_DATA_TYPE_ID)?;
    if element.children().len() < 45 {
        bail!(
            "InteractOptionData has {} children, expected at least 45",
            element.children().len()
        );
    }
    Ok(InteractOptionData {
        name: string_child(element, 0)?,
        display_name: string_child(element, 1)?,
        interact_input_type: wrapped_i32(child(element, 2)?)?,
        ui_interact_action: wrapped_u8(child(element, 3)?)?,
        additional_info_type: wrapped_i32(child(element, 4)?)?,
        interact_option_category: wrapped_i32(child(element, 5)?)?,
        delayed_interaction_data: parse_delayed_interaction_data(child(element, 6)?)?,
        interact_privilege_ids: child(element, 7)?
            .children()
            .iter()
            .map(wrapped_u32)
            .collect::<Result<Vec<_>>>()?,
        blueprint_privilege_id: wrapped_u32(child(element, 8)?)?,
        requires_confirmation: bool_child(element, 9)?,
        is_committed_interaction: bool_child(element, 10)?,
        is_instant_cancel: bool_child(element, 11)?,
        close_prompt_on_interaction: bool_child(element, 12)?,
        force_secondary_interact: bool_child(element, 13)?,
        only_show_if_bound_to_camp: bool_child(element, 14)?,
        display_priority: i32_child(element, 15)?,
        interact_option_icon: first_string_descendant(child(element, 16)?).unwrap_or_default(),
        ui_additional_info_slice_path: string_child(element, 17)?,
        requires_security_level_validation: bool_child(element, 18)?,
        mannequin_fragment: string_child(element, 19)?,
        mannequin_tag: string_child(element, 20)?,
        align_to_interaction: bool_child(element, 21)?,
        hold_action_press_time: f32_child(element, 22)?,
        cooldown_time: i32_child(element, 23)?,
        set_ownership_on_interact: bool_child(element, 24)?,
        required_item_name: string_child(element, 25)?,
        required_item_count: i32_child(element, 26)?,
        required_currency: i32_child(element, 27)?,
        availability: wrapped_i32(child(element, 28)?)?,
        siege_warfare_game_event_name: string_child(element, 29)?,
        added_status_effects: parse_effects(child(element, 30)?)?,
        required_status_effects: parse_effects(child(element, 31)?)?,
        remove_status_effects: parse_effects(child(element, 32)?)?,
        excluded_status_effects: parse_effects(child(element, 33)?)?,
        delay_before_adding_removing_effect: f32_child(element, 34)?,
        remove_added_effects_on_interaction_end: bool_child(element, 35)?,
        check_pvp_flag_is_set: bool_child(element, 36)?,
        faction_required: bool_child(element, 37)?,
        show_instanced_loot_item_count: bool_child(element, 38)?,
        required_achievement_name: string_child(element, 39)?,
        required_level: u32_child(element, 40)?,
        committed_interaction_max_usage_timeout: f32_child(element, 41)?,
        committed_interaction_max_usage_timeout_notification: string_child(element, 42)?,
        committed_interaction_inactive_timeout: f32_child(element, 43)?,
        committed_interaction_inactive_timeout_notification: string_child(element, 44)?,
    })
}

fn parse_delayed_interaction_data(element: &Element) -> Result<DelayedInteractionData> {
    Ok(DelayedInteractionData {
        delay_time: f32_child(element, 0)?,
        delay_mannequin_tag: string_child(element, 1)?,
    })
}

fn parse_effects(element: &Element) -> Result<Vec<EffectData>> {
    Ok(element
        .children()
        .iter()
        .map(|effect| EffectData {
            effect_id: first_string_descendant(effect).unwrap_or_default(),
        })
        .collect())
}

fn interact_option_by_crc(
    options: &[InteractOptionData],
    key: Crc32,
) -> Option<&InteractOptionData> {
    options
        .iter()
        .find(|option| Crc32::from_str_lower(&option.name) == key)
}

fn interact_options_by_category(
    options: &[InteractOptionData],
    category: i32,
) -> impl Iterator<Item = &InteractOptionData> {
    options
        .iter()
        .filter(move |option| {
            option.interact_option_category == category
                || option.interact_option_category == ALL_INTERACT_OPTIONS_CATEGORY
        })
}

fn parse_game_camera_settings(bytes: &[u8]) -> Result<GameCameraSettings> {
    let xml = String::from_utf8_lossy(bytes);
    let xml = xml.trim_start_matches('\u{feff}');
    let fields = xml_fields(xml);
    let mut camera_states = Vec::new();
    for (attrs, body) in xml_tag_blocks(xml, "CameraState") {
        let mut from_transitions = Vec::new();
        for (transition_attrs, transition_body) in xml_transition_blocks(&body) {
            let transition_fields = xml_fields(&transition_body);
            from_transitions.push(CameraStateTransition {
                from_camera: first_non_empty([
                    transition_attrs.get("FromCamera"),
                    transition_attrs.get("fromCamera"),
                    transition_fields.get("FromCamera"),
                ])
                .unwrap_or_default()
                .to_owned(),
                smooth_time: first_non_empty([
                    transition_attrs.get("SmoothTime"),
                    transition_attrs.get("smoothTime"),
                    transition_fields.get("SmoothTime"),
                ])
                .and_then(parse_optional_f32),
            });
        }
        camera_states.push(CameraStateSettings {
            name: attrs.get("name").cloned().unwrap_or_default(),
            include: attrs.get("include").cloned(),
            fields: xml_fields(&body),
            from_transitions,
        });
    }
    Ok(GameCameraSettings {
        default_state_name: fields.get("defaultStateName").cloned().unwrap_or_default(),
        fields,
        camera_states,
    })
}

fn parse_player_base_attributes(bytes: &[u8]) -> Result<PlayerBaseAttributes> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, PLAYER_BASE_ATTRIBUTES_TYPE_ID)?;
    Ok(PlayerBaseAttributes {
        player_attribute_data: parse_player_attribute_data(required_section(root, "Player Attribute Data", PLAYER_ATTRIBUTE_DATA_TYPE_ID)?)?,
        guild_siege_window_region_data: parse_guild_regions(required_section(root, "Guild Siege Window Region Data", GUILD_SIEGE_WINDOW_REGION_DATA_TYPE_ID)?)?,
        faction_influence_config_data: parse_faction_influence_config(required_section(root, "Faction Influence Config Data", FACTION_INFLUENCE_CONFIG_DATA_TYPE_ID)?)?,
        valid_group_data: parse_valid_group_data(required_section(root, "Valid Group Data", VALID_GROUP_DATA_TYPE_ID)?)?,
        war_data: parse_war_data(required_section(root, "War Data", WAR_DATA_TYPE_ID)?)?,
    })
}

fn parse_player_attribute_data(element: &Element) -> Result<PlayerAttributeData> {
    Ok(PlayerAttributeData {
        base_deployable_limit: required_i32_field_by_name(element, "Base Deployable Limit")?,
        player_display_level_unlock_free_gear_sets: required_i32_field_by_name(element, "Player Display Level Unlock Free Gear Sets")?,
        item_rarity_data: required_field_by_name(element, "Item Rarity Data")?.children().iter().map(parse_item_rarity_data).collect::<Result<Vec<_>>>()?,
        perk_generation_data: parse_perk_generation_data(required_field_by_name(element, "Perk Generation Data")?)?,
        perk_chance_item_id: required_string_field_by_name(element, "Perk Chance ItemId")?,
        ability_points_required_in_tree_to_unlock_final_row: required_i32_field_by_name(element, "Ability Points Required In Tree to Unlock Final Row")?,
        perk_chance_modifier: required_f32_field_by_name(element, "Perk Chance Modifier")?,
        attribute_chance_modifier: required_f32_field_by_name(element, "Attribute Chance Modifier")?,
        gem_slot_chance_modifier: required_f32_field_by_name(element, "Gem Slot Chance Modifier")?,
    })
}

fn parse_item_rarity_data(element: &Element) -> Result<ItemRarityData> {
    Ok(ItemRarityData {
        rarity_level_loc_string: required_string_field_by_name(element, "Rarity Level Loc String")?,
        max_perk_count: required_i32_field_by_name(element, "Max Perk Count")?,
        level_requirement_modifier: required_i32_field_by_name(element, "Level Requirement Modifier")?,
    })
}

fn parse_perk_generation_data(element: &Element) -> Result<PerkGenerationData> {
    Ok(PerkGenerationData {
        perk_data_per_tier: required_field_by_name(element, "Perk Data Per Tier")?.children().iter().map(parse_perk_tier_data).collect::<Result<Vec<_>>>()?,
        crafting_result_loot_bucket_id: required_crc32_field_by_name(element, "Crafting Result Loot Bucket Id")?,
        crafting_result_loot_bucket: required_string_field_by_name(element, "Crafting Result Loot Bucket")?,
        roll_perk_on_upgrade_gs: required_i32_field_by_name(element, "Roll Perk On Upgrade GS")?,
        roll_perk_on_upgrade_tier: required_i32_field_by_name(element, "Roll Perk On Upgrade Tier")?,
        roll_perk_on_upgrade_perk_count: required_i32_field_by_name(element, "Roll Perk On Upgrade Perk Count")?,
    })
}

fn parse_perk_tier_data(element: &Element) -> Result<PerkTierData> {
    Ok(PerkTierData {
        max_perk_channel: required_i32_field_by_name(element, "Max Perk Channel")?,
        gem_slot_probability: required_f32_field_by_name(element, "Gem Slot Probability")?,
        attribute_perk_probability: required_f32_field_by_name(element, "Attribute Perk Probability")?,
        general_gear_score_perk_count: parse_i32_range_map(required_field_by_name(element, "General Gear Score Perk Count")?)?,
        crafting_gear_score_perk_count: parse_i32_range_map(required_field_by_name(element, "Crafting Gear Score Perk Count")?)?,
        attribute_perk_bucket: required_string_field_by_name(element, "Attribute Perk Bucket")?,
        attribute_perk_bucket_id: required_crc32_field_by_name(element, "Attribute Perk Bucket Id")?,
    })
}

fn parse_i32_range_map(element: &Element) -> Result<HashMap<i32, Vec<IntRange>>> {
    let mut out = HashMap::new();
    for pair in element.children() {
        let key = required_i32_field_by_name(pair, "value1")?;
        let ranges = required_field_by_name(pair, "value2")?.children().iter().map(|range| {
            Ok(IntRange {
                min: required_i32_field_by_name(range, "value1")?,
                max: required_i32_field_by_name(range, "value2")?,
            })
        }).collect::<Result<Vec<_>>>()?;
        out.insert(key, ranges);
    }
    Ok(out)
}

fn parse_guild_regions(element: &Element) -> Result<HashMap<String, GuildSiegeWindowRegionData>> {
    let mut out = HashMap::new();
    for pair in element.children() {
        out.insert(required_string_field_by_name(pair, "value1")?, parse_guild_region(required_field_by_name(pair, "value2")?)?);
    }
    Ok(out)
}

fn parse_guild_region(element: &Element) -> Result<GuildSiegeWindowRegionData> {
    Ok(GuildSiegeWindowRegionData {
        start_hour: required_u32_field_by_name(element, "Start Hour")?,
        end_hour: required_u32_field_by_name(element, "End Hour")?,
        utc_offset: required_i32_field_by_name(element, "UTCOffset")?,
        dst_rule_id: required_crc32_field_by_name(element, "DstRuleId")?,
        dst_rule: required_string_field_by_name(element, "DstRule")?,
        observes_dst: required_bool_field_by_name(element, "ObservesDst")?,
    })
}

fn parse_faction_influence_config(element: &Element) -> Result<FactionInfluenceConfigData> {
    Ok(FactionInfluenceConfigData {
        max_influence: required_f32_field_by_name(element, "MaxInfluence")?,
        decrement_rate: required_f32_field_by_name(element, "DecrementRate")?,
        increment_rate: required_f32_field_by_name(element, "IncrementRate")?,
        max_increment_time_modifier: required_f32_field_by_name(element, "MaxIncrementTimeModifier")?,
        max_decrement_time_modifier: required_f32_field_by_name(element, "MaxDecrementTimeModifier")?,
        minimum_time_since_last_war: required_f32_field_by_name(element, "MinimumTimeSinceLastWar")?,
        min_territory_diff_to_apply_ud_mechanics: required_i32_field_by_name(element, "MinTerritoryDiffToApplyUDMechanics")?,
        min_time_to_apply_ud_mechanics: required_i32_field_by_name(element, "MinTimeToApplyUDMechanics")?,
        under_dog_mission_influence_gain: required_f32_field_by_name(element, "UnderDogMissionInfluenceGain")?,
        under_dog_mission_influence_gain_cap: required_f32_field_by_name(element, "UnderDogMissionInfluenceGainCap")?,
        uder_dog_faction_rep_gain: required_f32_field_by_name(element, "UderDogFactionRepGain")?,
        under_dog_faction_rep_gain_cap: required_f32_field_by_name(element, "UnderDogFactionRepGainCap")?,
        under_dog_pvp_influence_gain: required_f32_field_by_name(element, "UnderDogPVPInfluenceGain")?,
        under_dog_pvp_influence_gain_cap: required_f32_field_by_name(element, "UnderDogPVPInfluenceGainCap")?,
        minimum_influence_threshold_for_war: required_f32_field_by_name(element, "MinimumInfluenceThresholdForWar")?,
        influence_race_attacker_win_game_event_id: parse_edit_crc(required_field_by_name(element, "Influence Race Attacker Win GameEventId")?)?,
        influence_race_defender_win_game_event_id: parse_edit_crc(required_field_by_name(element, "Influence Race Defender Win GameEventId")?)?,
        influence_race_lose_game_event_id: parse_edit_crc(required_field_by_name(element, "Influence Race Lose GameEventId")?)?,
    })
}

fn parse_valid_group_data(element: &Element) -> Result<ValidGroupData> {
    Ok(ValidGroupData {
        names: required_string_sequence_by_name(element, "names")?,
        objectives: required_string_sequence_by_name(element, "Objectives")?,
        icon_paths: required_string_sequence_by_name(element, "IconPaths")?,
        colors: required_field_by_name(element, "Colors")?.children().iter().map(read_color_rgba).collect::<Result<Vec<_>>>()?,
    })
}

fn parse_war_data(element: &Element) -> Result<WarData> {
    let mut deployable_limits = HashMap::new();
    for pair in required_field_by_name(element, "Deployable Limits")?.children() {
        deployable_limits.insert(required_crc32_field_by_name(pair, "value1")?, parse_war_deployable_limit(required_field_by_name(pair, "value2")?)?);
    }
    Ok(WarData { deployable_limits })
}

fn parse_war_deployable_limit(element: &Element) -> Result<WarDeployableLimitData> {
    Ok(WarDeployableLimitData {
        id: required_crc32_field_by_name(element, "m_id")?,
        display_name: required_string_field_by_name(element, "m_displayName")?,
        buildable_names: required_string_sequence_by_name(element, "m_buildableNames")?,
        buildable_ids: required_crc32_sequence_by_name(element, "m_buildableIds")?,
        attacker_limits: read_i32_triple(required_field_by_name(element, "m_attackerLimits")?)?,
        defender_limit: required_i32_field_by_name(element, "m_defenderLimit")?,
    })
}

fn parse_settlement_progression_data(bytes: &[u8]) -> Result<SettlementProgressionData> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, SETTLEMENT_PROGRESSION_DATA_TYPE_ID)?;
    Ok(SettlementProgressionData {
        settlement_progression_categories: required_typed_child(root, SETTLEMENT_PROGRESSION_CATEGORIES_FIELD_CRC, SETTLEMENT_PROGRESSION_CATEGORY_VECTOR_TYPE_ID)?
            .children().iter().map(parse_progression_category_entry).collect::<Result<Vec<_>>>()?,
    })
}

fn parse_progression_category_entry(element: &Element) -> Result<ProgressionCategoryEntry> {
    require_type(element, PROGRESSION_CATEGORY_ENTRY_TYPE_ID)?;
    Ok(ProgressionCategoryEntry {
        settlement_progression_category: required_string_field(element, SETTLEMENT_PROGRESSION_CATEGORY_FIELD_CRC)?,
        settlement_progression_entries: required_typed_child(element, SETTLEMENT_PROGRESSION_ENTRIES_FIELD_CRC, PROGRESSION_SPAWNER_ENTRY_VECTOR_TYPE_ID)?
            .children().iter().map(parse_progression_spawner_entry).collect::<Result<Vec<_>>>()?,
    })
}

fn parse_progression_spawner_entry(element: &Element) -> Result<ProgressionSpawnerEntry> {
    require_type(element, PROGRESSION_SPAWNER_ENTRY_TYPE_ID)?;
    Ok(ProgressionSpawnerEntry {
        settlement_progression_category_level: required_i32_field(element, SETTLEMENT_PROGRESSION_CATEGORY_LEVEL_FIELD_CRC)?,
        slice: read_asset_reference(required_typed_child(element, SLICE_FIELD_CRC, ASSET_TYPE_ID)?)?,
        alternate_slice: read_asset_reference(required_typed_child(element, ALTERNATE_SLICE_FIELD_CRC, ASSET_TYPE_ID)?)?,
        display_loc_string: required_string_field(element, DISPLAY_LOC_STRING_FIELD_CRC)?,
        icon: read_texture_reference(required_typed_child(element, ICON_FIELD_CRC, SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID)?)?,
    })
}

fn parse_gathering_database(bytes: &[u8]) -> Result<GatheringDatabase> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, GATHERING_DATABASE_TYPE_ID)?;
    Ok(GatheringDatabase { gathering_data: parse_gathering_data(required_typed_child(root, GATHERING_DATA_FIELD_CRC, GATHERING_DATA_TYPE_ID)?)? })
}

fn parse_gathering_data(element: &Element) -> Result<GatheringData> {
    Ok(GatheringData {
        gathering_types: required_typed_child(element, GATHERING_TYPES_FIELD_CRC, GATHERING_TYPE_DATA_VECTOR_TYPE_ID)?.children().iter().map(parse_gathering_type_data).collect::<Result<Vec<_>>>()?,
        gathering_actions: required_typed_child(element, GATHERING_ACTIONS_FIELD_CRC, GATHERING_ACTION_VECTOR_TYPE_ID)?.children().iter().map(parse_gathering_action).collect::<Result<Vec<_>>>()?,
        required_water_gathering_type: required_string_field(element, REQUIRED_WATER_GATHERING_TYPE_FIELD_CRC)?,
        none_gathering_type: required_string_field(element, NONE_GATHERING_TYPE_FIELD_CRC)?,
    })
}

fn parse_gathering_type_data(element: &Element) -> Result<GatheringTypeData> {
    require_type(element, GATHERING_TYPE_DATA_TYPE_ID)?;
    Ok(GatheringTypeData {
        gathering_type: required_string_field(element, TYPE_FIELD_CRC)?,
        ui_icon: read_texture_reference(required_typed_child(element, UI_ICON_FIELD_CRC, SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID)?)?,
        requirement_text: required_string_field(element, REQUIREMENT_TEXT_FIELD_CRC)?,
    })
}

fn parse_gathering_action(element: &Element) -> Result<GatheringAction> {
    require_type(element, GATHERING_ACTION_TYPE_ID)?;
    Ok(GatheringAction {
        name: required_string_field(element, NAME_FIELD_CRC)?,
        mannequin_tag: required_string_field(element, MANNEQUIN_TAG_FIELD_CRC)?,
    })
}

fn parse_gathering_action_database(bytes: &[u8]) -> Result<GatheringActionDatabase> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, GATHERING_ACTION_DATABASE_TYPE_ID)?;
    Ok(GatheringActionDatabase {
        gathering_actions: required_typed_child(root, GATHERING_ACTIONS_FIELD_CRC, GATHERING_ACTION_DATA_VECTOR_TYPE_ID)?
            .children().iter().map(parse_gathering_action_data).collect::<Result<Vec<_>>>()?,
    })
}

fn parse_gathering_action_data(element: &Element) -> Result<GatheringActionData> {
    require_type(element, GATHERING_ACTION_DATA_TYPE_ID)?;
    Ok(GatheringActionData {
        name: required_string_field(element, NAME_FIELD_CRC)?,
        mannequin_tag: required_string_field(element, MANNEQUIN_TAG_FIELD_CRC)?,
    })
}

fn parse_crafting_station_database(bytes: &[u8]) -> Result<CraftingStationDatabase> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, CRAFTING_STATION_DATABASE_TYPE_ID)?;
    Ok(CraftingStationDatabase {
        crafting_stations: required_typed_child(root, CRAFTING_STATIONS_FIELD_CRC, CRAFTING_STATION_DATA_VECTOR_TYPE_ID)?
            .children().iter().map(parse_crafting_station_data).collect::<Result<Vec<_>>>()?,
    })
}

fn parse_crafting_station_data(element: &Element) -> Result<CraftingStationData> {
    require_type(element, CRAFTING_STATION_DATA_TYPE_ID)?;
    Ok(CraftingStationData {
        name: required_string_field(element, NAME_FIELD_CRC)?,
        crafting_types: read_string_vector(required_typed_child(element, CRAFTING_TYPES_FIELD_CRC, CRAFTING_TYPE_STRING_VECTOR_TYPE_ID)?)?,
        mannequin_tag: required_string_field(element, CRAFTING_MANNEQUIN_TAG_FIELD_CRC)?,
        azoth_discount_percent: required_f32_field(element, AZOTH_DISCOUNT_PERCENT_FIELD_CRC)?,
    })
}

fn parse_social_rank_database(bytes: &[u8]) -> Result<SocialRankDatabase> {
    let stream = strict_object_stream(bytes)?;
    let root = single_root(&stream, SOCIAL_RANK_DATABASE_TYPE_ID)?;
    Ok(SocialRankDatabase {
        ranks: required_typed_child(root, SOCIAL_RANKS_FIELD_CRC, SOCIAL_RANK_DATA_VECTOR_TYPE_ID)?
            .children().iter().map(parse_social_rank_data).collect::<Result<Vec<_>>>()?,
    })
}

fn parse_social_rank_data(element: &Element) -> Result<SocialRankData> {
    require_type(element, SOCIAL_RANK_DATA_TYPE_ID)?;
    Ok(SocialRankData {
        guild_rank_data: parse_social_guild_rank_data(required_typed_child(element, SOCIAL_GUILD_RANK_DATA_FIELD_CRC, SOCIAL_GUILD_RANK_DATA_TYPE_ID)?)?,
    })
}

fn parse_social_guild_rank_data(element: &Element) -> Result<SocialGuildRankData> {
    Ok(SocialGuildRankData {
        name: required_string_field(element, SOCIAL_GUILD_RANK_NAME_FIELD_CRC)?,
        security_level: required_u32_field(element, SOCIAL_GUILD_RANK_SECURITY_LEVEL_FIELD_CRC)?,
        all_privileges: required_bool_field(element, SOCIAL_GUILD_RANK_ALL_PRIVILEGES_FIELD_CRC)?,
        privilege_ids: required_typed_child(element, SOCIAL_GUILD_RANK_PRIVILEGE_IDS_FIELD_CRC, SOCIAL_PRIVILEGE_ID_SET_TYPE_ID)?
            .children().iter().map(read_u32_value).collect::<Result<Vec<_>>>()?,
    })
}

fn strict_object_stream(bytes: &[u8]) -> Result<ObjectStream> {
    let stream = ObjectStream::from_bytes(bytes, None)?;
    if stream.version() != 3 {
        bail!("unsupported ObjectStream version {}", stream.version());
    }
    Ok(stream)
}

fn single_root<'a>(stream: &'a ObjectStream, type_id: &str) -> Result<&'a Element> {
    let [root] = stream.elements() else {
        bail!("expected one ObjectStream root, found {}", stream.elements().len());
    };
    require_type(root, type_id)?;
    Ok(root)
}

fn require_type(element: &Element, type_id: &str) -> Result<()> {
    if element.id().to_string().eq_ignore_ascii_case(type_id) {
        Ok(())
    } else {
        bail!("expected ObjectStream type {type_id}, found {}", element.id())
    }
}

fn required_typed_child<'a>(element: &'a Element, name_crc: u32, type_id: &str) -> Result<&'a Element> {
    let child = required_child_by_crc(element, name_crc)?;
    require_type(child, type_id)?;
    Ok(child)
}

fn required_child_by_crc(element: &Element, name_crc: u32) -> Result<&Element> {
    element.children().iter().find(|child| child.name_crc() == Some(name_crc))
        .with_context(|| format!("ObjectStream element {} is missing field CRC {name_crc}", element.id()))
}

fn required_section<'a>(element: &'a Element, field_name: &str, type_id: &str) -> Result<&'a Element> {
    required_typed_child(element, crc32_lowercase(field_name), type_id)
}

fn required_field_by_name<'a>(element: &'a Element, field_name: &str) -> Result<&'a Element> {
    required_child_by_crc(element, crc32_lowercase(field_name))
}

fn required_string_field(element: &Element, name_crc: u32) -> Result<String> {
    Ok(required_typed_child(element, name_crc, AZSTD_STRING_TYPE_ID)?.decode::<String>()?)
}

fn required_string_field_by_name(element: &Element, field_name: &str) -> Result<String> {
    required_string_field(element, crc32_lowercase(field_name))
}

fn required_i32_field(element: &Element, name_crc: u32) -> Result<i32> {
    Ok(required_typed_child(element, name_crc, INT_TYPE_ID)?.decode::<i32>()?)
}

fn required_i32_field_by_name(element: &Element, field_name: &str) -> Result<i32> {
    required_i32_field(element, crc32_lowercase(field_name))
}

fn required_u32_field(element: &Element, name_crc: u32) -> Result<u32> {
    Ok(required_typed_child(element, name_crc, U32_TYPE_ID)?.decode::<u32>()?)
}

fn required_u32_field_by_name(element: &Element, field_name: &str) -> Result<u32> {
    required_u32_field(element, crc32_lowercase(field_name))
}

fn required_f32_field(element: &Element, name_crc: u32) -> Result<f32> {
    Ok(required_typed_child(element, name_crc, FLOAT_TYPE_ID)?.decode::<f32>()?)
}

fn required_f32_field_by_name(element: &Element, field_name: &str) -> Result<f32> {
    required_f32_field(element, crc32_lowercase(field_name))
}

fn required_bool_field(element: &Element, name_crc: u32) -> Result<bool> {
    Ok(required_typed_child(element, name_crc, BOOL_TYPE_ID)?.decode::<bool>()?)
}

fn required_bool_field_by_name(element: &Element, field_name: &str) -> Result<bool> {
    required_bool_field(element, crc32_lowercase(field_name))
}

fn required_vec3_field(element: &Element, name_crc: u32) -> Result<Vec3> {
    read_vec3_value(required_typed_child(element, name_crc, VECTOR3_TYPE_ID)?)
}

fn child(element: &Element, index: usize) -> Result<&Element> {
    element
        .children()
        .get(index)
        .with_context(|| format!("ObjectStream element {} is missing child {index}", element.id()))
}

fn child_at<'a>(element: &'a Element, index: usize, type_id: &str) -> Result<&'a Element> {
    let child = child(element, index)?;
    require_type(child, type_id)?;
    Ok(child)
}

fn string_child(element: &Element, index: usize) -> Result<String> {
    Ok(child(element, index)?.decode::<String>()?)
}

fn bool_child(element: &Element, index: usize) -> Result<bool> {
    Ok(child(element, index)?.decode::<bool>()?)
}

fn i32_child(element: &Element, index: usize) -> Result<i32> {
    Ok(child(element, index)?.decode::<i32>()?)
}

fn u32_child(element: &Element, index: usize) -> Result<u32> {
    Ok(child(element, index)?.decode::<u32>()?)
}

fn f32_child(element: &Element, index: usize) -> Result<f32> {
    Ok(child(element, index)?.decode::<f32>()?)
}

fn required_crc32_field_by_name(element: &Element, field_name: &str) -> Result<Crc32> {
    read_crc32(required_field_by_name(element, field_name)?)
}

fn required_string_sequence_by_name(element: &Element, field_name: &str) -> Result<Vec<String>> {
    read_string_vector(required_field_by_name(element, field_name)?)
}

fn required_crc32_sequence_by_name(element: &Element, field_name: &str) -> Result<Vec<Crc32>> {
    required_field_by_name(element, field_name)?.children().iter().map(read_crc32).collect()
}

fn read_string_vector(element: &Element) -> Result<Vec<String>> {
    element.children().iter().map(read_string_value).collect()
}

fn read_string_value(element: &Element) -> Result<String> {
    require_type(element, AZSTD_STRING_TYPE_ID)?;
    Ok(element.decode::<String>()?)
}

fn read_vec3_value(element: &Element) -> Result<Vec3> {
    let [x, y, z] = value::read_vec3(element)?;
    Ok(Vec3::new(x, y, z))
}

fn read_i32_value(element: &Element) -> Result<i32> {
    if element.id().to_string().eq_ignore_ascii_case(INT_TYPE_ID) {
        return Ok(element.decode::<i32>()?);
    }
    let [child] = element.children() else {
        bail!("ObjectStream element {} is not an i32 value", element.id());
    };
    read_i32_value(child)
}

fn read_u32_value(element: &Element) -> Result<u32> {
    require_type(element, U32_TYPE_ID)?;
    Ok(element.decode::<u32>()?)
}

fn wrapped_i32(element: &Element) -> Result<i32> {
    if element.id().to_string().eq_ignore_ascii_case(INT_TYPE_ID) {
        return Ok(element.decode::<i32>()?);
    }
    let [child] = element.children() else {
        bail!("ObjectStream element {} is not a wrapped i32 value", element.id());
    };
    wrapped_i32(child)
}

fn wrapped_u32(element: &Element) -> Result<u32> {
    if element.id().to_string().eq_ignore_ascii_case(U32_TYPE_ID) {
        return Ok(element.decode::<u32>()?);
    }
    let [child] = element.children() else {
        bail!("ObjectStream element {} is not a wrapped u32 value", element.id());
    };
    wrapped_u32(child)
}

fn wrapped_u8(element: &Element) -> Result<u8> {
    if element.id().to_string().eq_ignore_ascii_case(U8_TYPE_ID) {
        return Ok(element.decode::<u8>()?);
    }
    let [child] = element.children() else {
        bail!("ObjectStream element {} is not a wrapped u8 value", element.id());
    };
    wrapped_u8(child)
}

fn first_string_descendant(element: &Element) -> Option<String> {
    if let Ok(value) = value::read_string(element) {
        return Some(value.to_owned());
    }
    element.children().iter().find_map(first_string_descendant)
}

fn read_i32_triple(element: &Element) -> Result<[i32; 3]> {
    let values = element.children().iter().map(read_i32_value).collect::<Result<Vec<_>>>()?;
    values.try_into().map_err(|values: Vec<i32>| anyhow::anyhow!("expected 3 i32 values, found {}", values.len()))
}

fn read_crc32(element: &Element) -> Result<Crc32> {
    Ok(Crc32::new(value::read_crc32(element)?))
}

fn parse_edit_crc(element: &Element) -> Result<EditCrc> {
    require_type(element, EDIT_CRC_TYPE_ID)?;
    Ok(EditCrc {
        value_str: required_string_field_by_name(element, "m_valueStr")?,
        value_crc: required_crc32_field_by_name(element, "m_valueCrc")?,
    })
}

fn read_color_rgba(element: &Element) -> Result<ColorRgba> {
    let [r, g, b, a] = value::read_color(element)?;
    Ok(ColorRgba { r, g, b, a })
}

fn read_asset_reference(element: &Element) -> Result<AssetReference> {
    Ok(asset_reference::read_asset_value(element)?.into_asset_reference())
}

fn read_texture_reference(element: &Element) -> Result<SimpleAssetReferenceTextureAsset> {
    require_type(element, SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID)?;
    Ok(SimpleAssetReferenceTextureAsset {
        asset_path: asset_reference::read_simple_asset_reference_path_any_owned(element)?.unwrap_or_default(),
    })
}

fn xml_fields(xml: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (tag, attrs) in xml_empty_elements(xml) {
        let name = attrs.get("name").cloned().unwrap_or(tag);
        let value = attrs.get("value").cloned().unwrap_or_default();
        out.insert(name, value);
    }
    out
}

fn xml_tag_blocks(xml: &str, tag: &str) -> Vec<(HashMap<String, String>, String)> {
    let mut out = Vec::new();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut cursor = 0;
    while let Some(relative_start) = xml[cursor..].find(&open) {
        let start = cursor + relative_start;
        let boundary = xml[start + open.len()..].chars().next();
        if !matches!(boundary, Some(' ' | '\t' | '\r' | '\n' | '>' | '/')) {
            cursor = start + 1;
            continue;
        }
        let Some(header_end_relative) = xml[start..].find('>') else {
            break;
        };
        let header_end = start + header_end_relative;
        let attrs = xml_attributes(&xml[start + open.len()..header_end]);
        let body_start = header_end + 1;
        let Some(body_end_relative) = xml[body_start..].find(&close) else {
            break;
        };
        let body_end = body_start + body_end_relative;
        out.push((attrs, xml[body_start..body_end].to_owned()));
        cursor = body_end + close.len();
    }
    out
}

fn xml_transition_blocks(xml: &str) -> Vec<(HashMap<String, String>, String)> {
    let mut out = Vec::new();
    let open = "<FromTransition";
    let close = "</FromTransition>";
    let mut cursor = 0;
    while let Some(relative_start) = xml[cursor..].find(open) {
        let start = cursor + relative_start;
        let Some(header_end_relative) = xml[start..].find('>') else {
            break;
        };
        let header_end = start + header_end_relative;
        let attrs_source = &xml[start + open.len()..header_end];
        let attrs = xml_attributes(attrs_source);
        if attrs_source.trim_end().ends_with('/') {
            out.push((attrs, String::new()));
            cursor = header_end + 1;
            continue;
        }
        let body_start = header_end + 1;
        let Some(body_end_relative) = xml[body_start..].find(close) else {
            break;
        };
        let body_end = body_start + body_end_relative;
        out.push((attrs, xml[body_start..body_end].to_owned()));
        cursor = body_end + close.len();
    }
    out
}

fn xml_empty_elements(xml: &str) -> Vec<(String, HashMap<String, String>)> {
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = xml[cursor..].find('<') {
        let start = cursor + relative_start;
        let Some(end_relative) = xml[start..].find('>') else {
            break;
        };
        let end = start + end_relative;
        let mut inner = xml[start + 1..end].trim();
        cursor = end + 1;
        if !inner.ends_with('/') {
            continue;
        }
        inner = inner.trim_end_matches('/').trim();
        if inner.starts_with('/') || inner.starts_with('!') || inner.starts_with('?') {
            continue;
        }
        let tag_end = inner
            .find(|character: char| character.is_ascii_whitespace())
            .unwrap_or(inner.len());
        let tag = &inner[..tag_end];
        if tag.is_empty() {
            continue;
        }
        out.push((tag.to_owned(), xml_attributes(&inner[tag_end..])));
    }
    out
}

fn xml_attributes(source: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && !is_xml_name_byte(bytes[index]) {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len() && is_xml_name_byte(bytes[index]) {
            index += 1;
        }
        if name_start == index {
            break;
        }
        let name = &source[name_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'"') {
            continue;
        }
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != b'"' {
            index += 1;
        }
        let value = decode_xml_entities(&source[value_start..index]);
        out.insert(name.to_owned(), value);
        if index < bytes.len() {
            index += 1;
        }
    }
    out
}

fn is_xml_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':' | b'-')
}

fn decode_xml_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn first_non_empty<const N: usize>(values: [Option<&String>; N]) -> Option<&str> {
    values
        .into_iter()
        .flatten()
        .find(|value| !value.is_empty())
        .map(String::as_str)
}

fn parse_optional_f32(value: &str) -> Option<f32> {
    let trimmed = value.trim_end_matches(['f', 'F']);
    trimmed
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

fn crc32_lowercase(value: &str) -> u32 {
    Crc32::from_str_lower(value).value()
}

"#;

const RUST_STANDALONE_DYNAMIC_MANAGER_RUNTIME: &str = r#"
#[derive(Debug, Clone, Copy)]
struct TableSelector {
    name: &'static str,
    row_type: &'static str,
}

impl TableSelector {
    const fn new(name: &'static str, row_type: &'static str) -> Self {
        Self { name, row_type }
    }
}

#[derive(Debug, Clone)]
struct ManagerResources {
    manager_name: &'static str,
    tables: HashMap<String, HashMap<String, Arc<DynamicTable>>>,
    table_order: Vec<Arc<DynamicTable>>,
    assets: HashMap<String, Arc<[u8]>>,
}

struct ManagerCache {
    loader: crate::assets::AssetLoader,
    assets_by_path: HashMap<String, Arc<[u8]>>,
    table_cache: HashMap<String, Arc<DynamicTable>>,
}

impl ManagerCache {
    fn new(loader: crate::assets::AssetLoader) -> Self {
        Self {
            loader,
            assets_by_path: HashMap::new(),
            table_cache: HashMap::new(),
        }
    }

    fn resources_for_tables(
        &mut self,
        manager_name: &'static str,
        tables: &[TableSelector],
        asset_paths: &[&str],
    ) -> Result<ManagerResources> {
        let schemas = tables
            .iter()
            .map(|selector| {
                table_schema(*selector).with_context(|| format!("manager {manager_name}"))
            })
            .collect::<Result<Vec<_>>>()?;
        self.resources_from_schemas(manager_name, schemas, asset_paths)
    }

    fn resources_for_rows(
        &mut self,
        manager_name: &'static str,
        row_types: &[&str],
        asset_paths: &[&str],
    ) -> Result<ManagerResources> {
        for row_type in row_types {
            if !TABLES.iter().any(|table| table.row_type == *row_type) {
                bail!("manager {manager_name} uses unknown row type {row_type}");
            }
        }
        let schemas = TABLES
            .iter()
            .filter(|table| row_types.contains(&table.row_type.as_str()))
            .collect::<Vec<_>>();
        self.resources_from_schemas(manager_name, schemas, asset_paths)
    }

    fn resources_from_schemas(
        &mut self,
        manager_name: &'static str,
        schemas: Vec<&'static TableDescriptor>,
        asset_paths: &[&str],
    ) -> Result<ManagerResources> {
        let mut tables = HashMap::new();
        let mut table_order = Vec::with_capacity(schemas.len());
        let mut assets = HashMap::new();
        for schema in schemas {
            let table = self.build_table(schema)?;
            tables
                .entry(schema.name.clone())
                .or_insert_with(HashMap::new)
                .insert(schema.row_type.clone(), table.clone());
            table_order.push(table);
        }
        for path in asset_paths {
            let asset = self
                .load_asset(path)
                .with_context(|| format!("manager {manager_name} asset {path}"))?;
            assets.insert(normalize_data_path(path), asset);
        }
        Ok(ManagerResources {
            manager_name,
            tables,
            table_order,
            assets,
        })
    }

    fn build_table(&mut self, schema: &'static TableDescriptor) -> Result<Arc<DynamicTable>> {
        let cache_key = format!("{}:{}", schema.name, schema.row_type);
        if let Some(cached) = self.table_cache.get(&cache_key) {
            return Ok(cached.clone());
        }
        let row_key_column = schema.columns.iter().find(|column| column.row_key);

        let mut rows = Vec::new();
        for source_path in &schema.sources {
            let bytes = self.load_asset(source_path)?;
            let sheet = nw_datasheet::Datasheet::parse(bytes.as_ref())
                .with_context(|| format!("parse datasheet {source_path}"))?;
            let Some(row_key_column) = row_key_column else {
                if !sheet.is_empty() {
                    bail!("non-empty datasheet source {source_path} has no row-key column");
                }
                continue;
            };
            let column_slots = Arc::new(column_slots_for_sheet(schema, &sheet));
            let row_key_slot = *column_slots.get(&row_key_column.crc).with_context(|| {
                format!(
                    "datasheet source {source_path} missing row-key column {}",
                    row_key_column.name
                )
            })?;
            for (row_index, row) in sheet.rows().enumerate() {
                let Some(key_cell) = row.cell(row_key_slot) else {
                    continue;
                };
                let Some(key) = row_key_value(key_cell.value()) else {
                    continue;
                };
                let cells = row
                    .cells()
                    .iter()
                    .map(|cell| owned_cell_value(cell.value()))
                    .collect();
                let dynamic_row = DynamicTableRow {
                    source_path: normalize_data_path(source_path),
                    row_index,
                    key: key.clone(),
                    cells,
                    column_slots: column_slots.clone(),
                };
                rows.push(dynamic_row);
            }
        }

        let column_crcs = schema
            .columns
            .iter()
            .map(|column| (column.name.clone(), column.crc))
            .collect();
        let table = Arc::new(DynamicTable {
            schema,
            rows,
            column_crcs,
        });
        self.table_cache.insert(cache_key, table.clone());
        Ok(table)
    }

    fn asset(&self, path: &str) -> Option<&Arc<[u8]>> {
        let normalized = normalize_data_path(path);
        self.assets_by_path
            .get(&normalized)
            .or_else(|| {
                self.assets_by_path
                    .iter()
                    .find_map(|(candidate, bytes)| candidate.ends_with(&format!("/{normalized}")).then_some(bytes))
            })
    }

    fn load_asset(&mut self, path: &str) -> Result<Arc<[u8]>> {
        if let Some(bytes) = self.asset(path) {
            return Ok(bytes.clone());
        }
        let bytes: Arc<[u8]> = self
            .loader
            .read(path)
            .with_context(|| format!("read asset {path}"))?
            .into();
        self.assets_by_path
            .insert(normalize_data_path(path), bytes.clone());
        Ok(bytes)
    }
}

impl ManagerResources {
    #[must_use]
    fn table(&self, selector: TableSelector) -> Option<&DynamicTable> {
        self.tables
            .get(selector.name)?
            .get(selector.row_type)
            .map(Arc::as_ref)
    }

    #[must_use]
    fn asset_bytes(&self, path: &str) -> Option<&[u8]> {
        let normalized = normalize_data_path(path);
        self.assets.get(&normalized).map(AsRef::as_ref).or_else(|| {
            self.assets.iter().find_map(|(candidate, bytes)| {
                candidate
                    .ends_with(&format!("/{normalized}"))
                    .then_some(bytes.as_ref())
            })
        })
    }

    fn required_asset_bytes(&self, path: &str) -> Result<&[u8]> {
        self.asset_bytes(path)
            .with_context(|| format!("manager {} asset {path} was not loaded", self.manager_name))
    }

    fn schema_family_entries<Table: Copy + Eq + std::hash::Hash, T>(
        &self,
        row_type: &str,
        resolve_table: fn(&str) -> Option<Table>,
        read: fn(&DynamicTable, &DynamicTableRow) -> Result<T>,
    ) -> Result<Vec<RowEntry<Table, T>>> {
        let mut entries = Vec::new();
        for table in &self.table_order {
            if table.schema.row_type != row_type {
                continue;
            }
            let table_id = resolve_table(&table.schema.name).with_context(|| {
                format!(
                    "manager {} row family {row_type} cannot resolve table {}",
                    self.manager_name,
                    table.schema.name,
                )
            })?;
            for row in &table.rows {
                entries.push(RowEntry {
                    reference: RowRef::new(table_id, &row.key),
                    slot: RowSlot::new(table_id, row.row_index),
                    row: read(table.as_ref(), row)?,
                });
            }
        }
        Ok(entries)
    }

}

fn table_schema(selector: TableSelector) -> Result<&'static TableDescriptor> {
    let mut matches = TABLES
        .iter()
        .filter(|table| table.name == selector.name && table.row_type == selector.row_type);
    let table = matches
        .next()
        .with_context(|| format!("unknown table {}:{}", selector.name, selector.row_type))?;
    if matches.next().is_some() {
        bail!(
            "duplicate table schema {}:{}",
            selector.name,
            selector.row_type
        );
    }
    Ok(table)
}

fn column_slots_for_sheet(
    schema: &TableDescriptor,
    sheet: &nw_datasheet::Datasheet<'_>,
) -> HashMap<u32, usize> {
    let mut slots = HashMap::new();
    for column in &schema.columns {
        if let Some(slot) = sheet.column_index_by_crc(column.crc) {
            slots.insert(column.crc, slot);
        }
    }
    slots
}

fn row_cell<'a>(
    table: &DynamicTable,
    row: &'a DynamicTableRow,
    column_name: &str,
) -> Option<&'a DatasheetCellValue> {
    let column_crc = table.column_crcs.get(column_name)?;
    let slot = *row.column_slots.get(column_crc)?;
    row.cells.get(slot)
}

fn required_string_cell<'a>(
    table: &DynamicTable,
    row: &'a DynamicTableRow,
    column_name: &str,
) -> Result<&'a str> {
    match row_cell(table, row, column_name) {
        Some(DatasheetCellValue::String(value)) => Ok(value),
        _ => bail!(
            "row {}:{} missing string {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn optional_string_cell<'a>(
    table: &DynamicTable,
    row: &'a DynamicTableRow,
    column_name: &str,
) -> Result<Option<&'a str>> {
    match row_cell(table, row, column_name) {
        None => Ok(None),
        Some(DatasheetCellValue::String(value)) if value.is_empty() => Ok(None),
        Some(DatasheetCellValue::String(value)) => Ok(Some(value)),
        Some(_) => bail!(
            "row {}:{} has non-string {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn required_schema_string_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<String> {
    match row_cell(table, row, column_name) {
        Some(value) => Ok(schema_string_cell_value(value)),
        None => bail!(
            "row {}:{} missing string {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn optional_schema_string_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<String>> {
    match row_cell(table, row, column_name) {
        None => Ok(None),
        Some(DatasheetCellValue::String(value)) if value.is_empty() => Ok(None),
        Some(value) => Ok(Some(schema_string_cell_value(value))),
    }
}

fn schema_string_cell_value(value: &DatasheetCellValue) -> String {
    match value {
        DatasheetCellValue::String(value) => value.clone(),
        DatasheetCellValue::Number(value) => value.to_string(),
        DatasheetCellValue::Boolean(value) => value.to_string(),
    }
}

fn required_number_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<f32> {
    let Some(value) = row_cell(table, row, column_name) else {
        bail!(
            "row {}:{} missing number {column_name}",
            row.source_path,
            row.row_index + 1
        );
    };
    match number_cell_value(value, row, column_name)? {
        Some(value) => Ok(value),
        None => bail!(
            "row {}:{} missing number {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn optional_number_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<f32>> {
    match row_cell(table, row, column_name) {
        None => Ok(None),
        Some(value) => number_cell_value(value, row, column_name),
    }
}

fn number_cell_value(
    value: &DatasheetCellValue,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<f32>> {
    match value {
        DatasheetCellValue::Number(value) => Ok(Some(*value)),
        DatasheetCellValue::Boolean(value) => Ok(Some(if *value { 1.0 } else { 0.0 })),
        DatasheetCellValue::String(value) => {
            let text = value.trim().to_ascii_lowercase();
            match text.as_str() {
                "" => Ok(None),
                "false" | "no" => Ok(Some(0.0)),
                "true" | "yes" => Ok(Some(1.0)),
                _ => match text.strip_suffix('f').unwrap_or(&text).parse::<f32>() {
                    Ok(value) => Ok(Some(value)),
                    Err(_) => bail!(
                        "row {}:{} has non-number {column_name}={value:?}",
                        row.source_path,
                        row.row_index + 1
                    ),
                },
            }
        }
    }
}

fn required_bool_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<bool> {
    let Some(value) = row_cell(table, row, column_name) else {
        bail!(
            "row {}:{} missing bool {column_name}",
            row.source_path,
            row.row_index + 1
        );
    };
    match bool_cell_value(value, row, column_name)? {
        Some(value) => Ok(value),
        None => bail!(
            "row {}:{} missing bool {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn optional_bool_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<bool>> {
    match row_cell(table, row, column_name) {
        None => Ok(None),
        Some(value) => bool_cell_value(value, row, column_name),
    }
}

fn bool_cell_value(
    value: &DatasheetCellValue,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<bool>> {
    match value {
        DatasheetCellValue::Boolean(value) => Ok(Some(*value)),
        DatasheetCellValue::Number(value) if *value == 0.0 => Ok(Some(false)),
        DatasheetCellValue::Number(value) if *value == 1.0 => Ok(Some(true)),
        DatasheetCellValue::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "" => Ok(None),
            "false" | "0" | "no" => Ok(Some(false)),
            "true" | "1" | "yes" => Ok(Some(true)),
            _ => bail!(
                "row {}:{} has non-bool {column_name}",
                row.source_path,
                row.row_index + 1
            ),
        },
        _ => bail!(
            "row {}:{} has non-bool {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn required_u8_cell(table: &DynamicTable, row: &DynamicTableRow, column_name: &str) -> Result<u8> {
    let value = required_u32_cell(table, row, column_name)?;
    u8::try_from(value).with_context(|| {
        format!(
            "row {}:{} {column_name} exceeds u8",
            row.source_path,
            row.row_index + 1
        )
    })
}

fn optional_u8_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<u8>> {
    optional_u32_cell(table, row, column_name)?
        .map(|value| {
            u8::try_from(value).with_context(|| {
                format!(
                    "row {}:{} {column_name} exceeds u8",
                    row.source_path,
                    row.row_index + 1
                )
            })
        })
        .transpose()
}

fn required_non_zero_u8_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<u8> {
    let value = required_u8_cell(table, row, column_name)?;
    if value == 0 {
        bail!(
            "row {}:{} {column_name} must be non-zero",
            row.source_path,
            row.row_index + 1
        );
    }
    Ok(value)
}

fn required_u16_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<u16> {
    let value = required_u32_cell(table, row, column_name)?;
    u16::try_from(value).with_context(|| {
        format!(
            "row {}:{} {column_name} exceeds u16",
            row.source_path,
            row.row_index + 1
        )
    })
}

fn required_non_zero_u16_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<u16> {
    let value = required_u16_cell(table, row, column_name)?;
    if value == 0 {
        bail!(
            "row {}:{} {column_name} must be non-zero",
            row.source_path,
            row.row_index + 1
        );
    }
    Ok(value)
}

fn required_u32_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<u32> {
    normalize_u32(required_number_cell(table, row, column_name)?)
}

fn required_non_zero_u32_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<u32> {
    require_non_zero_u32(
        required_u32_cell(table, row, column_name)?,
        row,
        column_name,
    )
}

fn require_non_zero_u32(value: u32, row: &DynamicTableRow, column_name: &str) -> Result<u32> {
    if value == 0 {
        bail!(
            "row {}:{} {column_name} must be non-zero",
            row.source_path,
            row.row_index + 1
        );
    }
    Ok(value)
}

fn optional_u32_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<u32>> {
    optional_number_cell(table, row, column_name)?
        .map(normalize_u32)
        .transpose()
}

fn required_i32_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<i32> {
    let value = required_number_cell(table, row, column_name)?;
    if value.fract() != 0.0 || value < i32::MIN as f32 || value > i32::MAX as f32 {
        bail!(
            "row {}:{} expected i32 {column_name}",
            row.source_path,
            row.row_index + 1
        );
    }
    Ok(value as i32)
}

fn required_crc32_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<u32> {
    match row_cell(table, row, column_name) {
        Some(DatasheetCellValue::Number(value)) => normalize_u32(*value),
        Some(DatasheetCellValue::String(value)) => Ok(crc32_lowercase(value)),
        _ => bail!(
            "row {}:{} missing crc {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn optional_crc32_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
    zero_as_none: bool,
) -> Result<Option<u32>> {
    let value = match row_cell(table, row, column_name) {
        None => None,
        Some(DatasheetCellValue::String(value)) if value.is_empty() => None,
        Some(DatasheetCellValue::Number(value)) => Some(normalize_u32(*value)?),
        Some(DatasheetCellValue::String(value)) => Some(crc32_lowercase(value)),
        Some(DatasheetCellValue::Boolean(_)) => None,
    };
    Ok(value.filter(|value| !zero_as_none || *value != 0))
}

fn optional_lowercase_crc_string_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<u32>> {
    Ok(optional_string_cell(table, row, column_name)?.map(crc32_lowercase))
}

fn optional_qualified_crc_u16_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<(u32, u16)>> {
    let Some(value) = optional_string_cell(table, row, column_name)? else {
        return Ok(None);
    };
    let (identifier, rank) = value.split_once(':').with_context(|| {
        format!(
            "row {}:{} column `{column_name}` value `{value}` must have `identifier:rank` syntax",
            row.source_path,
            row.row_index + 1,
        )
    })?;
    let identifier = identifier.trim();
    let rank = rank.trim();
    if identifier.is_empty() || rank.is_empty() {
        bail!(
            "row {}:{} column `{column_name}` value `{value}` must have non-empty identifier and rank",
            row.source_path,
            row.row_index + 1,
        );
    }
    let rank = rank.parse::<u16>().with_context(|| {
        format!(
            "row {}:{} column `{column_name}` value `{value}` has invalid u16 rank `{rank}`",
            row.source_path,
            row.row_index + 1,
        )
    })?;
    Ok(Some((crc32_lowercase(identifier), rank)))
}

fn optional_trimmed_lowercase_crc_string_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<u32>> {
    Ok(optional_string_cell(table, row, column_name)?.and_then(|value| {
        let value = value.trim_ascii();
        (!value.is_empty()).then(|| crc32_lowercase(value))
    }))
}

fn upper_bound_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<f32> {
    let value = required_number_cell(table, row, column_name)?;
    Ok(if value.is_nan() || value.abs() <= f32::EPSILON {
        10_000.0
    } else {
        value.clamp(0.0, 10_000.0)
    })
}

fn lower_bound_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
    upper_bound: f32,
) -> Result<f32> {
    let value = required_number_cell(table, row, column_name)?;
    let value = if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 10_000.0)
    };
    Ok(value.min(upper_bound))
}

fn string_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Vec<String>> {
    match row_cell(table, row, column_name) {
        None => Ok(Vec::new()),
        Some(DatasheetCellValue::String(value)) => Ok(split_designer_list(value)
            .into_iter()
            .map(str::to_owned)
            .collect()),
        Some(DatasheetCellValue::Number(value)) => Ok(vec![value.to_string()]),
        Some(DatasheetCellValue::Boolean(value)) => Ok(vec![value.to_string()]),
    }
}

fn non_empty_string_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Vec<String>> {
    Ok(string_list_cell(table, row, column_name)?
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect())
}

fn optional_string_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<Vec<String>>> {
    if row_cell(table, row, column_name).is_none() {
        return Ok(None);
    }
    let values = string_list_cell(table, row, column_name)?;
    if values.is_empty() {
        Ok(None)
    } else {
        Ok(Some(values))
    }
}

fn number_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Vec<f32>> {
    match row_cell(table, row, column_name) {
        None => Ok(Vec::new()),
        Some(DatasheetCellValue::Number(value)) => Ok(vec![*value]),
        Some(DatasheetCellValue::String(value)) => split_designer_list(value)
            .into_iter()
            .map(|part| parse_designer_number(part, row, column_name))
            .collect(),
        Some(DatasheetCellValue::Boolean(_)) => bail!(
            "row {}:{} has non-number-list {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn f32_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Vec<f32>> {
    number_list_cell(table, row, column_name)
}

fn i32_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Vec<i32>> {
    number_list_cell(table, row, column_name)?
        .into_iter()
        .map(|value| {
            if value.fract() != 0.0 || value < i32::MIN as f32 || value > i32::MAX as f32 {
                bail!(
                    "row {}:{} expected i32 list {column_name}",
                    row.source_path,
                    row.row_index + 1
                );
            }
            Ok(value as i32)
        })
        .collect()
}

fn crc32_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Vec<u32>> {
    match row_cell(table, row, column_name) {
        None => Ok(Vec::new()),
        Some(DatasheetCellValue::Number(value)) => Ok(vec![normalize_u32(*value)?]),
        Some(DatasheetCellValue::String(value)) => split_designer_list(value)
            .into_iter()
            .map(|part| match part.parse::<f32>() {
                Ok(value) => normalize_u32(value),
                Err(_) => Ok(crc32_lowercase(part)),
            })
            .collect(),
        Some(DatasheetCellValue::Boolean(_)) => bail!(
            "row {}:{} has non-crc-list {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn lowercase_crc_string_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Vec<u32>> {
    Ok(string_list_cell(table, row, column_name)?
        .into_iter()
        .filter(|value| !value.is_empty())
        .map(|value| crc32_lowercase(&value))
        .collect())
}

fn optional_crc32_f32_pair_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<Vec<(Crc32, f32)>>> {
    optional_pair_list_cell(table, row, column_name, |source| {
        Ok(source
            .parse::<u32>()
            .map_or_else(|_| Crc32::from_str_lower(source), Crc32::new))
    })
}

fn optional_u8_f32_pair_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
    parse_first: fn(&str) -> Result<u8>,
) -> Result<Option<Vec<(u8, f32)>>> {
    optional_pair_list_cell(table, row, column_name, parse_first)
}

fn optional_pair_list_cell<T>(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
    parse_first: impl Fn(&str) -> Result<T>,
) -> Result<Option<Vec<(T, f32)>>> {
    let source = match row_cell(table, row, column_name) {
        None | Some(DatasheetCellValue::Number(0.0))
        | Some(DatasheetCellValue::Boolean(false)) => return Ok(None),
        Some(DatasheetCellValue::String(value)) if value.trim().is_empty() => return Ok(None),
        Some(DatasheetCellValue::String(value)) => value,
        Some(_) => {
            bail!(
                "row {}:{} has non-pair-list {column_name}",
                row.source_path,
                row.row_index + 1
            )
        }
    };
    let values = split_designer_list(source)
        .into_iter()
        .map(|entry| {
            let (first, second) = entry.split_once('=').with_context(|| {
                format!(
                    "row {}:{} has invalid pair in {column_name}",
                    row.source_path,
                    row.row_index + 1
                )
            })?;
            Ok((
                parse_first(first.trim())?,
                parse_designer_number(second.trim(), row, column_name)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((!values.is_empty()).then_some(values))
}

fn f32_range_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<(f32, f32)> {
    match row_cell(table, row, column_name) {
        Some(DatasheetCellValue::Number(value)) if value.is_finite() => Ok((*value, *value)),
        Some(DatasheetCellValue::String(value)) => Ok(f32_range_from_text(value)),
        _ => bail!(
            "row {}:{} missing range {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn u32_range_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<(u32, u32)> {
    match row_cell(table, row, column_name) {
        Some(DatasheetCellValue::Number(value)) => {
            let endpoint = normalize_u32(*value)?;
            Ok((endpoint, endpoint))
        }
        Some(DatasheetCellValue::String(value)) => u32_range_from_text(value).with_context(|| {
            format!(
                "row {}:{} has invalid unsigned range {column_name}",
                row.source_path,
                row.row_index + 1
            )
        }),
        _ => bail!(
            "row {}:{} missing unsigned range {column_name}",
            row.source_path,
            row.row_index + 1
        ),
    }
}

fn split_designer_list(value: &str) -> Vec<&str> {
    value
        .split([',', '+'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn f32_range_from_text(value: &str) -> (f32, f32) {
    let parts = value.trim().split('-').map(str::trim).collect::<Vec<_>>();
    match parts.as_slice() {
        [value] => value
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map_or((0.0, 0.0), |value| (value, value)),
        [first, second] => match (first.parse::<f32>(), second.parse::<f32>()) {
            (Ok(first), Ok(second)) if first.is_finite() && second.is_finite() => {
                if first <= second {
                    (first, second)
                } else {
                    (second, first)
                }
            }
            _ => (0.0, 0.0),
        },
        _ => (0.0, 0.0),
    }
}

fn u32_range_from_text(value: &str) -> Result<(u32, u32)> {
    let parts = value.trim().split('-').map(str::trim).collect::<Vec<_>>();
    match parts.as_slice() {
        [value] if !value.is_empty() => {
            let endpoint = value.parse::<u32>()?;
            Ok((endpoint, endpoint))
        }
        [first, second] if !first.is_empty() && !second.is_empty() => {
            Ok((first.parse::<u32>()?, second.parse::<u32>()?))
        }
        _ => bail!("invalid u32 range"),
    }
}

fn parse_designer_number(part: &str, row: &DynamicTableRow, column_name: &str) -> Result<f32> {
    part.parse::<f32>().with_context(|| {
        format!(
            "row {}:{} has invalid number in {column_name}",
            row.source_path,
            row.row_index + 1
        )
    })
}

fn normalize_u32(value: f32) -> Result<u32> {
    if value.fract() != 0.0 || value < 0.0 || value > u32::MAX as f32 {
        bail!("expected u32, got {value}");
    }
    Ok(value as u32)
}

fn row_key_value(value: &nw_datasheet::CellValue<'_>) -> Option<String> {
    match value {
        nw_datasheet::CellValue::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        }
        nw_datasheet::CellValue::Number(value) => {
            if value.fract() == 0.0 {
                Some(format!("{value:.0}"))
            } else {
                Some(value.to_string())
            }
        }
        nw_datasheet::CellValue::Boolean(value) => Some(value.to_string()),
    }
}

fn owned_cell_value(value: &nw_datasheet::CellValue<'_>) -> DatasheetCellValue {
    match value {
        nw_datasheet::CellValue::String(value) => DatasheetCellValue::String((*value).to_owned()),
        nw_datasheet::CellValue::Number(value) => DatasheetCellValue::Number(*value),
        nw_datasheet::CellValue::Boolean(value) => DatasheetCellValue::Boolean(*value),
    }
}

"#;
