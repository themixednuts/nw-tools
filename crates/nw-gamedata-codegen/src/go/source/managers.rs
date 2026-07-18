use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use nw_datasheet::ColumnType;

use crate::compiler::GameDataCompileUnit;
use crate::emit::GameDataCodegenFile;
use crate::go::source::format_go_source;
use crate::manager::{NativeManagerProductKind, NativeManagerShape};
use crate::manager_records::{
    CompositionManagerKind, CompositionManagerSurface, DirectManagerSurface, DirectManagerTable,
    ItemDataManagerSurface, ManagerSurface, SemanticLookupKind, SemanticManagerKey,
    SemanticManagerRecord, SemanticNumericKeyType, SemanticProjectionTransform,
    SemanticRowFilterPredicate, default_direct_manager_row_type, go_field_name, go_local_name,
    go_method_name, manager_accessor_domain, manager_surface_name, manager_surfaces,
    semantic_enum_default_variant, semantic_enum_type_name, semantic_manager_record_unit,
};
use nw_serialize_codegen::{
    GoSourceEmitter as SerializeGoSourceEmitter, GoSourceOptions as SerializeGoSourceOptions,
};

mod native;

const DEFAULT_GO_GAMEASSETS_IMPORT: &str = "example.com/newworld/gamedata/internal/gameassets";
const DEFAULT_GO_ASSETS_IMPORT: &str = "example.com/newworld/gamedata/assets";
const DEFAULT_GO_TYPES_IMPORT: &str = "example.com/newworld/gamedata/types";

pub(super) fn emit_dynamic_manager_files(
    unit: &GameDataCompileUnit,
) -> Result<Vec<GameDataCodegenFile>> {
    let surfaces = manager_surfaces(unit)?;
    let records = semantic_records(&surfaces);
    let manager_source = manager_source(unit, &surfaces)?;
    let mut files = split_go_manager_source(&manager_source)?;
    files.extend([
        GameDataCodegenFile::new(
            "managers/datasheet_catalog.go",
            datasheet_catalog_go_source()?,
        ),
        GameDataCodegenFile::binary(
            "managers/datasheet_catalog.json.gz",
            crate::rust::source::tables::compressed_datasheet_catalog_json(unit)?,
        ),
    ]);
    if !records.is_empty() {
        files.push(GameDataCodegenFile::new(
            "types/manager_rows.go",
            manager_record_types_source(&records)?,
        ));
    }
    Ok(files)
}

fn split_go_manager_source(source: &str) -> Result<Vec<GameDataCodegenFile>> {
    const TARGET_LINES: usize = 800;

    let mut parser = treesitter_types_go::tree_sitter::Parser::new();
    parser
        .set_language(&treesitter_types_go::tree_sitter_go::LANGUAGE.into())
        .map_err(|error| anyhow::anyhow!("configure Go parser: {error}"))?;
    let tree = parser
        .parse(source.as_bytes(), None)
        .ok_or_else(|| anyhow::anyhow!("parse formatted Go manager source"))?;
    let root = tree.root_node();
    if root.has_error() {
        anyhow::bail!("formatted Go manager source contains a syntax error");
    }

    let mut declarations = Vec::<String>::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if matches!(
            child.kind(),
            "package_clause" | "import_declaration" | "comment"
        ) {
            continue;
        }
        let text = child
            .utf8_text(source.as_bytes())
            .map_err(|error| anyhow::anyhow!("read Go declaration: {error}"))?
            .trim();
        if !text.is_empty() {
            declarations.push(text.to_owned());
        }
    }

    let mut chunks = Vec::<String>::new();
    let mut current = String::new();
    let mut current_lines = 0usize;
    for declaration in declarations {
        let declaration_lines = declaration.lines().count() + 2;
        if !current.is_empty() && current_lines + declaration_lines > TARGET_LINES {
            chunks.push(std::mem::take(&mut current));
            current_lines = 0;
        }
        current.push_str(&declaration);
        current.push_str("\n\n");
        current_lines += declaration_lines;
    }
    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
        .into_iter()
        .enumerate()
        .map(|(index, body)| {
            let file_name = go_manager_chunk_name(&body, index);
            let source = go_manager_chunk_source(&body)?;
            Ok(GameDataCodegenFile::new(
                format!("managers/{file_name}"),
                source,
            ))
        })
        .collect()
}

fn go_manager_chunk_name(body: &str, index: usize) -> String {
    let first_name = body
        .lines()
        .find_map(|line| {
            let line = line.trim_start();
            for prefix in ["type ", "func ", "const ", "var "] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    let rest = rest.trim_start_matches('(').trim_start();
                    let candidate = rest
                        .split(|character: char| {
                            !character.is_ascii_alphanumeric() && character != '_'
                        })
                        .next()
                        .unwrap_or_default();
                    if !candidate.is_empty() {
                        return Some(candidate);
                    }
                }
            }
            None
        })
        .unwrap_or("part");
    let name = crate::naming::to_snake_ident(first_name, "part");
    format!("{index:03}_{name}.go")
}

fn go_manager_chunk_source(body: &str) -> Result<String> {
    let qualifiers = go_import_qualifiers(body)?;
    let imports = [
        ("binary", "encoding/binary"),
        ("fmt", "fmt"),
        ("html", "html"),
        ("iter", "iter"),
        ("math", "math"),
        ("regexp", "regexp"),
        ("sort", "sort"),
        ("slices", "slices"),
        ("strconv", "strconv"),
        ("strings", "strings"),
        ("sync", "sync"),
        ("assets", DEFAULT_GO_ASSETS_IMPORT),
        ("gameassets", DEFAULT_GO_GAMEASSETS_IMPORT),
        ("gametypes", DEFAULT_GO_TYPES_IMPORT),
        ("uuid", "github.com/google/uuid"),
    ]
    .into_iter()
    .filter(|(qualifier, _)| qualifiers.contains(*qualifier))
    .map(|(qualifier, path)| match qualifier {
        "gameassets" | "gametypes" => format!("\t{qualifier} {path:?}\n"),
        _ => format!("\t{path:?}\n"),
    })
    .collect::<String>();
    let import_block = if imports.is_empty() {
        String::new()
    } else {
        format!("\nimport (\n{imports})\n")
    };
    format_go_source(&format!("package managers\n{import_block}\n{body}")).map_err(Into::into)
}

fn go_import_qualifiers(source: &str) -> Result<BTreeSet<String>> {
    let parse_source = format!("package managers\n\n{source}");
    let mut parser = treesitter_types_go::tree_sitter::Parser::new();
    parser
        .set_language(&treesitter_types_go::tree_sitter_go::LANGUAGE.into())
        .map_err(|error| anyhow::anyhow!("configure Go parser: {error}"))?;
    let tree = parser
        .parse(parse_source.as_bytes(), None)
        .ok_or_else(|| anyhow::anyhow!("parse Go declarations before import synthesis"))?;
    if tree.root_node().has_error() {
        anyhow::bail!("Go declarations contain a syntax error before import synthesis");
    }

    let mut qualifiers = BTreeSet::new();
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
        let qualifier = match node.kind() {
            "selector_expression" => node.child_by_field_name("operand"),
            "qualified_type" => node.child_by_field_name("package"),
            _ => None,
        };
        let Some(qualifier) = qualifier else {
            continue;
        };
        qualifiers.insert(
            qualifier
                .utf8_text(parse_source.as_bytes())
                .map_err(|error| anyhow::anyhow!("read Go import qualifier: {error}"))?
                .to_owned(),
        );
    }
    Ok(qualifiers)
}

fn manager_source(unit: &GameDataCompileUnit, surfaces: &[ManagerSurface]) -> Result<String> {
    let mut source = format!(
        r#"
package managers

import (
	"encoding/binary"
	"fmt"
	"html"
	"iter"
	"math"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"{}"
	"{}"
	gametypes "{}"
	"github.com/google/uuid"
)

type columnSchema struct {{
	Name   string `json:"name"`
	CRC    uint32 `json:"crc"`
	RowKey bool   `json:"row_key"`
}}

type tableSchema struct {{
	Name    string         `json:"name"`
	RowType string         `json:"row_type"`
	Sources []string       `json:"sources"`
	Columns []columnSchema `json:"columns"`
}}

type dynamicTableRow struct {{
	SourcePath  string
	RowIndex    int
	Key         string
	Row         gameassets.DatasheetRow
	ColumnSlots map[uint32]int
}}

type dynamicTable struct {{
	Schema     tableSchema
	Rows       []dynamicTableRow
	ColumnCRCs map[string]uint32
}}

type Rows[T any] interface {{
	Rows() iter.Seq[T]
}}

func rowValues[T any](values []T) iter.Seq[T] {{
	return func(yield func(T) bool) {{
		for index := range values {{
			if !yield(values[index]) {{
				return
			}}
		}}
	}}
}}

func rowCopy[T any](value T) *T {{
	return &value
}}

func exactUint32(value float32) (uint32, bool) {{
	if value < 0 || value >= 4294967296 || value != float32(uint32(value)) {{
		return 0, false
	}}
	return uint32(value), true
}}

func stringValue(value *string) string {{
	if value == nil {{
		return ""
	}}
	return *value
}}

func float32Value(value *float32) float32 {{
	if value == nil {{
		return 0
	}}
	return *value
}}

func parseFloat32OrZero(value string) float32 {{
	parsed, err := strconv.ParseFloat(strings.TrimSpace(value), 32)
	if err != nil || math.IsNaN(parsed) || math.IsInf(parsed, 0) {{
		return 0
	}}
	return float32(parsed)
}}

func boolFromText(value string) bool {{
	value = strings.TrimSpace(value)
	switch strings.ToLower(value) {{
	case "true", "yes":
		return true
	case "false", "no", "":
		return false
	}}
	parsed, err := strconv.ParseFloat(value, 64)
	return err == nil && parsed != 0
}}

func boolPointer(value bool) *bool {{ return &value }}

func optionalNumberBool(value *float32) *bool {{
	if value == nil {{ return nil }}
	return boolPointer(*value != 0)
}}

func optionalBoolFromText(value string) *bool {{
	if strings.TrimSpace(value) == "" {{ return nil }}
	return boolPointer(boolFromText(value))
}}

func parseStoreProductType(value string) (gametypes.StoreProductType, error) {{
	switch strings.TrimSpace(value) {{
	case "Invalid": return gametypes.StoreProductTypeInvalid, nil
	case "ApparelSkin": return gametypes.StoreProductTypeApparelSkin, nil
	case "ApparelSkinSet": return gametypes.StoreProductTypeApparelSkinSet, nil
	case "Bundle": return gametypes.StoreProductTypeBundle, nil
	case "Campskin": return gametypes.StoreProductTypeCampskin, nil
	case "Emote": return gametypes.StoreProductTypeEmote, nil
	case "EmotePermit": return gametypes.StoreProductTypeEmotePermit, nil
	case "GuildCrestPack": return gametypes.StoreProductTypeGuildCrestPack, nil
	case "HousePet": return gametypes.StoreProductTypeHousePet, nil
	case "HousingItem": return gametypes.StoreProductTypeHousingItem, nil
	case "HousingSet": return gametypes.StoreProductTypeHousingSet, nil
	case "InstrumentSkinDrum": return gametypes.StoreProductTypeInstrumentSkinDrum, nil
	case "InstrumentSkinFlute": return gametypes.StoreProductTypeInstrumentSkinFlute, nil
	case "InstrumentSkinGuitar": return gametypes.StoreProductTypeInstrumentSkinGuitar, nil
	case "InstrumentSkinMandolin": return gametypes.StoreProductTypeInstrumentSkinMandolin, nil
	case "InstrumentSkinUprightBass": return gametypes.StoreProductTypeInstrumentSkinUprightBass, nil
	case "ItemDyePack": return gametypes.StoreProductTypeItemDyePack, nil
	case "Loadout": return gametypes.StoreProductTypeLoadout, nil
	case "MarksOfFortune": return gametypes.StoreProductTypeMarksOfFortune, nil
	case "Mount": return gametypes.StoreProductTypeMount, nil
	case "MountAttachment": return gametypes.StoreProductTypeMountAttachment, nil
	case "MountDye": return gametypes.StoreProductTypeMountDye, nil
	case "MountBear": return gametypes.StoreProductTypeMountBear, nil
	case "MountHorse": return gametypes.StoreProductTypeMountHorse, nil
	case "MountLion": return gametypes.StoreProductTypeMountLion, nil
	case "MountTurkey": return gametypes.StoreProductTypeMountTurkey, nil
	case "MountWolf": return gametypes.StoreProductTypeMountWolf, nil
	case "Service": return gametypes.StoreProductTypeService, nil
	case "Title": return gametypes.StoreProductTypeTitle, nil
	case "Token": return gametypes.StoreProductTypeToken, nil
	case "TokenSingle": return gametypes.StoreProductTypeTokenSingle, nil
	case "TokenPack": return gametypes.StoreProductTypeTokenPack, nil
	case "ToolSkin": return gametypes.StoreProductTypeToolSkin, nil
	case "ToolSkinSet": return gametypes.StoreProductTypeToolSkinSet, nil
	case "WeaponSkinBlunderbass": return gametypes.StoreProductTypeWeaponSkinBlunderbass, nil
	case "WeaponSkinBow": return gametypes.StoreProductTypeWeaponSkinBow, nil
	case "WeaponSkinFireStaff": return gametypes.StoreProductTypeWeaponSkinFireStaff, nil
	case "WeaponSkinFlail": return gametypes.StoreProductTypeWeaponSkinFlail, nil
	case "WeaponSkinGreatAxe": return gametypes.StoreProductTypeWeaponSkinGreatAxe, nil
	case "WeaponSkinGreatsword": return gametypes.StoreProductTypeWeaponSkinGreatsword, nil
	case "WeaponSkinHatchet": return gametypes.StoreProductTypeWeaponSkinHatchet, nil
	case "WeaponSkinIceGauntlet": return gametypes.StoreProductTypeWeaponSkinIceGauntlet, nil
	case "WeaponSkinKiteshield": return gametypes.StoreProductTypeWeaponSkinKiteshield, nil
	case "WeaponSkinLifeStaff": return gametypes.StoreProductTypeWeaponSkinLifeStaff, nil
	case "WeaponSkinMusket": return gametypes.StoreProductTypeWeaponSkinMusket, nil
	case "WeaponSkinRapier": return gametypes.StoreProductTypeWeaponSkinRapier, nil
	case "WeaponSkinShield": return gametypes.StoreProductTypeWeaponSkinShield, nil
	case "WeaponSkinSpear": return gametypes.StoreProductTypeWeaponSkinSpear, nil
	case "WeaponSkinSword": return gametypes.StoreProductTypeWeaponSkinSword, nil
	case "WeaponSkinVoidGauntlet": return gametypes.StoreProductTypeWeaponSkinVoidGauntlet, nil
	case "WeaponSkinWarhammer": return gametypes.StoreProductTypeWeaponSkinWarhammer, nil
	default: return 0, fmt.Errorf("unknown StoreProductType value %q", value)
	}}
}}

type RowRef[TTable ~string, TRow any] struct {{
	table TTable
	path  string
	key   string
}}

func (ref RowRef[TTable, TRow]) Table() TTable {{ return ref.table }}
func (ref RowRef[TTable, TRow]) Key() string {{ return ref.key }}

type RowSlot[TTable ~string, TRow any] struct {{
	table    TTable
	path     string
	rowIndex int
}}

func (slot RowSlot[TTable, TRow]) Table() TTable {{ return slot.table }}
func (slot RowSlot[TTable, TRow]) RowIndex() int {{ return slot.rowIndex }}

type RowEntry[TTable ~string, TRow any] struct {{
	Ref  RowRef[TTable, TRow]
	Slot RowSlot[TTable, TRow]
	Row  TRow
}}

type TableReference struct {{
	Path string
	Key  string
}}

type RowSet[TTable ~string, TRow any] struct {{
	entries      []RowEntry[TTable, TRow]
	tableIndexes map[string]*rowTableIndex
	tableOrder   []TTable
}}

type rowTableIndex struct {{
	entries   []int
	byKey     map[string]int
	byRowIndex map[int]int
}}

func newRowSet[TTable ~string, TRow any](entries []RowEntry[TTable, TRow]) RowSet[TTable, TRow] {{
	tableIndexes := make(map[string]*rowTableIndex)
	tableOrder := make([]TTable, 0)
	for entryIndex := range entries {{
		entry := &entries[entryIndex]
		tableKey := normalizeDataPath(string(entry.Ref.table))
		index := tableIndexes[tableKey]
		if index == nil {{
			index = &rowTableIndex{{
				byKey:      make(map[string]int),
				byRowIndex: make(map[int]int),
			}}
			tableIndexes[tableKey] = index
			tableOrder = append(tableOrder, entry.Ref.table)
		}}
		index.entries = append(index.entries, entryIndex)
		key := normalizeLookupKey(entry.Ref.key)
		if _, exists := index.byKey[key]; !exists {{
			index.byKey[key] = entryIndex
		}}
		if _, exists := index.byRowIndex[entry.Slot.rowIndex]; !exists {{
			index.byRowIndex[entry.Slot.rowIndex] = entryIndex
		}}
	}}
	return RowSet[TTable, TRow]{{
		entries:      entries,
		tableIndexes: tableIndexes,
		tableOrder:   tableOrder,
	}}
}}

func (rows RowSet[TTable, TRow]) Len() int {{
	return len(rows.entries)
}}

func (rows RowSet[TTable, TRow]) Empty() bool {{
	return len(rows.entries) == 0
}}

func (rows RowSet[TTable, TRow]) Rows() iter.Seq[RowEntry[TTable, TRow]] {{
	return func(yield func(RowEntry[TTable, TRow]) bool) {{
		for index := range rows.entries {{
			if !yield(rows.entries[index]) {{
				return
			}}
		}}
	}}
}}

func (rows RowSet[TTable, TRow]) table(table TTable) TableRows[TTable, TRow] {{
	return TableRows[TTable, TRow]{{rows: &rows, table: table}}
}}

func (rows RowSet[TTable, TRow]) Get(ref RowRef[TTable, TRow]) *TRow {{
	index := rows.tableIndex(ref.table)
	if index == nil {{
		return nil
	}}
	entryIndex, exists := index.byKey[normalizeLookupKey(ref.key)]
	if !exists {{
		return nil
	}}
	row := rows.entries[entryIndex].Row
	return &row
}}

func (rows RowSet[TTable, TRow]) RowByIndex(slot RowSlot[TTable, TRow]) *TRow {{
	index := rows.tableIndex(slot.table)
	if index == nil {{
		return nil
	}}
	entryIndex, exists := index.byRowIndex[slot.rowIndex]
	if !exists {{
		return nil
	}}
	row := rows.entries[entryIndex].Row
	return &row
}}

func (rows RowSet[TTable, TRow]) RowKeyByIndex(slot RowSlot[TTable, TRow]) (string, bool) {{
	index := rows.tableIndex(slot.table)
	if index == nil {{
		return "", false
	}}
	entryIndex, exists := index.byRowIndex[slot.rowIndex]
	if !exists {{
		return "", false
	}}
	return rows.entries[entryIndex].Ref.key, true
}}

func (rows RowSet[TTable, TRow]) tableIndex(table TTable) *rowTableIndex {{
	normalized := normalizeDataPath(string(table))
	if index := rows.tableIndexes[normalized]; index != nil {{
		return index
	}}
	for _, candidate := range rows.tableOrder {{
		candidateKey := normalizeDataPath(string(candidate))
		if tablePathMatches(candidateKey, normalized) {{
			return rows.tableIndexes[candidateKey]
		}}
	}}
	return nil
}}

type TableRows[TTable ~string, TRow any] struct {{
	rows  *RowSet[TTable, TRow]
	table TTable
}}

func (rows TableRows[TTable, TRow]) Table() TTable {{
	return rows.table
}}

func (rows TableRows[TTable, TRow]) Rows() iter.Seq[RowEntry[TTable, TRow]] {{
	return func(yield func(RowEntry[TTable, TRow]) bool) {{
		index := rows.rows.tableIndex(rows.table)
		if index == nil {{
			return
		}}
		for _, entryIndex := range index.entries {{
			if !yield(rows.rows.entries[entryIndex]) {{
				return
			}}
		}}
	}}
}}

func (rows TableRows[TTable, TRow]) Get(key string) *TRow {{
	return rows.rows.Get(RowRef[TTable, TRow]{{table: rows.table, key: key}})
}}

func (rows TableRows[TTable, TRow]) RowByIndex(rowIndex int) *TRow {{
	return rows.rows.RowByIndex(RowSlot[TTable, TRow]{{table: rows.table, rowIndex: rowIndex}})
}}

func (rows TableRows[TTable, TRow]) RowKeyByIndex(rowIndex int) (string, bool) {{
	return rows.rows.RowKeyByIndex(RowSlot[TTable, TRow]{{table: rows.table, rowIndex: rowIndex}})
}}

"#,
        DEFAULT_GO_ASSETS_IMPORT, DEFAULT_GO_GAMEASSETS_IMPORT, DEFAULT_GO_TYPES_IMPORT,
    );

    let readable_row_types = direct_schema_row_types(surfaces);
    push_schema_row_types(&mut source, unit, &readable_row_types);
    push_go_enum_parsers(&mut source, surfaces);
    push_direct_row_family_types(&mut source, unit, surfaces);
    push_manager_surface_types(&mut source, unit, surfaces);
    source.push_str(PRODUCT_MANAGER_RUNTIME_GO);
    source.push_str(DYNAMIC_MANAGER_RUNTIME_GO);
    push_go_managers_facade(&mut source, surfaces);

    format_go_source(&qualify_go_shared_types(&source, surfaces)?).map_err(Into::into)
}

fn qualify_go_shared_types(source: &str, surfaces: &[ManagerSurface]) -> Result<String> {
    let mut shared_types = ["CRC32", "UUID", "AssetID", "AssetReference", "Vector3"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    shared_types.extend(
        semantic_records(surfaces)
            .into_iter()
            .map(|record| go_method_name(&record.record_type_name)),
    );
    for shape in semantic_enum_shapes(surfaces) {
        let enum_type = go_method_name(&shape.name);
        shared_types.insert(enum_type.clone());
        shared_types.extend(
            shape
                .variants
                .into_iter()
                .map(|variant| format!("{enum_type}{}", go_method_name(&variant.name))),
        );
    }

    let mut parser = treesitter_types_go::tree_sitter::Parser::new();
    parser
        .set_language(&treesitter_types_go::tree_sitter_go::LANGUAGE.into())
        .map_err(|error| anyhow::anyhow!("configure Go parser: {error}"))?;
    let tree = parser.parse(source.as_bytes(), None).ok_or_else(|| {
        anyhow::anyhow!("parse Go manager source before shared-type qualification")
    })?;
    if tree.root_node().has_error() {
        anyhow::bail!("Go manager source contains a syntax error before shared-type qualification");
    }

    let mut replacements = Vec::new();
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
        if !matches!(node.kind(), "identifier" | "type_identifier") {
            continue;
        }
        let identifier = node
            .utf8_text(source.as_bytes())
            .map_err(|error| anyhow::anyhow!("read Go identifier: {error}"))?;
        if !shared_types.contains(identifier) {
            continue;
        }
        let start = node.start_byte();
        if source.as_bytes()[..start].ends_with(b"gametypes.") {
            continue;
        }
        if go_identifier_is_struct_literal_field(node) {
            continue;
        }
        replacements.push((start, node.end_byte(), identifier));
    }

    let mut qualified = source.to_owned();
    replacements.sort_unstable_by_key(|(start, _, _)| *start);
    for (start, end, identifier) in replacements.into_iter().rev() {
        qualified.replace_range(start..end, &format!("gametypes.{identifier}"));
    }
    Ok(qualified)
}

fn go_identifier_is_struct_literal_field(node: treesitter_types_go::tree_sitter::Node<'_>) -> bool {
    let Some(literal_element) = node.parent() else {
        return false;
    };
    if literal_element.kind() != "literal_element" {
        return false;
    }
    let Some(keyed_element) = literal_element.parent() else {
        return false;
    };
    if keyed_element.kind() != "keyed_element" {
        return false;
    }
    let Some(key) = keyed_element.child_by_field_name("key") else {
        return false;
    };
    if key.start_byte() != literal_element.start_byte()
        || key.end_byte() != literal_element.end_byte()
    {
        return false;
    }
    let Some(literal_value) = keyed_element.parent() else {
        return false;
    };
    if literal_value.kind() != "literal_value" {
        return false;
    }
    let Some(composite_literal) = literal_value.parent() else {
        return false;
    };
    if composite_literal.kind() != "composite_literal" {
        return false;
    }
    let Some(literal_type) = composite_literal.child_by_field_name("type") else {
        return false;
    };
    !matches!(
        literal_type.kind(),
        "map_type" | "slice_type" | "array_type" | "implicit_length_array_type"
    )
}

