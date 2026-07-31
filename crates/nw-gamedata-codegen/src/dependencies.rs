//! GameData native table encoding helpers and dependency inference.
//!
//! `nw-extract gamedata` is a source bootstrap command, and
//! `paks extract --transform` imports dev/source payloads. Production
//! GameData products are built later by the normal asset processor/package
//! pipeline.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use gamedata::{DEPENDENCY_KIND_FOREIGN_KEY, TableDependency};
use gamedata::{GameDataDependency, GameDataDependencyKind, RowIndex, SchemaHash};
#[cfg(test)]
use nw_datasheet::ColumnType;
#[cfg(test)]
use nw_datasheet::game_system::{
    GameSystemAsset, GameSystemCell, GameSystemColumn, OwnedCellValue,
};
use nw_datasheet::game_system::{GameSystemDataTables as GameSystemCatalog, GameSystemTable};

use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape,
    GameSystemDataTablesSchemaReport as GameSystemCatalogSchemaReport,
    GameSystemForeignKeyCandidate, GameSystemTableSchema,
};

use crate::table::{
    SchemaByTable, column_has_list, is_typed_foreign_key_candidate, schema_by_table,
    schema_for_table, schema_hash, sorted_tables,
};

const FOREIGN_KEY_CONFIDENCE_THRESHOLD: f64 = 0.80;

#[derive(Debug)]
struct ForeignKeyLookup {
    target_table_name_crc: u32,
    target_schema_hash: SchemaHash,
}

fn datasheet_list_entries<'a>(
    column: &GameSystemColumnSchema,
    value: &'a str,
) -> Result<Vec<&'a str>> {
    let GameSystemColumnValueShape::String {
        list: Some(list), ..
    } = &column.value_shape
    else {
        return Err(anyhow::anyhow!(
            "column {} is encoded as DataType::List without list shape metadata",
            column.name
        ));
    };
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    let separators = list
        .separators
        .iter()
        .filter_map(|separator| {
            let mut chars = separator.chars();
            let first = chars.next()?;
            chars.next().is_none().then_some(first)
        })
        .collect::<Vec<_>>();
    Ok(value
        .split(|character| separators.contains(&character))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect())
}

fn foreign_key_candidates(column: &GameSystemColumnSchema) -> Vec<&GameSystemForeignKeyCandidate> {
    let GameSystemColumnValueShape::String { foreign_keys, .. } = &column.value_shape else {
        return Vec::new();
    };
    let mut candidates = foreign_keys
        .iter()
        .filter(|candidate| candidate.confidence >= FOREIGN_KEY_CONFIDENCE_THRESHOLD)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.target_table.cmp(&right.target_table))
            .then_with(|| left.target_column.cmp(&right.target_column))
    });
    candidates
}

fn build_foreign_key_lookups(
    catalog: &GameSystemCatalog,
    table: &GameSystemTable,
    schema: &GameSystemTableSchema,
    schema_by_table: &SchemaByTable<'_>,
) -> Result<HashMap<usize, ForeignKeyLookup>> {
    let mut lookups = HashMap::new();
    for (column_index, column) in schema.columns.iter().enumerate() {
        for fk in foreign_key_candidates(column) {
            let Some(lookup) = resolve_foreign_key_lookup(
                catalog,
                table,
                schema_by_table,
                column,
                column_index,
                fk,
            )?
            else {
                continue;
            };
            lookups.insert(column_index, lookup);
            break;
        }
    }
    Ok(lookups)
}

