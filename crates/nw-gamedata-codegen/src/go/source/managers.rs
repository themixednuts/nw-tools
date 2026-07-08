use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use nw_datasheet::ColumnType;

use crate::compiler::GameDataCompileUnit;
use crate::emit::GameDataCodegenFile;
use crate::game_system_schema::GameSystemTableSchema;
use crate::go::source::format_go_source;
use crate::manager_records::{
    DirectManagerSurface, ItemDataManagerSurface, ManagerSurface, ManagerSurfaceDependency,
    SemanticLookupKind, SemanticManagerKey, SemanticManagerRecord, SemanticNumericKeyType,
    SemanticProjectionTransform, SemanticRowFilterPredicate, go_field_name, go_method_name,
    lower_camel, manager_surface_dependencies, manager_surface_name, manager_surfaces,
    semantic_manager_record_unit,
};
use crate::naming::{to_snake_ident, to_upper_camel_ident};
use nw_serialize_codegen::{
    GoSourceEmitter as SerializeGoSourceEmitter, GoSourceOptions as SerializeGoSourceOptions,
};

const DEFAULT_GO_GAMEASSETS_IMPORT: &str = "example.com/newworld/gamedata/gameassets";

pub(super) fn emit_manager_files(unit: &GameDataCompileUnit) -> Result<Vec<GameDataCodegenFile>> {
    let surfaces = manager_surfaces(unit)?;
    Ok(vec![GameDataCodegenFile::new(
        "managers/managers.go",
        manager_source(unit, false, &surfaces)?,
    )])
}

pub(super) fn emit_dynamic_manager_files(
    unit: &GameDataCompileUnit,
) -> Result<Vec<GameDataCodegenFile>> {
    let surfaces = manager_surfaces(unit)?;
    let records = semantic_records(&surfaces);
    let mut files = vec![GameDataCodegenFile::new(
        "managers/managers.go",
        manager_source(unit, true, &surfaces)?,
    )];
    if !records.is_empty() {
        files.push(GameDataCodegenFile::new(
            "managers/types.go",
            manager_record_types_source(&records)?,
        ));
    }
    Ok(files)
}

fn manager_source(
    unit: &GameDataCompileUnit,
    dynamic_assets: bool,
    surfaces: &[ManagerSurface],
) -> Result<String> {
    if !dynamic_assets {
        return manifest_source(unit, surfaces);
    }

    let mut source = format!(
        r#"
package managers

import (
	"encoding/binary"
	"fmt"
	"html"
	"math"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"{}"
)

type DatasheetCellKind string

const (
	DatasheetCellString  DatasheetCellKind = "string"
	DatasheetCellNumber  DatasheetCellKind = "number"
	DatasheetCellBoolean DatasheetCellKind = "boolean"
)

type ColumnSchema struct {{
	Name      string
	FieldName string
	CRC       uint32
	Kind      DatasheetCellKind
	RowKey    bool
	Required  bool
}}

type TableSchema struct {{
	Name        string
	NameCRC     uint32
	RowType     string
	RowTypeCRC  uint32
	RowCount    int
	Sources     []string
	Columns     []ColumnSchema
}}

type managerDependencyKind string

const (
	managerDependencyTable   managerDependencyKind = "table"
	managerDependencyAsset   managerDependencyKind = "asset"
)

type managerDependency struct {{
	Kind        managerDependencyKind
	Name        string
	Row         string
	Path        string
}}

type managerDefinition struct {{
	Name         string
	Dependencies []managerDependency
}}

type dynamicTableRow struct {{
	SourcePath  string
	RowIndex    int
	Key         string
	Row         gameassets.DatasheetRow
	ColumnSlots map[uint32]int
}}

type dynamicTable struct {{
	Schema        TableSchema
	Sheets        []gameassets.Datasheet
	Rows          []dynamicTableRow
	RowsByKey     map[string]dynamicTableRow
	RowsByLookupKey map[string]dynamicTableRow
	DuplicateKeys map[string][]dynamicTableRow
}}

"#,
        DEFAULT_GO_GAMEASSETS_IMPORT,
    );

    push_schema_row_types(&mut source, unit);
    push_table_schemas(&mut source, unit);
    push_managers(&mut source, unit, surfaces);
    push_manager_surface_types(&mut source, unit, surfaces);
    source.push_str(PRODUCT_MANAGER_RUNTIME_GO);
    source.push_str(DYNAMIC_MANAGER_RUNTIME_GO);

    format_go_source(&source).map_err(Into::into)
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

#[derive(Debug, Clone)]
struct GoSchemaRow {
    type_name: String,
    source_row_type: String,
    fields: Vec<GoSchemaField>,
}

#[derive(Debug, Clone)]
struct GoSchemaField {
    source_name: String,
    field_name: String,
    column_type: ColumnType,
    required: bool,
    row_key: bool,
}

fn push_schema_row_types(source: &mut String, unit: &GameDataCompileUnit) {
    for row in go_schema_rows(unit) {
        if row.source_row_type == "LootBucketData" {
            push_loot_bucket_schema_row_type(source);
            continue;
        }
        source.push_str(&format!("type {} struct {{\n", row.type_name));
        for field in &row.fields {
            source.push_str(&format!(
                "\t{} {}\n",
                field.field_name,
                go_schema_field_type(field.column_type, field.required)
            ));
        }
        source.push_str("}\n\n");
        source.push_str(&format!(
            "func {}(table *dynamicTable, row dynamicTableRow) ({}, error) {{\n",
            go_schema_reader_name(&row.source_row_type),
            row.type_name
        ));
        source.push_str(&format!("\tvar out {}\n", row.type_name));
        source.push_str("\tvar err error\n");
        for field in &row.fields {
            source.push_str(&format!(
                "\tout.{}, err = {}\n",
                field.field_name,
                go_schema_field_read_expression(field)
            ));
            source.push_str("\tif err != nil {\n");
            source.push_str(&format!("\t\treturn {}{{}}, err\n", row.type_name));
            source.push_str("\t}\n");
        }
        source.push_str("\treturn out, nil\n");
        source.push_str("}\n\n");
    }
}

fn push_loot_bucket_schema_row_type(source: &mut String) {
    source.push_str(
        r#"
type LootBucketDataSchemaRow struct {
	RowPlaceholders      string
	Entries              []LootBucketDataEntry
	LootBiasingDisabled  []LootBucketBiasingDisabled
}

type LootBucketDataEntry struct {
	Slot       uint16
	LootBucket *string
	Tags       *string
	MatchOne   *string
	Item       *string
	Quantity   *string
	Odds       *string
}

type LootBucketBiasingDisabled struct {
	Slot     uint16
	Disabled bool
}

func readLootBucketDataSchemaRow(table *dynamicTable, row dynamicTableRow) (LootBucketDataSchemaRow, error) {
	rowPlaceholders, err := requiredStringCell(table, row, "RowPlaceholders")
	if err != nil {
		return LootBucketDataSchemaRow{}, err
	}

	entries := []LootBucketDataEntry{}
	for _, slot := range numberedColumnSlots(table, []string{"LootBucket", "Tags", "MatchOne", "Item", "Quantity", "Odds"}) {
		lootBucket, err := optionalCellText(table, row, numberedColumnName("LootBucket", slot))
		if err != nil {
			return LootBucketDataSchemaRow{}, err
		}
		tags, err := optionalCellText(table, row, numberedColumnName("Tags", slot))
		if err != nil {
			return LootBucketDataSchemaRow{}, err
		}
		matchOne, err := optionalCellText(table, row, numberedColumnName("MatchOne", slot))
		if err != nil {
			return LootBucketDataSchemaRow{}, err
		}
		item, err := optionalCellText(table, row, numberedColumnName("Item", slot))
		if err != nil {
			return LootBucketDataSchemaRow{}, err
		}
		quantity, err := optionalCellText(table, row, numberedColumnName("Quantity", slot))
		if err != nil {
			return LootBucketDataSchemaRow{}, err
		}
		odds, err := optionalCellText(table, row, numberedColumnName("Odds", slot))
		if err != nil {
			return LootBucketDataSchemaRow{}, err
		}
		if lootBucket != nil || tags != nil || matchOne != nil || item != nil || quantity != nil || odds != nil {
			entries = append(entries, LootBucketDataEntry{
				Slot: slot,
				LootBucket: lootBucket,
				Tags: tags,
				MatchOne: matchOne,
				Item: item,
				Quantity: quantity,
				Odds: odds,
			})
		}
	}

	lootBiasingDisabled := []LootBucketBiasingDisabled{}
	for _, slot := range numberedColumnSlots(table, []string{"LootBiasingDisabled"}) {
		disabled, err := optionalCellBoolText(table, row, numberedColumnName("LootBiasingDisabled", slot))
		if err != nil {
			return LootBucketDataSchemaRow{}, err
		}
		if disabled != nil {
			lootBiasingDisabled = append(lootBiasingDisabled, LootBucketBiasingDisabled{Slot: slot, Disabled: *disabled})
		}
	}

	return LootBucketDataSchemaRow{
		RowPlaceholders: rowPlaceholders,
		Entries: entries,
		LootBiasingDisabled: lootBiasingDisabled,
	}, nil
}

func numberedColumnSlots(table *dynamicTable, prefixes []string) []uint16 {
	seen := map[uint16]struct{}{}
	for _, column := range table.Schema.Columns {
		for _, prefix := range prefixes {
			if slot, ok := numberedColumnSlot(column.Name, prefix); ok {
				seen[slot] = struct{}{}
			}
		}
	}
	slots := make([]uint16, 0, len(seen))
	for slot := range seen {
		slots = append(slots, slot)
	}
	sort.Slice(slots, func(left, right int) bool { return slots[left] < slots[right] })
	return slots
}

func numberedColumnSlot(name string, prefix string) (uint16, bool) {
	if !strings.HasPrefix(name, prefix) {
		return 0, false
	}
	suffix := name[len(prefix):]
	if suffix == "" {
		return 0, false
	}
	value, err := strconv.ParseUint(suffix, 10, 16)
	if err != nil {
		return 0, false
	}
	return uint16(value), true
}

func numberedColumnName(prefix string, slot uint16) string {
	return fmt.Sprintf("%s%d", prefix, slot)
}

func optionalCellText(table *dynamicTable, row dynamicTableRow, columnName string) (*string, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return nil, nil
	}
	switch value.Kind {
	case gameassets.DatasheetCellString:
		if value.String == "" {
			return nil, nil
		}
		return &value.String, nil
	case gameassets.DatasheetCellNumber:
		text := strconv.FormatFloat(float64(value.Number), 'f', -1, 32)
		return &text, nil
	case gameassets.DatasheetCellBoolean:
		text := strconv.FormatBool(value.Boolean)
		return &text, nil
	default:
		return nil, nil
	}
}

func optionalCellBoolText(table *dynamicTable, row dynamicTableRow, columnName string) (*bool, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return nil, nil
	}
	switch value.Kind {
	case gameassets.DatasheetCellBoolean:
		return &value.Boolean, nil
	case gameassets.DatasheetCellNumber:
		out := value.Number != 0
		return &out, nil
	case gameassets.DatasheetCellString:
		text := strings.ToLower(strings.TrimSpace(value.String))
		if text == "" {
			return nil, nil
		}
		if text == "true" || text == "1" || text == "yes" {
			out := true
			return &out, nil
		}
		if text == "false" || text == "0" || text == "no" {
			out := false
			return &out, nil
		}
	}
	return nil, fmt.Errorf("row %s:%d has non-bool %s", row.SourcePath, row.RowIndex + 1, columnName)
}

"#,
    );
}

fn go_schema_rows(unit: &GameDataCompileUnit) -> Vec<GoSchemaRow> {
    let mut rows = BTreeMap::<String, Vec<GoSchemaField>>::new();
    for table in &unit.schema_report().tables {
        let row_type = table.row_type_name.clone();
        let fields = rows.entry(row_type.clone()).or_default();
        for column in &table.columns {
            let field_name = go_field_name(&column.name);
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
            fields.push(GoSchemaField {
                source_name: column.name.clone(),
                field_name,
                column_type: column.declared_type,
                required: column.row_key,
                row_key: column.row_key,
            });
        }
    }
    rows.into_iter()
        .map(|(source_row_type, fields)| GoSchemaRow {
            type_name: go_schema_row_type_name(&source_row_type),
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

fn go_schema_field_type(column_type: ColumnType, required: bool) -> &'static str {
    match (column_type, required) {
        (ColumnType::String, true) => "string",
        (ColumnType::String, false) => "*string",
        (ColumnType::Number, true) => "float32",
        (ColumnType::Number, false) => "*float32",
        (ColumnType::Boolean, true) => "bool",
        (ColumnType::Boolean, false) => "*bool",
    }
}

fn go_schema_field_read_expression(field: &GoSchemaField) -> String {
    let column = go_string(&field.source_name);
    match (field.column_type, field.required) {
        (ColumnType::String, true) => format!("requiredStringCell(table, row, {column})"),
        (ColumnType::String, false) => format!("optionalStringCell(table, row, {column})"),
        (ColumnType::Number, true) => format!("requiredFloat32Cell(table, row, {column})"),
        (ColumnType::Number, false) => format!("optionalFloat32Cell(table, row, {column})"),
        (ColumnType::Boolean, true) => format!("requiredBoolCell(table, row, {column})"),
        (ColumnType::Boolean, false) => format!("optionalBoolCell(table, row, {column})"),
    }
}

fn go_schema_reader_name(row_type: &str) -> String {
    format!("read{}", go_schema_row_type_name(row_type))
}

fn go_schema_row_type_name(row_type: &str) -> String {
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

    #[test]
    fn semantic_materializer_tracks_duplicate_keys_across_all_tables() {
        let record = SemanticManagerRecord {
            manager_name: "ExampleDataManager".to_owned(),
            manager_class_name: "ExampleDataManager".to_owned(),
            record_type_name: "ExampleData".to_owned(),
            tables: vec![
                crate::manager_records::SemanticManagerTable {
                    table_name: "ExampleA".to_owned(),
                },
                crate::manager_records::SemanticManagerTable {
                    table_name: "ExampleB".to_owned(),
                },
            ],
            key: Some(SemanticManagerKey::Crc {
                key_field: "example_id".to_owned(),
                crc_field: "example_id_crc".to_owned(),
                key_column: "ExampleID".to_owned(),
                skip_empty_key: true,
                trim_key: true,
                reject_zero_crc: true,
                duplicate_key_policy: crate::manager::NativeDuplicateKeyPolicy::FirstWins,
            }),
            source_row_field: Some("source_row".to_owned()),
            source_row_method: Some("source_row".to_owned()),
            row_filters: Vec::new(),
            fields: Vec::new(),
            lookup_methods: Vec::new(),
            ids_method: None,
            rows_method: Some("rows".to_owned()),
            len_method: Some("len".to_owned()),
            is_empty_method: Some("is_empty".to_owned()),
        };
        let mut source = String::new();
        push_go_semantic_materializer(&mut source, &record);

        let seen_index = source
            .find("\tseen := map[any]struct{}{}")
            .expect("materializer should track duplicate keys");
        let table_loop_index = source
            .find("\tfor _, tableName := range []string{")
            .expect("materializer should iterate tables");
        let row_loop_index = source
            .find("\t\tfor _, sourceRow := range table.Rows {")
            .expect("materializer should iterate rows");
        assert!(
            seen_index < table_loop_index && table_loop_index < row_loop_index,
            "duplicate-key tracking must be scoped across every table and row"
        );
    }
}

fn manifest_source(unit: &GameDataCompileUnit, surfaces: &[ManagerSurface]) -> Result<String> {
    let mut source = String::from(
        r#"
package managers

type managerDependencyKind string

const (
	managerDependencyTable   managerDependencyKind = "table"
	managerDependencyAsset   managerDependencyKind = "asset"
)

type managerDependency struct {
	Kind        managerDependencyKind
	Name        string
	Row         string
	Path        string
}

type managerDefinition struct {
	Name         string
	Dependencies []managerDependency
}

"#,
    );

    push_managers(&mut source, unit, surfaces);
    source.push_str(
        r#"
func managerByName(name string) *managerDefinition {
	for i := range managers {
		if managers[i].Name == name {
			return &managers[i]
		}
	}
	return nil
}
"#,
    );

    format_go_source(&source).map_err(Into::into)
}

fn push_table_schemas(source: &mut String, unit: &GameDataCompileUnit) {
    source.push_str("var TableSchemas = []TableSchema{\n");
    for table in &unit.schema_report().tables {
        push_table_schema(source, table);
    }
    source.push_str("}\n\n");
    source.push_str(
        r#"
func TableSchemaByName(name string) *TableSchema {
	for i := range TableSchemas {
		if TableSchemas[i].Name == name {
			return &TableSchemas[i]
		}
	}
	return nil
}

func TableSchemaByNameAndRow(name string, rowType string) *TableSchema {
	for i := range TableSchemas {
		if TableSchemas[i].Name == name && TableSchemas[i].RowType == rowType {
			return &TableSchemas[i]
		}
	}
	return nil
}

func TableSchemaBySourcePath(sourcePath string) *TableSchema {
	normalized := normalizeDataPath(sourcePath)
	for i := range TableSchemas {
		for _, candidate := range TableSchemas[i].Sources {
			if normalizeDataPath(candidate) == normalized {
				return &TableSchemas[i]
			}
		}
	}
	return nil
}

"#,
    );
}

fn push_table_schema(source: &mut String, table: &GameSystemTableSchema) {
    source.push_str("\t{\n");
    source.push_str(&format!("\t\tName: {},\n", go_string(&table.table_name)));
    source.push_str(&format!("\t\tNameCRC: {},\n", table.table_name_crc));
    source.push_str(&format!(
        "\t\tRowType: {},\n",
        go_string(&table.row_type_name)
    ));
    source.push_str(&format!("\t\tRowTypeCRC: {},\n", table.row_type_crc));
    source.push_str(&format!("\t\tRowCount: {},\n", table.row_count));
    source.push_str("\t\tSources: []string{");
    for (index, source_path) in table.sources.iter().enumerate() {
        if index > 0 {
            source.push_str(", ");
        }
        source.push_str(&go_string(source_path));
    }
    source.push_str("},\n");
    source.push_str("\t\tColumns: []ColumnSchema{\n");
    for column in &table.columns {
        source.push_str("\t\t\t{\n");
        source.push_str(&format!("\t\t\t\tName: {},\n", go_string(&column.name)));
        source.push_str(&format!(
            "\t\t\t\tFieldName: {},\n",
            go_string(&to_snake_ident(&column.name, "column"))
        ));
        source.push_str(&format!("\t\t\t\tCRC: {},\n", column.crc));
        source.push_str(&format!(
            "\t\t\t\tKind: {},\n",
            cell_kind(column.declared_type)
        ));
        source.push_str(&format!("\t\t\t\tRowKey: {},\n", column.row_key));
        source.push_str(&format!("\t\t\t\tRequired: {},\n", column.required));
        source.push_str("\t\t\t},\n");
    }
    source.push_str("\t\t},\n");
    source.push_str("\t},\n");
}

fn push_managers(source: &mut String, unit: &GameDataCompileUnit, surfaces: &[ManagerSurface]) {
    source.push_str("var managers = []managerDefinition{\n");
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
        source.push_str("\t{\n");
        source.push_str(&format!("\t\tName: {},\n", go_string(manager_name)));
        source.push_str("\t\tDependencies: []managerDependency{\n");
        for dependency in &dependencies {
            push_manager_dependency(source, dependency);
        }
        source.push_str("\t\t},\n");
        source.push_str("\t},\n");
    }
    source.push_str("}\n\n");
}

fn push_manager_dependency(source: &mut String, input: &ManagerSurfaceDependency) {
    match input {
        ManagerSurfaceDependency::Table { name, row } => {
            source.push_str("\t\t\t{\n");
            source.push_str("\t\t\t\tKind: managerDependencyTable,\n");
            source.push_str(&format!("\t\t\t\tName: {},\n", go_string(name)));
            source.push_str(&format!("\t\t\t\tRow: {},\n", go_string(row)));
            source.push_str("\t\t\t},\n");
        }
        ManagerSurfaceDependency::Asset { path } => {
            source.push_str("\t\t\t{\n");
            source.push_str("\t\t\t\tKind: managerDependencyAsset,\n");
            source.push_str(&format!("\t\t\t\tPath: {},\n", go_string(path)));
            source.push_str("\t\t\t},\n");
        }
    }
}

fn semantic_type_name(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

fn cell_kind(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::String => "DatasheetCellString",
        ColumnType::Number => "DatasheetCellNumber",
        ColumnType::Boolean => "DatasheetCellBoolean",
    }
}

fn go_string(value: &str) -> String {
    format!("{value:?}")
}

fn manager_record_types_source(records: &[SemanticManagerRecord]) -> Result<String> {
    let unit = semantic_manager_record_unit(records);
    SerializeGoSourceEmitter
        .emit(
            &unit,
            &SerializeGoSourceOptions {
                package_name: "managers".to_owned(),
                include_support_aliases: false,
            },
        )
        .map_err(|err| anyhow::anyhow!("emit Go manager record types: {err}"))
}

fn push_manager_surface_types(
    source: &mut String,
    unit: &GameDataCompileUnit,
    surfaces: &[ManagerSurface],
) {
    if surfaces.is_empty() {
        return;
    }
    for surface in surfaces {
        match surface {
            ManagerSurface::Direct(manager) => push_direct_manager_type(source, unit, manager),
            ManagerSurface::Semantic(record) => push_semantic_manager_type(source, record),
            ManagerSurface::ItemData(manager) => push_item_data_manager_type(source, manager),
            ManagerSurface::ProductBacked(manager) => {
                push_product_backed_manager_type(source, manager)
            }
        }
    }
    source.push_str(SEMANTIC_MANAGER_RUNTIME_GO);
}

fn push_direct_manager_type(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) {
    let manager_type = &manager.manager_class_name;
    let mut product_methods = direct_go_product_methods(manager);
    product_methods.push_str(&special_go_manager_extra_methods(manager_type));
    let row_methods = direct_go_schema_methods(unit, manager);
    source.push_str(&format!(
        r#"
type {manager_type} struct {{
	instance *managerInstance
}}

func New{manager_type}(runtime *ManagerRuntime) (*{manager_type}, error) {{
	instance, err := runtime.manager({})
	if err != nil {{
		return nil, err
	}}
	return &{manager_type}{{instance: instance}}, nil
}}

{row_methods}
{product_methods}
"#,
        go_string(&manager.manager_name)
    ));
}

fn direct_go_schema_methods(unit: &GameDataCompileUnit, manager: &DirectManagerSurface) -> String {
    let row_specs = go_schema_rows(unit);
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
            "Rows".to_owned()
        } else {
            format!("{}Rows", go_method_name(&row_type))
        };
        source.push_str(&format!(
            r#"func (manager *{manager_type}) {rows_method}() ([]{type_name}, error) {{
	return schemaRows(manager.instance, {row_type:?}, {reader})
}}

"#,
            manager_type = manager.manager_class_name,
            reader = go_schema_reader_name(&row_type),
        ));
        if let Some(key_field) = row_spec.fields.iter().find(|field| field.row_key) {
            let lookup_method = if single_row_type {
                "Get".to_owned()
            } else {
                go_method_name(&row_type)
            };
            source.push_str(&format!(
                r#"func (manager *{manager_type}) {lookup_method}(key any) (*{type_name}, error) {{
	return schemaRow(manager.instance, {row_type:?}, key, {reader}, func(row {type_name}) any {{ return row.{key_field} }})
}}

"#,
                manager_type = manager.manager_class_name,
                key_field = key_field.field_name.as_str(),
                reader = go_schema_reader_name(&row_type),
            ));
        }
    }
    source
}