fn datasheet_catalog_go_source() -> Result<String> {
    format_go_source(
        r#"
package managers

import (
	"bytes"
	"compress/gzip"
	_ "embed"
	"encoding/json"
	"fmt"
	"io"
)

//go:embed datasheet_catalog.json.gz
var compressedDatasheetCatalog []byte

func loadTableSchemas() ([]tableSchema, error) {
	reader, err := gzip.NewReader(bytes.NewReader(compressedDatasheetCatalog))
	if err != nil {
		return nil, fmt.Errorf("open generated datasheet catalog: %w", err)
	}
	jsonBytes, readErr := io.ReadAll(reader)
	closeErr := reader.Close()
	if readErr != nil {
		return nil, fmt.Errorf("read generated datasheet catalog: %w", readErr)
	}
	if closeErr != nil {
		return nil, fmt.Errorf("close generated datasheet catalog: %w", closeErr)
	}
	var schemas []tableSchema
	if err := json.Unmarshal(jsonBytes, &schemas); err != nil {
		return nil, fmt.Errorf("decode generated datasheet catalog: %w", err)
	}
	return schemas, nil
}
"#,
    )
    .map_err(Into::into)
}

fn semantic_records(surfaces: &[ManagerSurface]) -> Vec<SemanticManagerRecord> {
    surfaces
        .iter()
        .flat_map(|surface| match surface {
            ManagerSurface::Semantic(record) => vec![record.clone()],
            ManagerSurface::Native {
                semantic_projections,
                ..
            } => semantic_projections.clone(),
            ManagerSurface::Direct(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_)
            | ManagerSurface::ProductBacked(_) => Vec::new(),
        })
        .collect()
}

fn semantic_enum_shapes(
    surfaces: &[ManagerSurface],
) -> Vec<crate::game_system_schema::GameSystemEnumShape> {
    let mut shapes = BTreeMap::new();
    for shape in surfaces
        .iter()
        .flat_map(|surface| match surface {
            ManagerSurface::Semantic(record) => std::slice::from_ref(record),
            ManagerSurface::Native {
                semantic_projections,
                ..
            } => semantic_projections.as_slice(),
            ManagerSurface::Direct(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_)
            | ManagerSurface::ProductBacked(_) => &[],
        })
        .flat_map(|record| record.fields.iter())
        .filter_map(|field| field.enum_shape.as_ref())
    {
        shapes
            .entry(shape.name.clone())
            .or_insert_with(|| shape.clone());
    }
    shapes.into_values().collect()
}

fn semantic_pair_first_enum_shapes(
    surfaces: &[ManagerSurface],
) -> Vec<crate::game_system_schema::GameSystemEnumShape> {
    let mut shapes = BTreeMap::new();
    for shape in surfaces
        .iter()
        .flat_map(|surface| match surface {
            ManagerSurface::Semantic(record) => std::slice::from_ref(record),
            ManagerSurface::Native {
                semantic_projections,
                ..
            } => semantic_projections.as_slice(),
            ManagerSurface::Direct(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_)
            | ManagerSurface::ProductBacked(_) => &[],
        })
        .flat_map(|record| record.fields.iter())
        .filter_map(|field| field.pair_first_enum_shape.as_ref())
    {
        shapes
            .entry(shape.name.clone())
            .or_insert_with(|| shape.clone());
    }
    shapes.into_values().collect()
}

fn push_go_enum_parsers(source: &mut String, surfaces: &[ManagerSurface]) {
    for shape in semantic_enum_shapes(surfaces) {
        let enum_type = go_method_name(&shape.name);
        source.push_str(&format!(
            "func parse{enum_type}(source string) ({enum_type}, error) {{\n\tswitch strings.TrimSpace(source) {{\n"
        ));
        let mut tokens = BTreeMap::<String, String>::new();
        for variant in &shape.variants {
            let variant_name = go_method_name(&variant.name);
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
                "\tcase {}:\n\t\treturn {enum_type}{variant}, nil\n",
                go_string(&token)
            ));
        }
        source.push_str(&format!(
            "\tdefault:\n\t\treturn 0, fmt.Errorf(\"unknown {} value %q\", source)\n\t}}\n}}\n\n",
            shape.name
        ));
    }
    for shape in semantic_pair_first_enum_shapes(surfaces) {
        let parser = go_pair_enum_parser_name(&shape.name);
        source.push_str(&format!(
            "func {parser}(source string) (uint8, error) {{\n\tswitch strings.TrimSpace(source) {{\n"
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
            source.push_str(&format!(
                "\tcase {}:\n\t\treturn {discriminant}, nil\n",
                go_string(&token)
            ));
        }
        source.push_str(&format!(
            "\tdefault:\n\t\tvalue, err := strconv.ParseUint(strings.TrimSpace(source), 10, 8)\n\t\tif err != nil {{ return 0, fmt.Errorf(\"unknown {} value %q\", source) }}\n\t\treturn uint8(value), nil\n\t}}\n}}\n\n",
            shape.name
        ));
    }
}

fn go_pair_enum_parser_name(enum_name: &str) -> String {
    format!("parse{}Discriminant", go_method_name(enum_name))
}

fn direct_schema_row_types(surfaces: &[ManagerSurface]) -> BTreeSet<String> {
    let mut row_types = BTreeSet::new();
    for surface in surfaces {
        let manager = match surface {
            ManagerSurface::Direct(manager) => manager.clone(),
            ManagerSurface::Native { manager, shape, .. } => {
                go_effective_native_manager_surface(manager, shape)
            }
            ManagerSurface::Semantic(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_)
            | ManagerSurface::ProductBacked(_) => continue,
        };
        row_types.extend(manager.tables.into_iter().map(|table| table.row_type_name));
    }
    row_types
}

fn go_manager_accessor_name(manager_name: &str) -> String {
    go_method_name(manager_accessor_domain(manager_name))
}

fn go_manager_dependency_name(manager_name: &str) -> String {
    go_method_name(manager_name.strip_suffix("Manager").unwrap_or(manager_name))
}

fn go_manager_constructor_name(manager_type: &str) -> String {
    format!("new{manager_type}")
}

fn go_manager_resources_expression<'a>(
    manager_name: &str,
    tables: impl IntoIterator<Item = (&'a str, &'a str)>,
    asset_paths: impl IntoIterator<Item = &'a str>,
) -> String {
    format!(
        "cache.resourcesForTables({}, {}, {})",
        go_string(manager_name),
        go_table_selector_slice(tables),
        go_string_slice(asset_paths)
    )
}

fn go_table_selector_slice<'a>(tables: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let tables = tables
        .into_iter()
        .map(|(name, row_type)| {
            format!(
                "{{name: {}, rowType: {}}}",
                go_string(name),
                go_string(row_type)
            )
        })
        .collect::<Vec<_>>();
    if tables.is_empty() {
        "nil".to_owned()
    } else {
        format!(
            "[]tableSelector{{\n\t{}\n}}",
            tables
                .into_iter()
                .map(|table| format!("{table},"))
                .collect::<Vec<_>>()
                .join("\n\t")
        )
    }
}

fn go_direct_manager_resources_expression(manager: &DirectManagerSurface) -> String {
    let row_types = manager
        .tables
        .iter()
        .map(|table| table.row_type_name.as_str())
        .collect::<BTreeSet<_>>();
    format!(
        "cache.resourcesForRows({}, {}, {})",
        go_string(&manager.manager_name),
        go_string_slice(row_types),
        go_string_slice(manager.products.iter().map(|product| product.path.as_str()))
    )
}

fn go_string_slice<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    let values = values.into_iter().map(go_string).collect::<Vec<_>>();
    if values.is_empty() {
        "nil".to_owned()
    } else {
        format!("[]string{{{}}}", values.join(", "))
    }
}

#[derive(Debug, Clone)]
pub(super) struct GoSchemaRow {
    pub(super) type_name: String,
    pub(super) source_row_type: String,
    pub(super) fields: Vec<GoSchemaField>,
}

#[derive(Debug, Clone)]
pub(super) struct GoSchemaField {
    pub(super) source_name: String,
    pub(super) field_name: String,
    pub(super) column_type: ColumnType,
    pub(super) required: bool,
    pub(super) row_key: bool,
}

