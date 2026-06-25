use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

use anyhow::Result;
use nw_localization::{
    KeyFileIndex, LANGUAGE_MANIFEST_ASSET_PATH, LanguageCode, LanguageManifest, LocalizationCatalog,
    LocalizationCatalogBuilder, LocalizationDocument, LocalizationKey, LocalizationLoader,
    LocalizationTag, localization_asset_path,
};
use nw_pak::PakMmapReader;

use crate::support::{PakSet, collect_matching};
use crate::tui::SheetSource;
use crate::ui::Report;

use super::common::path_label;

/// The active localization language and last error.
struct LocaleSlot {
    code: Option<String>,
    error: Option<String>,
}

/// A language's growing catalog: files are added on demand (per sheet), so it only
/// ever holds the entries the user has actually looked at.
struct LangState {
    builder: LocalizationCatalogBuilder,
    catalog: Arc<LocalizationCatalog>,
    loaded: HashSet<String>,
}

impl LangState {
    fn new(language: LanguageCode) -> Self {
        let builder = LocalizationCatalog::builder(language);
        let catalog = Arc::new(builder.clone().build());
        Self {
            builder,
            catalog,
            loaded: HashSet::new(),
        }
    }
}

/// Where one sheet's bytes come from: a loose file on disk, or an entry inside a
/// memory-mapped pak archive.
#[derive(Clone)]
enum SheetSrc {
    File(PathBuf),
    Pak {
        reader: Arc<PakMmapReader>,
        index: usize,
    },
}

/// The growing set of discovered sheets. Append-only so sheet ids stay stable
/// while the background discovery sweep fills it in.
#[derive(Default)]
struct Discovered {
    sources: Vec<SheetSrc>,
    labels: Vec<String>,
    sizes: Vec<u64>,
}

/// A workspace of datasheets browsed together — either loose files or the
/// datasheet entries streamed straight out of the game's pak archives. Pak
/// discovery and the cross-reference index both run on a background thread pool
/// so the picker is usable from the first frame and fills in live; queries
/// return whatever is ready and grow over time. Localization is loaded lazily so
/// the language can be switched from inside the TUI without blocking startup.
struct DatasheetWorkspace {
    me: Weak<DatasheetWorkspace>,
    sheets: Mutex<Discovered>,
    /// Shared worker pool (honors the `--jobs` setting) for discovery, indexing,
    /// and parallel localization loads.
    runner: nw_jobs::JobRunner,
    /// Pak files to scan for datasheets (empty in file mode).
    pak_paths: Vec<PathBuf>,
    /// Paks scanned so far, and the total to scan.
    scanned: AtomicUsize,
    discover_total: usize,
    discover_done: AtomicBool,
    index: Mutex<HashMap<String, Vec<crate::tui::Loc>>>,
    indexed: AtomicUsize,
    index_done: AtomicBool,
    cancel: AtomicBool,
    // Localization, loaded and swapped on demand from a background thread.
    asset_root: Option<PathBuf>,
    /// Available language codes, enumerated lazily from the install on first use.
    languages: Mutex<Option<Vec<String>>>,
    locale: Mutex<LocaleSlot>,
    /// Bumped whenever resolved localization changes; the viewer watches this.
    loc_gen: AtomicU64,
    /// True while a background localization load is in flight (drives the
    /// "loading" indicator and serializes ensures).
    loc_ensuring: AtomicBool,
    /// Growing catalogs per language (only the touched files' entries).
    langs: Mutex<HashMap<String, LangState>>,
    /// Key → file index (built once, persisted), enabling per-sheet targeted loads.
    key_index: Mutex<Option<Arc<KeyFileIndex>>>,
    /// All localization source file names, cached for the no-index fallback.
    source_names: Mutex<Option<Vec<String>>>,
    /// Localization files found during discovery: virtual path → owning pak
    /// reader. Seeds the asset store so locale loads resolve in O(1) and reuse
    /// the paks discovery already opened (no re-parsing). Only localization paks
    /// are retained here — a handful — so the memory cost is small.
    loc_index: Mutex<HashMap<String, Arc<PakMmapReader>>>,
}