fn direct_go_product_methods(manager: &DirectManagerSurface) -> String {
    let mut source = String::new();
    for product in &manager.products {
        let path = go_string(&product.path);
        let getter = go_method_name(&product.manager_getter);
        match product.value_type.as_str() {
            "newworld_plugin::assets::armor_offset_database::ArmorOffsetDatabase" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*ArmorOffsetDatabase, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseArmorOffsetDatabase(bytes)
}}

func (manager *{manager_type}) ArmorOffset(name string) (*ArmorOffsetData, error) {{
	database, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return ArmorOffsetByName(database, name), nil
}}

func (manager *{manager_type}) FurthestAttachmentOffset(armorOffsetNames []string, attachmentName string, currentPosition Vec3) (*AttachmentOffsetData, error) {{
	database, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return FurthestArmorAttachmentOffset(database, armorOffsetNames, attachmentName, currentPosition), nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::equip_types_database::EquipTypesDatabase" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*EquipTypesDatabase, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseEquipTypesDatabase(bytes)
}}

func (manager *{manager_type}) EquipTypes() ([]EquipTypeData, error) {{
	database, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return database.EquipTypes, nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::game_debug_settings::GameDebugSettings" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*GameDebugSettings, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseGameDebugSettings(bytes)
}}

func (manager *{manager_type}) Combat() (*CombatDebugSettings, error) {{
	settings, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return &settings.CombatSettings, nil
}}

func (manager *{manager_type}) DisabledCombatToggleCount() (int, error) {{
	combat, err := manager.Combat()
	if err != nil {{
		return 0, err
	}}
	return DisabledCombatToggleCount(*combat), nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::ui_database::UiDatabase" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*UiDatabase, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseUiDatabase(bytes)
}}

func (manager *{manager_type}) InteractOptions() ([]InteractOptionData, error) {{
	database, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return database.UnifiedInteractData.InteractOptions, nil
}}

func (manager *{manager_type}) InteractOption(id any) (*InteractOptionData, error) {{
	options, err := manager.InteractOptions()
	if err != nil {{
		return nil, err
	}}
	return InteractOptionByID(options, id), nil
}}

func (manager *{manager_type}) InteractOptionsByCategory(category int32) ([]InteractOptionData, error) {{
	options, err := manager.InteractOptions()
	if err != nil {{
		return nil, err
	}}
	return InteractOptionsByCategory(options, category), nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::camera_settings::GameCameraSettings" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*GameCameraSettings, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseGameCameraSettings(bytes)
}}

func (manager *{manager_type}) CameraStates() ([]CameraStateSettings, error) {{
	settings, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return settings.CameraStates, nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::player_base_attributes::PlayerBaseAttributes" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*PlayerBaseAttributes, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParsePlayerBaseAttributes(bytes)
}}

func (manager *{manager_type}) PlayerAttributeData() (*PlayerAttributeData, error) {{
	attributes, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return &attributes.PlayerAttributeData, nil
}}

func (manager *{manager_type}) MaxPerks(rarityLevel int) (*int32, error) {{
	data, err := manager.PlayerAttributeData()
	if err != nil {{
		return nil, err
	}}
	if rarityLevel < 0 || rarityLevel >= len(data.ItemRarityData) {{
		return nil, nil
	}}
	value := data.ItemRarityData[rarityLevel].MaxPerkCount
	return &value, nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::settlement_progression_data::SettlementProgressionData" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*SettlementProgressionData, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseSettlementProgressionData(bytes)
}}

func (manager *{manager_type}) SettlementProgressionCategories() ([]ProgressionCategoryEntry, error) {{
	data, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return data.SettlementProgressionCategories, nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::gathering_database::GatheringDatabase" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*GatheringDatabase, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseGatheringDatabase(bytes)
}}

func (manager *{manager_type}) GatheringData() (*GatheringData, error) {{
	database, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return &database.GatheringData, nil
}}

func (manager *{manager_type}) GatheringTypes() ([]GatheringTypeData, error) {{
	data, err := manager.GatheringData()
	if err != nil {{
		return nil, err
	}}
	return data.GatheringTypes, nil
}}

func (manager *{manager_type}) GatheringActions() ([]GatheringAction, error) {{
	data, err := manager.GatheringData()
	if err != nil {{
		return nil, err
	}}
	return data.GatheringActions, nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::gathering_database::GatheringActionDatabase" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*GatheringActionDatabase, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseGatheringActionDatabase(bytes)
}}

func (manager *{manager_type}) GatheringActionData() ([]GatheringActionData, error) {{
	database, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return database.GatheringActions, nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::crafting_station_database::CraftingStationDatabase" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*CraftingStationDatabase, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseCraftingStationDatabase(bytes)
}}

func (manager *{manager_type}) CraftingStations() ([]CraftingStationData, error) {{
	database, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return database.CraftingStations, nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            "newworld_plugin::assets::rank_database::SocialRankDatabase" => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() (*SocialRankDatabase, error) {{
	bytes, err := manager.instance.requiredAssetBytes({path})
	if err != nil {{
		return nil, err
	}}
	return ParseSocialRankDatabase(bytes)
}}

func (manager *{manager_type}) Ranks() ([]SocialRankData, error) {{
	database, err := manager.{getter}()
	if err != nil {{
		return nil, err
	}}
	return database.Ranks, nil
}}

"#,
                    getter = getter,
                    path = path,
                    manager_type = manager.manager_class_name,
                ));
            }
            _ => {}
        }
    }
    source
}

fn push_product_backed_manager_type(source: &mut String, manager: &DirectManagerSurface) {
    let manager_type = &manager.manager_class_name;
    let mut product_methods = direct_go_product_methods(manager);
    product_methods.push_str(&special_go_manager_extra_methods(manager_type));
    source.push_str(&format!(
        r#"
type {manager_type} struct {{
	instance *managerInstance
}}

func New{manager_type}(runtime *ManagerRuntime) (*{manager_type}, error) {{
	instance, err := runtime.manager({})
	if err != nil {{
		return nil, err
	}}
	return &{manager_type}{{instance: instance}}, nil
}}

{product_methods}
"#,
        go_string(&manager.manager_name)
    ));
}

fn push_item_data_manager_type(source: &mut String, manager: &ItemDataManagerSurface) {
    let manager_type = &manager.manager_class_name;
    let factory = format!("New{manager_type}");
    let table_type = &manager.table_type_name;
    let handle_type = &manager.handle_type_name;
    let data_type = &manager.data_type_name;
    let const_entries = manager
        .tables
        .iter()
        .map(|table| {
            format!(
                "\t{table_type}{} {table_type} = {}\n",
                table.variant_name,
                go_string(&table.table_name)
            )
        })
        .collect::<String>();
    let table_list = manager
        .tables
        .iter()
        .map(|table| format!("\t{table_type}{},\n", table.variant_name))
        .collect::<String>();

    source.push_str(&format!(
        r#"
type {table_type} string

const (
{const_entries})

func (table {table_type}) TableName() string {{
	return string(table)
}}

type {handle_type} struct {{
	Table {table_type}
	Row   uint32
}}

type {data_type} struct {{
	SourceHandle             {handle_type}
	ItemID                   string
	ItemIDCrc                uint32
	Name                     *string
	Description              *string
	ItemType                 *string
	ItemTypeDisplayName      *string
	UIItemClass              *string
	HeartgemRuneTooltipTitle *string
	ConfirmBeforeUse         bool
	ConsumeOnUse             bool
	BindOnPickup             bool
	DeathDropPercentage      float32
}}

var itemDataManagerTables = []{table_type}{{
{table_list}}}

type {manager_type} struct {{
	instance  *managerInstance
	items     []{data_type}
	itemsByID map[uint32]int
}}

func {factory}(runtime *ManagerRuntime) (*{manager_type}, error) {{
	instance, err := runtime.manager({})
	if err != nil {{
		return nil, err
	}}
	return new{manager_type}FromInstance(instance)
}}

func new{manager_type}FromInstance(instance *managerInstance) (*{manager_type}, error) {{
	items, err := materialize{manager_type}(instance)
	if err != nil {{
		return nil, err
	}}
	manager := &{manager_type}{{
		instance:  instance,
		items:     items,
		itemsByID: map[uint32]int{{}},
	}}
	for index := range items {{
		manager.itemsByID[items[index].ItemIDCrc] = index
	}}
	return manager, nil
}}

func (manager *{manager_type}) Get(itemID string) *{data_type} {{
	return manager.GetFromID(crc32Lowercase(itemID))
}}

func (manager *{manager_type}) GetFromID(itemID uint32) *{data_type} {{
	index, ok := manager.itemsByID[itemID]
	if !ok {{
		return nil
	}}
	return &manager.items[index]
}}

func (manager *{manager_type}) ByIndex(index uint32) *{data_type} {{
	if index == 0 {{
		return nil
	}}
	zeroBased := index - 1
	if int(zeroBased) >= len(manager.items) {{
		return nil
	}}
	return &manager.items[zeroBased]
}}

func (manager *{manager_type}) Items() []{data_type} {{
	return manager.items
}}

func (manager *{manager_type}) Len() int {{
	return len(manager.items)
}}

func (manager *{manager_type}) IsEmpty() bool {{
	return len(manager.items) == 0
}}

func materialize{manager_type}(instance *managerInstance) ([]{data_type}, error) {{
	items := []{data_type}{{}}
	seen := map[uint32]struct{{}}{{}}
	for _, tableID := range itemDataManagerTables {{
		table := instance.table(tableID.TableName())
		if table == nil {{
			return nil, fmt.Errorf("manager {manager_type} table %s was not loaded", tableID.TableName())
		}}
		if err := cache{manager_type}Rows(&items, seen, tableID, table); err != nil {{
			return nil, err
		}}
	}}
	return items, nil
}}

func cache{manager_type}Rows(items *[]{data_type}, seen map[uint32]struct{{}}, tableID {table_type}, table *dynamicTable) error {{
	for _, sourceRow := range table.Rows {{
		itemID, err := requiredStringCell(table, sourceRow, "ItemID")
		if err != nil {{
			return err
		}}
		itemID = strings.TrimSpace(itemID)
		if itemID == "" {{
			continue
		}}
		itemIDCrc := crc32Lowercase(itemID)
		if itemIDCrc == 0 {{
			continue
		}}
		if _, exists := seen[itemIDCrc]; exists {{
			continue
		}}
		seen[itemIDCrc] = struct{{}}{{}}
		name, err := optionalStringCell(table, sourceRow, "Name")
		if err != nil {{
			return err
		}}
		description, err := optionalStringCell(table, sourceRow, "Description")
		if err != nil {{
			return err
		}}
		itemType, err := optionalStringCell(table, sourceRow, "ItemType")
		if err != nil {{
			return err
		}}
		itemTypeDisplayName, err := optionalStringCell(table, sourceRow, "ItemTypeDisplayName")
		if err != nil {{
			return err
		}}
		uiItemClass, err := optionalStringCell(table, sourceRow, "UiItemClass")
		if err != nil {{
			return err
		}}
		heartgemRuneTooltipTitle, err := optionalStringCell(table, sourceRow, "HeartgemRuneTooltipTitle")
		if err != nil {{
			return err
		}}
		confirmBeforeUseValue, err := optionalBoolCell(table, sourceRow, "ConfirmBeforeUse")
		if err != nil {{
			return err
		}}
		consumeOnUseValue, err := optionalBoolCell(table, sourceRow, "ConsumeOnUse")
		if err != nil {{
			return err
		}}
		bindOnPickupValue, err := optionalBoolCell(table, sourceRow, "BindOnPickup")
		if err != nil {{
			return err
		}}
		deathDropPercentageValue, err := optionalFloat32Cell(table, sourceRow, "DeathDropPercentage")
		if err != nil {{
			return err
		}}
		confirmBeforeUse := false
		if confirmBeforeUseValue != nil {{
			confirmBeforeUse = *confirmBeforeUseValue
		}}
		consumeOnUse := false
		if consumeOnUseValue != nil {{
			consumeOnUse = *consumeOnUseValue
		}}
		bindOnPickup := false
		if bindOnPickupValue != nil {{
			bindOnPickup = *bindOnPickupValue
		}}
		deathDropPercentage := float32(0)
		if deathDropPercentageValue != nil {{
			deathDropPercentage = *deathDropPercentageValue
		}}
		*items = append(*items, {data_type}{{
			SourceHandle: {handle_type}{{
				Table: tableID,
				Row:   uint32(sourceRow.RowIndex + 1),
			}},
			ItemID:                   itemID,
			ItemIDCrc:                itemIDCrc,
			Name:                     name,
			Description:              description,
			ItemType:                 itemType,
			ItemTypeDisplayName:      itemTypeDisplayName,
			UIItemClass:              uiItemClass,
			HeartgemRuneTooltipTitle: heartgemRuneTooltipTitle,
			ConfirmBeforeUse:         confirmBeforeUse,
			ConsumeOnUse:             consumeOnUse,
			BindOnPickup:             bindOnPickup,
			DeathDropPercentage:      deathDropPercentage,
		}})
	}}
	return nil
}}

"#,
        go_string(&manager.manager_name)
    ));
}

fn push_semantic_manager_type(source: &mut String, record: &SemanticManagerRecord) {
    let manager_type = &record.manager_class_name;
    let record_type = &record.record_type_name;
    let by_key_field = "entriesByKey";
    let by_source_row_field = "entriesBySourceRow";
    let key_map_type = go_key_map_type(record);
    source.push_str(&format!(
        r#"
type {manager_type} struct {{
	instance *managerInstance
	entries []{record_type}
	{by_key_field} map[{key_map_type}]int
	{by_source_row_field} map[uint32]int
}}

func New{manager_type}(runtime *ManagerRuntime) (*{manager_type}, error) {{
	instance, err := runtime.manager({})
	if err != nil {{
		return nil, err
	}}
	return new{manager_type}FromInstance(instance)
}}

func new{manager_type}FromInstance(instance *managerInstance) (*{manager_type}, error) {{
	rows, err := materialize{manager_type}(instance)
	if err != nil {{
		return nil, err
	}}
	manager := &{manager_type}{{
		instance: instance,
		entries: rows,
		{by_key_field}: map[{key_map_type}]int{{}},
		{by_source_row_field}: map[uint32]int{{}},
	}}
	for index := range rows {{
"#,
        go_string(&record.manager_name)
    ));
    if let Some(index_expression) = go_row_index_expression(record) {
        source.push_str(&format!(
            "\t\tmanager.{by_key_field}[{index_expression}] = index\n"
        ));
    }
    if let Some(field) = &record.source_row_field {
        source.push_str(&format!(
            "\t\tmanager.{by_source_row_field}[rows[index].{}] = index\n",
            go_field_name(field)
        ));
    }
    source.push_str(&format!(
        r#"	}}
	return manager, nil
}}

"#
    ));

    for method in &record.lookup_methods {
        let method_name = go_method_name(&method.name);
        let parameter_name = lower_camel(&method.parameter);
        match method.kind {
            SemanticLookupKind::CrcStringKey => source.push_str(&format!(
                r#"func (manager *{manager_type}) {method_name}({parameter_name} string) *{record_type} {{
	index, ok := manager.{by_key_field}[crc32Lowercase({parameter_name})]
	if !ok {{
		return nil
	}}
	return &manager.entries[index]
}}

"#
            )),
            SemanticLookupKind::CrcKey => source.push_str(&format!(
                r#"func (manager *{manager_type}) {method_name}({parameter_name} uint32) *{record_type} {{
	index, ok := manager.{by_key_field}[{parameter_name}]
	if !ok {{
		return nil
	}}
	return &manager.entries[index]
}}

"#
            )),
            SemanticLookupKind::NumericKey(key_type) => {
                let parameter_type = go_numeric_key_type(key_type);
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {method_name}({parameter_name} {parameter_type}) *{record_type} {{
	index, ok := manager.{by_key_field}[uint32({parameter_name})]
	if !ok {{
		return nil
	}}
	return &manager.entries[index]
}}

"#
                ));
            }
            SemanticLookupKind::StringKey => source.push_str(&format!(
                r#"func (manager *{manager_type}) {method_name}({parameter_name} string) *{record_type} {{
	index, ok := manager.{by_key_field}[normalizeStringKey({parameter_name})]
	if !ok {{
		return nil
	}}
	return &manager.entries[index]
}}