fn resolve_foreign_key_lookup(
    catalog: &GameSystemCatalog,
    table: &GameSystemTable,
    schema_by_table: &SchemaByTable<'_>,
    column: &GameSystemColumnSchema,
    column_index: usize,
    fk: &GameSystemForeignKeyCandidate,
) -> Result<Option<ForeignKeyLookup>> {
    let Some(target) = catalog.table(&fk.target_table) else {
        return Ok(None);
    };
    let target_schema = schema_for_table(schema_by_table, target)?;
    let Some(target_column_index) = target
        .columns()
        .iter()
        .position(|entry| entry.name() == fk.target_column)
    else {
        return Ok(None);
    };
    let Some(target_column) = target_schema.columns.get(target_column_index) else {
        return Ok(None);
    };
    if !target_column.row_key {
        return Ok(None);
    }
    let rows = row_index_lookup(target, target_column_index);
    let resolution = column_foreign_key_resolution(table, column, column_index, &rows)?;
    if !resolution.has_references() {
        return Ok(None);
    }
    if resolution.missing_values > 0 && !is_typed_foreign_key_candidate(fk) {
        return Ok(None);
    }
    Ok(Some(ForeignKeyLookup {
        target_table_name_crc: target.name_crc(),
        target_schema_hash: schema_hash(target_schema)?,
    }))
}

fn build_dependency_edges(
    schema: &GameSystemTableSchema,
    fk_lookups: &HashMap<usize, ForeignKeyLookup>,
) -> Vec<TableDependency> {
    let mut edges = Vec::new();
    for (&column_index, lookup) in fk_lookups {
        let column = &schema.columns[column_index];
        edges.push(TableDependency {
            column_crc: column.crc,
            target_table_name_crc: lookup.target_table_name_crc,
            target_schema_hash: lookup.target_schema_hash,
            kind: DEPENDENCY_KIND_FOREIGN_KEY,
        });
    }
    edges.sort_by_key(|edge| (edge.column_crc, edge.target_table_name_crc));
    edges.dedup_by_key(|edge| (edge.column_crc, edge.target_table_name_crc));
    edges
}