impl DatasheetWorkspace {
    fn new(
        sheets: Discovered,
        pak_paths: Vec<PathBuf>,
        discover_done: bool,
        asset_root: Option<PathBuf>,
        jobs: Option<usize>,
    ) -> Arc<Self> {
        let discover_total = pak_paths.len();
        let runner =
            nw_jobs::JobRunner::from_jobs(jobs).unwrap_or_else(|_| nw_jobs::JobRunner::automatic());
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            sheets: Mutex::new(sheets),
            runner,
            pak_paths,
            scanned: AtomicUsize::new(0),
            discover_total,
            discover_done: AtomicBool::new(discover_done),
            index: Mutex::new(HashMap::new()),
            indexed: AtomicUsize::new(0),
            index_done: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            asset_root,
            languages: Mutex::new(None),
            locale: Mutex::new(LocaleSlot {
                code: None,
                error: None,
            }),
            loc_gen: AtomicU64::new(0),
            loc_ensuring: AtomicBool::new(false),
            langs: Mutex::new(HashMap::new()),
            key_index: Mutex::new(None),
            source_names: Mutex::new(None),
            loc_index: Mutex::new(HashMap::new()),
        })
    }

    /// Build a workspace over loose `.datasheet` files on disk (discovery is
    /// immediate — the list is known up front).
    fn from_files(
        paths: Vec<PathBuf>,
        asset_root: Option<PathBuf>,
        jobs: Option<usize>,
    ) -> Arc<Self> {
        let labels = paths.iter().map(|path| path_label(path)).collect();
        let sizes = paths
            .iter()
            .map(|path| std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0))
            .collect();
        let sources = paths.into_iter().map(SheetSrc::File).collect();
        let sheets = Discovered {
            sources,
            labels,
            sizes,
        };
        Self::new(sheets, Vec::new(), true, asset_root, jobs)
    }

    /// Build a workspace that streams datasheets out of `pak_paths`. Returns
    /// instantly; the paks are opened and scanned on the background pool.
    fn from_paks(
        pak_paths: Vec<PathBuf>,
        asset_root: Option<PathBuf>,
        jobs: Option<usize>,
    ) -> Arc<Self> {
        Self::new(Discovered::default(), pak_paths, false, asset_root, jobs)
    }

    fn sheet_lock(&self) -> std::sync::MutexGuard<'_, Discovered> {
        self.sheets.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn len(&self) -> usize {
        self.sheet_lock().sources.len()
    }

    /// Available language codes known so far (non-blocking). Empty until the
    /// background sweep has read the install's language manifest.
    fn cached_languages(&self) -> Vec<String> {
        self.languages
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .unwrap_or_default()
    }

    fn languages_known(&self) -> bool {
        self.languages
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
    }

    /// Store the enumerated language codes (first writer wins).
    fn set_languages(&self, manifest: &LanguageManifest) {
        let codes = manifest
            .languages()
            .iter()
            .map(|entry| entry.code().as_str().to_string())
            .collect::<Vec<_>>();
        let mut slot = self
            .languages
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if slot.is_none() {
            *slot = Some(codes);
        }
    }

    /// An asset store seeded with the localization paks discovery already opened,
    /// so locale reads resolve in O(1) without re-parsing any pak.
    fn asset_store(&self) -> Option<nw_asset::AssetStore> {
        let root = self.asset_root.as_ref()?;
        let store = nw_asset::AssetStore::open(root).ok()?;
        let index = self
            .loc_index
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        store.seed_paths(index.iter().map(|(path, reader)| (path.clone(), reader.clone())));
        Some(store)
    }

    /// Enumerate languages through the asset store. Used in file mode (no
    /// discovery sweep), off the UI thread.
    fn warm_languages(&self) {
        if self.languages_known() {
            return;
        }
        if let Some(assets) = self.asset_store()
            && let Ok(Some(bytes)) = assets.read_path(LANGUAGE_MANIFEST_ASSET_PATH)
            && let Ok(manifest) = LanguageManifest::parse_bytes(&bytes)
        {
            self.set_languages(&manifest);
        }
    }

    /// Capture languages from an already-open pak if it holds the manifest — free
    /// during discovery, so the language list appears within seconds.
    fn capture_languages(&self, reader: &PakMmapReader) {
        if self.languages_known() {
            return;
        }
        if let Some(entry) = reader.entry(LANGUAGE_MANIFEST_ASSET_PATH)
            && let Ok(bytes) = reader.read_by_index(entry.index())
            && let Ok(manifest) = LanguageManifest::parse_bytes(&bytes)
        {
            self.set_languages(&manifest);
        }
    }

    /// Read one sheet's raw datasheet bytes, from disk or straight out of its pak.
    fn read_bytes(&self, sheet: usize) -> Option<Vec<u8>> {
        let source = self.sheet_lock().sources.get(sheet)?.clone();
        match source {
            SheetSrc::File(path) => std::fs::read(path).ok(),
            SheetSrc::Pak { reader, index } => reader.read_by_index(index).ok(),
        }
    }

    fn lock_locale(&self) -> std::sync::MutexGuard<'_, LocaleSlot> {
        self.locale
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn active_code(&self) -> Option<String> {
        self.lock_locale().code.clone()
    }

    /// The current (growing) catalog for `code` — covers whatever files have been
    /// loaded so far for it.
    fn lang_catalog(&self, code: &str) -> Option<Arc<LocalizationCatalog>> {
        self.langs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(code)
            .map(|state| state.catalog.clone())
    }

    /// Every localization source file name for the install (cached). The no-index
    /// fallback loads all of these.
    fn all_source_names(&self) -> Vec<String> {
        if let Some(names) = self
            .source_names
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
        {
            return names;
        }
        let names = self
            .asset_store()
            .and_then(|assets| {
                LocalizationLoader::new(&assets, default_language())
                    .source_file_names()
                    .ok()
            })
            .unwrap_or_default();
        *self
            .source_names
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(names.clone());
        names
    }

    /// Source files still to load to resolve `keys` for `code` — targeted via the
    /// index when ready, else the whole language.
    fn needed_files(&self, code: &str, keys: &[LocalizationKey]) -> Vec<String> {
        let loaded = self
            .langs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(code)
            .map(|state| state.loaded.clone())
            .unwrap_or_default();
        let wanted: HashSet<String> = match self
            .key_index
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
        {
            Some(index) => keys
                .iter()
                .filter_map(|key| index.file_name(key).map(str::to_string))
                .collect(),
            None => self.all_source_names().into_iter().collect(),
        };
        wanted
            .into_iter()
            .filter(|name| !loaded.contains(name))
            .collect()
    }

    /// Load the files holding `keys` for `code` in the background, then bump the
    /// locale generation so the grid re-renders with the resolved text. One ensure
    /// runs at a time; the generation bump re-triggers `load` for anything still
    /// missing.
    fn ensure_localization(&self, code: String, keys: Vec<LocalizationKey>) {
        if self.needed_files(&code, &keys).is_empty() {
            return;
        }
        if self.loc_ensuring.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(this) = self.me.upgrade() else {
            self.loc_ensuring.store(false, Ordering::SeqCst);
            return;
        };
        std::thread::spawn(move || {
            let names = this.needed_files(&code, &keys);
            this.run_ensure(&code, names);
            this.loc_ensuring.store(false, Ordering::SeqCst);
            this.loc_gen.fetch_add(1, Ordering::SeqCst);
        });
    }

    fn run_ensure(&self, code: &str, names: Vec<String>) {
        if names.is_empty() {
            return;
        }
        let Ok(language) = code.parse::<LanguageCode>() else {
            return;
        };
        let Some(assets) = self.asset_store() else {
            return;
        };
        // Read + parse the needed files in parallel on the --jobs pool.
        let documents = self.runner.map(&names, |name| {
            let path = localization_asset_path(&language, name);
            let bytes = assets.read_path(&path).ok().flatten()?;
            let document = LocalizationDocument::parse_bytes(&bytes).ok()?;
            Some((name.clone(), document))
        });
        let mut langs = self
            .langs
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let state = langs
            .entry(code.to_string())
            .or_insert_with(|| LangState::new(language.clone()));
        for entry in documents.into_iter().flatten() {
            let (name, document) = entry;
            let _ =
                state
                    .builder
                    .add_document(name.into(), &LocalizationTag::init(), &document);
        }
        // Mark every requested file loaded (even absent ones) so we never retry it.
        state.loaded.extend(names);
        state.catalog = Arc::new(state.builder.clone().build());
    }

    /// A fingerprint of the install's paks (size + mtime) used to invalidate a
    /// persisted index when the game updates.
    fn pak_fingerprint(&self) -> u64 {
        let fold = |hash: u64, value: u64| (hash ^ value).wrapping_mul(0x0000_0001_0000_01b3);
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for path in &self.pak_paths {
            if let Ok(meta) = std::fs::metadata(path) {
                hash = fold(hash, meta.len());
                if let Ok(modified) = meta.modified()
                    && let Ok(elapsed) = modified.duration_since(std::time::UNIX_EPOCH)
                {
                    hash = fold(hash, elapsed.as_secs());
                }
            }
        }
        hash
    }

    fn set_key_index(&self, index: KeyFileIndex) {
        *self
            .key_index
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Arc::new(index));
        // Re-render so subsequent per-sheet loads target only the needed files.
        self.loc_gen.fetch_add(1, Ordering::SeqCst);
    }
}

