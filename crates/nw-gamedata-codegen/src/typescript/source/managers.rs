use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use base64::Engine;
use nw_datasheet::ColumnType;
use oxc_ast::ast::{
    IdentifierReference, ImportDeclarationSpecifier, ImportOrExportKind, Statement,
};
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;

use crate::compiler::GameDataCompileUnit;
use crate::emit::GameDataCodegenFile;
use crate::manager::{NativeManagerProductKind, NativeManagerShape};
use crate::manager_records::{
    CompositionManagerKind, CompositionManagerSurface, DirectManagerSurface, DirectManagerTable,
    ItemDataManagerSurface, ManagerSurface, SemanticLookupKind, SemanticManagerKey,
    SemanticManagerRecord, SemanticNumericKeyType, SemanticProjectionTransform,
    SemanticRowFilterPredicate, default_direct_manager_row_type, manager_accessor_domain,
    manager_surface_name, manager_surfaces, semantic_enum_default_variant, semantic_enum_type_name,
    semantic_manager_record_unit, ts_field_name, ts_method_name,
};
use crate::naming::to_upper_camel_ident;
use crate::typescript::source::{format_typescript_source, typescript_string_literal};
use nw_serialize_codegen::{
    TypeScriptSourceEmitter as SerializeTypeScriptSourceEmitter,
    TypeScriptSourceOptions as SerializeTypeScriptSourceOptions,
};

mod native;

pub(super) fn emit_dynamic_manager_files(
    unit: &GameDataCompileUnit,
) -> Result<Vec<GameDataCodegenFile>> {
    let surfaces = manager_surfaces(unit)?;
    let records = semantic_records(&surfaces);
    let manager_source = manager_index_source(unit, &surfaces)?;
    let mut files = split_typescript_declaration_source(
        &manager_source,
        "",
        "src/managers/index.ts",
        (!records.is_empty()).then_some("./types.js"),
    )?;
    files.extend(datasheet_catalog_files(unit)?);
    if !records.is_empty() {
        files.extend(split_typescript_declaration_source(
            &manager_record_types_source(&records)?,
            "type-",
            "src/managers/types.ts",
            None,
        )?);
    }
    Ok(files)
}

#[derive(Debug, Clone)]
struct TypeScriptManagerStatement {
    source: String,
    name: String,
    public: bool,
    type_only: bool,
}

#[derive(Debug, Clone)]
struct TypeScriptExternalImport {
    module: String,
    imported: String,
    type_only: bool,
}

#[derive(Debug, Default)]
struct TypeScriptImportGroup {
    values: BTreeSet<String>,
    types: BTreeSet<String>,
}

#[derive(Debug, Default)]
struct TypeScriptReferenceCollector {
    names: BTreeSet<String>,
}

impl<'a> Visit<'a> for TypeScriptReferenceCollector {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.names.insert(identifier.name.to_string());
    }
}

fn split_typescript_declaration_source(
    source: &str,
    chunk_prefix: &str,
    index_path: &str,
    extra_export: Option<&str>,
) -> Result<Vec<GameDataCodegenFile>> {
    const TARGET_LINES: usize = 700;

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::ts()).parse();
    if !parsed.errors.is_empty() {
        anyhow::bail!(
            "parse formatted TypeScript manager source: {}",
            parsed
                .errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
    }

    let mut external_imports = BTreeMap::new();
    let mut statements = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                let module = import.source.value.to_string();
                let declaration_type_only = import.import_kind == ImportOrExportKind::Type;
                for specifier in import.specifiers.iter().flatten() {
                    let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                        anyhow::bail!(
                            "standalone manager splitting requires named TypeScript imports"
                        );
                    };
                    let local = specifier.local.name.to_string();
                    external_imports.insert(
                        local,
                        TypeScriptExternalImport {
                            module: module.clone(),
                            imported: specifier.imported.name().to_string(),
                            type_only: declaration_type_only
                                || specifier.import_kind == ImportOrExportKind::Type,
                        },
                    );
                }
            }
            _ => {
                let span = statement.span();
                let statement_source = &source[span.start as usize..span.end as usize];
                let Some((name, public, type_only)) =
                    typescript_declaration_header(statement_source)
                else {
                    anyhow::bail!(
                        "unsupported top-level TypeScript manager statement: {}",
                        statement_source.lines().next().unwrap_or_default()
                    );
                };
                statements.push(TypeScriptManagerStatement {
                    source: statement_source.to_owned(),
                    name,
                    public,
                    type_only,
                });
            }
        }
    }

    let mut chunks = Vec::<Vec<usize>>::new();
    for (index, statement) in statements.iter().enumerate() {
        if statement.name == "CREATE_MANAGER" {
            chunks.push(vec![index]);
        }
    }
    let mut current = Vec::<usize>::new();
    let mut current_lines = 0usize;
    for (index, statement) in statements.iter().enumerate() {
        if statement.name == "CREATE_MANAGER" {
            continue;
        }
        let lines = statement.source.lines().count() + 2;
        let same_declaration = current
            .last()
            .is_some_and(|previous| statements[*previous].name == statement.name);
        if !current.is_empty() && current_lines + lines > TARGET_LINES && !same_declaration {
            chunks.push(std::mem::take(&mut current));
            current_lines = 0;
        }
        current.push(index);
        current_lines += lines;
    }
    if !current.is_empty() {
        chunks.push(current);
    }

    let mut statement_chunks = vec![0usize; statements.len()];
    let mut chunk_names = Vec::with_capacity(chunks.len());
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        for statement in chunk {
            statement_chunks[*statement] = chunk_index;
        }
        let anchor = chunk
            .iter()
            .find_map(|index| {
                statements[*index]
                    .public
                    .then_some(&statements[*index].name)
            })
            .unwrap_or(&statements[chunk[0]].name);
        chunk_names.push(format!(
            "{chunk_prefix}{chunk_index:03}-{}.ts",
            crate::naming::to_snake_ident(anchor, "part").replace('_', "-")
        ));
    }

    let mut declaration_kinds = BTreeMap::<String, bool>::new();
    for statement in &statements {
        declaration_kinds
            .entry(statement.name.clone())
            .and_modify(|type_only| *type_only &= statement.type_only)
            .or_insert(statement.type_only);
    }

    let declaration_chunks = statements
        .iter()
        .enumerate()
        .map(|(index, statement)| (statement.name.clone(), statement_chunks[index]))
        .collect::<BTreeMap<_, _>>();
    let mut imports = (0..chunks.len())
        .map(|_| BTreeMap::<String, TypeScriptImportGroup>::new())
        .collect::<Vec<_>>();

    for (statement_index, statement) in parsed
        .program
        .body
        .iter()
        .filter(|statement| !matches!(statement, Statement::ImportDeclaration(_)))
        .enumerate()
    {
        let reference_chunk = statement_chunks[statement_index];
        let mut references = TypeScriptReferenceCollector::default();
        references.visit_statement(statement);
        for symbol_name in references.names {
            let external = external_imports.get(&symbol_name);
            let owner_chunk = declaration_chunks.get(&symbol_name).copied();
            if (external.is_none() && owner_chunk.is_none()) || owner_chunk == Some(reference_chunk)
            {
                continue;
            }
            let (module, imported, type_only) = if let Some(external) = external {
                (
                    external.module.clone(),
                    external.imported.clone(),
                    external.type_only,
                )
            } else {
                let owner_chunk = owner_chunk.expect("internal symbol has an owner chunk");
                (
                    format!("./{}", chunk_names[owner_chunk].replace(".ts", ".js")),
                    symbol_name.clone(),
                    *declaration_kinds.get(&symbol_name).unwrap_or(&false),
                )
            };
            let group = imports[reference_chunk].entry(module).or_default();
            if type_only {
                group.types.insert(imported);
            } else {
                group.values.insert(imported.clone());
                group.types.remove(&imported);
            }
        }
    }

    let mut files = Vec::with_capacity(chunks.len() + 1);
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        let mut chunk_source = typescript_imports_source(&imports[chunk_index]);
        for statement_index in chunk {
            let statement = &statements[*statement_index];
            if statement.source.trim_start().starts_with("export ") {
                chunk_source.push_str(&statement.source);
            } else {
                chunk_source.push_str("export ");
                chunk_source.push_str(&statement.source);
            }
            chunk_source.push_str("\n\n");
        }
        files.push(GameDataCodegenFile::new(
            format!("src/managers/{}", chunk_names[chunk_index]),
            format_typescript_source(&chunk_source)?,
        ));
    }

    let mut exports = BTreeMap::<String, TypeScriptImportGroup>::new();
    for (statement_index, statement) in statements.iter().enumerate() {
        if !statement.public {
            continue;
        }
        let module = format!(
            "./{}",
            chunk_names[statement_chunks[statement_index]].replace(".ts", ".js")
        );
        let group = exports.entry(module).or_default();
        if statement.type_only {
            if !group.values.contains(&statement.name) {
                group.types.insert(statement.name.clone());
            }
        } else {
            group.values.insert(statement.name.clone());
            group.types.remove(&statement.name);
        }
    }
    let mut index_source = typescript_exports_source(&exports);
    if let Some(module) = extra_export {
        index_source.push_str(&format!(
            "export * from {};\n",
            typescript_string_literal(module)
        ));
    }
    files.push(GameDataCodegenFile::new(
        index_path,
        format_typescript_source(&index_source)?,
    ));
    Ok(files)
}

fn typescript_declaration_header(source: &str) -> Option<(String, bool, bool)> {
    let mut source = source.trim_start();
    let public = if let Some(rest) = source.strip_prefix("export ") {
        source = rest.trim_start();
        true
    } else {
        false
    };
    if let Some(rest) = source.strip_prefix("declare ") {
        source = rest.trim_start();
    }
    let (rest, type_only) = [
        ("interface ", true),
        ("type ", true),
        ("enum ", false),
        ("class ", false),
        ("function ", false),
        ("function* ", false),
        ("async function ", false),
        ("const ", false),
        ("let ", false),
        ("var ", false),
    ]
    .into_iter()
    .find_map(|(prefix, type_only)| source.strip_prefix(prefix).map(|rest| (rest, type_only)))?;
    let name = rest
        .trim_start()
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        .collect::<String>();
    (!name.is_empty()).then_some((name, public, type_only))
}

fn typescript_imports_source(imports: &BTreeMap<String, TypeScriptImportGroup>) -> String {
    let mut source = String::new();
    for (module, group) in imports {
        if !group.values.is_empty() {
            source.push_str(&format!(
                "import {{ {} }} from {};\n",
                group.values.iter().cloned().collect::<Vec<_>>().join(", "),
                typescript_string_literal(module)
            ));
        }
        let types = group
            .types
            .difference(&group.values)
            .cloned()
            .collect::<Vec<_>>();
        if !types.is_empty() {
            source.push_str(&format!(
                "import type {{ {} }} from {};\n",
                types.join(", "),
                typescript_string_literal(module)
            ));
        }
    }
    source.push('\n');
    source
}

fn typescript_exports_source(exports: &BTreeMap<String, TypeScriptImportGroup>) -> String {
    let mut source = String::new();
    for (module, group) in exports {
        if !group.values.is_empty() {
            source.push_str(&format!(
                "export {{ {} }} from {};\n",
                group.values.iter().cloned().collect::<Vec<_>>().join(", "),
                typescript_string_literal(module)
            ));
        }
        let types = group
            .types
            .difference(&group.values)
            .cloned()
            .collect::<Vec<_>>();
        if !types.is_empty() {
            source.push_str(&format!(
                "export type {{ {} }} from {};\n",
                types.join(", "),
                typescript_string_literal(module)
            ));
        }
    }
    source
}

fn manager_index_source(unit: &GameDataCompileUnit, surfaces: &[ManagerSurface]) -> Result<String> {
    let mut source = String::from(
        r#"
import { parseDatasheet, type Datasheet, type DatasheetCellValue, type DatasheetRow } from "../game-assets/datasheet.js";
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
} from "../game-assets/object-stream.js";
import { type AssetLoader } from "../game-assets/loader.js";
import { AssetId, Crc32, Uuid, Vector3, type AssetReference } from "../values.js";
import { loadTableSchemas, type TableSchema } from "./datasheet-catalog.js";

"#,
    );
    let records = semantic_records(surfaces);
    if !records.is_empty() {
        let mut imports = semantic_enum_shapes(surfaces)
            .into_iter()
            .map(|shape| shape.name)
            .collect::<Vec<_>>();
        imports.extend(
            records
                .iter()
                .map(|record| format!("type {}", record.record_type_name)),
        );
        source.push_str(&format!(
            "import {{ {} }} from \"./types.js\";\n\n",
            imports.join(", ")
        ));
    }
    source.push_str(
        r#"
export interface Rows<Row> extends Iterable<Row> {
  rows(): IterableIterator<Row>;
}

export interface RowLookup<Key, Row> extends Rows<Row> {
  get(key: Key): Row | undefined;
}

declare const ROW_TYPE: unique symbol;

export interface RowRef<Table extends string, Row> {
  readonly table: Table;
  readonly key: string;
  readonly [ROW_TYPE]?: Row;
}

export interface RowSlot<Table extends string, Row> {
  readonly table: Table;
  readonly rowIndex: number;
  readonly [ROW_TYPE]?: Row;
}

export interface TableReference {
  readonly path: string;
  readonly key: string;
}

export interface RowEntry<Table extends string, Row> {
  readonly ref: RowRef<Table, Row>;
  readonly slot: RowSlot<Table, Row>;
  readonly row: Row;
}

interface ResolvedRowEntry<Row> {
  readonly sourcePath: string;
  readonly key: string;
  readonly rowIndex: number;
  readonly row: Row;
}

export interface RowCollection<Row, Table extends string> extends Rows<RowEntry<Table, Row>> {
  readonly length: number;
  readonly empty: boolean;
  table(table: Table): TableRows<Row, Table>;
  get(ref: RowRef<Table, Row>): Row | undefined;
  rowByIndex(slot: RowSlot<Table, Row>): Row | undefined;
  rowKeyByIndex(slot: RowSlot<Table, Row>): string | undefined;
}

interface RowTableIndex<Table extends string, Row> {
  readonly entries: RowEntry<Table, Row>[];
  readonly byKey: Map<string, RowEntry<Table, Row>>;
  readonly byRowIndex: Map<number, RowEntry<Table, Row>>;
}

class RowCollectionImpl<Row, Table extends string> implements RowCollection<Row, Table> {
  private readonly entriesByTable = new Map<string, RowTableIndex<Table, Row>>();
  private readonly entries: RowEntry<Table, Row>[] = [];

  constructor(
    resolvedEntries: readonly ResolvedRowEntry<Row>[],
    tableSources: ReadonlyMap<Table, readonly string[]>,
  ) {
    for (const [table, sources] of tableSources) {
      const alias = normalizeDataPath(table);
      const index = { entries: [], byKey: new Map(), byRowIndex: new Map() } as RowTableIndex<Table, Row>;
      for (const resolved of resolvedEntries) {
        if (!sources.some((source) => tablePathMatches(resolved.sourcePath, source))) continue;
        const entry: RowEntry<Table, Row> = {
          ref: { table, key: resolved.key },
          slot: { table, rowIndex: resolved.rowIndex },
          row: resolved.row,
        };
        this.entries.push(entry);
        index.entries.push(entry);
        const key = normalizeLookupKey(resolved.key);
        if (!index.byKey.has(key)) index.byKey.set(key, entry);
        if (!index.byRowIndex.has(resolved.rowIndex)) index.byRowIndex.set(resolved.rowIndex, entry);
      }
      this.entriesByTable.set(alias, index);
    }
  }

  get length(): number {
    return this.entries.length;
  }

  get empty(): boolean {
    return this.entries.length === 0;
  }

  rows(): IterableIterator<RowEntry<Table, Row>> {
    return this.entries.values();
  }

  table(table: Table): TableRows<Row, Table> {
    return new TableRowsImpl(this, table);
  }

  get(ref: RowRef<Table, Row>): Row | undefined {
    return this.tableIndex(ref.table)?.byKey.get(normalizeLookupKey(ref.key))?.row;
  }

  rowByIndex(slot: RowSlot<Table, Row>): Row | undefined {
    return this.tableIndex(slot.table)?.byRowIndex.get(slot.rowIndex)?.row;
  }

  rowKeyByIndex(slot: RowSlot<Table, Row>): string | undefined {
    return this.tableIndex(slot.table)?.byRowIndex.get(slot.rowIndex)?.ref.key;
  }

  entriesForTable(table: Table): readonly RowEntry<Table, Row>[] {
    return this.tableIndex(table)?.entries ?? [];
  }

  [Symbol.iterator](): Iterator<RowEntry<Table, Row>> {
    return this.rows();
  }

  private tableIndex(table: Table): RowTableIndex<Table, Row> | undefined {
    const normalized = normalizeDataPath(table);
    const exact = this.entriesByTable.get(normalized);
    if (exact !== undefined) {
      return exact;
    }
    for (const [candidate, index] of this.entriesByTable) {
      if (tablePathMatches(candidate, normalized)) {
        return index;
      }
    }
    return undefined;
  }
}

export interface TableRows<Row, Table extends string> extends Rows<RowEntry<Table, Row>> {
  readonly table: Table;
  get(key: string): Row | undefined;
  rowByIndex(rowIndex: number): Row | undefined;
  rowKeyByIndex(rowIndex: number): string | undefined;
}

class TableRowsImpl<Row, Table extends string> implements TableRows<Row, Table> {
  constructor(
    private readonly collection: RowCollectionImpl<Row, Table>,
    readonly table: Table,
  ) {}

  *rows(): IterableIterator<RowEntry<Table, Row>> {
    yield* this.collection.entriesForTable(this.table);
  }

  get(key: string): Row | undefined {
    return this.collection.get({ table: this.table, key });
  }

  rowByIndex(rowIndex: number): Row | undefined {
    return this.collection.rowByIndex({ table: this.table, rowIndex });
  }

  rowKeyByIndex(rowIndex: number): string | undefined {
    return this.collection.rowKeyByIndex({ table: this.table, rowIndex });
  }

  [Symbol.iterator](): Iterator<RowEntry<Table, Row>> {
    return this.rows();
  }
}

const CREATE_MANAGER = Symbol("createManager");

interface DynamicTableRow {
  readonly sourcePath: string;
  readonly rowIndex: number;
  readonly key: string;
  readonly row: DatasheetRow;
  readonly columnSlots: ReadonlyMap<number, number>;
}

interface DynamicTable {
  readonly schema: TableSchema;
  readonly rows: readonly DynamicTableRow[];
  readonly columnCrcs: ReadonlyMap<string, number>;
}

"#,
    );

    let readable_row_types = direct_schema_row_types(surfaces);
    push_schema_row_types(&mut source, unit, &readable_row_types);
    push_ts_enum_parsers(&mut source, surfaces);
    push_direct_row_family_types(&mut source, unit, surfaces);
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
            | ManagerSurface::Native { .. }
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_)
            | ManagerSurface::ProductBacked(_) => None,
        })
        .collect()
}