"#
            )),
        }
    }
    if let Some(method) = &record.source_row_method {
        let method_name = go_method_name(method);
        source.push_str(&format!(
            r#"func (manager *{manager_type}) {method_name}(row uint32) *{record_type} {{
	index, ok := manager.{by_source_row_field}[row]
	if !ok {{
		return nil
	}}
	return &manager.entries[index]
}}

"#
        ));
    }
    if let Some(method) = &record.ids_method {
        let method_name = go_method_name(method);
        let id_type = go_ids_type(record);
        let id_expression = go_ids_expression(record);
        source.push_str(&format!(
            r#"func (manager *{manager_type}) {method_name}() []{id_type} {{
	ids := make([]{id_type}, 0, len(manager.entries))
	for index := range manager.entries {{
		ids = append(ids, {id_expression})
	}}
	return ids
}}

"#
        ));
    }
    if let Some(method) = &record.rows_method {
        let method_name = go_method_name(method);
        source.push_str(&format!(
            r#"func (manager *{manager_type}) {method_name}() []{record_type} {{
	return manager.entries
}}

"#
        ));
    }
    if let Some(method) = &record.len_method {
        let method_name = go_method_name(method);
        source.push_str(&format!(
            r#"func (manager *{manager_type}) {method_name}() int {{
	return len(manager.entries)
}}

"#
        ));
    }
    if let Some(method) = &record.is_empty_method {
        let method_name = go_method_name(method);
        source.push_str(&format!(
            r#"func (manager *{manager_type}) {method_name}() bool {{
	return len(manager.entries) == 0
}}

"#
        ));
    }

    source.push_str(&special_go_manager_extra_methods(manager_type));

    push_go_semantic_materializer(source, record);
}

fn special_go_manager_extra_methods(manager_type: &str) -> String {
    match manager_type {
        "PlayerDataManager" => r#"func (manager *PlayerDataManager) CategoricalProgressionID(tradeskill any) (*uint32, error) {
	normalized, err := normalizeTradeskillType(tradeskill)
	if err != nil {
		return nil, err
	}
	if normalized == "None" || normalized == "WildernessSurvival" {
		return nil, nil
	}
	value := crc32Lowercase(normalized)
	return &value, nil
}

"#
        .to_owned(),
        _ => String::new(),
    }
}

fn go_key_map_type(record: &SemanticManagerRecord) -> &'static str {
    match record.key {
        Some(SemanticManagerKey::String { .. } | SemanticManagerKey::EnumString { .. }) => "string",
        Some(_) => "uint32",
        None => "uint32",
    }
}

fn go_row_index_expression(record: &SemanticManagerRecord) -> Option<String> {
    Some(match record.key.as_ref()? {
        SemanticManagerKey::Crc { crc_field, .. }
        | SemanticManagerKey::FallbackCrc { crc_field, .. } => {
            format!("rows[index].{}", go_field_name(crc_field))
        }
        SemanticManagerKey::Numeric { key_field, .. } => {
            format!("uint32(rows[index].{})", go_field_name(key_field))
        }
        SemanticManagerKey::EnumString { key_field, .. }
        | SemanticManagerKey::String { key_field, .. } => {
            format!(
                "normalizeStringKey(rows[index].{})",
                go_field_name(key_field)
            )
        }
    })
}

fn go_numeric_key_type(key_type: SemanticNumericKeyType) -> &'static str {
    match key_type {
        SemanticNumericKeyType::U8 => "uint8",
        SemanticNumericKeyType::U16 => "uint16",
        SemanticNumericKeyType::U32 => "uint32",
    }
}

fn go_ids_type(record: &SemanticManagerRecord) -> &'static str {
    match record.key {
        Some(SemanticManagerKey::String { .. } | SemanticManagerKey::EnumString { .. }) => "string",
        Some(SemanticManagerKey::Numeric { key_type, .. }) => go_numeric_key_type(key_type),
        _ => "uint32",
    }
}

fn go_ids_expression(record: &SemanticManagerRecord) -> String {
    match record.key.as_ref() {
        Some(SemanticManagerKey::Crc { crc_field, .. })
        | Some(SemanticManagerKey::FallbackCrc { crc_field, .. }) => {
            format!("manager.entries[index].{}", go_field_name(crc_field))
        }
        Some(SemanticManagerKey::Numeric { key_field, .. })
        | Some(SemanticManagerKey::EnumString { key_field, .. })
        | Some(SemanticManagerKey::String { key_field, .. }) => {
            format!("manager.entries[index].{}", go_field_name(key_field))
        }
        None => "0".to_owned(),
    }
}

fn push_go_semantic_materializer(source: &mut String, record: &SemanticManagerRecord) {
    let manager_type = &record.manager_class_name;
    let record_type = &record.record_type_name;
    source.push_str(&format!(
        r#"func materialize{manager_type}(instance *managerInstance) ([]{record_type}, error) {{
	rows := []{record_type}{{}}
"#
    ));
    if record.key.is_some() {
        source.push_str("\tseen := map[any]struct{}{}\n");
    }
    source.push_str(&format!(
        r#"
	for _, tableName := range []string{{{}}} {{
		table := instance.table(tableName)
		if table == nil {{
			return nil, fmt.Errorf("manager {} missing table %s", tableName)
		}}
		for _, sourceRow := range table.Rows {{
"#,
        record
            .tables
            .iter()
            .map(|table| go_string(&table.table_name))
            .collect::<Vec<_>>()
            .join(", "),
        record.manager_name
    ));
    push_go_key_materializer(source, record);
    for (filter_index, filter) in record.row_filters.iter().enumerate() {
        let column = go_string(&filter.column);
        let filter_value = format!("filterValue{filter_index}");
        match filter.predicate {
            SemanticRowFilterPredicate::BoolTrueWhenPresent => source.push_str(&format!(
                r#"			{filter_value}, err := optionalBoolCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			if {filter_value} != nil && *{filter_value} {{
				continue
			}}
"#
            )),
            SemanticRowFilterPredicate::BoolMustBeTrue => source.push_str(&format!(
                r#"			{filter_value}, err := optionalBoolCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			if {filter_value} == nil || !*{filter_value} {{
				continue
			}}
"#
            )),
            SemanticRowFilterPredicate::F32GreaterThanOrEqualZero => source.push_str(&format!(
                r#"			{filter_value}, err := requiredFloat32Cell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			if {filter_value} < 0 {{
				continue
			}}
"#
            )),
            SemanticRowFilterPredicate::F32LessThanOrEqualZero => source.push_str(&format!(
                r#"			{filter_value}, err := requiredFloat32Cell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			if {filter_value} > 0 {{
				continue
			}}
"#
            )),
            SemanticRowFilterPredicate::F32AnyGreaterThanZero => {
                let columns = std::iter::once(filter.column.as_str())
                    .chain(filter.extra_columns.iter().map(String::as_str))
                    .collect::<Vec<_>>();
                source.push_str("\t\t\tfilterAnyPositive := false\n");
                for (column_index, column) in columns.into_iter().enumerate() {
                    let column = go_string(column);
                    let filter_value = format!("filterValue{filter_index}_{column_index}");
                    source.push_str(&format!(
                        r#"			{filter_value}, err := requiredFloat32Cell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			filterAnyPositive = filterAnyPositive || {filter_value} > 0
"#
                    ));
                }
                source.push_str(
                    r#"			if !filterAnyPositive {
				continue
			}
"#,
                );
            }
            SemanticRowFilterPredicate::I32LessThanOrEqualZero => source.push_str(&format!(
                r#"			{filter_value}, err := requiredInt32Cell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			if {filter_value} > 0 {{
				continue
			}}
"#
            )),
            SemanticRowFilterPredicate::LowercaseCrcStringNonZero => source.push_str(&format!(
                r#"			filterText{filter_index}, err := requiredStringCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			if crc32Lowercase(filterText{filter_index}) == 0 {{
				continue
			}}
"#
            )),
            SemanticRowFilterPredicate::StringNotEqualToColumn => {
                let compare_column = go_string(
                    filter
                        .compare_column
                        .as_deref()
                        .expect("string comparison row filters have compare columns"),
                );
                source.push_str(&format!(
                    r#"			filterText{filter_index}, err := requiredStringCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			compareText{filter_index}, err := requiredStringCell(table, sourceRow, {compare_column})
			if err != nil {{
				return nil, err
			}}
			if filterText{filter_index} == compareText{filter_index} {{
				continue
			}}
"#
                ));
            }
        }
    }
    push_go_duplicate_key_policy(source, record);

    for (index, field) in record.fields.iter().enumerate() {
        source.push_str(&format!(
            "			{}, err := {}\n			if err != nil {{\n				return nil, err\n			}}\n",
            field_temp_name(index),
            go_projection_value(field)
        ));
    }
    source.push_str(&format!("			row := {record_type}{{\n"));
    if let Some(field) = &record.source_row_field {
        source.push_str(&format!(
            "\t\t\t\t{}: uint32(sourceRow.RowIndex + 1),\n",
            go_field_name(field)
        ));
    }
    push_go_key_row_fields(source, record);
    for (index, field) in record.fields.iter().enumerate() {
        source.push_str(&format!(
            "\t\t\t\t{}: {},\n",
            go_field_name(&field.name),
            field_temp_name(index)
        ));
    }
    source.push_str(
        r#"			}
			rows = append(rows, row)
"#,
    );
    if record.key.is_some() {
        source.push_str("\t\t\tseen[seenKey] = struct{}{}\n");
    }
    source.push_str(
        r#"		}
	}
	return rows, nil
}

"#,
    );
}

fn push_go_key_materializer(source: &mut String, record: &SemanticManagerRecord) {
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
                r#"			keyText, err := requiredStringCell(table, sourceRow, {})
			if err != nil {{
				return nil, err
			}}
"#,
                go_string(key_column)
            ));
            if *trim_key {
                source.push_str("\t\t\tkeyValue := strings.TrimSpace(keyText)\n");
            } else {
                source.push_str("\t\t\tkeyValue := keyText\n");
            }
            if *skip_empty_key {
                source.push_str(
                    r#"			if keyValue == "" {
				continue
			}
"#,
                );
            }
            source.push_str("\t\t\tkeyCRC := crc32Lowercase(keyValue)\n");
            if *reject_zero_crc {
                source.push_str(
                    r#"			if keyCRC == 0 {
				continue
			}
"#,
                );
            }
            source.push_str("\t\t\tseenKey := keyCRC\n");
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
                r#"			primaryKeyValue, err := optionalStringCell(table, sourceRow, {})
			if err != nil {{
				return nil, err
			}}
			fallbackKeyValue, err := optionalStringCell(table, sourceRow, {})
			if err != nil {{
				return nil, err
			}}
			keyKind := {}
			keyValue := ""
			if primaryKeyValue != nil && *primaryKeyValue != "" {{
				keyValue = *primaryKeyValue
			}} else {{
				keyKind = {}
				if fallbackKeyValue != nil {{
					keyValue = *fallbackKeyValue
				}}
			}}
"#,
                go_string(primary_key_column),
                go_string(fallback_key_column),
                go_string(primary_key_kind),
                go_string(fallback_key_kind)
            ));
            if *skip_empty_key {
                source.push_str(
                    r#"			if keyValue == "" {
				continue
			}
"#,
                );
            }
            source.push_str(
                r#"			keyCRC := crc32Lowercase(keyValue)
			seenKey := keyCRC
"#,
            );
        }
        SemanticManagerKey::Numeric {
            key_column,
            key_type,
            ..
        } => {
            source.push_str(&format!(
                "			keyValue, err := {}\n			if err != nil {{\n				return nil, err\n			}}\n			seenKey := uint32(keyValue)\n",
                go_numeric_key_value("table", "sourceRow", key_column, *key_type)
            ));
        }
        SemanticManagerKey::EnumString {
            key_column,
            skip_empty_key,
            trim_key,
            ..
        } => {
            source.push_str(&format!(
                r#"			keyText, err := requiredStringCell(table, sourceRow, {})
			if err != nil {{
				return nil, err
			}}
"#,
                go_string(key_column)
            ));
            if *trim_key {
                source.push_str("\t\t\tkeyValue := strings.TrimSpace(keyText)\n");
            } else {
                source.push_str("\t\t\tkeyValue := keyText\n");
            }
            if *skip_empty_key {
                source.push_str(
                    r#"			if keyValue == "" {
				continue
			}
"#,
                );
            }
            source.push_str("\t\t\tseenKey := normalizeStringKey(keyValue)\n");
        }
        SemanticManagerKey::String {
            key_column,
            skip_empty_key,
            ..
        } => {
            source.push_str(&format!(
                r#"			keyValue, err := requiredStringCell(table, sourceRow, {})
			if err != nil {{
				return nil, err
			}}
"#,
                go_string(key_column)
            ));
            if *skip_empty_key {
                source.push_str(
                    r#"			if keyValue == "" {
				continue
			}
"#,
                );
            }
            source.push_str("\t\t\tseenKey := normalizeStringKey(keyValue)\n");
        }
    }
}