impl crate::tui::SheetSource for DatasheetWorkspace {
    fn load(&self, sheet: u32) -> Option<crate::tui::SheetData> {
        let bytes = self.read_bytes(sheet as usize)?;
        let mut doc = nw_datasheet::Datasheet::parse(&bytes).ok()?;
        let code = self.active_code();
        let catalog = code.as_ref().and_then(|code| self.lang_catalog(code));
        if let Some(catalog) = &catalog {
            doc.set_localization(Some(catalog.as_ref()));
        }
        let has_localization = code.is_some();
        // Kick off loading this sheet's localization files in the background;
        // until they land, in-progress keys render as plain keys (not red).
        if let Some(code) = code {
            self.ensure_localization(code, collect_sheet_keys(&doc));
        }
        let in_progress = self.loc_ensuring.load(Ordering::SeqCst);
        let columns = doc
            .columns()
            .iter()
            .map(|column| crate::tui::GridColumn {
                name: column.name().to_string(),
                kind: grid_type(column.column_type()),
            })
            .collect();
        let mut rows = Vec::with_capacity(doc.len());
        for row in doc.rows() {
            rows.push(
                row.cells()
                    .iter()
                    .map(|cell| grid_cell(&doc, cell, has_localization, in_progress))
                    .collect(),
            );
        }
        Some(crate::tui::SheetData {
            label: self.label(sheet),
            columns,
            rows,
            has_localization,
        })
    }