fn semantic_enum_shapes(
    surfaces: &[ManagerSurface],
) -> Vec<crate::game_system_schema::GameSystemEnumShape> {
    let mut shapes = BTreeMap::new();
    for shape in surfaces
        .iter()
        .filter_map(|surface| match surface {
            ManagerSurface::Semantic(record) => Some(record),
            ManagerSurface::Direct(_)
            | ManagerSurface::Native { .. }
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_)
            | ManagerSurface::ProductBacked(_) => None,
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
        .filter_map(|surface| match surface {
            ManagerSurface::Semantic(record) => Some(record),
            ManagerSurface::Direct(_)
            | ManagerSurface::Native { .. }
            | ManagerSurface::ItemData(_)
            | ManagerSurface::Composition(_)
            | ManagerSurface::ProductBacked(_) => None,
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

fn push_ts_enum_parsers(source: &mut String, surfaces: &[ManagerSurface]) {
    for shape in semantic_enum_shapes(surfaces) {
        let parser = format!("parse{}", shape.name);
        source.push_str(&format!(
            "function {parser}(source: string): {} {{\n  switch (source.trim()) {{\n",
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
                "    case {}:\n      return {}.{variant};\n",
                typescript_string_literal(&token),
                shape.name
            ));
        }
        source.push_str(&format!(
            "    default:\n      throw new Error(`unknown {} value ${{source}}`);\n  }}\n}}\n\n",
            shape.name
        ));
    }
    for shape in semantic_pair_first_enum_shapes(surfaces) {
        let parser = ts_pair_enum_parser_name(&shape.name);
        source.push_str(&format!(
            "function {parser}(source: string): number {{\n  switch (source.trim()) {{\n"
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
                "    case {}:\n      return {discriminant};\n",
                typescript_string_literal(&token)
            ));
        }
        source.push_str(&format!(
            "    default: {{\n      const value = Number(source);\n      if (Number.isInteger(value) && value >= 0 && value <= 0xff) return value;\n      throw new Error(`unknown {} value ${{source}}`);\n    }}\n  }}\n}}\n\n",
            shape.name
        ));
    }
}

fn ts_pair_enum_parser_name(enum_name: &str) -> String {
    format!("parse{enum_name}Discriminant")
}

fn direct_schema_row_types(surfaces: &[ManagerSurface]) -> BTreeSet<String> {
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

fn ts_manager_accessor_name(manager_name: &str) -> String {
    ts_method_name(manager_accessor_domain(manager_name))
}

fn ts_manager_dependency_name(manager_name: &str) -> String {
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
  readonly entries: readonly LootBucketDataSlotEntry[];
  readonly lootBiasingDisabled: readonly LootBucketBiasingDisabled[];
}

export interface LootBucketDataSlotEntry {
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
  const entries: LootBucketDataSlotEntry[] = [];
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
    use nw_datasheet::game_system::Crc32;

    use crate::game_system_schema::{
        GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemDataTablesSchemaReport,
        GameSystemTableSchema,
    };
    use crate::manager_records::{DirectManagerTable, ItemDataManagerTable, SemanticLookupMethod};
    use crate::plan::GameDataCodegenPlan;
    use crate::schema::GameDataCompileMode;

    use super::*;

    #[test]
    fn semantic_resources_use_exact_table_schema_identity() {
        let expression = ts_manager_instance_expression(
            "ExampleManager",
            [("SharedTable", "ExampleRow")],
            std::iter::empty(),
        );

        assert!(expression.contains("cache.resourcesForTables("));
        assert!(expression.contains("{ name: \"SharedTable\", rowType: \"ExampleRow\" }"));
        assert!(!expression.contains("cache.resources("));
    }

    #[test]
    fn skip_empty_semantic_keys_accept_missing_cells() {
        let mut source = String::new();
        push_ts_key_materializer(&mut source, &semantic_lookup_record());

        assert!(source.contains("optionalStringCell"));
        assert!(source.contains("keyText === null"));
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
    fn ability_coordinates_are_filtering_parsers() {
        let helper = SEMANTIC_MANAGER_RUNTIME_TS
            .split("function abilityCoordinate")
            .nth(1)
            .expect("ability coordinate helper")
            .split("function tableNumberLookupKey")
            .next()
            .expect("ability coordinate body");

        assert!(helper.contains("optionalSchemaNumber(value)"));
        assert!(helper.contains("return null"));
        assert!(!helper.contains("requiredSchemaNumber"));
        assert!(!helper.contains("throw new RangeError"));
    }

    #[test]
    fn direct_schema_manager_uses_rows_contract_for_primary_row_type() {
        let unit = damage_compile_unit();
        let manager = damage_manager_surface();
        let rows_interface = direct_ts_rows_interface(&unit, &manager);
        let methods = direct_ts_schema_methods(&unit, &manager);
        let resources = ts_direct_manager_instance_expression(&manager);

        assert!(resources.contains("cache.resourcesForRows"));
        assert!(resources.contains("\"AfflictionData\""));
        assert!(resources.contains("\"DamageTypeData\""));
        assert_eq!(
            rows_interface,
            " implements Rows<RowEntry<DamageDataTable, DamageDataSchemaRow>>"
        );
        assert!(
            methods.contains(
                "rows(): IterableIterator<RowEntry<DamageDataTable, DamageDataSchemaRow>>"
            )
        );
        assert!(methods.contains(
            "table(table: DamageDataTable): TableRows<DamageDataSchemaRow, DamageDataTable>"
        ));
        assert!(!methods.contains("table(table: string)"));
        assert!(methods.contains(
            "row(ref: RowRef<DamageDataTable, DamageDataSchemaRow>): DamageDataSchemaRow | undefined"
        ));
        assert!(methods.contains(
            "rowByIndex(slot: RowSlot<DamageDataTable, DamageDataSchemaRow>): DamageDataSchemaRow | undefined"
        ));
        assert!(methods.contains(
            "[Symbol.iterator](): Iterator<RowEntry<DamageDataTable, DamageDataSchemaRow>>"
        ));
        assert!(methods.contains(
            "afflictionDataRows(): RowCollection<AfflictionDataSchemaRow, DamageDataAfflictionDataTable>"
        ));
        assert!(methods.contains(
            "damageTypeDataRows(): RowCollection<DamageTypeDataSchemaRow, DamageDataDamageTypeDataTable>"
        ));
        assert!(!methods.contains("afflictionData(): RowCollection"));
        assert!(!methods.contains("damageTypeData(): RowCollection"));
        assert!(
            !methods.contains("afflictionData(key: AfflictionDataSchemaRow[\"afflictionId\"])")
        );
        assert!(
            !methods.contains("damageTypeData(key: DamageTypeDataSchemaRow[\"damageTypeId\"])")
        );
        assert!(!methods.contains("get(key: DamageDataSchemaRow"));
        assert!(!methods.contains("damageDataRows(): readonly DamageDataSchemaRow[]"));
        assert!(!methods.contains("damageData(key: DamageDataSchemaRow"));
    }

    #[test]
    fn generic_direct_manager_emits_typed_table_selector() {
        let unit = damage_compile_unit();
        let manager = DirectManagerSurface {
            manager_name: "AfflictionDataManager".to_owned(),
            manager_class_name: "AfflictionDataManager".to_owned(),
            tables: vec![DirectManagerTable {
                table_name: "AfflictionData".to_owned(),
                row_type_name: "AfflictionData".to_owned(),
            }],
            products: Vec::new(),
        };
        let mut source = String::new();

        push_direct_manager_class(&mut source, &unit, &manager);

        assert!(source.contains("export type AfflictionDataTable = \"AfflictionData\";"));
        assert!(source.contains(
            "table(table: AfflictionDataTable): TableRows<AfflictionDataSchemaRow, AfflictionDataTable>"
        ));
        assert!(!source.contains("table(table: string)"));
        assert!(source.contains("row(ref: RowRef<AfflictionDataTable, AfflictionDataSchemaRow>)"));
        assert!(!source.contains("RowRef<AfflictionDataSchemaRow>"));
    }

    #[test]
    fn resolved_asset_paths_do_not_escape_through_public_row_refs() {
        assert!(DYNAMIC_MANAGER_RUNTIME_TS.contains("sourcePath: row.sourcePath"));
        assert!(!DYNAMIC_MANAGER_RUNTIME_TS.contains("ref: { table: row.sourcePath"));
        let source = manager_index_source(
            &damage_compile_unit(),
            &[ManagerSurface::Direct(damage_manager_surface())],
        )
        .expect("manager source");
        assert!(source.contains("export interface RowRef<"));
        assert!(source.contains("Table extends string"));
    }

    #[test]
    fn replication_manager_builds_reverse_index_once() {
        assert!(REPLICATION_DATA_MANAGER_TS.contains("private readonly indexesById"));
        assert!(REPLICATION_DATA_MANAGER_TS.contains("this.indexesById.set(id, index)"));
        assert!(REPLICATION_DATA_MANAGER_TS.contains("return this.indexesById.get(key) ?? 0"));
        assert!(!REPLICATION_DATA_MANAGER_TS.contains("this.idsCache.indexOf"));
    }

    #[test]
    fn static_tradeskill_mapping_is_materialized_from_dependencies() {
        assert!(
            STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_TS
                .contains("private readonly playerLevelsByDisplayName")
        );
        assert!(
            STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_TS
                .contains("private readonly tradeskillRanksByDisplayName")
        );
        assert!(
            STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_TS.contains("playerLevelForDisplayName")
        );
        assert!(
            STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_TS.contains("tradeskillRankForDisplayName")
        );
        assert!(!STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_TS.contains("_experience"));
    }

    #[test]
    fn experience_native_manager_materializes_lookup_indexes() {
        let augmentation = experience_manager_augmentation();

        assert!(augmentation.fields.contains("experienceByLevel"));
        assert!(
            augmentation
                .initializers
                .contains("gearScoreThresholds.sort")
        );
        assert!(augmentation.methods.contains("experienceDataFromId"));
        assert!(augmentation.methods.contains("levelForXp"));
        assert!(augmentation.methods.contains("maxLevel"));
    }

    #[test]
    fn item_data_manager_uses_rows_contract() {
        let mut source = String::new();
        push_item_data_manager_class(&mut source, &item_data_manager_surface());

        assert!(
            source.contains("export class ItemDataManager implements RowLookup<string, ItemData>")
        );
        assert!(source.contains("rows(): IterableIterator<ItemData>"));
        assert!(source.contains("[Symbol.iterator](): Iterator<ItemData>"));
        assert!(!source.contains("items(): readonly ItemData[]"));
    }

    #[test]
    fn semantic_into_crc_lookup_accepts_string_or_crc_key() {
        let mut source = String::new();
        push_semantic_manager_class(&mut source, &semantic_lookup_record());

        assert!(
            source.contains(
                "backstory(backstoryId: string | Crc32): StaticBackstoryData | undefined"
            )
        );
        assert!(source.contains("this.rowsByKey.get(crc32LookupKey(backstoryId))"));
        assert!(
            source
                .contains("backstoryByKey(backstoryKey: string): StaticBackstoryData | undefined")
        );
    }

    #[test]
    fn semantic_managers_emit_only_consumed_indexes() {
        let mut record = semantic_lookup_record();
        record.lookup_methods.clear();
        let mut source = String::new();

        push_semantic_manager_class(&mut source, &record);

        assert!(!source.contains("rowsByKey"));
        assert!(!source.contains("rowsBySourceRow"));
    }

    #[test]
    fn skip_invalid_enum_projection_continues_without_fabricating_a_variant() {
        let mut record = semantic_lookup_record();
        record.fields.push(skip_invalid_enum_field());
        let mut source = String::new();

        push_semantic_materializer(&mut source, &record);

        assert!(source.contains("let missionGoalTypeValue: MissionGoalType;"));
        assert!(source.contains("missionGoalTypeValue = parseMissionGoalType"));
        assert!(source.contains("catch {\n        continue;"));
        assert!(!source.contains("MissionGoalType.Invalid"));
    }

    #[test]
    fn every_native_manager_has_an_explicit_typescript_contract() {
        let managers = crate::manager::validated_native_manager_specs();
        let surfaces = crate::manager_records::manager_surfaces_from_managers(&managers)
            .expect("manager surfaces");
        let mut seen = BTreeSet::new();
        let tables = surfaces
            .iter()
            .flat_map(|surface| match surface {
                ManagerSurface::Native { manager, shape, .. } => {
                    ts_effective_native_manager_surface(manager, shape).tables
                }
                _ => Vec::new(),
            })
            .filter(|table| seen.insert((table.table_name.clone(), table.row_type_name.clone())))
            .map(|table| {
                schema_table(
                    &table.table_name,
                    &table.row_type_name,
                    native_contract_columns(),
                )
            })
            .collect::<Vec<_>>();
        let schema_report = GameSystemDataTablesSchemaReport {
            tables,
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        };
        let plan = GameDataCodegenPlan::from_schema_report(
            GameDataCompileMode::SourceFormat,
            &schema_report,
        );
        let unit = GameDataCompileUnit::new(schema_report.clone(), schema_report, plan);

        let mut covered = 0usize;
        for surface in &surfaces {
            let ManagerSurface::Native { manager, shape, .. } = surface else {
                continue;
            };
            let effective = ts_effective_native_manager_surface(manager, shape);
            let augmentation = ts_native_manager_augmentation(&unit, &effective, shape);
            assert!(
                !augmentation.fields.is_empty() || !augmentation.methods.is_empty(),
                "{} emitted an empty native contract",
                manager.manager_name
            );
            assert!(
                !augmentation.initializers.is_empty() || effective.tables.is_empty(),
                "{} did not build indexes during construction",
                manager.manager_name
            );
            let contract = format!(
                "{}{}{}",
                augmentation.declarations, augmentation.fields, augmentation.methods
            );
            let marker = native_contract_marker(shape);
            assert!(
                contract.contains(marker),
                "{} omitted native contract marker {marker}",
                manager.manager_name
            );
            assert!(
                !contract.contains("table: string"),
                "{} leaked an untyped table",
                manager.manager_name
            );
            assert!(
                !contract.contains("table(table: string)"),
                "{} leaked stringly table selection",
                manager.manager_name
            );
            assert!(
                !augmentation.initializers.contains("source.ref.key"),
                "{} used the generic row key",
                manager.manager_name
            );
            let emitted_contract = format!(
                "{}{}{}{}",
                augmentation.declarations,
                augmentation.fields,
                augmentation.initializers,
                augmentation.methods
            );
            let emitted_contract_lower = emitted_contract.to_ascii_lowercase();
            for banned in ["native", "runtime", "context", "loadpak"] {
                assert!(
                    !emitted_contract_lower.contains(banned),
                    "{} leaked banned generated vocabulary `{banned}`",
                    manager.manager_name
                );
            }
            if semantic_rows_shape(shape) {
                assert!(
                    augmentation.rows_interface.is_some() && augmentation.row_methods.is_some(),
                    "{} did not expose its semantic DTO through Rows and iteration",
                    manager.manager_name
                );
            }
            covered += 1;
        }
        assert!(
            covered >= 50,
            "expected the complete native manager inventory"
        );
    }

    fn native_contract_columns() -> Vec<GameSystemColumnSchema> {
        const STRING_COLUMNS: &[&str] = &[
            "AbilityID",
            "TreeID",
            "TreeRowPosition",
            "AffixID",
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
            "BalanceTarget",
            "BalanceCategory",
            "AbilityBaseDamageAdjustment",
            "AffixStatAdjustment",
            "IncomingHealAdjustment",
            "ConsumableHealAdjustment",
            "Chapter",
            "ChapterID",
            "ChapterRewardID",
            "ChapterType",
            "ContainerTypeID",
            "ContributionID",
            "ConversionID",
            "CostumeChangeID",
            "CostumeChangeMesh",
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
            "FootprintID",
            "FromItemID",
            "GameEventIDRankAmazing",
            "GameEventIDRankBad",
            "GameEventIDRankGreat",
            "GameEventIDRankOkay",
            "GameModeIds",
            "GatherableID",
            "GatheringAction",
            "GatheringType",
            "Ingredient1",
            "Ingredient2",
            "Ingredient3",
            "Ingredient4",
            "Ingredient5",
            "Ingredient6",
            "Ingredient7",
            "Instrument",
            "ItemID",
            "ItemIds",
            "JourneyTaskID",
            "MountID",
            "TimedRaceNodeTypeId",
            "ObjectiveID",
            "Pages",
            "PrefabPath",
            "ProfileName",
            "ProfileType",
            "ProgressionPointID",
            "Promotion1",
            "Promotion2",
            "Promotion3",
            "PromotionMutationID",
            "QuickCourseID",
            "PathReferenceQuickCourseID",
            "RecipeID",
            "RewardID",
            "RewardID1",
            "Reward(s)",
            "RewardType",
            "RotationalQueueID",
            "RuleID",
            "Hub",
            "Zone",
            "Tags",
            "SheetID",
            "Slot01",
            "Slot02",
            "Slot03",
            "Slot04",
            "Slot05",
            "SongID",
            "StatusEffect_1",
            "StatusEffect_2",
            "StatusEffect_3",
            "StatusEffect_4",
            "StatusEffect_5",
            "StatusID",
            "StoreCategory",
            "StructurePieceID",
            "TaskID",
            "TerritoryName",
            "TrackedStatID",
            "TradeSkillType",
            "UniqueTagID",
            "VitalsID",
            "WeaponName",
            "WhisperID",
            "WhisperVfxID",
            "WorldEncounterID",
            "ActivitiesTaskID",
            "ReusableScoreboardTabId",
            "TableType",
            "CraftingCategory",
            "ToItemID",
            "FeatureID",
            "Tag1",
            "MatchOne1",
            "Type1",
            "ExcludeTypeStage1",
            "ExcludeTypeShop1",
            "Mesh",
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
        ];
        const NUMBER_COLUMNS: &[&str] = &[
            "Index",
            "EntitlementIndex",
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
            "SortOrder",
            "MaxRoll",
            "OutputQty",
            "Qty1",
            "Qty2",
            "Qty3",
            "Qty4",
            "Qty5",
            "Qty6",
            "Qty7",
            "TerritoryID",
            "UIPriority",
            "MaxEvents",
            "MinDistance",
            "StartingTimerSeconds",
            "NodeTimeOverrideMultiplier",
            "DetectionRadius",
            "AddTimeSeconds",
            "WeaponBaseDamageAdjustment",
            "SelfHealAdjustment",
            "CooldownAdjustment",
            "PotencyAdjustment",
            "DurationAdjustment",
            "RandomWeights1",
            "BudgetContribution1",
            "MeshRenderZPosOffset",
            "Quantity",
            "BuyCategoricalProgressionCost",
        ];
        const BOOLEAN_COLUMNS: &[&str] = &[
            "KeepPerks",
            "MatchesPlayerSkeleton",
            "Disabled",
            "IsTimed",
            "AccumulateTime",
            "UseTimeOverride",
            "IsEntitlement",
            "RollOnPresent",
            "UseLevelGS",
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

    fn semantic_rows_shape(shape: &NativeManagerShape) -> bool {
        matches!(
            shape,
            NativeManagerShape::AbilityData(_)
                | NativeManagerShape::DamageData(_)
                | NativeManagerShape::VitalsData(_)
                | NativeManagerShape::TradeskillRankData(_)
                | NativeManagerShape::OneTableWorldEventRule(_)
                | NativeManagerShape::QuickCourseData(_)
                | NativeManagerShape::DynamicDifficultyData(_)
                | NativeManagerShape::OneTablePvpBalance(_)
                | NativeManagerShape::OneTableDyeColor(_)
                | NativeManagerShape::RewardTrackData(_)
                | NativeManagerShape::OneTableCostumeChange(_)
                | NativeManagerShape::LootBucketData(_)
                | NativeManagerShape::BuffBucketData(_)
                | NativeManagerShape::ElementalMutationStaticData(_)
                | NativeManagerShape::PromotionMutationStaticData(_)
                | NativeManagerShape::ItemTransformData(_)
                | NativeManagerShape::GatherableData(_)
                | NativeManagerShape::RecipeData(_)
        )
    }

    fn native_contract_marker(shape: &NativeManagerShape) -> &'static str {
        match shape {
            NativeManagerShape::OneTableExperience(_) => "experienceDataFromId",
            NativeManagerShape::AbilityData(_) => "abilityDataFromId",
            NativeManagerShape::DamageData(_) => "damageById",
            NativeManagerShape::VitalsData(_) => "creatureTypeIds",
            NativeManagerShape::StatusEffectData(_) => "statusEffectDataFromId",
            NativeManagerShape::ItemConversionData(_) => "byId",
            NativeManagerShape::MultiTableCrcKeyProjection(_) => "affixDataFromId",
            NativeManagerShape::TradeskillRankData(_) => "tradeskillRank",
            NativeManagerShape::ObjectivesData(_) => "objectiveTaskDataFromId",
            NativeManagerShape::ContributionData(_) => "contributionDataByKey",
            NativeManagerShape::BuffBucketData(_) => "visitAllBuffsFromId",
            NativeManagerShape::StructureData(_) => "structurePieceDataFromId",
            NativeManagerShape::ReusableScoreboardData(_) => "reusableScoreboardDataFromId",
            NativeManagerShape::MountHitVolumeData(_) => "mountHitVolumeFromMountTypeId",
            NativeManagerShape::OneTableCampSkin(_) => "campSkinDataFromId",
            NativeManagerShape::OneTableEmote(_) => "emoteDataFromId",
            NativeManagerShape::OneTableStoreCategory(_) => "storeCategoryPropertiesFromId",
            NativeManagerShape::OneTableStoreProduct(_) => "storeProductDataFromId",
            NativeManagerShape::OneTableRewardTrackItem(_) => "rewardTrackItemFromId",
            NativeManagerShape::OneTableWorldEventRule(_) => "worldEventRuleByCrc32",
            NativeManagerShape::QuickCourseData(_) => "nodeTypeByCrc32",
            NativeManagerShape::RotationalQueueData(_) => "rotationalQueueFromId",
            NativeManagerShape::DynamicDifficultyData(_) => "DynamicDifficultyStatusEffectPotency",
            NativeManagerShape::ProgressionPointData(_) => "progressionPointFromId",
            NativeManagerShape::EntitlementData(_) => "entitlementsForReward",
            NativeManagerShape::EquipmentSetData(_) => "setsForPerk",
            NativeManagerShape::OneTablePvpBalance(_) => "balances",
            NativeManagerShape::OneTableDyeColor(_) => "dyeColorDataFromIndex",
            NativeManagerShape::RewardTrackData(_) => "RewardTrackSlot",
            NativeManagerShape::PostSkillCapProgression(_) => "postSkillCapProgressionDataFromId",
            NativeManagerShape::WhisperData(_) => "whisperVfxFromId",
            NativeManagerShape::OneTableCostumeChange(_) => "costumeChangeDataFromId",
            NativeManagerShape::OneTableCrestPart(_) => "crestPartDataFromIndex",
            NativeManagerShape::OneTableDungeonTile(_) => "dungeonTileStaticDataByKey",
            NativeManagerShape::OneTableLevelDisparity(_) => {
                "clampedLevelDisparityDataForLevelsWithPlayerLevelCap"
            }
            NativeManagerShape::OneTableEncumbrance(_) => "encumbranceDataFromId",
            NativeManagerShape::OneTableDifficultyScaling(_) => "difficultyScalingDataFromId",
            NativeManagerShape::OneTableDarkness(_) => "darknessDataByCrc32",
            NativeManagerShape::OneTableParticleData(_) => "particleDataFromId",
            NativeManagerShape::CharacterAttributeData(_) => "clampedAttributeData",
            NativeManagerShape::GovernanceData(_) => "governanceRows",
            NativeManagerShape::LootBucketData(_) => "LootBucketSlot",
            NativeManagerShape::TerritoryDefinitionsData(_) => "territoryForAchievement",
            NativeManagerShape::StatModifierData(_) => "FromId",
            NativeManagerShape::SeasonsRewardsData(_) => "rewardsByType",
            NativeManagerShape::SeasonsTrackedStatData(_) => "trackedStatFromId",
            NativeManagerShape::SeasonsRewardsActivitiesTasksData(_) => "activityTaskByKey",
            NativeManagerShape::SeasonsRewardsBattlePassData(_) => "rankBySeasonKey",
            NativeManagerShape::SeasonsRewardsCardTemplateData(_) => "cardTemplateByKey",
            NativeManagerShape::SeasonsRewardsChapterData(_) => "chapterByKindIndex",
            NativeManagerShape::SeasonsRewardsJourneyData(_) => "journeysForChapter",
            NativeManagerShape::SongBookSheetData(_) => "sheetIdsForPage",
            NativeManagerShape::SongBookData(_) => "sheetIdsForInstrument",
            NativeManagerShape::ElementalMutationStaticData(_) => "possibleElementalStatusEffects",
            NativeManagerShape::PromotionMutationStaticData(_) => {
                "possiblePromotionalStatusEffectsForElement"
            }
            NativeManagerShape::MusicalRewardsData(_) => "rewardForGameEvent",
            NativeManagerShape::CombatProfilesData(_) => "activeAbilityProfileByKey",
            NativeManagerShape::ItemTransformData(_) => "transformByKey",
            NativeManagerShape::GatherableData(_) => "gatheringActionByKey",
            NativeManagerShape::SocialData(_) => "rankBySecurityLevel",
            NativeManagerShape::PlayerData(_) => "hasPlayerBaseAttributes",
            NativeManagerShape::RecipeData(_) => "craftingRecipeDataByResult",
            _ => panic!("pre-lowered shape reached TypeScript native contract test: {shape:?}"),
        }
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
}

fn datasheet_catalog_files(unit: &GameDataCompileUnit) -> Result<Vec<GameDataCodegenFile>> {
    let compressed = crate::rust::source::tables::compressed_datasheet_catalog_json(unit)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(compressed);
    Ok(vec![
        GameDataCodegenFile::new(
            "src/managers/datasheet-catalog.ts",
            format_typescript_source(DATASHEET_CATALOG_TS)?,
        ),
        GameDataCodegenFile::new(
            "src/managers/datasheet-catalog-data.ts",
            format!(
                "export const DATASHEET_CATALOG_BASE64 = {};\n",
                typescript_string_literal(&encoded)
            ),
        ),
    ])
}

const DATASHEET_CATALOG_TS: &str = r#"
import { DATASHEET_CATALOG_BASE64 } from "./datasheet-catalog-data.js";

export interface ColumnSchema {
  readonly name: string;
  readonly crc: number;
  readonly rowKey: boolean;
}

export interface TableSchema {
  readonly name: string;
  readonly rowType: string;
  readonly sources: readonly string[];
  readonly columns: readonly ColumnSchema[];
}

export async function loadTableSchemas(): Promise<readonly TableSchema[]> {
  const compressed = Uint8Array.from(atob(DATASHEET_CATALOG_BASE64), (value) => value.charCodeAt(0));
  const stream = new Blob([compressed]).stream().pipeThrough(new DecompressionStream("gzip"));
  const decoded: unknown = JSON.parse(await new Response(stream).text());
  if (!Array.isArray(decoded)) {
    throw new TypeError("generated datasheet catalog must contain an array");
  }
  return decoded.map(parseTableSchema);
}

function parseTableSchema(value: unknown, index: number): TableSchema {
  const table = record(value, `table ${index}`);
  return {
    name: stringField(table, "name"),
    rowType: stringField(table, "row_type"),
    sources: stringArrayField(table, "sources"),
    columns: arrayField(table, "columns").map(parseColumnSchema),
  };
}

function parseColumnSchema(value: unknown, index: number): ColumnSchema {
  const column = record(value, `column ${index}`);
  return {
    name: stringField(column, "name"),
    crc: u32Field(column, "crc"),
    rowKey: booleanField(column, "row_key"),
  };
}

function record(value: unknown, label: string): Readonly<Record<string, unknown>> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError(`${label} must be an object`);
  }
  return value as Readonly<Record<string, unknown>>;
}

function stringField(value: Readonly<Record<string, unknown>>, field: string): string {
  const fieldValue = value[field];
  if (typeof fieldValue !== "string") {
    throw new TypeError(`${field} must be a string`);
  }
  return fieldValue;
}

function u32Field(value: Readonly<Record<string, unknown>>, field: string): number {
  const fieldValue = value[field];
  if (!Number.isInteger(fieldValue) || (fieldValue as number) < 0 || (fieldValue as number) > 0xffff_ffff) {
    throw new TypeError(`${field} must be an unsigned 32-bit integer`);
  }
  return fieldValue as number;
}

function booleanField(value: Readonly<Record<string, unknown>>, field: string): boolean {
  const fieldValue = value[field];
  if (typeof fieldValue !== "boolean") {
    throw new TypeError(`${field} must be a boolean`);
  }
  return fieldValue;
}

function arrayField(value: Readonly<Record<string, unknown>>, field: string): readonly unknown[] {
  const fieldValue = value[field];
  if (!Array.isArray(fieldValue)) {
    throw new TypeError(`${field} must be an array`);
  }
  return fieldValue;
}

function stringArrayField(value: Readonly<Record<string, unknown>>, field: string): readonly string[] {
  return arrayField(value, field).map((entry, index) => {
    if (typeof entry !== "string") {
      throw new TypeError(`${field}[${index}] must be a string`);
    }
    return entry;
  });
}
"#;

fn manager_record_types_source(records: &[SemanticManagerRecord]) -> Result<String> {
    let unit = semantic_manager_record_unit(records);
    SerializeTypeScriptSourceEmitter
        .emit_with_options(
            &unit,
            &SerializeTypeScriptSourceOptions {
                include_support_aliases: false,
                use_support_aliases: true,
                immutable: true,
            },
        )
        .map(|source| format!("import {{ Crc32 }} from \"../values.js\";\n\n{source}"))
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
            ManagerSurface::Native {
                manager,
                shape,
                dependencies,
                ..
            } => push_native_manager_class(source, unit, manager, shape, dependencies),
            ManagerSurface::Semantic(record) => push_semantic_manager_class(source, record),
            ManagerSurface::ItemData(manager) => push_item_data_manager_class(source, manager),
            ManagerSurface::Composition(manager) => push_composition_manager_class(source, manager),
            ManagerSurface::ProductBacked(manager) => {
                push_product_backed_manager_class(source, manager)
            }
        }
    }
    source.push_str(SEMANTIC_MANAGER_RUNTIME_TS);
    if surfaces.iter().any(|surface| {
        matches!(surface, ManagerSurface::Semantic(record) if record.fields.iter().any(
            |field| matches!(
                field.transform,
                SemanticProjectionTransform::OptionalU32
                    | SemanticProjectionTransform::U32DefaultZero
                    | SemanticProjectionTransform::OptionalNonZeroU32
            )
        ))
    }) {
        source.push_str(OPTIONAL_UINT32_CELL_TS);
    }
    if surfaces.iter().any(|surface| {
        matches!(surface, ManagerSurface::Semantic(record) if record.fields.iter().any(
            |field| field.transform == SemanticProjectionTransform::OptionalNonZeroU32
        ))
    }) {
        source.push_str(OPTIONAL_NON_ZERO_UINT32_CELL_TS);
    }
}

fn push_managers_facade(source: &mut String, surfaces: &[ManagerSurface]) {
    let mut fields = String::new();
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
            ManagerSurface::Native { manager, .. } => manager.manager_class_name.as_str(),
            ManagerSurface::Semantic(record) => record.manager_class_name.as_str(),
            ManagerSurface::ItemData(manager) => manager.manager_class_name.as_str(),
            ManagerSurface::Composition(manager) => manager.manager_class_name.as_str(),
        };
        let accessor = ts_manager_accessor_name(manager_name);
        let field = format!("{accessor}Value");
        fields.push_str(&format!("  private {field}?: Promise<{manager_class}>;\n"));
        let build = match surface {
            ManagerSurface::Composition(manager) => {
                let dependencies = manager
                    .dependencies
                    .iter()
                    .map(|dependency| {
                        format!("await this.{}()", ts_manager_accessor_name(dependency))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("return {manager_class}[CREATE_MANAGER]({dependencies});")
            }
            ManagerSurface::Native { dependencies, .. } => {
                let dependencies = dependencies
                    .iter()
                    .map(|dependency| {
                        format!("await this.{}()", ts_manager_accessor_name(dependency))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let arguments = if dependencies.is_empty() {
                    "this.cache".to_owned()
                } else {
                    format!("this.cache, {dependencies}")
                };
                format!("return {manager_class}[CREATE_MANAGER]({arguments});")
            }
            ManagerSurface::Direct(_)
            | ManagerSurface::Semantic(_)
            | ManagerSurface::ItemData(_)
            | ManagerSurface::ProductBacked(_) => {
                format!("return {manager_class}[CREATE_MANAGER](this.cache);")
            }
        };
        let prepare = if matches!(surface, ManagerSurface::Composition(_)) {
            String::new()
        } else {
            let (selectors, assets) = typescript_manager_resource_inputs(surface);
            format!(
                "await this.cache.prepare({manager_name:?}, {selectors}, {assets});\n          "
            )
        };
        methods.push_str(&format!(
            r#"  {accessor}(): Promise<{manager_class}> {{
    return this.{field} ??= (async () => {{
      try {{
        {prepare}{build}
      }} catch (cause) {{
        throw new ManagerLoadError({manager_name:?}, cause);
      }}
    }})();
  }}

"#
        ));
    }

    source.push_str(&format!(
        r#"
export class ManagerLoadError extends Error {{
  readonly manager: string;

  constructor(manager: string, cause: unknown) {{
    super(`load ${{manager}}`, {{ cause }});
    this.name = "ManagerLoadError";
    this.manager = manager;
  }}
}}

export class Managers {{
{fields}
  private constructor(private readonly cache: ManagerCache) {{}}

  static async open(loader: AssetLoader): Promise<Managers> {{
    const tableSchemas = await loadTableSchemas();
    return new Managers(new ManagerCache(loader, tableSchemas));
  }}

{methods}}}

"#
    ));
}

fn typescript_manager_resource_inputs(surface: &ManagerSurface) -> (String, String) {
    let (tables, products): (Vec<(String, String)>, Vec<&str>) = match surface {
        ManagerSurface::Direct(manager)
        | ManagerSurface::ProductBacked(manager)
        | ManagerSurface::Native { manager, .. } => (
            manager
                .tables
                .iter()
                .map(|table| (table.table_name.clone(), table.row_type_name.clone()))
                .collect(),
            manager
                .products
                .iter()
                .map(|product| product.path.as_str())
                .collect(),
        ),
        ManagerSurface::Semantic(manager) => (
            manager
                .tables
                .iter()
                .map(|table| (table.table_name.clone(), table.row_type_name.clone()))
                .collect(),
            Vec::new(),
        ),
        ManagerSurface::ItemData(manager) => (
            manager
                .tables
                .iter()
                .map(|table| (table.table_name.clone(), table.row_type_name.clone()))
                .collect(),
            Vec::new(),
        ),
        ManagerSurface::Composition(_) => (Vec::new(), Vec::new()),
    };
    let selectors = tables
        .into_iter()
        .map(|(table_name, row_type_name)| {
            format!(
                "{{ name: {}, rowType: {} }}",
                typescript_string_literal(&table_name),
                typescript_string_literal(&row_type_name),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let products = products
        .into_iter()
        .map(typescript_string_literal)
        .collect::<Vec<_>>()
        .join(", ");
    (format!("[{selectors}]"), format!("[{products}]"))
}

fn push_composition_manager_class(source: &mut String, manager: &CompositionManagerSurface) {
    source.push_str(match manager.kind {
        CompositionManagerKind::CurrencyExchangeMapping => CURRENCY_EXCHANGE_MAPPING_MANAGER_TS,
        CompositionManagerKind::ReplicationData => REPLICATION_DATA_MANAGER_TS,
        CompositionManagerKind::StaticTradeskillRankDataMapping => {
            STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_TS
        }
        CompositionManagerKind::VitalsModifierMapping => VITALS_MODIFIER_MAPPING_MANAGER_TS,
    });
}

const CURRENCY_EXCHANGE_MAPPING_MANAGER_TS: &str = r#"
export type CurrencyExchangeEndpoint =
  | { readonly kind: "currency" }
  | { readonly kind: "categoricalProgression"; readonly id: Crc32 };

export interface CurrencyExchangeMapping {
  readonly source: CurrencyExchangeEndpoint;
  readonly target: CurrencyExchangeEndpoint;
  readonly exchange: CurrencyExchangeData;
}

export class CurrencyExchangeMappingManager {
  private readonly mappingsCache: readonly CurrencyExchangeMapping[];
  private readonly mappingsByEndpoint = new Map<string, CurrencyExchangeMapping>();

  private constructor(
    currencyExchangeData: CurrencyExchangeDataManager,
    categoricalProgressionData: CategoricalProgressionDataManager,
  ) {
    const mappings: CurrencyExchangeMapping[] = [];
    for (const exchange of currencyExchangeData) {
      const source = currencyExchangeEndpoint(
        exchange.fromCurrencyCrc,
        exchange.fromCurrencyIsCategoricalProgression,
        categoricalProgressionData,
      );
      const target = currencyExchangeEndpoint(
        exchange.toCurrencyCrc,
        exchange.toCurrencyIsCategoricalProgression,
        categoricalProgressionData,
      );
      if (source === undefined || target === undefined) continue;
      if (
        source.kind === "categoricalProgression" &&
        target.kind === "categoricalProgression" &&
        source.id === target.id
      ) continue;
      const key = currencyExchangeEndpointPairKey(source, target);
      if (this.mappingsByEndpoint.has(key)) continue;
      const mapping = Object.freeze({ source, target, exchange });
      this.mappingsByEndpoint.set(key, mapping);
      mappings.push(mapping);
    }
    this.mappingsCache = Object.freeze(mappings);
  }

  static [CREATE_MANAGER](
    currencyExchangeData: CurrencyExchangeDataManager,
    categoricalProgressionData: CategoricalProgressionDataManager,
  ): CurrencyExchangeMappingManager {
    return new CurrencyExchangeMappingManager(currencyExchangeData, categoricalProgressionData);
  }

  mapping(source: CurrencyExchangeEndpoint, target: CurrencyExchangeEndpoint): CurrencyExchangeMapping | undefined {
    return this.mappingsByEndpoint.get(currencyExchangeEndpointPairKey(source, target));
  }

  currencyExchange(source: CurrencyExchangeEndpoint, target: CurrencyExchangeEndpoint): CurrencyExchangeData | undefined {
    return this.mapping(source, target)?.exchange;
  }

  conversionId(source: CurrencyExchangeEndpoint, target: CurrencyExchangeEndpoint): Crc32 | undefined {
    return this.currencyExchange(source, target)?.conversionCrc;
  }

  mappings(): IterableIterator<CurrencyExchangeMapping> { return this.mappingsCache.values(); }
  [Symbol.iterator](): Iterator<CurrencyExchangeMapping> { return this.mappings(); }
  len(): number { return this.mappingsCache.length; }
  isEmpty(): boolean { return this.mappingsCache.length === 0; }
}

function currencyExchangeEndpoint(
  id: Crc32,
  categorical: boolean,
  progressions: CategoricalProgressionDataManager,
): CurrencyExchangeEndpoint | undefined {
  if (!categorical) return Object.freeze({ kind: "currency" });
  const progression = progressions.categoricalProgressionDataFromId(id);
  return progression === undefined
    ? undefined
    : Object.freeze({ kind: "categoricalProgression", id: progression.categoricalProgressionIdCrc });
}

function currencyExchangeEndpointPairKey(source: CurrencyExchangeEndpoint, target: CurrencyExchangeEndpoint): string {
  return `${currencyExchangeEndpointKey(source)}>${currencyExchangeEndpointKey(target)}`;
}

function currencyExchangeEndpointKey(endpoint: CurrencyExchangeEndpoint): string {
  return endpoint.kind === "currency" ? "currency" : `progression:${endpoint.id}`;
}
"#;

const REPLICATION_DATA_MANAGER_TS: &str = r#"
export class ReplicationDataManager {
  private readonly indexesById = new Map<Crc32, number>();

  private constructor(private readonly idsCache: readonly Crc32[]) {
    for (let index = 1; index < idsCache.length && index <= 0xffff; index += 1) {
      const id = idsCache[index];
      if (id !== Crc32.ZERO && !this.indexesById.has(id)) this.indexesById.set(id, index);
    }
  }

  static [CREATE_MANAGER](perkData: PerkDataManager): ReplicationDataManager {
    return new ReplicationDataManager(Object.freeze([Crc32.ZERO, ...perkData.perkIds()]));
  }

  idAt(index: number): Crc32 {
    return Number.isInteger(index) && index >= 0 ? (this.idsCache[index] ?? Crc32.ZERO) : Crc32.ZERO;
  }

  indexOf(id: string | Crc32): number {
    const key = crc32LookupKey(id);
    if (key === Crc32.ZERO) return 0;
    return this.indexesById.get(key) ?? 0;
  }

  ids(): readonly Crc32[] { return this.idsCache; }
  len(): number { return this.idsCache.length; }
  isEmpty(): boolean { return this.idsCache.length === 0; }
}
"#;

const VITALS_MODIFIER_MAPPING_MANAGER_TS: &str = r#"
export interface VitalsModifierMapping {
  readonly key: string;
  readonly id: Crc32;
}

export class VitalsModifierMappingManager {
  private readonly entriesCache: VitalsModifierMapping[] = [];
  private readonly entriesById = new Map<Crc32, VitalsModifierMapping>();

  private constructor(vitals: VitalsDataManager, damage: DamageDataManager, items: ItemDataManager) {
    for (const entry of vitals) this.insertLowercase(entry.key);
    for (const entry of damage.damageTypes()) this.insertLowercase(entry.key);
    for (const entry of damage) {
      this.insertLowercase(normalizeWeaponCategory(entry.weaponCategory));
    }
    this.insertLowercase("Physical");
    this.insertLowercase("Elemental");
    for (const item of items) this.insertItemAliases(item.itemId, item.itemIdCrc);
  }

  static [CREATE_MANAGER](
    vitals: VitalsDataManager,
    damage: DamageDataManager,
    items: ItemDataManager,
  ): VitalsModifierMappingManager {
    return new VitalsModifierMappingManager(vitals, damage, items);
  }

  get(id: string | Crc32): VitalsModifierMapping | undefined { return this.entriesById.get(crc32LookupKey(id)); }
  byKey(key: string): VitalsModifierMapping | undefined { return this.entriesById.get(Crc32.fromStringLower(key)); }
  rows(): IterableIterator<VitalsModifierMapping> { return this.entriesCache.values(); }
  [Symbol.iterator](): Iterator<VitalsModifierMapping> { return this.rows(); }
  len(): number { return this.entriesCache.length; }
  isEmpty(): boolean { return this.entriesCache.length === 0; }

  private insertLowercase(raw: string): void {
    const key = raw.trim();
    if (key.length !== 0) this.insertWithId(key, Crc32.fromStringLower(key));
  }

  private insertItemAliases(raw: string, id: Crc32): void {
    const key = raw.trim();
    if (key.length === 0 || id === Crc32.ZERO) return;
    const entry = this.insertWithId(key, id);
    const lowercaseId = Crc32.fromStringLower(key);
    if (lowercaseId !== Crc32.ZERO && !this.entriesById.has(lowercaseId)) this.entriesById.set(lowercaseId, entry);
  }

  private insertWithId(key: string, id: Crc32): VitalsModifierMapping {
    const existing = this.entriesById.get(id);
    if (existing !== undefined) return existing;
    const entry = Object.freeze({ key, id });
    this.entriesById.set(id, entry);
    this.entriesCache.push(entry);
    return entry;
  }
}

function normalizeWeaponCategory(value: string): string {
  const normalized = value.trim();
  return normalized.length === 0 || normalized.toLowerCase() === "none" ? "Default" : normalized;
}
"#;

const STATIC_TRADESKILL_RANK_DATA_MAPPING_MANAGER_TS: &str = r#"
export interface StaticTradeskillRankDataMapping {
  readonly categoricalProgressionId: Crc32;
  readonly table: TradeskillRankDataTable;
  readonly rank: number;
}

export interface PlayerLevelRankMapping {
  readonly displayNameId: Crc32;
  readonly rank: number;
}

const TRADESKILL_TYPES: readonly TradeskillType[] = Object.freeze([
  "Weaponsmithing", "Armoring", "Jewelcrafting", "Arcana", "Cooking", "Furnishing",
  "Engineering", "Smelting", "Woodworking", "Leatherworking", "Weaving", "Stonecutting",
  "Skinning", "Mining", "Logging", "Harvesting", "Fishing", "AzothStaff", "Musician", "Riding",
]);

const TRADESKILL_RANK_TABLES: ReadonlySet<string> = new Set(TRADESKILL_TYPES);

function isTradeskillRankTable(
  value: string,
): value is TradeskillRankDataTable {
  return TRADESKILL_RANK_TABLES.has(value);
}

export class StaticTradeskillRankDataMappingManager {
  private readonly playerLevelsByDisplayName = new Map<Crc32, number>();
  private readonly tradeskillRanksByDisplayName = new Map<Crc32, StaticTradeskillRankDataMapping>();

  private constructor(
    experience: ExperienceDataManager,
    player: PlayerDataManager,
    progressions: CategoricalProgressionDataManager,
    ranks: TradeskillRankDataManager,
  ) {
    const maxPlayerLevel = experience.maxLevel();
    if (maxPlayerLevel !== undefined) {
      for (const row of ranks.playerLevels()) {
        if (row.rank > maxPlayerLevel || row.displayNameId === Crc32.ZERO) continue;
        if (!this.playerLevelsByDisplayName.has(row.displayNameId)) {
          this.playerLevelsByDisplayName.set(row.displayNameId, row.rank);
        }
      }
    }

    for (const tradeskill of TRADESKILL_TYPES) {
      const categoricalProgressionId = player.categoricalProgressionId(tradeskill);
      if (categoricalProgressionId === undefined) continue;
      const progression = progressions.categoricalProgressionDataFromId(categoricalProgressionId);
      if (
        progression === undefined
        || progression.rankTableId === null
        || !isTradeskillRankTable(progression.rankTableId)
      ) continue;
      for (let rank = 0; rank <= progression.maxLevel; rank += 1) {
        if (rank > 0xffff) {
          throw new RangeError(
            `categorical progression ${progression.categoricalProgressionId} max rank ${rank} exceeds u16 range`,
          );
        }
        const row = ranks.tradeskillRank(progression.rankTableId, rank);
        if (row === undefined || row.displayNameId === Crc32.ZERO) continue;
        if (!this.tradeskillRanksByDisplayName.has(row.displayNameId)) {
          this.tradeskillRanksByDisplayName.set(
            row.displayNameId,
            Object.freeze({ categoricalProgressionId, table: row.table, rank }),
          );
        }
      }
    }
  }

  static [CREATE_MANAGER](
    experience: ExperienceDataManager,
    player: PlayerDataManager,
    progressions: CategoricalProgressionDataManager,
    ranks: TradeskillRankDataManager,
  ): StaticTradeskillRankDataMappingManager {
    return new StaticTradeskillRankDataMappingManager(experience, player, progressions, ranks);
  }

  playerLevelForDisplayName(displayName: string | Crc32): number | undefined {
    return this.playerLevelsByDisplayName.get(crc32LookupKey(displayName));
  }

  tradeskillRankForDisplayName(
    displayName: string | Crc32,
  ): StaticTradeskillRankDataMapping | undefined {
    return this.tradeskillRanksByDisplayName.get(crc32LookupKey(displayName));
  }

  *playerLevels(): IterableIterator<PlayerLevelRankMapping> {
    for (const [displayNameId, rank] of this.playerLevelsByDisplayName) {
      yield Object.freeze({ displayNameId, rank });
    }
  }

  tradeskillRanks(): IterableIterator<StaticTradeskillRankDataMapping> {
    return this.tradeskillRanksByDisplayName.values();
  }

  len(): number {
    return this.playerLevelsByDisplayName.size + this.tradeskillRanksByDisplayName.size;
  }

  isEmpty(): boolean {
    return this.playerLevelsByDisplayName.size === 0 && this.tradeskillRanksByDisplayName.size === 0;
  }
}
"#;

fn push_direct_manager_class(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) {
    push_direct_manager_class_with_dependencies(
        source,
        unit,
        manager,
        &[],
        &TsNativeManagerAugmentation::default(),
    );
}

#[derive(Debug, Default)]
struct TsNativeManagerAugmentation {
    declarations: String,
    fields: String,
    initializers: String,
    methods: String,
    rows_interface: Option<String>,
    row_methods: Option<String>,
}

fn push_direct_manager_class_with_dependencies(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    dependencies: &[String],
    augmentation: &TsNativeManagerAugmentation,
) {
    let manager_class = &manager.manager_class_name;
    let manager_instance = ts_direct_manager_instance_expression(manager);
    let mut product_methods = direct_ts_product_methods(manager);
    product_methods.push_str(&special_ts_manager_extra_methods(manager_class));
    let row_methods = augmentation.row_methods.as_ref().map_or_else(
        || direct_ts_schema_methods(unit, manager),
        |semantic_methods| {
            format!(
                "{semantic_methods}{}",
                direct_ts_named_row_family_methods(unit, manager)
            )
        },
    );
    let rows_interface = augmentation
        .rows_interface
        .clone()
        .unwrap_or_else(|| direct_ts_rows_interface(unit, manager));
    let row_specs = ts_direct_row_specs(unit, manager);
    let row_fields = row_specs
        .iter()
        .map(|row| {
            let table_type = ts_direct_table_type_name(manager, &row.source_row_type);
            format!(
                "  private readonly {}: RowCollection<{}, {}>;\n",
                ts_direct_row_field_name(&row.source_row_type),
                row.type_name,
                table_type,
            )
        })
        .collect::<String>();
    let row_initializers = row_specs
        .iter()
        .map(|row| {
            let table_sources = ts_direct_table_sources_expression(unit, manager, row);
            format!(
                "    this.{} = new RowCollectionImpl(resources.schemaFamilyEntries({:?}, {}), {table_sources});\n",
                ts_direct_row_field_name(&row.source_row_type),
                row.source_row_type,
                ts_schema_reader_name(&row.source_row_type)
            )
        })
        .collect::<String>();
    let table_types = ts_direct_table_type_declarations(unit, manager, &row_specs);
    let (product_fields, product_initializers) = ts_product_storage(manager);
    let dependency_parameters = dependencies
        .iter()
        .map(|dependency| {
            format!(
                ", _{}: {}",
                ts_manager_dependency_name(dependency),
                dependency
            )
        })
        .collect::<String>();
    let dependency_arguments = dependencies
        .iter()
        .map(|dependency| format!(", _{}", ts_manager_dependency_name(dependency)))
        .collect::<String>();
    let native_fields = &augmentation.fields;
    let native_initializers = &augmentation.initializers;
    let native_methods = &augmentation.methods;
    source.push_str(&augmentation.declarations);
    let constructor = if row_specs.is_empty()
        && product_fields.is_empty()
        && native_initializers.is_empty()
    {
        format!("private constructor(_resources: ManagerResources{dependency_parameters}) {{}}")
    } else {
        format!(
            "private constructor(resources: ManagerResources{dependency_parameters}) {{\n{row_initializers}{product_initializers}{native_initializers}  }}"
        )
    };
    source.push_str(&format!(
        r#"
{table_types}
export class {manager_class}{rows_interface} {{
{row_fields}
{product_fields}
{native_fields}
  {constructor}

  static [CREATE_MANAGER](cache: ManagerCache{dependency_parameters}): {manager_class} {{
    return new {manager_class}({manager_instance}{dependency_arguments});
  }}

{row_methods}
{product_methods}
{native_methods}
}}

"#
    ));
}

fn push_native_manager_class(
    source: &mut String,
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
    dependencies: &[String],
) {
    let effective = ts_effective_native_manager_surface(manager, shape);
    let augmentation = ts_native_manager_augmentation(unit, &effective, shape);
    push_direct_manager_class_with_dependencies(
        source,
        unit,
        &effective,
        dependencies,
        &augmentation,
    );
}

fn ts_effective_native_manager_surface(
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> DirectManagerSurface {
    let mut effective = manager.clone();
    if let NativeManagerShape::RecipeData(recipe) = shape {
        for table in recipe.tables() {
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

fn ts_native_manager_augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> TsNativeManagerAugmentation {
    native::augmentation(unit, manager, shape)
}

fn experience_manager_augmentation() -> TsNativeManagerAugmentation {
    TsNativeManagerAugmentation {
        fields: r#"  private readonly experienceByLevel = new Map<number, ExperienceDataSchemaRow>();
  private readonly gearScoreThresholds: Array<readonly [number, number]> = [];
  private readonly xpThresholds: Array<readonly [number, number]> = [];
  private maxLevelValue: number | undefined;
"#
        .to_owned(),
        initializers: r#"    for (const { row } of this.experienceDataCollection) {
      const level = normalizeUnsignedInteger(row.levelNumber);
      this.experienceByLevel.set(level, row);
      this.maxLevelValue = this.maxLevelValue === undefined ? level : Math.max(this.maxLevelValue, level);
      const gearScore = normalizeOptionalPositiveInteger(row.maxEquippableGearScore);
      if (gearScore !== undefined) this.gearScoreThresholds.push([gearScore, level]);
      this.xpThresholds.push([normalizeOptionalUnsignedInteger(row.xpToLevel), level]);
    }
    this.gearScoreThresholds.sort(compareNumberPairs);
    this.xpThresholds.sort(compareNumberPairs);
"#
        .to_owned(),
        methods: r#"  experienceDataFromId(level: number): ExperienceDataSchemaRow | undefined {
    return this.experienceByLevel.get(normalizeUnsignedInteger(level));
  }

  experienceData(level: number): ExperienceDataSchemaRow | undefined {
    return this.experienceDataFromId(level);
  }

  experienceDataForMaxEquippableGearScore(gearScore: number): ExperienceDataSchemaRow | undefined {
    const normalized = normalizeUnsignedInteger(gearScore);
    const match = this.gearScoreThresholds.find(([threshold]) => normalized <= threshold);
    return match === undefined ? undefined : this.experienceByLevel.get(match[1]);
  }

  levelForXp(xp: number | bigint): number {
    const normalized = typeof xp === "bigint" ? xp : BigInt(normalizeUnsignedInteger(xp));
    let level = 0;
    for (const [threshold, candidate] of this.xpThresholds) {
      if (BigInt(threshold) <= normalized) level = Math.max(level, candidate);
    }
    return level;
  }

  maxLevel(): number | undefined {
    return this.maxLevelValue;
  }

  len(): number { return this.experienceByLevel.size; }
  isEmpty(): boolean { return this.experienceByLevel.size === 0; }
"#
        .to_owned(),
        ..TsNativeManagerAugmentation::default()
    }
}

fn damage_manager_augmentation(
    _unit: &GameDataCompileUnit,
    _manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    TsNativeManagerAugmentation {
        declarations: r#"export interface DamageDataReference {
  readonly table: DamageDataTable;
  readonly id: Crc32;
}

export interface DamageDataSlot {
  readonly table: DamageDataTable;
  readonly rowIndex: number;
}

export interface DamageData {
  readonly reference: DamageDataReference;
  readonly slot: DamageDataSlot;
  readonly key: string;
  readonly id: Crc32;
  readonly weaponCategory: string;
  readonly weaponCategoryId: Crc32;
  readonly source: RowRef<DamageDataTable, DamageDataSchemaRow>;
}

export interface DamageTypeData {
  readonly key: string;
  readonly id: Crc32;
  readonly numericId: number;
  readonly source: RowRef<DamageDataDamageTypeDataTable, DamageTypeDataSchemaRow>;
}

export interface AfflictionData {
  readonly key: string;
  readonly id: Crc32;
  readonly numericId: number;
  readonly source: RowRef<DamageDataAfflictionDataTable, AfflictionDataSchemaRow>;
}

"#
        .to_owned(),
        fields: r#"  private readonly damageEntries: DamageData[] = [];
  private readonly damageByReference = new Map<string, DamageData>();
  private readonly damageBySlotIndex = new Map<string, DamageData>();
  private readonly damageTypeEntries: DamageTypeData[] = [];
  private readonly damageTypesById = new Map<Crc32, DamageTypeData>();
  private readonly afflictionEntries: AfflictionData[] = [];
  private readonly afflictionsById = new Map<Crc32, AfflictionData>();
  private readonly weaponCategoryEntries: string[] = [];
  private readonly weaponCategoryIds = new Set<Crc32>();
"#
        .to_owned(),
        initializers: r#"    for (const source of this.damageDataCollection) {
      const table = source.ref.table;
      const key = source.row.damageId.trim();
      if (key.length === 0) continue;
      const id = Crc32.fromStringLower(key);
      if (id === Crc32.ZERO || source.slot.rowIndex >= 0xffff) continue;
      const reference = Object.freeze({ table, id });
      const slot = Object.freeze({ table, rowIndex: source.slot.rowIndex });
      const referenceKey = damageReferenceLookupKey(reference);
      const slotKey = damageSlotLookupKey(slot);
      if (this.damageByReference.has(referenceKey) || this.damageBySlotIndex.has(slotKey)) continue;
      const weaponCategory = normalizeWeaponCategory(source.row.weaponCategory ?? "");
      const weaponCategoryId = Crc32.fromStringLower(weaponCategory);
      const data = Object.freeze({ reference, slot, key, id, weaponCategory, weaponCategoryId, source: source.ref });
      this.damageEntries.push(data);
      this.damageByReference.set(referenceKey, data);
      this.damageBySlotIndex.set(slotKey, data);
      if (weaponCategoryId !== Crc32.ZERO && !this.weaponCategoryIds.has(weaponCategoryId)) {
        this.weaponCategoryIds.add(weaponCategoryId);
        this.weaponCategoryEntries.push(weaponCategory);
      }
    }
    for (const source of this.damageTypeDataCollection) {
      const key = source.row.typeId.trim();
      const id = Crc32.fromStringLower(key);
      const numericId = normalizeOptionalUnsignedInteger(source.row.intId);
      if (key.length === 0 || id === Crc32.ZERO || numericId === 0 || numericId > 0xff || this.damageTypesById.has(id)) continue;
      const data = Object.freeze({ key, id, numericId, source: source.ref });
      this.damageTypeEntries.push(data);
      this.damageTypesById.set(id, data);
    }
    for (const source of this.afflictionDataCollection) {
      const key = source.row.afflictionId.trim();
      const id = Crc32.fromStringLower(key);
      const numericId = normalizeOptionalUnsignedInteger(source.row.intId);
      if (key.length === 0 || id === Crc32.ZERO || numericId === 0 || numericId >= 0xff || this.afflictionsById.has(id)) continue;
      const data = Object.freeze({ key, id, numericId, source: source.ref });
      this.afflictionEntries.push(data);
      this.afflictionsById.set(id, data);
    }
"#
        .to_owned(),
        methods: r#"  damage(reference: DamageDataReference): DamageData | undefined {
    return this.damageByReference.get(damageReferenceLookupKey(reference));
  }

  damageBySlot(slot: DamageDataSlot): DamageData | undefined {
    return this.damageBySlotIndex.get(damageSlotLookupKey(slot));
  }

  damageById(table: DamageDataTable, id: string | Crc32): DamageData | undefined {
    return this.damage({ table, id: crc32LookupKey(id) });
  }

  damageByKey(table: DamageDataTable, key: string): DamageData | undefined {
    return this.damageById(table, key);
  }

  resolve(reference: TableReference): DamageData | undefined {
    const table = parseDamageDataTable(reference.path);
    return table === undefined ? undefined : this.damageByKey(table, reference.key);
  }

  damageRefBySlot(slot: DamageDataSlot): DamageDataReference | undefined {
    return this.damageBySlot(slot)?.reference;
  }

  damageKeyBySlot(slot: DamageDataSlot): string | undefined {
    return this.damageBySlot(slot)?.key;
  }

  damageType(id: string | Crc32): DamageTypeData | undefined {
    return this.damageTypesById.get(crc32LookupKey(id));
  }

  damageTypeByKey(key: string): DamageTypeData | undefined { return this.damageType(key); }
  affliction(id: string | Crc32): AfflictionData | undefined { return this.afflictionsById.get(crc32LookupKey(id)); }
  afflictionByKey(key: string): AfflictionData | undefined { return this.affliction(key); }
  damageRows(): IterableIterator<DamageData> { return this.damageEntries.values(); }
  damageTypes(): IterableIterator<DamageTypeData> { return this.damageTypeEntries.values(); }
  *damageTypeIds(): IterableIterator<Crc32> { for (const row of this.damageTypeEntries) yield row.id; }
  afflictions(): IterableIterator<AfflictionData> { return this.afflictionEntries.values(); }
  weaponCategories(): IterableIterator<string> { return this.weaponCategoryEntries.values(); }
  len(): number { return this.damageEntries.length; }
  isEmpty(): boolean { return this.damageEntries.length === 0; }
"#
        .to_owned(),
        rows_interface: Some(" implements Rows<DamageData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<DamageData> { return this.damageEntries.values(); }\n  [Symbol.iterator](): Iterator<DamageData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn vitals_manager_augmentation() -> TsNativeManagerAugmentation {
    TsNativeManagerAugmentation {
        declarations: r#"export interface VitalsLevelVariantData {
  readonly key: string;
  readonly id: Crc32;
  readonly baseVitalsKey: string;
  readonly baseVitalsId: Crc32;
  readonly source: RowRef<VitalsDataTable, VitalsLevelVariantDataSchemaRow>;
}

"#
        .to_owned(),
        fields: r#"  private readonly vitalsEntries: VitalsLevelVariantData[] = [];
  private readonly vitalsById = new Map<Crc32, VitalsLevelVariantData>();
  private readonly creatureTypeIdEntries: Crc32[] = [];
  private readonly creatureTypeIdSet = new Set<Crc32>();
"#
        .to_owned(),
        initializers: r#"    for (const source of this.vitalsLevelVariantDataCollection) {
      const key = source.row.vitalsId.trim();
      const id = Crc32.fromStringLower(key);
      if (key.length === 0 || id === Crc32.ZERO || this.vitalsById.has(id)) continue;
      const baseVitalsKey = source.row.baseVitalsId?.trim() ?? "";
      const baseVitalsId = Crc32.fromStringLower(baseVitalsKey);
      const data = Object.freeze({ key, id, baseVitalsKey, baseVitalsId, source: source.ref });
      this.vitalsEntries.push(data);
      this.vitalsById.set(id, data);
      const creatureTypeId = _vitalsBaseData.vitalsBaseDataFromId(baseVitalsId)?.creatureTypeCrc;
      if (creatureTypeId !== undefined && creatureTypeId !== Crc32.ZERO && !this.creatureTypeIdSet.has(creatureTypeId)) {
        this.creatureTypeIdSet.add(creatureTypeId);
        this.creatureTypeIdEntries.push(creatureTypeId);
      }
    }
"#
        .to_owned(),
        methods: r#"  getById(id: string | Crc32): VitalsLevelVariantData | undefined {
    return this.vitalsById.get(crc32LookupKey(id));
  }

  byKey(key: string): VitalsLevelVariantData | undefined { return this.getById(key); }
  vitals(): IterableIterator<VitalsLevelVariantData> { return this.vitalsEntries.values(); }
  creatureTypeIds(): IterableIterator<Crc32> { return this.creatureTypeIdEntries.values(); }
  len(): number { return this.vitalsEntries.length; }
  isEmpty(): boolean { return this.vitalsEntries.length === 0; }
"#
        .to_owned(),
        rows_interface: Some(" implements Rows<VitalsLevelVariantData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<VitalsLevelVariantData> { return this.vitalsEntries.values(); }\n  [Symbol.iterator](): Iterator<VitalsLevelVariantData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn tradeskill_rank_manager_augmentation(
    _unit: &GameDataCompileUnit,
    _manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    TsNativeManagerAugmentation {
        declarations: r#"export interface PlayerLevelRankData {
  readonly rank: number;
  readonly displayNameId: Crc32;
  readonly source: RowRef<TradeskillRankDataExperienceDataTable, ExperienceDataSchemaRow>;
}

export interface StaticTradeskillRankData {
  readonly table: TradeskillRankDataTable;
  readonly rank: number;
  readonly displayName: string | null;
  readonly displayNameId: Crc32;
  readonly source: RowRef<TradeskillRankDataTable, TradeskillRankDataSchemaRow>;
}

export type TradeskillRankData = PlayerLevelRankData | StaticTradeskillRankData;

"#
        .to_owned(),
        fields: r#"  private readonly playerLevelsByRank = new Map<number, PlayerLevelRankData>();
  private readonly tradeskillRanksByTableAndRank = new Map<string, StaticTradeskillRankData>();
"#
        .to_owned(),
        initializers: r#"    for (const source of this.experienceDataCollection) {
      const rank = normalizeUnsignedInteger(source.row.levelNumber);
      if (!this.playerLevelsByRank.has(rank)) {
        this.playerLevelsByRank.set(rank, Object.freeze({ rank, displayNameId: Crc32.ZERO, source: source.ref }));
      }
    }
    for (const source of this.tradeskillRankDataCollection) {
      const table = source.ref.table;
      const rank = normalizeUnsignedInteger(source.row.level);
      const key = tradeskillRankLookupKey(table, rank);
      if (this.tradeskillRanksByTableAndRank.has(key)) continue;
      const displayName = source.row.displayName?.trim() || null;
      this.tradeskillRanksByTableAndRank.set(key, Object.freeze({
        table,
        rank,
        displayName,
        displayNameId: displayName === null ? Crc32.ZERO : Crc32.fromStringLower(displayName),
        source: source.ref,
      }));
    }
"#
        .to_owned(),
        methods: r#"  playerLevelRow(rank: number): PlayerLevelRankData | undefined {
    return this.playerLevelsByRank.get(normalizeUnsignedInteger(rank));
  }

  tradeskillRank(table: TradeskillRankDataTable, rank: number): StaticTradeskillRankData | undefined {
    return this.tradeskillRanksByTableAndRank.get(tradeskillRankLookupKey(table, rank));
  }

  playerLevels(): IterableIterator<PlayerLevelRankData> {
    return this.playerLevelsByRank.values();
  }

  tradeskillRanks(): IterableIterator<StaticTradeskillRankData> {
    return this.tradeskillRanksByTableAndRank.values();
  }

  len(): number { return this.playerLevelsByRank.size + this.tradeskillRanksByTableAndRank.size; }
  isEmpty(): boolean { return this.playerLevelsByRank.size === 0 && this.tradeskillRanksByTableAndRank.size === 0; }
"#
        .to_owned(),
        rows_interface: Some(" implements Rows<TradeskillRankData>".to_owned()),
        row_methods: Some("  *rows(): IterableIterator<TradeskillRankData> { yield* this.playerLevelsByRank.values(); yield* this.tradeskillRanksByTableAndRank.values(); }\n  [Symbol.iterator](): Iterator<TradeskillRankData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn ts_manager_instance_expression<'a>(
    manager_name: &str,
    tables: impl IntoIterator<Item = (&'a str, &'a str)>,
    asset_paths: impl IntoIterator<Item = &'a str>,
) -> String {
    let tables = tables
        .into_iter()
        .map(|(name, row_type)| {
            format!(
                "{{ name: {}, rowType: {} }}",
                typescript_string_literal(name),
                typescript_string_literal(row_type)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let assets = asset_paths
        .into_iter()
        .map(typescript_string_literal)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "cache.resourcesForTables({}, [{tables}], [{assets}])",
        typescript_string_literal(manager_name)
    )
}

fn ts_direct_manager_instance_expression(manager: &DirectManagerSurface) -> String {
    let row_types = manager
        .tables
        .iter()
        .map(|table| table.row_type_name.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(typescript_string_literal)
        .collect::<Vec<_>>()
        .join(", ");
    let assets = manager
        .products
        .iter()
        .map(|product| typescript_string_literal(&product.path))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "cache.resourcesForRows({}, [{row_types}], [{assets}])",
        typescript_string_literal(&manager.manager_name)
    )
}

fn direct_ts_rows_interface(unit: &GameDataCompileUnit, manager: &DirectManagerSurface) -> String {
    let Some(row_spec) = ts_direct_default_row_spec(unit, manager) else {
        return String::new();
    };
    let table_type = ts_direct_table_type_name(manager, &row_spec.source_row_type);
    format!(
        " implements Rows<RowEntry<{table_type}, {}>>",
        row_spec.type_name
    )
}

fn ts_direct_table_type_name(manager: &DirectManagerSurface, source_row_type: &str) -> String {
    let manager_stem = manager
        .manager_class_name
        .strip_suffix("Manager")
        .unwrap_or(&manager.manager_class_name);
    let row_types = manager
        .tables
        .iter()
        .map(|table| table.row_type_name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if default_direct_manager_row_type(&manager.manager_class_name, &row_types)
        == Some(source_row_type)
    {
        format!("{manager_stem}Table")
    } else {
        format!(
            "{manager_stem}{}Table",
            to_upper_camel_ident(source_row_type, "Rows")
        )
    }
}

fn ts_direct_table_type_declarations(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    rows: &[TsSchemaRow],
) -> String {
    rows.iter()
        .map(|row| {
            let table_type = ts_direct_table_type_name(manager, &row.source_row_type);
            let family_tables = manager
                .tables
                .iter()
                .filter(|table| table.row_type_name == row.source_row_type)
                .collect::<Vec<_>>();
            let tables = family_tables
                .iter()
                .map(|table| typescript_string_literal(&table.table_name))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(" | ");
            let checks = family_tables
                .iter()
                .map(|table| {
                    let mut paths = vec![table.table_name.as_str()];
                    if let Some(schema) = unit.schema_report().tables.iter().find(|schema| {
                        schema.table_name == table.table_name
                            && schema.row_type_name == table.row_type_name
                    }) {
                        paths.extend(schema.sources.iter().map(String::as_str));
                    }
                    let matches = paths
                        .into_iter()
                        .map(|path| {
                            format!(
                                "tablePathMatches(path, {})",
                                typescript_string_literal(path)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" || ");
                    format!(
                        "  if ({matches}) return {};\n",
                        typescript_string_literal(&table.table_name)
                    )
                })
                .collect::<String>();
            format!(
                "export type {table_type} = {tables};\n\nexport function parse{table_type}(path: string): {table_type} | undefined {{\n{checks}  return undefined;\n}}\n"
            )
        })
        .collect()
}

fn ts_direct_table_sources_expression(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row: &TsSchemaRow,
) -> String {
    let table_type = ts_direct_table_type_name(manager, &row.source_row_type);
    let entries = manager
        .tables
        .iter()
        .filter(|input| input.row_type_name == row.source_row_type)
        .map(|input| {
            let sources = unit
                .schema_report()
                .tables
                .iter()
                .find(|table| {
                    table.table_name == input.table_name
                        && table.row_type_name == input.row_type_name
                })
                .map(|table| table.sources.as_slice())
                .unwrap_or_default()
                .iter()
                .map(|source| typescript_string_literal(source))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "[{}, [{}]]",
                typescript_string_literal(&input.table_name),
                sources
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    format!("new Map<{table_type}, readonly string[]>([{entries}])")
}

fn direct_ts_schema_methods(unit: &GameDataCompileUnit, manager: &DirectManagerSurface) -> String {
    let default_row_type = ts_direct_default_row_spec(unit, manager).map(|row| row.source_row_type);
    let mut source = String::new();
    for row_spec in ts_direct_row_specs(unit, manager) {
        let row_type = &row_spec.source_row_type;
        let is_default_row_type = default_row_type.as_deref() == Some(row_type.as_str());
        if is_default_row_type {
            source.push_str(&ts_direct_primary_row_family_methods(manager, &row_spec));
        } else {
            let accessor = format!("{}Rows", ts_method_name(row_type));
            let field = ts_direct_row_field_name(row_type);
            let schema_row_type = &row_spec.type_name;
            let table_type = ts_direct_table_type_name(manager, row_type);
            source.push_str(&format!(
                r#"  {accessor}(): RowCollection<{schema_row_type}, {table_type}> {{
    return this.{field};
  }}

"#
            ));
        }
    }
    source
}

fn direct_ts_named_row_family_methods(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> String {
    ts_direct_row_specs(unit, manager)
        .into_iter()
        .map(|row_spec| {
            let accessor = format!("{}Rows", ts_method_name(&row_spec.source_row_type));
            let field = ts_direct_row_field_name(&row_spec.source_row_type);
            let table_type = ts_direct_table_type_name(manager, &row_spec.source_row_type);
            let schema_row_type = row_spec.type_name;
            format!(
                "  {accessor}(): RowCollection<{schema_row_type}, {table_type}> {{\n    return this.{field};\n  }}\n\n"
            )
        })
        .collect()
}

fn ts_direct_row_specs(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> Vec<TsSchemaRow> {
    let row_specs = ts_schema_rows(unit);
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

fn ts_direct_default_row_spec(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> Option<TsSchemaRow> {
    let row_specs = ts_direct_row_specs(unit, manager);
    let row_types = row_specs
        .iter()
        .map(|row| row.source_row_type.clone())
        .collect::<Vec<_>>();
    let default_row_type = default_direct_manager_row_type(&manager.manager_name, &row_types)?;
    row_specs
        .into_iter()
        .find(|row| row.source_row_type == default_row_type)
}

fn push_direct_row_family_types(
    _source: &mut String,
    _unit: &GameDataCompileUnit,
    _surfaces: &[ManagerSurface],
) {
}

fn ts_direct_primary_row_family_methods(
    manager: &DirectManagerSurface,
    row_spec: &TsSchemaRow,
) -> String {
    let row_type = &row_spec.type_name;
    let source_row_type = &row_spec.source_row_type;
    let field = ts_direct_row_field_name(source_row_type);
    let table_type = ts_direct_table_type_name(manager, source_row_type);
    format!(
        r#"  rows(): IterableIterator<RowEntry<{table_type}, {row_type}>> {{
    return this.{field}.rows();
  }}

  table(table: {table_type}): TableRows<{row_type}, {table_type}> {{
    return this.{field}.table(table);
  }}

  resolveRow(reference: TableReference): {row_type} | undefined {{
    const table = parse{table_type}(reference.path);
    return table === undefined ? undefined : this.{field}.table(table).get(reference.key);
  }}

  row(ref: RowRef<{table_type}, {row_type}>): {row_type} | undefined {{
    return this.{field}.get(ref);
  }}

  rowByIndex(slot: RowSlot<{table_type}, {row_type}>): {row_type} | undefined {{
    return this.{field}.rowByIndex(slot);
  }}

  rowKeyByIndex(slot: RowSlot<{table_type}, {row_type}>): string | undefined {{
    return this.{field}.rowKeyByIndex(slot);
  }}

  [Symbol.iterator](): Iterator<RowEntry<{table_type}, {row_type}>> {{
    return this.rows();
  }}

"#
    )
}

fn ts_direct_row_field_name(source_row_type: &str) -> String {
    format!("{}Collection", ts_method_name(source_row_type))
}

fn ts_product_info(
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
        NativeManagerProductKind::UiDatabase => ("UiDatabase", "parseUiDatabase"),
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

fn ts_product_storage(manager: &DirectManagerSurface) -> (String, String) {
    let mut fields = String::new();
    let mut initializers = String::new();
    let mut seen = BTreeSet::new();
    for product in &manager.products {
        let (_, type_name, parser) = ts_product_info(&product.value_type).unwrap_or_else(|| {
            panic!(
                "TypeScript manager {} has no typed product parser for {} ({})",
                manager.manager_name, product.value_type, product.path
            )
        });
        let field = ts_product_field_name(type_name);
        if !seen.insert(field.clone()) {
            continue;
        }
        fields.push_str(&format!("  private readonly {field}: {type_name};\n"));
        initializers.push_str(&format!(
            "    this.{field} = {parser}(resources.requiredAssetBytes({}));\n",
            typescript_string_literal(&product.path)
        ));
    }
    (fields, initializers)
}

fn ts_product_field_name(type_name: &str) -> String {
    format!("{}Value", ts_method_name(type_name))
}

fn direct_ts_product_methods(manager: &DirectManagerSurface) -> String {
    let mut source = String::new();
    for product in &manager.products {
        let getter = ts_method_name(&product.manager_getter);
        let (kind, type_name, _) = ts_product_info(&product.value_type).unwrap_or_else(|| {
            panic!(
                "TypeScript manager {} has no typed product API for {} ({})",
                manager.manager_name, product.value_type, product.path
            )
        });
        let field = ts_product_field_name(type_name);
        match kind {
            NativeManagerProductKind::ArmorOffsetDatabase => {
                source.push_str(&format!(
                    r#"  {getter}(): ArmorOffsetDatabase {{
    return this.{field};
  }}

  armorOffset(name: string): ArmorOffsetData | undefined {{
    return armorOffsetByName(this.{getter}(), name);
  }}

  furthestAttachmentOffset(
    armorOffsetNames: readonly string[],
    attachmentName: string,
    currentPosition: Vector3 = Vector3.ZERO,
  ): AttachmentOffsetData | undefined {{
    return furthestArmorAttachmentOffset(
      this.{getter}(),
      armorOffsetNames,
      attachmentName,
      currentPosition,
    );
  }}

"#,
                ));
            }
            NativeManagerProductKind::EquipTypesDatabase => {
                source.push_str(&format!(
                    r#"  {getter}(): EquipTypesDatabase {{
    return this.{field};
  }}

  equipTypes(): readonly EquipTypeData[] {{
    return this.{getter}().equipTypes;
  }}

"#,
                ));
            }
            NativeManagerProductKind::GameDebugSettings => {
                source.push_str(&format!(
                    r#"  {getter}(): GameDebugSettings {{
    return this.{field};
  }}

  combat(): CombatDebugSettings {{
    return this.{getter}().combatSettings;
  }}

  disabledCombatToggleCount(): number {{
    return disabledCombatToggleCount(this.combat());
  }}

"#,
                ));
            }
            NativeManagerProductKind::PlayerBaseAttributes => {
                source.push_str(&format!(
                    r#"  {getter}(): PlayerBaseAttributes {{
    return this.{field};
  }}

  playerAttributeData(): PlayerAttributeData {{
    return this.{getter}().playerAttributeData;
  }}

  maxPerks(rarityLevel: number): number | undefined {{
    return this.{getter}().playerAttributeData.itemRarityData[rarityLevel]?.maxPerkCount;
  }}

"#,
                ));
            }
            NativeManagerProductKind::SettlementProgressionData => {
                source.push_str(&format!(
                    r#"  {getter}(): SettlementProgressionData {{
    return this.{field};
  }}

  settlementProgressionCategories(): readonly ProgressionCategoryEntry[] {{
    return this.{getter}().settlementProgressionCategories;
  }}

"#,
                ));
            }
            NativeManagerProductKind::UiDatabase => {
                source.push_str(&format!(
                    r#"  private interactOptionsByNameCrc?: ReadonlyMap<Crc32, InteractOptionData>;

  {getter}(): UiDatabase {{
    return this.{field};
  }}

  interactOptions(): readonly InteractOptionData[] {{
    return this.{getter}().unifiedInteractData.interactOptions;
  }}

  interactOption(id: string | Crc32): InteractOptionData | undefined {{
    const key = typeof id === "number" ? id : crc32Lowercase(id);
    this.interactOptionsByNameCrc ??= indexInteractOptionsByNameCrc(this.interactOptions());
    return this.interactOptionsByNameCrc.get(key);
  }}

  *interactOptionsByCategory(category: number): IterableIterator<InteractOptionData> {{
    for (const option of this.interactOptions()) {{
      if (
        option.interactOptionCategory === category ||
        option.interactOptionCategory === ALL_INTERACT_OPTIONS_CATEGORY
      ) {{
        yield option;
      }}
    }}
  }}

"#,
                ));
            }
            NativeManagerProductKind::GameCameraSettings => {
                source.push_str(&format!(
                    r#"  {getter}(): GameCameraSettings {{
    return this.{field};
  }}

  cameraStates(): readonly CameraStateSettings[] {{
    return this.{getter}().cameraStates;
  }}

"#,
                ));
            }
            NativeManagerProductKind::GatheringDatabase => {
                source.push_str(&format!(
                    r#"  {getter}(): GatheringDatabase {{
    return this.{field};
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
                ));
            }
            NativeManagerProductKind::GatheringActionDatabase => {
                source.push_str(&format!(
                    r#"  {getter}(): GatheringActionDatabase {{
    return this.{field};
  }}

  gatheringActionData(): readonly GatheringActionData[] {{
    return this.{getter}().gatheringActions;
  }}

"#,
                ));
            }
            NativeManagerProductKind::CraftingStationDatabase => {
                source.push_str(&format!(
                    r#"  {getter}(): CraftingStationDatabase {{
    return this.{field};
  }}

  craftingStations(): readonly CraftingStationData[] {{
    return this.{getter}().craftingStations;
  }}

"#,
                ));
            }
            NativeManagerProductKind::SocialRankDatabase => {
                source.push_str(&format!(
                    r#"  {getter}(): SocialRankDatabase {{
    return this.{field};
  }}

  ranks(): readonly SocialRankData[] {{
    return this.{getter}().ranks;
  }}

"#,
                ));
            }
        }
    }
    source
}

