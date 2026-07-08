use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use nw_datasheet::ColumnType;

use crate::compiler::GameDataCompileUnit;
use crate::emit::GameDataCodegenFile;
use crate::game_system_schema::GameSystemTableSchema;
use crate::manager_records::{
    DirectManagerSurface, ItemDataManagerSurface, ManagerSurface, ManagerSurfaceDependency,
    SemanticLookupKind, SemanticManagerKey, SemanticManagerRecord, SemanticNumericKeyType,
    SemanticProjectionTransform, SemanticRowFilterPredicate, manager_surface_dependencies,
    manager_surface_name, manager_surfaces, semantic_manager_record_unit, ts_field_name,
    ts_method_name,
};
use crate::naming::{to_snake_ident, to_upper_camel_ident};
use crate::typescript::source::{format_typescript_source, typescript_string_literal};
use nw_serialize_codegen::{
    TypeScriptSourceEmitter as SerializeTypeScriptSourceEmitter,
    TypeScriptSourceOptions as SerializeTypeScriptSourceOptions,
};

pub(super) fn emit_manager_files(unit: &GameDataCompileUnit) -> Result<Vec<GameDataCodegenFile>> {
    let surfaces = manager_surfaces(unit)?;
    Ok(vec![GameDataCodegenFile::new(
        "src/managers/index.ts",
        manager_index_source(unit, false, &surfaces)?,
    )])
}

pub(super) fn emit_dynamic_manager_files(
    unit: &GameDataCompileUnit,
) -> Result<Vec<GameDataCodegenFile>> {
    let surfaces = manager_surfaces(unit)?;
    let records = semantic_records(&surfaces);
    let mut files = vec![GameDataCodegenFile::new(
        "src/managers/index.ts",
        manager_index_source(unit, true, &surfaces)?,
    )];
    if !records.is_empty() {
        files.push(GameDataCodegenFile::new(
            "src/managers/types.ts",
            manager_record_types_source(&records)?,
        ));
    }
    Ok(files)
}

