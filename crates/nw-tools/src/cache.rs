//! On-disk cache (SQLite via `drizzle`) for expensive, install-stable indexes —
//! the pak table-of-contents and the asset-catalog GUID→path map. Building these
//! from the paks costs ~minute of cold I/O + a 365 MB catalog parse; cached, a run
//! resolves by query instead.

use drizzle::migrations::Tracking;
use drizzle::sqlite::prelude::*;
use drizzle::sqlite::rusqlite::Drizzle;

/// Pak table-of-contents: virtual asset path → owning pak + entry index.
#[SQLiteTable]
pub struct Toc {
    #[column(primary)]
    pub path: String,
    pub pak: String,
    pub entry: i64,
}

/// Catalog GUID → asset path (the canonical AssetId guid, lowercased).
#[SQLiteTable]
pub struct Guid {
    #[column(primary)]
    pub guid: String,
    pub path: String,
}

#[derive(SQLiteSchema)]
pub struct Schema {
    pub toc: Toc,
    pub guid: Guid,
}

/// A connected cache database paired with its schema handles for building queries.
pub type Cache = (Drizzle<Schema>, Schema);

/// Open the cache at `path` (creating the file if missing) with all pending
/// migrations applied.
///
/// The migration set is embedded at compile time from `./drizzle`, which
/// `build.rs` regenerates from [`Schema`] whenever the table definitions change.
///
/// # Errors
///
/// Returns an error if the database cannot be opened or a migration fails.
pub fn open(path: impl AsRef<std::path::Path>) -> drizzle::Result<Cache> {
    migrated(rusqlite::Connection::open(path)?)
}

/// Open an in-memory cache with migrations applied — for tests and ephemeral runs.
///
/// # Errors
///
/// Returns an error if the in-memory database cannot be created or a migration fails.
pub fn open_in_memory() -> drizzle::Result<Cache> {
    migrated(rusqlite::Connection::open_in_memory()?)
}

fn migrated(conn: rusqlite::Connection) -> drizzle::Result<Cache> {
    let (db, schema) = Drizzle::new(conn, Schema::new());
    db.migrate(&drizzle::include_migrations!("./drizzle"), Tracking::SQLITE)?;
    Ok((db, schema))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_in_memory() {
        let (db, Schema { toc, guid: _ }) = open_in_memory().unwrap();
        db.insert(toc)
            .value(InsertToc::new("objects/foo.cgf", "DataStrm.pak", 7i64))
            .execute()
            .unwrap();
        let rows: Vec<SelectToc> = db.select(()).from(toc).all().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "objects/foo.cgf");
        assert_eq!(rows[0].entry, 7);
    }
}