fn push_product_backed_manager_class(source: &mut String, manager: &DirectManagerSurface) {
    let manager_class = &manager.manager_class_name;
    let manager_instance = ts_manager_instance_expression(
        &manager.manager_name,
        manager
            .tables
            .iter()
            .map(|table| (table.table_name.as_str(), table.row_type_name.as_str())),
        manager.products.iter().map(|product| product.path.as_str()),
    );
    let mut product_methods = direct_ts_product_methods(manager);
    product_methods.push_str(&special_ts_manager_extra_methods(manager_class));
    let (product_fields, product_initializers) = ts_product_storage(manager);
    let constructor = if product_fields.is_empty() {
        "private constructor(_resources: ManagerResources) {}".to_owned()
    } else {
        format!("private constructor(resources: ManagerResources) {{\n{product_initializers}  }}")
    };
    source.push_str(&format!(
        r#"
export class {manager_class} {{
{product_fields}
  {constructor}

  static [CREATE_MANAGER](cache: ManagerCache): {manager_class} {{
    return new {manager_class}({manager_instance});
  }}

{product_methods}
}}

"#
    ));
}

fn push_item_data_manager_class(source: &mut String, manager: &ItemDataManagerSurface) {
    let manager_class = &manager.manager_class_name;
    let manager_instance = ts_manager_instance_expression(
        &manager.manager_name,
        manager
            .tables
            .iter()
            .map(|table| (table.table_name.as_str(), table.row_type_name.as_str())),
        std::iter::empty(),
    );
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
        .map(|table| {
            format!(
                "  {{ table: {table_type}.{}, selector: {{ name: {}, rowType: {} }} }},\n",
                table.variant_name,
                typescript_string_literal(&table.table_name),
                typescript_string_literal(&table.row_type_name)
            )
        })
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
  readonly definition: MasterItemDefinitionsSchemaRow;
  readonly itemId: string;
  readonly itemIdCrc: Crc32;
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

const ITEM_DATA_MANAGER_TABLES: readonly {{
  readonly table: {table_type};
  readonly selector: TableSelector;
}}[] = [
{table_list}];

export class {manager_class} implements RowLookup<string, {data_type}> {{
  private readonly rowsCache: readonly {data_type}[];
  private readonly rowsById = new Map<Crc32, {data_type}>();

  private constructor(resources: ManagerResources) {{
    this.rowsCache = materialize{manager_class}(resources);
    for (const row of this.rowsCache) {{
      this.rowsById.set(row.itemIdCrc, row);
    }}
  }}

  static [CREATE_MANAGER](cache: ManagerCache): {manager_class} {{
    return new {manager_class}({manager_instance});
  }}

  get(itemId: string): {data_type} | undefined {{
    return this.getFromId(Crc32.fromStringLower(itemId));
  }}

  getFromId(itemId: Crc32): {data_type} | undefined {{
    return this.rowsById.get(itemId);
  }}

  byIndex(index: number): {data_type} | undefined {{
    if (!Number.isInteger(index) || index <= 0) {{
      return undefined;
    }}
    return this.rowsCache[index - 1];
  }}

  rows(): IterableIterator<{data_type}> {{
    return this.rowsCache.values();
  }}

  [Symbol.iterator](): Iterator<{data_type}> {{
    return this.rows();
  }}

  len(): number {{
    return this.rowsCache.length;
  }}

  isEmpty(): boolean {{
    return this.rowsCache.length === 0;
  }}
}}

function materialize{manager_class}(resources: ManagerResources): {data_type}[] {{
  const items: {data_type}[] = [];
  const seen = new Set<Crc32>();
  for (const {{ table: tableName, selector }} of ITEM_DATA_MANAGER_TABLES) {{
    const table = resources.table(selector);
    if (table === undefined) {{
      throw new Error(`manager {manager_class} table ${{tableName}} was not loaded`);
    }}
    cache{manager_class}Rows(items, seen, tableName, table);
  }}
  return items;
}}

function cache{manager_class}Rows(
  items: {data_type}[],
  seen: Set<Crc32>,
  tableName: {table_type},
  table: DynamicTable,
): void {{
  for (const sourceRow of table.rows) {{
    const definition = readMasterItemDefinitionsSchemaRow(table, sourceRow);
    const itemId = definition.itemId.trim();
    if (itemId.length === 0) {{
      continue;
    }}
    const itemIdCrc = Crc32.fromStringLower(itemId);
    if (itemIdCrc === 0 || seen.has(itemIdCrc)) {{
      continue;
    }}
    seen.add(itemIdCrc);
    items.push({{
      sourceHandle: {{
        table: tableName,
        row: sourceRow.rowIndex + 1,
      }},
      definition,
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
    let manager_instance = ts_manager_instance_expression(
        &record.manager_name,
        record
            .tables
            .iter()
            .map(|table| (table.table_name.as_str(), table.row_type_name.as_str())),
        std::iter::empty(),
    );
    let entries_field = "rowsCache";
    let by_key_field = "rowsByKey";
    let source_row_field = "rowsBySourceRow";
    let key_map_type = ts_key_map_type(record);
    let has_lookup_index = !record.lookup_methods.is_empty();
    assert!(
        !has_lookup_index || record.key.is_some(),
        "{manager_class} exposes key lookups without a semantic key"
    );
    let source_row_index_field = record.source_row_method.as_ref().map(|_| {
        record
            .source_row_field
            .as_ref()
            .expect("source-row lookup methods require a source-row field")
    });
    source.push_str(&format!(
        r#"
export class {manager_class} implements Rows<{record_type}> {{
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

  private constructor(resources: ManagerResources) {{
    this.{entries_field} = materialize{manager_class}(resources);
"#
    ));
    if has_lookup_index || source_row_index_field.is_some() {
        source.push_str(&format!("    for (const row of this.{entries_field}) {{\n"));
    }
    if has_lookup_index {
        let index_expression = ts_row_index_expression(record)
            .expect("semantic manager key index requires a semantic key");
        source.push_str(&format!(
            "      this.{by_key_field}.set({index_expression}, row);\n"
        ));
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

  static [CREATE_MANAGER](cache: ManagerCache): {manager_class} {{
    return new {manager_class}({manager_instance});
  }}

"#
    ));
    for method in &record.lookup_methods {
        let method_name = ts_method_name(&method.name);
        let parameter_name = ts_field_name(&method.parameter);
        match method.kind {
            SemanticLookupKind::CrcString => source.push_str(&format!(
                r#"  {method_name}({parameter_name}: string): {record_type} | undefined {{
    return this.{by_key_field}.get(Crc32.fromStringLower({parameter_name}));
  }}

"#
            )),
            SemanticLookupKind::Crc => source.push_str(&format!(
                r#"  {method_name}({parameter_name}: Crc32): {record_type} | undefined {{
    return this.{by_key_field}.get({parameter_name});
  }}

"#
            )),
            SemanticLookupKind::IntoCrc => source.push_str(&format!(
                r#"  {method_name}({parameter_name}: string | Crc32): {record_type} | undefined {{
    return this.{by_key_field}.get(crc32LookupKey({parameter_name}));
  }}

"#
            )),
            SemanticLookupKind::Numeric(_) => source.push_str(&format!(
                r#"  {method_name}({parameter_name}: number): {record_type} | undefined {{
    return this.{by_key_field}.get(normalizeNumericKey({parameter_name}));
  }}

"#
            )),
            SemanticLookupKind::String => source.push_str(&format!(
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
            r#"  *{method_name}(): IterableIterator<{id_type}> {{
    for (const row of this.{entries_field}) {{
      yield {id_expression};
    }}
  }}

"#
        ));
    }
    source.push_str(&format!(
        r#"  rows(): IterableIterator<{record_type}> {{
    return this.{entries_field}.values();
  }}

"#
    ));
    if let Some(method) = &record.rows_method {
        let method_name = ts_method_name(method);
        if method_name != "rows" {
            source.push_str(&format!(
                r#"  {method_name}(): IterableIterator<{record_type}> {{
    return this.rows();
  }}

"#
            ));
        }
    }
    source.push_str(&format!(
        r#"  [Symbol.iterator](): Iterator<{record_type}> {{
    return this.rows();
  }}

