use anyhow::{Context, Result};
use nw_datasheet::ColumnType;
use std::collections::{BTreeMap, BTreeSet};

use crate::game_system_schema::GameSystemDataTablesSchemaReport;
use crate::manager::{
    ManagerCodegenOutput, ManagerEmissionContext, ManagerEmitter, NativeDuplicateKeyPolicy,
};
use crate::manager_records::{
    DirectManagerSurface, ItemDataManagerSurface, ManagerSurface, ManagerSurfaceDependency,
    SemanticLookupKind, SemanticManagerKey, SemanticManagerRecord, SemanticNumericKeyType,
    SemanticProjectionTransform, SemanticRecordField, SemanticRowFilterPredicate,
    manager_surface_dependencies, manager_surface_name, manager_surfaces_from_managers,
};
use crate::naming::{to_snake_ident, to_upper_camel_ident};
use crate::native::NativeCodegenFile;
use crate::target::GameDataTargetLanguage;

use super::format_rust_source;
use nw_serialize_codegen::rust_field_ident as serialize_rust_field_ident;

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
    let surfaces = manager_surfaces_from_managers(context.plan().managers())?;
    let records = rust_semantic_records(&surfaces);
    let mut runtime_source = String::from(
        r#"
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use nw_objectstream::{asset_reference, value, Element, ObjectStream};

use crate::assets::PakDatasheetSource;
use crate::table_manifest::{ColumnDescriptor, TableDescriptor, TABLES};

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
    rows_by_key: HashMap<String, usize>,
    rows_by_lookup_key: HashMap<String, usize>,
    duplicate_keys: HashMap<String, Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagerDependency {
    Table {
        name: &'static str,
        row: &'static str,
    },
    Asset {
        path: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ManagerDefinition {
    name: &'static str,
    dependencies: &'static [ManagerDependency],
}

"#,
    );
    push_rust_standalone_manager_definitions(&mut runtime_source, context, &surfaces);
    runtime_source.push_str(
        r#"
mod rows;
mod surfaces;

pub use rows::*;
pub use surfaces::*;

"#,
    );
    runtime_source.push_str(RUST_STANDALONE_PRODUCT_MANAGER_RUNTIME);
    runtime_source.push_str(RUST_STANDALONE_DYNAMIC_MANAGER_RUNTIME);

    let direct_schema_row_types = rust_direct_schema_row_types(&surfaces);
    let mut rows_source = String::from("use super::*;\n\n");
    push_rust_standalone_schema_rows(&mut rows_source, schema_report, &direct_schema_row_types);
    push_rust_semantic_record_types(&mut rows_source, &records);

    let mut surfaces_source = String::from("use super::*;\nuse super::rows::*;\n\n");
    push_rust_standalone_manager_surfaces(&mut surfaces_source, &surfaces, schema_report);

    Ok(vec![
        NativeCodegenFile::new(
            "src/managers/mod.rs",
            format_rust_source(&runtime_source)
                .context("format Rust standalone manager runtime")?,
        ),
        NativeCodegenFile::new(
            "src/managers/rows.rs",
            format_rust_source(&rows_source).context("format Rust standalone manager rows")?,
        ),
        NativeCodegenFile::new(
            "src/managers/surfaces.rs",
            format_rust_source(&surfaces_source)
                .context("format Rust standalone manager surfaces")?,
        ),
    ])
}

fn rust_semantic_records(surfaces: &[ManagerSurface]) -> Vec<SemanticManagerRecord> {
    surfaces
        .iter()
        .filter_map(|surface| match surface {
            ManagerSurface::Semantic(record) => Some(record.clone()),
            ManagerSurface::Direct(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::ProductBacked(_) => None,
        })
        .collect()
}

fn rust_direct_schema_row_types(surfaces: &[ManagerSurface]) -> BTreeSet<String> {
    let mut row_types = BTreeSet::new();
    for surface in surfaces {
        let ManagerSurface::Direct(manager) = surface else {
            continue;
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

fn push_rust_standalone_schema_rows(
    source: &mut String,
    schema_report: &GameSystemDataTablesSchemaReport,
    readable_row_types: &BTreeSet<String>,
) {
    for row in rust_standalone_schema_rows(schema_report) {
        if !readable_row_types.contains(&row.source_row_type) {
            continue;
        }
        if row.source_row_type == "LootBucketData" {
            push_rust_loot_bucket_schema_row(source);
            continue;
        }
        source.push_str(&format!("pub struct {} {{\n", row.type_name));
        for field in &row.fields {
            source.push_str(&format!(
                "    pub {}: {},\n",
                field.field_name,
                rust_standalone_schema_field_type(field.column_type, field.required)
            ));
        }
        source.push_str("}\n\n");
        source.push_str(&format!(
            "pub(super) fn {}(table: &DynamicTable, row: &DynamicTableRow) -> Result<{}> {{\n",
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
}

fn push_rust_loot_bucket_schema_row(source: &mut String) {
    source.push_str(
        r#"
pub struct LootBucketDataSchemaRow {
    pub row_placeholders: String,
    pub entries: Vec<LootBucketDataEntry>,
    pub loot_biasing_disabled: Vec<LootBucketBiasingDisabled>,
}

pub struct LootBucketDataEntry {
    pub slot: u16,
    pub loot_bucket: Option<String>,
    pub tags: Option<String>,
    pub match_one: Option<String>,
    pub item: Option<String>,
    pub quantity: Option<String>,
    pub odds: Option<String>,
}

pub struct LootBucketBiasingDisabled {
    pub slot: u16,
    pub disabled: bool,
}

pub(super) fn read_loot_bucket_data(
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
            entries.push(LootBucketDataEntry {
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

#[cfg(test)]
mod tests {
    use super::*;

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

fn push_rust_standalone_manager_definitions(
    source: &mut String,
    context: ManagerEmissionContext<'_>,
    surfaces: &[ManagerSurface],
) {
    let contracts = context.plan().contracts();
    let managers = surfaces
        .iter()
        .filter_map(|surface| {
            let manager_name = manager_surface_name(surface);
            let contract = contracts.iter().find(|contract| {
                semantic_manager_type_name(contract.manager().as_str()) == manager_name
            })?;
            Some((
                manager_name,
                manager_surface_dependencies(surface, contract.inputs()),
            ))
        })
        .collect::<Vec<_>>();
    for (index, (_, dependencies)) in managers.iter().enumerate() {
        source.push_str(&format!(
            "const MANAGER_{index:03}_DEPENDENCIES: &[ManagerDependency] = &[\n"
        ));
        for input in dependencies {
            source.push_str("    ");
            source.push_str(&rust_standalone_manager_dependency(input));
            source.push_str(",\n");
        }
        source.push_str("];\n");
    }
    source.push_str("\nconst MANAGERS: &[ManagerDefinition] = &[\n");
    for (index, (manager_name, _)) in managers.iter().enumerate() {
        source.push_str("    ManagerDefinition {\n");
        source.push_str(&format!(
            "        name: {},\n",
            rust_string_literal(manager_name)
        ));
        source.push_str(&format!(
            "        dependencies: MANAGER_{index:03}_DEPENDENCIES,\n"
        ));
        source.push_str("    },\n");
    }
    source.push_str("];\n\n");
}

fn push_rust_standalone_manager_surfaces(
    source: &mut String,
    surfaces: &[ManagerSurface],
    schema_report: &GameSystemDataTablesSchemaReport,
) {
    for surface in surfaces {
        match surface {
            ManagerSurface::Direct(manager) => {
                push_rust_direct_manager_wrapper(source, manager, schema_report);
            }
            ManagerSurface::Semantic(record) => push_rust_semantic_manager_wrapper(source, record),
            ManagerSurface::ItemData(manager) => {
                push_rust_item_data_manager_wrapper(source, manager)
            }
            ManagerSurface::ProductBacked(manager) => {
                push_rust_product_backed_manager_wrapper(source, manager)
            }
        }
    }
}

fn push_rust_direct_manager_wrapper(
    source: &mut String,
    manager: &DirectManagerSurface,
    schema_report: &GameSystemDataTablesSchemaReport,
) {
    let manager_name = &manager.manager_class_name;
    let factory = to_snake_ident(manager_name, "manager");
    let mut product_methods = rust_direct_product_methods(manager);
    product_methods.push_str(rust_standalone_special_manager_extra_methods(manager_name));
    let row_methods = rust_direct_schema_methods(manager, schema_report);
    source.push_str(&format!(
        r#"
#[derive(Debug, Clone)]
pub struct {manager_name} {{
    instance: Arc<ManagerInstance>,
}}

impl {manager_name} {{
    pub fn from_runtime(runtime: &mut ManagerRuntime) -> Result<Self> {{
        Ok(Self {{
            instance: runtime.manager({manager_name:?})?,
        }})
    }}

    fn from_instance(instance: Arc<ManagerInstance>) -> Self {{
        Self {{ instance }}
    }}

{row_methods}
{product_methods}
}}

pub fn {factory}(runtime: &mut ManagerRuntime) -> Result<{manager_name}> {{
    {manager_name}::from_runtime(runtime)
}}
"#
    ));
}

fn push_rust_product_backed_manager_wrapper(source: &mut String, manager: &DirectManagerSurface) {
    let manager_name = &manager.manager_class_name;
    let factory = to_snake_ident(manager_name, "manager");
    let mut product_methods = rust_direct_product_methods(manager);
    product_methods.push_str(rust_standalone_special_manager_extra_methods(manager_name));
    source.push_str(&format!(
        r#"
#[derive(Debug, Clone)]
pub struct {manager_name} {{
    instance: Arc<ManagerInstance>,
}}

impl {manager_name} {{
    pub fn from_runtime(runtime: &mut ManagerRuntime) -> Result<Self> {{
        Ok(Self {{
            instance: runtime.manager({manager_name:?})?,
        }})
    }}

{product_methods}
}}

pub fn {factory}(runtime: &mut ManagerRuntime) -> Result<{manager_name}> {{
    {manager_name}::from_runtime(runtime)
}}
"#
    ));
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
    let table_list = manager
        .tables
        .iter()
        .map(|table| format!("    {table_type}::{},\n", table.variant_name))
        .collect::<String>();
    let manager_name_literal = rust_string_literal(&manager.manager_name);

    source.push_str(&format!(
        r#"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum {table_type} {{
{table_variants}}}

impl {table_type} {{
    #[must_use]
    pub const fn table_name(self) -> &'static str {{
        match self {{
{table_name_arms}        }}
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
    pub const fn table_name(self) -> &'static str {{
        self.table.table_name()
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
    item_id: String,
    item_id_crc: u32,
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
    pub const fn item_id_crc(&self) -> u32 {{
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
    instance: Arc<ManagerInstance>,
    items: Arc<Vec<{data_type}>>,
    items_by_id: Arc<HashMap<u32, usize>>,
}}

impl {manager_name} {{
    pub fn from_runtime(runtime: &mut ManagerRuntime) -> Result<Self> {{
        Self::from_instance(runtime.manager({manager_name_literal})?)
    }}

    fn from_instance(instance: Arc<ManagerInstance>) -> Result<Self> {{
        let items = materialize_{factory}(&instance)?;
        let mut items_by_id = HashMap::new();
        for (index, item) in items.iter().enumerate() {{
            items_by_id.insert(item.item_id_crc, index);
        }}
        Ok(Self {{
            instance,
            items: Arc::new(items),
            items_by_id: Arc::new(items_by_id),
        }})
    }}

    #[must_use]
    pub fn get(&self, item_id: impl AsRef<str>) -> Option<&{data_type}> {{
        self.get_from_id(crc32_lowercase(item_id.as_ref()))
    }}

    #[must_use]
    pub fn get_from_id(&self, item_id: u32) -> Option<&{data_type}> {{
        self.items.get(*self.items_by_id.get(&item_id)?)
    }}

    #[must_use]
    pub fn by_index(&self, index: std::num::NonZeroU32) -> Option<&{data_type}> {{
        let zero_based = usize::try_from(index.get() - 1).ok()?;
        self.items.get(zero_based)
    }}

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &{data_type}> + '_ {{
        self.items.iter()
    }}

    #[must_use]
    pub fn items(&self) -> &[{data_type}] {{
        &self.items
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

pub fn {factory}(runtime: &mut ManagerRuntime) -> Result<{manager_name}> {{
    {manager_name}::from_runtime(runtime)
}}

fn materialize_{factory}(instance: &ManagerInstance) -> Result<Vec<{data_type}>> {{
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for table_id in ITEM_DATA_MANAGER_TABLES {{
        let table = instance.table(table_id.table_name()).with_context(|| {{
            format!(
                "manager {{}} table {{}} was not loaded",
                instance.definition.name,
                table_id.table_name()
            )
        }})?;
        cache_item_data_rows(&mut items, &mut seen, *table_id, table)?;
    }}
    Ok(items)
}}

fn cache_item_data_rows(
    items: &mut Vec<{data_type}>,
    seen: &mut HashSet<u32>,
    table_id: {table_type},
    table: &DynamicTable,
) -> Result<()> {{
    for source_row in &table.rows {{
        let item_id = required_string_cell(table, source_row, "ItemID")?.trim();
        if item_id.is_empty() {{
            continue;
        }}
        let item_id_crc = crc32_lowercase(item_id);
        if item_id_crc == 0 || !seen.insert(item_id_crc) {{
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
            item_id: item_id.to_owned(),
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
) -> String {
    let row_specs = rust_standalone_schema_rows(schema_report);
    let mut seen = BTreeSet::new();
    let row_types = manager
        .tables
        .iter()
        .filter_map(|table| {
            seen.insert(table.row_type_name.clone())
                .then_some(table.row_type_name.clone())
        })
        .collect::<Vec<_>>();
    if row_types.is_empty() {
        return String::new();
    }

    let single_row_type = row_types.len() == 1;
    let mut source = String::new();
    for source_row_type in row_types {
        let Some(row_spec) = row_specs
            .iter()
            .find(|row| row.source_row_type == source_row_type)
        else {
            continue;
        };
        let row_type = &row_spec.type_name;
        let rows_method = if single_row_type {
            "rows".to_owned()
        } else {
            format!("{}_rows", to_snake_ident(&source_row_type, "rows"))
        };
        source.push_str(&format!(
            r#"    pub fn {rows_method}(&self) -> Result<Vec<{row_type}>> {{
        self.instance.schema_rows({source_row_type:?}, {reader})
    }}

"#,
            reader = rust_standalone_schema_reader_name(&source_row_type),
        ));
        if let Some(key_field) = row_spec.fields.iter().find(|field| field.row_key) {
            let lookup_method = if single_row_type {
                "get".to_owned()
            } else {
                to_snake_ident(&source_row_type, "row")
            };
            source.push_str(&format!(
                r#"    pub fn {lookup_method}(&self, key: impl ToString) -> Result<Option<{row_type}>> {{
        self.instance.schema_row({source_row_type:?}, key, {reader}, |row| row.{key_field}.to_string())
    }}

"#,
                reader = rust_standalone_schema_reader_name(&source_row_type),
                key_field = key_field.field_name,
            ));
        }
    }
    source
}

fn rust_direct_product_methods(manager: &DirectManagerSurface) -> String {
    let mut source = String::new();
    for product in &manager.products {
        let path = rust_string_literal(&product.path);
        let getter = to_snake_ident(&product.manager_getter, "asset");
        match product.value_type.as_str() {
            "newworld_plugin::assets::armor_offset_database::ArmorOffsetDatabase" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<ArmorOffsetDatabase> {{
        parse_armor_offset_database(self.instance.required_asset_bytes({path})?)
    }}

    pub fn armor_offset(&self, name: &str) -> Result<Option<ArmorOffsetData>> {{
        Ok(armor_offset_by_name(&self.{getter}()?, name).cloned())
    }}

    pub fn furthest_attachment_offset(
        &self,
        armor_offset_names: &[String],
        attachment_name: &str,
        current_position: Vec3,
    ) -> Result<Option<AttachmentOffsetData>> {{
        Ok(furthest_armor_attachment_offset(
            &self.{getter}()?,
            armor_offset_names,
            attachment_name,
            current_position,
        )
        .cloned())
    }}

"#
                ));
            }
            "newworld_plugin::assets::equip_types_database::EquipTypesDatabase" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<EquipTypesDatabase> {{
        parse_equip_types_database(self.instance.required_asset_bytes({path})?)
    }}

    pub fn equip_types(&self) -> Result<Vec<EquipTypeData>> {{
        Ok(self.{getter}()?.equip_types)
    }}

"#
                ));
            }
            "newworld_plugin::assets::game_debug_settings::GameDebugSettings" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<GameDebugSettings> {{
        parse_game_debug_settings(self.instance.required_asset_bytes({path})?)
    }}

    pub fn combat(&self) -> Result<CombatDebugSettings> {{
        Ok(self.{getter}()?.combat_settings)
    }}

    pub fn disabled_combat_toggle_count(&self) -> Result<usize> {{
        Ok(disabled_combat_toggle_count(&self.combat()?))
    }}

"#
                ));
            }
            "newworld_plugin::assets::player_base_attributes::PlayerBaseAttributes" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<PlayerBaseAttributes> {{
        parse_player_base_attributes(self.instance.required_asset_bytes({path})?)
    }}

    pub fn player_attribute_data(&self) -> Result<PlayerAttributeData> {{
        Ok(self.{getter}()?.player_attribute_data)
    }}

    pub fn max_perks(&self, rarity_level: usize) -> Result<Option<i32>> {{
        Ok(self
            .player_attribute_data()?
            .item_rarity_data
            .get(rarity_level)
            .map(|rarity| rarity.max_perk_count))
    }}

"#
                ));
            }
            "newworld_plugin::assets::settlement_progression_data::SettlementProgressionData" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<SettlementProgressionData> {{
        parse_settlement_progression_data(self.instance.required_asset_bytes({path})?)
    }}

    pub fn settlement_progression_categories(&self) -> Result<Vec<ProgressionCategoryEntry>> {{
        Ok(self.{getter}()?.settlement_progression_categories)
    }}

"#
                ));
            }
            "newworld_plugin::assets::ui_database::UiDatabase" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<UiDatabase> {{
        parse_ui_database(self.instance.required_asset_bytes({path})?)
    }}

    pub fn interact_options(&self) -> Result<Vec<InteractOptionData>> {{
        Ok(self.{getter}()?.unified_interact_data.interact_options)
    }}

    pub fn interact_option(&self, id: impl ToString) -> Result<Option<InteractOptionData>> {{
        let key = crc32_lowercase(&id.to_string());
        Ok(interact_option_by_crc(&self.interact_options()?, key).cloned())
    }}

    pub fn interact_options_by_category(&self, category: i32) -> Result<Vec<InteractOptionData>> {{
        Ok(interact_options_by_category(&self.interact_options()?, category))
    }}

"#
                ));
            }
            "newworld_plugin::assets::camera_settings::GameCameraSettings" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<GameCameraSettings> {{
        parse_game_camera_settings(self.instance.required_asset_bytes({path})?)
    }}

    pub fn camera_states(&self) -> Result<Vec<CameraStateSettings>> {{
        Ok(self.{getter}()?.camera_states)
    }}

"#
                ));
            }
            "newworld_plugin::assets::gathering_database::GatheringDatabase" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<GatheringDatabase> {{
        parse_gathering_database(self.instance.required_asset_bytes({path})?)
    }}

    pub fn gathering_data(&self) -> Result<GatheringData> {{
        Ok(self.{getter}()?.gathering_data)
    }}

    pub fn gathering_types(&self) -> Result<Vec<GatheringTypeData>> {{
        Ok(self.gathering_data()?.gathering_types)
    }}

    pub fn gathering_actions(&self) -> Result<Vec<GatheringAction>> {{
        Ok(self.gathering_data()?.gathering_actions)
    }}

"#
                ));
            }
            "newworld_plugin::assets::gathering_database::GatheringActionDatabase" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<GatheringActionDatabase> {{
        parse_gathering_action_database(self.instance.required_asset_bytes({path})?)
    }}

    pub fn gathering_action_data(&self) -> Result<Vec<GatheringActionData>> {{
        Ok(self.{getter}()?.gathering_actions)
    }}

