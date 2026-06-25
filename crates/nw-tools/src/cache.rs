//! On-disk cache (SQLite via `drizzle`) for the asset catalog (its RASC table).
//!
//! Parsing New World's 365 MB asset catalog out of `Engine.pak` costs ~12 s per
//! run. The catalog only changes when the game updates, so we parse the RASC
//! catalog once and persist it — both the full `AssetId → path/type/size` index
//! (for `asset`/`format catalog` lookups) and the derived `.mtl` material map
//! (for `format model`). On later runs both load straight from SQLite, gated on a
//! fingerprint of `Engine.pak` so a game patch transparently rebuilds them.
//!
//! The migration set is generated from the [`Schema`] below by `build.rs` (see
//! `drizzle-migrations`) and embedded at compile time via
//! `drizzle::include_migrations!`, so the on-disk schema always matches.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use drizzle::migrations::Tracking;
use drizzle::sqlite::connection::SQLiteTransactionType;
use drizzle::sqlite::prelude::*;
use drizzle::sqlite::rusqlite::Drizzle;

/// One RASC catalog entry: its `AssetId` string (`{GUID}:subid`), virtual path,
/// AZ asset-type string, and asset size in bytes.
#[derive(Debug, Clone)]
pub struct CatalogRecord {
    pub asset_id: String,
    pub path: String,
    pub asset_type: String,
    pub size: i64,
}

/// Full RASC catalog index, keyed by `AssetId` string.
#[SQLiteTable]
pub struct Catalog {
    #[column(primary)]
    pub asset_id: String,
    pub path: String,
    pub asset_type: String,
    pub size: i64,
}

/// Catalog material map: asset-id string (`{GUID}:subid`) → `.mtl` asset path.
#[SQLiteTable]
pub struct Guid {
    #[column(primary)]
    pub guid: String,
    pub path: String,
}

/// Single-row key/value metadata — currently the catalog fingerprint.
#[SQLiteTable]
pub struct Meta {
    #[column(primary)]
    pub key: String,
    pub value: String,
}

#[derive(SQLiteSchema)]
pub struct Schema {
    pub catalog: Catalog,
    pub guid: Guid,
    pub meta: Meta,
}

const FINGERPRINT_KEY: &str = "engine_pak_fingerprint";

/// Connection pragmas. This database is a disposable cache — it is rebuilt from
/// the catalog whenever `Engine.pak` changes — so we trade durability for speed:
/// WAL with relaxed `synchronous` for fast concurrent reads and cheap bulk writes,
/// memory temp store, and a memory-mapped read window over the file.
const PRAGMAS: &str = "\
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 268435456;";

/// SQLite is capped at 999 bound parameters per statement. [`Guid`] binds two
/// columns per row and [`Catalog`] four, so these are the largest safe multi-row
/// inserts for each.
const GUID_CHUNK: usize = 400;
const CATALOG_CHUNK: usize = 240;

/// The catalog material cache, backed by a migrated SQLite database.
pub struct Cache {
    db: Drizzle<Schema>,
}