"#
    ));
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
    push_semantic_materializer(source, record);
}

fn special_ts_manager_extra_methods(manager_class_name: &str) -> String {
    match manager_class_name {
        "PlayerDataManager" => {
            r#"  categoricalProgressionId(tradeskill: TradeskillType): Crc32 | undefined {
    if (tradeskill === "None" || tradeskill === "WildernessSurvival") {
      return undefined;
    }
    return Crc32.fromStringLower(tradeskill);
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
        Some(SemanticManagerKey::Crc { .. } | SemanticManagerKey::FallbackCrc { .. }) => "Crc32",
        Some(SemanticManagerKey::Numeric { .. }) => "number",
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
        Some(SemanticManagerKey::Crc { .. } | SemanticManagerKey::FallbackCrc { .. }) => "Crc32",
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
        r#"function materialize{manager_class}(resources: ManagerResources): readonly {record_type}[] {{
  const rows: {record_type}[] = [];
"#
    ));
    if record.key.is_some() {
        source.push_str("  const seen = new Set<string | number>();\n");
    }
    source.push_str(
        r#"  for (const table of resources) {
    for (const sourceRow of table.rows) {
"#,
    );
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
    for field in &record.fields {
        let local = ts_projection_local_name(&field.name);
        if matches!(
            field.transform,
            SemanticProjectionTransform::EnumStringSkipInvalid
                | SemanticProjectionTransform::EnumStringRejectDefault
        ) {
            let enum_type = semantic_enum_type_name(field);
            source.push_str(&format!(
                "      let {local}: {enum_type};\n      try {{\n        {local} = {};\n      }} catch {{\n        continue;\n      }}\n",
                ts_projection_value(field)
            ));
        } else {
            source.push_str(&format!(
                "      const {local} = {};\n",
                ts_projection_value(field)
            ));
        }
    }
    for field in &record.fields {
        let local = ts_projection_local_name(&field.name);
        match field.transform {
            SemanticProjectionTransform::NonEmptyString => source.push_str(&format!(
                "      if ({local}.length === 0) {{\n        continue;\n      }}\n"
            )),
            SemanticProjectionTransform::NonEmptyStringList => source.push_str(&format!(
                "      if ({local}.length === 0) {{\n        continue;\n      }}\n"
            )),
            SemanticProjectionTransform::EnumStringRejectDefault => {
                let enum_type = semantic_enum_type_name(field);
                let default = semantic_enum_default_variant(field);
                source.push_str(&format!(
                    "      if ({local} === {enum_type}.{}) {{\n        continue;\n      }}\n",
                    to_upper_camel_ident(default, "Variant")
                ));
            }
            _ => {}
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
            ts_projection_local_name(&field.name)
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

fn ts_projection_local_name(field_name: &str) -> String {
    format!("{}Value", ts_field_name(field_name))
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
            let column = typescript_string_literal(key_column);
            if *skip_empty_key {
                source.push_str(&format!(
                    "      const keyText = optionalStringCell(table, sourceRow, {column});\n      if (keyText === null) {{\n        continue;\n      }}\n"
                ));
            } else {
                source.push_str(&format!(
                    "      const keyText = requiredStringCell(table, sourceRow, {column});\n"
                ));
            }
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
            source.push_str("      const keyCrc = Crc32.fromStringLower(keyValue);\n");
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
                r#"      const keyCrc = Crc32.fromStringLower(keyValue);
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
            let column = typescript_string_literal(key_column);
            if *skip_empty_key {
                source.push_str(&format!(
                    "      const keyText = optionalStringCell(table, sourceRow, {column});\n      if (keyText === null) {{\n        continue;\n      }}\n"
                ));
            } else {
                source.push_str(&format!(
                    "      const keyText = requiredStringCell(table, sourceRow, {column});\n"
                ));
            }
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
            let column = typescript_string_literal(key_column);
            if *skip_empty_key {
                source.push_str(&format!(
                    "      const keyValue = optionalStringCell(table, sourceRow, {column});\n      if (keyValue === null) {{\n        continue;\n      }}\n"
                ));
            } else {
                source.push_str(&format!(
                    "      const keyValue = requiredStringCell(table, sourceRow, {column});\n"
                ));
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
        SemanticProjectionTransform::String | SemanticProjectionTransform::NonEmptyString => {
            format!("requiredStringCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::EnumString
        | SemanticProjectionTransform::EnumStringSkipInvalid
        | SemanticProjectionTransform::EnumStringRejectDefault => {
            format!(
                "parse{}(requiredStringCell(table, sourceRow, {column}))",
                semantic_enum_type_name(field)
            )
        }
        SemanticProjectionTransform::EnumDefault => {
            let enum_type = semantic_enum_type_name(field);
            let default = to_upper_camel_ident(semantic_enum_default_variant(field), "Variant");
            format!(
                "enumCellOr(table, sourceRow, {column}, {enum_type}.{default}, parse{enum_type})"
            )
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
        SemanticProjectionTransform::OptionalFirstString => {
            format!("optionalStringListCell(table, sourceRow, {column})?.[0] ?? null")
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
        SemanticProjectionTransform::BoolDefaultFalse => {
            format!("optionalBoolCell(table, sourceRow, {column}) ?? false")
        }
        SemanticProjectionTransform::Crc32NonZeroBool => {
            let reference = field
                .reference_field
                .as_deref()
                .expect("CRC presence projections have reference fields");
            format!("{} !== Crc32.ZERO", ts_projection_local_name(reference))
        }
        SemanticProjectionTransform::U8 => {
            format!("requiredUint8Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::NonZeroU8 => {
            format!("requiredNonZeroUint8Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U8DefaultZero => {
            format!("normalizeUint8(optionalNumberCell(table, sourceRow, {column}) ?? 0)")
        }
        SemanticProjectionTransform::U8DefaultMax => {
            format!("normalizeUint8(optionalNumberCell(table, sourceRow, {column}) ?? 0xff)")
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
                "(() => {{ const value = requiredUint16Cell(table, sourceRow, {column}); if (value >= {max}) throw new Error(`row ${{sourceRow.sourcePath}}:${{sourceRow.rowIndex + 1}} {column} exceeds supported cap {max}`); return value; }})()"
            )
        }
        SemanticProjectionTransform::U32 => {
            format!("requiredUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalU32 => {
            format!("optionalUint32Cell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U32DefaultZero => {
            format!("optionalUint32Cell(table, sourceRow, {column}) ?? 0")
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
            format!("requiredNumberCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalF32 => {
            format!("optionalNumberCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::F32MinutesToSeconds => {
            format!("requiredNumberCell(table, sourceRow, {column}) * 60")
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
                ts_projection_local_name(reference)
            )
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
        SemanticProjectionTransform::LowercaseCrcString => {
            format!("Crc32.fromStringLower(requiredStringCell(table, sourceRow, {column}))")
        }
        SemanticProjectionTransform::LowercaseCrcStringDefaultZero => {
            format!("Crc32.fromStringLower(optionalStringCell(table, sourceRow, {column}) ?? \"\")")
        }
        SemanticProjectionTransform::FirstLowercaseCrcStringDefaultZero => {
            format!(
                "Crc32.fromStringLower(optionalStringListCell(table, sourceRow, {column})?.[0] ?? \"\")"
            )
        }
        SemanticProjectionTransform::TrimmedLowercaseCrcStringDefaultZero => {
            format!(
                "Crc32.fromStringLower((optionalStringCell(table, sourceRow, {column}) ?? \"\").trim())"
            )
        }
        SemanticProjectionTransform::OptionalCrc32 => {
            format!("optionalCrc32Cell(table, sourceRow, {column})")
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
            format!(
                "stringListCell(table, sourceRow, {column}).filter((value) => value.length > 0).map((value) => Crc32.fromStringLower(value))"
            )
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
            format!("numberRangeCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::U32RangeInclusive => {
            format!("uint32RangeCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalCrc32F32PairList => {
            format!("optionalCrc32Float32PairListCell(table, sourceRow, {column})")
        }
        SemanticProjectionTransform::OptionalU8F32PairList => {
            let enum_shape = field
                .pair_first_enum_shape
                .as_ref()
                .expect("u8 pair-list projections have a reconciled enum schema");
            let parser = ts_pair_enum_parser_name(&enum_shape.name);
            format!("optionalUint8Float32PairListCell(table, sourceRow, {column}, {parser})")
        }
    }
}

const SEMANTIC_MANAGER_RUNTIME_TS: &str = r#"
function enumCellOr<T>(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
  fallback: T,
  parse: (source: string) => T,
): T {
  const value = optionalStringCell(table, row, columnName);
  return value === null ? fallback : parse(value);
}

function rowCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): DatasheetCellValue | undefined {
  const columnCrc = table.columnCrcs.get(columnName);
  if (columnCrc === undefined) {
    return undefined;
  }
  const slot = row.columnSlots.get(columnCrc);
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
  throw new Error(
    `row ${row.sourcePath}:${row.rowIndex + 1} has non-number ${columnName}=${JSON.stringify(value.value)}`,
  );
}

function requiredUint8Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  return normalizeUint8(requiredUint32Cell(table, row, columnName));
}

function requiredUint16Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  return normalizeUint16(requiredUint32Cell(table, row, columnName));
}

function requiredNonZeroUint8Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  return requireNonZero(requiredUint8Cell(table, row, columnName), row, columnName);
}

function requiredNonZeroUint16Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  return requireNonZero(requiredUint16Cell(table, row, columnName), row, columnName);
}

function requiredUint32Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  return normalizeUint32(requiredNumberCell(table, row, columnName));
}

function requiredNonZeroUint32Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number {
  return requireNonZero(requiredUint32Cell(table, row, columnName), row, columnName);
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
): Crc32 {
  const value = rowCell(table, row, columnName);
  if (value?.kind === "number") {
    return Crc32.from(value.value);
  }
  if (value?.kind === "string") {
    return Crc32.fromStringLower(value.value);
  }
  throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing crc ${columnName}`);
}

function optionalCrc32Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
  zeroAsNone = false,
): Crc32 | null {
  const value = rowCell(table, row, columnName);
  if (value === undefined || value.kind === "boolean") {
    return null;
  }
  if (value.kind === "string" && value.value.length === 0) {
    return null;
  }
  const crc = value.kind === "number" ? Crc32.from(value.value) : Crc32.fromStringLower(value.value);
  return zeroAsNone && crc === Crc32.ZERO ? null : crc;
}

function optionalLowercaseCrcStringCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): Crc32 | null {
  const value = optionalStringCell(table, row, columnName);
  return value === null ? null : Crc32.fromStringLower(value);
}

function optionalTrimmedLowercaseCrcStringCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): Crc32 | null {
  const value = optionalStringCell(table, row, columnName)?.trim();
  return value === undefined || value.length === 0 ? null : Crc32.fromStringLower(value);
}

function upperBoundCell(table: DynamicTable, row: DynamicTableRow, columnName: string): number {
  const value = requiredNumberCell(table, row, columnName);
  if (Number.isNaN(value) || Math.abs(value) <= 1.1920928955078125e-7) {
    return 10_000;
  }
  return Math.min(Math.max(value, 0), 10_000);
}

function lowerBoundCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
  upperBound: number,
): number {
  const value = requiredNumberCell(table, row, columnName);
  const bounded = Number.isNaN(value) ? 0 : Math.min(Math.max(value, 0), 10_000);
  return Math.min(bounded, upperBound);
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
): Crc32[] {
  const value = rowCell(table, row, columnName);
  if (value === undefined) {
    return [];
  }
  if (value.kind === "number") {
    return [Crc32.from(value.value)];
  }
  if (value.kind !== "string") {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has non-crc-list ${columnName}`);
  }
  return splitDesignerList(value.value).map((part) => {
    const number = Number(part);
    return Number.isFinite(number) ? Crc32.from(number) : Crc32.fromStringLower(part);
  });
}

function optionalCrc32Float32PairListCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): ReadonlyArray<readonly [Crc32, number]> | null {
  return optionalPairListCell(table, row, columnName, (source) => {
    const numeric = Number(source);
    return Number.isInteger(numeric) && numeric >= 0 && numeric <= 0xffffffff
      ? Crc32.from(numeric)
      : Crc32.fromStringLower(source);
  });
}

function optionalUint8Float32PairListCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
  parseFirst: (source: string) => number,
): ReadonlyArray<readonly [number, number]> | null {
  return optionalPairListCell(table, row, columnName, parseFirst);
}

function optionalPairListCell<T>(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
  parseFirst: (source: string) => T,
): ReadonlyArray<readonly [T, number]> | null {
  const value = rowCell(table, row, columnName);
  if (
    value === undefined ||
    (value.kind === "number" && value.value === 0) ||
    (value.kind === "boolean" && !value.value) ||
    (value.kind === "string" && value.value.trim().length === 0)
  ) {
    return null;
  }
  if (value.kind !== "string") {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has non-pair-list ${columnName}`);
  }
  const pairs = splitDesignerList(value.value).map((entry): readonly [T, number] => {
    const separator = entry.indexOf("=");
    if (separator < 0) {
      throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has invalid pair in ${columnName}`);
    }
    const first = entry.slice(0, separator).trim();
    const second = entry.slice(separator + 1).trim();
    return [parseFirst(first), parseDesignerNumber(second, row, columnName)];
  });
  return pairs.length === 0 ? null : pairs;
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
  if (value.kind === "number") {
    return [value.value, value.value];
  }
  if (value.kind === "boolean") {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has non-number range ${columnName}`);
  }
  return floatRangeFromText(value.value);
}

function uint32RangeCell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): [number, number] {
  const value = rowCell(table, row, columnName);
  if (value === undefined || value.kind === "boolean") {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} missing unsigned range ${columnName}`);
  }
  if (value.kind === "number") {
    const endpoint = normalizeUint32(value.value);
    return [endpoint, endpoint];
  }
  const parts = value.value.trim().split("-").map((part) => part.trim());
  if (parts.length === 1 && parts[0].length > 0) {
    const endpoint = normalizeUint32(Number(parts[0]));
    return [endpoint, endpoint];
  }
  if (parts.length === 2 && parts.every((part) => part.length > 0)) {
    return [normalizeUint32(Number(parts[0])), normalizeUint32(Number(parts[1]))];
  }
  throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} has invalid unsigned range ${columnName}`);
}

function floatRangeFromText(value: string): [number, number] {
  const parts = value.trim().split("-").map((part) => part.trim());
  if (parts.length === 1) {
    const endpoint = Number(parts[0]);
    return Number.isFinite(endpoint) ? [endpoint, endpoint] : [0, 0];
  }
  if (parts.length === 2) {
    const first = Number(parts[0]);
    const second = Number(parts[1]);
    if (Number.isFinite(first) && Number.isFinite(second)) {
      return first <= second ? [first, second] : [second, first];
    }
  }
  return [0, 0];
}

function splitDesignerList(value: string): string[] {
  return value
    .split(/[,+]/)
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
}

function normalizeLookupText(value: string): string {
  return value.trim().toLowerCase();
}


function tableCrcLookupKey(table: string, id: Crc32): string {
  return `${table}\u0000${id}`;
}

function abilityPositionLookupKey(table: string, position: number): string {
  return `${table}\u0000${normalizeUint16(position)}`;
}

function abilityCoordinate(
  value: number | string | null,
): number | null {
  const coordinate = optionalSchemaNumber(value);
  if (coordinate === null) return null;
  if (!Number.isInteger(coordinate) || coordinate < 0 || coordinate > 0xff) {
    return null;
  }
  return coordinate;
}

function tableNumberLookupKey(table: string, value: number): string {
  return `${table}\u0000${value}`;
}

function tableCrcTextLookupKey(table: string, id: Crc32, text: string): string {
  return `${table}\u0000${id}\u0000${text}`;
}

function crcNumberLookupKey(id: Crc32, value: number): string {
  return `${id}\u0000${value}`;
}

function crcPairLookupKey(left: Crc32, right: Crc32): string {
  return `${left}\u0000${right}`;
}

function crcTextNumberLookupKey(id: Crc32, text: string, value: number): string {
  return `${id}\u0000${text}\u0000${value}`;
}

function seasonIdFromTable(table: string): Crc32 {
  const separator = table.lastIndexOf("_");
  return separator < 0 || separator + 1 === table.length
    ? Crc32.ZERO
    : Crc32.fromStringLower(table.slice(separator + 1));
}

function appendMapValue<Key, Value>(map: Map<Key, Value[]>, key: Key, value: Value): void {
  const values = map.get(key);
  if (values === undefined) map.set(key, [value]);
  else values.push(value);
}

function appendUniqueMapValue<Key, Value>(map: Map<Key, Value[]>, key: Key, value: Value): void {
  const values = map.get(key);
  if (values === undefined) map.set(key, [value]);
  else if (!values.includes(value)) values.push(value);
}

function floorInSorted(values: readonly number[], target: number): number | undefined {
  if (values.length === 0) return undefined;
  let low = 0;
  let high = values.length;
  while (low < high) {
    const middle = low + Math.floor((high - low) / 2);
    if (values[middle] <= target) low = middle + 1;
    else high = middle;
  }
  return values[Math.max(0, low - 1)];
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

function normalizeUint8(value: number): number {
  const normalized = normalizeUint32(value);
  if (normalized > 0xff) {
    throw new RangeError(`expected u8, got ${value}`);
  }
  return normalized;
}

function normalizeUint16(value: number): number {
  const normalized = normalizeUint32(value);
  if (normalized > 0xffff) {
    throw new RangeError(`expected u16, got ${value}`);
  }
  return normalized;
}

function requireNonZero(value: number, row: DynamicTableRow, columnName: string): number {
  if (value === 0) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} ${columnName} must be non-zero`);
  }
  return value;
}

