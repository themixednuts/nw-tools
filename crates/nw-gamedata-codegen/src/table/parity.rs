//! Semantic-parity validation against the raw datasheet corpus.
//!
//! Ground truth for these checks is the native GameData manager behavior
//! documented in `docs/subsystems/datasheets.md` ("Native manager load
//! semantics") and the RTTI evidence it cites:
//!
//! - key identity is lowercase-then-CRC32 (`AZ::Crc32` over ASCII-lowercased
//!   key text);
//! - duplicate keys resolve family-wide first-wins: the manager cache is
//!   cleared once, then every physical table in the family is enumerated in
//!   registration order through a map-insert duplicate check;
//! - rows with empty keys are skipped.
//!
//! `table_parity` checks one physical table's transform for losslessness and
//! key-identity agreement; `family_first_wins_replay` replays the native
//! duplicate resolution over the raw corpus so the declared per-manager
//! duplicate policy can be validated against real data. Parity green is the
//! first freeze criterion before any generated table source may flip to
//! hand-editable (`docs/subsystems/gamedata-ownership.md`).

use anyhow::Result;
use az_core::crc::Crc32;
use nw_datasheet::game_system::{GameSystemDataTables as GameSystemCatalog, GameSystemTable};
use std::collections::BTreeMap;

use super::identity::{
    native_table_source_path_from_modules, row_type_module_name, schema_by_table, schema_for_table,
    table_module_name,
};
use super::source_output::{native_dev_table_file, row_key_source_value};
use crate::game_system_schema::{
    GameSystemDataTablesSchemaReport as GameSystemCatalogSchemaReport, GameSystemTableSchema,
};

/// Independent reference implementation of the native key lowering:
/// CRC-32 (IEEE, reflected, init/final-xor `0xFFFF_FFFF`) over the
/// ASCII-lowercased key bytes. Deliberately NOT implemented in terms of
/// `az_core::Crc32` so corpus tests cross-check the engine implementation
/// against the documented native semantic.
#[must_use]
pub fn reference_lowered_key_crc(key: &str) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for byte in key.bytes() {
        let lowered = byte.to_ascii_lowercase();
        crc ^= u32::from(lowered);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Parity findings for one physical table's transform.
#[derive(Debug)]
pub struct TableParity {
    pub table_name: String,
    /// Rows in the raw datasheet.
    pub source_rows: usize,
    /// Rows in the transformed (authored RON) output. Must equal
    /// `source_rows`: the per-table transform is faithful; duplicate/validity
    /// policy is applied by managers at load, never by the transform.
    pub emitted_rows: usize,
    /// Rows whose row-key cell is empty (natively skipped at manager load).
    pub empty_key_rows: usize,
    /// Keys where `az_core::Crc32::from_str_lower` disagrees with the
    /// independent reference implementation. Must be empty.
    pub key_crc_mismatches: Vec<String>,
    /// Non-empty keys lowering to CRC 0 (natively rejected as invalid).
    pub zero_crc_keys: Vec<String>,
}

impl TableParity {
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        self.emitted_rows == self.source_rows && self.key_crc_mismatches.is_empty()
    }
}

/// Checks one physical table's transform against the raw datasheet: row-count
/// losslessness and key-identity agreement between the engine CRC and the
/// reference implementation of the native lowering.
pub fn table_parity(
    table: &GameSystemTable,
    schema: &GameSystemTableSchema,
) -> Result<TableParity> {
    let type_module = row_type_module_name(schema);
    let table_module = table_module_name(schema);
    let ron_path = native_table_source_path_from_modules(&type_module, &table_module);
    let transform = native_dev_table_file(table, schema, &ron_path)?;

    let mut empty_key_rows = 0usize;
    let mut key_crc_mismatches = Vec::new();
    let mut zero_crc_keys = Vec::new();
    for row in table.row_refs() {
        let Some(key) = row_key_source_value(schema, row.cells()) else {
            empty_key_rows += 1;
            continue;
        };
        if key.is_empty() {
            empty_key_rows += 1;
            continue;
        }
        let engine = Crc32::from_str_lower(&key).value();
        if engine != reference_lowered_key_crc(&key) {
            key_crc_mismatches.push(key.clone());
        }
        if engine == 0 {
            zero_crc_keys.push(key);
        }
    }

    Ok(TableParity {
        table_name: table.name().to_string(),
        source_rows: table.len(),
        emitted_rows: transform.rows.len(),
        empty_key_rows,
        key_crc_mismatches,
        zero_crc_keys,
    })
}