impl Cache {
    /// Open (creating if needed) the cache at `path`, applying pending migrations.
    ///
    /// # Errors
    ///
    /// Returns an error if the parent directory or database cannot be created, or
    /// a migration fails.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self::migrated(rusqlite::Connection::open(path)?)?)
    }

    /// Open an in-memory cache — for tests and ephemeral runs.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be created or a migration fails.
    pub fn open_in_memory() -> anyhow::Result<Self> {
        Ok(Self::migrated(rusqlite::Connection::open_in_memory()?)?)
    }

    fn migrated(conn: rusqlite::Connection) -> drizzle::Result<Self> {
        conn.execute_batch(PRAGMAS)?;
        let (db, _) = Drizzle::new(conn, Schema::new());
        db.migrate(&drizzle::include_migrations!("./drizzle"), Tracking::SQLITE)?;
        Ok(Self { db })
    }

    /// The stored `Engine.pak` fingerprint, if the cache has been populated.
    #[must_use]
    pub fn fingerprint(&self) -> Option<String> {
        let Schema { meta, .. } = Schema::new();
        let rows: Vec<SelectMeta> = self.db.select(()).from(meta).all().ok()?;
        rows.into_iter()
            .find(|row| row.key == FINGERPRINT_KEY)
            .map(|row| row.value)
    }

    /// Load the whole material map (asset-id → `.mtl` path) into memory.
    #[must_use]
    pub fn material_map(&self) -> HashMap<String, String> {
        let Schema { guid, .. } = Schema::new();
        self.db
            .select(())
            .from(guid)
            .all()
            .map(|rows: Vec<SelectGuid>| {
                rows.into_iter().map(|row| (row.guid, row.path)).collect()
            })
            .unwrap_or_default()
    }

    /// Load the full RASC catalog index into memory.
    #[must_use]
    pub fn catalog_records(&self) -> Vec<CatalogRecord> {
        let Schema { catalog, .. } = Schema::new();
        self.db
            .select(())
            .from(catalog)
            .all()
            .map(|rows: Vec<SelectCatalog>| {
                rows.into_iter()
                    .map(|row| CatalogRecord {
                        asset_id: row.asset_id,
                        path: row.path,
                        asset_type: row.asset_type,
                        size: row.size,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Replace the catalog index (and its derived `.mtl` material map) and record
    /// the fingerprint that produced it — all in one transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if any insert fails.
    pub fn store(&mut self, fingerprint: &str, records: &[CatalogRecord]) -> drizzle::Result<()> {
        let Schema { catalog, guid, meta } = Schema::new();
        let materials = records
            .iter()
            .filter(|record| record.path.ends_with(".mtl"))
            .collect::<Vec<_>>();
        self.db.transaction(SQLiteTransactionType::Deferred, |tx| {
            for chunk in records.chunks(CATALOG_CHUNK) {
                let rows = chunk
                    .iter()
                    .map(|record| {
                        InsertCatalog::new(
                            record.asset_id.as_str(),
                            record.path.as_str(),
                            record.asset_type.as_str(),
                            record.size,
                        )
                    })
                    .collect::<Vec<_>>();
                tx.insert(catalog).values(rows).execute()?;
            }
            for chunk in materials.chunks(GUID_CHUNK) {
                let rows = chunk
                    .iter()
                    .map(|record| InsertGuid::new(record.asset_id.as_str(), record.path.as_str()))
                    .collect::<Vec<_>>();
                tx.insert(guid).values(rows).execute()?;
            }
            tx.insert(meta)
                .value(InsertMeta::new(FINGERPRINT_KEY, fingerprint))
                .execute()?;
            Ok(())
        })
    }
}

/// Default cache file location, under the OS cache/data directory.
#[must_use]
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("nw-tools").join("catalog.sqlite")
}

/// Fingerprint a file by its length and modification time — enough to detect a
/// game patch replacing `Engine.pak`.
#[must_use]
pub fn file_fingerprint(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(format!("{}:{modified}", meta.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_indexes_catalog_and_derives_material_map() {
        let mut cache = Cache::open_in_memory().unwrap();
        assert!(cache.fingerprint().is_none());

        let records = vec![
            CatalogRecord {
                asset_id: "{ABC}:0".to_string(),
                path: "objects/foo_mat.mtl".to_string(),
                asset_type: "{MTL}".to_string(),
                size: 10,
            },
            CatalogRecord {
                asset_id: "{DEF}:0".to_string(),
                path: "objects/foo_mesh.cgf".to_string(),
                asset_type: "{MESH}".to_string(),
                size: 20,
            },
        ];
        cache.store("123:456", &records).unwrap();

        assert_eq!(cache.fingerprint().as_deref(), Some("123:456"));
        assert_eq!(cache.catalog_records().len(), 2);

        // Only the `.mtl` entry is projected into the material map.
        let map = cache.material_map();
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("{ABC}:0").map(String::as_str), Some("objects/foo_mat.mtl"));
    }
}