"#
                ));
            }
            "newworld_plugin::assets::crafting_station_database::CraftingStationDatabase" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<CraftingStationDatabase> {{
        parse_crafting_station_database(self.instance.required_asset_bytes({path})?)
    }}

    pub fn crafting_stations(&self) -> Result<Vec<CraftingStationData>> {{
        Ok(self.{getter}()?.crafting_stations)
    }}

"#
                ));
            }
            "newworld_plugin::assets::rank_database::SocialRankDatabase" => {
                source.push_str(&format!(
                    r#"    pub fn {getter}(&self) -> Result<SocialRankDatabase> {{
        parse_social_rank_database(self.instance.required_asset_bytes({path})?)
    }}

    pub fn ranks(&self) -> Result<Vec<SocialRankData>> {{
        Ok(self.{getter}()?.ranks)
    }}

"#
                ));
            }
            _ => {}
        }
    }
    source
}

fn push_rust_semantic_record_types(source: &mut String, records: &[SemanticManagerRecord]) {
    for record in records {
        source.push_str("#[derive(Debug, Clone)]\n");
        source.push_str(&format!("pub struct {} {{\n", record.record_type_name));
        for (field_name, field_type) in rust_semantic_record_fields(record) {
            source.push_str(&format!("    pub {field_name}: {field_type},\n"));
        }
        source.push_str("}\n\n");
    }
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
                push_rust_semantic_record_field(&mut fields, &mut seen, crc_field, "u32");
            }
            SemanticManagerKey::FallbackCrc {
                key_kind_field,
                key_field,
                crc_field,
                ..
            } => {
                push_rust_semantic_record_field(&mut fields, &mut seen, key_kind_field, "String");
                push_rust_semantic_record_field(&mut fields, &mut seen, key_field, "String");
                push_rust_semantic_record_field(&mut fields, &mut seen, crc_field, "u32");
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
            rust_projection_type(field.transform),
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
    let key_map_type = rust_key_map_type(record);
    let lookup_methods = rust_semantic_lookup_methods(record);
    let source_row_method = rust_semantic_source_row_method(record);
    let ids_method = rust_semantic_ids_method(record);
    let rows_method = rust_semantic_rows_method(record);
    let len_method = rust_semantic_len_method(record);
    let is_empty_method = rust_semantic_is_empty_method(record);
    let special_methods = rust_standalone_special_manager_extra_methods(manager_name);
    let key_index_insert = rust_semantic_key_index_insert(record);
    let source_row_index_insert = rust_semantic_source_row_index_insert(record);

    source.push_str(&format!(
        r#"
#[derive(Debug, Clone)]
pub struct {manager_name} {{
    instance: Arc<ManagerInstance>,
    entries: Arc<Vec<{record_type}>>,
    entries_by_key: Arc<HashMap<{key_map_type}, usize>>,
    entries_by_source_row: Arc<HashMap<u32, usize>>,
}}

impl {manager_name} {{
    pub fn from_runtime(runtime: &mut ManagerRuntime) -> Result<Self> {{
        Self::from_instance(runtime.manager({manager_name:?})?)
    }}

    fn from_instance(instance: Arc<ManagerInstance>) -> Result<Self> {{
        let entries = materialize_{factory}(&instance)?;
        let mut entries_by_key = HashMap::new();
        let mut entries_by_source_row = HashMap::new();
        for (index, row) in entries.iter().enumerate() {{
{key_index_insert}{source_row_index_insert}        }}
        Ok(Self {{
            instance,
            entries: Arc::new(entries),
            entries_by_key: Arc::new(entries_by_key),
            entries_by_source_row: Arc::new(entries_by_source_row),
        }})
    }}

{lookup_methods}{source_row_method}{ids_method}{rows_method}{len_method}{is_empty_method}{special_methods}
}}

pub fn {factory}(runtime: &mut ManagerRuntime) -> Result<{manager_name}> {{
    {manager_name}::from_runtime(runtime)
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

fn rust_projection_type(transform: SemanticProjectionTransform) -> &'static str {
    match transform {
        SemanticProjectionTransform::String
        | SemanticProjectionTransform::StringDefaultEmpty
        | SemanticProjectionTransform::PlusJoinedList => "String",
        SemanticProjectionTransform::OptionalString => "Option<String>",
        SemanticProjectionTransform::StringList
        | SemanticProjectionTransform::NonEmptyStringList => "Vec<String>",
        SemanticProjectionTransform::OptionalStringList => "Option<Vec<String>>",
        SemanticProjectionTransform::Bool => "bool",
        SemanticProjectionTransform::OptionalBool => "Option<bool>",
        SemanticProjectionTransform::U8 => "u8",
        SemanticProjectionTransform::U16 => "u16",
        SemanticProjectionTransform::U32
        | SemanticProjectionTransform::Crc32
        | SemanticProjectionTransform::RowIndex => "u32",
        SemanticProjectionTransform::OptionalU32
        | SemanticProjectionTransform::OptionalLowercaseCrcString
        | SemanticProjectionTransform::OptionalRowIndex => "Option<u32>",
        SemanticProjectionTransform::I32 => "i32",
        SemanticProjectionTransform::F32 => "f32",
        SemanticProjectionTransform::OptionalF32 => "Option<f32>",
        SemanticProjectionTransform::F32List => "Vec<f32>",
        SemanticProjectionTransform::I32List => "Vec<i32>",
        SemanticProjectionTransform::Crc32List
        | SemanticProjectionTransform::LowercaseCrcStringList
        | SemanticProjectionTransform::RowIndexList => "Vec<u32>",
        SemanticProjectionTransform::F32RangeInclusive => "(f32, f32)",
        SemanticProjectionTransform::U32RangeInclusive => "(u32, u32)",
    }
}

fn rust_key_map_type(record: &SemanticManagerRecord) -> &'static str {
    match record.key {
        Some(SemanticManagerKey::String { .. } | SemanticManagerKey::EnumString { .. }) => "String",
        Some(_) | None => "u32",
    }
}

fn rust_semantic_lookup_methods(record: &SemanticManagerRecord) -> String {
    let mut source = String::new();
    let record_type = &record.record_type_name;
    for method in &record.lookup_methods {
        let method_name = to_snake_ident(&method.name, "method");
        let parameter_name = to_snake_ident(&method.parameter, "key");
        match method.kind {
            SemanticLookupKind::CrcStringKey => source.push_str(&format!(
                r#"    pub fn {method_name}(&self, {parameter_name}: impl AsRef<str>) -> Option<&{record_type}> {{
        let key = crc32_lowercase({parameter_name}.as_ref());
        self.entries_by_key.get(&key).map(|index| &self.entries[*index])
    }}

"#
            )),
            SemanticLookupKind::CrcKey => source.push_str(&format!(
                r#"    pub fn {method_name}(&self, {parameter_name}: u32) -> Option<&{record_type}> {{
        self.entries_by_key.get(&{parameter_name}).map(|index| &self.entries[*index])
    }}

"#
            )),
            SemanticLookupKind::NumericKey(key_type) => {
                let parameter_type = rust_numeric_key_type(key_type);
                source.push_str(&format!(
                    r#"    pub fn {method_name}(&self, {parameter_name}: {parameter_type}) -> Option<&{record_type}> {{
        let key = {parameter_name} as u32;
        self.entries_by_key.get(&key).map(|index| &self.entries[*index])
    }}

"#
                ));
            }
            SemanticLookupKind::StringKey => source.push_str(&format!(
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
        r#"    pub fn {method_name}(&self) -> Vec<{id_type}> {{
        self.entries.iter().map(|row| {id_expr}).collect()
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
    format!(
        r#"    pub fn {method_name}(&self) -> &[{record_type}] {{
        self.entries.as_slice()
    }}

"#
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
        _ => "u32",
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
        SemanticManagerKey::Numeric { key_field, .. } => {
            format!("row.{} as u32", rust_semantic_field_name(key_field))
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
        r#"fn materialize_{manager_factory}(instance: &ManagerInstance) -> Result<Vec<{record_type}>> {{
    let mut rows = Vec::new();
"#
    ));
    if record.key.is_some() {
        source.push_str("    let mut seen = HashSet::new();\n");
    }
    source.push_str(&format!(
        "    for table_name in &[{}] {{\n",
        record
            .tables
            .iter()
            .map(|table| rust_string_literal(&table.table_name))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    source.push_str(&format!(
        r#"        let table = instance
            .table(table_name)
            .with_context(|| format!("manager {} missing table {{table_name}}"))?;
        for source_row in &table.rows {{
"#,
        record.manager_name
    ));
    source.push_str(&rust_semantic_key_materializer(record));
    source.push_str(&rust_semantic_row_filters(record));
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
            rust_projection_value(field)
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
            let mut source = format!(
                "            let key_text = required_string_cell(table, source_row, {column})?;\n"
            );
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
            source.push_str("            let key_crc = crc32_lowercase(&key_value);\n");
            if *reject_zero_crc {
                source.push_str(
                    r#"            if key_crc == 0 {
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
                r#"            let key_crc = crc32_lowercase(&key_value);
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
            let mut source = format!(
                "            let key_text = required_string_cell(table, source_row, {column})?;\n"
            );
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
            let mut source = format!(
                "            let key_value = required_string_cell(table, source_row, {column})?.to_owned();\n"
            );
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
                r#"            if crc32_lowercase(required_string_cell(table, source_row, {column})?) == 0 {{
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
        } => format!(
            "                {}: key_value,\n                {}: key_crc,\n",
            rust_semantic_field_name(key_field),
            rust_semantic_field_name(crc_field)
        ),
        SemanticManagerKey::FallbackCrc {
            key_kind_field,
            key_field,
            crc_field,
            ..
        } => format!(
            "                {}: key_kind,\n                {}: key_value,\n                {}: key_crc,\n",
            rust_semantic_field_name(key_kind_field),
            rust_semantic_field_name(key_field),
            rust_semantic_field_name(crc_field)
        ),
        SemanticManagerKey::Numeric { key_field, .. }
        | SemanticManagerKey::EnumString { key_field, .. }
        | SemanticManagerKey::String { key_field, .. } => {
            format!(
                "                {}: key_value,\n",
                rust_semantic_field_name(key_field)
            )
        }
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
            format!("required_string_cell(table, source_row, {column})?.to_owned()")
        }
        SemanticProjectionTransform::StringDefaultEmpty => {
            format!("optional_string_cell(table, source_row, {column})?.unwrap_or(\"\").to_owned()")
        }
        SemanticProjectionTransform::PlusJoinedList => {
            format!("string_list_cell(table, source_row, {column})?.join(\"+\")")
        }
        SemanticProjectionTransform::OptionalString => {
            format!("optional_string_cell(table, source_row, {column})?.map(str::to_owned)")
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
        SemanticProjectionTransform::U8 => {
            format!("required_u8_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::U16 => {
            format!("required_u16_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::U32 => {
            format!("required_u32_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalU32 => {
            format!("optional_u32_cell(table, source_row, {column})?")
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
        SemanticProjectionTransform::F32List => {
            format!("f32_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::I32List => {
            format!("i32_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::Crc32 => {
            format!("required_crc32_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::Crc32List => {
            format!("crc32_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalLowercaseCrcString => {
            format!("optional_lowercase_crc_string_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::LowercaseCrcStringList => {
            format!("lowercase_crc_string_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::RowIndex => {
            format!("required_u32_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::OptionalRowIndex => {
            format!("optional_u32_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::RowIndexList => {
            format!("u32_list_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::F32RangeInclusive => {
            format!("f32_range_cell(table, source_row, {column})?")
        }
        SemanticProjectionTransform::U32RangeInclusive => {
            format!("u32_range_cell(table, source_row, {column})?")
        }
    }
}

fn rust_standalone_special_manager_extra_methods(manager_name: &str) -> &'static str {
    match manager_name {
        "PlayerDataManager" => {
            r#"    pub fn categorical_progression_id(&self, tradeskill: impl ToString) -> Option<u32> {
        let normalized = tradeskill.to_string();
        if normalized == "None" || normalized == "WildernessSurvival" {
            return None;
        }
        Some(crc32_lowercase(&normalized))
    }
"#
        }
        _ => "",
    }
}

fn rust_standalone_manager_dependency(input: &ManagerSurfaceDependency) -> String {
    match input {
        ManagerSurfaceDependency::Table { name, row } => format!(
            "ManagerDependency::Table {{ name: {}, row: {} }}",
            rust_string_literal(name),
            rust_string_literal(row)
        ),
        ManagerSurfaceDependency::Asset { path } => format!(
            "ManagerDependency::Asset {{ path: {} }}",
            rust_string_literal(path)
        ),
    }
}

fn semantic_manager_type_name(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

fn rust_string_literal(value: &str) -> String {
    format!("{value:?}")
}

const RUST_STANDALONE_PRODUCT_MANAGER_RUNTIME: &str = r#"
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    #[must_use]
    pub fn length(self) -> f32 {
        self.x.hypot(self.y).hypot(self.z)
    }
}

pub const ZERO_VEC3: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
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
pub struct AssetReference {
    pub guid: String,
    pub sub_id: u32,
    pub asset_type: String,
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleAssetReferenceTextureAsset {
    pub asset_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditCrc {
    pub value_str: String,
    pub value_crc: u32,
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
    pub crafting_result_loot_bucket_id: u32,
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
    pub attribute_perk_bucket_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildSiegeWindowRegionData {
    pub start_hour: u32,
    pub end_hour: u32,
    pub utc_offset: i32,
    pub dst_rule_id: u32,
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
    pub deployable_limits: HashMap<u32, WarDeployableLimitData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarDeployableLimitData {
    pub id: u32,
    pub display_name: String,
    pub buildable_names: Vec<String>,
    pub buildable_ids: Vec<u32>,
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

pub fn parse_armor_offset_database(bytes: &[u8]) -> Result<ArmorOffsetDatabase> {
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

pub fn armor_offset_by_name<'a>(
    database: &'a ArmorOffsetDatabase,
    name: &str,
) -> Option<&'a ArmorOffsetData> {
    database.offsets.iter().find(|offset| offset.name == name)
}

pub fn furthest_armor_attachment_offset<'a>(
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

pub fn parse_equip_types_database(bytes: &[u8]) -> Result<EquipTypesDatabase> {
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

pub fn parse_game_debug_settings(bytes: &[u8]) -> Result<GameDebugSettings> {
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

pub fn disabled_combat_toggle_count(combat: &CombatDebugSettings) -> usize {
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

pub fn parse_ui_database(bytes: &[u8]) -> Result<UiDatabase> {
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

pub fn interact_option_by_crc(
    options: &[InteractOptionData],
    key: u32,
) -> Option<&InteractOptionData> {
    options
        .iter()
        .find(|option| crc32_lowercase(&option.name) == key)
}

pub fn interact_options_by_category(
    options: &[InteractOptionData],
    category: i32,
) -> Vec<InteractOptionData> {
    options
        .iter()
        .filter(|option| {
            option.interact_option_category == category
                || option.interact_option_category == ALL_INTERACT_OPTIONS_CATEGORY
        })
        .cloned()
        .collect()
}

pub fn parse_game_camera_settings(bytes: &[u8]) -> Result<GameCameraSettings> {
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

pub fn parse_player_base_attributes(bytes: &[u8]) -> Result<PlayerBaseAttributes> {
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

pub fn parse_settlement_progression_data(bytes: &[u8]) -> Result<SettlementProgressionData> {
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

pub fn parse_gathering_database(bytes: &[u8]) -> Result<GatheringDatabase> {
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

pub fn parse_gathering_action_database(bytes: &[u8]) -> Result<GatheringActionDatabase> {
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

pub fn parse_crafting_station_database(bytes: &[u8]) -> Result<CraftingStationDatabase> {
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

pub fn parse_social_rank_database(bytes: &[u8]) -> Result<SocialRankDatabase> {
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

fn required_crc32_field_by_name(element: &Element, field_name: &str) -> Result<u32> {
    read_crc32(required_field_by_name(element, field_name)?)
}

fn required_string_sequence_by_name(element: &Element, field_name: &str) -> Result<Vec<String>> {
    read_string_vector(required_field_by_name(element, field_name)?)
}

fn required_crc32_sequence_by_name(element: &Element, field_name: &str) -> Result<Vec<u32>> {
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
    Ok(Vec3 { x, y, z })
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

fn read_crc32(element: &Element) -> Result<u32> {
    Ok(value::read_crc32(element)?)
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
    let asset = asset_reference::read_asset_value(element)?;
    Ok(AssetReference {
        guid: asset.guid().to_string(),
        sub_id: asset.sub_id(),
        asset_type: asset.asset_type().to_string(),
        hint: asset.hint().to_owned(),
    })
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

fn first_non_empty<'a, const N: usize>(values: [Option<&'a String>; N]) -> Option<&'a str> {
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
    let mut crc = 0xffff_ffffu32;
    for byte in value.bytes() {
        let current = if byte.is_ascii_uppercase() { byte + 32 } else { byte } as u32;
        crc ^= current;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { 0xedb8_8320 ^ (crc >> 1) } else { crc >> 1 };
        }
    }
    crc ^ 0xffff_ffff
}

"#;

const RUST_STANDALONE_DYNAMIC_MANAGER_RUNTIME: &str = r#"
#[derive(Debug, Clone)]
struct ManagerInstance {
    definition: &'static ManagerDefinition,
    tables: HashMap<String, Arc<DynamicTable>>,
    assets: HashMap<String, Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct ManagerRuntime {
    datasheets_by_path: HashMap<String, crate::assets::DatasheetAsset>,
    assets_by_path: HashMap<String, Vec<u8>>,
    table_cache: HashMap<String, Arc<DynamicTable>>,
    manager_cache: HashMap<&'static str, Arc<ManagerInstance>>,
}

impl ManagerRuntime {
    #[must_use]
    pub fn from_pak_source(source: PakDatasheetSource) -> Self {
        let datasheets_by_path = source
            .datasheets
            .into_iter()
            .map(|asset| (normalize_data_path(&asset.path), asset))
            .collect();
        let assets_by_path = source
            .assets
            .into_iter()
            .map(|asset| (normalize_data_path(&asset.path), asset.bytes))
            .collect();
        Self {
            datasheets_by_path,
            assets_by_path,
            table_cache: HashMap::new(),
            manager_cache: HashMap::new(),
        }
    }

    fn manager(&mut self, name: &str) -> Result<Arc<ManagerInstance>> {
        let definition = manager_by_name(name).with_context(|| format!("unknown manager {name}"))?;
        self.build_manager(definition, &mut HashSet::new())
    }

    fn table(&mut self, name_or_source_path: &str) -> Result<Option<Arc<DynamicTable>>> {
        let Some(schema) =
            table_schema_by_name(name_or_source_path).or_else(|| table_schema_by_source_path(name_or_source_path))
        else {
            return Ok(None);
        };
        self.build_table(schema).map(Some)
    }

    fn build_manager(
        &mut self,
        definition: &'static ManagerDefinition,
        stack: &mut HashSet<&'static str>,
    ) -> Result<Arc<ManagerInstance>> {
        if let Some(cached) = self.manager_cache.get(definition.name) {
            return Ok(cached.clone());
        }
        if !stack.insert(definition.name) {
            bail!("manager dependency cycle at {}", definition.name);
        }

        let mut tables = HashMap::new();
        let mut assets = HashMap::new();
        for dependency in definition.dependencies {
            match dependency {
                ManagerDependency::Table { name, row } => {
                    let schema = table_schema_by_name_and_row(name, row).with_context(|| {
                        format!(
                            "manager {} depends on unknown table {name}/{row}",
                            definition.name
                        )
                    })?;
                    let table = self.build_table(schema)?;
                    tables.insert((*name).to_owned(), table.clone());
                    tables.insert(schema.name.to_owned(), table.clone());
                    tables.insert(format!("{}:{}", schema.name, schema.row_type), table);
                }
                ManagerDependency::Asset { path } => {
                    assets.insert(normalize_data_path(path), self.required_asset_bytes(path)?.to_vec());
                }
            }
        }

        stack.remove(definition.name);
        let instance = Arc::new(ManagerInstance {
            definition,
            tables,
            assets,
        });
        self.manager_cache
            .insert(definition.name, instance.clone());
        Ok(instance)
    }

    fn build_table(&mut self, schema: &'static TableDescriptor) -> Result<Arc<DynamicTable>> {
        let cache_key = format!("{}:{}", schema.name, schema.row_type);
        if let Some(cached) = self.table_cache.get(&cache_key) {
            return Ok(cached.clone());
        }
        let row_key_column = schema
            .columns
            .iter()
            .find(|column| column.row_key)
            .with_context(|| format!("table {} has no row-key column", schema.name))?;

        let mut rows = Vec::new();
        let mut rows_by_key = HashMap::new();
        let mut rows_by_lookup_key = HashMap::new();
        let mut duplicate_keys: HashMap<String, Vec<usize>> = HashMap::new();
        for source_path in &schema.sources {
            let asset = self
                .datasheet_asset(source_path)
                .with_context(|| format!("datasheet source {source_path} was not loaded"))?;
            let sheet = nw_datasheet::Datasheet::parse(&asset.bytes)
                .with_context(|| format!("parse datasheet {}", asset.path))?;
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
                    source_path: asset.path.clone(),
                    row_index,
                    key: key.clone(),
                    cells,
                    column_slots: column_slots.clone(),
                };
                let row_slot = rows.len();
                rows.push(dynamic_row);
                rows_by_key.entry(key.clone()).or_insert(row_slot);
                let lookup_key = normalize_lookup_key(&key);
                if let Some(existing) = rows_by_lookup_key.get(&lookup_key).copied() {
                    duplicate_keys
                        .entry(lookup_key)
                        .or_insert_with(|| vec![existing])
                        .push(row_slot);
                } else {
                    rows_by_lookup_key.insert(lookup_key, row_slot);
                }
            }
        }

        let table = Arc::new(DynamicTable {
            schema,
            rows,
            rows_by_key,
            rows_by_lookup_key,
            duplicate_keys,
        });
        self.table_cache.insert(cache_key, table.clone());
        Ok(table)
    }

    fn datasheet_asset(&self, source_path: &str) -> Option<&crate::assets::DatasheetAsset> {
        let normalized = normalize_data_path(source_path);
        self.datasheets_by_path.get(&normalized).or_else(|| {
            self.datasheets_by_path
                .iter()
                .find_map(|(path, asset)| path.ends_with(&format!("/{normalized}")).then_some(asset))
        })
    }

    fn asset_bytes(&self, path: &str) -> Option<&[u8]> {
        let normalized = normalize_data_path(path);
        self.assets_by_path
            .get(&normalized)
            .map(Vec::as_slice)
            .or_else(|| {
                self.assets_by_path
                    .iter()
                    .find_map(|(candidate, bytes)| {
                        candidate
                            .ends_with(&format!("/{normalized}"))
                            .then_some(bytes.as_slice())
                    })
            })
    }

    fn required_asset_bytes(&self, path: &str) -> Result<&[u8]> {
        self.asset_bytes(path)
            .with_context(|| format!("asset {path} was not loaded"))
    }
}

impl ManagerInstance {
    #[must_use]
    fn table(&self, name: &str) -> Option<&DynamicTable> {
        self.tables.get(name).map(Arc::as_ref)
    }

    #[must_use]
    fn asset_bytes(&self, path: &str) -> Option<&[u8]> {
        let normalized = normalize_data_path(path);
        self.assets.get(&normalized).map(Vec::as_slice).or_else(|| {
            self.assets.iter().find_map(|(candidate, bytes)| {
                candidate
                    .ends_with(&format!("/{normalized}"))
                    .then_some(bytes.as_slice())
            })
        })
    }

    fn required_asset_bytes(&self, path: &str) -> Result<&[u8]> {
        self.asset_bytes(path)
            .with_context(|| format!("manager {} asset {path} was not loaded", self.definition.name))
    }

    fn schema_rows<T>(
        &self,
        row_type: &str,
        read: fn(&DynamicTable, &DynamicTableRow) -> Result<T>,
    ) -> Result<Vec<T>> {
        let mut rows = Vec::new();
        for table in self.all_tables() {
            if table.schema.row_type != row_type {
                continue;
            }
            for row in &table.rows {
                rows.push(read(table, row)?);
            }
        }
        Ok(rows)
    }

    fn schema_row<T>(
        &self,
        row_type: &str,
        key: impl ToString,
        read: fn(&DynamicTable, &DynamicTableRow) -> Result<T>,
        key_of: impl Fn(&T) -> String,
    ) -> Result<Option<T>> {
        let lookup_key = normalize_lookup_key(&key.to_string());
        for row in self.schema_rows(row_type, read)? {
            if normalize_lookup_key(&key_of(&row)) == lookup_key {
                return Ok(Some(row));
            }
        }
        Ok(None)
    }

    fn all_tables(&self) -> Vec<&DynamicTable> {
        let mut seen = HashSet::new();
        let mut tables = Vec::new();
        for table in self.tables.values() {
            if seen.insert(Arc::as_ptr(table)) {
                tables.push(table.as_ref());
            }
        }
        tables
    }
}

impl DynamicTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

}

#[must_use]
fn manager_by_name(name: &str) -> Option<&'static ManagerDefinition> {
    MANAGERS.iter().find(|entry| entry.name == name)
}

#[must_use]
fn table_schema_by_name(name: &str) -> Option<&'static TableDescriptor> {
    TABLES.iter().find(|table| table.name == name)
}

#[must_use]
fn table_schema_by_name_and_row(
    name: &str,
    row_type: &str,
) -> Option<&'static TableDescriptor> {
    TABLES
        .iter()
        .find(|table| table.name == name && table.row_type == row_type)
}

#[must_use]
fn table_schema_by_source_path(source_path: &str) -> Option<&'static TableDescriptor> {
    let normalized = normalize_data_path(source_path);
    TABLES.iter().find(|table| {
        table
            .sources
            .iter()
            .any(|candidate| normalize_data_path(candidate) == normalized)
    })
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
    let column = table
        .schema
        .columns
        .iter()
        .find(|column| column_matches(column, column_name))?;
    let slot = *row.column_slots.get(&column.crc)?;
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
                        "row {}:{} has non-number {column_name}",
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

fn required_u32_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<u32> {
    normalize_u32(required_number_cell(table, row, column_name)?)
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

fn optional_lowercase_crc_string_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Option<u32>> {
    Ok(optional_string_cell(table, row, column_name)?.map(crc32_lowercase))
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

fn u32_list_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Vec<u32>> {
    number_list_cell(table, row, column_name)?
        .into_iter()
        .map(normalize_u32)
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

fn f32_range_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<(f32, f32)> {
    let values = number_range_values(table, row, column_name)?;
    let [first, second, ..] = values.as_slice() else {
        bail!(
            "row {}:{} missing range {column_name}",
            row.source_path,
            row.row_index + 1
        );
    };
    Ok((*first, *second))
}

fn u32_range_cell(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<(u32, u32)> {
    let (first, second) = f32_range_cell(table, row, column_name)?;
    Ok((normalize_u32(first)?, normalize_u32(second)?))
}

fn number_range_values(
    table: &DynamicTable,
    row: &DynamicTableRow,
    column_name: &str,
) -> Result<Vec<f32>> {
    match row_cell(table, row, column_name) {
        Some(DatasheetCellValue::String(value)) => split_designer_range(value)
            .into_iter()
            .map(|part| parse_designer_number(part, row, column_name))
            .collect(),
        _ => number_list_cell(table, row, column_name),
    }
}

fn split_designer_list(value: &str) -> Vec<&str> {
    value
        .split([',', '+'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn split_designer_range(value: &str) -> Vec<&str> {
    let listed = split_designer_list(value);
    if listed.len() >= 2 {
        return listed.into_iter().take(2).collect();
    }
    let text = value.trim();
    for (index, byte) in text.bytes().enumerate().skip(1) {
        if byte != b'-' {
            continue;
        }
        let left = text[..index].trim();
        let right = text[index + 1..].trim();
        if !left.is_empty() && !right.is_empty() {
            return vec![left, right];
        }
    }
    listed
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

fn normalize_lookup_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

fn column_matches(column: &ColumnDescriptor, name: &str) -> bool {
    column.name == name || column.field_name == name
}

fn normalize_data_path(path: &str) -> String {
    path.replace('\\', "/").replace("//", "/").to_ascii_lowercase()
}
"#;