/// Infer FK edges from datasheet schema when native table assets are unavailable
/// (for example during `nw-extract gamedata` before transform).
pub(crate) fn infer_table_dependencies(
    catalog: &GameSystemCatalog,
    schema_report: &GameSystemCatalogSchemaReport,
) -> Result<Vec<GameDataDependency>> {
    let schema_by_table = schema_by_table(schema_report);
    let name_crc_to_asset: HashMap<u32, nw_asset::AssetId> = catalog
        .tables()
        .iter()
        .filter_map(|table| {
            table
                .source_asset_id()
                .map(|asset_id| (table.name_crc(), asset_id))
        })
        .collect();
    let mut edges = HashSet::new();
    for table in sorted_tables(catalog) {
        let Some(from) = table.source_asset_id() else {
            continue;
        };
        let schema = schema_for_table(&schema_by_table, table)?;
        let fk_lookups = build_foreign_key_lookups(catalog, table, schema, &schema_by_table)?;
        for dependency in build_dependency_edges(schema, &fk_lookups) {
            let Some(to) = name_crc_to_asset.get(&dependency.target_table_name_crc) else {
                continue;
            };
            edges.insert((from, *to));
        }
    }
    let mut edges = edges
        .into_iter()
        .map(|(from, to)| GameDataDependency {
            from,
            to,
            kind: GameDataDependencyKind::ForeignKey,
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|edge| (edge.from, edge.to));
    Ok(edges)
}

fn row_index_lookup(
    target: &GameSystemTable,
    target_column_index: usize,
) -> HashMap<String, RowIndex> {
    let mut lookup = HashMap::new();
    for (row_index, row) in target.row_refs().enumerate() {
        let Some(value) = row
            .cells()
            .get(target_column_index)
            .and_then(|cell| cell.value().as_str())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(index) =
            RowIndex::from_one_based(u32::try_from(row_index + 1).expect("row index"))
        else {
            continue;
        };
        lookup.entry(value.to_owned()).or_insert(index);
    }
    lookup
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ForeignKeyResolution {
    checked_values: usize,
    matched_values: usize,
    missing_values: usize,
}

impl ForeignKeyResolution {
    const fn has_references(self) -> bool {
        self.checked_values > 0
    }
}

fn column_foreign_key_resolution(
    table: &GameSystemTable,
    column: &GameSystemColumnSchema,
    column_index: usize,
    lookup: &HashMap<String, RowIndex>,
) -> Result<ForeignKeyResolution> {
    let mut resolution = ForeignKeyResolution::default();
    for row in table.row_refs() {
        let Some(value) = row
            .cells()
            .get(column_index)
            .and_then(|cell| cell.value().as_str())
        else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let entries = if column_has_list(column) {
            datasheet_list_entries(column, value)?
        } else {
            vec![value.trim()]
        };
        for entry in entries {
            if entry.is_empty() {
                continue;
            }
            resolution.checked_values += 1;
            if lookup.contains_key(entry) {
                resolution.matched_values += 1;
            } else {
                resolution.missing_values += 1;
            }
        }
    }
    Ok(resolution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nw_asset::AssetId;
    use uuid::Uuid;

    fn string_column(name: &str) -> GameSystemColumn {
        GameSystemColumn::new(crc(name), name, ColumnType::String)
    }

    fn string_cell(name: &str, value: &str) -> GameSystemCell {
        GameSystemCell::new(crc(name), OwnedCellValue::String(value.to_owned()))
    }

    fn crc(name: &str) -> u32 {
        name.bytes().fold(0x811c_9dc5, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
        })
    }

    fn source_asset(path: &str, salt: u128) -> GameSystemAsset {
        GameSystemAsset::with_asset_id(path, asset_id(salt))
    }

    fn asset_id(salt: u128) -> AssetId {
        AssetId::new(Uuid::from_u128(salt), 0)
    }

    fn source_table() -> GameSystemTable {
        source_table_with_values(["DamageA"])
    }

    fn source_table_with_values<const N: usize>(values: [&str; N]) -> GameSystemTable {
        GameSystemTable::from_native_columns(
            "SourceTable",
            crc("SourceTable"),
            "SourceRow",
            crc("SourceRow"),
            vec![string_column("Id"), string_column("DamageTableRow")],
            values.into_iter().enumerate().map(|(index, value)| {
                let id = format!("Source{}", index + 1);
                (
                    crc(&id),
                    vec![string_cell("Id", &id), string_cell("DamageTableRow", value)],
                )
            }),
        )
        .with_source_asset(source_asset("source.datasheet", 1))
    }

    fn target_table() -> GameSystemTable {
        GameSystemTable::from_native_columns(
            "DamageData",
            crc("DamageData"),
            "DamageRow",
            crc("DamageRow"),
            vec![string_column("Id")],
            [(crc("DamageA"), vec![string_cell("Id", "DamageA")])],
        )
        .with_source_asset(source_asset("damage.datasheet", 2))
    }

    fn source_schema(foreign_keys: Vec<GameSystemForeignKeyCandidate>) -> GameSystemTableSchema {
        GameSystemTableSchema {
            table_name: "SourceTable".to_owned(),
            table_name_crc: crc("SourceTable"),
            row_type_name: "SourceRow".to_owned(),
            row_type_crc: crc("SourceRow"),
            row_count: 1,
            sources: vec!["source.datasheet".to_owned()],
            columns: vec![
                string_schema("Id", true, Vec::new()),
                string_schema("DamageTableRow", false, foreign_keys),
            ],
        }
    }

    fn target_schema() -> GameSystemTableSchema {
        GameSystemTableSchema {
            table_name: "DamageData".to_owned(),
            table_name_crc: crc("DamageData"),
            row_type_name: "DamageRow".to_owned(),
            row_type_crc: crc("DamageRow"),
            row_count: 1,
            sources: vec!["damage.datasheet".to_owned()],
            columns: vec![string_schema("Id", true, Vec::new())],
        }
    }

    fn foreign_key(target_table: &str, confidence: f64) -> GameSystemForeignKeyCandidate {
        GameSystemForeignKeyCandidate {
            target_table: target_table.to_owned(),
            target_column: "Id".to_owned(),
            checked_values: 1,
            matched_values: 1,
            missing_values: 0,
            confidence,
        }
    }

    fn partial_foreign_key(
        target_table: &str,
        checked_values: usize,
        matched_values: usize,
        missing_values: usize,
        confidence: f64,
    ) -> GameSystemForeignKeyCandidate {
        GameSystemForeignKeyCandidate {
            target_table: target_table.to_owned(),
            target_column: "Id".to_owned(),
            checked_values,
            matched_values,
            missing_values,
            confidence,
        }
    }

    fn string_schema(
        name: &str,
        row_key: bool,
        foreign_keys: Vec<GameSystemForeignKeyCandidate>,
    ) -> GameSystemColumnSchema {
        GameSystemColumnSchema {
            name: name.to_owned(),
            crc: crc(name),
            declared_type: ColumnType::String,
            row_key,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: true,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                qualified_reference_like: false,
                list: None,
                foreign_keys,
            },
        }
    }

    #[test]
    fn dependency_inference_keeps_missing_inferred_foreign_keys_untyped() -> Result<()> {
        let mut catalog = GameSystemCatalog::default();
        catalog.insert(source_table())?;
        let schema_report = GameSystemCatalogSchemaReport {
            tables: vec![source_schema(vec![foreign_key("MissingDamageData", 1.0)])],
            type_affinities: Vec::new(),
            diagnostics: Vec::new(),
        };

        let dependencies = infer_table_dependencies(&catalog, &schema_report)?;

        assert!(dependencies.is_empty());
        Ok(())
    }

    #[test]
    fn dependency_inference_uses_first_resolvable_foreign_key_candidate() -> Result<()> {
        let mut catalog = GameSystemCatalog::default();
        catalog.insert(source_table())?;
        catalog.insert(target_table())?;
        let schema_report = GameSystemCatalogSchemaReport {
            tables: vec![
                source_schema(vec![
                    foreign_key("MissingDamageData", 1.0),
                    foreign_key("DamageData", 0.95),
                ]),
                target_schema(),
            ],
            type_affinities: Vec::new(),
            diagnostics: Vec::new(),
        };

        let dependencies = infer_table_dependencies(&catalog, &schema_report)?;

        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].from, asset_id(1));
        assert_eq!(dependencies[0].to, asset_id(2));
        assert_eq!(dependencies[0].kind, GameDataDependencyKind::ForeignKey);
        Ok(())
    }

    #[test]
    fn dependency_inference_keeps_strong_partial_foreign_key_edge() -> Result<()> {
        let mut catalog = GameSystemCatalog::default();
        catalog.insert(source_table_with_values(["DamageA", "MissingDamage"]))?;
        catalog.insert(target_table())?;
        let schema_report = GameSystemCatalogSchemaReport {
            tables: vec![
                source_schema(vec![partial_foreign_key("DamageData", 2, 1, 1, 0.95)]),
                target_schema(),
            ],
            type_affinities: Vec::new(),
            diagnostics: Vec::new(),
        };

        let dependencies = infer_table_dependencies(&catalog, &schema_report)?;

        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].from, asset_id(1));
        assert_eq!(dependencies[0].to, asset_id(2));
        assert_eq!(dependencies[0].kind, GameDataDependencyKind::ForeignKey);
        Ok(())
    }

    #[test]
    fn dependency_inference_keeps_family_typed_missing_value_edge() -> Result<()> {
        let mut catalog = GameSystemCatalog::default();
        catalog.insert(source_table_with_values(["MissingDamage"]))?;
        catalog.insert(target_table())?;
        let schema_report = GameSystemCatalogSchemaReport {
            tables: vec![
                source_schema(vec![partial_foreign_key("DamageData", 1, 0, 1, 1.0)]),
                target_schema(),
            ],
            type_affinities: Vec::new(),
            diagnostics: Vec::new(),
        };

        let dependencies = infer_table_dependencies(&catalog, &schema_report)?;

        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].from, asset_id(1));
        assert_eq!(dependencies[0].to, asset_id(2));
        assert_eq!(dependencies[0].kind, GameDataDependencyKind::ForeignKey);
        Ok(())
    }
}