    fn label(&self, sheet: u32) -> String {
        self.sheet_lock()
            .labels
            .get(sheet as usize)
            .cloned()
            .unwrap_or_default()
    }

    fn references(&self, value: &str) -> Vec<crate::tui::Loc> {
        self.index
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(value)
            .cloned()
            .unwrap_or_default()
    }

    fn progress(&self) -> crate::tui::IndexProgress {
        crate::tui::IndexProgress {
            indexed: self.indexed.load(Ordering::Relaxed),
            total: self.len(),
            done: self.index_done.load(Ordering::Relaxed),
        }
    }

    fn sheets(&self) -> Vec<(String, u64)> {
        let sheets = self.sheet_lock();
        sheets
            .labels
            .iter()
            .cloned()
            .zip(sheets.sizes.iter().copied())
            .collect()
    }

    fn discovery(&self) -> crate::tui::IndexProgress {
        crate::tui::IndexProgress {
            indexed: self.scanned.load(Ordering::Relaxed),
            total: self.discover_total,
            done: self.discover_done.load(Ordering::Relaxed),
        }
    }

    fn locales(&self) -> Vec<String> {
        self.cached_languages()
    }

    fn locale(&self) -> crate::tui::LocaleState {
        let slot = self.lock_locale();
        crate::tui::LocaleState {
            code: slot.code.clone(),
            loading: self.loc_ensuring.load(Ordering::SeqCst),
            generation: self.loc_gen.load(Ordering::Relaxed),
            error: slot.error.clone(),
        }
    }