fn manager_index_source(
    unit: &GameDataCompileUnit,
    dynamic_assets: bool,
    surfaces: &[ManagerSurface],
) -> Result<String> {
    if !dynamic_assets {
        return manager_manifest_source(unit, surfaces);
    }

    let mut source = String::from(
        r#"
import { parseDatasheet, type Datasheet, type DatasheetAsset, type DatasheetCellValue, type DatasheetRow } from "../game-assets/datasheet.js";
import {
  objectStreamBool,
  objectStreamF32,
  objectStreamI32,
  objectStreamString,
  objectStreamU8,
  objectStreamU32,
  objectStreamVec3,
  parseObjectStream,
  requiredChildByNameCrc,
  requireObjectStreamType,
  singleObjectStreamRoot,
  type ObjectStreamElement,
  type ObjectStreamVec3,
} from "../game-assets/object-stream.js";
import { type AssetLoader } from "../game-assets/pak.js";

"#,
    );
    let records = semantic_records(surfaces);
    if !records.is_empty() {
        source.push_str(&format!(
            "import {{ {} }} from \"./types.js\";\n\n",
            records
                .iter()
                .map(|record| format!("type {}", record.record_type_name))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    source.push_str(
        r#"
export type DatasheetCellKind = "string" | "number" | "boolean";

export interface ColumnSchema {
  readonly name: string;
  readonly fieldName: string;
  readonly crc: number;
  readonly kind: DatasheetCellKind;
  readonly rowKey: boolean;
  readonly required: boolean;
}

export interface TableSchema {
  readonly name: string;
  readonly nameCrc: number;
  readonly rowType: string;
  readonly rowTypeCrc: number;
  readonly rowCount: number;
  readonly sources: readonly string[];
  readonly columns: readonly ColumnSchema[];
}

interface TableDependency {
  readonly kind: "table";
  readonly name: string;
  readonly row: string;
}

interface AssetDependency {
  readonly kind: "asset";
  readonly path: string;
}

type ManagerDependency =
  | TableDependency
  | AssetDependency;

interface ManagerDefinition {
  readonly name: string;
  readonly dependencies: readonly ManagerDependency[];
}

const MANAGER_INSTANCE = Symbol("managerInstance");

interface DynamicTableRow {
  readonly sourcePath: string;
  readonly rowIndex: number;
  readonly key: string;
  readonly row: DatasheetRow;
  readonly columnSlots: ReadonlyMap<number, number>;
}

interface DynamicTable {
  readonly schema: TableSchema;
  readonly sheets: readonly Datasheet[];
  readonly rows: readonly DynamicTableRow[];
  readonly rowsByKey: ReadonlyMap<string, DynamicTableRow>;
  readonly rowsByLookupKey: ReadonlyMap<string, DynamicTableRow>;
  readonly duplicateKeys: ReadonlyMap<string, readonly DynamicTableRow[]>;
}

interface BinaryAsset {
  readonly path: string;
  readonly bytes: Uint8Array;
}

interface PakDatasheetSource {
  readonly datasheets: readonly DatasheetAsset[];
  readonly assets: readonly BinaryAsset[];
}

const ASSET_LOADER_SOURCE = Symbol.for("@nw-tools/asset-loader/source");

type AssetLoaderSourceAccessor = {
  [key: symbol]: () => Promise<PakDatasheetSource>;
};

"#,
    );

    let readable_row_types = direct_schema_row_types(surfaces);
    push_schema_row_types(&mut source, unit, &readable_row_types);
    push_table_schemas(&mut source, unit);
    push_managers(&mut source, unit, surfaces);
    push_manager_surface_classes(&mut source, unit, surfaces);
    source.push_str(PRODUCT_MANAGER_RUNTIME_TS);
    source.push_str(DYNAMIC_MANAGER_RUNTIME_TS);
    push_managers_facade(&mut source, surfaces);

    Ok(format_typescript_source(&source)?)
}

fn semantic_records(surfaces: &[ManagerSurface]) -> Vec<SemanticManagerRecord> {
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

fn direct_schema_row_types(surfaces: &[ManagerSurface]) -> BTreeSet<String> {
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

fn ts_manager_accessor_name(manager_name: &str) -> String {
    ts_method_name(manager_name.strip_suffix("Manager").unwrap_or(manager_name))
}

#[derive(Debug, Clone)]
struct TsSchemaRow {
    type_name: String,
    source_row_type: String,
    fields: Vec<TsSchemaField>,
}

#[derive(Debug, Clone)]
struct TsSchemaField {
    source_name: String,
    field_name: String,
    column_type: ColumnType,
    required: bool,
    row_key: bool,
}

fn push_schema_row_types(
    source: &mut String,
    unit: &GameDataCompileUnit,
    readable_row_types: &BTreeSet<String>,
) {
    for row in ts_schema_rows(unit) {
        if row.source_row_type == "LootBucketData" {
            push_loot_bucket_schema_row_type(
                source,
                readable_row_types.contains(&row.source_row_type),
            );
            continue;
        }
        source.push_str(&format!("export interface {} {{\n", row.type_name));
        for field in &row.fields {
            let value_type = ts_schema_field_type(field.column_type);
            let nullable = if field.required { "" } else { " | null" };
            source.push_str(&format!(
                "  readonly {}: {}{};\n",
                field.field_name, value_type, nullable
            ));
        }
        source.push_str("}\n\n");
        if !readable_row_types.contains(&row.source_row_type) {
            continue;
        }
        source.push_str(&format!(
            "function {}(table: DynamicTable, row: DynamicTableRow): {} {{\n",
            ts_schema_reader_name(&row.source_row_type),
            row.type_name
        ));
        source.push_str("  return {\n");
        for field in &row.fields {
            source.push_str(&format!(
                "    {}: {},\n",
                field.field_name,
                ts_schema_field_read_expression(field)
            ));
        }
        source.push_str("  };\n");
        source.push_str("}\n\n");
    }
}

fn push_loot_bucket_schema_row_type(source: &mut String, readable: bool) {
    source.push_str(
        r#"
export interface LootBucketDataSchemaRow {
  readonly rowPlaceholders: string;
  readonly entries: readonly LootBucketDataEntry[];
  readonly lootBiasingDisabled: readonly LootBucketBiasingDisabled[];
}

export interface LootBucketDataEntry {
  readonly slot: number;
  readonly lootBucket: string | null;
  readonly tags: string | null;
  readonly matchOne: string | null;
  readonly item: string | null;
  readonly quantity: string | null;
  readonly odds: string | null;
}

export interface LootBucketBiasingDisabled {
  readonly slot: number;
  readonly disabled: boolean;
}

"#,
    );

    if !readable {
        return;
    }

    source.push_str(
        r#"
function readLootBucketDataSchemaRow(
  table: DynamicTable,
  row: DynamicTableRow,
): LootBucketDataSchemaRow {
  const entries: LootBucketDataEntry[] = [];
  for (const slot of numberedColumnSlots(table, [
    "LootBucket",
    "Tags",
    "MatchOne",
    "Item",
    "Quantity",
    "Odds",
  ])) {
    const lootBucket = optionalCellText(table, row, numberedColumnName("LootBucket", slot));
    const tags = optionalCellText(table, row, numberedColumnName("Tags", slot));
    const matchOne = optionalCellText(table, row, numberedColumnName("MatchOne", slot));
    const item = optionalCellText(table, row, numberedColumnName("Item", slot));
    const quantity = optionalCellText(table, row, numberedColumnName("Quantity", slot));
    const odds = optionalCellText(table, row, numberedColumnName("Odds", slot));
    if (
      lootBucket !== null ||
      tags !== null ||
      matchOne !== null ||
      item !== null ||
      quantity !== null ||
      odds !== null
    ) {
      entries.push({ slot, lootBucket, tags, matchOne, item, quantity, odds });
    }
  }

  const lootBiasingDisabled: LootBucketBiasingDisabled[] = [];
  for (const slot of numberedColumnSlots(table, ["LootBiasingDisabled"])) {
    const disabled = optionalCellBoolText(
      table,
      row,
      numberedColumnName("LootBiasingDisabled", slot),
    );
    if (disabled !== null) {
      lootBiasingDisabled.push({ slot, disabled });
    }
  }

  return {
    rowPlaceholders: requiredStringCell(table, row, "RowPlaceholders"),
    entries,
    lootBiasingDisabled,
  };
}

function numberedColumnSlots(
  table: DynamicTable,
  prefixes: readonly string[],
): number[] {
  const slots = new Set<number>();
  for (const column of table.schema.columns) {
    for (const prefix of prefixes) {
      const slot = numberedColumnSlot(column.name, prefix);
      if (slot !== null) {
        slots.add(slot);
      }
    }
  }
  return [...slots].sort((left, right) => left - right);
}

function numberedColumnSlot(name: string, prefix: string): number | null {
  if (!name.startsWith(prefix)) {
    return null;
  }
  const suffix = name.slice(prefix.length);
  if (!/^\d+$/.test(suffix)) {
    return null;
  }
  return Number.parseInt(suffix, 10);
}

function numberedColumnName(prefix: string, slot: number): string {
  return `${prefix}${slot}`;
}

function optionalCellText(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): string | null {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return null;
  }
  if (value.kind === "string") {
    return value.value.length === 0 ? null : value.value;
  }
  if (value.kind === "number" || value.kind === "boolean") {
    return String(value.value);
  }
  return null;
}

function optionalCellBoolText(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): boolean | null {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return null;
  }
  if (value.kind === "boolean") {
    return value.value;
  }
  if (value.kind === "number") {
    return value.value !== 0;
  }
  if (value.kind === "string") {
    const text = value.value.trim().toLowerCase();
    if (text.length === 0) {
      return null;
    }
    if (text === "true" || text === "1" || text === "yes") {
      return true;
    }
    if (text === "false" || text === "0" || text === "no") {
      return false;
    }
  }
  throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has non-bool ${columnName}`);
}

"#,
    );
}

fn ts_schema_rows(unit: &GameDataCompileUnit) -> Vec<TsSchemaRow> {
    let mut rows = BTreeMap::<String, Vec<TsSchemaField>>::new();
    for table in &unit.schema_report().tables {
        let row_type = table.row_type_name.clone();
        let fields = rows.entry(row_type.clone()).or_default();
        for column in &table.columns {
            let field_name = ts_field_name(&column.name);
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
            fields.push(TsSchemaField {
                source_name: column.name.clone(),
                field_name,
                column_type: column.declared_type,
                required: column.row_key,
                row_key: column.row_key,
            });
        }
    }
    rows.into_iter()
        .map(|(source_row_type, fields)| TsSchemaRow {
            type_name: ts_schema_row_type_name(&source_row_type),
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

fn ts_schema_field_type(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::String => "string",
        ColumnType::Number => "number",
        ColumnType::Boolean => "boolean",
    }
}

fn ts_schema_field_read_expression(field: &TsSchemaField) -> String {
    let column = typescript_string_literal(&field.source_name);
    match (field.column_type, field.required) {
        (ColumnType::String, true) => format!("requiredStringCell(table, row, {column})"),
        (ColumnType::String, false) => format!("optionalStringCell(table, row, {column})"),
        (ColumnType::Number, true) => format!("requiredNumberCell(table, row, {column})"),
        (ColumnType::Number, false) => format!("optionalNumberCell(table, row, {column})"),
        (ColumnType::Boolean, true) => format!("requiredBoolCell(table, row, {column})"),
        (ColumnType::Boolean, false) => format!("optionalBoolCell(table, row, {column})"),
    }
}

fn ts_schema_reader_name(row_type: &str) -> String {
    format!("read{}", ts_schema_row_type_name(row_type))
}

fn ts_schema_row_type_name(row_type: &str) -> String {
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

fn manager_manifest_source(
    unit: &GameDataCompileUnit,
    surfaces: &[ManagerSurface],
) -> Result<String> {
    let mut source = String::from(
        r#"
interface TableDependency {
  readonly kind: "table";
  readonly name: string;
  readonly row: string;
}

interface AssetDependency {
  readonly kind: "asset";
  readonly path: string;
}

type ManagerDependency =
  | TableDependency
  | AssetDependency;

interface ManagerDefinition {
  readonly name: string;
  readonly dependencies: readonly ManagerDependency[];
}

"#,
    );

    push_managers(&mut source, unit, surfaces);
    source.push_str(
        r#"
function managerByName(
  name: string,
): ManagerDefinition | undefined {
  return MANAGERS.find((entry) => entry.name === name);
}
"#,
    );

    Ok(format_typescript_source(&source)?)
}

fn push_table_schemas(source: &mut String, unit: &GameDataCompileUnit) {
    source.push_str("export const TABLE_SCHEMAS: readonly TableSchema[] = [\n");
    for table in &unit.schema_report().tables {
        push_table_schema(source, table);
    }
    source.push_str(
        r#"] as const satisfies readonly TableSchema[];

export function tableSchemaByName(name: string): TableSchema | undefined {
  return TABLE_SCHEMAS.find((table) => table.name === name);
}

export function tableSchemaByNameAndRow(
  name: string,
  rowType: string,
): TableSchema | undefined {
  return TABLE_SCHEMAS.find((table) => table.name === name && table.rowType === rowType);
}

export function tableSchemaBySourcePath(sourcePath: string): TableSchema | undefined {
  const normalized = normalizeDataPath(sourcePath);
  return TABLE_SCHEMAS.find((table) =>
    table.sources.some((candidate) => normalizeDataPath(candidate) === normalized),
  );
}

"#,
    );
}

fn push_table_schema(source: &mut String, table: &GameSystemTableSchema) {
    source.push_str("  {\n");
    source.push_str(&format!(
        "    name: {},\n",
        typescript_string_literal(&table.table_name)
    ));
    source.push_str(&format!("    nameCrc: {},\n", table.table_name_crc));
    source.push_str(&format!(
        "    rowType: {},\n",
        typescript_string_literal(&table.row_type_name)
    ));
    source.push_str(&format!("    rowTypeCrc: {},\n", table.row_type_crc));
    source.push_str(&format!("    rowCount: {},\n", table.row_count));
    source.push_str("    sources: [");
    for (index, source_path) in table.sources.iter().enumerate() {
        if index > 0 {
            source.push_str(", ");
        }
        source.push_str(&typescript_string_literal(source_path));
    }
    source.push_str("],\n");
    source.push_str("    columns: [\n");
    for column in &table.columns {
        source.push_str("      {\n");
        source.push_str(&format!(
            "        name: {},\n",
            typescript_string_literal(&column.name)
        ));
        source.push_str(&format!(
            "        fieldName: {},\n",
            typescript_string_literal(&to_snake_ident(&column.name, "column"))
        ));
        source.push_str(&format!("        crc: {},\n", column.crc));
        source.push_str(&format!(
            "        kind: {},\n",
            typescript_string_literal(cell_kind(column.declared_type))
        ));
        source.push_str(&format!("        rowKey: {},\n", column.row_key));
        source.push_str(&format!("        required: {},\n", column.required));
        source.push_str("      },\n");
    }
    source.push_str("    ],\n");
    source.push_str("  },\n");
}

fn push_managers(source: &mut String, unit: &GameDataCompileUnit, surfaces: &[ManagerSurface]) {
    source.push_str("const MANAGERS: readonly ManagerDefinition[] = [\n");
    let contracts = unit.codegen_plan_ref().managers().contracts();
    for surface in surfaces {
        let manager_name = manager_surface_name(surface);
        let Some(contract) = contracts
            .iter()
            .find(|contract| semantic_type_name(contract.manager().as_str()) == manager_name)
        else {
            continue;
        };
        let dependencies = manager_surface_dependencies(surface, contract.inputs());
        source.push_str("  {\n");
        source.push_str(&format!(
            "    name: {},\n",
            typescript_string_literal(manager_name)
        ));
        source.push_str(&format!(
            "    dependencies: [{}],\n",
            manager_dependencies(&dependencies)
        ));
        source.push_str("  },\n");
    }
    source.push_str("];\n\n");
}

fn manager_dependencies(dependencies: &[ManagerSurfaceDependency]) -> String {
    dependencies
        .iter()
        .map(manager_dependency)
        .collect::<Vec<_>>()
        .join(", ")
}

fn manager_dependency(input: &ManagerSurfaceDependency) -> String {
    match input {
        ManagerSurfaceDependency::Table { name, row } => format!(
            "{{ kind: \"table\", name: {}, row: {} }}",
            typescript_string_literal(name),
            typescript_string_literal(row),
        ),
        ManagerSurfaceDependency::Asset { path } => format!(
            "{{ kind: \"asset\", path: {} }}",
            typescript_string_literal(path),
        ),
    }
}

fn semantic_type_name(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

fn cell_kind(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::String => "string",
        ColumnType::Number => "number",
        ColumnType::Boolean => "boolean",
    }
}

fn manager_record_types_source(records: &[SemanticManagerRecord]) -> Result<String> {
    let unit = semantic_manager_record_unit(records);
    SerializeTypeScriptSourceEmitter
        .emit_with_options(
            &unit,
            &SerializeTypeScriptSourceOptions {
                include_support_aliases: false,
            },
        )
        .map_err(|err| anyhow::anyhow!("emit TypeScript manager record types: {err}"))
}

fn push_manager_surface_classes(
    source: &mut String,
    unit: &GameDataCompileUnit,
    surfaces: &[ManagerSurface],
) {
    if surfaces.is_empty() {
        return;
    }
    for surface in surfaces {
        match surface {
            ManagerSurface::Direct(manager) => push_direct_manager_class(source, unit, manager),
            ManagerSurface::Semantic(record) => push_semantic_manager_class(source, record),
            ManagerSurface::ItemData(manager) => push_item_data_manager_class(source, manager),
            ManagerSurface::ProductBacked(manager) => {
                push_product_backed_manager_class(source, manager)
            }
        }
    }
    source.push_str(SEMANTIC_MANAGER_RUNTIME_TS);
}

fn push_managers_facade(source: &mut String, surfaces: &[ManagerSurface]) {
    let mut methods = String::new();
    let mut seen = BTreeSet::new();
    for surface in surfaces {
        let manager_name = manager_surface_name(surface);
        if !seen.insert(manager_name) {
            continue;
        }
        let manager_class = match surface {
            ManagerSurface::Direct(manager) | ManagerSurface::ProductBacked(manager) => {
                manager.manager_class_name.as_str()
            }
            ManagerSurface::Semantic(record) => record.manager_class_name.as_str(),
            ManagerSurface::ItemData(manager) => manager.manager_class_name.as_str(),
        };
        let accessor = ts_manager_accessor_name(&manager_name);
        methods.push_str(&format!(
            r#"  {accessor}(): {manager_class} {{
    return {manager_class}.fromCache(this.cache);
  }}

"#
        ));
    }

    source.push_str(&format!(
        r#"
export class Managers {{
  private constructor(private readonly cache: ManagerCache) {{}}

  static async open(loader: AssetLoader): Promise<Managers> {{
    const source = await (loader as unknown as AssetLoaderSourceAccessor)[ASSET_LOADER_SOURCE]();
    return new Managers(new ManagerCache(source));
  }}

{methods}}}

export async function openManagers(loader: AssetLoader): Promise<Managers> {{
  return Managers.open(loader);
}}

"#
    ));
}

fn push_direct_manager_class(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) {
    let manager_class = &manager.manager_class_name;
    let manager_name = typescript_string_literal(&manager.manager_name);
    let runtime_factory = ts_method_name(&manager.manager_name);
    let accessor = ts_manager_accessor_name(&manager.manager_name);
    let mut product_methods = direct_ts_product_methods(manager);
    product_methods.push_str(&special_ts_manager_extra_methods(manager_class));
    let row_methods = direct_ts_schema_methods(unit, manager);
    let constructor = if row_methods.trim().is_empty() && product_methods.trim().is_empty() {
        "constructor(instance: ManagerInstance) { void instance; }"
    } else {
        "constructor(private readonly instance: ManagerInstance) {}"
    };
    source.push_str(&format!(
        r#"
export class {manager_class} {{
  {constructor}

  static fromCache(cache: ManagerCache): {manager_class} {{
    return new {manager_class}(cache[MANAGER_INSTANCE]({manager_name}) as ManagerInstance);
  }}

{row_methods}
{product_methods}
}}

export function {runtime_factory}(managers: Managers): {manager_class} {{
  return managers.{accessor}();
}}

"#
    ));
}

fn direct_ts_schema_methods(unit: &GameDataCompileUnit, manager: &DirectManagerSurface) -> String {
    let row_specs = ts_schema_rows(unit);
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
    for row_type in row_types {
        let Some(row_spec) = row_specs.iter().find(|row| row.source_row_type == row_type) else {
            continue;
        };
        let type_name = &row_spec.type_name;
        let rows_method = if single_row_type {
            "rows".to_owned()
        } else {
            format!("{}Rows", ts_method_name(&row_type))
        };
        source.push_str(&format!(
            r#"  {rows_method}(): readonly {type_name}[] {{
    return this.instance.schemaRows({row_type:?}, {reader});
  }}

"#,
            reader = ts_schema_reader_name(&row_type),
        ));
        if let Some(key_field) = row_spec.fields.iter().find(|field| field.row_key) {
            let lookup_method = if single_row_type {
                "get".to_owned()
            } else {
                ts_method_name(&row_type)
            };
            source.push_str(&format!(
                r#"  {lookup_method}(key: {type_name}[{key_field:?}]): {type_name} | undefined {{
    return this.instance.schemaRow({row_type:?}, key, {reader}, (row) => row.{key_member});
  }}

"#,
                key_field = key_field.field_name.as_str(),
                key_member = key_field.field_name.as_str(),
                reader = ts_schema_reader_name(&row_type),
            ));
        }
    }
    source
}

fn direct_ts_product_methods(manager: &DirectManagerSurface) -> String {
    let mut source = String::new();
    for product in &manager.products {
        let path = typescript_string_literal(&product.path);
        let getter = ts_method_name(&product.manager_getter);
        match product.value_type.as_str() {
            "newworld_plugin::assets::armor_offset_database::ArmorOffsetDatabase" => {
                source.push_str(&format!(
                    r#"  private parsedArmorOffsetDatabase?: ArmorOffsetDatabase;

  {getter}(): ArmorOffsetDatabase {{
    this.parsedArmorOffsetDatabase ??= parseArmorOffsetDatabase(this.instance.requiredAssetBytes({path}));
    return this.parsedArmorOffsetDatabase;
  }}

  armorOffset(name: string): ArmorOffsetData | undefined {{
    return armorOffsetByName(this.{getter}(), name);
  }}

  furthestAttachmentOffset(
    armorOffsetNames: readonly string[],
    attachmentName: string,
    currentPosition: Vec3 = ZERO_VEC3,
  ): AttachmentOffsetData | undefined {{
    return furthestArmorAttachmentOffset(
      this.{getter}(),
      armorOffsetNames,
      attachmentName,
      currentPosition,
    );
  }}

"#
                    ,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::equip_types_database::EquipTypesDatabase" => {
                source.push_str(&format!(
                    r#"  private parsedEquipTypesDatabase?: EquipTypesDatabase;

  {getter}(): EquipTypesDatabase {{
    this.parsedEquipTypesDatabase ??= parseEquipTypesDatabase(this.instance.requiredAssetBytes({path}));
    return this.parsedEquipTypesDatabase;
  }}

  equipTypes(): readonly EquipTypeData[] {{
    return this.{getter}().equipTypes;
  }}

"#
                    ,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::game_debug_settings::GameDebugSettings" => {
                source.push_str(&format!(
                    r#"  private parsedGameDebugSettings?: GameDebugSettings;

  {getter}(): GameDebugSettings {{
    this.parsedGameDebugSettings ??= parseGameDebugSettings(this.instance.requiredAssetBytes({path}));
    return this.parsedGameDebugSettings;
  }}

  combat(): CombatDebugSettings {{
    return this.{getter}().combatSettings;
  }}

  disabledCombatToggleCount(): number {{
    return disabledCombatToggleCount(this.combat());
  }}

"#
                    ,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::player_base_attributes::PlayerBaseAttributes" => {
                source.push_str(&format!(
                    r#"  private parsedPlayerBaseAttributes?: PlayerBaseAttributes;

  {getter}(): PlayerBaseAttributes {{
    this.parsedPlayerBaseAttributes ??= parsePlayerBaseAttributes(this.instance.requiredAssetBytes({path}));
    return this.parsedPlayerBaseAttributes;
  }}

  playerAttributeData(): PlayerAttributeData {{
    return this.{getter}().playerAttributeData;
  }}

  maxPerks(rarityLevel: number): number | undefined {{
    return this.{getter}().playerAttributeData.itemRarityData[rarityLevel]?.maxPerkCount;
  }}

"#,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::settlement_progression_data::SettlementProgressionData" => {
                source.push_str(&format!(
                    r#"  private parsedSettlementProgressionData?: SettlementProgressionData;

  {getter}(): SettlementProgressionData {{
    this.parsedSettlementProgressionData ??= parseSettlementProgressionData(this.instance.requiredAssetBytes({path}));
    return this.parsedSettlementProgressionData;
  }}

  settlementProgressionCategories(): readonly ProgressionCategoryEntry[] {{
    return this.{getter}().settlementProgressionCategories;
  }}

"#,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::ui_database::UiDatabase" => {
                source.push_str(&format!(
                    r#"  private parsedUiDatabase?: UiDatabase;
  private interactOptionsByNameCrc?: ReadonlyMap<number, InteractOptionData>;

  {getter}(): UiDatabase {{
    this.parsedUiDatabase ??= parseUiDatabase(this.instance.requiredAssetBytes({path}));
    return this.parsedUiDatabase;
  }}

  interactOptions(): readonly InteractOptionData[] {{
    return this.{getter}().unifiedInteractData.interactOptions;
  }}

  interactOption(id: string | number): InteractOptionData | undefined {{
    const key = typeof id === "number" ? normalizeCrcKey(id) : crc32Lowercase(id);
    this.interactOptionsByNameCrc ??= indexInteractOptionsByNameCrc(this.interactOptions());
    return this.interactOptionsByNameCrc.get(key);
  }}

  interactOptionsByCategory(category: number): readonly InteractOptionData[] {{
    return this.interactOptions().filter(
      (option) =>
        option.interactOptionCategory === category ||
        option.interactOptionCategory === ALL_INTERACT_OPTIONS_CATEGORY,
    );
  }}

"#,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::camera_settings::GameCameraSettings" => {
                source.push_str(&format!(
                    r#"  private parsedCameraSettings?: GameCameraSettings;

  {getter}(): GameCameraSettings {{
    this.parsedCameraSettings ??= parseGameCameraSettings(this.instance.requiredAssetBytes({path}));
    return this.parsedCameraSettings;
  }}

  cameraStates(): readonly CameraStateSettings[] {{
    return this.{getter}().cameraStates;
  }}

"#,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::gathering_database::GatheringDatabase" => {
                source.push_str(&format!(
                    r#"  private parsedGatheringDatabase?: GatheringDatabase;

  {getter}(): GatheringDatabase {{
    this.parsedGatheringDatabase ??= parseGatheringDatabase(this.instance.requiredAssetBytes({path}));
    return this.parsedGatheringDatabase;
  }}

  gatheringData(): GatheringData {{
    return this.{getter}().gatheringData;
  }}

  gatheringTypes(): readonly GatheringTypeData[] {{
    return this.gatheringData().gatheringTypes;
  }}

  gatheringActions(): readonly GatheringAction[] {{
    return this.gatheringData().gatheringActions;
  }}

"#,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::gathering_database::GatheringActionDatabase" => {
                source.push_str(&format!(
                    r#"  private parsedGatheringActionDatabase?: GatheringActionDatabase;

  {getter}(): GatheringActionDatabase {{
    this.parsedGatheringActionDatabase ??= parseGatheringActionDatabase(this.instance.requiredAssetBytes({path}));
    return this.parsedGatheringActionDatabase;
  }}

  gatheringActionData(): readonly GatheringActionData[] {{
    return this.{getter}().gatheringActions;
  }}

"#,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::crafting_station_database::CraftingStationDatabase" => {
                source.push_str(&format!(
                    r#"  private parsedCraftingStationDatabase?: CraftingStationDatabase;

  {getter}(): CraftingStationDatabase {{
    this.parsedCraftingStationDatabase ??= parseCraftingStationDatabase(this.instance.requiredAssetBytes({path}));
    return this.parsedCraftingStationDatabase;
  }}

  craftingStations(): readonly CraftingStationData[] {{
    return this.{getter}().craftingStations;
  }}

"#,
                    getter = getter,
                    path = path,
                ));
            }
            "newworld_plugin::assets::rank_database::SocialRankDatabase" => {
                source.push_str(&format!(
                    r#"  private parsedSocialRankDatabase?: SocialRankDatabase;

  {getter}(): SocialRankDatabase {{
    this.parsedSocialRankDatabase ??= parseSocialRankDatabase(this.instance.requiredAssetBytes({path}));
    return this.parsedSocialRankDatabase;
  }}

  ranks(): readonly SocialRankData[] {{
    return this.{getter}().ranks;
  }}

"#,
                    getter = getter,
                    path = path,
                ));
            }
            _ => {}
        }
    }
    source
}

fn push_product_backed_manager_class(source: &mut String, manager: &DirectManagerSurface) {
    let manager_class = &manager.manager_class_name;
    let manager_name = typescript_string_literal(&manager.manager_name);
    let runtime_factory = ts_method_name(&manager.manager_name);
    let accessor = ts_manager_accessor_name(&manager.manager_name);
    let mut product_methods = direct_ts_product_methods(manager);
    product_methods.push_str(&special_ts_manager_extra_methods(manager_class));
    let constructor = if product_methods.trim().is_empty() {
        "constructor(instance: ManagerInstance) { void instance; }"
    } else {
        "constructor(private readonly instance: ManagerInstance) {}"
    };
    source.push_str(&format!(
        r#"
export class {manager_class} {{
  {constructor}

  static fromCache(cache: ManagerCache): {manager_class} {{
    return new {manager_class}(cache[MANAGER_INSTANCE]({manager_name}) as ManagerInstance);
  }}

{product_methods}
}}

export function {runtime_factory}(managers: Managers): {manager_class} {{
  return managers.{accessor}();
}}

"#
    ));
}

fn push_item_data_manager_class(source: &mut String, manager: &ItemDataManagerSurface) {
    let manager_class = &manager.manager_class_name;
    let manager_name = typescript_string_literal(&manager.manager_name);
    let runtime_factory = ts_method_name(&manager.manager_name);
    let accessor = ts_manager_accessor_name(&manager.manager_name);
    let table_type = &manager.table_type_name;
    let handle_type = &manager.handle_type_name;
    let data_type = &manager.data_type_name;
    let table_entries = manager
        .tables
        .iter()
        .map(|table| {
            format!(
                "  {}: {},\n",
                table.variant_name,
                typescript_string_literal(&table.table_name)
            )
        })
        .collect::<String>();
    let table_list = manager
        .tables
        .iter()
        .map(|table| format!("  {table_type}.{},\n", table.variant_name))
        .collect::<String>();

    source.push_str(&format!(
        r#"
export const {table_type} = {{
{table_entries}}} as const;

export type {table_type} = (typeof {table_type})[keyof typeof {table_type}];

export interface {handle_type} {{
  readonly table: {table_type};
  readonly row: number;
}}

export interface {data_type} {{
  readonly sourceHandle: {handle_type};
  readonly itemId: string;
  readonly itemIdCrc: number;
  readonly name: string | null;
  readonly description: string | null;
  readonly itemType: string | null;
  readonly itemTypeDisplayName: string | null;
  readonly uiItemClass: string | null;
  readonly heartgemRuneTooltipTitle: string | null;
  readonly confirmBeforeUse: boolean;
  readonly consumeOnUse: boolean;
  readonly bindOnPickup: boolean;
  readonly deathDropPercentage: number;
}}

const ITEM_DATA_MANAGER_TABLES: readonly {table_type}[] = [
{table_list}];

export class {manager_class} {{
  private readonly rowsCache: readonly {data_type}[];
  private readonly rowsById = new Map<number, {data_type}>();

  constructor(instance: ManagerInstance) {{
    this.rowsCache = materialize{manager_class}(instance);
    for (const row of this.rowsCache) {{
      this.rowsById.set(row.itemIdCrc, row);
    }}
  }}

  static fromCache(cache: ManagerCache): {manager_class} {{
    return new {manager_class}(cache[MANAGER_INSTANCE]({manager_name}) as ManagerInstance);
  }}

  get(itemId: string): {data_type} | undefined {{
    return this.getFromId(crc32Lowercase(itemId));
  }}

  getFromId(itemId: number): {data_type} | undefined {{
    return this.rowsById.get(normalizeCrcKey(itemId));
  }}

  byIndex(index: number): {data_type} | undefined {{
    if (!Number.isInteger(index) || index <= 0) {{
      return undefined;
    }}
    return this.rowsCache[index - 1];
  }}

  items(): readonly {data_type}[] {{
    return this.rowsCache;
  }}

  len(): number {{
    return this.rowsCache.length;
  }}

  isEmpty(): boolean {{
    return this.rowsCache.length === 0;
  }}
}}

export function {runtime_factory}(managers: Managers): {manager_class} {{
  return managers.{accessor}();
}}

function materialize{manager_class}(instance: ManagerInstance): {data_type}[] {{
  const items: {data_type}[] = [];
  const seen = new Set<number>();
  for (const tableName of ITEM_DATA_MANAGER_TABLES) {{
    const table = instance.table(tableName);
    if (table === undefined) {{
      throw new Error(`manager {manager_class} table ${{tableName}} was not loaded`);
    }}
    cache{manager_class}Rows(items, seen, tableName, table);
  }}
  return items;
}}

function cache{manager_class}Rows(
  items: {data_type}[],
  seen: Set<number>,
  tableName: {table_type},
  table: DynamicTable,
): void {{
  for (const sourceRow of table.rows) {{
    const itemId = requiredStringCell(table, sourceRow, "ItemID").trim();
    if (itemId.length === 0) {{
      continue;
    }}
    const itemIdCrc = crc32Lowercase(itemId);
    if (itemIdCrc === 0 || seen.has(itemIdCrc)) {{
      continue;
    }}
    seen.add(itemIdCrc);
    items.push({{
      sourceHandle: {{
        table: tableName,
        row: sourceRow.rowIndex + 1,
      }},
      itemId,
      itemIdCrc,
      name: optionalStringCell(table, sourceRow, "Name"),
      description: optionalStringCell(table, sourceRow, "Description"),
      itemType: optionalStringCell(table, sourceRow, "ItemType"),
      itemTypeDisplayName: optionalStringCell(table, sourceRow, "ItemTypeDisplayName"),
      uiItemClass: optionalStringCell(table, sourceRow, "UiItemClass"),
      heartgemRuneTooltipTitle: optionalStringCell(table, sourceRow, "HeartgemRuneTooltipTitle"),
      confirmBeforeUse: optionalBoolCell(table, sourceRow, "ConfirmBeforeUse") ?? false,
      consumeOnUse: optionalBoolCell(table, sourceRow, "ConsumeOnUse") ?? false,
      bindOnPickup: optionalBoolCell(table, sourceRow, "BindOnPickup") ?? false,
      deathDropPercentage: optionalNumberCell(table, sourceRow, "DeathDropPercentage") ?? 0,
    }});
  }}
}}

"#
    ));
}

fn push_semantic_manager_class(source: &mut String, record: &SemanticManagerRecord) {
    let record_type = &record.record_type_name;
    let manager_class = &record.manager_class_name;
    let manager_name = typescript_string_literal(&record.manager_name);
    let entries_field = "rowsCache";
    let by_key_field = "rowsByKey";
    let source_row_field = "rowsBySourceRow";
    let runtime_factory = ts_method_name(&record.manager_name);
    let accessor = ts_manager_accessor_name(&record.manager_name);
    let key_map_type = ts_key_map_type(record);
    let has_lookup_index = !record.lookup_methods.is_empty();
    let source_row_index_field = record.source_row_method.as_ref().map(|_| {
        record
            .source_row_field
            .as_ref()
            .expect("source-row lookup methods require a source-row field")
    });
    source.push_str(&format!(
        r#"
export class {manager_class} {{
  private readonly {entries_field}: readonly {record_type}[];
"#
    ));
    if has_lookup_index {
        source.push_str(&format!(
            "  private readonly {by_key_field} = new Map<{key_map_type}, {record_type}>();\n"
        ));
    }
    if source_row_index_field.is_some() {
        source.push_str(&format!(
            "  private readonly {source_row_field} = new Map<number, {record_type}>();\n"
        ));
    }
    source.push_str(&format!(
        r#"

  constructor(instance: ManagerInstance) {{
    this.{entries_field} = materialize{manager_class}(instance);
"#
    ));
    if has_lookup_index || source_row_index_field.is_some() {
        source.push_str(&format!("    for (const row of this.{entries_field}) {{\n"));
    }
    if has_lookup_index {
        if let Some(index_expression) = ts_row_index_expression(record) {
            source.push_str(&format!(
                "      this.{by_key_field}.set({index_expression}, row);\n"
            ));
        }
    }
    if let Some(field) = source_row_index_field {
        source.push_str(&format!(
            "      this.{source_row_field}.set(row.{}, row);\n",
            ts_field_name(field)
        ));
    }
    if has_lookup_index || record.source_row_method.is_some() {
        source.push_str("    }\n");
    }
    source.push_str(&format!(
        r#"
  }}

  static fromCache(cache: ManagerCache): {manager_class} {{
    return new {manager_class}(cache[MANAGER_INSTANCE]({manager_name}) as ManagerInstance);
  }}

"#
    ));
    for method in &record.lookup_methods {
        let method_name = ts_method_name(&method.name);
        let parameter_name = ts_field_name(&method.parameter);
        match method.kind {
            SemanticLookupKind::CrcStringKey => source.push_str(&format!(
                r#"  {method_name}({parameter_name}: string): {record_type} | undefined {{
    return this.{by_key_field}.get(crc32Lowercase({parameter_name}));
  }}

"#
            )),
            SemanticLookupKind::CrcKey => source.push_str(&format!(
                r#"  {method_name}({parameter_name}: number): {record_type} | undefined {{
    return this.{by_key_field}.get(normalizeCrcKey({parameter_name}));
  }}

"#
            )),
            SemanticLookupKind::NumericKey(_) => source.push_str(&format!(
                r#"  {method_name}({parameter_name}: number): {record_type} | undefined {{
    return this.{by_key_field}.get(normalizeNumericKey({parameter_name}));
  }}

"#
            )),
            SemanticLookupKind::StringKey => source.push_str(&format!(
                r#"  {method_name}({parameter_name}: string): {record_type} | undefined {{
    return this.{by_key_field}.get(normalizeStringKey({parameter_name}));
  }}

"#
            )),
        }
    }
    if let Some(method) = &record.source_row_method {
        let method_name = ts_method_name(method);
        source.push_str(&format!(
            r#"  {method_name}(row: number): {record_type} | undefined {{
    return this.{source_row_field}.get(row);
  }}

"#
        ));
    }
    if let Some(method) = &record.ids_method {
        let method_name = ts_method_name(method);
        let id_type = ts_ids_type(record);
        let id_expression = ts_ids_expression(record);
        source.push_str(&format!(
            r#"  {method_name}(): readonly {id_type}[] {{
    return this.{entries_field}.map((row) => {id_expression});
  }}

"#
        ));
    }
    if let Some(method) = &record.rows_method {
        let method_name = ts_method_name(method);
        source.push_str(&format!(
            r#"  {method_name}(): readonly {record_type}[] {{
    return this.{entries_field};
  }}

"#
        ));
    } else {
        source.push_str(&format!(
            r#"  rows(): readonly {record_type}[] {{
    return this.{entries_field};
  }}

"#
        ));
    }
    if let Some(method) = &record.len_method {
        let method_name = ts_method_name(method);
        source.push_str(&format!(
            r#"  {method_name}(): number {{
    return this.{entries_field}.length;
  }}

"#
        ));
    }
    if let Some(method) = &record.is_empty_method {
        let method_name = ts_method_name(method);
        source.push_str(&format!(
            r#"  {method_name}(): boolean {{
    return this.{entries_field}.length === 0;
  }}

"#
        ));
    }
    source.push_str(&special_ts_manager_extra_methods(manager_class));
    source.push_str("}\n\n");
    source.push_str(&format!(
        r#"export function {runtime_factory}(managers: Managers): {manager_class} {{
  return managers.{accessor}();
}}

"#
    ));
    push_semantic_materializer(source, record);
}

fn special_ts_manager_extra_methods(manager_class_name: &str) -> String {
    match manager_class_name {
        "PlayerDataManager" => {
            r#"  categoricalProgressionId(tradeskill: string | number): number | undefined {
    const normalized = normalizeTradeskillType(tradeskill);
    if (normalized === "None" || normalized === "WildernessSurvival") {
      return undefined;
    }
    return crc32Lowercase(normalized);
  }

"#
            .to_owned()
        }
        _ => String::new(),
    }
}

fn ts_key_map_type(record: &SemanticManagerRecord) -> &'static str {
    match record.key {
        Some(SemanticManagerKey::String { .. } | SemanticManagerKey::EnumString { .. }) => "string",
        Some(_) => "number",
        None => "number",
    }
}

fn ts_row_index_expression(record: &SemanticManagerRecord) -> Option<String> {
    Some(match record.key.as_ref()? {
        SemanticManagerKey::Crc { crc_field, .. }
        | SemanticManagerKey::FallbackCrc { crc_field, .. } => {
            format!("row.{}", ts_field_name(crc_field))
        }
        SemanticManagerKey::Numeric { key_field, .. } => {
            format!("row.{}", ts_field_name(key_field))
        }
        SemanticManagerKey::EnumString { key_field, .. }
        | SemanticManagerKey::String { key_field, .. } => {
            format!("normalizeStringKey(row.{})", ts_field_name(key_field))
        }
    })
}

fn ts_ids_type(record: &SemanticManagerRecord) -> &'static str {
    match record.key {
        Some(SemanticManagerKey::String { .. } | SemanticManagerKey::EnumString { .. }) => "string",
        _ => "number",
    }
}

fn ts_ids_expression(record: &SemanticManagerRecord) -> String {
    match record.key.as_ref() {
        Some(SemanticManagerKey::Crc { crc_field, .. })
        | Some(SemanticManagerKey::FallbackCrc { crc_field, .. }) => {
            format!("row.{}", ts_field_name(crc_field))
        }
        Some(SemanticManagerKey::Numeric { key_field, .. })
        | Some(SemanticManagerKey::EnumString { key_field, .. })
        | Some(SemanticManagerKey::String { key_field, .. }) => {
            format!("row.{}", ts_field_name(key_field))
        }
        None => "0".to_owned(),
    }
}

fn push_semantic_materializer(source: &mut String, record: &SemanticManagerRecord) {
    let record_type = &record.record_type_name;
    let manager_class = &record.manager_class_name;
    source.push_str(&format!(
        r#"function materialize{manager_class}(instance: ManagerInstance): readonly {record_type}[] {{
  const rows: {record_type}[] = [];
"#
    ));
    if record.key.is_some() {
        source.push_str("  const seen = new Set<string | number>();\n");
    }
    source.push_str(&format!(
        r#"  for (const tableName of [{}]) {{
    const table = instance.table(tableName);
    if (table === undefined) {{
      throw new Error(`manager {} missing table ${{tableName}}`);
    }}
    for (const sourceRow of table.rows) {{
"#,
        record
            .tables
            .iter()
            .map(|table| typescript_string_literal(&table.table_name))
            .collect::<Vec<_>>()
            .join(", "),
        record.manager_name
    ));
    push_ts_key_materializer(source, record);
    for filter in &record.row_filters {
        let column = typescript_string_literal(&filter.column);
        match filter.predicate {
            SemanticRowFilterPredicate::BoolTrueWhenPresent => source.push_str(&format!(
                r#"      if (optionalBoolCell(table, sourceRow, {column}) === true) {{
        continue;
      }}
"#
            )),
            SemanticRowFilterPredicate::BoolMustBeTrue => source.push_str(&format!(
                r#"      if (optionalBoolCell(table, sourceRow, {column}) !== true) {{
        continue;
      }}
"#
            )),
            SemanticRowFilterPredicate::F32GreaterThanOrEqualZero => source.push_str(&format!(
                r#"      if (requiredNumberCell(table, sourceRow, {column}) < 0) {{
        continue;
      }}
"#
            )),
            SemanticRowFilterPredicate::F32LessThanOrEqualZero => source.push_str(&format!(
                r#"      if (requiredNumberCell(table, sourceRow, {column}) > 0) {{
        continue;
      }}
"#
            )),
            SemanticRowFilterPredicate::F32AnyGreaterThanZero => {
                let checks = std::iter::once(filter.column.as_str())
                    .chain(filter.extra_columns.iter().map(String::as_str))
                    .map(|column| {
                        format!(
                            "requiredNumberCell(table, sourceRow, {}) > 0",
                            typescript_string_literal(column)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" || ");
                source.push_str(&format!(
                    r#"      if (!({checks})) {{
        continue;
      }}
"#
                ));
            }
            SemanticRowFilterPredicate::I32LessThanOrEqualZero => source.push_str(&format!(
                r#"      if (requiredInt32Cell(table, sourceRow, {column}) > 0) {{
        continue;
      }}
"#
            )),
            SemanticRowFilterPredicate::LowercaseCrcStringNonZero => source.push_str(&format!(
                r#"      if (crc32Lowercase(requiredStringCell(table, sourceRow, {column})) === 0) {{
        continue;
      }}
"#
            )),
            SemanticRowFilterPredicate::StringNotEqualToColumn => {
                let compare_column = typescript_string_literal(
                    filter
                        .compare_column
                        .as_deref()
                        .expect("string comparison row filters have compare columns"),
                );
                source.push_str(&format!(
                    r#"      if (requiredStringCell(table, sourceRow, {column}) === requiredStringCell(table, sourceRow, {compare_column})) {{
        continue;
      }}
"#
                ));
            }
        }
    }
    push_ts_duplicate_key_policy(source, record);
    source.push_str(&format!(
        r#"      const row: {record_type} = {{
"#
    ));
    if let Some(field) = &record.source_row_field {
        source.push_str(&format!(
            "        {}: sourceRow.rowIndex + 1,\n",
            ts_field_name(field)
        ));
    }
    push_ts_key_row_fields(source, record);
    for field in &record.fields {
        source.push_str(&format!(
            "        {}: {},\n",
            ts_field_name(&field.name),
            ts_projection_value(field)
        ));
    }
    source.push_str(
        r#"      };
      rows.push(row);
"#,
    );
    if record.key.is_some() {
        source.push_str("      seen.add(seenKey);\n");
    }
    source.push_str(
        r#"    }
  }
  return rows;
}

"#,
    );
}

fn push_ts_key_materializer(source: &mut String, record: &SemanticManagerRecord) {
    let Some(key) = &record.key else {
        return;
    };
    match key {
        SemanticManagerKey::Crc {
            key_column,
            skip_empty_key,
            trim_key,
            reject_zero_crc,
            ..
        } => {
            source.push_str(&format!(
                "      const keyText = requiredStringCell(table, sourceRow, {});\n",
                typescript_string_literal(key_column)
            ));
            if *trim_key {
                source.push_str("      const keyValue = keyText.trim();\n");
            } else {
                source.push_str("      const keyValue = keyText;\n");
            }
            if *skip_empty_key {
                source.push_str(
                    r#"      if (keyValue.length === 0) {
        continue;
      }
"#,
                );
            }
            source.push_str("      const keyCrc = crc32Lowercase(keyValue);\n");
            if *reject_zero_crc {
                source.push_str(
                    r#"      if (keyCrc === 0) {
        continue;
      }
"#,
                );
            }
            source.push_str("      const seenKey = keyCrc;\n");
        }
        SemanticManagerKey::FallbackCrc {
            primary_key_kind,
            fallback_key_kind,
            primary_key_column,
            fallback_key_column,
            skip_empty_key,
            ..
        } => {
            source.push_str(&format!(
                r#"      const primaryKeyValue = optionalStringCell(table, sourceRow, {});
      const fallbackKeyValue = optionalStringCell(table, sourceRow, {});
      const keyKind =
        primaryKeyValue !== null && primaryKeyValue.length > 0 ? {} : {};
      const keyValue =
        primaryKeyValue !== null && primaryKeyValue.length > 0
          ? primaryKeyValue
          : fallbackKeyValue ?? "";
"#,
                typescript_string_literal(primary_key_column),
                typescript_string_literal(fallback_key_column),
                typescript_string_literal(primary_key_kind),
                typescript_string_literal(fallback_key_kind)
            ));
            if *skip_empty_key {
                source.push_str(
                    r#"      if (keyValue.length === 0) {
        continue;
      }
"#,
                );
            }
            source.push_str(
                r#"      const keyCrc = crc32Lowercase(keyValue);
      const seenKey = keyCrc;
"#,
            );
        }
        SemanticManagerKey::Numeric {
            key_column,
            key_type,
            ..
        } => {
            source.push_str(&format!(
                "      const keyValue = {};\n      const seenKey = keyValue;\n",
                ts_numeric_key_value("table", "sourceRow", key_column, *key_type)
            ));
        }
        SemanticManagerKey::EnumString {
            key_column,
            skip_empty_key,
            trim_key,
            ..
        } => {
            source.push_str(&format!(
                "      const keyText = requiredStringCell(table, sourceRow, {});\n",
                typescript_string_literal(key_column)
            ));
            if *trim_key {
                source.push_str("      const keyValue = keyText.trim();\n");
            } else {
                source.push_str("      const keyValue = keyText;\n");
            }
            if *skip_empty_key {
                source.push_str(
                    r#"      if (keyValue.length === 0) {
        continue;
      }
"#,
                );
            }
            source.push_str("      const seenKey = normalizeStringKey(keyValue);\n");
        }
        SemanticManagerKey::String {
            key_column,
            skip_empty_key,
            ..
        } => {
            source.push_str(&format!(
                "      const keyValue = requiredStringCell(table, sourceRow, {});\n",
                typescript_string_literal(key_column)
            ));
            if *skip_empty_key {
                source.push_str(
                    r#"      if (keyValue.length === 0) {
        continue;
      }
"#,
                );
            }
            source.push_str("      const seenKey = normalizeStringKey(keyValue);\n");
        }
    }
}

fn push_ts_duplicate_key_policy(source: &mut String, record: &SemanticManagerRecord) {
    let Some(policy) = record.key.as_ref().map(semantic_key_duplicate_policy) else {
        return;
    };
    match policy {
        crate::manager::NativeDuplicateKeyPolicy::FirstWins => source.push_str(
            r#"      if (seen.has(seenKey)) {
        continue;
      }
"#,
        ),
        crate::manager::NativeDuplicateKeyPolicy::Error => source.push_str(&format!(
            r#"      if (seen.has(seenKey)) {{
        throw new Error(`manager {} duplicate key ${{String(seenKey)}}`);
      }}
"#,
            record.manager_name
        )),
        crate::manager::NativeDuplicateKeyPolicy::Overwrite => {}
    }
}

fn semantic_key_duplicate_policy(
    key: &SemanticManagerKey,
) -> crate::manager::NativeDuplicateKeyPolicy {
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

fn push_ts_key_row_fields(source: &mut String, record: &SemanticManagerRecord) {
    let Some(key) = &record.key else {
        return;
    };
    match key {
        SemanticManagerKey::Crc {
            key_field,
            crc_field,
            ..
        } => source.push_str(&format!(
            "        {}: keyValue,\n        {}: keyCrc,\n",
            ts_field_name(key_field),
            ts_field_name(crc_field)
        )),
        SemanticManagerKey::FallbackCrc {
            key_kind_field,
            key_field,
            crc_field,
            ..
        } => source.push_str(&format!(
            "        {}: keyKind,\n        {}: keyValue,\n        {}: keyCrc,\n",
            ts_field_name(key_kind_field),
            ts_field_name(key_field),
            ts_field_name(crc_field)
        )),
        SemanticManagerKey::Numeric { key_field, .. }
        | SemanticManagerKey::EnumString { key_field, .. }
        | SemanticManagerKey::String { key_field, .. } => {
            source.push_str(&format!(
                "        {}: keyValue,\n",
                ts_field_name(key_field)
            ));
        }
    }
}

fn ts_numeric_key_value(
    table: &str,
    row: &str,
    column: &str,
    key_type: SemanticNumericKeyType,
) -> String {
    let column = typescript_string_literal(column);
    match key_type {
        SemanticNumericKeyType::U8 => format!("requiredUint8Cell({table}, {row}, {column})"),
        SemanticNumericKeyType::U16 => format!("requiredUint16Cell({table}, {row}, {column})"),
        SemanticNumericKeyType::U32 => format!("requiredUint32Cell({table}, {row}, {column})"),
    }
}

fn ts_projection_value(field: &crate::manager_records::SemanticRecordField) -> String {
    let column = typescript_string_literal(&field.column);
    match field.transform {
        SemanticProjectionTransform::String => {
            format!("requiredStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::StringDefaultEmpty => {
            format!("optionalStringCell(table, sourceRow, {column}) ?? \"\"")
        }
        SemanticProjectionTransform::PlusJoinedList => {
            format!("stringListCell(table, sourceRow, {column}).join(\"+\")")
        }
        SemanticProjectionTransform::OptionalString => {
            format!("optionalStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::StringList => {
            format!("stringListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::NonEmptyStringList => {
            format!(
                "stringListCell(table, sourceRow, {column}).filter((value) => value.length > 0)"
            )
        }
        SemanticProjectionTransform::OptionalStringList => {
            format!("optionalStringListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::Bool => {
            format!("requiredBoolCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalBool => {
            format!("optionalBoolCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U8 => {
            format!("requiredUint8Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U16 => {
            format!("requiredUint16Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U32 => {
            format!("requiredUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalU32 => {
            format!("optionalUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::I32 => {
            format!("requiredInt32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::F32 => {
            format!("requiredNumberCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalF32 => {
            format!("optionalNumberCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::F32List => {
            format!("numberListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::I32List => {
            format!(
                "numberListCell(table, sourceRow, {column}).map((value) => normalizeInt32(value))"
            )
        }
        SemanticProjectionTransform::Crc32 => {
            format!("requiredCrc32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::Crc32List => {
            format!("crc32ListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalLowercaseCrcString => {
            format!("optionalLowercaseCrcStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::LowercaseCrcStringList => {
            format!(
                "stringListCell(table, sourceRow, {column}).filter((value) => value.length > 0).map((value) => crc32Lowercase(value))"
            )
        }
        SemanticProjectionTransform::RowIndex => {
            format!("requiredUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalRowIndex => {
            format!("optionalUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::RowIndexList => {
            format!(
                "numberListCell(table, sourceRow, {column}).map((value) => normalizeUint32(value))"
            )
        }
        SemanticProjectionTransform::F32RangeInclusive => {
            format!("numberRangeCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U32RangeInclusive => {
            format!("uint32RangeCell(table, sourceRow, {column})")
        }
    }
}

const SEMANTIC_MANAGER_RUNTIME_TS: &str = r#"
function rowCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): DatasheetCellValue | undefined {
  const column = table.schema.columns.find((candidate) =>
    columnMatches(candidate, columnName),
  );
  if (column === undefined) {
    return undefined;
  }
  const slot = row.columnSlots.get(column.crc);
  return slot === undefined ? undefined : row.row.cells[slot]?.value;
}

function requiredStringCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): string {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing string ${columnName}`);
  }
  return stringCellValue(value);
}

function optionalStringCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): string | null {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return null;
  }
  const text = stringCellValue(value);
  return text.length === 0 ? null : text;
}

function stringCellValue(value: DatasheetCellValue): string {
  switch (value.kind) {
    case "string":
      return value.value;
    case "number":
      return String(value.value);
    case "boolean":
      return String(value.value);
  }
}

function requiredBoolCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): boolean {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing bool ${columnName}`);
  }
  const bool = boolCellValue(value, row, columnName);
  if (bool === null) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing bool ${columnName}`);
  }
  return bool;
}

function optionalBoolCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): boolean | null {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return null;
  }
  return boolCellValue(value, row, columnName);
}

function boolCellValue(
  value: DatasheetCellValue,
  row: DynamicTableRow,
  columnName: string,
): boolean | null {
  if (value.kind === "boolean") {
    return value.value;
  }
  if (value.kind === "number") {
    if (value.value === 0) {
      return false;
    }
    if (value.value === 1) {
      return true;
    }
  }
  if (value.kind === "string") {
    const text = value.value.trim().toLowerCase();
    if (text.length === 0) {
      return null;
    }
    if (text === "false" || text === "0" || text === "no") {
      return false;
    }
    if (text === "true" || text === "1" || text === "yes") {
      return true;
    }
  }
  throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has non-bool ${columnName}`);
}

function requiredNumberCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing number ${columnName}`);
  }
  const number = numberCellValue(value, row, columnName);
  if (number === null) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing number ${columnName}`);
  }
  return number;
}

function optionalNumberCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number | null {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return null;
  }
  return numberCellValue(value, row, columnName);
}

function numberCellValue(
  value: DatasheetCellValue,
  row: DynamicTableRow,
  columnName: string,
): number | null {
  if (value.kind === "number") {
    return value.value;
  }
  if (value.kind === "boolean") {
    return value.value ? 1 : 0;
  }
  const text = value.value.trim().toLowerCase();
  if (text.length === 0) {
    return null;
  }
  if (text === "false" || text === "no") {
    return 0;
  }
  if (text === "true" || text === "yes") {
    return 1;
  }
  const parsed = Number(text.replace(/f$/i, ""));
  if (Number.isFinite(parsed)) {
    return parsed;
  }
  throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has non-number ${columnName}`);
}