/// Runs `table_parity` for every physical table in the catalog whose row
/// type is `row_type_name`.
pub fn family_tables_parity(
    catalog: &GameSystemCatalog,
    schema_report: &GameSystemCatalogSchemaReport,
    row_type_name: &str,
) -> Result<Vec<TableParity>> {
    let schemas = schema_by_table(schema_report);
    catalog
        .tables()
        .iter()
        .filter(|table| table.type_name() == row_type_name)
        .map(|table| table_parity(table, schema_for_table(&schemas, table)?))
        .collect()
}

/// One key that appeared in more than one place during a family replay.
#[derive(Debug, Clone)]
pub struct FamilyDuplicate {
    pub key: String,
    pub key_crc: u32,
    /// Table whose row survives under first-wins.
    pub winner_table: String,
    /// Table whose row is logged and discarded under first-wins (it would
    /// replace the winner under an overwrite policy).
    pub loser_table: String,
}

/// Result of replaying the native family-wide duplicate resolution.
#[derive(Debug)]
pub struct FamilyFirstWinsReplay {
    pub row_type_name: String,
    /// Physical tables in the enumeration order used for the replay.
    pub tables: Vec<String>,
    /// Distinct lowered-key count: the surviving row count under first-wins.
    pub distinct_keys: usize,
    pub duplicates: Vec<FamilyDuplicate>,
    pub skipped_empty_key_rows: usize,
}

/// Replays native `CacheAllDataTables` duplicate semantics over the raw
/// corpus for every physical table whose row type is `row_type_name`, in
/// `table_order` if given (native registration order), otherwise in catalog
/// order. The cache map persists across tables (cleared once), so a key in a
/// later table loses to the same key in an earlier table — family-wide
/// first-wins, matching the decompile evidence.
pub fn family_first_wins_replay(
    catalog: &GameSystemCatalog,
    schema_report: &GameSystemCatalogSchemaReport,
    row_type_name: &str,
    table_order: &[&str],
) -> Result<FamilyFirstWinsReplay> {
    let schemas = schema_by_table(schema_report);
    let mut family: Vec<&GameSystemTable> = catalog
        .tables()
        .iter()
        .filter(|table| table.type_name() == row_type_name)
        .collect();
    if !table_order.is_empty() {
        let rank = |name: &str| {
            table_order
                .iter()
                .position(|ordered| *ordered == name)
                .unwrap_or(usize::MAX)
        };
        family.sort_by_key(|table| (rank(table.name()), table.name().to_string()));
    }

    let mut cache: BTreeMap<u32, (String, String)> = BTreeMap::new();
    let mut duplicates = Vec::new();
    let mut skipped_empty_key_rows = 0usize;
    for table in &family {
        let schema = schema_for_table(&schemas, table)?;
        for row in table.row_refs() {
            let Some(key) = row_key_source_value(schema, row.cells()) else {
                skipped_empty_key_rows += 1;
                continue;
            };
            if key.is_empty() {
                skipped_empty_key_rows += 1;
                continue;
            }
            let key_crc = Crc32::from_str_lower(&key).value();
            match cache.get(&key_crc) {
                Some((winner_table, winner_key)) => duplicates.push(FamilyDuplicate {
                    key: winner_key.clone(),
                    key_crc,
                    winner_table: winner_table.clone(),
                    loser_table: table.name().to_string(),
                }),
                None => {
                    cache.insert(key_crc, (table.name().to_string(), key));
                }
            }
        }
    }

    Ok(FamilyFirstWinsReplay {
        row_type_name: row_type_name.to_string(),
        tables: family
            .iter()
            .map(|table| table.name().to_string())
            .collect(),
        distinct_keys: cache.len(),
        duplicates,
        skipped_empty_key_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::reference_lowered_key_crc;
    use az_core::crc::Crc32;

    #[test]
    fn reference_crc_matches_engine_lowering_on_known_values() {
        for key in [
            "Archetype_Soldier",
            "MISSION_WEIGHT_01",
            "already_lower",
            "MiXeD CaSe With Spaces",
            "123_numeric",
        ] {
            assert_eq!(
                reference_lowered_key_crc(key),
                Crc32::from_str_lower(key).value(),
                "engine and reference lowering disagree for {key:?}"
            );
        }
    }

    #[test]
    fn reference_crc_is_case_insensitive_ascii() {
        assert_eq!(
            reference_lowered_key_crc("SwordT5"),
            reference_lowered_key_crc("swordt5")
        );
    }
}