    fn set_locale(&self, code: Option<&str>) {
        // Just record the active language and re-render. The grid's reload calls
        // `load`, which resolves the visible sheet's keys (loading their files on
        // demand). Per-language catalogs persist, so revisiting a language reuses
        // whatever was already loaded.
        {
            let mut slot = self.lock_locale();
            slot.code = code.map(str::to_string);
            slot.error = None;
        }
        self.loc_gen.fetch_add(1, Ordering::SeqCst);
    }
}

/// Background pipeline: discover datasheets across the paks in parallel (the
/// picker streams as they appear), then build the cross-reference index in
/// parallel. Both phases run on a shared thread pool so nothing blocks the UI.
fn discover_and_index(workspace: Arc<DatasheetWorkspace>) {
    let runner = &workspace.runner;

    if workspace.pak_paths.is_empty() {
        // File mode: no pak sweep, so enumerate languages directly (off the UI thread).
        workspace.warm_languages();
    } else {
        runner.map(&workspace.pak_paths, |path| discover_pak(&workspace, path));
    }
    workspace.discover_done.store(true, Ordering::Relaxed);

    let ids = (0..workspace.len()).collect::<Vec<_>>();
    runner.map(&ids, |&id| index_sheet(&workspace, id));
    workspace.index_done.store(true, Ordering::Relaxed);
}

/// Open one pak, append its datasheet entries to the workspace, and bump the
/// scan counter. Runs on a worker thread.
fn discover_pak(workspace: &DatasheetWorkspace, path: &Path) {
    if !workspace.cancel.load(Ordering::Relaxed)
        && let Ok(reader) = PakMmapReader::open(path)
    {
        let reader = Arc::new(reader);
        // Grab the language list for free if this pak holds the manifest.
        workspace.capture_languages(&reader);
        // Classify entries in one pass: datasheets feed the picker, localization
        // files feed the asset-store fast-path index.
        let mut found = Vec::new();
        let mut loc = Vec::new();
        for entry in reader.entries() {
            let name = entry.name();
            if nw_datasheet::is_datasheet_path(Path::new(name)) {
                found.push((entry.index(), name.to_string(), entry.uncompressed_size()));
            } else if is_localization_entry(name) {
                loc.push(name.to_string());
            }
        }
        if !found.is_empty() {
            let mut sheets = workspace.sheet_lock();
            for (index, name, size) in found {
                sheets.sources.push(SheetSrc::Pak {
                    reader: reader.clone(),
                    index,
                });
                sheets.labels.push(name);
                sheets.sizes.push(size);
            }
        }
        if !loc.is_empty() {
            let mut index = workspace
                .loc_index
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for name in loc {
                index.insert(name, reader.clone());
            }
        }
    }
    workspace.scanned.fetch_add(1, Ordering::Relaxed);
}