function normalizeInt32(value: number): number {
  if (!Number.isInteger(value) || value < -0x80000000 || value > 0x7fffffff) {
    throw new Error(`expected i32, got ${value}`);
  }
  return value | 0;
}

function crc32LookupKey(value: string | Crc32): Crc32 {
  return typeof value === "number" ? value : Crc32.fromStringLower(value);
}

function normalizeNumericKey(value: number): number {
  return normalizeUint32(value);
}

function normalizeUnsignedInteger(value: number): number {
  return normalizeUint32(value);
}

function normalizeOptionalUnsignedInteger(value: number | null): number {
  return value === null ? 0 : normalizeUnsignedInteger(value);
}

function normalizeOptionalPositiveInteger(value: number | null): number | undefined {
  const normalized = normalizeOptionalUnsignedInteger(value);
  return normalized === 0 ? undefined : normalized;
}

function compareNumberPairs(
  left: readonly [number, number],
  right: readonly [number, number],
): number {
  return left[0] - right[0] || left[1] - right[1];
}

function tradeskillRankLookupKey(table: string, rank: number): string {
  return `${normalizeStringKey(table)}:${normalizeUnsignedInteger(rank)}`;
}

function damageReferenceLookupKey(reference: DamageDataReference): string {
  return `${normalizeStringKey(reference.table)}:${reference.id}`;
}