fn push_schema_row_types(
    source: &mut String,
    unit: &GameDataCompileUnit,
    readable_row_types: &BTreeSet<String>,
) {
    for row in go_schema_rows(unit) {
        if !readable_row_types.contains(&row.source_row_type) {
            continue;
        }
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
	RowPlaceholders string
	Entries         []LootBucketDataSlotEntry
}

type LootBucketDataSlotEntry struct {
	Slot                  uint16
	LootBucket            *string
	FilterLootedItems     *string
	LootBiasingDisabled   *string
	Tags                  *string
	MatchOne              *string
	Item                  *string
	Quantity              *string
	Odds                  *string
}

func readLootBucketDataSchemaRow(table *dynamicTable, row dynamicTableRow) (LootBucketDataSchemaRow, error) {
	rowPlaceholders, err := requiredStringCell(table, row, "RowPlaceholders")
	if err != nil {
		return LootBucketDataSchemaRow{}, err
	}

	entries := []LootBucketDataSlotEntry{}
	for _, slot := range numberedColumnSlots(table, []string{"LootBucket", "FilterLootedItems", "LootBiasingDisabled", "Tags", "MatchOne", "Item", "Quantity", "Odds"}) {
		lootBucket, err := optionalCellText(table, row, numberedColumnName("LootBucket", slot))
		if err != nil {
			return LootBucketDataSchemaRow{}, err
		}
		filterLootedItems, err := optionalCellText(table, row, numberedColumnName("FilterLootedItems", slot))
		if err != nil {
			return LootBucketDataSchemaRow{}, err
		}
		lootBiasingDisabled, err := optionalCellText(table, row, numberedColumnName("LootBiasingDisabled", slot))
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
		if lootBucket != nil || filterLootedItems != nil || lootBiasingDisabled != nil || tags != nil || matchOne != nil || item != nil || quantity != nil || odds != nil {
			entries = append(entries, LootBucketDataSlotEntry{
				Slot:                slot,
				LootBucket:          lootBucket,
				FilterLootedItems:   filterLootedItems,
				LootBiasingDisabled: lootBiasingDisabled,
				Tags:                tags,
				MatchOne:            matchOne,
				Item:                item,
				Quantity:            quantity,
				Odds:                odds,
			})
		}
	}

	return LootBucketDataSchemaRow{
		RowPlaceholders: rowPlaceholders,
		Entries:         entries,
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
    format!("{}SchemaRow", go_field_name(row_type))
}

#[cfg(test)]
mod tests {
    use nw_datasheet::game_system::Crc32;

    use crate::game_system_schema::{
        GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemDataTablesSchemaReport,
        GameSystemTableSchema,
    };
    use crate::manager_records::{
        DirectManagerTable, DirectProductAsset, ItemDataManagerTable, SemanticLookupMethod,
    };
    use crate::plan::GameDataCodegenPlan;
    use crate::schema::GameDataCompileMode;

    use super::*;

    #[test]
    fn semantic_resources_use_exact_table_schema_identity() {
        let expression = go_manager_resources_expression(
            "ExampleManager",
            [("SharedTable", "ExampleRow")],
            std::iter::empty(),
        );

        assert!(expression.contains("cache.resourcesForTables("));
        assert!(expression.contains("{name: \"SharedTable\", rowType: \"ExampleRow\"},"));
        assert!(!expression.contains("cache.resources("));
    }

    #[test]
    fn skip_empty_semantic_keys_accept_missing_cells() {
        let mut source = String::new();
        push_go_key_materializer(&mut source, &semantic_lookup_record());

        assert!(source.contains("optionalStringCell"));
        assert!(source.contains("keyTextValue == nil"));
        assert!(!source.contains("requiredStringCell"));
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
    fn semantic_materializer_tracks_duplicate_keys_across_all_tables() {
        let record = SemanticManagerRecord {
            manager_name: "ExampleDataManager".to_owned(),
            manager_class_name: "ExampleDataManager".to_owned(),
            record_type_name: "ExampleData".to_owned(),
            tables: vec![
                crate::manager_records::SemanticManagerTable {
                    table_name: "ExampleA".to_owned(),
                    row_type_name: "ExampleRow".to_owned(),
                },
                crate::manager_records::SemanticManagerTable {
                    table_name: "ExampleB".to_owned(),
                    row_type_name: "ExampleRow".to_owned(),
                },
            ],
            key: Some(SemanticManagerKey::Crc {
                key_field: "example_id".to_owned(),
                crc_field: "example_id_crc32".to_owned(),
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
        let record_types = manager_record_types_source(std::slice::from_ref(&record))
            .expect("semantic record types");

        assert!(source.contains("ExampleIDCRC32: keyCRC"));
        assert!(record_types.contains("ExampleIDCRC32 CRC32"));
        assert!(!source.contains("ExampleIDCrc32"));
        assert!(!record_types.contains("ExampleIDCrc32"));

        let seen_index = source
            .find("\tseen := map[any]struct{}{}")
            .expect("materializer should track duplicate keys");
        let table_loop_index = source
            .find("\tfor _, table := range resources.tableOrder {")
            .expect("materializer should iterate tables");
        let row_loop_index = source
            .find("\t\tfor _, sourceRow := range table.Rows {")
            .expect("materializer should iterate rows");
        assert!(
            seen_index < table_loop_index && table_loop_index < row_loop_index,
            "duplicate-key tracking must be scoped across every table and row"
        );
    }

    #[test]
    fn direct_schema_manager_uses_rows_contract_for_primary_row_type() {
        let unit = damage_compile_unit();
        let manager = damage_manager_surface();
        let methods = direct_go_schema_methods(&unit, &manager, true);
        let resources = go_direct_manager_resources_expression(&manager);

        assert!(resources.contains("cache.resourcesForRows"));
        assert!(resources.contains("\"AfflictionData\""));
        assert!(resources.contains("\"DamageTypeData\""));
        assert!(methods.contains(
            "func (manager *DamageDataManager) Rows() iter.Seq[RowEntry[DamageDataTable, DamageDataSchemaRow]]"
        ));
        assert!(methods.contains(
            "func (manager *DamageDataManager) Table(table DamageDataTable) TableRows[DamageDataTable, DamageDataSchemaRow]"
        ));
        assert!(methods.contains("type DamageDataTable string"));
        assert!(
            methods.contains(
            "func (table DamageDataTable) Ref(key string) RowRef[DamageDataTable, DamageDataSchemaRow]"
            )
        );
        assert!(!methods.contains("Table(table string)"));
        assert!(
            methods
                .contains(
                    "func (manager *DamageDataManager) Row(ref RowRef[DamageDataTable, DamageDataSchemaRow]) *DamageDataSchemaRow"
                )
        );
        assert!(methods.contains(
            "func (manager *DamageDataManager) RowByIndex(slot RowSlot[DamageDataTable, DamageDataSchemaRow]) *DamageDataSchemaRow"
        ));
        assert!(methods.contains(
            "func (manager *DamageDataManager) AfflictionDataRows() RowSet[DamageDataAfflictionDataTable, AfflictionDataSchemaRow]"
        ));
        assert!(methods.contains(
            "func (manager *DamageDataManager) DamageTypeDataRows() RowSet[DamageDataDamageTypeDataTable, DamageTypeDataSchemaRow]"
        ));
        assert!(!methods.contains("func (manager *DamageDataManager) AfflictionData() RowSet"));
        assert!(!methods.contains("func (manager *DamageDataManager) DamageTypeData() RowSet"));
        assert!(!methods.contains(
            "func (manager *DamageDataManager) AfflictionData(key string) (*AfflictionDataSchemaRow, error)"
        ));
        assert!(!methods.contains(
            "func (manager *DamageDataManager) Get(key string) (*DamageDataSchemaRow, error)"
        ));
        assert!(!methods.contains(
            "func (manager *DamageDataManager) DamageDataRows() ([]DamageDataSchemaRow, error)"
        ));
        assert!(!methods.contains(
            "func (manager *DamageDataManager) DamageData(key string) (*DamageDataSchemaRow, error)"
        ));
    }

    #[test]
    fn generic_direct_manager_uses_typed_table_selection() {
        let schema_report = GameSystemDataTablesSchemaReport {
            tables: vec![
                schema_table(
                    "ExamplePrimary",
                    "ExampleData",
                    vec![schema_column("ExampleID", ColumnType::String, true)],
                ),
                schema_table(
                    "ExampleSecondary",
                    "ExampleData",
                    vec![schema_column("ExampleID", ColumnType::String, true)],
                ),
            ],
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        };
        let codegen_plan = GameDataCodegenPlan::from_schema_report(
            GameDataCompileMode::SourceFormat,
            &schema_report,
        );
        let unit = GameDataCompileUnit::new(schema_report.clone(), schema_report, codegen_plan);
        let manager = DirectManagerSurface {
            manager_name: "ExampleDataManager".to_owned(),
            manager_class_name: "ExampleDataManager".to_owned(),
            tables: vec![
                DirectManagerTable {
                    table_name: "ExamplePrimary".to_owned(),
                    row_type_name: "ExampleData".to_owned(),
                },
                DirectManagerTable {
                    table_name: "ExampleSecondary".to_owned(),
                    row_type_name: "ExampleData".to_owned(),
                },
            ],
            products: Vec::new(),
        };

        let methods = direct_go_schema_methods(&unit, &manager, true);

        assert!(methods.contains("type ExampleDataTable string"));
        assert!(methods.contains(
            "func (manager *ExampleDataManager) Table(table ExampleDataTable) TableRows[ExampleDataTable, ExampleDataSchemaRow]"
        ));
        assert!(methods.contains(
            "func (table ExampleDataTable) Ref(key string) RowRef[ExampleDataTable, ExampleDataSchemaRow]"
        ));
        assert!(!methods.contains("Table(table string)"));
    }

    #[test]
    fn every_direct_and_native_table_surface_uses_typed_table_selection() {
        let managers = crate::manager::validated_native_manager_specs();
        let surfaces = crate::manager_records::manager_surfaces_from_managers(&managers)
            .expect("manager surfaces");
        let row_types = surfaces
            .iter()
            .filter_map(|surface| match surface {
                ManagerSurface::Direct(manager) | ManagerSurface::Native { manager, .. } => Some(
                    manager
                        .tables
                        .iter()
                        .map(|table| table.row_type_name.clone()),
                ),
                ManagerSurface::Semantic(_)
                | ManagerSurface::ItemData(_)
                | ManagerSurface::Composition(_)
                | ManagerSurface::ProductBacked(_) => None,
            })
            .flatten()
            .collect::<BTreeSet<_>>();
        let schema_report = GameSystemDataTablesSchemaReport {
            tables: row_types
                .into_iter()
                .map(|row_type| {
                    schema_table(
                        &row_type,
                        &row_type,
                        vec![schema_column("ID", ColumnType::String, true)],
                    )
                })
                .collect(),
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        };
        let codegen_plan = GameDataCodegenPlan::from_schema_report(
            GameDataCompileMode::SourceFormat,
            &schema_report,
        );
        let unit = GameDataCompileUnit::new(schema_report.clone(), schema_report, codegen_plan);

        for surface in &surfaces {
            let manager = match surface {
                ManagerSurface::Direct(manager) | ManagerSurface::Native { manager, .. } => manager,
                ManagerSurface::Semantic(_)
                | ManagerSurface::ItemData(_)
                | ManagerSurface::Composition(_)
                | ManagerSurface::ProductBacked(_) => continue,
            };
            if manager.tables.is_empty() {
                continue;
            }
            let methods = direct_go_schema_methods(&unit, manager, true);
            assert!(
                methods.contains("Table string"),
                "{} must emit a manager-specific typed table identifier",
                manager.manager_name
            );
            assert!(
                !methods.contains("Table(table string)"),
                "{} must not expose stringly table selection",
                manager.manager_name
            );
            assert!(
                methods.contains("Ref(key string) RowRef["),
                "{} must construct typed row references",
                manager.manager_name
            );
        }
    }

    #[test]
    fn composition_managers_build_indexes_and_emit_complete_surfaces() {
        assert!(REPLICATION_DATA_MANAGER_GO.contains("indexByID map[gametypes.CRC32]uint16"));
        let index_of = REPLICATION_DATA_MANAGER_GO
            .split("func (manager *ReplicationDataManager) IndexOf")
            .nth(1)
            .expect("IndexOf method")
            .split("func (manager *ReplicationDataManager) IDs")
            .next()
            .expect("IndexOf body");
        assert!(!index_of.contains("for "));

        assert!(
            !STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_GO
                .contains("type StaticTradeskillRankDataMappingManager struct{}")
        );
        assert!(
            STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_GO
                .contains("tradeskillRanksByName map[gametypes.CRC32]int")
        );
        assert!(STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_GO.contains(
            "func (manager *StaticTradeskillRankDataMappingManager) TradeskillRanks() iter.Seq"
        ));
        assert!(
            STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_GO
                .contains("ranks.ExperienceDataRows().Rows()")
        );
        assert!(CURRENCY_EXCHANGE_MAPPING_MANAGER_GO.contains("mappingsByEndpoint map"));
        assert!(VITALS_MODIFIER_MAPPING_MANAGER_GO.contains("entriesByID map"));
    }

    #[test]
    #[should_panic(expected = "declares unsupported product type")]
    fn declared_products_cannot_silently_disappear_from_go_output() {
        let manager = DirectManagerSurface {
            manager_name: "UnsupportedProductManager".to_owned(),
            manager_class_name: "UnsupportedProductManager".to_owned(),
            tables: Vec::new(),
            products: vec![DirectProductAsset {
                path: "sharedassets/example.unsupported".to_owned(),
                product_type: "UnsupportedProduct".to_owned(),
                value_type: "example::UnsupportedProduct".to_owned(),
                manager_getter: "unsupported_product".to_owned(),
            }],
        };

        let _ = go_product_storage(&manager);
    }

    #[test]
    fn every_residual_native_manager_has_an_explicit_indexed_go_contract() {
        let managers = crate::manager::validated_native_manager_specs();
        let surfaces = crate::manager_records::manager_surfaces_from_managers(&managers)
            .expect("manager surfaces");
        let mut covered = 0usize;
        for surface in &surfaces {
            let ManagerSurface::Native { manager, shape, .. } = surface else {
                continue;
            };
            if !is_residual_go_native_shape(shape) {
                continue;
            }
            let effective = go_effective_native_manager_surface(manager, shape);
            let schema_report = GameSystemDataTablesSchemaReport {
                tables: residual_contract_schema_tables(surface, &surfaces),
                diagnostics: Vec::new(),
                type_affinities: Vec::new(),
            };
            let codegen_plan = GameDataCodegenPlan::from_schema_report(
                GameDataCompileMode::SourceFormat,
                &schema_report,
            );
            let unit = GameDataCompileUnit::new(schema_report.clone(), schema_report, codegen_plan);
            let augmentation =
                native::residual_native_manager_augmentation(&unit, &effective, shape);
            assert!(
                !augmentation.fields.is_empty() || !augmentation.methods.is_empty(),
                "{} emitted an empty native contract",
                manager.manager_name
            );
            assert!(
                !augmentation.initializers.is_empty() || effective.tables.is_empty(),
                "{} did not build its indexes during construction",
                manager.manager_name
            );
            assert!(
                !augmentation.methods.contains("Table(table string)"),
                "{} leaked stringly table selection",
                manager.manager_name
            );
            let contract = format!("{}{}", augmentation.declarations, augmentation.methods);
            format_go_source(&format!("package managers\n\n{contract}")).unwrap_or_else(|error| {
                panic!(
                    "{} emitted invalid Go syntax: {error}",
                    manager.manager_name
                )
            });
            let marker = residual_native_contract_marker(shape);
            assert!(
                contract.contains(marker),
                "{} omitted native contract marker {marker}",
                manager.manager_name
            );
            if matches!(shape, NativeManagerShape::VitalsData(_)) {
                assert!(
                    contract.contains("VitalsLevelVariantDataSchemaRow"),
                    "VitalsDataManager must own level-variant rows, not its VitalsBaseDataManager dependency rows"
                );
            }
            let emitted = format!(
                "{}{}{}{}",
                augmentation.declarations,
                augmentation.fields,
                augmentation.initializers,
                augmentation.methods
            );
            assert!(
                !emitted.contains("native") && !emitted.contains("Native"),
                "{} leaked an implementation-history marker",
                manager.manager_name
            );
            covered += 1;
        }
        assert!(
            covered >= 40,
            "expected every residual native manager surface"
        );
    }

    #[test]
    fn every_generic_projection_lowers_before_residual_go_dispatch() {
        let managers = crate::manager::validated_native_manager_specs();
        let surfaces = crate::manager_records::manager_surfaces_from_managers(&managers)
            .expect("manager surfaces");
        let empty_report = GameSystemDataTablesSchemaReport {
            tables: Vec::new(),
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        };
        let codegen_plan = GameDataCodegenPlan::from_schema_report(
            GameDataCompileMode::SourceFormat,
            &empty_report,
        );
        let unit = GameDataCompileUnit::new(empty_report.clone(), empty_report, codegen_plan);
        let mut projection_managers = 0usize;

        for manager in &managers {
            let shape = manager.shape().expect("validated manager shape");
            if !is_generic_semantic_projection_shape(shape) {
                continue;
            }
            projection_managers += 1;
            let manager_name = manager
                .rust_type()
                .as_str()
                .rsplit("::")
                .next()
                .expect("manager type name");
            let records =
                crate::manager_records::semantic_projection_records(manager_name, manager, shape)
                    .expect("generic projection shape must enter semantic projection lowering")
                    .expect("generic projection lowering must succeed");
            assert!(
                !records.is_empty(),
                "{manager_name} lowered to no semantic projection records"
            );

            if let Some(ManagerSurface::Native {
                manager,
                shape,
                semantic_projections,
                ..
            }) = surfaces.iter().find(|surface| {
                crate::manager_records::manager_surface_name(surface) == manager_name
            }) {
                assert_eq!(semantic_projections, &records);
                let augmentation =
                    go_native_manager_augmentation(&unit, manager, shape, semantic_projections);
                assert!(
                    !augmentation.fields.is_empty() && !augmentation.methods.is_empty(),
                    "{manager_name} did not emit its semantic projection contract"
                );
            }
        }

        assert!(
            projection_managers >= 10,
            "expected the validated generic semantic projection family"
        );
    }

    #[test]
    fn grouped_semantic_projection_emits_one_typed_go_manager() {
        let managers = crate::manager::validated_native_manager_specs();
        let surfaces = crate::manager_records::manager_surfaces_from_managers(&managers)
            .expect("manager surfaces");
        let surface = surfaces
            .iter()
            .find(|surface| {
                crate::manager_records::manager_surface_name(surface) == "AffixDataManager"
            })
            .expect("AffixDataManager surface")
            .clone();
        let schema_report = GameSystemDataTablesSchemaReport {
            tables: residual_contract_schema_tables(&surface, &surfaces),
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        };
        let codegen_plan = GameDataCodegenPlan::from_schema_report(
            GameDataCompileMode::SourceFormat,
            &schema_report,
        );
        let unit = GameDataCompileUnit::new(schema_report.clone(), schema_report, codegen_plan);
        let source = manager_source(&unit, &[surface]).expect("grouped Go manager source");

        assert!(source.contains("materializeAffixDataProjection0"));
        assert!(source.contains("materializeAffixStatDataProjection1"));
        assert!(
            source
                .contains("func (manager *AffixDataManager) Rows() iter.Seq[gametypes.AffixData]")
        );
        assert!(source.contains(
            "func (manager *AffixDataManager) AffixStats() iter.Seq[gametypes.AffixStatData]"
        ));
        assert!(!source.contains("func (manager *AffixDataManager) Rows() iter.Seq[*RowEntry["));
    }

    fn residual_contract_schema_tables(
        root: &ManagerSurface,
        surfaces: &[ManagerSurface],
    ) -> Vec<GameSystemTableSchema> {
        let mut pending = vec![crate::manager_records::manager_surface_name(root).to_owned()];
        let mut seen_managers = BTreeSet::new();
        let mut tables = BTreeMap::<(String, String), DirectManagerTable>::new();

        while let Some(manager_name) = pending.pop() {
            if !seen_managers.insert(manager_name.clone()) {
                continue;
            }
            let surface = surfaces
                .iter()
                .find(|surface| {
                    crate::manager_records::manager_surface_name(surface) == manager_name
                })
                .unwrap_or_else(|| panic!("missing dependency surface {manager_name}"));
            match surface {
                ManagerSurface::Direct(manager) | ManagerSurface::ProductBacked(manager) => {
                    for table in &manager.tables {
                        tables.insert(
                            (table.table_name.clone(), table.row_type_name.clone()),
                            table.clone(),
                        );
                    }
                }
                ManagerSurface::Native {
                    manager,
                    shape,
                    dependencies,
                    ..
                } => {
                    for table in go_effective_native_manager_surface(manager, shape).tables {
                        tables.insert(
                            (table.table_name.clone(), table.row_type_name.clone()),
                            table,
                        );
                    }
                    pending.extend(dependencies.iter().cloned());
                }
                ManagerSurface::Semantic(manager) => {
                    for table in &manager.tables {
                        let table = DirectManagerTable {
                            table_name: table.table_name.clone(),
                            row_type_name: table.row_type_name.clone(),
                        };
                        tables.insert(
                            (table.table_name.clone(), table.row_type_name.clone()),
                            table,
                        );
                    }
                }
                ManagerSurface::ItemData(manager) => {
                    for table in &manager.tables {
                        let table = DirectManagerTable {
                            table_name: table.table_name.clone(),
                            row_type_name: table.row_type_name.clone(),
                        };
                        tables.insert(
                            (table.table_name.clone(), table.row_type_name.clone()),
                            table,
                        );
                    }
                }
                ManagerSurface::Composition(manager) => {
                    pending.extend(manager.dependencies.iter().cloned());
                }
            }
        }

        tables
            .into_values()
            .map(|table| {
                schema_table(
                    &table.table_name,
                    &table.row_type_name,
                    residual_contract_columns(),
                )
            })
            .collect()
    }

    #[test]
    fn item_data_manager_uses_rows_contract() {
        let mut source = String::new();
        push_item_data_manager_type(&mut source, &item_data_manager_surface());

        assert!(source.contains("func (manager *ItemDataManager) Rows() iter.Seq[ItemData]"));
        assert!(!source.contains("func (manager *ItemDataManager) Items()"));
    }

    #[test]
    fn semantic_into_crc_lookup_accepts_string_or_crc_key() {
        let mut source = String::new();
        push_semantic_manager_type(&mut source, &semantic_lookup_record());
        let source = format_go_source(&source).expect("semantic manager source should parse");

        assert!(source.contains(
            "func (manager *StaticBackstoryDataManager) Backstory(backstoryID CRC32) *StaticBackstoryData"
        ));
        assert!(source.contains(
            "func (manager *StaticBackstoryDataManager) BackstoryByKey(backstoryKey string) *StaticBackstoryData"
        ));
    }

    #[test]
    fn semantic_managers_emit_only_consumed_indexes() {
        let mut record = semantic_lookup_record();
        record.lookup_methods.clear();
        let mut source = String::new();

        push_semantic_manager_type(&mut source, &record);

        assert!(!source.contains("entriesByKey"));
        assert!(!source.contains("entriesBySourceRow"));
    }

    #[test]
    fn skip_invalid_enum_projection_continues_without_fabricating_a_variant() {
        let mut record = semantic_lookup_record();
        record.fields.push(skip_invalid_enum_field());
        let mut source = String::new();

        push_go_semantic_materializer(&mut source, &record);

        assert!(source.contains(
            "missionGoalTypeValue, err := requiredEnumCell(table, sourceRow, \"MissionGoalType\", parseMissionGoalType)"
        ));
        assert!(source.contains("if err != nil {\n\t\t\t\tcontinue"));
        assert!(!source.contains("MissionGoalTypeInvalid"));
    }

    #[test]
    fn numeric_key_conversion_preserves_uint32_values() {
        assert_eq!(
            go_numeric_key_as_u32("row.Level", SemanticNumericKeyType::U8),
            "uint32(row.Level)"
        );
        assert_eq!(
            go_numeric_key_as_u32("row.Level", SemanticNumericKeyType::U32),
            "row.Level"
        );
    }

    #[test]
    fn import_synthesis_uses_syntax_qualifiers_instead_of_substrings() {
        let qualifiers = go_import_qualifiers(
            "func read(row gameassets.DatasheetRow) iter.Seq[gametypes.CRC32] { return nil }",
        )
        .expect("import qualifiers");

        assert!(qualifiers.contains("gameassets"));
        assert!(qualifiers.contains("gametypes"));
        assert!(qualifiers.contains("iter"));
        assert!(!qualifiers.contains("assets"));
    }

    #[test]
    fn shared_type_qualification_preserves_struct_literal_field_names() {
        let source = r#"
package managers

type Example struct {
	AssetID AssetID
}

func example() Example {
	return Example{AssetID: AssetID{}}
}
"#;
        let qualified =
            qualify_go_shared_types(source, &[]).expect("qualify shared Go types structurally");

        assert!(qualified.contains("AssetID gametypes.AssetID"));
        assert!(qualified.contains("Example{AssetID: gametypes.AssetID{}}"));
        assert!(!qualified.contains("gametypes.AssetID:"));
    }

    fn damage_compile_unit() -> GameDataCompileUnit {
        let schema_report = damage_schema_report();
        let codegen_plan = GameDataCodegenPlan::from_schema_report(
            GameDataCompileMode::SourceFormat,
            &schema_report,
        );
        GameDataCompileUnit::new(schema_report.clone(), schema_report, codegen_plan)
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

    fn skip_invalid_enum_field() -> crate::manager_records::SemanticRecordField {
        crate::manager_records::SemanticRecordField {
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
                list: None,
                foreign_keys: Vec::new(),
            },
        }
    }

    fn residual_contract_columns() -> Vec<GameSystemColumnSchema> {
        const STRING_COLUMNS: &[&str] = &[
            "UIPriority",
            "OutputQty",
            "AbilityID",
            "AfflictionID",
            "Attribute",
            "Buff1",
            "Buff2",
            "Buff3",
            "Buff4",
            "Buff5",
            "Buff6",
            "BuffBucketID",
            "BuffType1",
            "BuffType2",
            "BuffType3",
            "BuffType4",
            "BuffType5",
            "BuffType6",
            "Bucket1",
            "BucketID",
            "CampSkinID",
            "CardAndRowID",
            "Category",
            "ChapterID",
            "ChapterRewardID",
            "ChapterType",
            "CraftingCategory",
            "ContainerTypeID",
            "ContributionID",
            "ConversionID",
            "CostumeChangeID",
            "DamageID",
            "DarknessID",
            "DungeonTileID",
            "Dungeon",
            "Dungeon2",
            "Dungeon3",
            "DungeonMiniBoss",
            "DungeonBoss",
            "DynamicDifficultyID",
            "Effect Name",
            "ElementalMutationID",
            "EquipmentSetID",
            "ItemIds",
            "ItemID",
            "Ingredient1",
            "Ingredient2",
            "Ingredient3",
            "Ingredient4",
            "Ingredient5",
            "Ingredient6",
            "Ingredient7",
            "FromItemID",
            "GameEventIDRankAmazing",
            "GameEventIDRankBad",
            "GameEventIDRankGreat",
            "GameEventIDRankOkay",
            "GatherableID",
            "GatheringAction",
            "GatheringType",
            "FootprintID",
            "StructureFootprintID",
            "JourneyTaskID",
            "LootBucketID",
            "MountID",
            "ObjectiveID",
            "TimedRaceNodeTypeId",
            "ParticleID",
            "PrefabPath",
            "ProfileName",
            "ProfileType",
            "ProgressionPointID",
            "PointPoolID",
            "PoolCategory",
            "TerritoryBonusCategory",
            "RequiredCategoricalProgressionID",
            "RequiredProgressionPointID",
            "Description",
            "UpgradeCardDescription",
            "UpgradeCardSprite",
            "UpgradeCardIcon",
            "UpgradeCardCategory",
            "UpgradeCardStat",
            "PromotionMutationID",
            "Promotion1",
            "Promotion2",
            "Promotion3",
            "QuickCourseID",
            "PathReferenceQuickCourseID",
            "AudioGroup",
            "RewardID",
            "Reward(s)",
            "Tag1",
            "MatchOne1",
            "Type1",
            "SelectOnceOnly1",
            "ExcludeTypeStage1",
            "ExcludeTypeShop1",
            "RotationalQueueID",
            "QueueStartTime",
            "QueueEndTime",
            "QueueGameModes",
            "Notes",
            "RuleID",
            "Category",
            "Hub",
            "Zone",
            "SheetID",
            "Instrument",
            "Pages",
            "Slot01",
            "Slot02",
            "Slot03",
            "Slot04",
            "Slot05",
            "SongID",
            "StatusEffect_1",
            "StatusEffect_2",
            "GameModeIds",
            "StatusID",
            "EffectCategories",
            "StoreCategory",
            "CategoryText",
            "DisplayName",
            "PortraitImage",
            "LandscapeImage",
            "SquareImage",
            "ThumbnailImage",
            "TypeDescription",
            "ChildCategoryList",
            "StoreProductTypeList",
            "StoreProductType",
            "StructurePieceID",
            "TaskID",
            "TerritoryName",
            "TrackedStatID",
            "TradeSkillType",
            "UniqueTagID",
            "CampSkinID",
            "ItemID",
            "RequiredAchievementID",
            "Entitlement",
            "GameEvent",
            "Item",
            "Name",
            "Color",
            "SpecColor",
            "CategoricalProgressionId",
            "IconPath",
            "HiResIconPath",
            "VitalsID",
            "BaseVitalsID",
            "AbilityID",
            "ConversionID",
            "FromItemID",
            "ToItemID",
            "FeatureID",
            "WeaponName",
            "WhisperID",
            "WhisperVfxID",
            "WorldEncounterID",
            "AffectedCreatureTypes",
            "MaxHealthMod",
            "CostumeChangeID",
            "CostumeChangeMesh",
            "DungeonTileID",
            "DungeonTileId",
            "Connections",
            "VariationAssetPaths",
            "SupportedRoomTypes",
            "ContainerTypeID",
            "EquipLoadCCStatusEffectCategories",
            "DarknessID",
            "DarknessLevels",
            "DarknessActivationSpec",
            "DarknessGroupSpec",
            "DifficultyScalingGroup",
            "DifficultyScalingTable",
            "Effect Name",
            "Group",
            "BalanceTarget",
            "BalanceCategory",
            "AbilityBaseDamageAdjustment",
            "AffixStatAdjustment",
            "IncomingHealAdjustment",
            "ConsumableHealAdjustment",
            "HEAD_SLOT_Left",
            "HEAD_SLOT_Right",
            "CHEST_SLOT_Left",
            "CHEST_SLOT_Right",
            "HANDS_SLOT_Left",
            "HANDS_SLOT_Right",
            "LEGS_SLOT_Left",
            "LEGS_SLOT_Right",
            "FEET_SLOT_Left",
            "FEET_SLOT_Right",
            "ActivitiesTaskID",
            "RecipeID",
            "TableType",
            "ReusableScoreboardTabId",
            "BuffBucketID",
            "ContributionID",
            "DynamicDifficultyID",
            "ItemIds",
            "Chapter",
            "ChapterID",
            "ChapterRewardID",
            "ChapterType",
            "RewardType",
        ];
        const NUMBER_COLUMNS: &[&str] = &[
            "Index",
            "EntitlementIndex",
            "CategoryOrder",
            "Quantity",
            "BuyCategoricalProgressionCost",
            "MaxEvents",
            "MinDistance",
            "QueueStartIndex",
            "GameModeTimeSpan",
            "StartingTimerSeconds",
            "NodeTimeOverrideMultiplier",
            "DetectionRadius",
            "AddTimeSeconds",
            "MaxLevel",
            "RequiredCharacterLevel",
            "RequiredCategoricalProgressionLevel",
            "RequiredProgressionPointLevel",
            "TreeID",
            "TreeRowPosition",
            "ExpectedParticipantCount",
            "ScalingFactorMin",
            "ScalingFactorMax",
            "FunctionCoefficient",
            "Rotations",
            "TileSize",
            "Weight",
            "MeshRenderZPosOffset",
            "TerritoryType",
            "DarknessDuration",
            "Max Number",
            "Priority",
            "Constants",
            "PotencyAdjustment",
            "DurationAdjustment",
            "WeaponBaseDamageAdjustment",
            "SelfHealAdjustment",
            "CooldownAdjustment",
            "ColorAmount",
            "ColorOverride",
            "SpecAmount",
            "MaskGlossShift",
            "TradeSkillRewardXP",
            "SubRewardPerc1",
            "SubRewardPerc2",
            "BuffPotency1",
            "BuffPotency2",
            "BuffPotency3",
            "BuffPotency4",
            "BuffPotency5",
            "BuffPotency6",
            "ChapterIndex",
            "DifficultyTier",
            "Level",
            "LevelDisparity",
            "MaximumInfluence",
            "RewardIndex",
            "RewardID1",
            "RandomWeights1",
            "BudgetContribution1",
            "SortOrder",
            "MaxRoll",
            "Qty1",
            "Qty2",
            "Qty3",
            "Qty4",
            "Qty5",
            "Qty6",
            "Qty7",
            "TerritoryID",
        ];
        const BOOLEAN_COLUMNS: &[&str] = &[
            "IsEntitlement",
            "IsEnabled",
            "RollOnPresent",
            "UseLevelGS",
            "Disabled",
            "IsTimed",
            "AccumulateTime",
            "UseTimeOverride",
            "IsAbility",
            "DoNotSpendPoint",
            "MatchesPlayerSkeleton",
            "UpdateEnabled",
            "KeepPerks",
            "Bought",
            "Sold",
            "InContracts",
        ];
        STRING_COLUMNS
            .iter()
            .map(|name| schema_column(name, ColumnType::String, true))
            .chain(
                NUMBER_COLUMNS
                    .iter()
                    .map(|name| schema_column(name, ColumnType::Number, true)),
            )
            .chain(
                BOOLEAN_COLUMNS
                    .iter()
                    .map(|name| schema_column(name, ColumnType::Boolean, true)),
            )
            .collect()
    }

    fn is_residual_go_native_shape(shape: &NativeManagerShape) -> bool {
        !matches!(
            shape,
            NativeManagerShape::RequirementsOnly
                | NativeManagerShape::AbilityData(_)
                | NativeManagerShape::OneTableCrcIndex(_)
                | NativeManagerShape::TableFamilyCrcIndex(_)
                | NativeManagerShape::OneTableOwnedStringCrcIndex(_)
                | NativeManagerShape::TableFamilyOwnedStringCrcIndex(_)
                | NativeManagerShape::OneTableCrcKeyProjection(_)
                | NativeManagerShape::MultiTableCrcKeyProjection(_)
                | NativeManagerShape::TableFamilyCrcKeyProjection(_)
                | NativeManagerShape::TableFamilyFallbackCrcKeyProjection(_)
                | NativeManagerShape::TableFamilyPartitionedCrcKeyProjection(_)
                | NativeManagerShape::OneTableNumericKeyProjection(_)
                | NativeManagerShape::TableFamilyNumericKeyProjection(_)
                | NativeManagerShape::OneTableEnumKeyProjection(_)
                | NativeManagerShape::OneTableStringKeyProjection(_)
                | NativeManagerShape::OneTableRowProjection(_)
                | NativeManagerShape::OneTableExperience(_)
                | NativeManagerShape::ItemData(_)
                | NativeManagerShape::ItemConversionData(_)
                | NativeManagerShape::DamageData(_)
                | NativeManagerShape::VitalsData(_)
                | NativeManagerShape::StatusEffectData(_)
                | NativeManagerShape::CurrencyExchangeMapping(_)
                | NativeManagerShape::TradeskillRankData(_)
                | NativeManagerShape::StaticTradeskillRankDataMapping(_)
                | NativeManagerShape::VitalsModifierMapping(_)
                | NativeManagerShape::ReplicationData(_)
                | NativeManagerShape::ProductAssetResource(_)
                | NativeManagerShape::ComposedResource(_)
        )
    }

    fn is_generic_semantic_projection_shape(shape: &NativeManagerShape) -> bool {
        matches!(
            shape,
            NativeManagerShape::OneTableCrcIndex(_)
                | NativeManagerShape::TableFamilyCrcIndex(_)
                | NativeManagerShape::OneTableOwnedStringCrcIndex(_)
                | NativeManagerShape::TableFamilyOwnedStringCrcIndex(_)
                | NativeManagerShape::OneTableCrcKeyProjection(_)
                | NativeManagerShape::MultiTableCrcKeyProjection(_)
                | NativeManagerShape::TableFamilyCrcKeyProjection(_)
                | NativeManagerShape::TableFamilyFallbackCrcKeyProjection(_)
                | NativeManagerShape::TableFamilyPartitionedCrcKeyProjection(_)
                | NativeManagerShape::OneTableNumericKeyProjection(_)
                | NativeManagerShape::TableFamilyNumericKeyProjection(_)
                | NativeManagerShape::OneTableEnumKeyProjection(_)
                | NativeManagerShape::OneTableStringKeyProjection(_)
                | NativeManagerShape::OneTableRowProjection(_)
        )
    }

    fn residual_native_contract_marker(shape: &NativeManagerShape) -> &'static str {
        match shape {
            NativeManagerShape::ObjectivesData(_) => "ObjectiveTaskDataFromID",
            NativeManagerShape::ContributionData(_) => "ContributionDataByKey",
            NativeManagerShape::BuffBucketData(_) => "VisitAllBuffsFromID",
            NativeManagerShape::StructureData(_) => "StructurePieceDataFromID",
            NativeManagerShape::ReusableScoreboardData(_) => "ReusableScoreboardDataFromID",
            NativeManagerShape::MountHitVolumeData(_) => "MountHitVolumeFromMountTypeID",
            NativeManagerShape::OneTableCampSkin(_) => "CampSkinDataFromID",
            NativeManagerShape::OneTableEmote(_) => "EmoteDataFromID",
            NativeManagerShape::OneTableStoreCategory(_) => "StoreCategoryPropertiesFromID",
            NativeManagerShape::OneTableStoreProduct(_) => "StoreProductDataFromID",
            NativeManagerShape::OneTableRewardTrackItem(_) => "RewardTrackItemFromID",
            NativeManagerShape::OneTableWorldEventRule(_) => "WorldEventRuleByCRC32",
            NativeManagerShape::QuickCourseData(_) => "NodeTypeByCRC32",
            NativeManagerShape::RotationalQueueData(_) => "RotationalQueueFromID",
            NativeManagerShape::DynamicDifficultyData(_) => "DynamicDifficultyStatusEffectPotency",
            NativeManagerShape::ProgressionPointData(_) => "ProgressionPointFromID",
            NativeManagerShape::EntitlementData(_) => "EntitlementsForReward",
            NativeManagerShape::EquipmentSetData(_) => "SetsForPerk",
            NativeManagerShape::OneTablePvpBalance(_) => "Balances",
            NativeManagerShape::OneTableDyeColor(_) => "DyeColorDataFromIndex",
            NativeManagerShape::RewardTrackData(_) => "RewardTrackSlot",
            NativeManagerShape::PostSkillCapProgression(_) => "PostSkillCapProgressionDataFromID",
            NativeManagerShape::WhisperData(_) => "WhisperVfxFromID",
            NativeManagerShape::OneTableCostumeChange(_) => "CostumeChangeDataFromID",
            NativeManagerShape::OneTableCrestPart(_) => "CrestPartDataFromIndex",
            NativeManagerShape::OneTableDungeonTile(_) => "DungeonTileStaticDataByKey",
            NativeManagerShape::OneTableLevelDisparity(_) => {
                "ClampedLevelDisparityDataForLevelsWithPlayerLevelCap"
            }
            NativeManagerShape::OneTableEncumbrance(_) => "EncumbranceDataFromID",
            NativeManagerShape::OneTableDifficultyScaling(_) => "DifficultyScalingDataFromID",
            NativeManagerShape::OneTableDarkness(_) => "DarknessDataByCRC32",
            NativeManagerShape::OneTableParticleData(_) => "ParticleDataFromID",
            NativeManagerShape::CharacterAttributeData(_) => "ClampedAttributeData",
            NativeManagerShape::GovernanceData(_) => "GovernanceRows",
            NativeManagerShape::LootBucketData(_) => "LootBucketSlot",
            NativeManagerShape::TerritoryDefinitionsData(_) => "TerritoryForAchievement",
            NativeManagerShape::StatModifierData(_) => "FromID",
            NativeManagerShape::SeasonsRewardsData(_) => "RewardsByType",
            NativeManagerShape::SeasonsTrackedStatData(_) => "TrackedStatFromID",
            NativeManagerShape::SeasonsRewardsActivitiesTasksData(_) => "ActivityTaskByKey",
            NativeManagerShape::SeasonsRewardsBattlePassData(_) => "RankBySeasonKey",
            NativeManagerShape::SeasonsRewardsCardTemplateData(_) => "CardTemplateByKey",
            NativeManagerShape::SeasonsRewardsChapterData(_) => "ChapterByKindIndex",
            NativeManagerShape::SeasonsRewardsJourneyData(_) => "JourneysForChapter",
            NativeManagerShape::SongBookSheetData(_) => "SheetIDsForPage",
            NativeManagerShape::SongBookData(_) => "SheetIDsForInstrument",
            NativeManagerShape::ElementalMutationStaticData(_) => "PossibleElementalStatusEffects",
            NativeManagerShape::PromotionMutationStaticData(_) => {
                "PossiblePromotionalStatusEffectsForElement"
            }
            NativeManagerShape::MusicalRewardsData(_) => "RewardForGameEvent",
            NativeManagerShape::CombatProfilesData(_) => "ActiveAbilityProfileByKey",
            NativeManagerShape::ItemTransformData(_) => "TransformByKey",
            NativeManagerShape::GatherableData(_) => "GatheringActionByKey",
            NativeManagerShape::SocialData(_) => "RankBySecurityLevel",
            NativeManagerShape::PlayerData(_) => "HasPlayerBaseAttributes",
            NativeManagerShape::RecipeData(_) => "CraftingRecipeDataByResult",
            _ => panic!("pre-lowered shape reached residual marker test: {shape:?}"),
        }
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
                package_name: "types".to_owned(),
                include_support_aliases: false,
                use_support_aliases: true,
                idiomatic_initialisms: true,
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
            ManagerSurface::Native {
                manager,
                shape,
                dependencies,
                semantic_projections,
            } => push_native_manager_type(
                source,
                unit,
                manager,
                shape,
                dependencies,
                semantic_projections,
            ),
            ManagerSurface::Semantic(record) => push_semantic_manager_type(source, record),
            ManagerSurface::ItemData(manager) => push_item_data_manager_type(source, manager),
            ManagerSurface::Composition(manager) => push_composition_manager_type(source, manager),
            ManagerSurface::ProductBacked(manager) => {
                push_product_backed_manager_type(source, manager)
            }
        }
    }
    source.push_str(SEMANTIC_MANAGER_RUNTIME_GO);
}

fn push_composition_manager_type(source: &mut String, manager: &CompositionManagerSurface) {
    source.push_str(match manager.kind {
        CompositionManagerKind::CurrencyExchangeMapping => CURRENCY_EXCHANGE_MAPPING_MANAGER_GO,
        CompositionManagerKind::ReplicationData => REPLICATION_DATA_MANAGER_GO,
        CompositionManagerKind::StaticTradeskillRankDataMapping => {
            STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_GO
        }
        CompositionManagerKind::VitalsModifierMapping => VITALS_MODIFIER_MAPPING_MANAGER_GO,
    });
}

const CURRENCY_EXCHANGE_MAPPING_MANAGER_GO: &str = r#"
type CurrencyExchangeEndpointKind uint8

const (
	CurrencyExchangeEndpointCurrency CurrencyExchangeEndpointKind = iota
	CurrencyExchangeEndpointCategoricalProgression
)

type CurrencyExchangeEndpoint struct {
	Kind CurrencyExchangeEndpointKind
	ID   gametypes.CRC32
}

func CurrencyEndpoint() CurrencyExchangeEndpoint {
	return CurrencyExchangeEndpoint{Kind: CurrencyExchangeEndpointCurrency}
}

func CategoricalProgressionEndpoint(id gametypes.CRC32) CurrencyExchangeEndpoint {
	return CurrencyExchangeEndpoint{Kind: CurrencyExchangeEndpointCategoricalProgression, ID: id}
}

type CurrencyExchangeMapping struct {
	Source   CurrencyExchangeEndpoint
	Target   CurrencyExchangeEndpoint
	Exchange gametypes.CurrencyExchangeData
}

type currencyExchangeEndpointPair struct {
	source CurrencyExchangeEndpoint
	target CurrencyExchangeEndpoint
}

type CurrencyExchangeMappingManager struct {
	mappings           []CurrencyExchangeMapping
	mappingsByEndpoint map[currencyExchangeEndpointPair]int
}

func newCurrencyExchangeMappingManager(
	exchanges *CurrencyExchangeDataManager,
	progressions *CategoricalProgressionDataManager,
) (*CurrencyExchangeMappingManager, error) {
	manager := &CurrencyExchangeMappingManager{mappingsByEndpoint: map[currencyExchangeEndpointPair]int{}}
	for exchange := range exchanges.Rows() {
		source, ok := currencyExchangeEndpoint(exchange.FromCurrencyCRC, exchange.FromCurrencyIsCategoricalProgression, progressions)
		if !ok { continue }
		target, ok := currencyExchangeEndpoint(exchange.ToCurrencyCRC, exchange.ToCurrencyIsCategoricalProgression, progressions)
		if !ok { continue }
		if source.Kind == CurrencyExchangeEndpointCategoricalProgression && target.Kind == CurrencyExchangeEndpointCategoricalProgression && source.ID == target.ID { continue }
		key := currencyExchangeEndpointPair{source: source, target: target}
		if _, exists := manager.mappingsByEndpoint[key]; exists { continue }
		manager.mappingsByEndpoint[key] = len(manager.mappings)
		manager.mappings = append(manager.mappings, CurrencyExchangeMapping{Source: source, Target: target, Exchange: exchange})
	}
	return manager, nil
}

func (manager *CurrencyExchangeMappingManager) Mapping(source, target CurrencyExchangeEndpoint) *CurrencyExchangeMapping {
	index, ok := manager.mappingsByEndpoint[currencyExchangeEndpointPair{source: source, target: target}]
	if !ok { return nil }
	return rowCopy(manager.mappings[index])
}

func (manager *CurrencyExchangeMappingManager) CurrencyExchange(source, target CurrencyExchangeEndpoint) *gametypes.CurrencyExchangeData {
	mapping := manager.Mapping(source, target)
	if mapping == nil { return nil }
	return rowCopy(mapping.Exchange)
}

func (manager *CurrencyExchangeMappingManager) ConversionID(source, target CurrencyExchangeEndpoint) (gametypes.CRC32, bool) {
	exchange := manager.CurrencyExchange(source, target)
	if exchange == nil { return 0, false }
	return exchange.ConversionCRC, true
}

func (manager *CurrencyExchangeMappingManager) Mappings() iter.Seq[CurrencyExchangeMapping] { return rowValues(manager.mappings) }
func (manager *CurrencyExchangeMappingManager) Len() int { return len(manager.mappings) }
func (manager *CurrencyExchangeMappingManager) IsEmpty() bool { return len(manager.mappings) == 0 }

func currencyExchangeEndpoint(id gametypes.CRC32, categorical bool, progressions *CategoricalProgressionDataManager) (CurrencyExchangeEndpoint, bool) {
	if !categorical { return CurrencyEndpoint(), true }
	progression := progressions.CategoricalProgressionDataFromID(id)
	if progression == nil { return CurrencyExchangeEndpoint{}, false }
	return CategoricalProgressionEndpoint(progression.CategoricalProgressionIDCRC), true
}
"#;

const REPLICATION_DATA_MANAGER_GO: &str = r#"
type ReplicationDataManager struct {
	ids      []gametypes.CRC32
	indexByID map[gametypes.CRC32]uint16
}

func newReplicationDataManager(perks *PerkDataManager) (*ReplicationDataManager, error) {
	ids := []gametypes.CRC32{0}
	for id := range perks.PerkIds() { ids = append(ids, id) }
	indexByID := make(map[gametypes.CRC32]uint16, len(ids))
	for index, id := range ids {
		if index > 0xffff { break }
		if _, exists := indexByID[id]; !exists { indexByID[id] = uint16(index) }
	}
	return &ReplicationDataManager{ids: ids, indexByID: indexByID}, nil
}

func (manager *ReplicationDataManager) IDAt(index uint16) gametypes.CRC32 {
	if int(index) >= len(manager.ids) { return 0 }
	return manager.ids[index]
}

func (manager *ReplicationDataManager) IndexOf(id gametypes.CRC32) uint16 {
	return manager.indexByID[id]
}

func (manager *ReplicationDataManager) IDs() iter.Seq[gametypes.CRC32] {
	return func(yield func(gametypes.CRC32) bool) {
		for _, id := range manager.ids { if !yield(id) { return } }
	}
}
func (manager *ReplicationDataManager) Len() int { return len(manager.ids) }
func (manager *ReplicationDataManager) IsEmpty() bool { return len(manager.ids) == 0 }
"#;

const VITALS_MODIFIER_MAPPING_MANAGER_GO: &str = r#"
type VitalsModifierMapping struct {
	Key string
	ID  gametypes.CRC32
}

type VitalsModifierMappingManager struct {
	entries     []VitalsModifierMapping
	entriesByID map[gametypes.CRC32]int
}

func newVitalsModifierMappingManager(vitals *VitalsDataManager, damage *DamageDataManager, items *ItemDataManager) (*VitalsModifierMappingManager, error) {
	manager := &VitalsModifierMappingManager{entriesByID: map[gametypes.CRC32]int{}}
	for entry := range vitals.Rows() { manager.insertLowercase(entry.Key) }
	for entry := range damage.DamageTypes() { manager.insertLowercase(entry.Key) }
	for entry := range damage.Rows() {
		manager.insertLowercase(normalizeWeaponCategory(entry.WeaponCategory))
	}
	manager.insertLowercase("Physical")
	manager.insertLowercase("Elemental")
	for item := range items.Rows() { manager.insertItemAliases(item.ItemID, item.ItemIDCRC) }
	return manager, nil
}

func (manager *VitalsModifierMappingManager) Get(id gametypes.CRC32) *VitalsModifierMapping {
	index, ok := manager.entriesByID[id]
	if !ok { return nil }
	return rowCopy(manager.entries[index])
}

func (manager *VitalsModifierMappingManager) ByKey(key string) *VitalsModifierMapping {
	return manager.Get(gametypes.CRC32(crc32Lowercase(key)))
}

func (manager *VitalsModifierMappingManager) Rows() iter.Seq[VitalsModifierMapping] { return rowValues(manager.entries) }
func (manager *VitalsModifierMappingManager) Len() int { return len(manager.entries) }
func (manager *VitalsModifierMappingManager) IsEmpty() bool { return len(manager.entries) == 0 }

func (manager *VitalsModifierMappingManager) insertLowercase(raw string) {
	key := strings.TrimSpace(raw)
	if key != "" { manager.insertWithID(key, gametypes.CRC32(crc32Lowercase(key))) }
}

func (manager *VitalsModifierMappingManager) insertItemAliases(raw string, id gametypes.CRC32) {
	key := strings.TrimSpace(raw)
	if key == "" || id == 0 { return }
	index := manager.insertWithID(key, id)
	lowercaseID := gametypes.CRC32(crc32Lowercase(key))
	if lowercaseID != 0 {
		if _, exists := manager.entriesByID[lowercaseID]; !exists { manager.entriesByID[lowercaseID] = index }
	}
}

func (manager *VitalsModifierMappingManager) insertWithID(key string, id gametypes.CRC32) int {
	if id == 0 { return 0 }
	if index, exists := manager.entriesByID[id]; exists { return index }
	index := len(manager.entries)
	manager.entriesByID[id] = index
	manager.entries = append(manager.entries, VitalsModifierMapping{Key: key, ID: id})
	return index
}

func normalizeWeaponCategory(value string) string {
	normalized := strings.TrimSpace(value)
	if normalized == "" || strings.EqualFold(normalized, "none") { return "Default" }
	return normalized
}
"#;

const STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_GO: &str = r#"
type StaticTradeskillRankDataMapping struct {
	CategoricalProgressionID gametypes.CRC32
	Table                    TradeskillRankDataTable
	Rank                     TradeskillRank
}

type PlayerLevelDisplayNameMapping struct {
	DisplayNameID gametypes.CRC32
	Rank          TradeskillRank
}

type StaticTradeskillRankDataMappingManager struct {
	playerLevels          []PlayerLevelDisplayNameMapping
	playerLevelsByName    map[gametypes.CRC32]TradeskillRank
	tradeskillRanks       []StaticTradeskillRankDataMapping
	tradeskillRanksByName map[gametypes.CRC32]int
}

func newStaticTradeskillRankDataMappingManager(
	experience *ExperienceDataManager,
	player *PlayerDataManager,
	progressions *CategoricalProgressionDataManager,
	ranks *TradeskillRankDataManager,
) (*StaticTradeskillRankDataMappingManager, error) {
	manager := &StaticTradeskillRankDataMappingManager{
		playerLevelsByName:    make(map[gametypes.CRC32]TradeskillRank),
		tradeskillRanksByName: make(map[gametypes.CRC32]int),
	}
	maxPlayerLevel := float32(0)
	for entry := range experience.Rows() {
		if entry.Row.LevelNumber > maxPlayerLevel { maxPlayerLevel = entry.Row.LevelNumber }
	}
	manager.cachePlayerLevels(maxPlayerLevel, ranks)
	if err := manager.cacheTradeskillRanks(player, progressions, ranks); err != nil {
		return nil, err
	}
	return manager, nil
}

func (manager *StaticTradeskillRankDataMappingManager) PlayerLevelForDisplayName(displayName gametypes.CRC32) (TradeskillRank, bool) {
	rank, ok := manager.playerLevelsByName[displayName]
	return rank, ok
}

func (manager *StaticTradeskillRankDataMappingManager) TradeskillRankForDisplayName(displayName gametypes.CRC32) *StaticTradeskillRankDataMapping {
	index, ok := manager.tradeskillRanksByName[displayName]
	if !ok { return nil }
	return rowCopy(manager.tradeskillRanks[index])
}

func (manager *StaticTradeskillRankDataMappingManager) PlayerLevels() iter.Seq[PlayerLevelDisplayNameMapping] {
	return func(yield func(PlayerLevelDisplayNameMapping) bool) {
		for _, mapping := range manager.playerLevels { if !yield(mapping) { return } }
	}
}

func (manager *StaticTradeskillRankDataMappingManager) TradeskillRanks() iter.Seq[StaticTradeskillRankDataMapping] {
	return rowValues(manager.tradeskillRanks)
}

func (manager *StaticTradeskillRankDataMappingManager) Len() int {
	return len(manager.playerLevels) + len(manager.tradeskillRanks)
}

func (manager *StaticTradeskillRankDataMappingManager) IsEmpty() bool { return manager.Len() == 0 }

func (manager *StaticTradeskillRankDataMappingManager) cachePlayerLevels(maxPlayerLevel float32, ranks *TradeskillRankDataManager) {
	// The source XP-level projection currently has no display-name field. Preserve
	// that behavior: only cache rows when a future merged schema supplies one.
	for entry := range ranks.ExperienceDataRows().Rows() {
		if entry.Row.LevelNumber < 0 || entry.Row.LevelNumber > maxPlayerLevel { continue }
		if entry.Row.BlueprintID == nil || strings.TrimSpace(*entry.Row.BlueprintID) == "" { continue }
		displayNameID := gametypes.CRC32(crc32Lowercase(*entry.Row.BlueprintID))
		if displayNameID == 0 { continue }
		rank, ok := tradeskillRankFromFloat(entry.Row.LevelNumber)
		if !ok { continue }
		if _, exists := manager.playerLevelsByName[displayNameID]; exists { continue }
		manager.playerLevelsByName[displayNameID] = rank
		manager.playerLevels = append(manager.playerLevels, PlayerLevelDisplayNameMapping{DisplayNameID: displayNameID, Rank: rank})
	}
}

func (manager *StaticTradeskillRankDataMappingManager) cacheTradeskillRanks(
	player *PlayerDataManager,
	progressions *CategoricalProgressionDataManager,
	ranks *TradeskillRankDataManager,
) error {
	for _, tradeskill := range allTradeskillTypes {
		progressionID, err := player.CategoricalProgressionID(tradeskill)
		if err != nil { return err }
		if progressionID == nil { continue }
		progression := progressions.CategoricalProgressionDataFromID(*progressionID)
		if progression == nil || progression.RankTableID == nil { continue }
		table := TradeskillRankDataTable(strings.TrimSpace(*progression.RankTableID))
		for entry := range ranks.Table(table).Rows() {
			rank, ok := tradeskillRankFromFloat(entry.Row.Level)
			if !ok || uint32(rank) > progression.MaxLevel || entry.Row.DisplayName == nil { continue }
			displayNameID := gametypes.CRC32(crc32Lowercase(strings.TrimSpace(*entry.Row.DisplayName)))
			if displayNameID == 0 { continue }
			if _, exists := manager.tradeskillRanksByName[displayNameID]; exists { continue }
			index := len(manager.tradeskillRanks)
			manager.tradeskillRanksByName[displayNameID] = index
			manager.tradeskillRanks = append(manager.tradeskillRanks, StaticTradeskillRankDataMapping{
				CategoricalProgressionID: *progressionID,
				Table: table,
				Rank: rank,
			})
		}
	}
	return nil
}

func tradeskillRankFromFloat(raw float32) (TradeskillRank, bool) {
	if raw < 0 || raw > 65535 || raw != float32(uint16(raw)) { return 0, false }
	return TradeskillRank(uint16(raw)), true
}

var allTradeskillTypes = [...]TradeskillType{
	TradeskillWeaponsmithing, TradeskillArmoring, TradeskillJewelcrafting, TradeskillArcana,
	TradeskillCooking, TradeskillFurnishing, TradeskillEngineering, TradeskillSmelting,
	TradeskillWoodworking, TradeskillLeatherworking, TradeskillWeaving, TradeskillStonecutting,
	TradeskillSkinning, TradeskillMining, TradeskillLogging, TradeskillHarvesting,
	TradeskillFishing, TradeskillAzothStaff, TradeskillMusician, TradeskillRiding,
}
"#;

fn push_direct_manager_type(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) {
    push_direct_manager_type_with_dependencies(source, unit, manager, &[]);
}

fn push_direct_manager_type_with_dependencies(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    dependencies: &[String],
) {
    push_manager_type_with_dependencies(
        source,
        unit,
        manager,
        dependencies,
        &GoNativeManagerAugmentation::default(),
    );
}

#[derive(Debug, Default)]
pub(super) struct GoNativeManagerAugmentation {
    pub(super) declarations: String,
    pub(super) fields: String,
    pub(super) field_values: String,
    pub(super) initializers: String,
    pub(super) methods: String,
}

fn go_receiver_method_count(source: &str, receiver_type: &str, method_name: &str) -> usize {
    let parse_source = format!("package managers\n\n{source}");
    let mut parser = treesitter_types_go::tree_sitter::Parser::new();
    parser
        .set_language(&treesitter_types_go::tree_sitter_go::LANGUAGE.into())
        .expect("configure the bundled Go grammar");
    let Some(tree) = parser.parse(parse_source.as_bytes(), None) else {
        return 0;
    };
    if tree.root_node().has_error() {
        return 0;
    }

    let mut count = 0;
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
        if node.kind() != "method_declaration" {
            continue;
        }
        let Some(name) = node.child_by_field_name("name") else {
            continue;
        };
        let Some(receiver) = node.child_by_field_name("receiver") else {
            continue;
        };
        let Ok(name) = name.utf8_text(parse_source.as_bytes()) else {
            continue;
        };
        let Ok(receiver) = receiver.utf8_text(parse_source.as_bytes()) else {
            continue;
        };
        if name == method_name
            && receiver
                .split(|character: char| !character.is_alphanumeric())
                .any(|part| part == receiver_type)
        {
            count += 1;
        }
    }
    count
}

fn push_manager_type_with_dependencies(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    dependencies: &[String],
    augmentation: &GoNativeManagerAugmentation,
) {
    let manager_type = go_method_name(&manager.manager_class_name);
    let constructor = go_manager_constructor_name(&manager_type);
    let manager_resources = go_direct_manager_resources_expression(manager);
    let mut product_methods = direct_go_product_methods(manager);
    product_methods.push_str(&special_go_manager_extra_methods(
        &manager.manager_class_name,
    ));
    let semantic_rows = go_receiver_method_count(&augmentation.methods, &manager_type, "Rows") == 1;
    let row_methods = direct_go_schema_methods(unit, manager, !semantic_rows);
    let row_specs = go_direct_row_specs(unit, manager);
    let default_row_type = go_direct_default_row_spec(unit, manager).map(|row| row.source_row_type);
    let row_fields = row_specs
        .iter()
        .map(|row| {
            let table_type = go_direct_table_type_name(
                manager,
                &row.source_row_type,
                default_row_type.as_deref() == Some(row.source_row_type.as_str()),
            );
            format!(
                "\t{} RowSet[{}, {}]\n",
                go_direct_row_field_name(&row.source_row_type),
                table_type,
                row.type_name
            )
        })
        .collect::<String>();
    let row_initializers = row_specs
        .iter()
        .map(|row| {
            let field = go_direct_row_field_name(&row.source_row_type);
            let reader = go_schema_reader_name(&row.source_row_type);
            let table_type = go_direct_table_type_name(
                manager,
                &row.source_row_type,
                default_row_type.as_deref() == Some(row.source_row_type.as_str()),
            );
            let resolver = go_direct_table_resolver_name(&table_type);
            format!(
                "\t{field}Entries, err := schemaFamilyEntries(resources, {:?}, {reader}, {resolver})\n\tif err != nil {{\n\t\treturn nil, err\n\t}}\n\t{field} := newRowSet({field}Entries)\n",
                row.source_row_type
            )
        })
        .collect::<String>();
    let row_field_values = row_specs
        .iter()
        .map(|row| {
            let field = go_direct_row_field_name(&row.source_row_type);
            format!("\t\t{field}: {field},\n")
        })
        .collect::<String>();
    let (product_fields, product_initializers, product_field_values) = go_product_storage(manager);
    let dependency_parameters = dependencies
        .iter()
        .map(|dependency| {
            format!(
                ", _{} *{}",
                go_local_name(&go_manager_dependency_name(dependency)),
                go_method_name(dependency)
            )
        })
        .collect::<String>();
    source.push_str(&augmentation.declarations);
    source.push_str(&format!(
        r#"
type {manager_type} struct {{
{row_fields}{augmentation_fields}
{product_fields}
}}

func {constructor}(cache *managerCache{dependency_parameters}) (*{manager_type}, error) {{
	resources, err := {manager_resources}
	if err != nil {{
		return nil, err
	}}
{row_initializers}
{product_initializers}
	manager := &{manager_type}{{
{row_field_values}{product_field_values}{augmentation_field_values}	}}
{augmentation_initializers}	return manager, nil
}}

{row_methods}
{product_methods}{augmentation_methods}
"#,
        augmentation_fields = augmentation.fields,
        augmentation_field_values = augmentation.field_values,
        augmentation_initializers = augmentation.initializers,
        augmentation_methods = augmentation.methods,
    ));
}

fn push_native_manager_type(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
    dependencies: &[String],
    semantic_projections: &[SemanticManagerRecord],
) {
    debug_assert!(shape.exposes_native_api());
    let effective_manager = go_effective_native_manager_surface(manager, shape);
    let augmentation =
        go_native_manager_augmentation(unit, &effective_manager, shape, semantic_projections);
    push_manager_type_with_dependencies(
        source,
        unit,
        &effective_manager,
        dependencies,
        &augmentation,
    );
}

fn go_effective_native_manager_surface(
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> DirectManagerSurface {
    let mut effective = manager.clone();
    if let NativeManagerShape::RecipeData(shape) = shape {
        for table in shape.tables() {
            let candidate = DirectManagerTable {
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

fn go_native_manager_augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
    semantic_projections: &[SemanticManagerRecord],
) -> GoNativeManagerAugmentation {
    if !semantic_projections.is_empty() {
        return go_semantic_projection_augmentation(manager, semantic_projections);
    }
    match shape {
        NativeManagerShape::OneTableExperience(_) => go_experience_manager_augmentation(),
        NativeManagerShape::TradeskillRankData(_) => {
            go_tradeskill_rank_manager_augmentation(unit, manager)
        }
        NativeManagerShape::DamageData(_) => go_damage_manager_augmentation(unit, manager),
        NativeManagerShape::AbilityData(_)
        | NativeManagerShape::VitalsData(_)
        | NativeManagerShape::StatusEffectData(_)
        | NativeManagerShape::ItemConversionData(_)
        | NativeManagerShape::ItemTransformData(_) => {
            native::residual_native_manager_augmentation(unit, manager, shape)
        }
        shape => native::residual_native_manager_augmentation(unit, manager, shape),
    }
}

fn go_semantic_projection_augmentation(
    manager: &DirectManagerSurface,
    records: &[SemanticManagerRecord],
) -> GoNativeManagerAugmentation {
    let manager_type = go_method_name(&manager.manager_class_name);
    let mut augmentation = GoNativeManagerAugmentation::default();

    for (projection_index, record) in records.iter().enumerate() {
        debug_assert_eq!(record.manager_name, manager.manager_name);
        let record_type = go_method_name(&record.record_type_name);
        let stem = go_local_name(&format!("{} projection", record.record_type_name));
        let entries = format!("{stem}Entries");
        let by_key = format!("{stem}ByKey");
        let by_source_row = format!("{stem}BySourceRow");
        let rows_variable = format!("{stem}Rows");
        let materializer_name = format!("{record_type}Projection{projection_index}");

        let mut materializer_record = record.clone();
        materializer_record.manager_class_name = materializer_name.clone();
        push_go_semantic_materializer(&mut augmentation.declarations, &materializer_record);

        augmentation
            .fields
            .push_str(&format!("\t{entries} []{record_type}\n"));
        if !record.lookup_methods.is_empty() {
            augmentation
                .fields
                .push_str(&format!("\t{by_key} map[{}]int\n", go_key_map_type(record)));
        }
        if record.source_row_method.is_some() {
            augmentation
                .fields
                .push_str(&format!("\t{by_source_row} map[uint32]int\n"));
        }

        augmentation.initializers.push_str(&format!(
            "\t{rows_variable}, err := materialize{materializer_name}(resources)\n\tif err != nil {{ return nil, err }}\n\tmanager.{entries} = {rows_variable}\n"
        ));
        if !record.lookup_methods.is_empty() {
            augmentation.initializers.push_str(&format!(
                "\tmanager.{by_key} = make(map[{}]int)\n",
                go_key_map_type(record)
            ));
        }
        if record.source_row_method.is_some() {
            augmentation.initializers.push_str(&format!(
                "\tmanager.{by_source_row} = make(map[uint32]int)\n"
            ));
        }
        if !record.lookup_methods.is_empty() || record.source_row_method.is_some() {
            augmentation
                .initializers
                .push_str(&format!("\tfor index := range manager.{entries} {{\n"));
            if !record.lookup_methods.is_empty() {
                let expression = go_row_index_expression(record)
                    .expect("semantic projection lookup requires a key")
                    .replace("rows[index]", &format!("manager.{entries}[index]"));
                augmentation
                    .initializers
                    .push_str(&format!("\t\tmanager.{by_key}[{expression}] = index\n"));
            }
            if record.source_row_method.is_some() {
                let field = record
                    .source_row_field
                    .as_ref()
                    .expect("semantic projection source-row lookup requires a field");
                augmentation.initializers.push_str(&format!(
                    "\t\tmanager.{by_source_row}[manager.{entries}[index].{}] = index\n",
                    go_field_name(field)
                ));
            }
            augmentation.initializers.push_str("\t}\n");
        }

        push_go_projection_methods(
            &mut augmentation.methods,
            &manager_type,
            record,
            &entries,
            &by_key,
            &by_source_row,
            projection_index == 0,
        );
    }

    augmentation
}

fn push_go_projection_methods(
    source: &mut String,
    manager_type: &str,
    record: &SemanticManagerRecord,
    entries: &str,
    by_key: &str,
    by_source_row: &str,
    canonical_rows: bool,
) {
    let record_type = go_method_name(&record.record_type_name);
    for method in &record.lookup_methods {
        let method_name = go_method_name(&method.name);
        let parameter = go_local_name(&method.parameter);
        let (parameter_type, key) = match method.kind {
            SemanticLookupKind::CrcString => (
                "string".to_owned(),
                format!("CRC32(crc32Lowercase({parameter}))"),
            ),
            SemanticLookupKind::Crc | SemanticLookupKind::IntoCrc => {
                ("CRC32".to_owned(), parameter.clone())
            }
            SemanticLookupKind::Numeric(key_type) => (
                go_numeric_key_type(key_type).to_owned(),
                go_numeric_key_as_u32(&parameter, key_type),
            ),
            SemanticLookupKind::String => (
                "string".to_owned(),
                format!("normalizeStringKey({parameter})"),
            ),
        };
        source.push_str(&format!(
            "func (manager *{manager_type}) {method_name}({parameter} {parameter_type}) *{record_type} {{ index, ok := manager.{by_key}[{key}]; if !ok {{ return nil }}; return rowCopy(manager.{entries}[index]) }}\n\n"
        ));
    }
    if let Some(method) = &record.source_row_method {
        let method = go_method_name(method);
        source.push_str(&format!(
            "func (manager *{manager_type}) {method}(row uint32) *{record_type} {{ index, ok := manager.{by_source_row}[row]; if !ok {{ return nil }}; return rowCopy(manager.{entries}[index]) }}\n\n"
        ));
    }
    if let Some(method) = &record.ids_method {
        let method = go_method_name(method);
        let id_type = go_ids_type(record);
        let expression =
            go_ids_expression(record).replace("manager.entries", &format!("manager.{entries}"));
        source.push_str(&format!(
            "func (manager *{manager_type}) {method}() iter.Seq[{id_type}] {{ return func(yield func({id_type}) bool) {{ for index := range manager.{entries} {{ if !yield({expression}) {{ return }} }} }} }}\n\n"
        ));
    }
    if canonical_rows {
        source.push_str(&format!(
            "func (manager *{manager_type}) Rows() iter.Seq[{record_type}] {{ return rowValues(manager.{entries}) }}\n\n"
        ));
    }
    if let Some(method) = &record.rows_method {
        let method = go_method_name(method);
        if method != "Rows" {
            source.push_str(&format!(
                "func (manager *{manager_type}) {method}() iter.Seq[{record_type}] {{ return rowValues(manager.{entries}) }}\n\n"
            ));
        }
    }
    if let Some(method) = &record.len_method {
        let method = go_method_name(method);
        source.push_str(&format!(
            "func (manager *{manager_type}) {method}() int {{ return len(manager.{entries}) }}\n\n"
        ));
    }
    if let Some(method) = &record.is_empty_method {
        let method = go_method_name(method);
        source.push_str(&format!(
            "func (manager *{manager_type}) {method}() bool {{ return len(manager.{entries}) == 0 }}\n\n"
        ));
    }
}

fn go_table_path_cases(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    table_type: &str,
) -> String {
    manager
        .tables
        .iter()
        .filter(|table| table.row_type_name == row_type)
        .flat_map(|input| {
            unit.schema_report()
                .tables
                .iter()
                .filter(move |table| {
                    table.table_name == input.table_name
                        && table.row_type_name == input.row_type_name
                })
                .flat_map(move |table| {
                    table.sources.iter().map(move |source| {
                        format!(
                            "\tcase {:?}:\n\t\treturn {table_type}({:?}), true\n",
                            source.replace('\\', "/").to_ascii_lowercase(),
                            input.table_name
                        )
                    })
                })
        })
        .collect()
}

fn go_damage_manager_augmentation(
    _unit: &GameDataCompileUnit,
    _manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    GoNativeManagerAugmentation {
        declarations: r#"
type DamageDataRef struct { Table DamageDataTable; ID gametypes.CRC32 }
type DamageDataSlot struct { Table DamageDataTable; RowIndex int }

type StaticDamageData struct {
	Ref DamageDataRef
	Slot DamageDataSlot
	Key string
	ID gametypes.CRC32
	WeaponCategory string
	WeaponCategoryID gametypes.CRC32
	Source RowRef[DamageDataTable, DamageDataSchemaRow]
}

type StaticDamageTypeData struct {
	Key string
	ID gametypes.CRC32
	NumericID uint8
	Source RowRef[DamageDataDamageTypeDataTable, DamageTypeDataSchemaRow]
}

type StaticAfflictionData struct {
	Key string
	ID gametypes.CRC32
	NumericID uint8
	Source RowRef[DamageDataAfflictionDataTable, AfflictionDataSchemaRow]
}
"#
        .to_owned(),
        fields: r#"	damage []StaticDamageData
	damageByRef map[DamageDataRef]int
	damageBySlot map[DamageDataSlot]int
	damageTypes []StaticDamageTypeData
	damageTypesByID map[gametypes.CRC32]int
	afflictions []StaticAfflictionData
	afflictionsByID map[gametypes.CRC32]int
	weaponCategories []string
	weaponCategoriesByID map[gametypes.CRC32]struct{}
"#
        .to_owned(),
        field_values: r#"		damageByRef: make(map[DamageDataRef]int),
		damageBySlot: make(map[DamageDataSlot]int),
		damageTypesByID: make(map[gametypes.CRC32]int),
		afflictionsByID: make(map[gametypes.CRC32]int),
		weaponCategoriesByID: make(map[gametypes.CRC32]struct{}),
"#
        .to_owned(),
        initializers: r#"	for source := range manager.damageDataRows.Rows() {
		table := source.Ref.Table()
		key := strings.TrimSpace(source.Row.DamageID)
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 { continue }
		ref := DamageDataRef{Table: table, ID: id}
		slot := DamageDataSlot{Table: table, RowIndex: source.Slot.RowIndex()}
		if _, exists := manager.damageByRef[ref]; exists { continue }
		if _, exists := manager.damageBySlot[slot]; exists { continue }
		category := "Default"
		if source.Row.WeaponCategory != nil {
			candidate := strings.TrimSpace(*source.Row.WeaponCategory)
			if candidate != "" && !strings.EqualFold(candidate, "none") { category = candidate }
		}
		categoryID := gametypes.CRC32(crc32Lowercase(category))
		if _, exists := manager.weaponCategoriesByID[categoryID]; !exists && categoryID != 0 {
			manager.weaponCategoriesByID[categoryID] = struct{}{}
			manager.weaponCategories = append(manager.weaponCategories, category)
		}
		index := len(manager.damage)
		manager.damageByRef[ref] = index
		manager.damageBySlot[slot] = index
		manager.damage = append(manager.damage, StaticDamageData{Ref: ref, Slot: slot, Key: key, ID: id, WeaponCategory: category, WeaponCategoryID: categoryID, Source: source.Ref})
	}
	for source := range manager.damageTypeDataRows.Rows() {
		key := strings.TrimSpace(source.Row.TypeID)
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 || source.Row.IntID == nil { continue }
		raw, ok := exactUint32(*source.Row.IntID)
		if !ok || raw > 255 { continue }
		if _, exists := manager.damageTypesByID[id]; exists { continue }
		manager.damageTypesByID[id] = len(manager.damageTypes)
		manager.damageTypes = append(manager.damageTypes, StaticDamageTypeData{Key: key, ID: id, NumericID: uint8(raw), Source: source.Ref})
	}
	for source := range manager.afflictionDataRows.Rows() {
		key := strings.TrimSpace(source.Row.AfflictionID)
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 || source.Row.IntID == nil { continue }
		raw, ok := exactUint32(*source.Row.IntID)
		if !ok || raw >= 255 { continue }
		if _, exists := manager.afflictionsByID[id]; exists { continue }
		manager.afflictionsByID[id] = len(manager.afflictions)
		manager.afflictions = append(manager.afflictions, StaticAfflictionData{Key: key, ID: id, NumericID: uint8(raw), Source: source.Ref})
	}
"#
        .to_owned(),
        methods: r#"func (manager *DamageDataManager) Damage(ref DamageDataRef) *StaticDamageData {
	index, ok := manager.damageByRef[ref]
	if !ok { return nil }
	return rowCopy(manager.damage[index])
}

func (manager *DamageDataManager) DamageBySlot(slot DamageDataSlot) *StaticDamageData {
	index, ok := manager.damageBySlot[slot]
	if !ok { return nil }
	return rowCopy(manager.damage[index])
}

func (manager *DamageDataManager) DamageByID(table DamageDataTable, id gametypes.CRC32) *StaticDamageData {
	return manager.Damage(DamageDataRef{Table: table, ID: id})
}

func (manager *DamageDataManager) DamageByKey(table DamageDataTable, key string) *StaticDamageData {
	return manager.DamageByID(table, gametypes.CRC32(crc32Lowercase(key)))
}

func (manager *DamageDataManager) Resolve(ref TableReference) *StaticDamageData {
	table, ok := ParseDamageDataTable(ref.Path)
	if !ok { return nil }
	return manager.DamageByKey(table, ref.Key)
}

func (manager *DamageDataManager) DamageRefBySlot(slot DamageDataSlot) (DamageDataRef, bool) {
	data := manager.DamageBySlot(slot)
	if data == nil { return DamageDataRef{}, false }
	return data.Ref, true
}

func (manager *DamageDataManager) DamageKeyBySlot(slot DamageDataSlot) (string, bool) {
	data := manager.DamageBySlot(slot)
	if data == nil { return "", false }
	return data.Key, true
}

func (manager *DamageDataManager) DamageType(id gametypes.CRC32) *StaticDamageTypeData {
	index, ok := manager.damageTypesByID[id]
	if !ok { return nil }
	return rowCopy(manager.damageTypes[index])
}

func (manager *DamageDataManager) DamageTypeByKey(key string) *StaticDamageTypeData {
	return manager.DamageType(gametypes.CRC32(crc32Lowercase(key)))
}

func (manager *DamageDataManager) Affliction(id gametypes.CRC32) *StaticAfflictionData {
	index, ok := manager.afflictionsByID[id]
	if !ok { return nil }
	return rowCopy(manager.afflictions[index])
}

func (manager *DamageDataManager) AfflictionByKey(key string) *StaticAfflictionData {
	return manager.Affliction(gametypes.CRC32(crc32Lowercase(key)))
}

func (manager *DamageDataManager) Rows() iter.Seq[StaticDamageData] { return rowValues(manager.damage) }
func (manager *DamageDataManager) DamageTypes() iter.Seq[StaticDamageTypeData] { return rowValues(manager.damageTypes) }
func (manager *DamageDataManager) Afflictions() iter.Seq[StaticAfflictionData] { return rowValues(manager.afflictions) }
func (manager *DamageDataManager) WeaponCategories() iter.Seq[string] {
	return func(yield func(string) bool) { for _, category := range manager.weaponCategories { if !yield(category) { return } } }
}
func (manager *DamageDataManager) Len() int { return len(manager.damage) }
func (manager *DamageDataManager) IsEmpty() bool { return len(manager.damage) == 0 }

"#
        .to_owned(),
    }
}

fn go_experience_manager_augmentation() -> GoNativeManagerAugmentation {
    GoNativeManagerAugmentation {
        fields: r#"	experienceByLevel map[uint32]int
	gearScoreThresholds []experienceThreshold
	xpThresholds []experienceThreshold
	maxLevel uint32
	hasExperience bool
"#
        .to_owned(),
        field_values: "\t\texperienceByLevel: make(map[uint32]int),\n".to_owned(),
        declarations: r#"
type experienceThreshold struct { threshold uint32; level uint32 }
"#
        .to_owned(),
        initializers: r#"	for index := range manager.experienceDataRows.entries {
		row := rowCopy(manager.experienceDataRows.entries[index].Row)
		level, ok := exactUint32(row.LevelNumber)
		if !ok { continue }
		if _, exists := manager.experienceByLevel[level]; exists { continue }
		manager.experienceByLevel[level] = index
		if !manager.hasExperience || level > manager.maxLevel { manager.maxLevel = level }
		manager.hasExperience = true
		if row.MaxEquippableGearScore != nil {
			if threshold, ok := exactUint32(*row.MaxEquippableGearScore); ok && threshold != 0 {
				manager.gearScoreThresholds = append(manager.gearScoreThresholds, experienceThreshold{threshold: threshold, level: level})
			}
		}
		threshold := uint32(0)
		if row.XPToLevel != nil { threshold, _ = exactUint32(*row.XPToLevel) }
		manager.xpThresholds = append(manager.xpThresholds, experienceThreshold{threshold: threshold, level: level})
	}
	sort.Slice(manager.gearScoreThresholds, func(left, right int) bool { return manager.gearScoreThresholds[left].threshold < manager.gearScoreThresholds[right].threshold })
	sort.Slice(manager.xpThresholds, func(left, right int) bool { return manager.xpThresholds[left].threshold < manager.xpThresholds[right].threshold })
"#
        .to_owned(),
        methods: r#"func (manager *ExperienceDataManager) ExperienceDataFromID(level uint32) *ExperienceDataSchemaRow {
	index, ok := manager.experienceByLevel[level]
	if !ok { return nil }
	return rowCopy(manager.experienceDataRows.entries[index].Row)
}

func (manager *ExperienceDataManager) ExperienceData(level uint32) *ExperienceDataSchemaRow {
	return manager.ExperienceDataFromID(level)
}

func (manager *ExperienceDataManager) ExperienceDataForMaxEquippableGearScore(gearScore uint32) *ExperienceDataSchemaRow {
	index := sort.Search(len(manager.gearScoreThresholds), func(index int) bool { return gearScore <= manager.gearScoreThresholds[index].threshold })
	if index == len(manager.gearScoreThresholds) { return nil }
	return manager.ExperienceDataFromID(manager.gearScoreThresholds[index].level)
}

func (manager *ExperienceDataManager) LevelForXP(xp uint64) uint32 {
	level := uint32(0)
	for _, threshold := range manager.xpThresholds {
		if uint64(threshold.threshold) > xp { break }
		if threshold.level > level { level = threshold.level }
	}
	return level
}

func (manager *ExperienceDataManager) MaxLevel() (uint32, bool) { return manager.maxLevel, manager.hasExperience }
func (manager *ExperienceDataManager) Len() int { return len(manager.experienceByLevel) }
func (manager *ExperienceDataManager) IsEmpty() bool { return len(manager.experienceByLevel) == 0 }

"#
        .to_owned(),
    }
}

fn go_tradeskill_rank_manager_augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let table_cases = manager
        .tables
        .iter()
        .filter(|table| table.row_type_name == "TradeskillRankData")
        .flat_map(|input| {
            unit.schema_report()
                .tables
                .iter()
                .filter(move |table| {
                    table.table_name == input.table_name
                        && table.row_type_name == input.row_type_name
                })
                .flat_map(move |table| {
                    table.sources.iter().map(move |source| {
                        format!(
                            "\tcase {:?}:\n\t\treturn TradeskillRankDataTable({:?}), true\n",
                            source.replace('\\', "/").to_ascii_lowercase(),
                            input.table_name
                        )
                    })
                })
        })
        .collect::<String>();
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type TradeskillRank uint16

type PlayerLevelRankData struct {{
	Rank TradeskillRank
	Source RowRef[TradeskillRankDataExperienceDataTable, ExperienceDataSchemaRow]
}}

type StaticTradeskillRankData struct {{
	Table TradeskillRankDataTable
	Rank TradeskillRank
	DisplayName *string
	DisplayNameID gametypes.CRC32
	Source RowRef[TradeskillRankDataTable, TradeskillRankDataSchemaRow]
}}

type tradeskillRankKey struct {{ table TradeskillRankDataTable; rank TradeskillRank }}

func tradeskillRankTableFromPath(path string) (TradeskillRankDataTable, bool) {{
	switch normalizeDataPath(path) {{
{table_cases}	default:
		return "", false
	}}
}}
"#
        ),
        fields: r#"	playerLevelsByRank map[TradeskillRank]int
	playerLevels []PlayerLevelRankData
	tradeskillRanksByKey map[tradeskillRankKey]int
	tradeskillRanks []StaticTradeskillRankData
"#
        .to_owned(),
        field_values: r#"		playerLevelsByRank: make(map[TradeskillRank]int),
		tradeskillRanksByKey: make(map[tradeskillRankKey]int),
"#
        .to_owned(),
        initializers: r#"	for source := range manager.experienceDataRows.Rows() {
		raw, ok := exactUint32(source.Row.LevelNumber)
		if !ok || raw > 65535 { continue }
		rank := TradeskillRank(raw)
		if _, exists := manager.playerLevelsByRank[rank]; exists { continue }
		manager.playerLevelsByRank[rank] = len(manager.playerLevels)
		manager.playerLevels = append(manager.playerLevels, PlayerLevelRankData{Rank: rank, Source: source.Ref})
	}
	for source := range manager.tradeskillRankDataRows.Rows() {
		table := source.Ref.Table()
		raw, ok := exactUint32(source.Row.Level)
		if !ok || raw > 65535 { continue }
		rank := TradeskillRank(raw)
		key := tradeskillRankKey{table: table, rank: rank}
		if _, exists := manager.tradeskillRanksByKey[key]; exists { continue }
		var displayName *string
		if source.Row.DisplayName != nil {
			trimmed := strings.TrimSpace(*source.Row.DisplayName)
			if trimmed != "" { displayName = &trimmed }
		}
		displayNameID := gametypes.CRC32(0)
		if displayName != nil { displayNameID = gametypes.CRC32(crc32Lowercase(*displayName)) }
		manager.tradeskillRanksByKey[key] = len(manager.tradeskillRanks)
		manager.tradeskillRanks = append(manager.tradeskillRanks, StaticTradeskillRankData{Table: table, Rank: rank, DisplayName: displayName, DisplayNameID: displayNameID, Source: source.Ref})
	}
"#
        .to_owned(),
        methods: r#"func (manager *TradeskillRankDataManager) PlayerLevelRow(rank TradeskillRank) *PlayerLevelRankData {
	index, ok := manager.playerLevelsByRank[rank]
	if !ok { return nil }
	return rowCopy(manager.playerLevels[index])
}