function requiredUint8Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  const value = requiredUint32Cell(table, row, columnName);
  if (value > 0xff) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} ${columnName} exceeds u8`);
  }
  return value;
}

function requiredUint16Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  const value = requiredUint32Cell(table, row, columnName);
  if (value > 0xffff) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} ${columnName} exceeds u16`);
  }
  return value;
}

function requiredUint32Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  return normalizeUint32(requiredNumberCell(table, row, columnName));
}

function optionalUint32Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number | null {
  const value = optionalNumberCell(table, row, columnName);
  return value === null ? null : normalizeUint32(value);
}

function requiredInt32Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  return normalizeInt32(requiredNumberCell(table, row, columnName));
}

function requiredCrc32Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  const value = rowCell(table, row, columnName);
  if (value?.kind === "number") {
    return normalizeCrcKey(value.value);
  }
  if (value?.kind === "string") {
    return crc32Lowercase(value.value);
  }
  throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing crc ${columnName}`);
}

function optionalLowercaseCrcStringCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number | null {
  const value = optionalStringCell(table, row, columnName);
  return value === null ? null : crc32Lowercase(value);
}

function stringListCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): string[] {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return [];
  }
  if (value.kind === "string") {
    return splitDesignerList(value.value);
  }
  if (value.kind === "number") {
    return [String(value.value)];
  }
  if (value.kind === "boolean") {
    return [value.value ? "true" : "false"];
  }
  throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has unsupported list ${columnName}`);
}

function optionalStringListCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): string[] | null {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return null;
  }
  const values = stringListCell(table, row, columnName);
  return values.length === 0 ? null : values;
}

function numberListCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number[] {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return [];
  }
  if (value.kind === "number") {
    return [value.value];
  }
  if (value.kind !== "string") {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has non-number-list ${columnName}`);
  }
  return splitDesignerList(value.value).map((part) => parseDesignerNumber(part, row, columnName));
}

function crc32ListCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number[] {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return [];
  }
  if (value.kind === "number") {
    return [normalizeCrcKey(value.value)];
  }
  if (value.kind !== "string") {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has non-crc-list ${columnName}`);
  }
  return splitDesignerList(value.value).map((part) => {
    const number = Number(part);
    return Number.isFinite(number) ? normalizeCrcKey(number) : crc32Lowercase(part);
  });
}

function numberRangeCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): [number, number] {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing range ${columnName}`);
  }
  const values =
    value.kind === "string"
      ? splitDesignerRange(value.value).map((part) => parseDesignerNumber(part, row, columnName))
      : numberListCell(table, row, columnName);
  if (values.length < 2) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing range ${columnName}`);
  }
  return [values[0], values[1]];
}

function uint32RangeCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): [number, number] {
  const [min, max] = numberRangeCell(table, row, columnName);
  return [normalizeUint32(min), normalizeUint32(max)];
}

function splitDesignerList(value: string): string[] {
  return value
    .split(/[,+]/)
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
}

function splitDesignerRange(value: string): string[] {
  const listed = splitDesignerList(value);
  if (listed.length >= 2) {
    return [listed[0], listed[1]];
  }
  const text = value.trim();
  for (let index = 1; index < text.length; index += 1) {
    if (text[index] !== "-") {
      continue;
    }
    const left = text.slice(0, index).trim();
    const right = text.slice(index + 1).trim();
    if (left.length > 0 && right.length > 0) {
      return [left, right];
    }
  }
  return listed;
}