function damageSlotLookupKey(slot: DamageDataSlot): string {
  return `${normalizeStringKey(slot.table)}:${normalizeUnsignedInteger(slot.rowIndex)}`;
}

function nonEmptyString(value: string | null): string | null {
  const normalized = value?.trim() ?? "";
  return normalized.length === 0 ? null : normalized;
}

function optionalSchemaNumber(value: number | string | null): number | null {
  if (value === null) return null;
  if (typeof value === "string" && value.trim().length === 0) return null;
  const number = typeof value === "number" ? value : Number(value.trim());
  return Number.isFinite(number) ? number : null;
}

function schemaBoolean(value: boolean | number | string | null, fallback: boolean): boolean {
  if (value === null) return fallback;
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  switch (value.trim().toLowerCase()) {
    case "true":
    case "1":
    case "yes":
      return true;
    case "false":
    case "0":
    case "no":
    case "":
      return false;
    default:
      return fallback;
  }
}

function requiredSchemaNumber(
  value: number | string | null,
  field: string,
  ref: { readonly table: string; readonly key: string },
): number {
  const number = optionalSchemaNumber(value);
  if (number === null) throw new Error(`table ${ref.table} row ${ref.key} requires numeric ${field}`);
  return number;
}

function normalizeStringKey(value: string): string {
  return value.trim().toLowerCase();
}

