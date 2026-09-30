//! Compact relational content index over interned strings and dual CRC32 keys.
//!
//! Tokens are stored once; facts reference them by integer id. Lookups hash the
//! needle with [`nw_datasheet::game_system::Crc32`] and join through drizzle.
//! This is a speed layer for find/grep — the same facts are available via live
//! extract without a cold build. Build with `nw-tools index`. Other commands
//! may open an existing index; they do not start a cold build.

mod intern;
mod prompt;
mod query;
mod scan;

pub use prompt::{FIRST_BUILD_GIB, FIRST_BUILD_HOURS, confirm_accepted, first_build_warning};

use std::path::{Path, PathBuf};

use nw_datasheet::game_system::Crc32;
use nw_jobs::JobRunner;

use crate::cache::Cache;
use crate::grep::adapter::{FactDraft, FactKind};

pub(crate) fn fact_location(draft: &FactDraft) -> String {
    query::location(
        draft.kind,
        i64::from(draft.loc_a),
        &draft.field,
        &draft.value,
    )
}

/// Which token role a CRC lookup targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupTarget {
    Value,
    Field,
    Name,
}

/// Original-byte CRC vs lowercase CRC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrcCase {
    Original,
    Lower,
}

/// A CRC lookup against one token role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lookup {
    pub target: LookupTarget,
    pub case: CrcCase,
    pub crc: Crc32,
}

impl Lookup {
    #[must_use]
    pub fn value(text: &str) -> Self {
        Self {
            target: LookupTarget::Value,
            case: CrcCase::Lower,
            crc: Crc32::from_str_lower(text),
        }
    }

    #[must_use]
    pub fn field(text: &str) -> Self {
        Self {
            target: LookupTarget::Field,
            case: CrcCase::Original,
            crc: Crc32::from_str(text),
        }
    }

    #[must_use]
    pub fn name(text: &str) -> Self {
        Self {
            target: LookupTarget::Name,
            case: CrcCase::Lower,
            crc: Crc32::from_str_lower(text),
        }
    }
}

/// Format stored as a small integer on `idx_entry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum EntryFormat {
    Unknown = 0,
    Datasheet = 1,
    Mannequin = 2,
    ObjectStream = 3,
    Catalog = 4,
    Animation = 5,
    Audio = 6,
    Lua = 7,
    Resource = 8,
    Text = 9,
    Xml = 10,
}

impl EntryFormat {
    #[must_use]
    pub fn from_i64(value: i64) -> Self {
        match value {
            1 => Self::Datasheet,
            2 => Self::Mannequin,
            3 => Self::ObjectStream,
            4 => Self::Catalog,
            5 => Self::Animation,
            6 => Self::Audio,
            7 => Self::Lua,
            8 => Self::Resource,
            9 => Self::Text,
            10 => Self::Xml,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub fn from_adapter(name: &str) -> Self {
        match name {
            "datasheet" => Self::Datasheet,
            "mannequin" => Self::Mannequin,
            "objectstream" => Self::ObjectStream,
            "catalog" => Self::Catalog,
            "animation" => Self::Animation,
            "audio" => Self::Audio,
            "lua" => Self::Lua,
            "resource" => Self::Resource,
            "text" => Self::Text,
            "xml" => Self::Xml,
            _ => Self::Unknown,
        }
    }
}

/// One resolved fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedHit {
    pub pak: String,
    pub entry: String,
    pub format: EntryFormat,
    pub field: String,
    pub value: String,
    pub location: String,
    pub kind: FactKind,
}

/// Outcome of [`ContentIndex::build`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexStatus {
    Complete { paks: usize },
    Partial { indexed_paks: usize },
}

impl IndexStatus {
    #[must_use]
    pub fn is_complete(self) -> bool {
        matches!(self, Self::Complete { .. })
    }
}

/// The content index, backed by the same drizzle database as the catalog cache.
pub struct ContentIndex {
    cache: Cache,
}