func (manager *TradeskillRankDataManager) TradeskillRank(table TradeskillRankDataTable, rank TradeskillRank) *StaticTradeskillRankData {
	index, ok := manager.tradeskillRanksByKey[tradeskillRankKey{table: table, rank: rank}]
	if !ok { return nil }
	return rowCopy(manager.tradeskillRanks[index])
}

func (manager *TradeskillRankDataManager) PlayerLevels() iter.Seq[PlayerLevelRankData] { return rowValues(manager.playerLevels) }
func (manager *TradeskillRankDataManager) TradeskillRanks() iter.Seq[StaticTradeskillRankData] { return rowValues(manager.tradeskillRanks) }
func (manager *TradeskillRankDataManager) Len() int { return len(manager.playerLevels) + len(manager.tradeskillRanks) }
func (manager *TradeskillRankDataManager) IsEmpty() bool { return manager.Len() == 0 }

"#
        .to_owned(),
    }
}

fn push_direct_typed_table_api(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) {
    let row_specs = go_direct_row_specs(unit, manager);
    let Some(default_row) = go_direct_default_row_spec(unit, manager) else {
        return;
    };
    let manager_type = go_method_name(&manager.manager_class_name);

    for row_spec in &row_specs {
        let is_default = row_spec.source_row_type == default_row.source_row_type;
        let row_name = go_method_name(&row_spec.source_row_type);
        let table_type = go_direct_table_type_name(manager, &row_spec.source_row_type, is_default);
        let resolver = go_direct_table_resolver_name(&table_type);
        let method_prefix = if is_default { "" } else { row_name.as_str() };
        let table_rows_method = if is_default {
            "Table".to_owned()
        } else {
            format!("{method_prefix}Table")
        };
        let row_field = go_direct_row_field_name(&row_spec.source_row_type);
        let schema_row_type = &row_spec.type_name;
        let tables = manager
            .tables
            .iter()
            .filter(|table| table.row_type_name == row_spec.source_row_type)
            .collect::<Vec<_>>();
        if tables.is_empty() {
            continue;
        }

        source.push_str(&format!("type {table_type} string\n\nconst (\n"));
        for table in tables {
            let variant = go_method_name(&table.table_name);
            source.push_str(&format!(
                "\t{table_type}{variant} {table_type} = {:?}\n",
                table.table_name
            ));
        }
        let table_cases =
            go_table_path_cases(unit, manager, &row_spec.source_row_type, &table_type);
        source.push_str(&format!(
            r#")

func (table {table_type}) Name() string {{ return string(table) }}

func (table {table_type}) Ref(key string) RowRef[{table_type}, {schema_row_type}] {{
	return RowRef[{table_type}, {schema_row_type}]{{table: table, key: key}}
}}

func (table {table_type}) Slot(rowIndex int) RowSlot[{table_type}, {schema_row_type}] {{
	return RowSlot[{table_type}, {schema_row_type}]{{table: table, rowIndex: rowIndex}}
}}

func {resolver}(path string) ({table_type}, bool) {{
	switch normalizeDataPath(path) {{
{table_cases}	default:
		return "", false
	}}
}}

func (manager *{manager_type}) {table_rows_method}(table {table_type}) TableRows[{table_type}, {schema_row_type}] {{
	return manager.{row_field}.table(table)
}}

"#
        ));
    }
}