function crc32Lowercase(value: string): Crc32 {
  return Crc32.fromStringLower(value);
}

"#;

const OPTIONAL_UINT32_CELL_TS: &str = r#"
function optionalUint32Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number | null {
  const value = optionalNumberCell(table, row, columnName);
  return value === null ? null : normalizeUint32(value);
}
"#;

const OPTIONAL_NON_ZERO_UINT32_CELL_TS: &str = r#"
function optionalNonZeroUint32Cell(
  table: DynamicTable,
  row: DynamicTableRow,
  columnName: string,
): number | null {
  const value = optionalUint32Cell(table, row, columnName);
  if (value === 0) {
    throw new Error(`row ${row.sourcePath}:${row.rowIndex + 1} ${columnName} must be non-zero`);
  }
  return value;
}
"#;

const PRODUCT_MANAGER_RUNTIME_TS: &str = r#"
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
  readonly position: Vector3;
  readonly rotationDegrees: Vector3;
}

export interface EquipTypesDatabase {
  readonly equipTypes: readonly EquipTypeData[];
}

export interface EquipTypeData {
  readonly name: string;
  readonly attachment: string;
  readonly attachmentOffsetPosition: Vector3;
  readonly attachmentOffsetRotationDegrees: Vector3;
  readonly sheathData: string;
  readonly sheathOffsetPosition: Vector3;
  readonly sheathOffsetRotationDegrees: Vector3;
  readonly offHandAttachment: string;
  readonly offHandAttachmentOffsetPosition: Vector3;
  readonly offHandAttachmentOffsetRotationDegrees: Vector3;
  readonly offHandSheathData: string;
  readonly offHandSheathOffsetPosition: Vector3;
  readonly offHandSheathOffsetRotationDegrees: Vector3;
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
  readonly valueCrc: Crc32;
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
  readonly craftingResultLootBucketId: Crc32;
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
  readonly attributePerkBucketId: Crc32;
}

