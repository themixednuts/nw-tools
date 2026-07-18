//! On-disk cache (SQLite via `drizzle`) for the resolved asset catalog.
//!
//! Parsing New World's 365 MB asset catalog out of `Engine.pak` costs ~12 s per
//! run. The catalog only changes when the game updates, so we parse RASC and
//! RAOC once and persist the product index. On later
//! runs they reconstruct the same [`nw_asset::AssetCatalog`] used by library
//! consumers, gated on an `Engine.pak` fingerprint so a game patch transparently
//! rebuilds the cache.
//!
//! The migration set is generated from the [`Schema`] below by `build.rs` (see
//! `drizzle-migrations`) and embedded at compile time via
//! `drizzle::include_migrations!`, so the on-disk schema always matches.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use drizzle::migrations::Tracking;
use drizzle::sqlite::connection::SQLiteTransactionType;
use drizzle::sqlite::prelude::*;
use drizzle::sqlite::rusqlite::Drizzle;

/// Full RASC catalog index, keyed by `AssetId` string.
#[SQLiteTable]
pub struct Catalog {
    #[column(primary)]
    pub asset_id: String,
    pub path: String,
    pub asset_type: String,
    pub size: i64,
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
    pub meta: Meta,
}

const FINGERPRINT_KEY: &str = "engine_pak_fingerprint";
const RASC_VERSION_KEY: &str = "rasc_version";

/// Connection pragmas. This database is a disposable cache — it is rebuilt from
/// the catalog whenever `Engine.pak` changes — so we trade durability for speed:
/// WAL with relaxed `synchronous` for fast concurrent reads and cheap bulk writes,
/// memory temp store, and a memory-mapped read window over the file.
const PRAGMAS: &str = "\
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 268435456;";

/// SQLite is capped at 999 bound parameters per statement. [`Catalog`] binds
/// four columns per row, so this stays below the limit.
const CATALOG_CHUNK: usize = 240;