fn push_go_duplicate_key_policy(source: &mut String, record: &SemanticManagerRecord) {
    let Some(policy) = record.key.as_ref().map(semantic_key_duplicate_policy) else {
        return;
    };
    match policy {
        crate::manager::NativeDuplicateKeyPolicy::FirstWins => source.push_str(
            r#"			if _, exists := seen[seenKey]; exists {
				continue
			}
"#,
        ),
        crate::manager::NativeDuplicateKeyPolicy::Error => source.push_str(&format!(
            r#"			if _, exists := seen[seenKey]; exists {{
				return nil, fmt.Errorf("manager {} duplicate key %v", seenKey)
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

fn push_go_key_row_fields(source: &mut String, record: &SemanticManagerRecord) {
    let Some(key) = &record.key else {
        return;
    };
    match key {
        SemanticManagerKey::Crc {
            key_field,
            crc_field,
            ..
        } => source.push_str(&format!(
            "\t\t\t\t{}: keyValue,\n\t\t\t\t{}: keyCRC,\n",
            go_field_name(key_field),
            go_field_name(crc_field)
        )),
        SemanticManagerKey::FallbackCrc {
            key_kind_field,
            key_field,
            crc_field,
            ..
        } => source.push_str(&format!(
            "\t\t\t\t{}: keyKind,\n\t\t\t\t{}: keyValue,\n\t\t\t\t{}: keyCRC,\n",
            go_field_name(key_kind_field),
            go_field_name(key_field),
            go_field_name(crc_field)
        )),
        SemanticManagerKey::Numeric { key_field, .. }
        | SemanticManagerKey::EnumString { key_field, .. }
        | SemanticManagerKey::String { key_field, .. } => {
            source.push_str(&format!(
                "\t\t\t\t{}: keyValue,\n",
                go_field_name(key_field)
            ));
        }
    }
}

fn go_numeric_key_value(
    table: &str,
    row: &str,
    column: &str,
    key_type: SemanticNumericKeyType,
) -> String {
    let column = go_string(column);
    match key_type {
        SemanticNumericKeyType::U8 => format!("requiredUint8Cell({table}, {row}, {column})"),
        SemanticNumericKeyType::U16 => format!("requiredUint16Cell({table}, {row}, {column})"),
        SemanticNumericKeyType::U32 => format!("requiredUint32Cell({table}, {row}, {column})"),
    }
}

fn field_temp_name(index: usize) -> String {
    format!("fieldValue{index}")
}

fn go_projection_value(field: &crate::manager_records::SemanticRecordField) -> String {
    let column = go_string(&field.column);
    match field.transform {
        SemanticProjectionTransform::String => {
            format!("requiredStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::StringDefaultEmpty => {
            format!("stringCellDefaultEmpty(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::PlusJoinedList => {
            format!("plusJoinedListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalString => {
            format!("optionalStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::StringList => {
            format!("stringListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::NonEmptyStringList => {
            format!("nonEmptyStringListCell(table, sourceRow, {column})")
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
            format!("requiredFloat32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalF32 => {
            format!("optionalFloat32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::F32List => {
            format!("float32ListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::I32List => {
            format!("int32ListCell(table, sourceRow, {column})")
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
            format!("lowercaseCrcStringListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::RowIndex => {
            format!("requiredUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalRowIndex => {
            format!("optionalUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::RowIndexList => {
            format!("uint32ListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::F32RangeInclusive => {
            format!("float32RangeCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U32RangeInclusive => {
            format!("uint32RangeCell(table, sourceRow, {column})")
        }
    }
}

const SEMANTIC_MANAGER_RUNTIME_GO: &str = r#"
func rowCell(table *dynamicTable, row dynamicTableRow, columnName string) (*gameassets.DatasheetCellValue, bool) {
	var column *ColumnSchema
	for index := range table.Schema.Columns {
		if columnMatches(&table.Schema.Columns[index], columnName) {
			column = &table.Schema.Columns[index]
			break
		}
	}
	if column == nil {
		return nil, false
	}
	slot, ok := row.ColumnSlots[column.CRC]
	if !ok || slot < 0 || slot >= len(row.Row.Cells) {
		return nil, false
	}
	return &row.Row.Cells[slot].Value, true
}

func requiredStringCell(table *dynamicTable, row dynamicTableRow, columnName string) (string, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return "", fmt.Errorf("row %s:%d missing string %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	return stringCellValue(value), nil
}

func optionalStringCell(table *dynamicTable, row dynamicTableRow, columnName string) (*string, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return nil, nil
	}
	text := stringCellValue(value)
	if text == "" {
		return nil, nil
	}
	return &text, nil
}

func stringCellValue(value *gameassets.DatasheetCellValue) string {
	switch value.Kind {
	case gameassets.DatasheetCellString:
		return value.String
	case gameassets.DatasheetCellNumber:
		return strconv.FormatFloat(float64(value.Number), 'f', -1, 32)
	case gameassets.DatasheetCellBoolean:
		return strconv.FormatBool(value.Boolean)
	default:
		return ""
	}
}

func requiredBoolCell(table *dynamicTable, row dynamicTableRow, columnName string) (bool, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return false, fmt.Errorf("row %s:%d missing bool %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	boolean, ok, err := boolCellValue(value, row, columnName)
	if err != nil {
		return false, err
	}
	if !ok {
		return false, fmt.Errorf("row %s:%d missing bool %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	return boolean, nil
}

func optionalBoolCell(table *dynamicTable, row dynamicTableRow, columnName string) (*bool, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return nil, nil
	}
	boolean, ok, err := boolCellValue(value, row, columnName)
	if err != nil || !ok {
		return nil, err
	}
	return &boolean, nil
}

func boolCellValue(value *gameassets.DatasheetCellValue, row dynamicTableRow, columnName string) (bool, bool, error) {
	switch value.Kind {
	case gameassets.DatasheetCellBoolean:
		return value.Boolean, true, nil
	case gameassets.DatasheetCellNumber:
		if value.Number == 0 {
			return false, true, nil
		}
		if value.Number == 1 {
			return true, true, nil
		}
	case gameassets.DatasheetCellString:
		switch strings.ToLower(strings.TrimSpace(value.String)) {
		case "":
			return false, false, nil
		case "false", "0", "no":
			return false, true, nil
		case "true", "1", "yes":
			return true, true, nil
		}
	}
	return false, false, fmt.Errorf("row %s:%d has non-bool %s", row.SourcePath, row.RowIndex+1, columnName)
}

func stringCellDefaultEmpty(table *dynamicTable, row dynamicTableRow, columnName string) (string, error) {
	text, err := optionalStringCell(table, row, columnName)
	if err != nil || text == nil {
		return "", err
	}
	return *text, nil
}

func requiredFloat32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (float32, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return 0, fmt.Errorf("row %s:%d missing number %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	number, present, err := numberCellValue(value, row, columnName)
	if err != nil {
		return 0, err
	}
	if !present {
		return 0, fmt.Errorf("row %s:%d missing number %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	return number, nil
}

func optionalFloat32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (*float32, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return nil, nil
	}
	number, present, err := numberCellValue(value, row, columnName)
	if err != nil || !present {
		return nil, err
	}
	return &number, nil
}

func numberCellValue(value *gameassets.DatasheetCellValue, row dynamicTableRow, columnName string) (float32, bool, error) {
	switch value.Kind {
	case gameassets.DatasheetCellNumber:
		return value.Number, true, nil
	case gameassets.DatasheetCellBoolean:
		if value.Boolean {
			return 1, true, nil
		}
		return 0, true, nil
	case gameassets.DatasheetCellString:
		text := strings.ToLower(strings.TrimSpace(value.String))
		switch text {
		case "":
			return 0, false, nil
		case "false", "no":
			return 0, true, nil
		case "true", "yes":
			return 1, true, nil
		}
		parsed, err := strconv.ParseFloat(strings.TrimSuffix(text, "f"), 32)
		if err == nil {
			return float32(parsed), true, nil
		}
	}
	return 0, false, fmt.Errorf("row %s:%d has non-number %s", row.SourcePath, row.RowIndex+1, columnName)
}

func requiredUint32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (uint32, error) {
	value, err := requiredFloat32Cell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	return normalizeUint32(value)
}

func optionalUint32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (*uint32, error) {
	value, err := optionalFloat32Cell(table, row, columnName)
	if err != nil || value == nil {
		return nil, err
	}
	normalized, err := normalizeUint32(*value)
	if err != nil {
		return nil, err
	}
	return &normalized, nil
}

func requiredUint16Cell(table *dynamicTable, row dynamicTableRow, columnName string) (uint16, error) {
	value, err := requiredUint32Cell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	if value > 0xffff {
		return 0, fmt.Errorf("row %s:%d %s exceeds u16", row.SourcePath, row.RowIndex+1, columnName)
	}
	return uint16(value), nil
}

func optionalUint16Cell(table *dynamicTable, row dynamicTableRow, columnName string) (*uint16, error) {
	value, err := optionalUint32Cell(table, row, columnName)
	if err != nil || value == nil {
		return nil, err
	}
	if *value > 0xffff {
		return nil, fmt.Errorf("row %s:%d %s exceeds u16", row.SourcePath, row.RowIndex+1, columnName)
	}
	converted := uint16(*value)
	return &converted, nil
}

func requiredUint8Cell(table *dynamicTable, row dynamicTableRow, columnName string) (uint8, error) {
	value, err := requiredUint32Cell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	if value > 0xff {
		return 0, fmt.Errorf("row %s:%d %s exceeds u8", row.SourcePath, row.RowIndex+1, columnName)
	}
	return uint8(value), nil
}

func optionalUint8Cell(table *dynamicTable, row dynamicTableRow, columnName string) (*uint8, error) {
	value, err := optionalUint32Cell(table, row, columnName)
	if err != nil || value == nil {
		return nil, err
	}
	if *value > 0xff {
		return nil, fmt.Errorf("row %s:%d %s exceeds u8", row.SourcePath, row.RowIndex+1, columnName)
	}
	converted := uint8(*value)
	return &converted, nil
}

func requiredInt32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (int32, error) {
	value, err := requiredFloat32Cell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	if math.Trunc(float64(value)) != float64(value) || value < -2147483648 || value > 2147483647 {
		return 0, fmt.Errorf("row %s:%d expected i32 %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	return int32(value), nil
}

func requiredCrc32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (uint32, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return 0, fmt.Errorf("row %s:%d missing crc %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	switch value.Kind {
	case gameassets.DatasheetCellNumber:
		return normalizeUint32(value.Number)
	case gameassets.DatasheetCellString:
		return crc32Lowercase(value.String), nil
	default:
		return 0, fmt.Errorf("row %s:%d has non-crc %s", row.SourcePath, row.RowIndex+1, columnName)
	}
}

func optionalCrc32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (*uint32, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return nil, nil
	}
	var crc uint32
	var err error
	switch value.Kind {
	case gameassets.DatasheetCellNumber:
		crc, err = normalizeUint32(value.Number)
	case gameassets.DatasheetCellString:
		if value.String == "" {
			return nil, nil
		}
		crc = crc32Lowercase(value.String)
	default:
		return nil, fmt.Errorf("row %s:%d has non-crc %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	if err != nil || crc == 0 {
		return nil, err
	}
	return &crc, nil
}

func lowercaseCrcStringCell(table *dynamicTable, row dynamicTableRow, columnName string) (uint32, error) {
	text, err := requiredStringCell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	return crc32Lowercase(text), nil
}

func optionalLowercaseCrcStringCell(table *dynamicTable, row dynamicTableRow, columnName string) (*uint32, error) {
	text, err := optionalStringCell(table, row, columnName)
	if err != nil || text == nil {
		return nil, err
	}
	crc := crc32Lowercase(*text)
	return &crc, nil
}

func plusJoinedListCell(table *dynamicTable, row dynamicTableRow, columnName string) (string, error) {
	values, err := stringListCell(table, row, columnName)
	if err != nil {
		return "", err
	}
	return strings.Join(values, "+"), nil
}

func stringListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]string, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return []string{}, nil
	}
	switch value.Kind {
	case gameassets.DatasheetCellString:
		return splitDesignerList(value.String), nil
	case gameassets.DatasheetCellNumber:
		return []string{strconv.FormatFloat(float64(value.Number), 'f', -1, 32)}, nil
	case gameassets.DatasheetCellBoolean:
		if value.Boolean {
			return []string{"true"}, nil
		}
		return []string{"false"}, nil
	default:
		return nil, fmt.Errorf("row %s:%d has unsupported list %s", row.SourcePath, row.RowIndex+1, columnName)
	}
}

func nonEmptyStringListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]string, error) {
	values, err := stringListCell(table, row, columnName)
	if err != nil {
		return nil, err
	}
	out := []string{}
	for _, value := range values {
		if value != "" {
			out = append(out, value)
		}
	}
	return out, nil
}

func optionalStringListCell(table *dynamicTable, row dynamicTableRow, columnName string) (*[]string, error) {
	_, ok := rowCell(table, row, columnName)
	if !ok {
		return nil, nil
	}
	values, err := stringListCell(table, row, columnName)
	if err != nil || len(values) == 0 {
		return nil, err
	}
	return &values, nil
}

func float32ListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]float32, error) {
	values, err := numberListCell(table, row, columnName)
	if err != nil {
		return nil, err
	}
	out := make([]float32, 0, len(values))
	for _, value := range values {
		out = append(out, float32(value))
	}
	return out, nil
}

func int32ListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]int32, error) {
	values, err := numberListCell(table, row, columnName)
	if err != nil {
		return nil, err
	}
	out := make([]int32, 0, len(values))
	for _, value := range values {
		if math.Trunc(value) != value || value < -2147483648 || value > 2147483647 {
			return nil, fmt.Errorf("row %s:%d expected i32 list %s", row.SourcePath, row.RowIndex+1, columnName)
		}
		out = append(out, int32(value))
	}
	return out, nil
}

func uint32ListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]uint32, error) {
	values, err := numberListCell(table, row, columnName)
	if err != nil {
		return nil, err
	}
	out := make([]uint32, 0, len(values))
	for _, value := range values {
		normalized, err := normalizeUint32(float32(value))
		if err != nil {
			return nil, err
		}
		out = append(out, normalized)
	}
	return out, nil
}

func crc32ListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]uint32, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return []uint32{}, nil
	}
	if value.Kind == gameassets.DatasheetCellNumber {
		normalized, err := normalizeUint32(value.Number)
		if err != nil {
			return nil, err
		}
		return []uint32{normalized}, nil
	}
	if value.Kind != gameassets.DatasheetCellString {
		return nil, fmt.Errorf("row %s:%d has non-crc-list %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	parts := splitDesignerList(value.String)
	out := make([]uint32, 0, len(parts))
	for _, part := range parts {
		if number, err := strconv.ParseFloat(part, 32); err == nil {
			normalized, err := normalizeUint32(float32(number))
			if err != nil {
				return nil, err
			}
			out = append(out, normalized)
		} else {
			out = append(out, crc32Lowercase(part))
		}
	}
	return out, nil
}

func lowercaseCrcStringListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]uint32, error) {
	values, err := stringListCell(table, row, columnName)
	if err != nil {
		return nil, err
	}
	out := make([]uint32, 0, len(values))
	for _, value := range values {
		if value != "" {
			out = append(out, crc32Lowercase(value))
		}
	}
	return out, nil
}

func float32RangeCell(table *dynamicTable, row dynamicTableRow, columnName string) (struct{ First, Second float32 }, error) {
	values, err := numberRangeValues(table, row, columnName)
	if err != nil {
		return struct{ First, Second float32 }{}, err
	}
	if len(values) < 2 {
		return struct{ First, Second float32 }{}, fmt.Errorf("row %s:%d missing range %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	return struct{ First, Second float32 }{First: float32(values[0]), Second: float32(values[1])}, nil
}

func uint32RangeCell(table *dynamicTable, row dynamicTableRow, columnName string) (struct{ First, Second uint32 }, error) {
	values, err := numberRangeValues(table, row, columnName)
	if err != nil {
		return struct{ First, Second uint32 }{}, err
	}
	if len(values) < 2 {
		return struct{ First, Second uint32 }{}, fmt.Errorf("row %s:%d missing range %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	first, err := normalizeUint32(float32(values[0]))
	if err != nil {
		return struct{ First, Second uint32 }{}, err
	}
	second, err := normalizeUint32(float32(values[1]))
	if err != nil {
		return struct{ First, Second uint32 }{}, err
	}
	return struct{ First, Second uint32 }{First: first, Second: second}, nil
}

func numberListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]float64, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return []float64{}, nil
	}
	if value.Kind == gameassets.DatasheetCellNumber {
		return []float64{float64(value.Number)}, nil
	}
	if value.Kind != gameassets.DatasheetCellString {
		return nil, fmt.Errorf("row %s:%d has non-number-list %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	parts := splitDesignerList(value.String)
	out := make([]float64, 0, len(parts))
	for _, part := range parts {
		number, err := strconv.ParseFloat(part, 64)
		if err != nil {
			return nil, fmt.Errorf("row %s:%d has invalid number in %s", row.SourcePath, row.RowIndex+1, columnName)
		}
		out = append(out, number)
	}
	return out, nil
}

func numberRangeValues(table *dynamicTable, row dynamicTableRow, columnName string) ([]float64, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return []float64{}, nil
	}
	if value.Kind == gameassets.DatasheetCellString {
		return parseDesignerNumbers(splitDesignerRange(value.String), row, columnName)
	}
	return numberListCell(table, row, columnName)
}

func parseDesignerNumbers(parts []string, row dynamicTableRow, columnName string) ([]float64, error) {
	out := make([]float64, 0, len(parts))
	for _, part := range parts {
		number, err := strconv.ParseFloat(part, 64)
		if err != nil {
			return nil, fmt.Errorf("row %s:%d has invalid number in %s", row.SourcePath, row.RowIndex+1, columnName)
		}
		out = append(out, number)
	}
	return out, nil
}

func splitDesignerList(value string) []string {
	parts := strings.FieldsFunc(value, func(r rune) bool { return r == ',' || r == '+' })
	out := make([]string, 0, len(parts))
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part != "" {
			out = append(out, part)
		}
	}
	return out
}

func splitDesignerRange(value string) []string {
	listed := splitDesignerList(value)
	if len(listed) >= 2 {
		return listed[:2]
	}
	text := strings.TrimSpace(value)
	for index := 1; index < len(text); index++ {
		if text[index] != '-' {
			continue
		}
		left := strings.TrimSpace(text[:index])
		right := strings.TrimSpace(text[index+1:])
		if left != "" && right != "" {
			return []string{left, right}
		}
	}
	return listed
}

func normalizeUint32(value float32) (uint32, error) {
	if math.Trunc(float64(value)) != float64(value) || value < 0 || value > 4294967295 {
		return 0, fmt.Errorf("expected u32, got %v", value)
	}
	return uint32(value), nil
}

func normalizeStringKey(value string) string {
	return strings.ToLower(strings.TrimSpace(value))
}

var crc32Table = func() [256]uint32 {
	var table [256]uint32
	for index := uint32(0); index < 256; index++ {
		crc := index
		for bit := 0; bit < 8; bit++ {
			if crc&1 != 0 {
				crc = 0xedb88320 ^ (crc >> 1)
			} else {
				crc >>= 1
			}
		}
		table[index] = crc
	}
	return table
}()

func crc32Lowercase(value string) uint32 {
	crc := uint32(0xffffffff)
	for index := 0; index < len(value); index++ {
		b := value[index]
		if b >= 'A' && b <= 'Z' {
			b += 32
		}
		crc = crc32Table[(crc^uint32(b))&0xff] ^ (crc >> 8)
	}
	return crc ^ 0xffffffff
}

"#;

const PRODUCT_MANAGER_RUNTIME_GO: &str = r#"
type Vec3 = gameassets.Vec3

const AllInteractOptionsCategory int32 = 0x15

type ArmorOffsetDatabase struct {
	Offsets []ArmorOffsetData
}

type ArmorOffsetData struct {
	Name        string
	Attachments []AttachmentOffsetData
}

type AttachmentOffsetData struct {
	Attachment      string
	Position        Vec3
	RotationDegrees Vec3
}

type EquipTypesDatabase struct {
	EquipTypes []EquipTypeData
}

type EquipTypeData struct {
	Name                                   string
	Attachment                             string
	AttachmentOffsetPosition               Vec3
	AttachmentOffsetRotationDegrees        Vec3
	SheathData                             string
	SheathOffsetPosition                   Vec3
	SheathOffsetRotationDegrees            Vec3
	OffHandAttachment                      string
	OffHandAttachmentOffsetPosition        Vec3
	OffHandAttachmentOffsetRotationDegrees Vec3
	OffHandSheathData                      string
	OffHandSheathOffsetPosition            Vec3
	OffHandSheathOffsetRotationDegrees     Vec3
}

type GameDebugSettings struct {
	CombatSettings CombatDebugSettings
}

type CombatDebugSettings struct {
	DisablePlayerLootDropOnDeath     bool
	DisableWeaponDurability          bool
	DisableItemDurability            bool
	DisableDurabilityPenaltyOnDeath  bool
}

type UiDatabase struct {
	UnifiedInteractData UnifiedInteractData
}

type UnifiedInteractData struct {
	InteractOptions []InteractOptionData
}

type DelayedInteractionData struct {
	DelayTime         float32
	DelayMannequinTag string
}

type EffectData struct {
	EffectID string
}

type InteractOptionData struct {
	Name                                           string
	DisplayName                                    string
	InteractInputType                              int32
	UIInteractAction                               uint8
	AdditionalInfoType                             int32
	InteractOptionCategory                         int32
	DelayedInteractionData                         DelayedInteractionData
	InteractPrivilegeIDs                           []uint32
	BlueprintPrivilegeID                           uint32
	RequiresConfirmation                           bool
	IsCommittedInteraction                         bool
	IsInstantCancel                                bool
	ClosePromptOnInteraction                       bool
	ForceSecondaryInteract                         bool
	OnlyShowIfBoundToCamp                          bool
	DisplayPriority                                int32
	InteractOptionIcon                             string
	UIAdditionalInfoSlicePath                       string
	RequiresSecurityLevelValidation                bool
	MannequinFragment                              string
	MannequinTag                                   string
	AlignToInteraction                             bool
	HoldActionPressTime                            float32
	CooldownTime                                   int32
	SetOwnershipOnInteract                         bool
	RequiredItemName                               string
	RequiredItemCount                              int32
	RequiredCurrency                               int32
	Availability                                   int32
	SiegeWarfareGameEventName                      string
	AddedStatusEffects                             []EffectData
	RequiredStatusEffects                          []EffectData
	RemoveStatusEffects                            []EffectData
	ExcludedStatusEffects                          []EffectData
	DelayBeforeAddingRemovingEffect                float32
	RemoveAddedEffectsOnInteractionEnd             bool
	CheckPvpFlagIsSet                              bool
	FactionRequired                                bool
	ShowInstancedLootItemCount                      bool
	RequiredAchievementName                        string
	RequiredLevel                                  uint32
	CommittedInteractionMaxUsageTimeout            float32
	CommittedInteractionMaxUsageTimeoutNotification string
	CommittedInteractionInactiveTimeout            float32
	CommittedInteractionInactiveTimeoutNotification string
}

type GameCameraSettings struct {
	DefaultStateName string
	Fields           map[string]string
	CameraStates     []CameraStateSettings
}

type CameraStateSettings struct {
	Name            string
	Include         string
	Fields          map[string]string
	FromTransitions []CameraStateTransition
}

type CameraStateTransition struct {
	FromCamera string
	SmoothTime *float32
}

type TradeskillType string

type EditCrc struct {
	ValueStr string
	ValueCrc uint32
}

type ColorRgba struct {
	R float32
	G float32
	B float32
	A float32
}

type IntRange struct {
	Min int32
	Max int32
}

type AssetReference struct {
	Guid      string
	SubID     uint32
	AssetType string
	Hint      string
}

type SimpleAssetReferenceTextureAsset struct {
	AssetPath string
}

type PlayerBaseAttributes struct {
	PlayerAttributeData        PlayerAttributeData
	GuildSiegeWindowRegionData map[string]GuildSiegeWindowRegionData
	FactionInfluenceConfigData FactionInfluenceConfigData
	ValidGroupData             ValidGroupData
	WarData                    WarData
}

type PlayerAttributeData struct {
	BaseDeployableLimit                         int32
	PlayerDisplayLevelUnlockFreeGearSets        int32
	ItemRarityData                              []ItemRarityData
	PerkGenerationData                          PerkGenerationData
	PerkChanceItemID                            string
	AbilityPointsRequiredInTreeToUnlockFinalRow int32
	PerkChanceModifier                          float32
	AttributeChanceModifier                     float32
	GemSlotChanceModifier                       float32
}

type ItemRarityData struct {
	RarityLevelLocString    string
	MaxPerkCount            int32
	LevelRequirementModifier int32
}

type PerkGenerationData struct {
	PerkDataPerTier             []PerkTierData
	CraftingResultLootBucketID  uint32
	CraftingResultLootBucket    string
	RollPerkOnUpgradeGS         int32
	RollPerkOnUpgradeTier       int32
	RollPerkOnUpgradePerkCount  int32
}

type PerkTierData struct {
	MaxPerkChannel             int32
	GemSlotProbability         float32
	AttributePerkProbability   float32
	GeneralGearScorePerkCount  map[int32][]IntRange
	CraftingGearScorePerkCount map[int32][]IntRange
	AttributePerkBucket        string
	AttributePerkBucketID      uint32
}

type GuildSiegeWindowRegionData struct {
	StartHour  uint32
	EndHour    uint32
	UTCOffset  int32
	DstRuleID  uint32
	DstRule    string
	ObservesDst bool
}

type FactionInfluenceConfigData struct {
	MaxInfluence                     float32
	DecrementRate                    float32
	IncrementRate                    float32
	MaxIncrementTimeModifier         float32
	MaxDecrementTimeModifier         float32
	MinimumTimeSinceLastWar          float32
	MinTerritoryDiffToApplyUDMechanics int32
	MinTimeToApplyUDMechanics        int32
	UnderDogMissionInfluenceGain     float32
	UnderDogMissionInfluenceGainCap  float32
	UderDogFactionRepGain            float32
	UnderDogFactionRepGainCap        float32
	UnderDogPVPInfluenceGain         float32
	UnderDogPVPInfluenceGainCap      float32
	MinimumInfluenceThresholdForWar  float32
	InfluenceRaceAttackerWinGameEventID EditCrc
	InfluenceRaceDefenderWinGameEventID EditCrc
	InfluenceRaceLoseGameEventID     EditCrc
}

type ValidGroupData struct {
	Names      []string
	Objectives []string
	IconPaths  []string
	Colors     []ColorRgba
}

type WarData struct {
	DeployableLimits map[uint32]WarDeployableLimitData
}

type WarDeployableLimitData struct {
	ID             uint32
	DisplayName    string
	BuildableNames []string
	BuildableIDs   []uint32
	AttackerLimits [3]int32
	DefenderLimit  int32
}

type SettlementProgressionData struct {
	SettlementProgressionCategories []ProgressionCategoryEntry
}

type ProgressionCategoryEntry struct {
	SettlementProgressionCategory string
	SettlementProgressionEntries  []ProgressionSpawnerEntry
}

type ProgressionSpawnerEntry struct {
	SettlementProgressionCategoryLevel int32
	Slice                              AssetReference
	AlternateSlice                     AssetReference
	DisplayLocString                   string
	Icon                               SimpleAssetReferenceTextureAsset
}

type GatheringDatabase struct {
	GatheringData GatheringData
}

type GatheringData struct {
	GatheringTypes              []GatheringTypeData
	GatheringActions            []GatheringAction
	RequiredWaterGatheringType  string
	NoneGatheringType           string
}

type GatheringTypeData struct {
	GatheringType   string
	UIIcon          SimpleAssetReferenceTextureAsset
	RequirementText string
}

type GatheringAction struct {
	Name         string
	MannequinTag string
}

type GatheringActionDatabase struct {
	GatheringActions []GatheringActionData
}

type GatheringActionData struct {
	Name         string
	MannequinTag string
}

type CraftingStationDatabase struct {
	CraftingStations []CraftingStationData
}

type CraftingStationData struct {
	Name                 string
	CraftingTypes        []string
	MannequinTag         string
	AzothDiscountPercent float32
}

type SocialRankDatabase struct {
	Ranks []SocialRankData
}

type SocialRankData struct {
	GuildRankData SocialGuildRankData
}

type SocialGuildRankData struct {
	Name          string
	SecurityLevel uint32
	AllPrivileges bool
	PrivilegeIDs  []uint32
}

const (
	azstdStringTypeID = "03aaab3f-5c47-5a66-9ebc-d5fa4db353c9"
	vector3TypeID     = "8379eb7d-01fa-4538-b64b-a6543b4be73d"
	boolTypeID        = "a0ca880c-afe4-43cb-926c-59ac48496112"
	u8TypeID          = "72b9409a-7d1a-4831-9cfe-fcb3fadd3426"
	u32TypeID         = "43da906b-7def-4ca8-9790-854106d3f983"
	intTypeID         = "72039442-eb38-4d42-a1ad-cb68f7e0eef6"
	floatTypeID       = "ea2c3e90-afbe-44d4-a90d-faaf79baf93d"
	crc32TypeID       = "9f4e062e-06a0-46d4-85df-e0da96467d3a"
	colorTypeID       = "7894072a-9050-4f0f-901b-34b1a0d29417"
	assetTypeID       = "77a19d40-8731-4d3c-9041-1b43047366a4"
	editCrcTypeID     = "9a339de9-0d6e-4708-922f-f46af04370e9"
	simpleTextureAssetReferenceTypeID = "68e92460-5c0c-4031-9620-6f1a08763243"
	simpleAssetReferenceBaseTypeID = "e16ca6c5-5c78-4ad9-8e9b-f8c1fb4d1db8"

	armorOffsetDatabaseTypeID  = "8c1fa8a8-2e58-4791-acda-2c3625318832"
	armorOffsetVectorTypeID    = "d276dfb3-a8ec-58c2-96e2-145bc5a6ee3d"
	armorOffsetDataTypeID      = "13b87761-89ab-4a4b-a370-dad3875380da"
	attachmentOffsetVectorTypeID = "8b83aa0c-520e-5074-8c4e-5ad60c3d70fe"
	attachmentOffsetDataTypeID = "fc296230-5f66-473e-90c8-66ad7028fd07"

	armorOffsetsFieldCRC                  uint32 = 2282200990
	armorOffsetNameFieldCRC               uint32 = 1579384326
	armorOffsetAttachmentsFieldCRC        uint32 = 1204091606
	attachmentNameFieldCRC                uint32 = 2036324795
	attachmentOffsetPositionFieldCRC      uint32 = 379390882
	attachmentOffsetRotationDegreesFieldCRC uint32 = 581018980

	equipTypesDatabaseTypeID = "f937c753-ffc0-4f9c-a234-7c71c9a5bdb3"
	equipTypeVectorTypeID    = "53de1751-3981-5da4-8f72-f9e5712b3127"
	equipTypeDataTypeID      = "0386d9b0-3e95-411f-823f-4a800c74f7ed"

	equipTypesFieldCRC                              uint32 = 1388966666
	equipNameFieldCRC                               uint32 = 1579384326
	equipAttachmentFieldCRC                         uint32 = 2036324795
	equipAttachmentOffsetPositionFieldCRC           uint32 = 379390882
	equipAttachmentOffsetRotationDegreesFieldCRC    uint32 = 581018980
	equipSheathDataFieldCRC                         uint32 = 1966399264
	equipSheathOffsetPositionFieldCRC               uint32 = 619916990
	equipSheathOffsetRotationDegreesFieldCRC        uint32 = 768083228
	equipOffHandAttachmentFieldCRC                  uint32 = 2388996306
	equipOffHandAttachmentOffsetPositionFieldCRC    uint32 = 2522934056
	equipOffHandAttachmentOffsetRotationDegreesFieldCRC uint32 = 97015342
	equipOffHandSheathDataFieldCRC                  uint32 = 1101872181
	equipOffHandSheathOffsetPositionFieldCRC        uint32 = 1077303719
	equipOffHandSheathOffsetRotationDegreesFieldCRC uint32 = 789454983

	gameDebugSettingsTypeID = "3e5db037-ae49-43e4-8bcc-67f8c511a091"
	combatDebugSettingsTypeID = "3c0e5dc7-06b9-4411-893e-daac101731d3"
	combatSettingsFieldCRC uint32 = 3204566528
	disablePlayerLootDropOnDeathFieldCRC uint32 = 76657494
	disableWeaponDurabilityFieldCRC uint32 = 2559298940
	disableItemDurabilityFieldCRC uint32 = 880532799
	disableDurabilityPenaltyOnDeathFieldCRC uint32 = 429903575

	uiDatabaseTypeID = "7cc2b992-1c5b-4b27-bcb9-790175f09da6"
	unifiedInteractDataTypeID = "ebc0595e-4adb-4323-9527-82d07e30908c"
	interactOptionVectorTypeID = "33d6e083-a124-527f-baac-824deb5cd6e8"
	interactOptionDataTypeID = "f0887e97-5084-413c-bce7-5c24cecb03c0"

	playerBaseAttributesTypeID = "0f40ecc6-ace9-476a-9a5c-b83be6129a4b"
	playerAttributeDataTypeID = "46113bed-540d-4584-92aa-b9223d83875a"
	guildSiegeWindowRegionDataTypeID = "da0aab24-92a0-5ea4-9a1a-5cef4e8c3bf9"
	factionInfluenceConfigDataTypeID = "8ed959c4-b0e3-4d45-84d1-fcaec1c7d1a4"
	validGroupDataTypeID = "4f986681-3060-4a47-9a45-694a027e5f46"
	warDataTypeID = "4febcf31-140c-4ef1-8c53-814daa4426ac"

	settlementProgressionDataTypeID = "0543759c-4cf0-4eba-b0dd-f0f020b480b3"
	progressionCategoryEntryTypeID = "e1766b2b-75fd-4eb2-ab13-0e5f343b7e68"
	progressionSpawnerEntryTypeID = "d91778a1-a110-46e4-8b9a-30402d8996d6"
	settlementProgressionCategoryVectorTypeID = "2d93cc0a-78e0-5fdf-af40-c2f0491facd0"
	progressionSpawnerEntryVectorTypeID = "3999d332-be04-5382-9e40-a2bf965e61eb"
	settlementProgressionCategoriesFieldCRC uint32 = 2439926458
	settlementProgressionCategoryFieldCRC uint32 = 1188522208
	settlementProgressionEntriesFieldCRC uint32 = 1770189871
	settlementProgressionCategoryLevelFieldCRC uint32 = 2587150535
	sliceFieldCRC uint32 = 1034844325
	alternateSliceFieldCRC uint32 = 1867428434
	displayLocStringFieldCRC uint32 = 457484292
	iconFieldCRC uint32 = 1704208859
	baseClassFieldCRC uint32 = 3566360373
	assetPathFieldCRC uint32 = 741691769

	gatheringDatabaseTypeID = "1ef311cc-a16e-426d-9763-a40473495330"
	gatheringDataTypeID = "579abcc6-ec1e-4157-abc5-2569c7624b0a"
	gatheringActionDatabaseTypeID = "9ac82655-bc8f-4165-ae2f-6d6f3d543d9a"
	gatheringActionDataTypeID = "a6b5258c-2984-4225-88e9-b66813457286"
	gatheringActionTypeID = "5cfd353d-418d-4421-a207-2c748cfbdd16"
	gatheringTypeDataTypeID = "3266a19a-6bac-4703-b663-9f6ed48f1d76"
	gatheringTypeDataVectorTypeID = "779755e7-d85d-5d47-91d5-5fdbb851da57"
	gatheringActionVectorTypeID = "0c5b29e6-74c4-5adf-8fcf-c3204a7e4662"
	gatheringActionDataVectorTypeID = "ceef81af-b476-5463-af1e-b7ec9f65c02f"
	gatheringDataFieldCRC uint32 = 2208564949
	gatheringTypesFieldCRC uint32 = 2065483900
	gatheringActionsFieldCRC uint32 = 1482662604
	requiredWaterGatheringTypeFieldCRC uint32 = 674599067
	noneGatheringTypeFieldCRC uint32 = 3194172210
	typeFieldCRC uint32 = 2363381545
	uiIconFieldCRC uint32 = 2312546211
	requirementTextFieldCRC uint32 = 2484547296
	nameFieldCRC uint32 = 1579384326
	mannequinTagFieldCRC uint32 = 2777524544

	craftingStationDatabaseTypeID = "72175d3e-9370-4b21-970f-dc2adc11e52b"
	craftingStationDataVectorTypeID = "866eb75e-8cfd-511b-a4f0-b8dfa17138bd"
	craftingStationDataTypeID = "75cfb9e3-fe11-4d1d-ac0a-44916a5c27a0"
	craftingTypeStringVectorTypeID = "99dad0bc-740e-5e82-826b-8fc7968cc02c"
	craftingStationsFieldCRC uint32 = 2156395334
	craftingTypesFieldCRC uint32 = 169774472
	craftingMannequinTagFieldCRC uint32 = 1024826923
	azothDiscountPercentFieldCRC uint32 = 757151162

	socialRankDatabaseTypeID = "b0024f1f-651d-48a5-a56a-9dea80cb487e"
	socialRankDataVectorTypeID = "1297b8af-3355-5871-914e-82178f34b16e"
	socialRankDataTypeID = "2f2c2714-e932-43bf-a702-cacd8c9ae544"
	socialGuildRankDataTypeID = "e756a995-93ed-f487-1a76-23b1ad74df11"
	socialPrivilegeIDSetTypeID = "4c9c7f67-4b86-58af-b45a-abf4d2eae45f"
	socialRanksFieldCRC uint32 = 3420889108
	socialGuildRankDataFieldCRC uint32 = 2999919934
	socialGuildRankNameFieldCRC uint32 = 3230417959
	socialGuildRankSecurityLevelFieldCRC uint32 = 265698600
	socialGuildRankAllPrivilegesFieldCRC uint32 = 928054442
	socialGuildRankPrivilegeIDsFieldCRC uint32 = 2614315740
)

var tradeskillTypes = []string{
	"Weaponsmithing", "Armoring", "Jewelcrafting", "Arcana", "Cooking", "Furnishing",
	"Engineering", "Smelting", "Woodworking", "Leatherworking", "Weaving", "Stonecutting",
	"Skinning", "Mining", "Logging", "Harvesting", "WildernessSurvival", "Fishing",
	"AzothStaff", "Musician", "Riding",
}

func normalizeTradeskillType(value any) (string, error) {
	switch value := value.(type) {
	case uint8:
		return normalizeTradeskillType(int(value))
	case uint32:
		return normalizeTradeskillType(int(value))
	case int:
		if value == 255 {
			return "None", nil
		}
		if value >= 0 && value < len(tradeskillTypes) {
			return tradeskillTypes[value], nil
		}
		return "", fmt.Errorf("unknown TradeskillType value %d", value)
	case string:
		normalized := strings.TrimSpace(value)
		if normalized == "None" {
			return normalized, nil
		}
		for _, candidate := range tradeskillTypes {
			if candidate == normalized {
				return normalized, nil
			}
		}
		return "", fmt.Errorf("unknown TradeskillType %s", value)
	default:
		return normalizeTradeskillType(fmt.Sprint(value))
	}
}

func ParsePlayerBaseAttributes(bytes []byte) (*PlayerBaseAttributes, error) {
	root, err := strictObjectStreamRoot(bytes, playerBaseAttributesTypeID)
	if err != nil {
		return nil, err
	}
	playerAttributeElement, err := requiredSection(root, "Player Attribute Data", playerAttributeDataTypeID)
	if err != nil { return nil, err }
	playerAttributeData, err := parsePlayerAttributeData(playerAttributeElement)
	if err != nil { return nil, err }
	guildRegionElement, err := requiredSection(root, "Guild Siege Window Region Data", guildSiegeWindowRegionDataTypeID)
	if err != nil { return nil, err }
	guildRegions, err := parseGuildRegions(guildRegionElement)
	if err != nil { return nil, err }
	factionInfluenceElement, err := requiredSection(root, "Faction Influence Config Data", factionInfluenceConfigDataTypeID)
	if err != nil { return nil, err }
	factionInfluence, err := parseFactionInfluenceConfig(factionInfluenceElement)
	if err != nil { return nil, err }
	validGroupElement, err := requiredSection(root, "Valid Group Data", validGroupDataTypeID)
	if err != nil { return nil, err }
	validGroupData, err := parseValidGroupData(validGroupElement)
	if err != nil { return nil, err }
	warElement, err := requiredSection(root, "War Data", warDataTypeID)
	if err != nil { return nil, err }
	warData, err := parseWarData(warElement)
	if err != nil { return nil, err }
	return &PlayerBaseAttributes{
		PlayerAttributeData: playerAttributeData,
		GuildSiegeWindowRegionData: guildRegions,
		FactionInfluenceConfigData: factionInfluence,
		ValidGroupData: validGroupData,
		WarData: warData,
	}, nil
}

func parsePlayerAttributeData(element *gameassets.ObjectStreamElement) (PlayerAttributeData, error) {
	var out PlayerAttributeData
	var err error
	if out.BaseDeployableLimit, err = requiredI32FieldByName(element, "Base Deployable Limit"); err != nil { return out, err }
	if out.PlayerDisplayLevelUnlockFreeGearSets, err = requiredI32FieldByName(element, "Player Display Level Unlock Free Gear Sets"); err != nil { return out, err }
	rarityElement, err := requiredFieldByName(element, "Item Rarity Data")
	if err != nil { return out, err }
	for index := range rarityElement.Children {
		value, err := parseItemRarityData(&rarityElement.Children[index])
		if err != nil { return out, err }
		out.ItemRarityData = append(out.ItemRarityData, value)
	}
	perkGenerationElement, err := requiredFieldByName(element, "Perk Generation Data")
	if err != nil { return out, err }
	if out.PerkGenerationData, err = parsePerkGenerationData(perkGenerationElement); err != nil { return out, err }
	if out.PerkChanceItemID, err = requiredStringFieldByName(element, "Perk Chance ItemId"); err != nil { return out, err }
	if out.AbilityPointsRequiredInTreeToUnlockFinalRow, err = requiredI32FieldByName(element, "Ability Points Required In Tree to Unlock Final Row"); err != nil { return out, err }
	if out.PerkChanceModifier, err = requiredF32FieldByName(element, "Perk Chance Modifier"); err != nil { return out, err }
	if out.AttributeChanceModifier, err = requiredF32FieldByName(element, "Attribute Chance Modifier"); err != nil { return out, err }
	if out.GemSlotChanceModifier, err = requiredF32FieldByName(element, "Gem Slot Chance Modifier"); err != nil { return out, err }
	return out, nil
}

func parseItemRarityData(element *gameassets.ObjectStreamElement) (ItemRarityData, error) {
	var out ItemRarityData
	var err error
	if out.RarityLevelLocString, err = requiredStringFieldByName(element, "Rarity Level Loc String"); err != nil { return out, err }
	if out.MaxPerkCount, err = requiredI32FieldByName(element, "Max Perk Count"); err != nil { return out, err }
	if out.LevelRequirementModifier, err = requiredI32FieldByName(element, "Level Requirement Modifier"); err != nil { return out, err }
	return out, nil
}

func parsePerkGenerationData(element *gameassets.ObjectStreamElement) (PerkGenerationData, error) {
	var out PerkGenerationData
	var err error
	perTier, err := requiredFieldByName(element, "Perk Data Per Tier")
	if err != nil { return out, err }
	for index := range perTier.Children {
		value, err := parsePerkTierData(&perTier.Children[index])
		if err != nil { return out, err }
		out.PerkDataPerTier = append(out.PerkDataPerTier, value)
	}
	if out.CraftingResultLootBucketID, err = requiredCrc32FieldByName(element, "Crafting Result Loot Bucket Id"); err != nil { return out, err }
	if out.CraftingResultLootBucket, err = requiredStringFieldByName(element, "Crafting Result Loot Bucket"); err != nil { return out, err }
	if out.RollPerkOnUpgradeGS, err = requiredI32FieldByName(element, "Roll Perk On Upgrade GS"); err != nil { return out, err }
	if out.RollPerkOnUpgradeTier, err = requiredI32FieldByName(element, "Roll Perk On Upgrade Tier"); err != nil { return out, err }
	if out.RollPerkOnUpgradePerkCount, err = requiredI32FieldByName(element, "Roll Perk On Upgrade Perk Count"); err != nil { return out, err }
	return out, nil
}

func parsePerkTierData(element *gameassets.ObjectStreamElement) (PerkTierData, error) {
	var out PerkTierData
	var err error
	if out.MaxPerkChannel, err = requiredI32FieldByName(element, "Max Perk Channel"); err != nil { return out, err }
	if out.GemSlotProbability, err = requiredF32FieldByName(element, "Gem Slot Probability"); err != nil { return out, err }
	if out.AttributePerkProbability, err = requiredF32FieldByName(element, "Attribute Perk Probability"); err != nil { return out, err }
	general, err := requiredFieldByName(element, "General Gear Score Perk Count")
	if err != nil { return out, err }
	if out.GeneralGearScorePerkCount, err = parseI32RangeMap(general); err != nil { return out, err }
	crafting, err := requiredFieldByName(element, "Crafting Gear Score Perk Count")
	if err != nil { return out, err }
	if out.CraftingGearScorePerkCount, err = parseI32RangeMap(crafting); err != nil { return out, err }
	if out.AttributePerkBucket, err = requiredStringFieldByName(element, "Attribute Perk Bucket"); err != nil { return out, err }
	if out.AttributePerkBucketID, err = requiredCrc32FieldByName(element, "Attribute Perk Bucket Id"); err != nil { return out, err }
	return out, nil
}

func parseI32RangeMap(element *gameassets.ObjectStreamElement) (map[int32][]IntRange, error) {
	out := map[int32][]IntRange{}
	for index := range element.Children {
		pair := &element.Children[index]
		key, err := requiredI32FieldByName(pair, "value1")
		if err != nil { return nil, err }
		values, err := requiredFieldByName(pair, "value2")
		if err != nil { return nil, err }
		for rangeIndex := range values.Children {
			rangeElement := &values.Children[rangeIndex]
			min, err := requiredI32FieldByName(rangeElement, "value1")
			if err != nil { return nil, err }
			max, err := requiredI32FieldByName(rangeElement, "value2")
			if err != nil { return nil, err }
			out[key] = append(out[key], IntRange{Min: min, Max: max})
		}
	}
	return out, nil
}

func parseGuildRegions(element *gameassets.ObjectStreamElement) (map[string]GuildSiegeWindowRegionData, error) {
	out := map[string]GuildSiegeWindowRegionData{}
	for index := range element.Children {
		pair := &element.Children[index]
		key, err := requiredStringFieldByName(pair, "value1")
		if err != nil { return nil, err }
		valueElement, err := requiredFieldByName(pair, "value2")
		if err != nil { return nil, err }
		value, err := parseGuildRegion(valueElement)
		if err != nil { return nil, err }
		out[key] = value
	}
	return out, nil
}

func parseGuildRegion(element *gameassets.ObjectStreamElement) (GuildSiegeWindowRegionData, error) {
	var out GuildSiegeWindowRegionData
	var err error
	if out.StartHour, err = requiredU32FieldByName(element, "Start Hour"); err != nil { return out, err }
	if out.EndHour, err = requiredU32FieldByName(element, "End Hour"); err != nil { return out, err }
	if out.UTCOffset, err = requiredI32FieldByName(element, "UTCOffset"); err != nil { return out, err }
	if out.DstRuleID, err = requiredCrc32FieldByName(element, "DstRuleId"); err != nil { return out, err }
	if out.DstRule, err = requiredStringFieldByName(element, "DstRule"); err != nil { return out, err }
	if out.ObservesDst, err = requiredBoolFieldByName(element, "ObservesDst"); err != nil { return out, err }
	return out, nil
}

func parseFactionInfluenceConfig(element *gameassets.ObjectStreamElement) (FactionInfluenceConfigData, error) {
	var out FactionInfluenceConfigData
	var err error
	if out.MaxInfluence, err = requiredF32FieldByName(element, "MaxInfluence"); err != nil { return out, err }
	if out.DecrementRate, err = requiredF32FieldByName(element, "DecrementRate"); err != nil { return out, err }
	if out.IncrementRate, err = requiredF32FieldByName(element, "IncrementRate"); err != nil { return out, err }
	if out.MaxIncrementTimeModifier, err = requiredF32FieldByName(element, "MaxIncrementTimeModifier"); err != nil { return out, err }
	if out.MaxDecrementTimeModifier, err = requiredF32FieldByName(element, "MaxDecrementTimeModifier"); err != nil { return out, err }
	if out.MinimumTimeSinceLastWar, err = requiredF32FieldByName(element, "MinimumTimeSinceLastWar"); err != nil { return out, err }
	if out.MinTerritoryDiffToApplyUDMechanics, err = requiredI32FieldByName(element, "MinTerritoryDiffToApplyUDMechanics"); err != nil { return out, err }
	if out.MinTimeToApplyUDMechanics, err = requiredI32FieldByName(element, "MinTimeToApplyUDMechanics"); err != nil { return out, err }
	if out.UnderDogMissionInfluenceGain, err = requiredF32FieldByName(element, "UnderDogMissionInfluenceGain"); err != nil { return out, err }
	if out.UnderDogMissionInfluenceGainCap, err = requiredF32FieldByName(element, "UnderDogMissionInfluenceGainCap"); err != nil { return out, err }
	if out.UderDogFactionRepGain, err = requiredF32FieldByName(element, "UderDogFactionRepGain"); err != nil { return out, err }
	if out.UnderDogFactionRepGainCap, err = requiredF32FieldByName(element, "UnderDogFactionRepGainCap"); err != nil { return out, err }
	if out.UnderDogPVPInfluenceGain, err = requiredF32FieldByName(element, "UnderDogPVPInfluenceGain"); err != nil { return out, err }
	if out.UnderDogPVPInfluenceGainCap, err = requiredF32FieldByName(element, "UnderDogPVPInfluenceGainCap"); err != nil { return out, err }
	if out.MinimumInfluenceThresholdForWar, err = requiredF32FieldByName(element, "MinimumInfluenceThresholdForWar"); err != nil { return out, err }
	attackerWin, err := requiredFieldByName(element, "Influence Race Attacker Win GameEventId")
	if err != nil { return out, err }
	if out.InfluenceRaceAttackerWinGameEventID, err = parseEditCrc(attackerWin); err != nil { return out, err }
	defenderWin, err := requiredFieldByName(element, "Influence Race Defender Win GameEventId")
	if err != nil { return out, err }
	if out.InfluenceRaceDefenderWinGameEventID, err = parseEditCrc(defenderWin); err != nil { return out, err }
	raceLose, err := requiredFieldByName(element, "Influence Race Lose GameEventId")
	if err != nil { return out, err }
	if out.InfluenceRaceLoseGameEventID, err = parseEditCrc(raceLose); err != nil { return out, err }
	return out, nil
}

func parseValidGroupData(element *gameassets.ObjectStreamElement) (ValidGroupData, error) {
	var out ValidGroupData
	var err error
	if out.Names, err = requiredStringSequenceByName(element, "names"); err != nil { return out, err }
	if out.Objectives, err = requiredStringSequenceByName(element, "Objectives"); err != nil { return out, err }
	if out.IconPaths, err = requiredStringSequenceByName(element, "IconPaths"); err != nil { return out, err }
	colors, err := requiredFieldByName(element, "Colors")
	if err != nil { return out, err }
	for index := range colors.Children {
		color, err := readColorRgba(&colors.Children[index])
		if err != nil { return out, err }
		out.Colors = append(out.Colors, color)
	}
	return out, nil
}

func parseWarData(element *gameassets.ObjectStreamElement) (WarData, error) {
	out := WarData{DeployableLimits: map[uint32]WarDeployableLimitData{}}
	limits, err := requiredFieldByName(element, "Deployable Limits")
	if err != nil { return out, err }
	for index := range limits.Children {
		pair := &limits.Children[index]
		key, err := requiredCrc32FieldByName(pair, "value1")
		if err != nil { return out, err }
		valueElement, err := requiredFieldByName(pair, "value2")
		if err != nil { return out, err }
		value, err := parseWarDeployableLimit(valueElement)
		if err != nil { return out, err }
		out.DeployableLimits[key] = value
	}
	return out, nil
}

func parseWarDeployableLimit(element *gameassets.ObjectStreamElement) (WarDeployableLimitData, error) {
	var out WarDeployableLimitData
	var err error
	if out.ID, err = requiredCrc32FieldByName(element, "m_id"); err != nil { return out, err }
	if out.DisplayName, err = requiredStringFieldByName(element, "m_displayName"); err != nil { return out, err }
	if out.BuildableNames, err = requiredStringSequenceByName(element, "m_buildableNames"); err != nil { return out, err }
	if out.BuildableIDs, err = requiredCrc32SequenceByName(element, "m_buildableIds"); err != nil { return out, err }
	attackerLimits, err := requiredFieldByName(element, "m_attackerLimits")
	if err != nil { return out, err }
	if out.AttackerLimits, err = readI32Triple(attackerLimits); err != nil { return out, err }
	if out.DefenderLimit, err = requiredI32FieldByName(element, "m_defenderLimit"); err != nil { return out, err }
	return out, nil
}

func ParseSettlementProgressionData(bytes []byte) (*SettlementProgressionData, error) {
	root, err := strictObjectStreamRoot(bytes, settlementProgressionDataTypeID)
	if err != nil { return nil, err }
	categories, err := requiredTypedChild(root, settlementProgressionCategoriesFieldCRC, settlementProgressionCategoryVectorTypeID)
	if err != nil { return nil, err }
	out := &SettlementProgressionData{}
	for index := range categories.Children {
		value, err := parseProgressionCategoryEntry(&categories.Children[index])
		if err != nil { return nil, err }
		out.SettlementProgressionCategories = append(out.SettlementProgressionCategories, value)
	}
	return out, nil
}

func parseProgressionCategoryEntry(element *gameassets.ObjectStreamElement) (ProgressionCategoryEntry, error) {
	if err := gameassets.RequireObjectStreamType(element, progressionCategoryEntryTypeID); err != nil { return ProgressionCategoryEntry{}, err }
	category, err := requiredStringField(element, settlementProgressionCategoryFieldCRC)
	if err != nil { return ProgressionCategoryEntry{}, err }
	entries, err := requiredTypedChild(element, settlementProgressionEntriesFieldCRC, progressionSpawnerEntryVectorTypeID)
	if err != nil { return ProgressionCategoryEntry{}, err }
	out := ProgressionCategoryEntry{SettlementProgressionCategory: category}
	for index := range entries.Children {
		value, err := parseProgressionSpawnerEntry(&entries.Children[index])
		if err != nil { return out, err }
		out.SettlementProgressionEntries = append(out.SettlementProgressionEntries, value)
	}
	return out, nil
}

func parseProgressionSpawnerEntry(element *gameassets.ObjectStreamElement) (ProgressionSpawnerEntry, error) {
	if err := gameassets.RequireObjectStreamType(element, progressionSpawnerEntryTypeID); err != nil { return ProgressionSpawnerEntry{}, err }
	var out ProgressionSpawnerEntry
	var err error
	if out.SettlementProgressionCategoryLevel, err = requiredI32Field(element, settlementProgressionCategoryLevelFieldCRC); err != nil { return out, err }
	sliceElement, err := requiredTypedChild(element, sliceFieldCRC, assetTypeID)
	if err != nil { return out, err }
	if out.Slice, err = readAssetReference(sliceElement); err != nil { return out, err }
	alternateSliceElement, err := requiredTypedChild(element, alternateSliceFieldCRC, assetTypeID)
	if err != nil { return out, err }
	if out.AlternateSlice, err = readAssetReference(alternateSliceElement); err != nil { return out, err }
	if out.DisplayLocString, err = requiredStringField(element, displayLocStringFieldCRC); err != nil { return out, err }
	iconElement, err := requiredTypedChild(element, iconFieldCRC, simpleTextureAssetReferenceTypeID)
	if err != nil { return out, err }
	if out.Icon, err = readTextureReference(iconElement); err != nil { return out, err }
	return out, nil
}

func ParseGatheringDatabase(bytes []byte) (*GatheringDatabase, error) {
	root, err := strictObjectStreamRoot(bytes, gatheringDatabaseTypeID)
	if err != nil { return nil, err }
	dataElement, err := requiredTypedChild(root, gatheringDataFieldCRC, gatheringDataTypeID)
	if err != nil { return nil, err }
	data, err := parseGatheringData(dataElement)
	if err != nil { return nil, err }
	return &GatheringDatabase{GatheringData: data}, nil
}

func parseGatheringData(element *gameassets.ObjectStreamElement) (GatheringData, error) {
	var out GatheringData
	var err error
	typesElement, err := requiredTypedChild(element, gatheringTypesFieldCRC, gatheringTypeDataVectorTypeID)
	if err != nil { return out, err }
	for index := range typesElement.Children {
		value, err := parseGatheringTypeData(&typesElement.Children[index])
		if err != nil { return out, err }
		out.GatheringTypes = append(out.GatheringTypes, value)
	}
	actionsElement, err := requiredTypedChild(element, gatheringActionsFieldCRC, gatheringActionVectorTypeID)
	if err != nil { return out, err }
	for index := range actionsElement.Children {
		value, err := parseGatheringAction(&actionsElement.Children[index])
		if err != nil { return out, err }
		out.GatheringActions = append(out.GatheringActions, value)
	}
	if out.RequiredWaterGatheringType, err = requiredStringField(element, requiredWaterGatheringTypeFieldCRC); err != nil { return out, err }
	if out.NoneGatheringType, err = requiredStringField(element, noneGatheringTypeFieldCRC); err != nil { return out, err }
	return out, nil
}

func parseGatheringTypeData(element *gameassets.ObjectStreamElement) (GatheringTypeData, error) {
	if err := gameassets.RequireObjectStreamType(element, gatheringTypeDataTypeID); err != nil { return GatheringTypeData{}, err }
	var out GatheringTypeData
	var err error
	if out.GatheringType, err = requiredStringField(element, typeFieldCRC); err != nil { return out, err }
	icon, err := requiredTypedChild(element, uiIconFieldCRC, simpleTextureAssetReferenceTypeID)
	if err != nil { return out, err }
	if out.UIIcon, err = readTextureReference(icon); err != nil { return out, err }
	if out.RequirementText, err = requiredStringField(element, requirementTextFieldCRC); err != nil { return out, err }
	return out, nil
}

func parseGatheringAction(element *gameassets.ObjectStreamElement) (GatheringAction, error) {
	if err := gameassets.RequireObjectStreamType(element, gatheringActionTypeID); err != nil { return GatheringAction{}, err }
	name, err := requiredStringField(element, nameFieldCRC)
	if err != nil { return GatheringAction{}, err }
	tag, err := requiredStringField(element, mannequinTagFieldCRC)
	if err != nil { return GatheringAction{}, err }
	return GatheringAction{Name: name, MannequinTag: tag}, nil
}

func ParseGatheringActionDatabase(bytes []byte) (*GatheringActionDatabase, error) {
	root, err := strictObjectStreamRoot(bytes, gatheringActionDatabaseTypeID)
	if err != nil { return nil, err }
	actionsElement, err := requiredTypedChild(root, gatheringActionsFieldCRC, gatheringActionDataVectorTypeID)
	if err != nil { return nil, err }
	out := &GatheringActionDatabase{}
	for index := range actionsElement.Children {
		value, err := parseGatheringActionData(&actionsElement.Children[index])
		if err != nil { return nil, err }
		out.GatheringActions = append(out.GatheringActions, value)
	}
	return out, nil
}

func parseGatheringActionData(element *gameassets.ObjectStreamElement) (GatheringActionData, error) {
	if err := gameassets.RequireObjectStreamType(element, gatheringActionDataTypeID); err != nil { return GatheringActionData{}, err }
	name, err := requiredStringField(element, nameFieldCRC)
	if err != nil { return GatheringActionData{}, err }
	tag, err := requiredStringField(element, mannequinTagFieldCRC)
	if err != nil { return GatheringActionData{}, err }
	return GatheringActionData{Name: name, MannequinTag: tag}, nil
}

func ParseCraftingStationDatabase(bytes []byte) (*CraftingStationDatabase, error) {
	root, err := strictObjectStreamRoot(bytes, craftingStationDatabaseTypeID)
	if err != nil { return nil, err }
	stations, err := requiredTypedChild(root, craftingStationsFieldCRC, craftingStationDataVectorTypeID)
	if err != nil { return nil, err }
	out := &CraftingStationDatabase{}
	for index := range stations.Children {
		value, err := parseCraftingStationData(&stations.Children[index])
		if err != nil { return nil, err }
		out.CraftingStations = append(out.CraftingStations, value)
	}
	return out, nil
}

func parseCraftingStationData(element *gameassets.ObjectStreamElement) (CraftingStationData, error) {
	if err := gameassets.RequireObjectStreamType(element, craftingStationDataTypeID); err != nil { return CraftingStationData{}, err }
	var out CraftingStationData
	var err error
	if out.Name, err = requiredStringField(element, nameFieldCRC); err != nil { return out, err }
	craftingTypes, err := requiredTypedChild(element, craftingTypesFieldCRC, craftingTypeStringVectorTypeID)
	if err != nil { return out, err }
	if out.CraftingTypes, err = readStringVector(craftingTypes); err != nil { return out, err }
	if out.MannequinTag, err = requiredStringField(element, craftingMannequinTagFieldCRC); err != nil { return out, err }
	if out.AzothDiscountPercent, err = requiredF32Field(element, azothDiscountPercentFieldCRC); err != nil { return out, err }
	return out, nil
}

func ParseSocialRankDatabase(bytes []byte) (*SocialRankDatabase, error) {
	root, err := strictObjectStreamRoot(bytes, socialRankDatabaseTypeID)
	if err != nil { return nil, err }
	ranks, err := requiredTypedChild(root, socialRanksFieldCRC, socialRankDataVectorTypeID)
	if err != nil { return nil, err }
	out := &SocialRankDatabase{}
	for index := range ranks.Children {
		value, err := parseSocialRankData(&ranks.Children[index])
		if err != nil { return nil, err }
		out.Ranks = append(out.Ranks, value)
	}
	return out, nil
}

func parseSocialRankData(element *gameassets.ObjectStreamElement) (SocialRankData, error) {
	if err := gameassets.RequireObjectStreamType(element, socialRankDataTypeID); err != nil { return SocialRankData{}, err }
	guildRankElement, err := requiredTypedChild(element, socialGuildRankDataFieldCRC, socialGuildRankDataTypeID)
	if err != nil { return SocialRankData{}, err }
	guildRank, err := parseSocialGuildRankData(guildRankElement)
	if err != nil { return SocialRankData{}, err }
	return SocialRankData{GuildRankData: guildRank}, nil
}

func parseSocialGuildRankData(element *gameassets.ObjectStreamElement) (SocialGuildRankData, error) {
	var out SocialGuildRankData
	var err error
	if out.Name, err = requiredStringField(element, socialGuildRankNameFieldCRC); err != nil { return out, err }
	if out.SecurityLevel, err = requiredU32Field(element, socialGuildRankSecurityLevelFieldCRC); err != nil { return out, err }
	if out.AllPrivileges, err = requiredBoolField(element, socialGuildRankAllPrivilegesFieldCRC); err != nil { return out, err }
	privileges, err := requiredTypedChild(element, socialGuildRankPrivilegeIDsFieldCRC, socialPrivilegeIDSetTypeID)
	if err != nil { return out, err }
	for index := range privileges.Children {
		value, err := gameassets.ObjectStreamU32(&privileges.Children[index])
		if err != nil { return out, err }
		out.PrivilegeIDs = append(out.PrivilegeIDs, value)
	}
	return out, nil
}

func ParseArmorOffsetDatabase(bytes []byte) (*ArmorOffsetDatabase, error) {
	root, err := strictObjectStreamRoot(bytes, armorOffsetDatabaseTypeID)
	if err != nil {
		return nil, err
	}
	offsetsElement, err := requiredTypedChild(root, armorOffsetsFieldCRC, armorOffsetVectorTypeID)
	if err != nil {
		return nil, err
	}
	database := &ArmorOffsetDatabase{}
	for index := range offsetsElement.Children {
		offset, err := parseArmorOffsetData(&offsetsElement.Children[index])
		if err != nil {
			return nil, err
		}
		database.Offsets = append(database.Offsets, offset)
	}
	return database, nil
}

func parseArmorOffsetData(element *gameassets.ObjectStreamElement) (ArmorOffsetData, error) {
	if err := gameassets.RequireObjectStreamType(element, armorOffsetDataTypeID); err != nil {
		return ArmorOffsetData{}, err
	}
	name, err := requiredStringField(element, armorOffsetNameFieldCRC)
	if err != nil {
		return ArmorOffsetData{}, err
	}
	attachmentsElement, err := requiredTypedChild(element, armorOffsetAttachmentsFieldCRC, attachmentOffsetVectorTypeID)
	if err != nil {
		return ArmorOffsetData{}, err
	}
	offset := ArmorOffsetData{Name: name}
	for index := range attachmentsElement.Children {
		attachment, err := parseAttachmentOffsetData(&attachmentsElement.Children[index])
		if err != nil {
			return ArmorOffsetData{}, err
		}
		offset.Attachments = append(offset.Attachments, attachment)
	}
	return offset, nil
}

func parseAttachmentOffsetData(element *gameassets.ObjectStreamElement) (AttachmentOffsetData, error) {
	if err := gameassets.RequireObjectStreamType(element, attachmentOffsetDataTypeID); err != nil {
		return AttachmentOffsetData{}, err
	}
	attachment, err := requiredStringField(element, attachmentNameFieldCRC)
	if err != nil {
		return AttachmentOffsetData{}, err
	}
	position, err := requiredVec3Field(element, attachmentOffsetPositionFieldCRC)
	if err != nil {
		return AttachmentOffsetData{}, err
	}
	rotation, err := requiredVec3Field(element, attachmentOffsetRotationDegreesFieldCRC)
	if err != nil {
		return AttachmentOffsetData{}, err
	}
	return AttachmentOffsetData{Attachment: attachment, Position: position, RotationDegrees: rotation}, nil
}

func ArmorOffsetByName(database *ArmorOffsetDatabase, name string) *ArmorOffsetData {
	for index := range database.Offsets {
		if database.Offsets[index].Name == name {
			return &database.Offsets[index]
		}
	}
	return nil
}

func FurthestArmorAttachmentOffset(database *ArmorOffsetDatabase, armorOffsetNames []string, attachmentName string, currentPosition Vec3) *AttachmentOffsetData {
	var best *AttachmentOffsetData
	bestLength := vec3Length(currentPosition)
	for _, offsetName := range armorOffsetNames {
		offset := ArmorOffsetByName(database, offsetName)
		if offset == nil {
			continue
		}
		for index := range offset.Attachments {
			attachment := &offset.Attachments[index]
			if attachment.Attachment != attachmentName {
				continue
			}
			length := vec3Length(attachment.Position)
			if length > bestLength {
				bestLength = length
				best = attachment
			}
		}
	}
	return best
}

func ParseEquipTypesDatabase(bytes []byte) (*EquipTypesDatabase, error) {
	root, err := strictObjectStreamRoot(bytes, equipTypesDatabaseTypeID)
	if err != nil {
		return nil, err
	}
	equipTypesElement, err := requiredTypedChild(root, equipTypesFieldCRC, equipTypeVectorTypeID)
	if err != nil {
		return nil, err
	}
	database := &EquipTypesDatabase{}
	for index := range equipTypesElement.Children {
		equipType, err := parseEquipTypeData(&equipTypesElement.Children[index])
		if err != nil {
			return nil, err
		}
		database.EquipTypes = append(database.EquipTypes, equipType)
	}
	return database, nil
}

func parseEquipTypeData(element *gameassets.ObjectStreamElement) (EquipTypeData, error) {
	if err := gameassets.RequireObjectStreamType(element, equipTypeDataTypeID); err != nil {
		return EquipTypeData{}, err
	}
	name, err := requiredStringField(element, equipNameFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	attachment, err := requiredStringField(element, equipAttachmentFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	attachmentOffsetPosition, err := requiredVec3Field(element, equipAttachmentOffsetPositionFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	attachmentOffsetRotationDegrees, err := requiredVec3Field(element, equipAttachmentOffsetRotationDegreesFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	sheathData, err := requiredStringField(element, equipSheathDataFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	sheathOffsetPosition, err := requiredVec3Field(element, equipSheathOffsetPositionFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	sheathOffsetRotationDegrees, err := requiredVec3Field(element, equipSheathOffsetRotationDegreesFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	offHandAttachment, err := requiredStringField(element, equipOffHandAttachmentFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	offHandAttachmentOffsetPosition, err := requiredVec3Field(element, equipOffHandAttachmentOffsetPositionFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	offHandAttachmentOffsetRotationDegrees, err := requiredVec3Field(element, equipOffHandAttachmentOffsetRotationDegreesFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	offHandSheathData, err := requiredStringField(element, equipOffHandSheathDataFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	offHandSheathOffsetPosition, err := requiredVec3Field(element, equipOffHandSheathOffsetPositionFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	offHandSheathOffsetRotationDegrees, err := requiredVec3Field(element, equipOffHandSheathOffsetRotationDegreesFieldCRC)
	if err != nil {
		return EquipTypeData{}, err
	}
	return EquipTypeData{
		Name: name,
		Attachment: attachment,
		AttachmentOffsetPosition: attachmentOffsetPosition,
		AttachmentOffsetRotationDegrees: attachmentOffsetRotationDegrees,
		SheathData: sheathData,
		SheathOffsetPosition: sheathOffsetPosition,
		SheathOffsetRotationDegrees: sheathOffsetRotationDegrees,
		OffHandAttachment: offHandAttachment,
		OffHandAttachmentOffsetPosition: offHandAttachmentOffsetPosition,
		OffHandAttachmentOffsetRotationDegrees: offHandAttachmentOffsetRotationDegrees,
		OffHandSheathData: offHandSheathData,
		OffHandSheathOffsetPosition: offHandSheathOffsetPosition,
		OffHandSheathOffsetRotationDegrees: offHandSheathOffsetRotationDegrees,
	}, nil
}

func ParseGameDebugSettings(bytes []byte) (*GameDebugSettings, error) {
	root, err := strictObjectStreamRoot(bytes, gameDebugSettingsTypeID)
	if err != nil {
		return nil, err
	}
	combat, err := requiredTypedChild(root, combatSettingsFieldCRC, combatDebugSettingsTypeID)
	if err != nil {
		return nil, err
	}
	disablePlayerLootDropOnDeath, err := requiredBoolField(combat, disablePlayerLootDropOnDeathFieldCRC)
	if err != nil {
		return nil, err
	}
	disableWeaponDurability, err := requiredBoolField(combat, disableWeaponDurabilityFieldCRC)
	if err != nil {
		return nil, err
	}
	disableItemDurability, err := requiredBoolField(combat, disableItemDurabilityFieldCRC)
	if err != nil {
		return nil, err
	}
	disableDurabilityPenaltyOnDeath, err := requiredBoolField(combat, disableDurabilityPenaltyOnDeathFieldCRC)
	if err != nil {
		return nil, err
	}
	return &GameDebugSettings{CombatSettings: CombatDebugSettings{
		DisablePlayerLootDropOnDeath: disablePlayerLootDropOnDeath,
		DisableWeaponDurability: disableWeaponDurability,
		DisableItemDurability: disableItemDurability,
		DisableDurabilityPenaltyOnDeath: disableDurabilityPenaltyOnDeath,
	}}, nil
}

func DisabledCombatToggleCount(combat CombatDebugSettings) int {
	count := 0
	if combat.DisablePlayerLootDropOnDeath { count++ }
	if combat.DisableWeaponDurability { count++ }
	if combat.DisableItemDurability { count++ }
	if combat.DisableDurabilityPenaltyOnDeath { count++ }
	return count
}

func ParseUiDatabase(bytes []byte) (*UiDatabase, error) {
	root, err := strictObjectStreamRoot(bytes, uiDatabaseTypeID)
	if err != nil {
		return nil, err
	}
	unified, err := childAt(root, 0, unifiedInteractDataTypeID)
	if err != nil {
		return nil, err
	}
	optionsElement, err := childAt(unified, 0, interactOptionVectorTypeID)
	if err != nil {
		return nil, err
	}
	database := &UiDatabase{}
	for index := range optionsElement.Children {
		option, err := parseInteractOptionData(&optionsElement.Children[index])
		if err != nil {
			return nil, err
		}
		database.UnifiedInteractData.InteractOptions = append(database.UnifiedInteractData.InteractOptions, option)
	}
	return database, nil
}

func parseInteractOptionData(element *gameassets.ObjectStreamElement) (InteractOptionData, error) {
	if err := gameassets.RequireObjectStreamType(element, interactOptionDataTypeID); err != nil {
		return InteractOptionData{}, err
	}
	if len(element.Children) < 45 {
		return InteractOptionData{}, fmt.Errorf("InteractOptionData has %d children, expected at least 45", len(element.Children))
	}
	var option InteractOptionData
	var err error
	if option.Name, err = stringChild(element, 0); err != nil { return InteractOptionData{}, err }
	if option.DisplayName, err = stringChild(element, 1); err != nil { return InteractOptionData{}, err }
	if option.InteractInputType, err = wrappedI32(&element.Children[2]); err != nil { return InteractOptionData{}, err }
	if option.UIInteractAction, err = wrappedU8(&element.Children[3]); err != nil { return InteractOptionData{}, err }
	if option.AdditionalInfoType, err = wrappedI32(&element.Children[4]); err != nil { return InteractOptionData{}, err }
	if option.InteractOptionCategory, err = wrappedI32(&element.Children[5]); err != nil { return InteractOptionData{}, err }
	if option.DelayedInteractionData, err = parseDelayedInteractionData(&element.Children[6]); err != nil { return InteractOptionData{}, err }
	if option.InteractPrivilegeIDs, err = wrappedU32Children(&element.Children[7]); err != nil { return InteractOptionData{}, err }
	if option.BlueprintPrivilegeID, err = wrappedU32(&element.Children[8]); err != nil { return InteractOptionData{}, err }
	if option.RequiresConfirmation, err = boolChild(element, 9); err != nil { return InteractOptionData{}, err }
	if option.IsCommittedInteraction, err = boolChild(element, 10); err != nil { return InteractOptionData{}, err }
	if option.IsInstantCancel, err = boolChild(element, 11); err != nil { return InteractOptionData{}, err }
	if option.ClosePromptOnInteraction, err = boolChild(element, 12); err != nil { return InteractOptionData{}, err }
	if option.ForceSecondaryInteract, err = boolChild(element, 13); err != nil { return InteractOptionData{}, err }
	if option.OnlyShowIfBoundToCamp, err = boolChild(element, 14); err != nil { return InteractOptionData{}, err }
	if option.DisplayPriority, err = i32Child(element, 15); err != nil { return InteractOptionData{}, err }
	option.InteractOptionIcon = firstStringDescendant(&element.Children[16])
	if option.UIAdditionalInfoSlicePath, err = stringChild(element, 17); err != nil { return InteractOptionData{}, err }
	if option.RequiresSecurityLevelValidation, err = boolChild(element, 18); err != nil { return InteractOptionData{}, err }
	if option.MannequinFragment, err = stringChild(element, 19); err != nil { return InteractOptionData{}, err }
	if option.MannequinTag, err = stringChild(element, 20); err != nil { return InteractOptionData{}, err }
	if option.AlignToInteraction, err = boolChild(element, 21); err != nil { return InteractOptionData{}, err }
	if option.HoldActionPressTime, err = f32Child(element, 22); err != nil { return InteractOptionData{}, err }
	if option.CooldownTime, err = i32Child(element, 23); err != nil { return InteractOptionData{}, err }
	if option.SetOwnershipOnInteract, err = boolChild(element, 24); err != nil { return InteractOptionData{}, err }
	if option.RequiredItemName, err = stringChild(element, 25); err != nil { return InteractOptionData{}, err }
	if option.RequiredItemCount, err = i32Child(element, 26); err != nil { return InteractOptionData{}, err }
	if option.RequiredCurrency, err = i32Child(element, 27); err != nil { return InteractOptionData{}, err }
	if option.Availability, err = wrappedI32(&element.Children[28]); err != nil { return InteractOptionData{}, err }
	if option.SiegeWarfareGameEventName, err = stringChild(element, 29); err != nil { return InteractOptionData{}, err }
	if option.AddedStatusEffects, err = parseEffects(&element.Children[30]); err != nil { return InteractOptionData{}, err }
	if option.RequiredStatusEffects, err = parseEffects(&element.Children[31]); err != nil { return InteractOptionData{}, err }
	if option.RemoveStatusEffects, err = parseEffects(&element.Children[32]); err != nil { return InteractOptionData{}, err }
	if option.ExcludedStatusEffects, err = parseEffects(&element.Children[33]); err != nil { return InteractOptionData{}, err }
	if option.DelayBeforeAddingRemovingEffect, err = f32Child(element, 34); err != nil { return InteractOptionData{}, err }
	if option.RemoveAddedEffectsOnInteractionEnd, err = boolChild(element, 35); err != nil { return InteractOptionData{}, err }
	if option.CheckPvpFlagIsSet, err = boolChild(element, 36); err != nil { return InteractOptionData{}, err }
	if option.FactionRequired, err = boolChild(element, 37); err != nil { return InteractOptionData{}, err }
	if option.ShowInstancedLootItemCount, err = boolChild(element, 38); err != nil { return InteractOptionData{}, err }
	if option.RequiredAchievementName, err = stringChild(element, 39); err != nil { return InteractOptionData{}, err }
	if option.RequiredLevel, err = u32Child(element, 40); err != nil { return InteractOptionData{}, err }
	if option.CommittedInteractionMaxUsageTimeout, err = f32Child(element, 41); err != nil { return InteractOptionData{}, err }
	if option.CommittedInteractionMaxUsageTimeoutNotification, err = stringChild(element, 42); err != nil { return InteractOptionData{}, err }
	if option.CommittedInteractionInactiveTimeout, err = f32Child(element, 43); err != nil { return InteractOptionData{}, err }
	if option.CommittedInteractionInactiveTimeoutNotification, err = stringChild(element, 44); err != nil { return InteractOptionData{}, err }
	return option, nil
}

func InteractOptionByID(options []InteractOptionData, id any) *InteractOptionData {
	var key uint32
	switch value := id.(type) {
	case uint32:
		key = value
	case int:
		key = uint32(value)
	case string:
		key = crc32Lowercase(value)
	default:
		key = crc32Lowercase(fmt.Sprint(value))
	}
	for index := range options {
		if crc32Lowercase(options[index].Name) == key {
			return &options[index]
		}
	}
	return nil
}

func InteractOptionsByCategory(options []InteractOptionData, category int32) []InteractOptionData {
	var out []InteractOptionData
	for _, option := range options {
		if option.InteractOptionCategory == category || option.InteractOptionCategory == AllInteractOptionsCategory {
			out = append(out, option)
		}
	}
	return out
}

func ParseGameCameraSettings(bytes []byte) (*GameCameraSettings, error) {
	xml := strings.TrimPrefix(string(bytes), "\ufeff")
	settings := &GameCameraSettings{Fields: xmlFields(xml)}
	settings.DefaultStateName = settings.Fields["defaultStateName"]
	for _, match := range cameraStatePattern.FindAllStringSubmatch(xml, -1) {
		attrs := xmlAttributes(match[1])
		body := match[2]
		state := CameraStateSettings{
			Name: attrs["name"],
			Include: attrs["include"],
			Fields: xmlFields(body),
		}
		for _, transitionMatch := range fromTransitionPattern.FindAllStringSubmatch(body, -1) {
			transitionAttrs := xmlAttributes(transitionMatch[1])
			transitionFields := xmlFields(transitionMatch[2])
			smooth := parseOptionalFloat32(firstPresent(transitionAttrs["SmoothTime"], transitionAttrs["smoothTime"], transitionFields["SmoothTime"]))
			state.FromTransitions = append(state.FromTransitions, CameraStateTransition{
				FromCamera: firstPresent(transitionAttrs["FromCamera"], transitionAttrs["fromCamera"], transitionFields["FromCamera"]),
				SmoothTime: smooth,
			})
		}
		settings.CameraStates = append(settings.CameraStates, state)
	}
	return settings, nil
}

var cameraStatePattern = regexp.MustCompile(`(?s)<CameraState\b([^>]*)>(.*?)</CameraState>`)
var fromTransitionPattern = regexp.MustCompile(`(?s)<FromTransition\b([^>/]*)(?:/>|>(.*?)</FromTransition>)`)
var xmlEmptyElementPattern = regexp.MustCompile(`<([A-Za-z0-9_]+)\b([^>]*)/>`)
var xmlAttributePattern = regexp.MustCompile(`([A-Za-z0-9_:-]+)\s*=\s*"([^"]*)"`)

func strictObjectStreamRoot(bytes []byte, typeID string) (*gameassets.ObjectStreamElement, error) {
	stream, err := gameassets.ParseObjectStream(bytes)
	if err != nil {
		return nil, err
	}
	if stream.Version != 3 {
		return nil, fmt.Errorf("unsupported ObjectStream version %d", stream.Version)
	}
	return gameassets.SingleObjectStreamRoot(stream, typeID)
}

func requiredTypedChild(element *gameassets.ObjectStreamElement, nameCRC uint32, typeID string) (*gameassets.ObjectStreamElement, error) {
	child, err := gameassets.RequiredChildByNameCRC(element, nameCRC)
	if err != nil {
		return nil, err
	}
	return child, gameassets.RequireObjectStreamType(child, typeID)
}

func requiredStringField(element *gameassets.ObjectStreamElement, nameCRC uint32) (string, error) {
	child, err := requiredTypedChild(element, nameCRC, azstdStringTypeID)
	if err != nil {
		return "", err
	}
	return gameassets.ObjectStreamString(child), nil
}

func requiredVec3Field(element *gameassets.ObjectStreamElement, nameCRC uint32) (Vec3, error) {
	child, err := requiredTypedChild(element, nameCRC, vector3TypeID)
	if err != nil {
		return Vec3{}, err
	}
	return gameassets.ObjectStreamVec3(child)
}

func requiredBoolField(element *gameassets.ObjectStreamElement, nameCRC uint32) (bool, error) {
	child, err := requiredTypedChild(element, nameCRC, boolTypeID)
	if err != nil {
		return false, err
	}
	return gameassets.ObjectStreamBool(child)
}

func requiredI32Field(element *gameassets.ObjectStreamElement, nameCRC uint32) (int32, error) {
	child, err := requiredTypedChild(element, nameCRC, intTypeID)
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamI32(child)
}

func requiredU32Field(element *gameassets.ObjectStreamElement, nameCRC uint32) (uint32, error) {
	child, err := requiredTypedChild(element, nameCRC, u32TypeID)
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamU32(child)
}

func requiredF32Field(element *gameassets.ObjectStreamElement, nameCRC uint32) (float32, error) {
	child, err := requiredTypedChild(element, nameCRC, floatTypeID)
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamF32(child)
}

func requiredSection(element *gameassets.ObjectStreamElement, fieldName string, typeID string) (*gameassets.ObjectStreamElement, error) {
	return requiredTypedChild(element, crc32Lowercase(fieldName), typeID)
}

func requiredFieldByName(element *gameassets.ObjectStreamElement, fieldName string) (*gameassets.ObjectStreamElement, error) {
	return gameassets.RequiredChildByNameCRC(element, crc32Lowercase(fieldName))
}

func requiredStringFieldByName(element *gameassets.ObjectStreamElement, fieldName string) (string, error) {
	return requiredStringField(element, crc32Lowercase(fieldName))
}

func requiredI32FieldByName(element *gameassets.ObjectStreamElement, fieldName string) (int32, error) {
	return requiredI32Field(element, crc32Lowercase(fieldName))
}

func requiredU32FieldByName(element *gameassets.ObjectStreamElement, fieldName string) (uint32, error) {
	return requiredU32Field(element, crc32Lowercase(fieldName))
}

func requiredF32FieldByName(element *gameassets.ObjectStreamElement, fieldName string) (float32, error) {
	return requiredF32Field(element, crc32Lowercase(fieldName))
}

func requiredBoolFieldByName(element *gameassets.ObjectStreamElement, fieldName string) (bool, error) {
	return requiredBoolField(element, crc32Lowercase(fieldName))
}

func requiredCrc32FieldByName(element *gameassets.ObjectStreamElement, fieldName string) (uint32, error) {
	child, err := requiredFieldByName(element, fieldName)
	if err != nil {
		return 0, err
	}
	return readCrc32(child)
}

func requiredStringSequenceByName(element *gameassets.ObjectStreamElement, fieldName string) ([]string, error) {
	child, err := requiredFieldByName(element, fieldName)
	if err != nil {
		return nil, err
	}
	return readStringVector(child)
}

func requiredCrc32SequenceByName(element *gameassets.ObjectStreamElement, fieldName string) ([]uint32, error) {
	child, err := requiredFieldByName(element, fieldName)
	if err != nil {
		return nil, err
	}
	values := make([]uint32, 0, len(child.Children))
	for index := range child.Children {
		value, err := readCrc32(&child.Children[index])
		if err != nil {
			return nil, err
		}
		values = append(values, value)
	}
	return values, nil
}

func readStringVector(element *gameassets.ObjectStreamElement) ([]string, error) {
	values := make([]string, 0, len(element.Children))
	for index := range element.Children {
		child := &element.Children[index]
		if err := gameassets.RequireObjectStreamType(child, azstdStringTypeID); err != nil {
			return nil, err
		}
		values = append(values, gameassets.ObjectStreamString(child))
	}
	return values, nil
}

func readCrc32(element *gameassets.ObjectStreamElement) (uint32, error) {
	if err := gameassets.RequireObjectStreamType(element, crc32TypeID); err != nil {
		return 0, err
	}
	if len(element.Data) == 4 {
		return gameassets.ObjectStreamU32(element)
	}
	value, err := gameassets.RequiredChildByNameCRC(element, crc32Lowercase("Value"))
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamU32(value)
}

func parseEditCrc(element *gameassets.ObjectStreamElement) (EditCrc, error) {
	if err := gameassets.RequireObjectStreamType(element, editCrcTypeID); err != nil {
		return EditCrc{}, err
	}
	valueStr, err := requiredStringFieldByName(element, "m_valueStr")
	if err != nil {
		return EditCrc{}, err
	}
	valueCrc, err := requiredCrc32FieldByName(element, "m_valueCrc")
	if err != nil {
		return EditCrc{}, err
	}
	return EditCrc{ValueStr: valueStr, ValueCrc: valueCrc}, nil
}

func readI32Triple(element *gameassets.ObjectStreamElement) ([3]int32, error) {
	if len(element.Children) != 3 {
		return [3]int32{}, fmt.Errorf("ObjectStream element %s has %d values, expected 3", element.TypeID, len(element.Children))
	}
	var out [3]int32
	for index := range element.Children {
		value, err := readI32Value(&element.Children[index])
		if err != nil {
			return out, err
		}
		out[index] = value
	}
	return out, nil
}

func readI32Value(element *gameassets.ObjectStreamElement) (int32, error) {
	if element.TypeID == intTypeID {
		return gameassets.ObjectStreamI32(element)
	}
	if len(element.Children) == 1 {
		return readI32Value(&element.Children[0])
	}
	return 0, fmt.Errorf("ObjectStream element %s is not an i32 value", element.TypeID)
}

func readColorRgba(element *gameassets.ObjectStreamElement) (ColorRgba, error) {
	if err := gameassets.RequireObjectStreamType(element, colorTypeID); err != nil {
		return ColorRgba{}, err
	}
	if len(element.Data) != 16 {
		return ColorRgba{}, fmt.Errorf("ObjectStream color has %d bytes, expected 16", len(element.Data))
	}
	return ColorRgba{
		R: math.Float32frombits(binary.BigEndian.Uint32(element.Data[0:4])),
		G: math.Float32frombits(binary.BigEndian.Uint32(element.Data[4:8])),
		B: math.Float32frombits(binary.BigEndian.Uint32(element.Data[8:12])),
		A: math.Float32frombits(binary.BigEndian.Uint32(element.Data[12:16])),
	}, nil
}

func readAssetReference(element *gameassets.ObjectStreamElement) (AssetReference, error) {
	if err := gameassets.RequireObjectStreamType(element, assetTypeID); err != nil {
		return AssetReference{}, err
	}
	type assetLayout struct {
		subIDBytes      int
		assetTypeOffset int
		hintLenOffset   int
		hintOffset      int
		reservedStart   int
		reservedEnd     int
	}
	layouts := []assetLayout{
		{subIDBytes: 4, assetTypeOffset: 32, hintLenOffset: 48, hintOffset: 56, reservedStart: 20, reservedEnd: 32},
		{subIDBytes: 4, assetTypeOffset: 24, hintLenOffset: 40, hintOffset: 48, reservedStart: 20, reservedEnd: 24},
		{subIDBytes: 8, assetTypeOffset: 24, hintLenOffset: 40, hintOffset: 48},
		{subIDBytes: 4, assetTypeOffset: 20, hintLenOffset: 36, hintOffset: 44},
	}
	data := element.Data
	for _, layout := range layouts {
		if len(data) < layout.hintOffset {
			continue
		}
		if layout.reservedEnd > 0 {
			ok := true
			for _, b := range data[layout.reservedStart:layout.reservedEnd] {
				if b != 0 {
					ok = false
					break
				}
			}
			if !ok {
				continue
			}
		}
		hintLength := int(binary.BigEndian.Uint64(data[layout.hintLenOffset:layout.hintLenOffset+8]))
		if hintLength != len(data)-layout.hintOffset {
			continue
		}
		var subID uint32
		if layout.subIDBytes == 8 {
			subID = uint32(binary.BigEndian.Uint64(data[16:24]))
		} else {
			subID = binary.BigEndian.Uint32(data[16:20])
		}
		return AssetReference{
			Guid: uuidFromBytes(data[0:16]),
			SubID: subID,
			AssetType: uuidFromBytes(data[layout.assetTypeOffset:layout.assetTypeOffset+16]),
			Hint: string(data[layout.hintOffset:]),
		}, nil
	}
	return AssetReference{}, fmt.Errorf("unsupported AZ::Data::Asset layout with %d bytes", len(data))
}

func readTextureReference(element *gameassets.ObjectStreamElement) (SimpleAssetReferenceTextureAsset, error) {
	if err := gameassets.RequireObjectStreamType(element, simpleTextureAssetReferenceTypeID); err != nil {
		return SimpleAssetReferenceTextureAsset{}, err
	}
	base, err := requiredTypedChild(element, baseClassFieldCRC, simpleAssetReferenceBaseTypeID)
	if err != nil {
		return SimpleAssetReferenceTextureAsset{}, err
	}
	assetPath, err := requiredStringField(base, assetPathFieldCRC)
	if err != nil {
		return SimpleAssetReferenceTextureAsset{}, err
	}
	return SimpleAssetReferenceTextureAsset{AssetPath: assetPath}, nil
}

func uuidFromBytes(bytes []byte) string {
	return fmt.Sprintf("%x-%x-%x-%x-%x", bytes[0:4], bytes[4:6], bytes[6:8], bytes[8:10], bytes[10:16])
}

func childAt(element *gameassets.ObjectStreamElement, index int, typeID ...string) (*gameassets.ObjectStreamElement, error) {
	if index < 0 || index >= len(element.Children) {
		return nil, fmt.Errorf("ObjectStream element %s is missing child %d", element.TypeID, index)
	}
	child := &element.Children[index]
	if len(typeID) > 0 {
		if err := gameassets.RequireObjectStreamType(child, typeID[0]); err != nil {
			return nil, err
		}
	}
	return child, nil
}

func stringChild(element *gameassets.ObjectStreamElement, index int) (string, error) {
	child, err := childAt(element, index, azstdStringTypeID)
	if err != nil {
		return "", err
	}
	return gameassets.ObjectStreamString(child), nil
}

func boolChild(element *gameassets.ObjectStreamElement, index int) (bool, error) {
	child, err := childAt(element, index, boolTypeID)
	if err != nil {
		return false, err
	}
	return gameassets.ObjectStreamBool(child)
}

func i32Child(element *gameassets.ObjectStreamElement, index int) (int32, error) {
	child, err := childAt(element, index, intTypeID)
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamI32(child)
}

func u32Child(element *gameassets.ObjectStreamElement, index int) (uint32, error) {
	child, err := childAt(element, index, u32TypeID)
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamU32(child)
}

func f32Child(element *gameassets.ObjectStreamElement, index int) (float32, error) {
	child, err := childAt(element, index, floatTypeID)
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamF32(child)
}

func wrappedI32(element *gameassets.ObjectStreamElement) (int32, error) {
	child, err := childAt(element, 0, intTypeID)
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamI32(child)
}

func wrappedU8(element *gameassets.ObjectStreamElement) (uint8, error) {
	child, err := childAt(element, 0, u8TypeID)
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamU8(child)
}

func wrappedU32(element *gameassets.ObjectStreamElement) (uint32, error) {
	child, err := childAt(element, 0, u32TypeID)
	if err != nil {
		return 0, err
	}
	return gameassets.ObjectStreamU32(child)
}

func wrappedU32Children(element *gameassets.ObjectStreamElement) ([]uint32, error) {
	values := make([]uint32, 0, len(element.Children))
	for index := range element.Children {
		value, err := wrappedU32(&element.Children[index])
		if err != nil {
			return nil, err
		}
		values = append(values, value)
	}
	return values, nil
}

func parseDelayedInteractionData(element *gameassets.ObjectStreamElement) (DelayedInteractionData, error) {
	delayTime, err := f32Child(element, 0)
	if err != nil {
		return DelayedInteractionData{}, err
	}
	delayMannequinTag, err := stringChild(element, 1)
	if err != nil {
		return DelayedInteractionData{}, err
	}
	return DelayedInteractionData{DelayTime: delayTime, DelayMannequinTag: delayMannequinTag}, nil
}

func parseEffects(element *gameassets.ObjectStreamElement) ([]EffectData, error) {
	effects := make([]EffectData, 0, len(element.Children))
	for index := range element.Children {
		effects = append(effects, EffectData{EffectID: firstStringDescendant(&element.Children[index])})
	}
	return effects, nil
}

func firstStringDescendant(element *gameassets.ObjectStreamElement) string {
	if element.TypeID == azstdStringTypeID {
		return gameassets.ObjectStreamString(element)
	}
	for index := range element.Children {
		if value := firstStringDescendant(&element.Children[index]); value != "" {
			return value
		}
	}
	return ""
}

func vec3Length(value Vec3) float64 {
	return math.Sqrt(float64(value.X*value.X + value.Y*value.Y + value.Z*value.Z))
}

func xmlFields(xml string) map[string]string {
	fields := map[string]string{}
	for _, match := range xmlEmptyElementPattern.FindAllStringSubmatch(xml, -1) {
		attrs := xmlAttributes(match[2])
		name := firstPresent(attrs["name"], match[1])
		fields[name] = attrs["value"]
	}
	return fields
}

func xmlAttributes(source string) map[string]string {
	attrs := map[string]string{}
	for _, match := range xmlAttributePattern.FindAllStringSubmatch(source, -1) {
		attrs[match[1]] = html.UnescapeString(match[2])
	}
	return attrs
}

func parseOptionalFloat32(value string) *float32 {
	if value == "" {
		return nil
	}
	parsed, err := strconv.ParseFloat(strings.TrimSuffix(strings.TrimSuffix(value, "f"), "F"), 32)
	if err != nil {
		return nil
	}
	out := float32(parsed)
	return &out
}

func firstPresent(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}

"#;

const DYNAMIC_MANAGER_RUNTIME_GO: &str = r#"
type managerInstance struct {
	definition       managerDefinition
	tables           map[string]*dynamicTable
	assets           []string
	assetBytesByPath map[string][]byte
}

func (instance *managerInstance) table(name string) *dynamicTable {
	return instance.tables[name]
}

func (instance *managerInstance) assetBytes(path ...string) ([]byte, bool) {
	requested := ""
	if len(path) > 0 {
		requested = path[0]
	} else if len(instance.assets) == 1 {
		requested = instance.assets[0]
	}
	if requested == "" {
		return nil, false
	}
	normalized := normalizeDataPath(requested)
	if bytes, ok := instance.assetBytesByPath[normalized]; ok {
		return bytes, true
	}
	suffix := "/" + normalized
	for candidate, bytes := range instance.assetBytesByPath {
		if strings.HasSuffix(candidate, suffix) {
			return bytes, true
		}
	}
	return nil, false
}

func (instance *managerInstance) requiredAssetBytes(path ...string) ([]byte, error) {
	bytes, ok := instance.assetBytes(path...)
	if !ok {
		requested := "<single>"
		if len(path) > 0 {
			requested = path[0]
		}
		return nil, fmt.Errorf("manager %s asset %s was not loaded", instance.definition.Name, requested)
	}
	return bytes, nil
}

func schemaRows[T any](instance *managerInstance, rowType string, read func(*dynamicTable, dynamicTableRow) (T, error)) ([]T, error) {
	rows := []T{}
	for _, table := range instance.allTables() {
		if table.Schema.RowType != rowType {
			continue
		}
		for _, sourceRow := range table.Rows {
			row, err := read(table, sourceRow)
			if err != nil {
				return nil, err
			}
			rows = append(rows, row)
		}
	}
	return rows, nil
}

func schemaRow[T any](instance *managerInstance, rowType string, key any, read func(*dynamicTable, dynamicTableRow) (T, error), keyOf func(T) any) (*T, error) {
	lookupKey := normalizeLookupKey(key)
	rows, err := schemaRows(instance, rowType, read)
	if err != nil {
		return nil, err
	}
	for index := range rows {
		if normalizeLookupKey(keyOf(rows[index])) == lookupKey {
			return &rows[index], nil
		}
	}
	return nil, nil
}

func (instance *managerInstance) allTables() []*dynamicTable {
	seen := map[*dynamicTable]struct{}{}
	tables := []*dynamicTable{}
	for _, table := range instance.tables {
		if table == nil {
			continue
		}
		if _, exists := seen[table]; exists {
			continue
		}
		seen[table] = struct{}{}
		tables = append(tables, table)
	}
	return tables
}

type ManagerRuntime struct {
	datasheetsByPath map[string]gameassets.DatasheetAsset
	assetsByPath     map[string][]byte
	tableCache       map[string]*dynamicTable
	managerCache     map[string]*managerInstance
}

func NewManagerRuntimeFromPakSource(source *gameassets.PakDatasheetSource) *ManagerRuntime {
	datasheetsByPath := make(map[string]gameassets.DatasheetAsset, len(source.Datasheets))
	for _, asset := range source.Datasheets {
		datasheetsByPath[normalizeDataPath(asset.Path)] = asset
	}
	assetsByPath := make(map[string][]byte, len(source.Assets))
	for _, asset := range source.Assets {
		assetsByPath[normalizeDataPath(asset.Path)] = asset.Bytes
	}
	return &ManagerRuntime{
		datasheetsByPath: datasheetsByPath,
		assetsByPath:     assetsByPath,
		tableCache:       map[string]*dynamicTable{},
		managerCache:     map[string]*managerInstance{},
	}
}

func (runtime *ManagerRuntime) manager(name string) (*managerInstance, error) {
	definition := managerByName(name)
	if definition == nil {
		return nil, fmt.Errorf("unknown manager %s", name)
	}
	return runtime.buildManager(definition, map[string]struct{}{})
}

func (runtime *ManagerRuntime) buildManager(definition *managerDefinition, stack map[string]struct{}) (*managerInstance, error) {
	if cached := runtime.managerCache[definition.Name]; cached != nil {
		return cached, nil
	}
	if _, exists := stack[definition.Name]; exists {
		return nil, fmt.Errorf("manager dependency cycle at %s", definition.Name)
	}
	stack[definition.Name] = struct{}{}

	instance := &managerInstance{
		definition:       *definition,
		tables:           map[string]*dynamicTable{},
		assets:           []string{},
		assetBytesByPath: map[string][]byte{},
	}

	for _, dependency := range definition.Dependencies {
		switch dependency.Kind {
		case managerDependencyTable:
			schema := TableSchemaByNameAndRow(dependency.Name, dependency.Row)
			if schema == nil {
				return nil, fmt.Errorf("manager %s depends on unknown table %s/%s", definition.Name, dependency.Name, dependency.Row)
			}
			table, err := runtime.buildTable(schema)
			if err != nil {
				return nil, err
			}
			instance.tables[dependency.Name] = table
			instance.tables[schema.Name] = table
			instance.tables[schema.Name+":"+schema.RowType] = table
		case managerDependencyAsset:
			instance.assets = append(instance.assets, dependency.Path)
			bytes, ok := runtime.assetBytes(dependency.Path)
			if !ok {
				return nil, fmt.Errorf("asset %s was not loaded", dependency.Path)
			}
			instance.assetBytesByPath[normalizeDataPath(dependency.Path)] = bytes
		}
	}

	delete(stack, definition.Name)
	runtime.managerCache[definition.Name] = instance
	return instance, nil
}

func (runtime *ManagerRuntime) buildTable(schema *TableSchema) (*dynamicTable, error) {
	cacheKey := schema.Name + ":" + schema.RowType
	if cached := runtime.tableCache[cacheKey]; cached != nil {
		return cached, nil
	}

	var rowKeyColumn *ColumnSchema
	for i := range schema.Columns {
		if schema.Columns[i].RowKey {
			rowKeyColumn = &schema.Columns[i]
			break
		}
	}
	if rowKeyColumn == nil {
		return nil, fmt.Errorf("table %s has no row-key column", schema.Name)
	}

	table := &dynamicTable{
		Schema:          *schema,
		Sheets:          []gameassets.Datasheet{},
		Rows:            []dynamicTableRow{},
		RowsByKey:       map[string]dynamicTableRow{},
		RowsByLookupKey: map[string]dynamicTableRow{},
		DuplicateKeys:   map[string][]dynamicTableRow{},
	}

	for _, sourcePath := range schema.Sources {
		asset, ok := runtime.datasheetAsset(sourcePath)
		if !ok {
			return nil, fmt.Errorf("datasheet source %s was not loaded", sourcePath)
		}
		sheet, err := gameassets.ParseDatasheet(asset.Bytes)
		if err != nil {
			return nil, err
		}
		columnSlots := columnSlotsForSheet(schema, &sheet)
		rowKeySlot, ok := columnSlots[rowKeyColumn.CRC]
		if !ok {
			return nil, fmt.Errorf("datasheet source %s missing row-key column %s", sourcePath, rowKeyColumn.Name)
		}
		table.Sheets = append(table.Sheets, sheet)
		for rowIndex, row := range sheet.Rows {
			keyCell := row.Cells[rowKeySlot]
			key, ok := rowKeyValue(keyCell.Value)
			if !ok {
				continue
			}
			dynamicRow := dynamicTableRow{
				SourcePath:  asset.Path,
				RowIndex:    rowIndex,
				Key:         key,
				Row:         row,
				ColumnSlots: columnSlots,
			}
			table.Rows = append(table.Rows, dynamicRow)
			if _, exists := table.RowsByKey[key]; !exists {
				table.RowsByKey[key] = dynamicRow
			}
			lookupKey := normalizeLookupKey(key)
			if existing, exists := table.RowsByLookupKey[lookupKey]; exists {
				duplicates := table.DuplicateKeys[lookupKey]
				if len(duplicates) == 0 {
					duplicates = append(duplicates, existing)
				}
				duplicates = append(duplicates, dynamicRow)
				table.DuplicateKeys[lookupKey] = duplicates
			} else {
				table.RowsByLookupKey[lookupKey] = dynamicRow
			}
		}
	}

	runtime.tableCache[cacheKey] = table
	return table, nil
}

func columnSlotsForSheet(schema *TableSchema, sheet *gameassets.Datasheet) map[uint32]int {
	slots := map[uint32]int{}
	for _, column := range schema.Columns {
		for index := range sheet.Columns {
			if sheet.Columns[index].CRC == column.CRC {
				slots[column.CRC] = index
				break
			}
		}
	}
	return slots
}

func (runtime *ManagerRuntime) datasheetAsset(sourcePath string) (gameassets.DatasheetAsset, bool) {
	normalized := normalizeDataPath(sourcePath)
	if asset, ok := runtime.datasheetsByPath[normalized]; ok {
		return asset, true
	}
	suffix := "/" + normalized
	for path, asset := range runtime.datasheetsByPath {
		if strings.HasSuffix(path, suffix) {
			return asset, true
		}
	}
	return gameassets.DatasheetAsset{}, false
}

func (runtime *ManagerRuntime) assetBytes(path string) ([]byte, bool) {
	normalized := normalizeDataPath(path)
	if bytes, ok := runtime.assetsByPath[normalized]; ok {
		return bytes, true
	}
	suffix := "/" + normalized
	for candidate, bytes := range runtime.assetsByPath {
		if strings.HasSuffix(candidate, suffix) {
			return bytes, true
		}
	}
	return nil, false
}

func managerByName(name string) *managerDefinition {
	for i := range managers {
		if managers[i].Name == name {
			return &managers[i]
		}
	}
	return nil
}

func rowKeyValue(value gameassets.DatasheetCellValue) (string, bool) {
	switch value.Kind {
	case gameassets.DatasheetCellString:
		text := strings.TrimSpace(value.String)
		return text, text != ""
	case gameassets.DatasheetCellNumber:
		number := float64(value.Number)
		if math.Trunc(number) == number {
			return strconv.FormatInt(int64(number), 10), true
		}
		return strconv.FormatFloat(number, 'f', -1, 32), true
	case gameassets.DatasheetCellBoolean:
		if value.Boolean {
			return "true", true
		}
		return "false", true
	default:
		return "", false
	}
}

func normalizeLookupKey(key any) string {
	switch value := key.(type) {
	case nil:
		return ""
	case *string:
		if value == nil {
			return ""
		}
		return strings.ToLower(strings.TrimSpace(*value))
	case *float32:
		if value == nil {
			return ""
		}
		return strings.ToLower(strings.TrimSpace(fmt.Sprint(*value)))
	case *bool:
		if value == nil {
			return ""
		}
		return strings.ToLower(strings.TrimSpace(fmt.Sprint(*value)))
	}
	return strings.ToLower(strings.TrimSpace(fmt.Sprint(key)))
}

func columnMatches(column *ColumnSchema, name string) bool {
	return column.Name == name || column.FieldName == name
}

func normalizeDataPath(path string) string {
	path = strings.ReplaceAll(path, "\\", "/")
	for strings.Contains(path, "//") {
		path = strings.ReplaceAll(path, "//", "/")
	}
	return strings.ToLower(path)
}
"#;