export interface GuildSiegeWindowRegionData {
  readonly startHour: number;
  readonly endHour: number;
  readonly utcOffset: number;
  readonly dstRuleId: Crc32;
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
  readonly deployableLimits: ReadonlyMap<Crc32, WarDeployableLimitData>;
}

export interface WarDeployableLimitData {
  readonly id: Crc32;
  readonly displayName: string;
  readonly buildableNames: readonly string[];
  readonly buildableIds: readonly Crc32[];
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

const PRODUCT_TEXT_DECODER = new TextDecoder();

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
  const deployableLimits = new Map<Crc32, WarDeployableLimitData>();
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
  currentPosition: Vector3,
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
): ReadonlyMap<Crc32, InteractOptionData> {
  const out = new Map<Crc32, InteractOptionData>();
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

function requiredVec3Field(element: ObjectStreamElement, nameCrc: number): Vector3 {
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

function requiredCrc32FieldByName(element: ObjectStreamElement, fieldName: string): Crc32 {
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
): readonly Crc32[] {
  return requiredFieldByName(element, fieldName).children.map(readCrc32);
}

function readStringVector(element: ObjectStreamElement): readonly string[] {
  return element.children.map((child) => {
    requireObjectStreamType(child, AZSTD_STRING_TYPE_ID);
    return objectStreamString(child);
  });
}

function readCrc32(element: ObjectStreamElement): Crc32 {
  requireObjectStreamType(element, CRC32_TYPE_ID);
  if (element.data.length === 4) {
    return Crc32.from(objectStreamU32(element));
  }
  const value = requiredChildByNameCrc(element, crc32Lowercase("Value"));
  return Crc32.from(objectStreamU32(value));
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
      id: new AssetId(Uuid.fromBytes(data.slice(0, 16)), subId),
      assetType: Uuid.fromBytes(data.slice(candidate.assetTypeOffset, candidate.assetTypeOffset + 16)),
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

function vec3Length(value: Vector3): number {
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
interface TableSelector {
  readonly name: string;
  readonly rowType: string;
}

class ManagerResources {
  constructor(
    private readonly managerName: string,
    private readonly tablesByName: ReadonlyMap<string, ReadonlyMap<string, DynamicTable>>,
    private readonly tableOrder: readonly DynamicTable[],
    private readonly assets: ReadonlyMap<string, Uint8Array>,
  ) {}

  table(selector: TableSelector): DynamicTable | undefined {
    return this.tablesByName.get(selector.name)?.get(selector.rowType);
  }

  *[Symbol.iterator](): IterableIterator<DynamicTable> {
    yield* this.tableOrder;
  }

  private assetBytes(path?: string): Uint8Array | undefined {
    const requested = path ?? (this.assets.size === 1 ? this.assets.keys().next().value : undefined);
    if (requested === undefined) {
      return undefined;
    }
    const normalized = normalizeDataPath(requested);
    const exact = this.assets.get(normalized);
    if (exact !== undefined) {
      return exact;
    }
    for (const [candidate, bytes] of this.assets) {
      if (candidate.endsWith(`/${normalized}`)) {
        return bytes;
      }
    }
    return undefined;
  }

  requiredAssetBytes(path?: string): Uint8Array {
    const bytes = this.assetBytes(path);
    if (bytes === undefined) {
      throw new Error(`manager ${this.managerName} asset ${path ?? "<single>"} was not loaded`);
    }
    return bytes;
  }

  schemaFamilyEntries<T>(
    rowType: string,
    read: (table: DynamicTable, row: DynamicTableRow) => T,
  ): readonly ResolvedRowEntry<T>[] {
    const out: ResolvedRowEntry<T>[] = [];
    for (const table of this.tableOrder) {
      if (table.schema.rowType !== rowType) {
        continue;
      }
      for (const row of table.rows) {
        out.push({
          sourcePath: row.sourcePath,
          key: row.key,
          rowIndex: row.rowIndex,
          row: read(table, row),
        });
      }
    }
    return out;
  }

}

class ManagerCache {
  private readonly tableCache = new Map<string, DynamicTable>();
  private readonly assetsByPath = new Map<string, Uint8Array>();
  private readonly assetLoads = new Map<string, Promise<Uint8Array>>();

  constructor(
    private readonly loader: AssetLoader,
    private readonly tableSchemas: readonly TableSchema[],
  ) {}

  async prepare(
    managerName: string,
    selectors: readonly TableSelector[],
    assetPaths: readonly string[],
  ): Promise<void> {
    const paths = new Set<string>();
    for (const selector of selectors) {
      const matches = this.tableSchemas.filter(
        (table) => table.name === selector.name && table.rowType === selector.rowType,
      );
      if (matches.length !== 1) {
        throw new Error(
          matches.length === 0
            ? `manager ${managerName} uses unknown table ${selector.name}:${selector.rowType}`
            : `manager ${managerName} has duplicate table schema ${selector.name}:${selector.rowType}`,
        );
      }
      for (const source of matches[0].sources) {
        paths.add(source);
      }
    }
    for (const path of assetPaths) {
      paths.add(path);
    }
    await Promise.all(
      Array.from(paths)
        .sort()
        .map((path) => this.loadAsset(path)),
    );
  }

  resourcesForTables(
    managerName: string,
    selectors: readonly TableSelector[],
    assetPaths: readonly string[],
  ): ManagerResources {
    const schemas: TableSchema[] = [];
    for (const selector of selectors) {
      const matches = this.tableSchemas.filter(
        (table) => table.name === selector.name && table.rowType === selector.rowType,
      );
      if (matches.length === 0) {
        throw new Error(
          `manager ${managerName} uses unknown table ${selector.name}:${selector.rowType}`,
        );
      }
      if (matches.length !== 1) {
        throw new Error(
          `manager ${managerName} has duplicate table schema ${selector.name}:${selector.rowType}`,
        );
      }
      schemas.push(matches[0]);
    }
    return this.resourcesFromSchemas(managerName, schemas, assetPaths);
  }

  resourcesForRows(
    managerName: string,
    rowTypes: readonly string[],
    assetPaths: readonly string[],
  ): ManagerResources {
    for (const rowType of rowTypes) {
      if (!this.tableSchemas.some((table) => table.rowType === rowType)) {
        throw new Error(`manager ${managerName} uses unknown row type ${rowType}`);
      }
    }
    const requested = new Set(rowTypes);
    const schemas = this.tableSchemas.filter((table) => requested.has(table.rowType));
    return this.resourcesFromSchemas(managerName, schemas, assetPaths);
  }

  private resourcesFromSchemas(
    managerName: string,
    schemas: readonly TableSchema[],
    assetPaths: readonly string[],
  ): ManagerResources {
    const tablesByName = new Map<string, Map<string, DynamicTable>>();
    const tableOrder: DynamicTable[] = [];
    for (const schema of schemas) {
      const table = this.buildTable(schema);
      let rowsByType = tablesByName.get(schema.name);
      if (rowsByType === undefined) {
        rowsByType = new Map<string, DynamicTable>();
        tablesByName.set(schema.name, rowsByType);
      }
      rowsByType.set(schema.rowType, table);
      tableOrder.push(table);
    }

    const assets = new Map<string, Uint8Array>();
    for (const path of assetPaths) {
      assets.set(normalizeDataPath(path), this.requiredAssetBytes(path));
    }
    return new ManagerResources(managerName, tablesByName, tableOrder, assets);
  }

  private buildTable(schema: TableSchema): DynamicTable {
    const cacheKey = `${schema.name}:${schema.rowType}`;
    const cached = this.tableCache.get(cacheKey);
    if (cached !== undefined) {
      return cached;
    }

    const rowKeyColumn = schema.columns.find((column) => column.rowKey);
    const rows: DynamicTableRow[] = [];

    for (const sourcePath of schema.sources) {
      const sheet = parseDatasheet(this.requiredAssetBytes(sourcePath));
      if (rowKeyColumn === undefined) {
        if (sheet.rows.length !== 0) {
          throw new Error(`non-empty datasheet source ${sourcePath} has no row-key column`);
        }
        continue;
      }
      const columnSlots = columnSlotsForSheet(schema, sheet);
      const rowKeySlot = columnSlots.get(rowKeyColumn.crc);
      if (rowKeySlot === undefined) {
        throw new Error(`datasheet source ${sourcePath} missing row-key column ${rowKeyColumn.name}`);
      }
      for (const [rowIndex, row] of sheet.rows.entries()) {
        const keyCell = row.cells[rowKeySlot];
        const key = keyCell === undefined ? undefined : rowKeyValue(keyCell.value);
        if (key === undefined) {
          continue;
        }
        const dynamicRow: DynamicTableRow = {
          sourcePath: normalizeDataPath(sourcePath),
          rowIndex,
          key,
          row,
          columnSlots,
        };
        rows.push(dynamicRow);
      }
    }

    const table: DynamicTable = {
      schema,
      rows,
      columnCrcs: new Map(schema.columns.map((column) => [column.name, column.crc])),
    };
    this.tableCache.set(cacheKey, table);
    return table;
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

  private loadAsset(path: string): Promise<Uint8Array> {
    const normalized = normalizeDataPath(path);
    const loaded = this.assetsByPath.get(normalized);
    if (loaded !== undefined) {
      return Promise.resolve(loaded);
    }
    const pending = this.assetLoads.get(normalized);
    if (pending !== undefined) {
      return pending;
    }
    const load = this.loader.read(path).then((bytes) => {
      this.assetsByPath.set(normalized, bytes);
      this.assetLoads.delete(normalized);
      return bytes;
    }, (error: unknown) => {
      this.assetLoads.delete(normalized);
      throw error;
    });
    this.assetLoads.set(normalized, load);
    return load;
  }
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

function normalizeDataPath(path: string): string {
  return path.replace(/\\/g, "/").replace(/\/+/g, "/").toLowerCase();
}

function tablePathMatches(left: string, right: string): boolean {
  const normalizedLeft = normalizeDataPath(left);
  const normalizedRight = normalizeDataPath(right);
  return (
    normalizedLeft === normalizedRight ||
    normalizedLeft.endsWith(`/${normalizedRight}`) ||
    normalizedRight.endsWith(`/${normalizedLeft}`)
  );
}
"#;