function parseDesignerNumber(part: string, row: DynamicTableRow, columnName: string): number {
  const number = Number(part);
  if (!Number.isFinite(number)) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has invalid number in ${columnName}`);
  }
  return number;
}

function normalizeUint32(value: number): number {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new Error(`expected u32, got ${value}`);
  }
  return value >>> 0;
}

function normalizeInt32(value: number): number {
  if (!Number.isInteger(value) || value < -0x80000000 || value > 0x7fffffff) {
    throw new Error(`expected i32, got ${value}`);
  }
  return value | 0;
}

function normalizeCrcKey(value: number): number {
  return value >>> 0;
}

function normalizeNumericKey(value: number): number {
  return normalizeUint32(value);
}

function normalizeStringKey(value: string): string {
  return value.trim().toLowerCase();
}

const CRC32_TABLE = new Uint32Array(256);
for (let index = 0; index < 256; index += 1) {
  let crc = index;
  for (let bit = 0; bit < 8; bit += 1) {
    crc = (crc & 1) !== 0 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
  }
  CRC32_TABLE[index] = crc >>> 0;
}

function crc32Lowercase(value: string): number {
  const bytes = new TextEncoder().encode(value);
  let crc = 0xffffffff;
  for (const byte of bytes) {
    const lower = byte >= 65 && byte <= 90 ? byte + 32 : byte;
    crc = CRC32_TABLE[(crc ^ lower) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

"#;

const PRODUCT_MANAGER_RUNTIME_TS: &str = r#"
export type Vec3 = ObjectStreamVec3;

export const ZERO_VEC3: Vec3 = { x: 0, y: 0, z: 0 };
export const ALL_INTERACT_OPTIONS_CATEGORY = 0x15;

export interface ArmorOffsetDatabase {
  readonly offsets: readonly ArmorOffsetData[];
}

export interface ArmorOffsetData {
  readonly name: string;
  readonly attachments: readonly AttachmentOffsetData[];
}

export interface AttachmentOffsetData {
  readonly attachment: string;
  readonly position: Vec3;
  readonly rotationDegrees: Vec3;
}

export interface EquipTypesDatabase {
  readonly equipTypes: readonly EquipTypeData[];
}

export interface EquipTypeData {
  readonly name: string;
  readonly attachment: string;
  readonly attachmentOffsetPosition: Vec3;
  readonly attachmentOffsetRotationDegrees: Vec3;
  readonly sheathData: string;
  readonly sheathOffsetPosition: Vec3;
  readonly sheathOffsetRotationDegrees: Vec3;
  readonly offHandAttachment: string;
  readonly offHandAttachmentOffsetPosition: Vec3;
  readonly offHandAttachmentOffsetRotationDegrees: Vec3;
  readonly offHandSheathData: string;
  readonly offHandSheathOffsetPosition: Vec3;
  readonly offHandSheathOffsetRotationDegrees: Vec3;
}

export interface GameDebugSettings {
  readonly combatSettings: CombatDebugSettings;
}

export interface CombatDebugSettings {
  readonly disablePlayerLootDropOnDeath: boolean;
  readonly disableWeaponDurability: boolean;
  readonly disableItemDurability: boolean;
  readonly disableDurabilityPenaltyOnDeath: boolean;
}

export interface UiDatabase {
  readonly unifiedInteractData: UnifiedInteractData;
}

export interface UnifiedInteractData {
  readonly interactOptions: readonly InteractOptionData[];
}

export interface DelayedInteractionData {
  readonly delayTime: number;
  readonly delayMannequinTag: string;
}

export interface EffectData {
  readonly effectId: string;
}

export interface InteractOptionData {
  readonly name: string;
  readonly displayName: string;
  readonly interactInputType: number;
  readonly uiInteractAction: number;
  readonly additionalInfoType: number;
  readonly interactOptionCategory: number;
  readonly delayedInteractionData: DelayedInteractionData;
  readonly interactPrivilegeIds: readonly number[];
  readonly blueprintPrivilegeId: number;
  readonly requiresConfirmation: boolean;
  readonly isCommittedInteraction: boolean;
  readonly isInstantCancel: boolean;
  readonly closePromptOnInteraction: boolean;
  readonly forceSecondaryInteract: boolean;
  readonly onlyShowIfBoundToCamp: boolean;
  readonly displayPriority: number;
  readonly interactOptionIcon: string;
  readonly uiAdditionalInfoSlicePath: string;
  readonly requiresSecurityLevelValidation: boolean;
  readonly mannequinFragment: string;
  readonly mannequinTag: string;
  readonly alignToInteraction: boolean;
  readonly holdActionPressTime: number;
  readonly cooldownTime: number;
  readonly setOwnershipOnInteract: boolean;
  readonly requiredItemName: string;
  readonly requiredItemCount: number;
  readonly requiredCurrency: number;
  readonly availability: number;
  readonly siegeWarfareGameEventName: string;
  readonly addedStatusEffects: readonly EffectData[];
  readonly requiredStatusEffects: readonly EffectData[];
  readonly removeStatusEffects: readonly EffectData[];
  readonly excludedStatusEffects: readonly EffectData[];
  readonly delayBeforeAddingRemovingEffect: number;
  readonly removeAddedEffectsOnInteractionEnd: boolean;
  readonly checkPvpFlagIsSet: boolean;
  readonly factionRequired: boolean;
  readonly showInstancedLootItemCount: boolean;
  readonly requiredAchievementName: string;
  readonly requiredLevel: number;
  readonly committedInteractionMaxUsageTimeout: number;
  readonly committedInteractionMaxUsageTimeoutNotification: string;
  readonly committedInteractionInactiveTimeout: number;
  readonly committedInteractionInactiveTimeoutNotification: string;
}

export interface GameCameraSettings {
  readonly defaultStateName: string;
  readonly fields: Readonly<Record<string, string>>;
  readonly cameraStates: readonly CameraStateSettings[];
}

export interface CameraStateSettings {
  readonly name: string;
  readonly include?: string;
  readonly fields: Readonly<Record<string, string>>;
  readonly fromTransitions: readonly CameraStateTransition[];
}

export interface CameraStateTransition {
  readonly fromCamera: string;
  readonly smoothTime?: number;
}

export type TradeskillType =
  | "None"
  | "Weaponsmithing"
  | "Armoring"
  | "Jewelcrafting"
  | "Arcana"
  | "Cooking"
  | "Furnishing"
  | "Engineering"
  | "Smelting"
  | "Woodworking"
  | "Leatherworking"
  | "Weaving"
  | "Stonecutting"
  | "Skinning"
  | "Mining"
  | "Logging"
  | "Harvesting"
  | "WildernessSurvival"
  | "Fishing"
  | "AzothStaff"
  | "Musician"
  | "Riding";

export interface EditCrc {
  readonly valueStr: string;
  readonly valueCrc: number;
}

export interface ColorRgba {
  readonly r: number;
  readonly g: number;
  readonly b: number;
  readonly a: number;
}

export interface IntRange {
  readonly min: number;
  readonly max: number;
}

export interface AssetReference {
  readonly guid: string;
  readonly subId: number;
  readonly assetType: string;
  readonly hint: string;
}

export interface SimpleAssetReferenceTextureAsset {
  readonly assetPath: string;
}

export interface PlayerBaseAttributes {
  readonly playerAttributeData: PlayerAttributeData;
  readonly guildSiegeWindowRegionData: ReadonlyMap<string, GuildSiegeWindowRegionData>;
  readonly factionInfluenceConfigData: FactionInfluenceConfigData;
  readonly validGroupData: ValidGroupData;
  readonly warData: WarData;
}

export interface PlayerAttributeData {
  readonly baseDeployableLimit: number;
  readonly playerDisplayLevelUnlockFreeGearSets: number;
  readonly itemRarityData: readonly ItemRarityData[];
  readonly perkGenerationData: PerkGenerationData;
  readonly perkChanceItemId: string;
  readonly abilityPointsRequiredInTreeToUnlockFinalRow: number;
  readonly perkChanceModifier: number;
  readonly attributeChanceModifier: number;
  readonly gemSlotChanceModifier: number;
}

export interface ItemRarityData {
  readonly rarityLevelLocString: string;
  readonly maxPerkCount: number;
  readonly levelRequirementModifier: number;
}

export interface PerkGenerationData {
  readonly perkDataPerTier: readonly PerkTierData[];
  readonly craftingResultLootBucketId: number;
  readonly craftingResultLootBucket: string;
  readonly rollPerkOnUpgradeGs: number;
  readonly rollPerkOnUpgradeTier: number;
  readonly rollPerkOnUpgradePerkCount: number;
}

export interface PerkTierData {
  readonly maxPerkChannel: number;
  readonly gemSlotProbability: number;
  readonly attributePerkProbability: number;
  readonly generalGearScorePerkCount: ReadonlyMap<number, readonly IntRange[]>;
  readonly craftingGearScorePerkCount: ReadonlyMap<number, readonly IntRange[]>;
  readonly attributePerkBucket: string;
  readonly attributePerkBucketId: number;
}

export interface GuildSiegeWindowRegionData {
  readonly startHour: number;
  readonly endHour: number;
  readonly utcOffset: number;
  readonly dstRuleId: number;
  readonly dstRule: string;
  readonly observesDst: boolean;
}

export interface FactionInfluenceConfigData {
  readonly maxInfluence: number;
  readonly decrementRate: number;
  readonly incrementRate: number;
  readonly maxIncrementTimeModifier: number;
  readonly maxDecrementTimeModifier: number;
  readonly minimumTimeSinceLastWar: number;
  readonly minTerritoryDiffToApplyUdMechanics: number;
  readonly minTimeToApplyUdMechanics: number;
  readonly underDogMissionInfluenceGain: number;
  readonly underDogMissionInfluenceGainCap: number;
  readonly uderDogFactionRepGain: number;
  readonly underDogFactionRepGainCap: number;
  readonly underDogPvpInfluenceGain: number;
  readonly underDogPvpInfluenceGainCap: number;
  readonly minimumInfluenceThresholdForWar: number;
  readonly influenceRaceAttackerWinGameEventId: EditCrc;
  readonly influenceRaceDefenderWinGameEventId: EditCrc;
  readonly influenceRaceLoseGameEventId: EditCrc;
}

export interface ValidGroupData {
  readonly names: readonly string[];
  readonly objectives: readonly string[];
  readonly iconPaths: readonly string[];
  readonly colors: readonly ColorRgba[];
}

export interface WarData {
  readonly deployableLimits: ReadonlyMap<number, WarDeployableLimitData>;
}

export interface WarDeployableLimitData {
  readonly id: number;
  readonly displayName: string;
  readonly buildableNames: readonly string[];
  readonly buildableIds: readonly number[];
  readonly attackerLimits: readonly [number, number, number];
  readonly defenderLimit: number;
}

export interface SettlementProgressionData {
  readonly settlementProgressionCategories: readonly ProgressionCategoryEntry[];
}

export interface ProgressionCategoryEntry {
  readonly settlementProgressionCategory: string;
  readonly settlementProgressionEntries: readonly ProgressionSpawnerEntry[];
}

export interface ProgressionSpawnerEntry {
  readonly settlementProgressionCategoryLevel: number;
  readonly slice: AssetReference;
  readonly alternateSlice: AssetReference;
  readonly displayLocString: string;
  readonly icon: SimpleAssetReferenceTextureAsset;
}

export interface GatheringDatabase {
  readonly gatheringData: GatheringData;
}

export interface GatheringData {
  readonly gatheringTypes: readonly GatheringTypeData[];
  readonly gatheringActions: readonly GatheringAction[];
  readonly requiredWaterGatheringType: string;
  readonly noneGatheringType: string;
}

export interface GatheringTypeData {
  readonly gatheringType: string;
  readonly uiIcon: SimpleAssetReferenceTextureAsset;
  readonly requirementText: string;
}

export interface GatheringAction {
  readonly name: string;
  readonly mannequinTag: string;
}

export interface GatheringActionDatabase {
  readonly gatheringActions: readonly GatheringActionData[];
}

export interface GatheringActionData {
  readonly name: string;
  readonly mannequinTag: string;
}

export interface CraftingStationDatabase {
  readonly craftingStations: readonly CraftingStationData[];
}

export interface CraftingStationData {
  readonly name: string;
  readonly craftingTypes: readonly string[];
  readonly mannequinTag: string;
  readonly azothDiscountPercent: number;
}

export interface SocialRankDatabase {
  readonly ranks: readonly SocialRankData[];
}

export interface SocialRankData {
  readonly guildRankData: SocialGuildRankData;
}

export interface SocialGuildRankData {
  readonly name: string;
  readonly securityLevel: number;
  readonly allPrivileges: boolean;
  readonly privilegeIds: readonly number[];
}

const AZSTD_STRING_TYPE_ID = "03aaab3f-5c47-5a66-9ebc-d5fa4db353c9";
const VECTOR3_TYPE_ID = "8379eb7d-01fa-4538-b64b-a6543b4be73d";
const BOOL_TYPE_ID = "a0ca880c-afe4-43cb-926c-59ac48496112";
const U8_TYPE_ID = "72b9409a-7d1a-4831-9cfe-fcb3fadd3426";
const U32_TYPE_ID = "43da906b-7def-4ca8-9790-854106d3f983";
const INT_TYPE_ID = "72039442-eb38-4d42-a1ad-cb68f7e0eef6";
const FLOAT_TYPE_ID = "ea2c3e90-afbe-44d4-a90d-faaf79baf93d";
const CRC32_TYPE_ID = "9f4e062e-06a0-46d4-85df-e0da96467d3a";
const COLOR_TYPE_ID = "7894072a-9050-4f0f-901b-34b1a0d29417";
const ASSET_TYPE_ID = "77a19d40-8731-4d3c-9041-1b43047366a4";
const EDIT_CRC_TYPE_ID = "9a339de9-0d6e-4708-922f-f46af04370e9";
const SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID = "68e92460-5c0c-4031-9620-6f1a08763243";
const SIMPLE_ASSET_REFERENCE_BASE_TYPE_ID = "e16ca6c5-5c78-4ad9-8e9b-f8c1fb4d1db8";

const ARMOR_OFFSET_DATABASE_TYPE_ID = "8c1fa8a8-2e58-4791-acda-2c3625318832";
const ARMOR_OFFSET_VECTOR_TYPE_ID = "d276dfb3-a8ec-58c2-96e2-145bc5a6ee3d";
const ARMOR_OFFSET_DATA_TYPE_ID = "13b87761-89ab-4a4b-a370-dad3875380da";
const ATTACHMENT_OFFSET_VECTOR_TYPE_ID = "8b83aa0c-520e-5074-8c4e-5ad60c3d70fe";
const ATTACHMENT_OFFSET_DATA_TYPE_ID = "fc296230-5f66-473e-90c8-66ad7028fd07";
const ARMOR_OFFSETS_FIELD_CRC = 2282200990;
const ARMOR_OFFSET_NAME_FIELD_CRC = 1579384326;
const ARMOR_OFFSET_ATTACHMENTS_FIELD_CRC = 1204091606;
const ATTACHMENT_NAME_FIELD_CRC = 2036324795;
const ATTACHMENT_OFFSET_POSITION_FIELD_CRC = 379390882;
const ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC = 581018980;

const EQUIP_TYPES_DATABASE_TYPE_ID = "f937c753-ffc0-4f9c-a234-7c71c9a5bdb3";
const EQUIP_TYPE_DATA_VECTOR_TYPE_ID = "53de1751-3981-5da4-8f72-f9e5712b3127";
const EQUIP_TYPE_DATA_TYPE_ID = "0386d9b0-3e95-411f-823f-4a800c74f7ed";
const EQUIP_TYPES_FIELD_CRC = 1388966666;
const EQUIP_NAME_FIELD_CRC = 1579384326;
const EQUIP_ATTACHMENT_FIELD_CRC = 2036324795;
const EQUIP_ATTACHMENT_OFFSET_POSITION_FIELD_CRC = 379390882;
const EQUIP_ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC = 581018980;
const EQUIP_SHEATH_DATA_FIELD_CRC = 1966399264;
const EQUIP_SHEATH_OFFSET_POSITION_FIELD_CRC = 619916990;
const EQUIP_SHEATH_OFFSET_ROTATION_DEGREES_FIELD_CRC = 768083228;
const EQUIP_OFF_HAND_ATTACHMENT_FIELD_CRC = 2388996306;
const EQUIP_OFF_HAND_ATTACHMENT_OFFSET_POSITION_FIELD_CRC = 2522934056;
const EQUIP_OFF_HAND_ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC = 97015342;
const EQUIP_OFF_HAND_SHEATH_DATA_FIELD_CRC = 1101872181;
const EQUIP_OFF_HAND_SHEATH_OFFSET_POSITION_FIELD_CRC = 1077303719;
const EQUIP_OFF_HAND_SHEATH_OFFSET_ROTATION_DEGREES_FIELD_CRC = 789454983;

const GAME_DEBUG_SETTINGS_TYPE_ID = "3e5db037-ae49-43e4-8bcc-67f8c511a091";
const COMBAT_DEBUG_SETTINGS_TYPE_ID = "3c0e5dc7-06b9-4411-893e-daac101731d3";
const COMBAT_SETTINGS_FIELD_CRC = 3204566528;
const DISABLE_PLAYER_LOOT_DROP_ON_DEATH_FIELD_CRC = 76657494;
const DISABLE_WEAPON_DURABILITY_FIELD_CRC = 2559298940;
const DISABLE_ITEM_DURABILITY_FIELD_CRC = 880532799;
const DISABLE_DURABILITY_PENALTY_ON_DEATH_FIELD_CRC = 429903575;

const UI_DATABASE_TYPE_ID = "7cc2b992-1c5b-4b27-bcb9-790175f09da6";
const UNIFIED_INTERACT_DATA_TYPE_ID = "ebc0595e-4adb-4323-9527-82d07e30908c";
const INTERACT_OPTION_VECTOR_TYPE_ID = "33d6e083-a124-527f-baac-824deb5cd6e8";
const INTERACT_OPTION_DATA_TYPE_ID = "f0887e97-5084-413c-bce7-5c24cecb03c0";

const PLAYER_BASE_ATTRIBUTES_TYPE_ID = "0f40ecc6-ace9-476a-9a5c-b83be6129a4b";
const PLAYER_ATTRIBUTE_DATA_TYPE_ID = "46113bed-540d-4584-92aa-b9223d83875a";
const GUILD_SIEGE_WINDOW_REGION_DATA_TYPE_ID = "da0aab24-92a0-5ea4-9a1a-5cef4e8c3bf9";
const FACTION_INFLUENCE_CONFIG_DATA_TYPE_ID = "8ed959c4-b0e3-4d45-84d1-fcaec1c7d1a4";
const VALID_GROUP_DATA_TYPE_ID = "4f986681-3060-4a47-9a45-694a027e5f46";
const WAR_DATA_TYPE_ID = "4febcf31-140c-4ef1-8c53-814daa4426ac";

const SETTLEMENT_PROGRESSION_DATA_TYPE_ID = "0543759c-4cf0-4eba-b0dd-f0f020b480b3";
const PROGRESSION_CATEGORY_ENTRY_TYPE_ID = "e1766b2b-75fd-4eb2-ab13-0e5f343b7e68";
const PROGRESSION_SPAWNER_ENTRY_TYPE_ID = "d91778a1-a110-46e4-8b9a-30402d8996d6";
const SETTLEMENT_PROGRESSION_CATEGORY_VECTOR_TYPE_ID = "2d93cc0a-78e0-5fdf-af40-c2f0491facd0";
const PROGRESSION_SPAWNER_ENTRY_VECTOR_TYPE_ID = "3999d332-be04-5382-9e40-a2bf965e61eb";
const SETTLEMENT_PROGRESSION_CATEGORIES_FIELD_CRC = 2439926458;
const SETTLEMENT_PROGRESSION_CATEGORY_FIELD_CRC = 1188522208;
const SETTLEMENT_PROGRESSION_ENTRIES_FIELD_CRC = 1770189871;
const SETTLEMENT_PROGRESSION_CATEGORY_LEVEL_FIELD_CRC = 2587150535;
const SLICE_FIELD_CRC = 1034844325;
const ALTERNATE_SLICE_FIELD_CRC = 1867428434;
const DISPLAY_LOC_STRING_FIELD_CRC = 457484292;
const ICON_FIELD_CRC = 1704208859;
const BASE_CLASS_FIELD_CRC = 3566360373;
const ASSET_PATH_FIELD_CRC = 741691769;

const GATHERING_DATABASE_TYPE_ID = "1ef311cc-a16e-426d-9763-a40473495330";
const GATHERING_DATA_TYPE_ID = "579abcc6-ec1e-4157-abc5-2569c7624b0a";
const GATHERING_ACTION_DATABASE_TYPE_ID = "9ac82655-bc8f-4165-ae2f-6d6f3d543d9a";
const GATHERING_ACTION_DATA_TYPE_ID = "a6b5258c-2984-4225-88e9-b66813457286";
const GATHERING_ACTION_TYPE_ID = "5cfd353d-418d-4421-a207-2c748cfbdd16";
const GATHERING_TYPE_DATA_TYPE_ID = "3266a19a-6bac-4703-b663-9f6ed48f1d76";
const GATHERING_TYPE_DATA_VECTOR_TYPE_ID = "779755e7-d85d-5d47-91d5-5fdbb851da57";
const GATHERING_ACTION_VECTOR_TYPE_ID = "0c5b29e6-74c4-5adf-8fcf-c3204a7e4662";
const GATHERING_ACTION_DATA_VECTOR_TYPE_ID = "ceef81af-b476-5463-af1e-b7ec9f65c02f";
const GATHERING_DATA_FIELD_CRC = 2208564949;
const GATHERING_TYPES_FIELD_CRC = 2065483900;
const GATHERING_ACTIONS_FIELD_CRC = 1482662604;
const REQUIRED_WATER_GATHERING_TYPE_FIELD_CRC = 674599067;
const NONE_GATHERING_TYPE_FIELD_CRC = 3194172210;
const TYPE_FIELD_CRC = 2363381545;
const UI_ICON_FIELD_CRC = 2312546211;
const REQUIREMENT_TEXT_FIELD_CRC = 2484547296;
const NAME_FIELD_CRC = 1579384326;
const MANNEQUIN_TAG_FIELD_CRC = 2777524544;

const CRAFTING_STATION_DATABASE_TYPE_ID = "72175d3e-9370-4b21-970f-dc2adc11e52b";
const CRAFTING_STATION_DATA_VECTOR_TYPE_ID = "866eb75e-8cfd-511b-a4f0-b8dfa17138bd";
const CRAFTING_STATION_DATA_TYPE_ID = "75cfb9e3-fe11-4d1d-ac0a-44916a5c27a0";
const CRAFTING_TYPE_STRING_VECTOR_TYPE_ID = "99dad0bc-740e-5e82-826b-8fc7968cc02c";
const CRAFTING_STATIONS_FIELD_CRC = 2156395334;
const CRAFTING_TYPES_FIELD_CRC = 169774472;
const CRAFTING_MANNEQUIN_TAG_FIELD_CRC = 1024826923;
const AZOTH_DISCOUNT_PERCENT_FIELD_CRC = 757151162;

const SOCIAL_RANK_DATABASE_TYPE_ID = "b0024f1f-651d-48a5-a56a-9dea80cb487e";
const SOCIAL_RANK_DATA_VECTOR_TYPE_ID = "1297b8af-3355-5871-914e-82178f34b16e";
const SOCIAL_RANK_DATA_TYPE_ID = "2f2c2714-e932-43bf-a702-cacd8c9ae544";
const SOCIAL_GUILD_RANK_DATA_TYPE_ID = "e756a995-93ed-f487-1a76-23b1ad74df11";
const SOCIAL_PRIVILEGE_ID_SET_TYPE_ID = "4c9c7f67-4b86-58af-b45a-abf4d2eae45f";
const SOCIAL_RANKS_FIELD_CRC = 3420889108;
const SOCIAL_GUILD_RANK_DATA_FIELD_CRC = 2999919934;
const SOCIAL_GUILD_RANK_NAME_FIELD_CRC = 3230417959;
const SOCIAL_GUILD_RANK_SECURITY_LEVEL_FIELD_CRC = 265698600;
const SOCIAL_GUILD_RANK_ALL_PRIVILEGES_FIELD_CRC = 928054442;
const SOCIAL_GUILD_RANK_PRIVILEGE_IDS_FIELD_CRC = 2614315740;

const TRADESKILL_TYPES: readonly TradeskillType[] = [
  "Weaponsmithing",
  "Armoring",
  "Jewelcrafting",
  "Arcana",
  "Cooking",
  "Furnishing",
  "Engineering",
  "Smelting",
  "Woodworking",
  "Leatherworking",
  "Weaving",
  "Stonecutting",
  "Skinning",
  "Mining",
  "Logging",
  "Harvesting",
  "WildernessSurvival",
  "Fishing",
  "AzothStaff",
  "Musician",
  "Riding",
];

const PRODUCT_TEXT_DECODER = new TextDecoder();

function normalizeTradeskillType(value: string | number): TradeskillType {
  if (typeof value === "number") {
    if (value === 255) {
      return "None";
    }
    const tradeskill = TRADESKILL_TYPES[value];
    if (tradeskill === undefined) {
      throw new Error(`unknown TradeskillType value ${value}`);
    }
    return tradeskill;
  }
  const normalized = String(value).trim();
  if (normalized === "None" || TRADESKILL_TYPES.includes(normalized as TradeskillType)) {
    return normalized as TradeskillType;
  }
  throw new Error(`unknown TradeskillType ${value}`);
}

function parsePlayerBaseAttributes(bytes: Uint8Array): PlayerBaseAttributes {
  const root = strictObjectStreamRoot(bytes, PLAYER_BASE_ATTRIBUTES_TYPE_ID);
  return {
    playerAttributeData: parsePlayerAttributeData(
      requiredSection(root, "Player Attribute Data", PLAYER_ATTRIBUTE_DATA_TYPE_ID),
    ),
    guildSiegeWindowRegionData: parseGuildRegions(
      requiredSection(root, "Guild Siege Window Region Data", GUILD_SIEGE_WINDOW_REGION_DATA_TYPE_ID),
    ),
    factionInfluenceConfigData: parseFactionInfluenceConfig(
      requiredSection(root, "Faction Influence Config Data", FACTION_INFLUENCE_CONFIG_DATA_TYPE_ID),
    ),
    validGroupData: parseValidGroupData(requiredSection(root, "Valid Group Data", VALID_GROUP_DATA_TYPE_ID)),
    warData: parseWarData(requiredSection(root, "War Data", WAR_DATA_TYPE_ID)),
  };
}

function parsePlayerAttributeData(element: ObjectStreamElement): PlayerAttributeData {
  return {
    baseDeployableLimit: requiredI32FieldByName(element, "Base Deployable Limit"),
    playerDisplayLevelUnlockFreeGearSets: requiredI32FieldByName(
      element,
      "Player Display Level Unlock Free Gear Sets",
    ),
    itemRarityData: requiredFieldByName(element, "Item Rarity Data").children.map(parseItemRarityData),
    perkGenerationData: parsePerkGenerationData(requiredFieldByName(element, "Perk Generation Data")),
    perkChanceItemId: requiredStringFieldByName(element, "Perk Chance ItemId"),
    abilityPointsRequiredInTreeToUnlockFinalRow: requiredI32FieldByName(
      element,
      "Ability Points Required In Tree to Unlock Final Row",
    ),
    perkChanceModifier: requiredF32FieldByName(element, "Perk Chance Modifier"),
    attributeChanceModifier: requiredF32FieldByName(element, "Attribute Chance Modifier"),
    gemSlotChanceModifier: requiredF32FieldByName(element, "Gem Slot Chance Modifier"),
  };
}

function parseItemRarityData(element: ObjectStreamElement): ItemRarityData {
  return {
    rarityLevelLocString: requiredStringFieldByName(element, "Rarity Level Loc String"),
    maxPerkCount: requiredI32FieldByName(element, "Max Perk Count"),
    levelRequirementModifier: requiredI32FieldByName(element, "Level Requirement Modifier"),
  };
}

function parsePerkGenerationData(element: ObjectStreamElement): PerkGenerationData {
  return {
    perkDataPerTier: requiredFieldByName(element, "Perk Data Per Tier").children.map(parsePerkTierData),
    craftingResultLootBucketId: requiredCrc32FieldByName(element, "Crafting Result Loot Bucket Id"),
    craftingResultLootBucket: requiredStringFieldByName(element, "Crafting Result Loot Bucket"),
    rollPerkOnUpgradeGs: requiredI32FieldByName(element, "Roll Perk On Upgrade GS"),
    rollPerkOnUpgradeTier: requiredI32FieldByName(element, "Roll Perk On Upgrade Tier"),
    rollPerkOnUpgradePerkCount: requiredI32FieldByName(element, "Roll Perk On Upgrade Perk Count"),
  };
}

function parsePerkTierData(element: ObjectStreamElement): PerkTierData {
  return {
    maxPerkChannel: requiredI32FieldByName(element, "Max Perk Channel"),
    gemSlotProbability: requiredF32FieldByName(element, "Gem Slot Probability"),
    attributePerkProbability: requiredF32FieldByName(element, "Attribute Perk Probability"),
    generalGearScorePerkCount: parseI32RangeMap(
      requiredFieldByName(element, "General Gear Score Perk Count"),
    ),
    craftingGearScorePerkCount: parseI32RangeMap(
      requiredFieldByName(element, "Crafting Gear Score Perk Count"),
    ),
    attributePerkBucket: requiredStringFieldByName(element, "Attribute Perk Bucket"),
    attributePerkBucketId: requiredCrc32FieldByName(element, "Attribute Perk Bucket Id"),
  };
}

function parseI32RangeMap(element: ObjectStreamElement): ReadonlyMap<number, readonly IntRange[]> {
  const out = new Map<number, readonly IntRange[]>();
  for (const pair of element.children) {
    const key = requiredI32FieldByName(pair, "value1");
    const ranges = requiredFieldByName(pair, "value2").children.map((range) => ({
      min: requiredI32FieldByName(range, "value1"),
      max: requiredI32FieldByName(range, "value2"),
    }));
    out.set(key, ranges);
  }
  return out;
}

function parseGuildRegions(element: ObjectStreamElement): ReadonlyMap<string, GuildSiegeWindowRegionData> {
  const out = new Map<string, GuildSiegeWindowRegionData>();
  for (const pair of element.children) {
    out.set(
      requiredStringFieldByName(pair, "value1"),
      parseGuildRegion(requiredFieldByName(pair, "value2")),
    );
  }
  return out;
}

function parseGuildRegion(element: ObjectStreamElement): GuildSiegeWindowRegionData {
  return {
    startHour: requiredU32FieldByName(element, "Start Hour"),
    endHour: requiredU32FieldByName(element, "End Hour"),
    utcOffset: requiredI32FieldByName(element, "UTCOffset"),
    dstRuleId: requiredCrc32FieldByName(element, "DstRuleId"),
    dstRule: requiredStringFieldByName(element, "DstRule"),
    observesDst: requiredBoolFieldByName(element, "ObservesDst"),
  };
}

function parseFactionInfluenceConfig(element: ObjectStreamElement): FactionInfluenceConfigData {
  return {
    maxInfluence: requiredF32FieldByName(element, "MaxInfluence"),
    decrementRate: requiredF32FieldByName(element, "DecrementRate"),
    incrementRate: requiredF32FieldByName(element, "IncrementRate"),
    maxIncrementTimeModifier: requiredF32FieldByName(element, "MaxIncrementTimeModifier"),
    maxDecrementTimeModifier: requiredF32FieldByName(element, "MaxDecrementTimeModifier"),
    minimumTimeSinceLastWar: requiredF32FieldByName(element, "MinimumTimeSinceLastWar"),
    minTerritoryDiffToApplyUdMechanics: requiredI32FieldByName(element, "MinTerritoryDiffToApplyUDMechanics"),
    minTimeToApplyUdMechanics: requiredI32FieldByName(element, "MinTimeToApplyUDMechanics"),
    underDogMissionInfluenceGain: requiredF32FieldByName(element, "UnderDogMissionInfluenceGain"),
    underDogMissionInfluenceGainCap: requiredF32FieldByName(element, "UnderDogMissionInfluenceGainCap"),
    uderDogFactionRepGain: requiredF32FieldByName(element, "UderDogFactionRepGain"),
    underDogFactionRepGainCap: requiredF32FieldByName(element, "UnderDogFactionRepGainCap"),
    underDogPvpInfluenceGain: requiredF32FieldByName(element, "UnderDogPVPInfluenceGain"),
    underDogPvpInfluenceGainCap: requiredF32FieldByName(element, "UnderDogPVPInfluenceGainCap"),
    minimumInfluenceThresholdForWar: requiredF32FieldByName(element, "MinimumInfluenceThresholdForWar"),
    influenceRaceAttackerWinGameEventId: parseEditCrc(
      requiredFieldByName(element, "Influence Race Attacker Win GameEventId"),
    ),
    influenceRaceDefenderWinGameEventId: parseEditCrc(
      requiredFieldByName(element, "Influence Race Defender Win GameEventId"),
    ),
    influenceRaceLoseGameEventId: parseEditCrc(
      requiredFieldByName(element, "Influence Race Lose GameEventId"),
    ),
  };
}

function parseValidGroupData(element: ObjectStreamElement): ValidGroupData {
  return {
    names: requiredStringSequenceByName(element, "names"),
    objectives: requiredStringSequenceByName(element, "Objectives"),
    iconPaths: requiredStringSequenceByName(element, "IconPaths"),
    colors: requiredFieldByName(element, "Colors").children.map(readColorRgba),
  };
}

function parseWarData(element: ObjectStreamElement): WarData {
  const deployableLimits = new Map<number, WarDeployableLimitData>();
  for (const pair of requiredFieldByName(element, "Deployable Limits").children) {
    deployableLimits.set(
      requiredCrc32FieldByName(pair, "value1"),
      parseWarDeployableLimit(requiredFieldByName(pair, "value2")),
    );
  }
  return { deployableLimits };
}

function parseWarDeployableLimit(element: ObjectStreamElement): WarDeployableLimitData {
  return {
    id: requiredCrc32FieldByName(element, "m_id"),
    displayName: requiredStringFieldByName(element, "m_displayName"),
    buildableNames: requiredStringSequenceByName(element, "m_buildableNames"),
    buildableIds: requiredCrc32SequenceByName(element, "m_buildableIds"),
    attackerLimits: readI32Triple(requiredFieldByName(element, "m_attackerLimits")),
    defenderLimit: requiredI32FieldByName(element, "m_defenderLimit"),
  };
}

function parseSettlementProgressionData(bytes: Uint8Array): SettlementProgressionData {
  const root = strictObjectStreamRoot(bytes, SETTLEMENT_PROGRESSION_DATA_TYPE_ID);
  const categories = requiredTypedChild(
    root,
    SETTLEMENT_PROGRESSION_CATEGORIES_FIELD_CRC,
    SETTLEMENT_PROGRESSION_CATEGORY_VECTOR_TYPE_ID,
  );
  return {
    settlementProgressionCategories: categories.children.map(parseProgressionCategoryEntry),
  };
}

function parseProgressionCategoryEntry(element: ObjectStreamElement): ProgressionCategoryEntry {
  requireObjectStreamType(element, PROGRESSION_CATEGORY_ENTRY_TYPE_ID);
  const entries = requiredTypedChild(
    element,
    SETTLEMENT_PROGRESSION_ENTRIES_FIELD_CRC,
    PROGRESSION_SPAWNER_ENTRY_VECTOR_TYPE_ID,
  );
  return {
    settlementProgressionCategory: requiredStringField(element, SETTLEMENT_PROGRESSION_CATEGORY_FIELD_CRC),
    settlementProgressionEntries: entries.children.map(parseProgressionSpawnerEntry),
  };
}

function parseProgressionSpawnerEntry(element: ObjectStreamElement): ProgressionSpawnerEntry {
  requireObjectStreamType(element, PROGRESSION_SPAWNER_ENTRY_TYPE_ID);
  return {
    settlementProgressionCategoryLevel: requiredI32Field(element, SETTLEMENT_PROGRESSION_CATEGORY_LEVEL_FIELD_CRC),
    slice: readAssetReference(requiredTypedChild(element, SLICE_FIELD_CRC, ASSET_TYPE_ID)),
    alternateSlice: readAssetReference(requiredTypedChild(element, ALTERNATE_SLICE_FIELD_CRC, ASSET_TYPE_ID)),
    displayLocString: requiredStringField(element, DISPLAY_LOC_STRING_FIELD_CRC),
    icon: readTextureReference(requiredTypedChild(element, ICON_FIELD_CRC, SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID)),
  };
}

function parseGatheringDatabase(bytes: Uint8Array): GatheringDatabase {
  const root = strictObjectStreamRoot(bytes, GATHERING_DATABASE_TYPE_ID);
  const data = requiredTypedChild(root, GATHERING_DATA_FIELD_CRC, GATHERING_DATA_TYPE_ID);
  return { gatheringData: parseGatheringData(data) };
}

function parseGatheringData(element: ObjectStreamElement): GatheringData {
  const types = requiredTypedChild(element, GATHERING_TYPES_FIELD_CRC, GATHERING_TYPE_DATA_VECTOR_TYPE_ID);
  const actions = requiredTypedChild(element, GATHERING_ACTIONS_FIELD_CRC, GATHERING_ACTION_VECTOR_TYPE_ID);
  return {
    gatheringTypes: types.children.map(parseGatheringTypeData),
    gatheringActions: actions.children.map(parseGatheringAction),
    requiredWaterGatheringType: requiredStringField(element, REQUIRED_WATER_GATHERING_TYPE_FIELD_CRC),
    noneGatheringType: requiredStringField(element, NONE_GATHERING_TYPE_FIELD_CRC),
  };
}

function parseGatheringTypeData(element: ObjectStreamElement): GatheringTypeData {
  requireObjectStreamType(element, GATHERING_TYPE_DATA_TYPE_ID);
  return {
    gatheringType: requiredStringField(element, TYPE_FIELD_CRC),
    uiIcon: readTextureReference(requiredTypedChild(element, UI_ICON_FIELD_CRC, SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID)),
    requirementText: requiredStringField(element, REQUIREMENT_TEXT_FIELD_CRC),
  };
}

function parseGatheringAction(element: ObjectStreamElement): GatheringAction {
  requireObjectStreamType(element, GATHERING_ACTION_TYPE_ID);
  return {
    name: requiredStringField(element, NAME_FIELD_CRC),
    mannequinTag: requiredStringField(element, MANNEQUIN_TAG_FIELD_CRC),
  };
}

function parseGatheringActionDatabase(bytes: Uint8Array): GatheringActionDatabase {
  const root = strictObjectStreamRoot(bytes, GATHERING_ACTION_DATABASE_TYPE_ID);
  const actions = requiredTypedChild(root, GATHERING_ACTIONS_FIELD_CRC, GATHERING_ACTION_DATA_VECTOR_TYPE_ID);
  return {
    gatheringActions: actions.children.map(parseGatheringActionData),
  };
}

function parseGatheringActionData(element: ObjectStreamElement): GatheringActionData {
  requireObjectStreamType(element, GATHERING_ACTION_DATA_TYPE_ID);
  return {
    name: requiredStringField(element, NAME_FIELD_CRC),
    mannequinTag: requiredStringField(element, MANNEQUIN_TAG_FIELD_CRC),
  };
}

function parseCraftingStationDatabase(bytes: Uint8Array): CraftingStationDatabase {
  const root = strictObjectStreamRoot(bytes, CRAFTING_STATION_DATABASE_TYPE_ID);
  const stations = requiredTypedChild(root, CRAFTING_STATIONS_FIELD_CRC, CRAFTING_STATION_DATA_VECTOR_TYPE_ID);
  return {
    craftingStations: stations.children.map(parseCraftingStationData),
  };
}

function parseCraftingStationData(element: ObjectStreamElement): CraftingStationData {
  requireObjectStreamType(element, CRAFTING_STATION_DATA_TYPE_ID);
  return {
    name: requiredStringField(element, NAME_FIELD_CRC),
    craftingTypes: readStringVector(requiredTypedChild(element, CRAFTING_TYPES_FIELD_CRC, CRAFTING_TYPE_STRING_VECTOR_TYPE_ID)),
    mannequinTag: requiredStringField(element, CRAFTING_MANNEQUIN_TAG_FIELD_CRC),
    azothDiscountPercent: requiredF32Field(element, AZOTH_DISCOUNT_PERCENT_FIELD_CRC),
  };
}

function parseSocialRankDatabase(bytes: Uint8Array): SocialRankDatabase {
  const root = strictObjectStreamRoot(bytes, SOCIAL_RANK_DATABASE_TYPE_ID);
  const ranks = requiredTypedChild(root, SOCIAL_RANKS_FIELD_CRC, SOCIAL_RANK_DATA_VECTOR_TYPE_ID);
  return {
    ranks: ranks.children.map(parseSocialRankData),
  };
}

function parseSocialRankData(element: ObjectStreamElement): SocialRankData {
  requireObjectStreamType(element, SOCIAL_RANK_DATA_TYPE_ID);
  return {
    guildRankData: parseSocialGuildRankData(
      requiredTypedChild(element, SOCIAL_GUILD_RANK_DATA_FIELD_CRC, SOCIAL_GUILD_RANK_DATA_TYPE_ID),
    ),
  };
}

function parseSocialGuildRankData(element: ObjectStreamElement): SocialGuildRankData {
  const privileges = requiredTypedChild(
    element,
    SOCIAL_GUILD_RANK_PRIVILEGE_IDS_FIELD_CRC,
    SOCIAL_PRIVILEGE_ID_SET_TYPE_ID,
  );
  return {
    name: requiredStringField(element, SOCIAL_GUILD_RANK_NAME_FIELD_CRC),
    securityLevel: requiredU32Field(element, SOCIAL_GUILD_RANK_SECURITY_LEVEL_FIELD_CRC),
    allPrivileges: requiredBoolField(element, SOCIAL_GUILD_RANK_ALL_PRIVILEGES_FIELD_CRC),
    privilegeIds: privileges.children.map((child) => objectStreamU32(child)),
  };
}

function parseArmorOffsetDatabase(bytes: Uint8Array): ArmorOffsetDatabase {
  const root = strictObjectStreamRoot(bytes, ARMOR_OFFSET_DATABASE_TYPE_ID);
  const offsetsElement = requiredTypedChild(root, ARMOR_OFFSETS_FIELD_CRC, ARMOR_OFFSET_VECTOR_TYPE_ID);
  return {
    offsets: offsetsElement.children.map(parseArmorOffsetData),
  };
}

function parseArmorOffsetData(element: ObjectStreamElement): ArmorOffsetData {
  requireObjectStreamType(element, ARMOR_OFFSET_DATA_TYPE_ID);
  const attachments = requiredTypedChild(
    element,
    ARMOR_OFFSET_ATTACHMENTS_FIELD_CRC,
    ATTACHMENT_OFFSET_VECTOR_TYPE_ID,
  );
  return {
    name: requiredStringField(element, ARMOR_OFFSET_NAME_FIELD_CRC),
    attachments: attachments.children.map(parseAttachmentOffsetData),
  };
}

function parseAttachmentOffsetData(element: ObjectStreamElement): AttachmentOffsetData {
  requireObjectStreamType(element, ATTACHMENT_OFFSET_DATA_TYPE_ID);
  return {
    attachment: requiredStringField(element, ATTACHMENT_NAME_FIELD_CRC),
    position: requiredVec3Field(element, ATTACHMENT_OFFSET_POSITION_FIELD_CRC),
    rotationDegrees: requiredVec3Field(element, ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC),
  };
}

function armorOffsetByName(
  database: ArmorOffsetDatabase,
  name: string,
): ArmorOffsetData | undefined {
  return database.offsets.find((offset) => offset.name === name);
}

function furthestArmorAttachmentOffset(
  database: ArmorOffsetDatabase,
  armorOffsetNames: readonly string[],
  attachmentName: string,
  currentPosition: Vec3,
): AttachmentOffsetData | undefined {
  let best: AttachmentOffsetData | undefined;
  let bestLength = vec3Length(currentPosition);
  for (const offsetName of armorOffsetNames) {
    const offset = armorOffsetByName(database, offsetName);
    if (offset === undefined) {
      continue;
    }
    for (const attachment of offset.attachments) {
      if (attachment.attachment !== attachmentName) {
        continue;
      }
      const length = vec3Length(attachment.position);
      if (length > bestLength) {
        bestLength = length;
        best = attachment;
      }
    }
  }
  return best;
}

function parseEquipTypesDatabase(bytes: Uint8Array): EquipTypesDatabase {
  const root = strictObjectStreamRoot(bytes, EQUIP_TYPES_DATABASE_TYPE_ID);
  const equipTypes = requiredTypedChild(root, EQUIP_TYPES_FIELD_CRC, EQUIP_TYPE_DATA_VECTOR_TYPE_ID);
  return { equipTypes: equipTypes.children.map(parseEquipTypeData) };
}

function parseEquipTypeData(element: ObjectStreamElement): EquipTypeData {
  requireObjectStreamType(element, EQUIP_TYPE_DATA_TYPE_ID);
  return {
    name: requiredStringField(element, EQUIP_NAME_FIELD_CRC),
    attachment: requiredStringField(element, EQUIP_ATTACHMENT_FIELD_CRC),
    attachmentOffsetPosition: requiredVec3Field(element, EQUIP_ATTACHMENT_OFFSET_POSITION_FIELD_CRC),
    attachmentOffsetRotationDegrees: requiredVec3Field(element, EQUIP_ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC),
    sheathData: requiredStringField(element, EQUIP_SHEATH_DATA_FIELD_CRC),
    sheathOffsetPosition: requiredVec3Field(element, EQUIP_SHEATH_OFFSET_POSITION_FIELD_CRC),
    sheathOffsetRotationDegrees: requiredVec3Field(element, EQUIP_SHEATH_OFFSET_ROTATION_DEGREES_FIELD_CRC),
    offHandAttachment: requiredStringField(element, EQUIP_OFF_HAND_ATTACHMENT_FIELD_CRC),
    offHandAttachmentOffsetPosition: requiredVec3Field(element, EQUIP_OFF_HAND_ATTACHMENT_OFFSET_POSITION_FIELD_CRC),
    offHandAttachmentOffsetRotationDegrees: requiredVec3Field(element, EQUIP_OFF_HAND_ATTACHMENT_OFFSET_ROTATION_DEGREES_FIELD_CRC),
    offHandSheathData: requiredStringField(element, EQUIP_OFF_HAND_SHEATH_DATA_FIELD_CRC),
    offHandSheathOffsetPosition: requiredVec3Field(element, EQUIP_OFF_HAND_SHEATH_OFFSET_POSITION_FIELD_CRC),
    offHandSheathOffsetRotationDegrees: requiredVec3Field(element, EQUIP_OFF_HAND_SHEATH_OFFSET_ROTATION_DEGREES_FIELD_CRC),
  };
}

function parseGameDebugSettings(bytes: Uint8Array): GameDebugSettings {
  const root = strictObjectStreamRoot(bytes, GAME_DEBUG_SETTINGS_TYPE_ID);
  const combat = requiredTypedChild(root, COMBAT_SETTINGS_FIELD_CRC, COMBAT_DEBUG_SETTINGS_TYPE_ID);
  return {
    combatSettings: {
      disablePlayerLootDropOnDeath: requiredBoolField(combat, DISABLE_PLAYER_LOOT_DROP_ON_DEATH_FIELD_CRC),
      disableWeaponDurability: requiredBoolField(combat, DISABLE_WEAPON_DURABILITY_FIELD_CRC),
      disableItemDurability: requiredBoolField(combat, DISABLE_ITEM_DURABILITY_FIELD_CRC),
      disableDurabilityPenaltyOnDeath: requiredBoolField(combat, DISABLE_DURABILITY_PENALTY_ON_DEATH_FIELD_CRC),
    },
  };
}

function disabledCombatToggleCount(combat: CombatDebugSettings): number {
  return Number(combat.disablePlayerLootDropOnDeath) +
    Number(combat.disableWeaponDurability) +
    Number(combat.disableItemDurability) +
    Number(combat.disableDurabilityPenaltyOnDeath);
}

function parseUiDatabase(bytes: Uint8Array): UiDatabase {
  const root = strictObjectStreamRoot(bytes, UI_DATABASE_TYPE_ID);
  const unified = requiredChild(root, 0, UNIFIED_INTERACT_DATA_TYPE_ID);
  const options = requiredChild(unified, 0, INTERACT_OPTION_VECTOR_TYPE_ID);
  return {
    unifiedInteractData: {
      interactOptions: options.children.map(parseInteractOptionData),
    },
  };
}

function parseInteractOptionData(element: ObjectStreamElement): InteractOptionData {
  requireObjectStreamType(element, INTERACT_OPTION_DATA_TYPE_ID);
  const child = (index: number) => requiredChild(element, index);
  return {
    name: objectStreamString(child(0)),
    displayName: objectStreamString(child(1)),
    interactInputType: wrappedI32(child(2)),
    uiInteractAction: wrappedU8(child(3)),
    additionalInfoType: wrappedI32(child(4)),
    interactOptionCategory: wrappedI32(child(5)),
    delayedInteractionData: parseDelayedInteractionData(child(6)),
    interactPrivilegeIds: child(7).children.map(wrappedU32),
    blueprintPrivilegeId: wrappedU32(child(8)),
    requiresConfirmation: objectStreamBool(child(9)),
    isCommittedInteraction: objectStreamBool(child(10)),
    isInstantCancel: objectStreamBool(child(11)),
    closePromptOnInteraction: objectStreamBool(child(12)),
    forceSecondaryInteract: objectStreamBool(child(13)),
    onlyShowIfBoundToCamp: objectStreamBool(child(14)),
    displayPriority: objectStreamI32(child(15)),
    interactOptionIcon: firstStringDescendant(child(16)) ?? "",
    uiAdditionalInfoSlicePath: objectStreamString(child(17)),
    requiresSecurityLevelValidation: objectStreamBool(child(18)),
    mannequinFragment: objectStreamString(child(19)),
    mannequinTag: objectStreamString(child(20)),
    alignToInteraction: objectStreamBool(child(21)),
    holdActionPressTime: objectStreamF32(child(22)),
    cooldownTime: objectStreamI32(child(23)),
    setOwnershipOnInteract: objectStreamBool(child(24)),
    requiredItemName: objectStreamString(child(25)),
    requiredItemCount: objectStreamI32(child(26)),
    requiredCurrency: objectStreamI32(child(27)),
    availability: wrappedI32(child(28)),
    siegeWarfareGameEventName: objectStreamString(child(29)),
    addedStatusEffects: parseEffects(child(30)),
    requiredStatusEffects: parseEffects(child(31)),
    removeStatusEffects: parseEffects(child(32)),
    excludedStatusEffects: parseEffects(child(33)),
    delayBeforeAddingRemovingEffect: objectStreamF32(child(34)),
    removeAddedEffectsOnInteractionEnd: objectStreamBool(child(35)),
    checkPvpFlagIsSet: objectStreamBool(child(36)),
    factionRequired: objectStreamBool(child(37)),
    showInstancedLootItemCount: objectStreamBool(child(38)),
    requiredAchievementName: objectStreamString(child(39)),
    requiredLevel: objectStreamU32(child(40)),
    committedInteractionMaxUsageTimeout: objectStreamF32(child(41)),
    committedInteractionMaxUsageTimeoutNotification: objectStreamString(child(42)),
    committedInteractionInactiveTimeout: objectStreamF32(child(43)),
    committedInteractionInactiveTimeoutNotification: objectStreamString(child(44)),
  };
}

function parseDelayedInteractionData(element: ObjectStreamElement): DelayedInteractionData {
  return {
    delayTime: objectStreamF32(requiredChild(element, 0)),
    delayMannequinTag: objectStreamString(requiredChild(element, 1)),
  };
}

function parseEffects(element: ObjectStreamElement): readonly EffectData[] {
  return element.children.map((effect) => ({ effectId: firstStringDescendant(effect) ?? "" }));
}

function indexInteractOptionsByNameCrc(
  options: readonly InteractOptionData[],
): ReadonlyMap<number, InteractOptionData> {
  const out = new Map<number, InteractOptionData>();
  for (const option of options) {
    const key = crc32Lowercase(option.name);
    if (!out.has(key)) {
      out.set(key, option);
    }
  }
  return out;
}

function parseGameCameraSettings(bytes: Uint8Array): GameCameraSettings {
  const xml = PRODUCT_TEXT_DECODER.decode(bytes).replace(/^\uFEFF/, "");
  const fields = xmlFields(xml);
  const states: CameraStateSettings[] = [];
  for (const match of xml.matchAll(/<CameraState\b([^>]*)>([\s\S]*?)<\/CameraState>/g)) {
    const attrs = xmlAttributes(match[1]);
    const body = match[2];
    const stateFields = xmlFields(body);
    const fromTransitions = Array.from(body.matchAll(/<FromTransition\b([^/>]*)(?:\/>|>([\s\S]*?)<\/FromTransition>)/g)).map((transition) => {
      const transitionAttrs = xmlAttributes(transition[1]);
      const transitionFields = xmlFields(transition[2] ?? "");
      return {
        fromCamera: transitionAttrs.FromCamera ?? transitionAttrs.fromCamera ?? transitionFields.FromCamera ?? "",
        smoothTime: parseOptionalFloat(transitionAttrs.SmoothTime ?? transitionAttrs.smoothTime ?? transitionFields.SmoothTime),
      };
    });
    states.push({
      name: attrs.name ?? "",
      include: attrs.include,
      fields: stateFields,
      fromTransitions,
    });
  }
  return {
    defaultStateName: fields.defaultStateName ?? "",
    fields,
    cameraStates: states,
  };
}

function strictObjectStreamRoot(bytes: Uint8Array, typeId: string): ObjectStreamElement {
  const stream = parseObjectStream(bytes);
  if (stream.version !== 3) {
    throw new Error(`unsupported ObjectStream version ${stream.version}`);
  }
  return singleObjectStreamRoot(stream, typeId);
}

function requiredTypedChild(
  element: ObjectStreamElement,
  nameCrc: number,
  typeId: string,
): ObjectStreamElement {
  const child = requiredChildByNameCrc(element, nameCrc);
  requireObjectStreamType(child, typeId);
  return child;
}

function requiredStringField(element: ObjectStreamElement, nameCrc: number): string {
  const child = requiredTypedChild(element, nameCrc, AZSTD_STRING_TYPE_ID);
  return objectStreamString(child);
}

function requiredVec3Field(element: ObjectStreamElement, nameCrc: number): Vec3 {
  const child = requiredTypedChild(element, nameCrc, VECTOR3_TYPE_ID);
  return objectStreamVec3(child);
}

function requiredBoolField(element: ObjectStreamElement, nameCrc: number): boolean {
  const child = requiredTypedChild(element, nameCrc, BOOL_TYPE_ID);
  return objectStreamBool(child);
}

function requiredI32Field(element: ObjectStreamElement, nameCrc: number): number {
  const child = requiredTypedChild(element, nameCrc, INT_TYPE_ID);
  return objectStreamI32(child);
}

function requiredU32Field(element: ObjectStreamElement, nameCrc: number): number {
  const child = requiredTypedChild(element, nameCrc, U32_TYPE_ID);
  return objectStreamU32(child);
}

function requiredF32Field(element: ObjectStreamElement, nameCrc: number): number {
  const child = requiredTypedChild(element, nameCrc, FLOAT_TYPE_ID);
  return objectStreamF32(child);
}

function requiredSection(
  element: ObjectStreamElement,
  fieldName: string,
  typeId: string,
): ObjectStreamElement {
  return requiredTypedChild(element, crc32Lowercase(fieldName), typeId);
}

function requiredFieldByName(element: ObjectStreamElement, fieldName: string): ObjectStreamElement {
  return requiredChildByNameCrc(element, crc32Lowercase(fieldName));
}

function requiredStringFieldByName(element: ObjectStreamElement, fieldName: string): string {
  return requiredStringField(element, crc32Lowercase(fieldName));
}

function requiredI32FieldByName(element: ObjectStreamElement, fieldName: string): number {
  return requiredI32Field(element, crc32Lowercase(fieldName));
}

function requiredU32FieldByName(element: ObjectStreamElement, fieldName: string): number {
  return requiredU32Field(element, crc32Lowercase(fieldName));
}

function requiredF32FieldByName(element: ObjectStreamElement, fieldName: string): number {
  return requiredF32Field(element, crc32Lowercase(fieldName));
}

function requiredBoolFieldByName(element: ObjectStreamElement, fieldName: string): boolean {
  return requiredBoolField(element, crc32Lowercase(fieldName));
}

function requiredCrc32FieldByName(element: ObjectStreamElement, fieldName: string): number {
  return readCrc32(requiredFieldByName(element, fieldName));
}

function requiredStringSequenceByName(
  element: ObjectStreamElement,
  fieldName: string,
): readonly string[] {
  return readStringVector(requiredFieldByName(element, fieldName));
}

function requiredCrc32SequenceByName(
  element: ObjectStreamElement,
  fieldName: string,
): readonly number[] {
  return requiredFieldByName(element, fieldName).children.map(readCrc32);
}

function readStringVector(element: ObjectStreamElement): readonly string[] {
  return element.children.map((child) => {
    requireObjectStreamType(child, AZSTD_STRING_TYPE_ID);
    return objectStreamString(child);
  });
}

function readCrc32(element: ObjectStreamElement): number {
  requireObjectStreamType(element, CRC32_TYPE_ID);
  if (element.data.length === 4) {
    return objectStreamU32(element);
  }
  const value = requiredChildByNameCrc(element, crc32Lowercase("Value"));
  return objectStreamU32(value);
}

function parseEditCrc(element: ObjectStreamElement): EditCrc {
  requireObjectStreamType(element, EDIT_CRC_TYPE_ID);
  return {
    valueStr: requiredStringFieldByName(element, "m_valueStr"),
    valueCrc: requiredCrc32FieldByName(element, "m_valueCrc"),
  };
}

function readI32Triple(element: ObjectStreamElement): readonly [number, number, number] {
  const values = element.children.map(readI32Value);
  if (values.length !== 3) {
    throw new Error(`ObjectStream element ${element.typeId} has ${values.length} values, expected 3`);
  }
  return [values[0], values[1], values[2]];
}

function readI32Value(element: ObjectStreamElement): number {
  if (element.typeId === INT_TYPE_ID) {
    return objectStreamI32(element);
  }
  if (element.children.length === 1) {
    return readI32Value(element.children[0]);
  }
  throw new Error(`ObjectStream element ${element.typeId} is not an i32 value`);
}

function readColorRgba(element: ObjectStreamElement): ColorRgba {
  requireObjectStreamType(element, COLOR_TYPE_ID);
  if (element.data.length !== 16) {
    throw new Error(`ObjectStream color has ${element.data.length} bytes, expected 16`);
  }
  const view = objectStreamDataView(element);
  return {
    r: view.getFloat32(0, false),
    g: view.getFloat32(4, false),
    b: view.getFloat32(8, false),
    a: view.getFloat32(12, false),
  };
}

function readAssetReference(element: ObjectStreamElement): AssetReference {
  requireObjectStreamType(element, ASSET_TYPE_ID);
  const data = element.data;
  const candidates = [
    { subIdBytes: 4, assetTypeOffset: 32, hintLenOffset: 48, hintOffset: 56, reservedStart: 20, reservedEnd: 32 },
    { subIdBytes: 4, assetTypeOffset: 24, hintLenOffset: 40, hintOffset: 48, reservedStart: 20, reservedEnd: 24 },
    { subIdBytes: 8, assetTypeOffset: 24, hintLenOffset: 40, hintOffset: 48 },
    { subIdBytes: 4, assetTypeOffset: 20, hintLenOffset: 36, hintOffset: 44 },
  ];
  for (const candidate of candidates) {
    if (data.length < candidate.hintOffset) {
      continue;
    }
    if (
      candidate.reservedStart !== undefined &&
      data.slice(candidate.reservedStart, candidate.reservedEnd).some((byte) => byte !== 0)
    ) {
      continue;
    }
    const view = objectStreamDataView(element);
    const hintLength = Number(view.getBigUint64(candidate.hintLenOffset, false));
    if (hintLength !== data.length - candidate.hintOffset) {
      continue;
    }
    const subId = candidate.subIdBytes === 8
      ? Number(view.getBigUint64(16, false))
      : view.getUint32(16, false);
    return {
      guid: uuidFromBytes(data.slice(0, 16)),
      subId,
      assetType: uuidFromBytes(data.slice(candidate.assetTypeOffset, candidate.assetTypeOffset + 16)),
      hint: PRODUCT_TEXT_DECODER.decode(data.slice(candidate.hintOffset)),
    };
  }
  throw new Error(`unsupported AZ::Data::Asset layout with ${data.length} bytes`);
}

function readTextureReference(element: ObjectStreamElement): SimpleAssetReferenceTextureAsset {
  requireObjectStreamType(element, SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID);
  const base = requiredTypedChild(element, BASE_CLASS_FIELD_CRC, SIMPLE_ASSET_REFERENCE_BASE_TYPE_ID);
  return {
    assetPath: requiredStringField(base, ASSET_PATH_FIELD_CRC),
  };
}

function objectStreamDataView(element: ObjectStreamElement): DataView {
  return new DataView(
    element.data.buffer,
    element.data.byteOffset,
    element.data.byteLength,
  );
}

function uuidFromBytes(bytes: Uint8Array): string {
  if (bytes.length !== 16) {
    throw new Error(`uuid has ${bytes.length} bytes, expected 16`);
  }
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));
  return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10, 16).join("")}`;
}

