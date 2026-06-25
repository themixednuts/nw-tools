//! On-disk cache (SQLite via `drizzle`) for the asset catalog's material map.
//!
//! Parsing New World's 365 MB asset catalog out of `Engine.pak` costs ~12 s per
//! run. The catalog only changes when the game updates, so we parse it once,
//! persist the `MtlName`-GUID → `.mtl`-path map, and on later runs load that map
//! straight from SQLite — gated on a fingerprint of `Engine.pak` so a game patch
//! transparently rebuilds it.
//!
//! The migration set is generated from the [`Schema`] below by `build.rs` (see
//! `drizzle-migrations`) and embedded at compile time via
//! `drizzle::include_migrations!`, so the on-disk schema always matches.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use drizzle::migrations::Tracking;
use drizzle::sqlite::prelude::*;
use drizzle::sqlite::rusqlite::Drizzle;

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

/// SQLite is capped at 999 bound parameters per statement; [`Guid`] binds two
/// columns per row, so this many rows is the largest safe multi-row insert.
const INSERT_CHUNK: usize = 400;

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
    #[cfg(test)]
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

    /// Replace the material map and record the fingerprint that produced it.
    ///
    /// # Errors
    ///
    /// Returns an error if any insert fails.
    pub fn store(&self, fingerprint: &str, map: &HashMap<String, String>) -> drizzle::Result<()> {
        let Schema { guid, meta } = Schema::new();
        for chunk in map.iter().collect::<Vec<_>>().chunks(INSERT_CHUNK) {
            let rows = chunk
                .iter()
                .map(|(g, p)| InsertGuid::new(g.as_str(), p.as_str()))
                .collect::<Vec<_>>();
            self.db.insert(guid).values(rows).execute()?;
        }
        self.db
            .insert(meta)
            .value(InsertMeta::new(FINGERPRINT_KEY, fingerprint))
            .execute()?;
        Ok(())
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
    fn store_and_reload_map() {
        let cache = Cache::open_in_memory().unwrap();
        assert!(cache.fingerprint().is_none());

        let mut map = HashMap::new();
        map.insert("{ABC}:0".to_string(), "objects/foo_mat.mtl".to_string());
        map.insert("{DEF}:0".to_string(), "objects/bar_mat.mtl".to_string());
        cache.store("123:456", &map).unwrap();

        assert_eq!(cache.fingerprint().as_deref(), Some("123:456"));
        assert_eq!(cache.material_map(), map);
    }
}