/// The parsed catalog cache, backed by a migrated SQLite database.
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

    /// Reconstruct the shared catalog abstraction from cached products.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache is incomplete or contains malformed IDs,
    /// types, sizes, or version metadata.
    pub fn catalog(&self) -> anyhow::Result<nw_asset::AssetCatalog> {
        let Schema { catalog, meta } = Schema::new();
        let meta: Vec<SelectMeta> = self.db.select(()).from(meta).all()?;
        let version = meta
            .into_iter()
            .find(|row| row.key == RASC_VERSION_KEY)
            .ok_or_else(|| anyhow::anyhow!("catalog cache is missing RASC version"))?
            .value
            .parse::<u32>()?;
        let catalog_rows: Vec<SelectCatalog> = self.db.select(()).from(catalog).all()?;
        let entries = catalog_rows
            .into_iter()
            .map(|row| {
                Ok(nw_asset::RascEntry::new(
                    nw_asset::AssetId::from_str(&row.asset_id)?,
                    nw_asset::AssetType::from_str(&row.asset_type)?,
                    row.path,
                    u32::try_from(row.size)?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(nw_asset::AssetCatalog::new(
            nw_asset::Rasc::new(version, entries),
            None,
        ))
    }

    /// Persist a complete product index, then record the
    /// fingerprint that produced it — all in one transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if any insert fails.
    pub fn store(
        &mut self,
        fingerprint: &str,
        asset_catalog: &nw_asset::AssetCatalog,
    ) -> drizzle::Result<()> {
        let Schema { catalog, meta } = Schema::new();
        self.db.transaction(SQLiteTransactionType::Deferred, |tx| {
            // Rebuild in place instead of replacing the database file. SQLite may
            // still have the WAL open, and replacing only the main file can leave
            // a partially stale cache behind. Clearing and repopulating in one
            // transaction also preserves the last complete cache on failure.
            tx.delete(catalog).execute()?;
            tx.delete(meta).execute()?;

            for chunk in asset_catalog.entries().chunks(CATALOG_CHUNK) {
                let rows = chunk
                    .iter()
                    .map(|entry| {
                        InsertCatalog::new(
                            entry.asset_id().to_string(),
                            entry.path(),
                            entry.asset_type().to_string(),
                            i64::from(entry.size_bytes()),
                        )
                    })
                    .collect::<Vec<_>>();
                tx.insert(catalog).values(rows).execute()?;
            }
            tx.insert(meta)
                .value(InsertMeta::new(FINGERPRINT_KEY, fingerprint))
                .execute()?;
            tx.insert(meta)
                .value(InsertMeta::new(
                    RASC_VERSION_KEY,
                    asset_catalog.rasc().version().to_string(),
                ))
                .execute()?;
            Ok(())
        })
    }
}

/// Magic + format version for the persisted authored-dependency index blob.
/// Bump the trailing digit whenever the on-disk layout changes.
const DEP_INDEX_MAGIC: &[u8; 8] = b"NWDEPIX1";

/// Location of the persisted install-global dependency index, beside the catalog.
#[must_use]
pub fn dependency_index_path() -> PathBuf {
    default_path().with_file_name("dependency-index.bin")
}

/// Load the cached authored-dependency edge set when it matches `fingerprint`.
///
/// Rebuilding the whole-install reverse graph costs ~1–2 min and is identical
/// for every export against the same paks, so it is persisted keyed by a pak-set
/// fingerprint. Any magic/fingerprint mismatch, truncation, or IO error is a
/// cache miss (returns `None`) so the caller transparently rebuilds.
///
/// The blob is millions of edges (hundreds of MB), so it is streamed through a
/// buffered reader — never read whole into memory — to keep the peak footprint
/// at the resident edge set alone.
#[must_use]
pub fn load_dependency_index(fingerprint: &str) -> Option<Vec<nw_asset_graph::AssetDependencyEdge>> {
    let file = std::fs::File::open(dependency_index_path()).ok()?;
    decode_dependency_index(&mut std::io::BufReader::new(file), fingerprint)
}

/// Persist the authored-dependency edge set under `fingerprint`, atomically.
///
/// # Errors
///
/// Returns an error if the cache directory or file cannot be written.
pub fn store_dependency_index(
    fingerprint: &str,
    edges: &[nw_asset_graph::AssetDependencyEdge],
) -> anyhow::Result<()> {
    use std::io::Write;

    let path = dependency_index_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Write to a sibling temp file then rename, so a crash mid-write can never
    // leave a truncated index that would still pass the fingerprint check.
    // Stream through a buffered writer rather than staging the whole blob in a
    // Vec, keeping this off the peak while the full index is already resident.
    let tmp = path.with_extension("bin.tmp");
    let mut writer = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    encode_dependency_index(&mut writer, fingerprint, edges)?;
    writer.flush()?;
    drop(writer);
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

fn encode_dependency_index<W: std::io::Write>(
    writer: &mut W,
    fingerprint: &str,
    edges: &[nw_asset_graph::AssetDependencyEdge],
) -> std::io::Result<()> {
    writer.write_all(DEP_INDEX_MAGIC)?;
    write_blob_str(writer, fingerprint)?;
    writer.write_all(&(edges.len() as u64).to_le_bytes())?;
    for edge in edges {
        write_blob_str(writer, edge.source())?;
        write_blob_str(writer, edge.target())?;
        write_blob_str(writer, edge.relation())?;
        writer.write_all(&[u8::from(edge.is_required())])?;
        match edge.association() {
            Some(association) => {
                writer.write_all(&[1])?;
                write_blob_str(writer, association)?;
            }
            None => writer.write_all(&[0])?,
        }
    }
    Ok(())
}

fn decode_dependency_index<R: std::io::Read>(
    reader: &mut R,
    fingerprint: &str,
) -> Option<Vec<nw_asset_graph::AssetDependencyEdge>> {
    let mut magic = [0u8; DEP_INDEX_MAGIC.len()];
    reader.read_exact(&mut magic).ok()?;
    if &magic != DEP_INDEX_MAGIC {
        return None;
    }
    if read_blob_str(reader)? != fingerprint {
        return None;
    }
    let mut count_bytes = [0u8; 8];
    reader.read_exact(&mut count_bytes).ok()?;
    let count = usize::try_from(u64::from_le_bytes(count_bytes)).ok()?;
    let mut edges = Vec::with_capacity(count);
    let mut flag = [0u8; 1];
    for _ in 0..count {
        let source = read_blob_str(reader)?;
        let target = read_blob_str(reader)?;
        let relation = read_blob_str(reader)?;
        reader.read_exact(&mut flag).ok()?;
        let required = flag[0] != 0;
        reader.read_exact(&mut flag).ok()?;
        let association = if flag[0] != 0 {
            Some(read_blob_str(reader)?)
        } else {
            None
        };
        edges.push(nw_asset_graph::AssetDependencyEdge::new(
            source,
            target,
            relation,
            required,
            association,
        ));
    }
    Some(edges)
}

fn write_blob_str<W: std::io::Write>(writer: &mut W, value: &str) -> std::io::Result<()> {
    writer.write_all(&(value.len() as u32).to_le_bytes())?;
    writer.write_all(value.as_bytes())
}

fn read_blob_str<R: std::io::Read>(reader: &mut R) -> Option<String> {
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).ok()?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes).ok()?;
    String::from_utf8(bytes).ok()
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
    fn store_round_trips_and_replaces_catalog() {
        let mut cache = Cache::open_in_memory().unwrap();
        assert!(cache.fingerprint().is_none());
        let material_id = nw_asset::AssetId::new(uuid::Uuid::from_u128(1), 0);
        let mesh_id = nw_asset::AssetId::new(uuid::Uuid::from_u128(2), 7);
        let catalog = nw_asset::AssetCatalog::new(
            nw_asset::Rasc::new(
                5,
                vec![
                    nw_asset::RascEntry::new(
                        material_id,
                        nw_asset::AssetType::new(uuid::Uuid::from_u128(3)),
                        "objects/foo_mat.mtl",
                        10,
                    ),
                    nw_asset::RascEntry::new(
                        mesh_id,
                        nw_asset::AssetType::new(uuid::Uuid::from_u128(4)),
                        "objects/foo_mesh.cgf",
                        20,
                    ),
                ],
            ),
            None,
        );
        cache.store("123:456", &catalog).unwrap();

        assert_eq!(cache.fingerprint().as_deref(), Some("123:456"));
        let restored = cache.catalog().unwrap();
        assert_eq!(restored.rasc().version(), 5);
        assert_eq!(restored.entries(), catalog.entries());

        let replacement = nw_asset::AssetCatalog::new(
            nw_asset::Rasc::new(
                6,
                vec![nw_asset::RascEntry::new(
                    material_id,
                    nw_asset::AssetType::new(uuid::Uuid::from_u128(5)),
                    "objects/replacement.mtl",
                    30,
                )],
            ),
            None,
        );
        cache.store("789:012", &replacement).unwrap();

        assert_eq!(cache.fingerprint().as_deref(), Some("789:012"));
        let restored = cache.catalog().unwrap();
        assert_eq!(restored.rasc().version(), 6);
        assert_eq!(restored.entries(), replacement.entries());
    }

    #[test]
    fn dependency_index_blob_round_trips_and_rejects_stale_fingerprint() {
        let edges = vec![
            nw_asset_graph::AssetDependencyEdge::new(
                "objects/a.cdf".to_owned(),
                "objects/a_mat.mtl".to_owned(),
                "cry_model.material".to_owned(),
                true,
                None,
            ),
            nw_asset_graph::AssetDependencyEdge::new(
                "world/c.slice".to_owned(),
                "objects/a.cdf".to_owned(),
                "objectstream.asset_reference".to_owned(),
                false,
                Some("world/c.dynamicslice".to_owned()),
            ),
        ];
        let mut buf = Vec::new();
        encode_dependency_index(&mut buf, "fp-1", &edges).unwrap();

        // A matching fingerprint rehydrates every edge field.
        let decoded = decode_dependency_index(&mut buf.as_slice(), "fp-1").unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].source(), "objects/a.cdf");
        assert_eq!(decoded[0].relation(), "cry_model.material");
        assert!(decoded[0].is_required());
        assert_eq!(decoded[0].association(), None);
        assert_eq!(decoded[1].target(), "objects/a.cdf");
        assert!(!decoded[1].is_required());
        assert_eq!(decoded[1].association(), Some("world/c.dynamicslice"));

        // A stale fingerprint (install patched) is a cache miss, forcing a rebuild.
        assert!(decode_dependency_index(&mut buf.as_slice(), "fp-2").is_none());
        // Corruption of the magic is likewise rejected.
        buf[0] ^= 0xff;
        assert!(decode_dependency_index(&mut buf.as_slice(), "fp-1").is_none());
    }
}