function requiredChild(
  element: ObjectStreamElement,
  index: number,
  typeId?: string,
): ObjectStreamElement {
  const child = element.children[index];
  if (child === undefined) {
    throw new Error(`ObjectStream element ${element.typeId} is missing child ${index}`);
  }
  if (typeId !== undefined) {
    requireObjectStreamType(child, typeId);
  }
  return child;
}

function wrappedI32(element: ObjectStreamElement): number {
  return objectStreamI32(requiredChild(element, 0, INT_TYPE_ID));
}

function wrappedU8(element: ObjectStreamElement): number {
  return objectStreamU8(requiredChild(element, 0, U8_TYPE_ID));
}

function wrappedU32(element: ObjectStreamElement): number {
  return objectStreamU32(requiredChild(element, 0, U32_TYPE_ID));
}

function firstStringDescendant(element: ObjectStreamElement): string | undefined {
  if (element.typeId === AZSTD_STRING_TYPE_ID) {
    return objectStreamString(element);
  }
  for (const child of element.children) {
    const value = firstStringDescendant(child);
    if (value !== undefined) {
      return value;
    }
  }
  return undefined;
}

function vec3Length(value: Vec3): number {
  return Math.hypot(value.x, value.y, value.z);
}

function xmlFields(xml: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const match of xml.matchAll(/<([A-Za-z0-9_]+)\b([^>]*)\/>/g)) {
    const tag = match[1];
    const attrs = xmlAttributes(match[2]);
    const name = attrs.name ?? tag;
    out[name] = attrs.value ?? "";
  }
  return out;
}