pub(super) fn go_direct_table_type_name(
    manager: &DirectManagerSurface,
    source_row_type: &str,
    is_default: bool,
) -> String {
    let manager_type = go_method_name(&manager.manager_class_name);
    let manager_stem = manager_type
        .strip_suffix("Manager")
        .unwrap_or(&manager_type);
    if is_default {
        format!("{manager_stem}Table")
    } else {
        format!("{manager_stem}{}Table", go_method_name(source_row_type))
    }
}

fn go_direct_table_resolver_name(table_type: &str) -> String {
    format!("Parse{}", go_method_name(table_type))
}

fn direct_go_schema_methods(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    include_primary_rows: bool,
) -> String {
    let default_row_type = go_direct_default_row_spec(unit, manager).map(|row| row.source_row_type);
    let mut source = String::new();
    push_direct_typed_table_api(&mut source, unit, manager);
    for row_spec in go_direct_row_specs(unit, manager) {
        let row_type = &row_spec.source_row_type;
        let is_default_row_type = default_row_type.as_deref() == Some(row_type.as_str());
        if is_default_row_type {
            source.push_str(&go_direct_primary_row_family_methods(
                manager,
                &row_spec,
                include_primary_rows,
            ));
        } else {
            let accessor = format!("{}Rows", go_method_name(row_type));
            let field = go_direct_row_field_name(row_type);
            let schema_row_type = &row_spec.type_name;
            let table_type = go_direct_table_type_name(manager, row_type, false);
            source.push_str(&format!(
                r#"func (manager *{manager_type}) {accessor}() RowSet[{table_type}, {schema_row_type}] {{
	return manager.{field}
}}

"#,
                manager_type = go_method_name(&manager.manager_class_name)
            ));
        }
    }
    source
}