/// Whether an archive entry is a localization file or manifest — the assets the
/// locale loader reads. Cheap suffix check (no allocation) over many entries.
fn is_localization_entry(name: &str) -> bool {
    fn ends_ci(value: &str, suffix: &str) -> bool {
        value.len() >= suffix.len()
            && value[value.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    }
    ends_ci(name, ".loc.xml") || ends_ci(name, ".loc") || ends_ci(name, "localization.xml")
}

/// Index one sheet's string cells into the cross-reference map. Runs on a worker
/// thread.
fn index_sheet(workspace: &DatasheetWorkspace, id: usize) {
    if !workspace.cancel.load(Ordering::Relaxed)
        && let Some(bytes) = workspace.read_bytes(id)
        && let Ok(doc) = nw_datasheet::Datasheet::parse(&bytes)
    {
        let mut local: Vec<(String, crate::tui::Loc)> = Vec::new();
        for (row_index, row) in doc.rows().enumerate() {
            for (col_index, cell) in row.cells().iter().enumerate() {
                if let Some(value) = cell.as_str()
                    && !value.is_empty()
                {
                    local.push((
                        value.to_string(),
                        crate::tui::Loc {
                            sheet: id as u32,
                            row: row_index as u32,
                            col: col_index as u32,
                        },
                    ));
                }
            }
        }
        if !local.is_empty() {
            let mut index = workspace
                .index
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for (value, loc) in local {
                index.entry(value).or_default().push(loc);
            }
        }
    }
    workspace.indexed.fetch_add(1, Ordering::Relaxed);
}

/// Open the datasheet grid TUI. With an explicit `path` the workspace is the
/// loose `.datasheet` files there; with no path it streams every datasheet entry
/// out of the located game install's paks — that's the front door.
pub(super) fn browse_datasheets(
    path: Option<PathBuf>,
    loc_root: Option<PathBuf>,
    initial_locale: Option<String>,
    mode: u8,
    jobs: Option<usize>,
) -> Result<()> {
    let workspace = match path {
        Some(path) => {
            // Loose extracted datasheets under a directory (or a single file).
            let paths = collect_matching(&path, nw_datasheet::is_datasheet_path)?;
            if paths.is_empty() {
                Report::new("datasheet").stat("files", 0usize).print();
                return Ok(());
            }
            // Localization still auto-locates (or uses --loc-root) for these.
            let asset_root = loc_root.or_else(|| {
                nw_locator::Install::locate()
                    .ok()
                    .map(|install| install.assets())
            });
            DatasheetWorkspace::from_files(paths, asset_root, jobs)
        }
        None => {
            // No path: locate the install and stream datasheets from its paks.
            // We only enumerate the pak *files* here (fast); opening and scanning
            // them happens in parallel on the background pool.
            let install = match nw_locator::Install::locate() {
                Ok(install) => install,
                Err(_) => {
                    Report::new("datasheet")
                        .note("no New World install found — pass a directory of .datasheet files")
                        .print();
                    return Ok(());
                }
            };
            let pak_root = install.assets();
            let asset_root = Some(loc_root.unwrap_or_else(|| install.assets()));
            let paks = PakSet::collect(pak_root, Vec::new())?;
            if paks.paths().is_empty() {
                Report::new("datasheet")
                    .note("no pak archives found in the install")
                    .print();
                return Ok(());
            }
            DatasheetWorkspace::from_paks(paks.paths().to_vec(), asset_root, jobs)
        }
    };

    if let Some(code) = initial_locale {
        workspace.set_locale(Some(&code));
    }
    // Enumerate languages on their own thread so the `L` picker is populated
    // quickly, independent of (and in parallel with) the full discovery sweep.
    let warm = Arc::clone(&workspace);
    std::thread::spawn(move || warm.warm_languages());
    // Load (or build + persist) the key→file index so localization can resolve
    // per sheet by reading only the files it needs.
    let indexer = Arc::clone(&workspace);
    std::thread::spawn(move || load_or_build_key_index(indexer));
    let worker = Arc::clone(&workspace);
    std::thread::spawn(move || discover_and_index(worker));
    let source: Arc<dyn crate::tui::SheetSource> = workspace.clone();

    let result = crate::tui::datasheet_browser(source, mode);
    workspace.cancel.store(true, Ordering::Relaxed);
    Ok(result?)
}

/// The distinct localization keys referenced by a sheet's `@`-prefixed cells.
fn collect_sheet_keys(sheet: &nw_datasheet::Datasheet<'_>) -> Vec<LocalizationKey> {
    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    for row in sheet.rows() {
        for cell in row.cells() {
            if let Some(value) = cell.as_str()
                && value.starts_with('@')
                && let Ok(key) = LocalizationKey::from_label(value)
                && seen.insert(key.crc32())
            {
                keys.push(key);
            }
        }
    }
    keys
}

/// English is always present in a New World install; used to build the
/// language-invariant key→file index and to enumerate source file names.
fn default_language() -> LanguageCode {
    LanguageCode::new("en-US").expect("en-US is a valid language code")
}

/// On-disk location for the cached key→file index.
fn key_index_cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    Some(base.join("nw-tools").join("loc-key-index.bin"))
}