function xmlAttributes(source: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const match of source.matchAll(/([A-Za-z0-9_:-]+)\s*=\s*"([^"]*)"/g)) {
    out[match[1]] = decodeXmlEntities(match[2]);
  }
  return out;
}

function decodeXmlEntities(value: string): string {
  return value
    .replace(/&quot;/g, "\"")
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

function parseOptionalFloat(value: string | undefined): number | undefined {
  if (value === undefined || value.length === 0) {
    return undefined;
  }
  const parsed = Number(value.replace(/f$/i, ""));
  return Number.isFinite(parsed) ? parsed : undefined;
}

"#;

const DYNAMIC_MANAGER_RUNTIME_TS: &str = r#"
class ManagerInstance {
  constructor(
    private readonly definition: ManagerDefinition,
    private readonly tables: ReadonlyMap<string, DynamicTable>,
    private readonly assets: readonly string[],
    private readonly assetBytesByPath: ReadonlyMap<string, Uint8Array>,
  ) {}

  table(name: string): DynamicTable | undefined {
    return this.tables.get(name);
  }

  private assetBytes(path?: string): Uint8Array | undefined {
    const requested = path ?? (this.assets.length === 1 ? this.assets[0] : undefined);
    if (requested === undefined) {
      return undefined;
    }
    const normalized = normalizeDataPath(requested);
    const exact = this.assetBytesByPath.get(normalized);
    if (exact !== undefined) {
      return exact;
    }
    for (const [candidate, bytes] of this.assetBytesByPath) {
      if (candidate.endsWith(`/${normalized}`)) {
        return bytes;
      }
    }
    return undefined;
  }

  requiredAssetBytes(path?: string): Uint8Array {
    const bytes = this.assetBytes(path);
    if (bytes === undefined) {
      throw new Error(`manager ${this.definition.name} asset ${path ?? "<single>"} was not loaded`);
    }
    return bytes;
  }

  schemaRows<T>(
    rowType: string,
    read: (table: DynamicTable, row: DynamicTableRow) => T,
  ): readonly T[] {
    const out: T[] = [];
    for (const table of this.allTables()) {
      if (table.schema.rowType !== rowType) {
        continue;
      }
      for (const row of table.rows) {
        out.push(read(table, row));
      }
    }
    return out;
  }

  schemaRow<T>(
    rowType: string,
    key: string | number | boolean | null,
    read: (table: DynamicTable, row: DynamicTableRow) => T,
    keyOf: (row: T) => string | number | boolean | null,
  ): T | undefined {
    const lookupKey = normalizeLookupKey(key);
    return this.schemaRows(rowType, read).find((row) => normalizeLookupKey(keyOf(row)) === lookupKey);
  }

  private allTables(): readonly DynamicTable[] {
    return Array.from(new Set(this.tables.values()));
  }
}

class ManagerCache {
  private readonly datasheetsByPath: ReadonlyMap<string, DatasheetAsset>;
  private readonly assetsByPath: ReadonlyMap<string, Uint8Array>;
  private readonly tableCache = new Map<string, DynamicTable>();
  private readonly managerCache = new Map<string, ManagerInstance>();

  constructor(source: PakDatasheetSource) {
    this.datasheetsByPath = new Map(
      source.datasheets.map((asset) => [normalizeDataPath(asset.path), asset]),
    );
    this.assetsByPath = new Map(
      source.assets.map((asset) => [normalizeDataPath(asset.path), asset.bytes]),
    );
  }

  [MANAGER_INSTANCE](name: string): unknown {
    const definition = managerByName(name);
    if (definition === undefined) {
      throw new Error(`unknown manager ${name}`);
    }
    return this.buildManager(definition, new Set());
  }

  private buildManager(
    definition: ManagerDefinition,
    stack: Set<string>,
  ): ManagerInstance {
    const cached = this.managerCache.get(definition.name);
    if (cached !== undefined) {
      return cached;
    }
    if (stack.has(definition.name)) {
      throw new Error(`manager dependency cycle at ${definition.name}`);
    }
    stack.add(definition.name);

    const tables = new Map<string, DynamicTable>();
    const assets: string[] = [];
    const assetBytesByPath = new Map<string, Uint8Array>();

    for (const dependency of definition.dependencies) {
      switch (dependency.kind) {
        case "table": {
          const schema = tableSchemaByNameAndRow(dependency.name, dependency.row);
          if (schema === undefined) {
            throw new Error(
              `manager ${definition.name} depends on unknown table ${dependency.name}/${dependency.row}`,
            );
          }
          const table = this.buildTable(schema);
          tables.set(dependency.name, table);
          tables.set(schema.name, table);
          tables.set(`${schema.name}:${schema.rowType}`, table);
          break;
        }
        case "asset":
          assets.push(dependency.path);
          assetBytesByPath.set(
            normalizeDataPath(dependency.path),
            this.requiredAssetBytes(dependency.path),
          );
          break;
      }
    }

    stack.delete(definition.name);
    const instance = new ManagerInstance(definition, tables, assets, assetBytesByPath);
    this.managerCache.set(definition.name, instance);
    return instance;
  }

  private buildTable(schema: TableSchema): DynamicTable {
    const cacheKey = `${schema.name}:${schema.rowType}`;
    const cached = this.tableCache.get(cacheKey);
    if (cached !== undefined) {
      return cached;
    }

    const rowKeyColumn = schema.columns.find((column) => column.rowKey);
    if (rowKeyColumn === undefined) {
      throw new Error(`table ${schema.name} has no row-key column`);
    }

    const sheets: Datasheet[] = [];
    const rows: DynamicTableRow[] = [];
    const rowsByKey = new Map<string, DynamicTableRow>();
    const rowsByLookupKey = new Map<string, DynamicTableRow>();
    const duplicateKeys = new Map<string, DynamicTableRow[]>();

    for (const sourcePath of schema.sources) {
      const asset = this.datasheetAsset(sourcePath);
      if (asset === undefined) {
        throw new Error(`datasheet source ${sourcePath} was not loaded`);
      }
      const sheet = parseDatasheet(asset.bytes);
      const columnSlots = columnSlotsForSheet(schema, sheet);
      const rowKeySlot = columnSlots.get(rowKeyColumn.crc);
      if (rowKeySlot === undefined) {
        throw new Error(`datasheet source ${sourcePath} missing row-key column ${rowKeyColumn.name}`);
      }
      sheets.push(sheet);
      for (const [rowIndex, row] of sheet.rows.entries()) {
        const keyCell = row.cells[rowKeySlot];
        const key = keyCell === undefined ? undefined : rowKeyValue(keyCell.value);
        if (key === undefined) {
          continue;
        }
        const dynamicRow: DynamicTableRow = {
          sourcePath: asset.path,
          rowIndex,
          key,
          row,
          columnSlots,
        };
        rows.push(dynamicRow);
        const existing = rowsByKey.get(key);
        if (existing === undefined) {
          rowsByKey.set(key, dynamicRow);
        }
        const lookupKey = normalizeLookupKey(key);
        const existingLookup = rowsByLookupKey.get(lookupKey);
        if (existingLookup === undefined) {
          rowsByLookupKey.set(lookupKey, dynamicRow);
        } else {
          const duplicates = duplicateKeys.get(lookupKey) ?? [existingLookup];
          duplicates.push(dynamicRow);
          duplicateKeys.set(lookupKey, duplicates);
        }
      }
    }

    const table: DynamicTable = {
      schema,
      sheets,
      rows,
      rowsByKey,
      rowsByLookupKey,
      duplicateKeys,
    };
    this.tableCache.set(cacheKey, table);
    return table;
  }

  private datasheetAsset(sourcePath: string): DatasheetAsset | undefined {
    const normalized = normalizeDataPath(sourcePath);
    const exact = this.datasheetsByPath.get(normalized);
    if (exact !== undefined) {
      return exact;
    }
    for (const [path, asset] of this.datasheetsByPath) {
      if (path.endsWith(`/${normalized}`)) {
        return asset;
      }
    }
    return undefined;
  }

  private assetBytes(path: string): Uint8Array | undefined {
    const normalized = normalizeDataPath(path);
    const exact = this.assetsByPath.get(normalized);
    if (exact !== undefined) {
      return exact;
    }
    for (const [candidate, bytes] of this.assetsByPath) {
      if (candidate.endsWith(`/${normalized}`)) {
        return bytes;
      }
    }
    return undefined;
  }

  private requiredAssetBytes(path: string): Uint8Array {
    const bytes = this.assetBytes(path);
    if (bytes === undefined) {
      throw new Error(`asset ${path} was not loaded`);
    }
    return bytes;
  }
}

function managerByName(
  name: string,
): ManagerDefinition | undefined {
  return MANAGERS.find((entry) => entry.name === name);
}

function columnSlotsForSheet(
  schema: TableSchema,
  sheet: Datasheet,
): ReadonlyMap<number, number> {
  const slots = new Map<number, number>();
  for (const column of schema.columns) {
    const slot = sheet.columns.findIndex((candidate) => candidate.crc === column.crc);
    if (slot >= 0) {
      slots.set(column.crc, slot);
    }
  }
  return slots;
}

function rowKeyValue(value: DatasheetCellValue): string | undefined {
  switch (value.kind) {
    case "string": {
      const text = value.value.trim();
      return text.length === 0 ? undefined : text;
    }
    case "number":
      return Number.isInteger(value.value)
        ? value.value.toFixed(0)
        : String(value.value);
    case "boolean":
      return value.value ? "true" : "false";
  }
}

function normalizeLookupKey(key: string | number | boolean | null): string {
  return key === null ? "" : String(key).trim().toLowerCase();
}

function columnMatches(column: ColumnSchema, name: string): boolean {
  return column.name === name || column.fieldName === name;
}

function normalizeDataPath(path: string): string {
  return path.replace(/\\/g, "/").replace(/\/+/g, "/").toLowerCase();
}
"#;