pub(super) fn go_direct_row_specs(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> Vec<GoSchemaRow> {
    let row_specs = go_schema_rows(unit);
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

pub(super) fn go_direct_default_row_spec(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> Option<GoSchemaRow> {
    let row_specs = go_direct_row_specs(unit, manager);
    let row_types = row_specs
        .iter()
        .map(|row| row.source_row_type.clone())
        .collect::<Vec<_>>();
    let default_row_type = default_direct_manager_row_type(&manager.manager_name, &row_types);
    default_row_type
        .and_then(|default_row_type| {
            row_specs
                .iter()
                .find(|row| row.source_row_type == default_row_type)
                .cloned()
        })
        .or_else(|| row_specs.into_iter().next())
}

fn push_direct_row_family_types(
    _source: &mut String,
    _unit: &GameDataCompileUnit,
    _surfaces: &[ManagerSurface],
) {
}

fn go_direct_primary_row_family_methods(
    manager: &DirectManagerSurface,
    row_spec: &GoSchemaRow,
    include_rows: bool,
) -> String {
    let source_row_type = &row_spec.source_row_type;
    let row_type = &row_spec.type_name;
    let field = go_direct_row_field_name(source_row_type);
    let table_type = go_direct_table_type_name(manager, source_row_type, true);
    let resolver = go_direct_table_resolver_name(&table_type);
    let rows = if include_rows {
        format!(
            r#"func (manager *{manager_type}) Rows() iter.Seq[RowEntry[{table_type}, {row_type}]] {{
	return manager.{field}.Rows()
}}

"#,
            manager_type = go_method_name(&manager.manager_class_name)
        )
    } else {
        let accessor = format!("{}Rows", go_method_name(source_row_type));
        format!(
            r#"func (manager *{manager_type}) {accessor}() RowSet[{table_type}, {row_type}] {{
	return manager.{field}
}}

"#,
            manager_type = go_method_name(&manager.manager_class_name)
        )
    };
    format!(
        r#"{rows}func (manager *{manager_type}) Row(ref RowRef[{table_type}, {row_type}]) *{row_type} {{
	return manager.{field}.Get(ref)
}}

func (manager *{manager_type}) ResolveRow(ref TableReference) *{row_type} {{
	table, ok := {resolver}(ref.Path)
	if !ok {{
		return nil
	}}
	return manager.{field}.table(table).Get(ref.Key)
}}

func (manager *{manager_type}) RowByIndex(slot RowSlot[{table_type}, {row_type}]) *{row_type} {{
	return manager.{field}.RowByIndex(slot)
}}

func (manager *{manager_type}) RowKeyByIndex(slot RowSlot[{table_type}, {row_type}]) (string, bool) {{
	return manager.{field}.RowKeyByIndex(slot)
}}

"#,
        manager_type = go_method_name(&manager.manager_class_name)
    )
}

pub(super) fn go_direct_row_field_name(source_row_type: &str) -> String {
    go_local_name(&format!("{source_row_type}Rows"))
}

fn go_product_info(
    value_type: &str,
) -> Option<(NativeManagerProductKind, &'static str, &'static str)> {
    let kind = NativeManagerProductKind::from_canonical_type_path(value_type)?;
    let info = match kind {
        NativeManagerProductKind::ArmorOffsetDatabase => {
            ("ArmorOffsetDatabase", "parseArmorOffsetDatabase")
        }
        NativeManagerProductKind::EquipTypesDatabase => {
            ("EquipTypesDatabase", "parseEquipTypesDatabase")
        }
        NativeManagerProductKind::GameDebugSettings => {
            ("GameDebugSettings", "parseGameDebugSettings")
        }
        NativeManagerProductKind::PlayerBaseAttributes => {
            ("PlayerBaseAttributes", "parsePlayerBaseAttributes")
        }
        NativeManagerProductKind::SettlementProgressionData => (
            "SettlementProgressionData",
            "parseSettlementProgressionData",
        ),
        NativeManagerProductKind::UiDatabase => ("UIDatabase", "parseUIDatabase"),
        NativeManagerProductKind::GameCameraSettings => {
            ("GameCameraSettings", "parseGameCameraSettings")
        }
        NativeManagerProductKind::GatheringDatabase => {
            ("GatheringDatabase", "parseGatheringDatabase")
        }
        NativeManagerProductKind::GatheringActionDatabase => {
            ("GatheringActionDatabase", "parseGatheringActionDatabase")
        }
        NativeManagerProductKind::CraftingStationDatabase => {
            ("CraftingStationDatabase", "parseCraftingStationDatabase")
        }
        NativeManagerProductKind::SocialRankDatabase => {
            ("SocialRankDatabase", "parseSocialRankDatabase")
        }
    };
    Some((kind, info.0, info.1))
}

fn go_product_storage(manager: &DirectManagerSurface) -> (String, String, String) {
    let mut fields = String::new();
    let mut initializers = String::new();
    let mut field_values = String::new();
    let mut seen = BTreeSet::new();
    for product in &manager.products {
        let (_, type_name, parser) = go_product_info(&product.value_type).unwrap_or_else(|| {
            panic!(
                "Go product manager {} declares unsupported product type {} at {}",
                manager.manager_name, product.value_type, product.path
            )
        });
        let field = go_local_name(type_name);
        if !seen.insert(field.clone()) {
            continue;
        }
        fields.push_str(&format!("\t{field} *{type_name}\n"));
        initializers.push_str(&format!(
            "\t{field}Bytes, err := resources.requiredAssetBytes({})\n\tif err != nil {{\n\t\treturn nil, err\n\t}}\n\t{field}, err := {parser}({field}Bytes)\n\tif err != nil {{\n\t\treturn nil, err\n\t}}\n",
            go_string(&product.path)
        ));
        field_values.push_str(&format!("\t\t{field}: {field},\n"));
    }
    (fields, initializers, field_values)
}