fn read_cached_key_index(path: &Path, version: u64) -> Option<KeyFileIndex> {
    let index = KeyFileIndex::from_bytes(&std::fs::read(path).ok()?)?;
    (index.version() == version).then_some(index)
}

fn write_cached_key_index(path: &Path, index: &KeyFileIndex) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, index.to_bytes());
}

/// Load the persisted key→file index, or build it once (reading every loc file)
/// and persist it. Runs in the background; until it lands, per-sheet loads use the
/// load-everything fallback.
fn load_or_build_key_index(workspace: Arc<DatasheetWorkspace>) {
    if workspace.pak_paths.is_empty() {
        return; // file mode: no index, the fallback covers it
    }
    let version = workspace.pak_fingerprint();
    let Some(path) = key_index_cache_path() else {
        return;
    };
    if let Some(index) = read_cached_key_index(&path, version) {
        workspace.set_key_index(index);
        return;
    }
    let Some(assets) = workspace.asset_store() else {
        return;
    };
    let built = workspace
        .runner
        .install(|| LocalizationLoader::new(&assets, default_language()).build_key_index(version));
    if let Ok(index) = built {
        write_cached_key_index(&path, &index);
        workspace.set_key_index(index);
    }
}

fn grid_type(kind: nw_datasheet::ColumnType) -> crate::tui::GridType {
    match kind {
        nw_datasheet::ColumnType::String => crate::tui::GridType::String,
        nw_datasheet::ColumnType::Number => crate::tui::GridType::Number,
        nw_datasheet::ColumnType::Boolean => crate::tui::GridType::Boolean,
    }
}

fn grid_cell(
    sheet: &nw_datasheet::Datasheet<'_>,
    cell: &nw_datasheet::Cell<'_>,
    has_localization: bool,
    in_progress: bool,
) -> crate::tui::GridCell {
    let kind = grid_type(cell.column_type());
    if let Some(value) = cell.as_str() {
        let is_key = value.starts_with('@');
        let (localized, unresolved) = if is_key && has_localization {
            let text = sheet.localized(value);
            if text.as_ref() == value {
                // Not resolved: while its file is still loading show a plain key
                // (not the red "unresolved" state); flag it only once loading is done.
                if in_progress {
                    (None, false)
                } else {
                    (Some(text.into_owned()), true)
                }
            } else {
                (Some(text.into_owned()), false)
            }
        } else {
            (None, false)
        };
        crate::tui::GridCell {
            display: value.to_string(),
            localized,
            kind,
            is_key,
            unresolved,
        }
    } else {
        crate::tui::GridCell {
            display: cell.to_string(),
            localized: None,
            kind,
            is_key: false,
            unresolved: false,
        }
    }
}