impl ContentIndex {
    /// Open (creating if needed) the index at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be created or migrated.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(Self {
            cache: Cache::open(path)?,
        })
    }

    /// In-memory index for tests.
    ///
    /// # Errors
    ///
    /// Returns an error if the in-memory database cannot be created.
    pub fn open_in_memory() -> anyhow::Result<Self> {
        Ok(Self {
            cache: Cache::open_in_memory()?,
        })
    }

    /// Persist drafts for one entry, interned and CRC'd.
    ///
    /// # Errors
    ///
    /// Returns an error if a drizzle write fails.
    pub fn ingest(
        &mut self,
        pak: &str,
        name: &str,
        format: EntryFormat,
        facts: &[FactDraft],
    ) -> anyhow::Result<()> {
        scan::prepare(&mut self.cache)?;
        let mut interner = intern::Interner::load(&self.cache)?;
        let pak_id = interner.intern(pak);
        let name_id = interner.intern(name);
        let mut drafts = facts.to_vec();
        drafts.extend(crate::extract::path_name_facts(name));
        scan::ingest_entry(
            &mut self.cache,
            &mut interner,
            pak_id,
            name_id,
            format,
            &drafts,
        )
    }

    /// Look up facts by a token CRC.
    ///
    /// # Errors
    ///
    /// Returns an error if a drizzle read fails.
    pub fn lookup(&self, lookup: Lookup) -> anyhow::Result<Vec<IndexedHit>> {
        query::lookup(&self.cache, lookup)
    }

    /// Fuzzy (fff) or exact-substring match over interned token text, then
    /// the facts that use those tokens as a value or field.
    ///
    /// # Errors
    ///
    /// Returns an error if a drizzle read fails.
    pub fn search(&self, queries: &[String], exact: bool) -> anyhow::Result<Vec<IndexedHit>> {
        query::search(&self.cache, queries, exact)
    }

    /// Index every pak under `root` on the global jobs pool.
    ///
    /// # Errors
    ///
    /// Returns an error if pak discovery or a drizzle write fails.
    pub fn build(&mut self, root: impl AsRef<Path>) -> anyhow::Result<IndexStatus> {
        self.build_with_runner(root, &JobRunner::automatic())
    }

    /// Index every pak under `root`, extracting through `runner`.
    ///
    /// Decode/extract is scheduled on the jobs pool so it shares the same
    /// thread bound as the rest of the process. Interning and sqlite writes
    /// stay on the caller.
    ///
    /// # Errors
    ///
    /// Returns an error if pak discovery or a drizzle write fails.
    pub fn build_with_runner(
        &mut self,
        root: impl AsRef<Path>,
        runner: &JobRunner,
    ) -> anyhow::Result<IndexStatus> {
        scan::build(&mut self.cache, root.as_ref(), runner)
    }

    /// Persist the bundled serialize / behavior-context / module dumps.
    ///
    /// This is not part of [`Self::build`]: those files are not under the pak
    /// root. The CLI waiter calls this beside `build`. Overrides installed
    /// via [`crate::resources::install_session`] are not written here.
    ///
    /// # Errors
    ///
    /// Returns an error if a drizzle write fails.
    pub fn ingest_embedded(&mut self) -> anyhow::Result<()> {
        scan::prepare(&mut self.cache)?;
        let mut interner = intern::Interner::load(&self.cache)?;
        scan::index_resources(&mut self.cache, &mut interner)
    }

    /// Whether the bundled reflection dumps are indexed at their current stamp.
    #[must_use]
    pub fn has_embedded(&self) -> bool {
        scan::has_embedded(&self.cache)
    }

    /// Whether every pak under `root` is already indexed at its current stamp.
    #[must_use]
    pub fn is_complete(&self, root: impl AsRef<Path>) -> bool {
        scan::is_complete(&self.cache, root.as_ref())
    }
}

/// Open the default cache if that file exists. Does not build.
#[must_use]
pub fn open_existing() -> Option<ContentIndex> {
    let path = crate::cache::default_path();
    path.exists()
        .then(|| ContentIndex::open(path).ok())
        .flatten()
}

/// Build the install index on this thread. Complete paks are skipped.
///
/// # Errors
///
/// Returns an error if the install cannot be located or a drizzle write fails.
pub fn build_install(runner: &JobRunner) -> anyhow::Result<IndexStatus> {
    let root = nw_locator::Install::locate()?.assets().to_path_buf();
    let mut index = ContentIndex::open(crate::cache::default_path())?;
    let status = index.build_with_runner(root, runner)?;
    index.ingest_embedded()?;
    Ok(status)
}

/// A detached index build. Dropping it does not wait; progress is per-pak.
/// The join handle is only a waiter — extract work runs on [`JobRunner`].
pub struct IndexHandle {
    thread: Option<std::thread::JoinHandle<()>>,
}

impl IndexHandle {
    #[must_use]
    pub fn is_detached(&self) -> bool {
        self.thread.is_some()
    }

    pub fn join(self) {
        if let Some(thread) = self.thread {
            let _ = thread.join();
        }
    }
}

/// Start indexing `root` into `cache_path` without blocking the caller.
/// Extract work is scheduled on `runner` (defaults to the global jobs pool).
#[must_use]
pub fn start_alongside_at(cache_path: PathBuf, root: PathBuf) -> IndexHandle {
    start_alongside_on(cache_path, root, JobRunner::automatic())
}

/// Like [`start_alongside_at`], on a specific [`JobRunner`]. Paks only.
#[must_use]
pub fn start_alongside_on(cache_path: PathBuf, root: PathBuf, runner: JobRunner) -> IndexHandle {
    let thread = std::thread::Builder::new()
        .name("nw-content-index".to_owned())
        .spawn(move || {
            if let Ok(mut index) = ContentIndex::open(&cache_path) {
                let _ = index.build_with_runner(&root, &runner);
            }
        })
        .ok();
    IndexHandle { thread }
}

/// Start indexing the located install (or `root`) into the default cache.
/// Returns immediately; extract shares `runner` with the command.
/// Also persists the bundled serialize / behavior-context / module dumps.
#[must_use]
pub fn start_alongside(root: Option<PathBuf>, runner: JobRunner) -> IndexHandle {
    let Some(root) = root.or_else(|| {
        nw_locator::Install::locate()
            .ok()
            .map(|install| install.assets().to_path_buf())
    }) else {
        return IndexHandle { thread: None };
    };
    let cache_path = crate::cache::default_path();
    let thread = std::thread::Builder::new()
        .name("nw-content-index".to_owned())
        .spawn(move || {
            if let Ok(mut index) = ContentIndex::open(&cache_path) {
                let _ = index.build_with_runner(&root, &runner);
                let _ = index.ingest_embedded();
            }
        })
        .ok();
    IndexHandle { thread }
}