fn direct_go_product_methods(manager: &DirectManagerSurface) -> String {
    let mut source = String::new();
    let manager_type = go_method_name(&manager.manager_class_name);
    for product in &manager.products {
        let getter = go_method_name(&product.manager_getter);
        let (kind, type_name, _) = go_product_info(&product.value_type).unwrap_or_else(|| {
            panic!(
                "Go product manager {} declares unsupported product type {} at {}",
                manager.manager_name, product.value_type, product.path
            )
        });
        let field = go_local_name(type_name);
        match kind {
            NativeManagerProductKind::ArmorOffsetDatabase => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *ArmorOffsetDatabase {{
	return manager.{field}
}}

func (manager *{manager_type}) ArmorOffset(name string) *ArmorOffsetData {{
	return armorOffsetByName(manager.{getter}(), name)
}}

func (manager *{manager_type}) FurthestAttachmentOffset(armorOffsetNames []string, attachmentName string, currentPosition Vector3) *AttachmentOffsetData {{
	return furthestArmorAttachmentOffset(manager.{getter}(), armorOffsetNames, attachmentName, currentPosition)
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::EquipTypesDatabase => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *EquipTypesDatabase {{
	return manager.{field}
}}

func (manager *{manager_type}) EquipTypes() []EquipTypeData {{
	return manager.{getter}().EquipTypes
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::GameDebugSettings => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *GameDebugSettings {{
	return manager.{field}
}}

func (manager *{manager_type}) Combat() *CombatDebugSettings {{
	return rowCopy(manager.{getter}().CombatSettings)
}}

func (manager *{manager_type}) DisabledCombatToggleCount() int {{
	return disabledCombatToggleCount(*manager.Combat())
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::UiDatabase => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *UIDatabase {{
	return manager.{field}
}}

func (manager *{manager_type}) InteractOptions() []InteractOptionData {{
	return manager.{getter}().UnifiedInteractData.InteractOptions
}}

func (manager *{manager_type}) InteractOption(id CRC32) *InteractOptionData {{
	return interactOptionByID(manager.InteractOptions(), id)
}}

func (manager *{manager_type}) InteractOptionByName(name string) *InteractOptionData {{
	return manager.InteractOption(gametypes.CRC32FromStringLower(name))
}}

func (manager *{manager_type}) InteractOptionsByCategory(category int32) iter.Seq[InteractOptionData] {{
	return interactOptionsByCategory(manager.InteractOptions(), category)
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::GameCameraSettings => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *GameCameraSettings {{
	return manager.{field}
}}

func (manager *{manager_type}) CameraStates() []CameraStateSettings {{
	return manager.{getter}().CameraStates
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::PlayerBaseAttributes => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *PlayerBaseAttributes {{
	return manager.{field}
}}

func (manager *{manager_type}) PlayerAttributeData() *PlayerAttributeData {{
	return rowCopy(manager.{getter}().PlayerAttributeData)
}}

func (manager *{manager_type}) MaxPerks(rarityLevel int) *int32 {{
	data := manager.PlayerAttributeData()
	if rarityLevel < 0 || rarityLevel >= len(data.ItemRarityData) {{
		return nil
	}}
	value := data.ItemRarityData[rarityLevel].MaxPerkCount
	return &value
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::SettlementProgressionData => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *SettlementProgressionData {{
	return manager.{field}
}}

func (manager *{manager_type}) SettlementProgressionCategories() []ProgressionCategoryEntry {{
	return manager.{getter}().SettlementProgressionCategories
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::GatheringDatabase => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *GatheringDatabase {{
	return manager.{field}
}}

func (manager *{manager_type}) GatheringData() *GatheringData {{
	return rowCopy(manager.{getter}().GatheringData)
}}

func (manager *{manager_type}) GatheringTypes() []GatheringTypeData {{
	return manager.GatheringData().GatheringTypes
}}

func (manager *{manager_type}) GatheringActions() []GatheringAction {{
	return manager.GatheringData().GatheringActions
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::GatheringActionDatabase => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *GatheringActionDatabase {{
	return manager.{field}
}}

func (manager *{manager_type}) GatheringActionData() []GatheringActionData {{
	return manager.{getter}().GatheringActions
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::CraftingStationDatabase => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *CraftingStationDatabase {{
	return manager.{field}
}}

func (manager *{manager_type}) CraftingStations() []CraftingStationData {{
	return manager.{getter}().CraftingStations
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
            NativeManagerProductKind::SocialRankDatabase => {
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {getter}() *SocialRankDatabase {{
	return manager.{field}
}}

func (manager *{manager_type}) Ranks() []SocialRankData {{
	return manager.{getter}().Ranks
}}

"#,
                    getter = getter,
                    manager_type = manager_type,
                ));
            }
        }
    }
    source
}

fn push_product_backed_manager_type(source: &mut String, manager: &DirectManagerSurface) {
    let manager_type = go_method_name(&manager.manager_class_name);
    let constructor = go_manager_constructor_name(&manager_type);
    let manager_resources = go_manager_resources_expression(
        &manager.manager_name,
        manager
            .tables
            .iter()
            .map(|table| (table.table_name.as_str(), table.row_type_name.as_str())),
        manager.products.iter().map(|product| product.path.as_str()),
    );
    let mut product_methods = direct_go_product_methods(manager);
    product_methods.push_str(&special_go_manager_extra_methods(
        &manager.manager_class_name,
    ));
    let (product_fields, product_initializers, product_field_values) = go_product_storage(manager);
    source.push_str(&format!(
        r#"
type {manager_type} struct {{
{product_fields}
}}

func {constructor}(cache *managerCache) (*{manager_type}, error) {{
	resources, err := {manager_resources}
	if err != nil {{
		return nil, err
	}}
{product_initializers}
	return &{manager_type}{{
{product_field_values}	}}, nil
}}

{product_methods}
"#
    ));
}

fn push_item_data_manager_type(source: &mut String, manager: &ItemDataManagerSurface) {
    let manager_type = go_method_name(&manager.manager_class_name);
    let factory = go_manager_constructor_name(&manager_type);
    let table_type = go_method_name(&manager.table_type_name);
    let handle_type = go_method_name(&manager.handle_type_name);
    let data_type = go_method_name(&manager.data_type_name);
    let manager_resources = go_manager_resources_expression(
        &manager.manager_name,
        manager
            .tables
            .iter()
            .map(|table| (table.table_name.as_str(), table.row_type_name.as_str())),
        std::iter::empty(),
    );
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
    let table_selector_arms = manager
        .tables
        .iter()
        .map(|table| {
            format!(
                "\tcase {table_type}{}:\n\t\treturn tableSelector{{name: {}, rowType: {}}}\n",
                table.variant_name,
                go_string(&table.table_name),
                go_string(&table.row_type_name)
            )
        })
        .collect::<String>();

    source.push_str(&format!(
        r#"
type {table_type} string

const (
{const_entries})

func (table {table_type}) TableName() string {{
	return string(table)
}}

func (table {table_type}) selector() tableSelector {{
	switch table {{
{table_selector_arms}	default:
		panic("unknown {table_type}")
	}}
}}

type {handle_type} struct {{
	Table {table_type}
	Row   uint32
}}

type {data_type} struct {{
	SourceHandle             {handle_type}
	Definition               MasterItemDefinitionsSchemaRow
	ItemID                   string
	ItemIDCRC                CRC32
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
	items     []{data_type}
	itemsByID map[CRC32]int
}}

func {factory}(cache *managerCache) (*{manager_type}, error) {{
	resources, err := {manager_resources}
	if err != nil {{
		return nil, err
	}}
	items, err := materialize{manager_type}(resources)
	if err != nil {{
		return nil, err
	}}
	manager := &{manager_type}{{
		items:     items,
		itemsByID: map[CRC32]int{{}},
	}}
	for index := range items {{
		manager.itemsByID[items[index].ItemIDCRC] = index
	}}
	return manager, nil
}}

func (manager *{manager_type}) Get(itemID string) *{data_type} {{
	return manager.GetFromID(CRC32(crc32Lowercase(itemID)))
}}

func (manager *{manager_type}) GetFromID(itemID CRC32) *{data_type} {{
	index, ok := manager.itemsByID[itemID]
	if !ok {{
		return nil
	}}
	return rowCopy(manager.items[index])
}}

func (manager *{manager_type}) ByIndex(index uint32) *{data_type} {{
	if index == 0 {{
		return nil
	}}
	zeroBased := index - 1
	if int(zeroBased) >= len(manager.items) {{
		return nil
	}}
	return rowCopy(manager.items[zeroBased])
}}

func (manager *{manager_type}) Rows() iter.Seq[{data_type}] {{
	return rowValues(manager.items)
}}

func (manager *{manager_type}) Len() int {{
	return len(manager.items)
}}

func (manager *{manager_type}) IsEmpty() bool {{
	return len(manager.items) == 0
}}

func materialize{manager_type}(resources *managerResources) ([]{data_type}, error) {{
	items := []{data_type}{{}}
	seen := map[CRC32]struct{{}}{{}}
	for _, tableID := range itemDataManagerTables {{
		table := resources.table(tableID.selector())
		if table == nil {{
			return nil, fmt.Errorf("manager {manager_type} table %s was not loaded", tableID.TableName())
		}}
		if err := cache{manager_type}Rows(&items, seen, tableID, table); err != nil {{
			return nil, err
		}}
	}}
	return items, nil
}}

func cache{manager_type}Rows(items *[]{data_type}, seen map[CRC32]struct{{}}, tableID {table_type}, table *dynamicTable) error {{
	for _, sourceRow := range table.Rows {{
		definition, err := readMasterItemDefinitionsSchemaRow(table, sourceRow)
		if err != nil {{
			return err
		}}
		itemID := definition.ItemID
		itemID = strings.TrimSpace(itemID)
		if itemID == "" {{
			continue
		}}
		itemIDCRC := CRC32(crc32Lowercase(itemID))
		if itemIDCRC == 0 {{
			continue
		}}
		if _, exists := seen[itemIDCRC]; exists {{
			continue
		}}
		seen[itemIDCRC] = struct{{}}{{}}
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
			Definition:               definition,
			ItemID:                   itemID,
			ItemIDCRC:                itemIDCRC,
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

"#
    ));
}

fn push_semantic_manager_type(source: &mut String, record: &SemanticManagerRecord) {
    let manager_type = go_method_name(&record.manager_class_name);
    let record_type = go_method_name(&record.record_type_name);
    let constructor = go_manager_constructor_name(&manager_type);
    let manager_resources = go_manager_resources_expression(
        &record.manager_name,
        record
            .tables
            .iter()
            .map(|table| (table.table_name.as_str(), table.row_type_name.as_str())),
        std::iter::empty(),
    );
    let by_key_field = "entriesByKey";
    let by_source_row_field = "entriesBySourceRow";
    let has_key_index = !record.lookup_methods.is_empty();
    let has_source_row_index = record.source_row_method.is_some();
    assert!(
        !has_key_index || record.key.is_some(),
        "{manager_type} exposes key lookups without a semantic key"
    );
    assert!(
        !has_source_row_index || record.source_row_field.is_some(),
        "{manager_type} exposes a source-row lookup without a source-row field"
    );
    let key_index_field = if has_key_index {
        format!("\t{by_key_field} map[{}]int\n", go_key_map_type(record))
    } else {
        String::new()
    };
    let source_row_index_field = if has_source_row_index {
        "\tentriesBySourceRow map[uint32]int\n"
    } else {
        ""
    };
    let key_index_initializer = if has_key_index {
        format!(
            "\t\t{by_key_field}: map[{}]int{{}},\n",
            go_key_map_type(record)
        )
    } else {
        String::new()
    };
    let source_row_index_initializer = if has_source_row_index {
        "\t\tentriesBySourceRow: map[uint32]int{},\n"
    } else {
        ""
    };
    let mut index_build = String::new();
    if has_key_index || has_source_row_index {
        index_build.push_str("\tfor index := range rows {\n");
        if has_key_index {
            let index_expression = go_row_index_expression(record)
                .expect("semantic manager key index requires a semantic key");
            index_build.push_str(&format!(
                "\t\tmanager.{by_key_field}[{index_expression}] = index\n"
            ));
        }
        if has_source_row_index {
            let field = record
                .source_row_field
                .as_ref()
                .expect("source-row index requires a source-row field");
            index_build.push_str(&format!(
                "\t\tmanager.{by_source_row_field}[rows[index].{}] = index\n",
                go_field_name(field)
            ));
        }
        index_build.push_str("\t}\n");
    }
    source.push_str(&format!(
        r#"
type {manager_type} struct {{
	entries []{record_type}
{key_index_field}{source_row_index_field}
}}

func {constructor}(cache *managerCache) (*{manager_type}, error) {{
	resources, err := {manager_resources}
	if err != nil {{
		return nil, err
	}}
	rows, err := materialize{manager_type}(resources)
	if err != nil {{
		return nil, err
	}}
	manager := &{manager_type}{{
		entries: rows,
{key_index_initializer}{source_row_index_initializer}
	}}
{index_build}
"#
    ));
    source.push_str(
        r#"	return manager, nil
}

"#,
    );

    for method in &record.lookup_methods {
        let method_name = go_method_name(&method.name);
        let parameter_name = go_local_name(&method.parameter);
        match method.kind {
            SemanticLookupKind::CrcString => source.push_str(&format!(
                r#"func (manager *{manager_type}) {method_name}({parameter_name} string) *{record_type} {{
	index, ok := manager.{by_key_field}[CRC32(crc32Lowercase({parameter_name}))]
	if !ok {{
		return nil
	}}
	return rowCopy(manager.entries[index])
}}

"#
            )),
            SemanticLookupKind::Crc => source.push_str(&format!(
                r#"func (manager *{manager_type}) {method_name}({parameter_name} CRC32) *{record_type} {{
	index, ok := manager.{by_key_field}[{parameter_name}]
	if !ok {{
		return nil
	}}
	return rowCopy(manager.entries[index])
}}

"#
            )),
            SemanticLookupKind::IntoCrc => source.push_str(&format!(
                r#"func (manager *{manager_type}) {method_name}({parameter_name} CRC32) *{record_type} {{
	index, ok := manager.{by_key_field}[{parameter_name}]
	if !ok {{
		return nil
	}}
	return rowCopy(manager.entries[index])
}}

"#
            )),
            SemanticLookupKind::Numeric(key_type) => {
                let parameter_type = go_numeric_key_type(key_type);
                let key_value = go_numeric_key_as_u32(&parameter_name, key_type);
                source.push_str(&format!(
                    r#"func (manager *{manager_type}) {method_name}({parameter_name} {parameter_type}) *{record_type} {{
	index, ok := manager.{by_key_field}[{key_value}]
	if !ok {{
		return nil
	}}
	return rowCopy(manager.entries[index])
}}

"#
                ));
            }
            SemanticLookupKind::String => source.push_str(&format!(
                r#"func (manager *{manager_type}) {method_name}({parameter_name} string) *{record_type} {{
	index, ok := manager.{by_key_field}[normalizeStringKey({parameter_name})]
	if !ok {{
		return nil
	}}
	return rowCopy(manager.entries[index])
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
	return rowCopy(manager.entries[index])
}}

"#
        ));
    }
    if let Some(method) = &record.ids_method {
        let method_name = go_method_name(method);
        let id_type = go_ids_type(record);
        let id_expression = go_ids_expression(record);
        source.push_str(&format!(
            r#"func (manager *{manager_type}) {method_name}() iter.Seq[{id_type}] {{
	return func(yield func({id_type}) bool) {{
		for index := range manager.entries {{
			if !yield({id_expression}) {{
				return
			}}
		}}
	}}
}}

"#
        ));
    }
    source.push_str(&format!(
        r#"func (manager *{manager_type}) Rows() iter.Seq[{record_type}] {{
	return rowValues(manager.entries)
}}

"#
    ));
    if let Some(method) = &record.rows_method {
        let method_name = go_method_name(method);
        if method_name != "Rows" {
            source.push_str(&format!(
                r#"func (manager *{manager_type}) {method_name}() iter.Seq[{record_type}] {{
	return rowValues(manager.entries)
}}

"#
            ));
        }
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

    source.push_str(&special_go_manager_extra_methods(&manager_type));

    push_go_semantic_materializer(source, record);
}

fn special_go_manager_extra_methods(manager_type: &str) -> String {
    match manager_type {
        "PlayerDataManager" => r#"func (manager *PlayerDataManager) CategoricalProgressionID(tradeskill TradeskillType) (*CRC32, error) {
	normalized, err := normalizeTradeskillType(tradeskill)
	if err != nil {
		return nil, err
	}
	if normalized == "None" || normalized == "WildernessSurvival" {
		return nil, nil
	}
	value := CRC32(crc32Lowercase(normalized))
	return &value, nil
}

"#
        .to_owned(),
        _ => String::new(),
    }
}

fn push_go_managers_facade(source: &mut String, surfaces: &[ManagerSurface]) {
    let mut fields = String::new();
    let mut methods = String::new();
    let mut seen = BTreeSet::new();
    for surface in surfaces {
        let manager_name = manager_surface_name(surface);
        if !seen.insert(manager_name) {
            continue;
        }
        let manager_type = go_method_name(match surface {
            ManagerSurface::Direct(manager) | ManagerSurface::ProductBacked(manager) => {
                manager.manager_class_name.as_str()
            }
            ManagerSurface::Native { manager, .. } => manager.manager_class_name.as_str(),
            ManagerSurface::Semantic(record) => record.manager_class_name.as_str(),
            ManagerSurface::ItemData(manager) => manager.manager_class_name.as_str(),
            ManagerSurface::Composition(manager) => manager.manager_class_name.as_str(),
        });
        let accessor = go_manager_accessor_name(manager_name);
        let constructor = go_manager_constructor_name(&manager_type);
        let field = go_local_name(&accessor);
        fields.push_str(&format!(
            "\t{field}Once sync.Once\n\t{field} *{manager_type}\n\t{field}Err error\n"
        ));
        let (dependencies, arguments) = match surface {
            ManagerSurface::Composition(manager) => {
                go_lazy_manager_dependencies(&field, &manager.dependencies)
            }
            ManagerSurface::Native { dependencies, .. } => {
                let (load, names) = go_lazy_manager_dependencies(&field, dependencies);
                let arguments = if names.is_empty() {
                    "managers.cache".to_owned()
                } else {
                    format!("managers.cache, {names}")
                };
                (load, arguments)
            }
            ManagerSurface::Direct(_)
            | ManagerSurface::Semantic(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::ProductBacked(_) => (String::new(), "managers.cache".to_owned()),
        };
        methods.push_str(&format!(
            r#"func (managers *Managers) {accessor}() (*{manager_type}, error) {{
	managers.{field}Once.Do(func() {{
{dependencies}		value, err := {constructor}({arguments})
		if err != nil {{
			managers.{field}Err = &ManagerLoadError{{Manager: {manager_name:?}, Err: err}}
			return
		}}
		managers.{field} = value
	}})
	return managers.{field}, managers.{field}Err
}}

"#
        ));
    }
    source.push_str(&format!(
        r#"
type ManagerLoadError struct {{
	Manager string
	Err error
}}

func (err *ManagerLoadError) Error() string {{
	return fmt.Sprintf("load %s: %v", err.Manager, err.Err)
}}

func (err *ManagerLoadError) Unwrap() error {{ return err.Err }}

type Managers struct {{
    cache *managerCache
{fields}}}

func New(loader *assets.AssetLoader) (*Managers, error) {{
	tableSchemas, err := loadTableSchemas()
	if err != nil {{
		return nil, err
	}}
	return &Managers{{cache: newManagerCache(loader, tableSchemas)}}, nil
}}

{methods}"#
    ));
}

fn go_lazy_manager_dependencies(field: &str, dependencies: &[String]) -> (String, String) {
    let mut load = String::new();
    let mut names = Vec::new();
    for dependency in dependencies {
        let accessor = go_manager_accessor_name(dependency);
        let local = go_local_name(&accessor);
        load.push_str(&format!(
            "\t\t{local}, err := managers.{accessor}()\n\t\tif err != nil {{\n\t\t\tmanagers.{}Err = err\n\t\t\treturn\n\t\t}}\n",
            field,
        ));
        names.push(local);
    }
    (load, names.join(", "))
}

fn go_key_map_type(record: &SemanticManagerRecord) -> &'static str {
    match record.key {
        Some(SemanticManagerKey::String { .. } | SemanticManagerKey::EnumString { .. }) => "string",
        Some(SemanticManagerKey::Crc { .. } | SemanticManagerKey::FallbackCrc { .. }) => "CRC32",
        Some(SemanticManagerKey::Numeric { .. }) => "uint32",
        None => "uint32",
    }
}

fn go_row_index_expression(record: &SemanticManagerRecord) -> Option<String> {
    Some(match record.key.as_ref()? {
        SemanticManagerKey::Crc { crc_field, .. }
        | SemanticManagerKey::FallbackCrc { crc_field, .. } => {
            format!("rows[index].{}", go_field_name(crc_field))
        }
        SemanticManagerKey::Numeric {
            key_field,
            key_type,
            ..
        } => {
            let field = format!("rows[index].{}", go_field_name(key_field));
            go_numeric_key_as_u32(&field, *key_type)
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

fn go_numeric_key_as_u32(value: &str, key_type: SemanticNumericKeyType) -> String {
    match key_type {
        SemanticNumericKeyType::U8 | SemanticNumericKeyType::U16 => {
            format!("uint32({value})")
        }
        SemanticNumericKeyType::U32 => value.to_owned(),
    }
}

fn go_ids_type(record: &SemanticManagerRecord) -> &'static str {
    match record.key {
        Some(SemanticManagerKey::String { .. } | SemanticManagerKey::EnumString { .. }) => "string",
        Some(SemanticManagerKey::Numeric { key_type, .. }) => go_numeric_key_type(key_type),
        Some(SemanticManagerKey::Crc { .. } | SemanticManagerKey::FallbackCrc { .. }) => "CRC32",
        None => "uint32",
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
    let manager_type = go_method_name(&record.manager_class_name);
    let record_type = go_method_name(&record.record_type_name);
    source.push_str(&format!(
        r#"func materialize{manager_type}(resources *managerResources) ([]{record_type}, error) {{
	rows := []{record_type}{{}}
"#
    ));
    if record.key.is_some() {
        source.push_str("\tseen := map[any]struct{}{}\n");
    }
    source.push_str(
        r#"
	for _, table := range resources.tableOrder {
		for _, sourceRow := range table.Rows {
"#,
    );
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

    for field in &record.fields {
        let local = field_temp_name(field);
        let value = go_projection_value(field);
        if matches!(
            field.transform,
            SemanticProjectionTransform::EnumStringSkipInvalid
                | SemanticProjectionTransform::EnumStringRejectDefault
        ) {
            source.push_str(&format!(
                "			{local}, err := {value}\n			if err != nil {{\n				continue\n			}}\n"
            ));
        } else {
            source.push_str(&format!(
                "			{local}, err := {value}\n			if err != nil {{\n				return nil, err\n			}}\n"
            ));
        }
    }
    for field in &record.fields {
        let local = field_temp_name(field);
        match field.transform {
            SemanticProjectionTransform::NonEmptyString
            | SemanticProjectionTransform::NonEmptyStringList => source.push_str(&format!(
                "\t\t\tif len({local}) == 0 {{\n\t\t\t\tcontinue\n\t\t\t}}\n"
            )),
            SemanticProjectionTransform::EnumStringRejectDefault => {
                let enum_type = go_method_name(semantic_enum_type_name(field));
                let default = go_method_name(semantic_enum_default_variant(field));
                source.push_str(&format!(
                    "\t\t\tif {local} == {enum_type}{default} {{\n\t\t\t\tcontinue\n\t\t\t}}\n"
                ));
            }
            _ => {}
        }
    }
    source.push_str(&format!("			row := {record_type}{{\n"));
    if let Some(field) = &record.source_row_field {
        source.push_str(&format!(
            "\t\t\t\t{}: uint32(sourceRow.RowIndex + 1),\n",
            go_field_name(field)
        ));
    }
    push_go_key_row_fields(source, record);
    for field in &record.fields {
        source.push_str(&format!(
            "\t\t\t\t{}: {},\n",
            go_field_name(&field.name),
            field_temp_name(field)
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
            let column = go_string(key_column);
            if *skip_empty_key {
                source.push_str(&format!(
                    r#"			keyTextValue, err := optionalStringCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			if keyTextValue == nil {{
				continue
			}}
			keyText := *keyTextValue
"#
                ));
            } else {
                source.push_str(&format!(
                    r#"			keyText, err := requiredStringCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
"#
                ));
            }
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
            source.push_str("\t\t\tkeyCRC := CRC32(crc32Lowercase(keyValue))\n");
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
                r#"			keyCRC := CRC32(crc32Lowercase(keyValue))
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
            let column = go_string(key_column);
            if *skip_empty_key {
                source.push_str(&format!(
                    r#"			keyTextValue, err := optionalStringCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			if keyTextValue == nil {{
				continue
			}}
			keyText := *keyTextValue
"#
                ));
            } else {
                source.push_str(&format!(
                    r#"			keyText, err := requiredStringCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
"#
                ));
            }
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
            let column = go_string(key_column);
            if *skip_empty_key {
                source.push_str(&format!(
                    r#"			keyValuePointer, err := optionalStringCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
			if keyValuePointer == nil {{
				continue
			}}
			keyValue := *keyValuePointer
"#
                ));
            } else {
                source.push_str(&format!(
                    r#"			keyValue, err := requiredStringCell(table, sourceRow, {column})
			if err != nil {{
				return nil, err
			}}
"#
                ));
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

fn field_temp_name(field: &crate::manager_records::SemanticRecordField) -> String {
    field_temp_name_by_name(&field.name)
}

fn field_temp_name_by_name(field_name: &str) -> String {
    format!("{}Value", go_local_name(field_name))
}

fn go_projection_value(field: &crate::manager_records::SemanticRecordField) -> String {
    let column = go_string(&field.column);
    match field.transform {
        SemanticProjectionTransform::String | SemanticProjectionTransform::NonEmptyString => {
            format!("requiredStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::EnumString
        | SemanticProjectionTransform::EnumStringSkipInvalid
        | SemanticProjectionTransform::EnumStringRejectDefault => {
            let enum_type = go_method_name(semantic_enum_type_name(field));
            format!("requiredEnumCell(table, sourceRow, {column}, parse{enum_type})")
        }
        SemanticProjectionTransform::EnumDefault => {
            let enum_type = go_method_name(semantic_enum_type_name(field));
            let default = go_method_name(semantic_enum_default_variant(field));
            format!(
                "enumCellOr(table, sourceRow, {column}, {enum_type}{default}, parse{enum_type})"
            )
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
        SemanticProjectionTransform::OptionalFirstString => {
            format!("optionalFirstStringCell(table, sourceRow, {column})")
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
        SemanticProjectionTransform::BoolDefaultFalse => {
            format!("boolCellDefaultFalse(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::Crc32NonZeroBool => {
            let reference = field
                .reference_field
                .as_deref()
                .expect("CRC presence projections have reference fields");
            format!(
                "func() (bool, error) {{ return {} != 0, nil }}()",
                field_temp_name_by_name(reference)
            )
        }
        SemanticProjectionTransform::U8 => {
            format!("requiredUint8Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::NonZeroU8 => {
            format!("requiredNonZeroUint8Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U8DefaultZero => {
            format!("uint8CellDefault(table, sourceRow, {column}, 0)")
        }
        SemanticProjectionTransform::U8DefaultMax => {
            format!("uint8CellDefault(table, sourceRow, {column}, 0xff)")
        }
        SemanticProjectionTransform::U16 => {
            format!("requiredUint16Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::NonZeroU16 => {
            format!("requiredNonZeroUint16Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U16BelowMax => {
            let max = field
                .u16_max_exclusive
                .expect("capped u16 projections have a maximum");
            format!(
                "func() (uint16, error) {{ value, err := requiredUint16Cell(table, sourceRow, {column}); if err != nil {{ return 0, err }}; if uint32(value) >= {max} {{ return 0, fmt.Errorf(\"row %s:%d %s exceeds supported cap {max}\", sourceRow.SourcePath, sourceRow.RowIndex+1, {column}) }}; return value, nil }}()"
            )
        }
        SemanticProjectionTransform::U32 => {
            format!("requiredUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalU32 => {
            format!("optionalUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U32DefaultZero => {
            format!("uint32CellDefaultZero(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::NonZeroU32 => {
            format!("requiredNonZeroUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalNonZeroU32 => {
            format!("optionalNonZeroUint32Cell(table, sourceRow, {column})")
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
        SemanticProjectionTransform::F32MinutesToSeconds => {
            format!("float32MinutesToSecondsCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::F32UpperBound10000ZeroIsDefault => {
            format!("upperBoundCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::F32LowerBound10000CappedToField => {
            let reference = field
                .reference_field
                .as_deref()
                .expect("lower-bound projections have reference fields");
            format!(
                "lowerBoundCell(table, sourceRow, {column}, {})",
                field_temp_name_by_name(reference)
            )
        }
        SemanticProjectionTransform::F32List => {
            format!("float32ListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::I32List => {
            format!("int32ListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::Crc32 => {
            format!("requiredCRC32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::LowercaseCrcString => {
            format!("lowercaseCrcStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::LowercaseCrcStringDefaultZero => {
            format!("lowercaseCrcStringDefaultZero(table, sourceRow, {column}, false)")
        }
        SemanticProjectionTransform::FirstLowercaseCrcStringDefaultZero => {
            format!("lowercaseCrcStringDefaultZero(table, sourceRow, {column}, true)")
        }
        SemanticProjectionTransform::TrimmedLowercaseCrcStringDefaultZero => {
            format!("trimmedLowercaseCrcStringDefaultZero(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalCrc32 => {
            format!("optionalCrc32Cell(table, sourceRow, {column}, false)")
        }
        SemanticProjectionTransform::OptionalCrc32ZeroAsNone => {
            format!("optionalCrc32Cell(table, sourceRow, {column}, true)")
        }
        SemanticProjectionTransform::Crc32List => {
            format!("crc32ListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalLowercaseCrcString => {
            format!("optionalLowercaseCrcStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalTrimmedLowercaseCrcString => {
            format!("optionalTrimmedLowercaseCrcStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::LowercaseCrcStringList => {
            format!("lowercaseCrcStringListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::ForeignKey => {
            format!("requiredStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalForeignKey => {
            format!("optionalStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::ForeignKeyList => {
            format!("stringListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::F32RangeInclusive => {
            format!("float32RangeCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U32RangeInclusive => {
            format!("uint32RangeCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalCrc32F32PairList => {
            format!("optionalCRC32Float32PairListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalU8F32PairList => {
            let enum_shape = field
                .pair_first_enum_shape
                .as_ref()
                .expect("u8 pair-list projections have a reconciled enum schema");
            let parser = go_pair_enum_parser_name(&enum_shape.name);
            format!("optionalUint8Float32PairListCell(table, sourceRow, {column}, {parser})")
        }
    }
}

const SEMANTIC_MANAGER_RUNTIME_GO: &str = r#"
func requiredEnumCell[T any](table *dynamicTable, row dynamicTableRow, columnName string, parse func(string) (T, error)) (T, error) {
	var zero T
	value, err := requiredStringCell(table, row, columnName)
	if err != nil {
		return zero, err
	}
	return parse(value)
}

func enumCellOr[T any](table *dynamicTable, row dynamicTableRow, columnName string, fallback T, parse func(string) (T, error)) (T, error) {
	value, err := optionalStringCell(table, row, columnName)
	if err != nil {
		var zero T
		return zero, err
	}
	if value == nil {
		return fallback, nil
	}
	return parse(*value)
}

func rowCell(table *dynamicTable, row dynamicTableRow, columnName string) (*gameassets.DatasheetCellValue, bool) {
	columnCRC, ok := table.ColumnCRCs[columnName]
	if !ok {
		return nil, false
	}
	slot, ok := row.ColumnSlots[columnCRC]
	if !ok || slot < 0 || slot >= len(row.Row.Cells) {
		return nil, false
	}
	value := row.Row.Cells[slot].Value
	return &value, true
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

func optionalFirstStringCell(table *dynamicTable, row dynamicTableRow, columnName string) (*string, error) {
	values, err := optionalStringListCell(table, row, columnName)
	if err != nil || values == nil || len(*values) == 0 {
		return nil, err
	}
	value := (*values)[0]
	return &value, nil
}

func boolCellDefaultFalse(table *dynamicTable, row dynamicTableRow, columnName string) (bool, error) {
	value, err := optionalBoolCell(table, row, columnName)
	if err != nil || value == nil {
		return false, err
	}
	return *value, nil
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
	return 0, false, fmt.Errorf("row %s:%d has non-number %s=%q", row.SourcePath, row.RowIndex+1, columnName, value.String)
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

func uint32CellDefaultZero(table *dynamicTable, row dynamicTableRow, columnName string) (uint32, error) {
	value, err := optionalUint32Cell(table, row, columnName)
	if err != nil || value == nil {
		return 0, err
	}
	return *value, nil
}

func requiredNonZeroUint32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (uint32, error) {
	value, err := requiredUint32Cell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	if value == 0 {
		return 0, fmt.Errorf("row %s:%d %s must be non-zero", row.SourcePath, row.RowIndex+1, columnName)
	}
	return value, nil
}

func optionalNonZeroUint32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (*uint32, error) {
	value, err := optionalUint32Cell(table, row, columnName)
	if err != nil || value == nil {
		return nil, err
	}
	if *value == 0 {
		return nil, fmt.Errorf("row %s:%d %s must be non-zero", row.SourcePath, row.RowIndex+1, columnName)
	}
	return value, nil
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

func requiredNonZeroUint16Cell(table *dynamicTable, row dynamicTableRow, columnName string) (uint16, error) {
	value, err := requiredUint16Cell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	if value == 0 {
		return 0, fmt.Errorf("row %s:%d %s must be non-zero", row.SourcePath, row.RowIndex+1, columnName)
	}
	return value, nil
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

func uint8CellDefault(table *dynamicTable, row dynamicTableRow, columnName string, defaultValue uint8) (uint8, error) {
	value, err := optionalUint8Cell(table, row, columnName)
	if err != nil || value == nil {
		return defaultValue, err
	}
	return *value, nil
}

func requiredNonZeroUint8Cell(table *dynamicTable, row dynamicTableRow, columnName string) (uint8, error) {
	value, err := requiredUint8Cell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	if value == 0 {
		return 0, fmt.Errorf("row %s:%d %s must be non-zero", row.SourcePath, row.RowIndex+1, columnName)
	}
	return value, nil
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

func requiredCRC32Cell(table *dynamicTable, row dynamicTableRow, columnName string) (CRC32, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return 0, fmt.Errorf("row %s:%d missing crc %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	switch value.Kind {
	case gameassets.DatasheetCellNumber:
		normalized, err := normalizeUint32(value.Number)
		return CRC32(normalized), err
	case gameassets.DatasheetCellString:
		return CRC32(crc32Lowercase(value.String)), nil
	default:
		return 0, fmt.Errorf("row %s:%d has non-crc %s", row.SourcePath, row.RowIndex+1, columnName)
	}
}

func optionalCrc32Cell(table *dynamicTable, row dynamicTableRow, columnName string, zeroAsNone bool) (*CRC32, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return nil, nil
	}
	var crc CRC32
	var err error
	switch value.Kind {
	case gameassets.DatasheetCellNumber:
		var normalized uint32
		normalized, err = normalizeUint32(value.Number)
		crc = CRC32(normalized)
	case gameassets.DatasheetCellString:
		if value.String == "" {
			return nil, nil
		}
		crc = CRC32(crc32Lowercase(value.String))
	default:
		return nil, fmt.Errorf("row %s:%d has non-crc %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	if err != nil || zeroAsNone && crc == 0 {
		return nil, err
	}
	return &crc, nil
}

func lowercaseCrcStringCell(table *dynamicTable, row dynamicTableRow, columnName string) (CRC32, error) {
	text, err := requiredStringCell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	return CRC32(crc32Lowercase(text)), nil
}

func optionalLowercaseCrcStringCell(table *dynamicTable, row dynamicTableRow, columnName string) (*CRC32, error) {
	text, err := optionalStringCell(table, row, columnName)
	if err != nil || text == nil {
		return nil, err
	}
	crc := CRC32(crc32Lowercase(*text))
	return &crc, nil
}

func optionalTrimmedLowercaseCrcStringCell(table *dynamicTable, row dynamicTableRow, columnName string) (*CRC32, error) {
	text, err := optionalStringCell(table, row, columnName)
	if err != nil || text == nil {
		return nil, err
	}
	trimmed := strings.TrimSpace(*text)
	if trimmed == "" {
		return nil, nil
	}
	crc := CRC32(crc32Lowercase(trimmed))
	return &crc, nil
}

func lowercaseCrcStringDefaultZero(table *dynamicTable, row dynamicTableRow, columnName string, first bool) (CRC32, error) {
	if first {
		value, err := optionalFirstStringCell(table, row, columnName)
		if err != nil || value == nil {
			return 0, err
		}
		return CRC32(crc32Lowercase(*value)), nil
	}
	value, err := optionalStringCell(table, row, columnName)
	if err != nil || value == nil {
		return 0, err
	}
	return CRC32(crc32Lowercase(*value)), nil
}

func trimmedLowercaseCrcStringDefaultZero(table *dynamicTable, row dynamicTableRow, columnName string) (CRC32, error) {
	value, err := optionalStringCell(table, row, columnName)
	if err != nil || value == nil {
		return 0, err
	}
	return CRC32(crc32Lowercase(strings.TrimSpace(*value))), nil
}

func float32MinutesToSecondsCell(table *dynamicTable, row dynamicTableRow, columnName string) (float32, error) {
	value, err := requiredFloat32Cell(table, row, columnName)
	return value * 60, err
}

func upperBoundCell(table *dynamicTable, row dynamicTableRow, columnName string) (float32, error) {
	value, err := requiredFloat32Cell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	if math.IsNaN(float64(value)) || float32(math.Abs(float64(value))) <= 1.1920929e-7 {
		return 10000, nil
	}
	return min(max(value, 0), 10000), nil
}

func lowerBoundCell(table *dynamicTable, row dynamicTableRow, columnName string, upperBound float32) (float32, error) {
	value, err := requiredFloat32Cell(table, row, columnName)
	if err != nil {
		return 0, err
	}
	if math.IsNaN(float64(value)) {
		value = 0
	}
	return min(min(max(value, 0), 10000), upperBound), nil
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

func crc32ListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]CRC32, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return []CRC32{}, nil
	}
	if value.Kind == gameassets.DatasheetCellNumber {
		normalized, err := normalizeUint32(value.Number)
		if err != nil {
			return nil, err
		}
		return []CRC32{CRC32(normalized)}, nil
	}
	if value.Kind != gameassets.DatasheetCellString {
		return nil, fmt.Errorf("row %s:%d has non-crc-list %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	parts := splitDesignerList(value.String)
	out := make([]CRC32, 0, len(parts))
	for _, part := range parts {
		if number, err := strconv.ParseFloat(part, 32); err == nil {
			normalized, err := normalizeUint32(float32(number))
			if err != nil {
				return nil, err
			}
			out = append(out, CRC32(normalized))
		} else {
			out = append(out, CRC32(crc32Lowercase(part)))
		}
	}
	return out, nil
}

func lowercaseCrcStringListCell(table *dynamicTable, row dynamicTableRow, columnName string) ([]CRC32, error) {
	values, err := stringListCell(table, row, columnName)
	if err != nil {
		return nil, err
	}
	out := make([]CRC32, 0, len(values))
	for _, value := range values {
		if value != "" {
			out = append(out, CRC32(crc32Lowercase(value)))
		}
	}
	return out, nil
}

func optionalCRC32Float32PairListCell(table *dynamicTable, row dynamicTableRow, columnName string) (*[]struct{ First CRC32; Second float32 }, error) {
	return optionalPairListCell(table, row, columnName, func(source string) (CRC32, error) {
		if value, err := strconv.ParseUint(source, 10, 32); err == nil {
			return CRC32(value), nil
		}
		return CRC32(crc32Lowercase(source)), nil
	})
}

func optionalUint8Float32PairListCell(table *dynamicTable, row dynamicTableRow, columnName string, parseFirst func(string) (uint8, error)) (*[]struct{ First uint8; Second float32 }, error) {
	return optionalPairListCell(table, row, columnName, parseFirst)
}

func optionalPairListCell[T any](table *dynamicTable, row dynamicTableRow, columnName string, parseFirst func(string) (T, error)) (*[]struct{ First T; Second float32 }, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok ||
		(value.Kind == gameassets.DatasheetCellNumber && value.Number == 0) ||
		(value.Kind == gameassets.DatasheetCellBoolean && !value.Boolean) ||
		(value.Kind == gameassets.DatasheetCellString && strings.TrimSpace(value.String) == "") {
		return nil, nil
	}
	if value.Kind != gameassets.DatasheetCellString {
		return nil, fmt.Errorf("row %s:%d has non-pair-list %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	entries := splitDesignerList(value.String)
	pairs := make([]struct{ First T; Second float32 }, 0, len(entries))
	for _, entry := range entries {
		firstSource, secondSource, found := strings.Cut(entry, "=")
		if !found {
			return nil, fmt.Errorf("row %s:%d has invalid pair in %s", row.SourcePath, row.RowIndex+1, columnName)
		}
		first, err := parseFirst(strings.TrimSpace(firstSource))
		if err != nil {
			return nil, err
		}
		second, err := strconv.ParseFloat(strings.TrimSpace(secondSource), 32)
		if err != nil || math.IsNaN(second) || math.IsInf(second, 0) {
			return nil, fmt.Errorf("row %s:%d has invalid number in %s", row.SourcePath, row.RowIndex+1, columnName)
		}
		pairs = append(pairs, struct{ First T; Second float32 }{First: first, Second: float32(second)})
	}
	if len(pairs) == 0 {
		return nil, nil
	}
	return &pairs, nil
}

func float32RangeCell(table *dynamicTable, row dynamicTableRow, columnName string) (struct{ First, Second float32 }, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return struct{ First, Second float32 }{}, fmt.Errorf("row %s:%d missing range %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	if value.Kind == gameassets.DatasheetCellNumber && !float32IsFinite(value.Number) {
		return struct{ First, Second float32 }{}, fmt.Errorf("row %s:%d missing range %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	if value.Kind == gameassets.DatasheetCellNumber {
		return struct{ First, Second float32 }{First: value.Number, Second: value.Number}, nil
	}
	if value.Kind != gameassets.DatasheetCellString {
		return struct{ First, Second float32 }{}, fmt.Errorf("row %s:%d has non-number range %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	first, second := float32RangeFromText(value.String)
	return struct{ First, Second float32 }{First: first, Second: second}, nil
}

func uint32RangeCell(table *dynamicTable, row dynamicTableRow, columnName string) (struct{ First, Second uint32 }, error) {
	value, ok := rowCell(table, row, columnName)
	if !ok {
		return struct{ First, Second uint32 }{}, fmt.Errorf("row %s:%d missing unsigned range %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	if value.Kind == gameassets.DatasheetCellNumber {
		endpoint, err := normalizeUint32(value.Number)
		if err != nil {
			return struct{ First, Second uint32 }{}, err
		}
		return struct{ First, Second uint32 }{First: endpoint, Second: endpoint}, nil
	}
	if value.Kind != gameassets.DatasheetCellString {
		return struct{ First, Second uint32 }{}, fmt.Errorf("row %s:%d has invalid unsigned range %s", row.SourcePath, row.RowIndex+1, columnName)
	}
	first, second, err := uint32RangeFromText(value.String)
	if err != nil {
		return struct{ First, Second uint32 }{}, fmt.Errorf("row %s:%d has invalid unsigned range %s: %w", row.SourcePath, row.RowIndex+1, columnName, err)
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

func float32RangeFromText(value string) (float32, float32) {
	parts := strings.Split(strings.TrimSpace(value), "-")
	if len(parts) == 1 {
		endpoint, err := strconv.ParseFloat(strings.TrimSpace(parts[0]), 32)
		if err == nil && !math.IsInf(endpoint, 0) && !math.IsNaN(endpoint) {
			return float32(endpoint), float32(endpoint)
		}
		return 0, 0
	}
	if len(parts) == 2 {
		first, firstErr := strconv.ParseFloat(strings.TrimSpace(parts[0]), 32)
		second, secondErr := strconv.ParseFloat(strings.TrimSpace(parts[1]), 32)
		if firstErr == nil && secondErr == nil && !math.IsInf(first, 0) && !math.IsInf(second, 0) && !math.IsNaN(first) && !math.IsNaN(second) {
			if first <= second {
				return float32(first), float32(second)
			}
			return float32(second), float32(first)
		}
	}
	return 0, 0
}

func uint32RangeFromText(value string) (uint32, uint32, error) {
	parts := strings.Split(strings.TrimSpace(value), "-")
	if len(parts) == 1 && strings.TrimSpace(parts[0]) != "" {
		endpoint, err := strconv.ParseUint(strings.TrimSpace(parts[0]), 10, 32)
		return uint32(endpoint), uint32(endpoint), err
	}
	if len(parts) == 2 && strings.TrimSpace(parts[0]) != "" && strings.TrimSpace(parts[1]) != "" {
		first, firstErr := strconv.ParseUint(strings.TrimSpace(parts[0]), 10, 32)
		if firstErr != nil {
			return 0, 0, firstErr
		}
		second, secondErr := strconv.ParseUint(strings.TrimSpace(parts[1]), 10, 32)
		return uint32(first), uint32(second), secondErr
	}
	return 0, 0, fmt.Errorf("invalid u32 range")
}

func float32IsFinite(value float32) bool {
	return !math.IsInf(float64(value), 0) && !math.IsNaN(float64(value))
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

func crc32Lowercase(value string) uint32 {
	return gametypes.CRC32FromStringLower(value).Value()
}

"#;

const PRODUCT_MANAGER_RUNTIME_GO: &str = r#"
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
	Position        Vector3
	RotationDegrees Vector3
}

type EquipTypesDatabase struct {
	EquipTypes []EquipTypeData
}

type EquipTypeData struct {
	Name                                   string
	Attachment                             string
	AttachmentOffsetPosition               Vector3
	AttachmentOffsetRotationDegrees        Vector3
	SheathData                             string
	SheathOffsetPosition                   Vector3
	SheathOffsetRotationDegrees            Vector3
	OffHandAttachment                      string
	OffHandAttachmentOffsetPosition        Vector3
	OffHandAttachmentOffsetRotationDegrees Vector3
	OffHandSheathData                      string
	OffHandSheathOffsetPosition            Vector3
	OffHandSheathOffsetRotationDegrees     Vector3
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

type UIDatabase struct {
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
	CheckPVPFlagIsSet                              bool
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

const (
	TradeskillNone               TradeskillType = "None"
	TradeskillWeaponsmithing     TradeskillType = "Weaponsmithing"
	TradeskillArmoring           TradeskillType = "Armoring"
	TradeskillJewelcrafting      TradeskillType = "Jewelcrafting"
	TradeskillArcana             TradeskillType = "Arcana"
	TradeskillCooking            TradeskillType = "Cooking"
	TradeskillFurnishing         TradeskillType = "Furnishing"
	TradeskillEngineering        TradeskillType = "Engineering"
	TradeskillSmelting           TradeskillType = "Smelting"
	TradeskillWoodworking        TradeskillType = "Woodworking"
	TradeskillLeatherworking     TradeskillType = "Leatherworking"
	TradeskillWeaving            TradeskillType = "Weaving"
	TradeskillStonecutting       TradeskillType = "Stonecutting"
	TradeskillSkinning           TradeskillType = "Skinning"
	TradeskillMining             TradeskillType = "Mining"
	TradeskillLogging            TradeskillType = "Logging"
	TradeskillHarvesting         TradeskillType = "Harvesting"
	TradeskillWildernessSurvival TradeskillType = "WildernessSurvival"
	TradeskillFishing            TradeskillType = "Fishing"
	TradeskillAzothStaff         TradeskillType = "AzothStaff"
	TradeskillMusician           TradeskillType = "Musician"
	TradeskillRiding             TradeskillType = "Riding"
)

type EditCRC struct {
	ValueStr string
	ValueCRC CRC32
}

type ColorRGBA struct {
	R float32
	G float32
	B float32
	A float32
}

type IntRange struct {
	Min int32
	Max int32
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
	CraftingResultLootBucketID  CRC32
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
	AttributePerkBucketID      CRC32
}

type GuildSiegeWindowRegionData struct {
	StartHour  uint32
	EndHour    uint32
	UTCOffset  int32
	DSTRuleID  CRC32
	DstRule    string
	ObservesDST bool
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
	InfluenceRaceAttackerWinGameEventID EditCRC
	InfluenceRaceDefenderWinGameEventID EditCRC
	InfluenceRaceLoseGameEventID     EditCRC
}

type ValidGroupData struct {
	Names      []string
	Objectives []string
	IconPaths  []string
	Colors     []ColorRGBA
}

type WarData struct {
	DeployableLimits map[CRC32]WarDeployableLimitData
}

type WarDeployableLimitData struct {
	ID             CRC32
	DisplayName    string
	BuildableNames []string
	BuildableIDs   []CRC32
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
	editCRCTypeID     = "9a339de9-0d6e-4708-922f-f46af04370e9"
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

var tradeskillTypes = []TradeskillType{
	TradeskillWeaponsmithing, TradeskillArmoring, TradeskillJewelcrafting, TradeskillArcana,
	TradeskillCooking, TradeskillFurnishing, TradeskillEngineering, TradeskillSmelting,
	TradeskillWoodworking, TradeskillLeatherworking, TradeskillWeaving, TradeskillStonecutting,
	TradeskillSkinning, TradeskillMining, TradeskillLogging, TradeskillHarvesting,
	TradeskillWildernessSurvival, TradeskillFishing, TradeskillAzothStaff, TradeskillMusician,
	TradeskillRiding,
}

func normalizeTradeskillType(value TradeskillType) (string, error) {
	if value == TradeskillNone {
		return string(value), nil
	}
	for _, candidate := range tradeskillTypes {
		if candidate == value {
			return string(value), nil
		}
	}
	return "", fmt.Errorf("unknown TradeskillType %s", value)
}

func parsePlayerBaseAttributes(bytes []byte) (*PlayerBaseAttributes, error) {
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
	if out.CraftingResultLootBucketID, err = requiredCRC32FieldByName(element, "Crafting Result Loot Bucket Id"); err != nil { return out, err }
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
	if out.AttributePerkBucketID, err = requiredCRC32FieldByName(element, "Attribute Perk Bucket Id"); err != nil { return out, err }
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
	if out.DSTRuleID, err = requiredCRC32FieldByName(element, "DstRuleId"); err != nil { return out, err }
	if out.DstRule, err = requiredStringFieldByName(element, "DstRule"); err != nil { return out, err }
	if out.ObservesDST, err = requiredBoolFieldByName(element, "ObservesDst"); err != nil { return out, err }
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
	if out.InfluenceRaceAttackerWinGameEventID, err = parseEditCRC(attackerWin); err != nil { return out, err }
	defenderWin, err := requiredFieldByName(element, "Influence Race Defender Win GameEventId")
	if err != nil { return out, err }
	if out.InfluenceRaceDefenderWinGameEventID, err = parseEditCRC(defenderWin); err != nil { return out, err }
	raceLose, err := requiredFieldByName(element, "Influence Race Lose GameEventId")
	if err != nil { return out, err }
	if out.InfluenceRaceLoseGameEventID, err = parseEditCRC(raceLose); err != nil { return out, err }
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
		color, err := readColorRGBA(&colors.Children[index])
		if err != nil { return out, err }
		out.Colors = append(out.Colors, color)
	}
	return out, nil
}

func parseWarData(element *gameassets.ObjectStreamElement) (WarData, error) {
	out := WarData{DeployableLimits: map[CRC32]WarDeployableLimitData{}}
	limits, err := requiredFieldByName(element, "Deployable Limits")
	if err != nil { return out, err }
	for index := range limits.Children {
		pair := &limits.Children[index]
		key, err := requiredCRC32FieldByName(pair, "value1")
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
	if out.ID, err = requiredCRC32FieldByName(element, "m_id"); err != nil { return out, err }
	if out.DisplayName, err = requiredStringFieldByName(element, "m_displayName"); err != nil { return out, err }
	if out.BuildableNames, err = requiredStringSequenceByName(element, "m_buildableNames"); err != nil { return out, err }
	if out.BuildableIDs, err = requiredCRC32SequenceByName(element, "m_buildableIds"); err != nil { return out, err }
	attackerLimits, err := requiredFieldByName(element, "m_attackerLimits")
	if err != nil { return out, err }
	if out.AttackerLimits, err = readI32Triple(attackerLimits); err != nil { return out, err }
	if out.DefenderLimit, err = requiredI32FieldByName(element, "m_defenderLimit"); err != nil { return out, err }
	return out, nil
}

func parseSettlementProgressionData(bytes []byte) (*SettlementProgressionData, error) {
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

func parseGatheringDatabase(bytes []byte) (*GatheringDatabase, error) {
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

func parseGatheringActionDatabase(bytes []byte) (*GatheringActionDatabase, error) {
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

func parseCraftingStationDatabase(bytes []byte) (*CraftingStationDatabase, error) {
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

func parseSocialRankDatabase(bytes []byte) (*SocialRankDatabase, error) {
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

func parseArmorOffsetDatabase(bytes []byte) (*ArmorOffsetDatabase, error) {
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

func armorOffsetByName(database *ArmorOffsetDatabase, name string) *ArmorOffsetData {
	for index := range database.Offsets {
		if database.Offsets[index].Name == name {
			return rowCopy(database.Offsets[index])
		}
	}
	return nil
}

func furthestArmorAttachmentOffset(database *ArmorOffsetDatabase, armorOffsetNames []string, attachmentName string, currentPosition Vector3) *AttachmentOffsetData {
	var best *AttachmentOffsetData
	bestLength := vec3Length(currentPosition)
	for _, offsetName := range armorOffsetNames {
		offset := armorOffsetByName(database, offsetName)
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

func parseEquipTypesDatabase(bytes []byte) (*EquipTypesDatabase, error) {
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

func parseGameDebugSettings(bytes []byte) (*GameDebugSettings, error) {
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

func disabledCombatToggleCount(combat CombatDebugSettings) int {
	count := 0
	if combat.DisablePlayerLootDropOnDeath { count++ }
	if combat.DisableWeaponDurability { count++ }
	if combat.DisableItemDurability { count++ }
	if combat.DisableDurabilityPenaltyOnDeath { count++ }
	return count
}

func parseUIDatabase(bytes []byte) (*UIDatabase, error) {
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
	database := &UIDatabase{}
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
	if option.CheckPVPFlagIsSet, err = boolChild(element, 36); err != nil { return InteractOptionData{}, err }
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

func interactOptionByID(options []InteractOptionData, id CRC32) *InteractOptionData {
	for index := range options {
		if gametypes.CRC32FromStringLower(options[index].Name) == id {
			return rowCopy(options[index])
		}
	}
	return nil
}

func interactOptionsByCategory(options []InteractOptionData, category int32) iter.Seq[InteractOptionData] {
	return func(yield func(InteractOptionData) bool) {
		for index := range options {
			option := options[index]
			if option.InteractOptionCategory != category && option.InteractOptionCategory != AllInteractOptionsCategory {
				continue
			}
			if !yield(option) {
				return
			}
		}
	}
}

func parseGameCameraSettings(bytes []byte) (*GameCameraSettings, error) {
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

func requiredVec3Field(element *gameassets.ObjectStreamElement, nameCRC uint32) (Vector3, error) {
	child, err := requiredTypedChild(element, nameCRC, vector3TypeID)
	if err != nil {
		return Vector3{}, err
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

func requiredCRC32FieldByName(element *gameassets.ObjectStreamElement, fieldName string) (CRC32, error) {
	child, err := requiredFieldByName(element, fieldName)
	if err != nil {
		return 0, err
	}
	return readCRC32(child)
}

func requiredStringSequenceByName(element *gameassets.ObjectStreamElement, fieldName string) ([]string, error) {
	child, err := requiredFieldByName(element, fieldName)
	if err != nil {
		return nil, err
	}
	return readStringVector(child)
}

func requiredCRC32SequenceByName(element *gameassets.ObjectStreamElement, fieldName string) ([]CRC32, error) {
	child, err := requiredFieldByName(element, fieldName)
	if err != nil {
		return nil, err
	}
	values := make([]CRC32, 0, len(child.Children))
	for index := range child.Children {
		value, err := readCRC32(&child.Children[index])
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

func readCRC32(element *gameassets.ObjectStreamElement) (CRC32, error) {
	if err := gameassets.RequireObjectStreamType(element, crc32TypeID); err != nil {
		return 0, err
	}
	if len(element.Data) == 4 {
	value, err := gameassets.ObjectStreamU32(element)
	return CRC32(value), err
	}
	value, err := gameassets.RequiredChildByNameCRC(element, crc32Lowercase("Value"))
	if err != nil {
		return 0, err
	}
	raw, err := gameassets.ObjectStreamU32(value)
	return CRC32(raw), err
}

func parseEditCRC(element *gameassets.ObjectStreamElement) (EditCRC, error) {
	if err := gameassets.RequireObjectStreamType(element, editCRCTypeID); err != nil {
		return EditCRC{}, err
	}
	valueStr, err := requiredStringFieldByName(element, "m_valueStr")
	if err != nil {
		return EditCRC{}, err
	}
	valueCRC, err := requiredCRC32FieldByName(element, "m_valueCrc")
	if err != nil {
		return EditCRC{}, err
	}
	return EditCRC{ValueStr: valueStr, ValueCRC: valueCRC}, nil
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

func readColorRGBA(element *gameassets.ObjectStreamElement) (ColorRGBA, error) {
	if err := gameassets.RequireObjectStreamType(element, colorTypeID); err != nil {
		return ColorRGBA{}, err
	}
	if len(element.Data) != 16 {
		return ColorRGBA{}, fmt.Errorf("ObjectStream color has %d bytes, expected 16", len(element.Data))
	}
	return ColorRGBA{
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
		guid, err := uuid.FromBytes(data[0:16])
		if err != nil {
			return AssetReference{}, fmt.Errorf("decode AZ::Data::Asset id: %w", err)
		}
		assetType, err := uuid.FromBytes(data[layout.assetTypeOffset : layout.assetTypeOffset+16])
		if err != nil {
			return AssetReference{}, fmt.Errorf("decode AZ::Data::Asset type: %w", err)
		}
		return AssetReference{
			ID:        AssetID{GUID: guid, SubID: subID},
			AssetType: assetType,
			Hint:      string(data[layout.hintOffset:]),
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

func vec3Length(value Vector3) float64 {
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
type tableSelector struct {
	name    string
	rowType string
}

type managerResources struct {
	managerName string
	tables      map[tableSelector]*dynamicTable
	tableOrder  []*dynamicTable
	assets      map[string][]byte
}

func (resources *managerResources) table(selector tableSelector) *dynamicTable {
	return resources.tables[selector]
}

func (resources *managerResources) assetBytes(path string) ([]byte, bool) {
	normalized := normalizeDataPath(path)
	if bytes, ok := resources.assets[normalized]; ok {
		return bytes, true
	}
	suffix := "/" + normalized
	for candidate, bytes := range resources.assets {
		if strings.HasSuffix(candidate, suffix) {
			return bytes, true
		}
	}
	return nil, false
}

func (resources *managerResources) requiredAssetBytes(path string) ([]byte, error) {
	bytes, ok := resources.assetBytes(path)
	if !ok {
		return nil, fmt.Errorf("manager %s asset %s was not loaded", resources.managerName, path)
	}
	return bytes, nil
}

func schemaFamilyEntries[TTable ~string, TRow any](resources *managerResources, rowType string, read func(*dynamicTable, dynamicTableRow) (TRow, error), resolveTable func(string) (TTable, bool)) ([]RowEntry[TTable, TRow], error) {
	entries := []RowEntry[TTable, TRow]{}
	for _, table := range resources.tableOrder {
		if table.Schema.RowType != rowType {
			continue
		}
		for _, sourceRow := range table.Rows {
			tableID, ok := resolveTable(sourceRow.SourcePath)
			if !ok {
				return nil, fmt.Errorf("manager %s cannot resolve source %s to a typed table", resources.managerName, sourceRow.SourcePath)
			}
			row, err := read(table, sourceRow)
			if err != nil {
				return nil, err
			}
			entries = append(entries, RowEntry[TTable, TRow]{
				Ref:  RowRef[TTable, TRow]{table: tableID, path: sourceRow.SourcePath, key: sourceRow.Key},
				Slot: RowSlot[TTable, TRow]{table: tableID, path: sourceRow.SourcePath, rowIndex: sourceRow.RowIndex},
				Row:  row,
			})
		}
	}
	return entries, nil
}

type managerCache struct {
	mu           sync.Mutex
	loader       *assets.AssetLoader
	assetsByPath map[string][]byte
	tableSchemas []tableSchema
	tableCache   map[string]*dynamicTable
}

func newManagerCache(loader *assets.AssetLoader, tableSchemas []tableSchema) *managerCache {
	return &managerCache{
		loader:       loader,
		assetsByPath: map[string][]byte{},
		tableSchemas: tableSchemas,
		tableCache:   map[string]*dynamicTable{},
	}
}

func (cache *managerCache) tableSchema(selector tableSelector) (*tableSchema, error) {
	var found *tableSchema
	for index := range cache.tableSchemas {
		if cache.tableSchemas[index].Name != selector.name || cache.tableSchemas[index].RowType != selector.rowType {
			continue
		}
		if found != nil {
			return nil, fmt.Errorf("duplicate table schema %s:%s", selector.name, selector.rowType)
		}
		found = &cache.tableSchemas[index]
	}
	if found == nil {
		return nil, fmt.Errorf("unknown table %s:%s", selector.name, selector.rowType)
	}
	return found, nil
}

func (cache *managerCache) resourcesForTables(managerName string, selectors []tableSelector, assetPaths []string) (*managerResources, error) {
	cache.mu.Lock()
	defer cache.mu.Unlock()
	schemas := make([]*tableSchema, 0, len(selectors))
	for _, selector := range selectors {
		schema, err := cache.tableSchema(selector)
		if err != nil {
			return nil, fmt.Errorf("manager %s: %w", managerName, err)
		}
		schemas = append(schemas, schema)
	}
	return cache.resourcesFromSchemas(managerName, schemas, assetPaths)
}

func (cache *managerCache) resourcesForRows(managerName string, rowTypes []string, assetPaths []string) (*managerResources, error) {
	cache.mu.Lock()
	defer cache.mu.Unlock()
	requested := make(map[string]struct{}, len(rowTypes))
	missing := make(map[string]struct{}, len(rowTypes))
	for _, rowType := range rowTypes {
		requested[rowType] = struct{}{}
		missing[rowType] = struct{}{}
	}
	schemas := make([]*tableSchema, 0)
	for index := range cache.tableSchemas {
		schema := &cache.tableSchemas[index]
		if _, ok := requested[schema.RowType]; !ok {
			continue
		}
		schemas = append(schemas, schema)
		delete(missing, schema.RowType)
	}
	if len(missing) != 0 {
		missingRows := make([]string, 0, len(missing))
		for rowType := range missing {
			missingRows = append(missingRows, rowType)
		}
		sort.Strings(missingRows)
		return nil, fmt.Errorf("manager %s uses unknown row types %s", managerName, strings.Join(missingRows, ", "))
	}
	return cache.resourcesFromSchemas(managerName, schemas, assetPaths)
}

func (cache *managerCache) resourcesFromSchemas(managerName string, schemas []*tableSchema, assetPaths []string) (*managerResources, error) {
	resources := &managerResources{
		managerName: managerName,
		tables:      map[tableSelector]*dynamicTable{},
		assets:      map[string][]byte{},
	}
	for _, schema := range schemas {
		table, err := cache.buildTable(schema)
		if err != nil {
			return nil, err
		}
		resources.tables[tableSelector{name: schema.Name, rowType: schema.RowType}] = table
		resources.tableOrder = append(resources.tableOrder, table)
	}
	for _, path := range assetPaths {
		bytes, err := cache.requiredAssetBytes(path)
		if err != nil {
			return nil, fmt.Errorf("manager %s: %w", managerName, err)
		}
		resources.assets[normalizeDataPath(path)] = bytes
	}
	return resources, nil
}

func (cache *managerCache) buildTable(schema *tableSchema) (*dynamicTable, error) {
	cacheKey := schema.Name + ":" + schema.RowType
	if cached := cache.tableCache[cacheKey]; cached != nil {
		return cached, nil
	}

	var rowKeyColumn *columnSchema
	for i := range schema.Columns {
		if schema.Columns[i].RowKey {
			rowKeyColumn = &schema.Columns[i]
			break
		}
	}
	table := &dynamicTable{
		Schema:     *schema,
		Rows:       []dynamicTableRow{},
		ColumnCRCs: map[string]uint32{},
	}
	for _, column := range schema.Columns {
		table.ColumnCRCs[column.Name] = column.CRC
	}

	for _, sourcePath := range schema.Sources {
		bytes, err := cache.requiredAssetBytes(sourcePath)
		if err != nil {
			return nil, err
		}
		sheet, err := gameassets.ParseDatasheet(bytes)
		if err != nil {
			return nil, err
		}
		if rowKeyColumn == nil {
			if len(sheet.Rows) != 0 {
				return nil, fmt.Errorf("non-empty datasheet source %s has no row-key column", sourcePath)
			}
			continue
		}
		columnSlots := columnSlotsForSheet(schema, &sheet)
		rowKeySlot, ok := columnSlots[rowKeyColumn.CRC]
		if !ok {
			return nil, fmt.Errorf("datasheet source %s missing row-key column %s", sourcePath, rowKeyColumn.Name)
		}
		for rowIndex, row := range sheet.Rows {
			keyCell := row.Cells[rowKeySlot]
			key, ok := rowKeyValue(keyCell.Value)
			if !ok {
				continue
			}
			dynamicRow := dynamicTableRow{
				SourcePath:  normalizeDataPath(sourcePath),
				RowIndex:    rowIndex,
				Key:         key,
				Row:         row,
				ColumnSlots: columnSlots,
			}
			table.Rows = append(table.Rows, dynamicRow)
		}
	}

	cache.tableCache[cacheKey] = table
	return table, nil
}

func columnSlotsForSheet(schema *tableSchema, sheet *gameassets.Datasheet) map[uint32]int {
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

func (cache *managerCache) assetBytes(path string) ([]byte, bool) {
	normalized := normalizeDataPath(path)
	if bytes, ok := cache.assetsByPath[normalized]; ok {
		return bytes, true
	}
	suffix := "/" + normalized
	for candidate, bytes := range cache.assetsByPath {
		if strings.HasSuffix(candidate, suffix) {
			return bytes, true
		}
	}
	return nil, false
}

func (cache *managerCache) requiredAssetBytes(path string) ([]byte, error) {
	bytes, ok := cache.assetBytes(path)
	if ok {
		return bytes, nil
	}
	bytes, err := cache.loader.Read(path)
	if err != nil {
		return nil, fmt.Errorf("read asset %s: %w", path, err)
	}
	cache.assetsByPath[normalizeDataPath(path)] = bytes
	return bytes, nil
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

func normalizeDataPath(path string) string {
	path = strings.ReplaceAll(path, "\\", "/")
	for strings.Contains(path, "//") {
		path = strings.ReplaceAll(path, "//", "/")
	}
	return strings.ToLower(path)
}

func tablePathMatches(left string, right string) bool {
	left = normalizeDataPath(left)
	right = normalizeDataPath(right)
	return left == right || strings.HasSuffix(left, "/"+right) || strings.HasSuffix(right, "/"+left)
}
"#;
