//! Interactive DDS texture browser — an engine-style asset grid. A scrollable
//! grid of image thumbnails (via the kitty/sixel/iterm graphics protocols with a
//! unicode half-block fallback), filterable by path, with Enter to focus one
//! texture for a full mip/surface view. Thumbnails decode AND encode on a
//! background thread (so the UI never blocks on image work) as textures stream in
//! from discovery, and stale decode batches are cancelled when the view moves.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver as CbReceiver, Sender as CbSender, unbounded};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use image::{DynamicImage, ImageEncoder, RgbaImage};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect, Size};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};

use nw_jobs::{CancellationToken, JobRunner};

use super::app::{Flow, View};
use crate::fuzzy;
use crate::ui::theme::{self, Caps};

const HILITE: ratatui::style::Color = ratatui::style::Color::Indexed(238);

/// Largest preview decoded off-thread; the widget downscales further to the pane.
const PREVIEW_MAX: u32 = 1024;

/// Target grid cell width in terminal cells; columns are chosen so cells land
/// near this, clamped to [`MIN_COLS`, `MAX_COLS`]. The thumbnail fills the cell
/// minus a 1-cell gutter, with a label row beneath.
const TARGET_CELL_W: u16 = 40;
const MIN_COLS: u16 = 2;
const MAX_COLS: u16 = 6;
/// Keep this many decoded thumbnails before evicting (oldest off-screen first), so
/// scrolling back to a recently-viewed row is instant (no re-decode/re-encode).
/// Sized for several screens of frame-heavy sprite sequences; thumbnails are
/// cell-sized protocols, so this stays well-bounded in memory.
const THUMB_CACHE_CAP: usize = 2048;
/// Thumbnails are decoded + cached at this size (longest edge); the protocol is
/// then fit to whatever cell, so one cache entry serves any grid size.
const THUMB_CACHE_PX: u32 = 512;
/// How many recently-focused textures keep their full decoded previews (RGBA mip
/// chains, all sprite frames) in memory. Bounds the otherwise-unbounded preview
/// cache; the current focus is always pinned. Disk/thumbnail caches stay large.
const MAX_PREVIEW_ITEMS: usize = 3;
/// Cap the on-disk thumbnail cache; oldest files are pruned past this. The disk
/// cache is content-addressed and costs no RAM, so keep it generous — a hit skips
/// the pak sidecar reads + DDS decode entirely, across sessions and game patches.
const THUMB_DISK_CAP: u64 = 1024 * 1024 * 1024;

/// Sprite playback rate bounds and default. The `.dds` files carry no rate of
/// their own; the engine keeps it on the `UiFlipbookAnimationComponent` in the
/// `.uicanvas`, where the content overwhelmingly uses 30 fps (some at 24, a few at
/// 50) — so default to 30 and allow up to 60 (terminal image animation tops out
/// well below that, but allow it).
const FPS_MIN: u32 = 1;
const FPS_MAX: u32 = 60;
const FPS_DEFAULT: u32 = 30;

/// Resolves a texture key (a pak virtual path) to the archive entry that holds
/// it. Built during discovery from the readers it already opened, so reads are an
/// O(1) map lookup + decompress — no per-read scan over every pak.
pub type PakIndex = HashMap<String, (Arc<nw_pak::PakMmapReader>, usize)>;

/// The pak index shared between background discovery (which fills it) and the
/// [`TextureStore`] (which reads it), so the browser can open before discovery
/// finishes.
pub type SharedIndex = Arc<RwLock<PakIndex>>;

/// A fresh, empty [`SharedIndex`].
#[must_use]
pub fn shared_index() -> SharedIndex {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Where texture bytes are read from: the filesystem, or straight out of the
/// install's pak archives (by entry index — no catalog needed, the install ships
/// none).
pub enum TextureStore {
    Fs,
    Pak(SharedIndex),
}

impl TextureStore {
    fn read(&self, key: &str) -> Result<Vec<u8>, String> {
        match self {
            TextureStore::Fs => std::fs::read(key).map_err(|error| error.to_string()),
            TextureStore::Pak(index) => {
                let index = index.read().unwrap_or_else(|error| error.into_inner());
                let (reader, entry) = index
                    .get(key)
                    .ok_or_else(|| format!("not found in paks: {key}"))?;
                // Decompress without peeling an AZCS wrapper — DDS bytes are raw.
                reader
                    .read_wrapped_by_index(*entry)
                    .map_err(|error| error.to_string())
            }
        }
    }
}

/// Textures discovered so far, filled by a background discovery thread while the
/// browser is already on screen. The browser ingests new items on each tick, so
/// scanning the install never blocks the UI.
pub struct DdsCatalog {
    items: Mutex<Vec<DdsItem>>,
    scanned: AtomicUsize,
    total: usize,
}

impl DdsCatalog {
    /// A catalog awaiting discovery of `total` paks.
    #[must_use]
    pub fn new(total: usize) -> Self {
        Self {
            items: Mutex::new(Vec::new()),
            scanned: AtomicUsize::new(0),
            total,
        }
    }

    /// A fully-discovered catalog from a known item set — nothing scans in the
    /// background (used by the filesystem browser, where discovery is cheap).
    #[must_use]
    pub fn ready(items: Vec<DdsItem>) -> Self {
        Self {
            items: Mutex::new(items),
            scanned: AtomicUsize::new(0),
            total: 0,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<DdsItem>> {
        self.items.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Append a discovered pak's grouped textures.
    pub fn extend(&self, items: Vec<DdsItem>) {
        self.lock().extend(items);
    }

    /// Record that one more pak finished scanning.
    pub fn mark_pak_done(&self) {
        self.scanned.fetch_add(1, Ordering::Relaxed);
    }

    /// Clone the items appended since the caller already had `have` of them.
    fn tail_from(&self, have: usize) -> Vec<DdsItem> {
        let items = self.lock();
        items.get(have..).map(<[DdsItem]>::to_vec).unwrap_or_default()
    }

    fn item(&self, index: usize) -> Option<DdsItem> {
        self.lock().get(index).cloned()
    }

    /// Paks scanned so far, and the total.
    fn progress(&self) -> (usize, usize) {
        (self.scanned.load(Ordering::Relaxed), self.total)
    }

    fn is_scanning(&self) -> bool {
        self.scanned.load(Ordering::Relaxed) < self.total
    }
}

/// One frame of a texture: its DDS header path, split sidecars, and optional
/// attached-alpha surface. A standalone texture is a single frame; a sprite
/// sequence (`X_NN.dds`) has several.
#[derive(Clone)]
pub struct DdsFrame {
    pub header: String,
    pub sidecars: Vec<(nw_dds::SplitPart, String)>,
    /// The attached-alpha surface (`.dds.a` header + `.dds.Na` mips), if present —
    /// a second image of the same texture (gloss/opacity), viewable on its own.
    pub alpha: Option<AlphaSurface>,
}

/// A browser entry: a single texture, or a sprite sequence shown as one
/// animatable entry (its `X_NN.dds` frames grouped into `frames`).
#[derive(Clone)]
pub struct DdsItem {
    /// Display label (relative path, forward slashes).
    pub label: String,
    /// One or more frames; more than one means an animated sprite.
    pub frames: Vec<DdsFrame>,
}

impl DdsItem {
    /// A single-frame entry.
    #[must_use]
    pub fn single(label: String, frame: DdsFrame) -> Self {
        Self { label, frames: vec![frame] }
    }

    /// The frame at `index`, wrapping — for animation.
    #[must_use]
    pub fn frame(&self, index: usize) -> &DdsFrame {
        &self.frames[index % self.frames.len().max(1)]
    }

    #[must_use]
    pub fn is_sprite(&self) -> bool {
        self.frames.len() > 1
    }
}

/// The attached-alpha companion surface of a [`DdsItem`].
#[derive(Clone)]
pub struct AlphaSurface {
    pub header: String,
    pub sidecars: Vec<(nw_dds::SplitPart, String)>,
}

/// One decoded mip level, sized for display. `width`/`height` are the mip's true
/// dimensions (the `image` may be downscaled for transmission).
struct Mip {
    width: u32,
    height: u32,
    image: RgbaImage,
}

/// One viewable surface of a texture (the base colour, or the attached alpha),
/// with its full mip chain (largest first).
struct Surface {
    name: &'static str,
    mips: Vec<Mip>,
}

/// A decoded texture: its metadata chips and one or more surfaces.
struct Preview {
    meta: Vec<(String, String)>,
    surfaces: Vec<Surface>,
}

/// Per-texture decode state shared between the worker and the UI thread.
enum Slot {
    Pending,
    Ready(Arc<Preview>),
    Failed(String),
}

/// Preview cache keyed by (item, frame) so each sprite frame is decoded once.
type Cache = Mutex<HashMap<(usize, usize), Slot>>;

/// A grid thumbnail's decode state: in-flight, an encoded protocol ready to blit,
/// or failed.
enum Thumb {
    Pending,
    // `Arc` so the UI can cheaply snapshot a ready protocol under a brief lock and
    // blit it after releasing the lock (never holding it across rendering).
    Ready(Arc<Protocol>),
    Failed,
}

/// A snapshot of one grid cell's visual, taken under a brief lock and rendered
/// after the lock is released.
enum CellVisual {
    Image(Arc<Protocol>),
    Failed,
    Pending,
}

/// The thumbnail cache: `(item, frame)` → (last-used clock, state). UI-OWNED (no
/// lock) — workers hand results back over a channel and the UI drains them. Most
/// cells use frame 0; a selected sprite caches every frame so it can animate. The
/// clock drives LRU eviction so decoded thumbnails survive scrolling away and are
/// only dropped, oldest-off-screen first, past a cap.
type Thumbs = HashMap<(usize, usize), (u64, Thumb)>;

/// Identifies the focus-view protocol: (item, frame, surface, mip, area w, area h).
type FocusKey = (usize, usize, usize, usize, u16, u16);

/// Work for the off-thread protocol encoder. `picker.new_protocol` (the costly
/// encode of decoded RGBA into a terminal graphics protocol) must never run on the
/// render thread, so the UI ships decoded previews here and blits the result later.
enum EncodeJob {
    /// Encode one frame's `(surface, mip)` at the focus image size.
    Frame {
        preview: Arc<Preview>,
        surface: usize,
        mip: usize,
    },
    /// Assemble all of a sprite's frames into one montage sheet and encode it.
    Montage { frames: Vec<Arc<Preview>> },
}

/// A request to encode a focus-view protocol off the UI thread. `gen` is bumped by
/// the UI when the focused texture or image size changes, so results for a stale
/// generation are discarded on arrival.
struct EncodeReq {
    key: FocusKey,
    generation: u64,
    job: EncodeJob,
}

/// The encoded result handed back to the UI (lock-free) via a crossbeam channel.
struct EncodeResult {
    key: FocusKey,
    generation: u64,
    protocol: Option<Arc<Protocol>>,
}

/// One thumbnail decode job for the worker pool: decode `key` = `(item, frame)` at
/// `size` (cells). `generation` is the cell-size epoch when queued — a worker drops
/// the job if the grid has since resized (the thumbnail would be the wrong size).
/// There is NO per-scroll cancellation: a queued job always finishes and is cached,
/// so scrolling back to a row is instant. Jobs are pushed item-major (all of one
/// sprite's frames together) so each grid cell becomes smooth independently rather
/// than the whole screen lighting up at once.
struct ThumbJob {
    key: (usize, usize),
    size: Size,
    generation: u64,
}

/// A decoded thumbnail handed back to the UI (lock-free) via a crossbeam channel.
struct ThumbResult {
    key: (usize, usize),
    generation: u64,
    thumb: Thumb,
}

/// A request to decode focus-view previews for `(item, frame)` pairs (all of a
/// sprite's frames, so playback is smooth). `cancel` is tripped when the focused
/// texture changes, so cycling doesn't pile up stale full decodes.
struct PreviewReq {
    items: Vec<(usize, usize)>,
    cancel: CancellationToken,
}

/// Persistent, terminal-independent thumbnail cache: the decoded+downscaled image
/// (QOI), *content-addressed* by a hash of the DDS header bytes. Because the key is
/// the texture's own content, a cached thumbnail is reused whenever that DDS is
/// unchanged — across sessions, resizes, and even game patches (an unchanged
/// texture keeps its entry; a changed one gets a new hash, and the stale file is
/// pruned). A hit skips the pak read of sidecars + the DDS decode.
struct ThumbCache {
    dir: Option<PathBuf>,
}

impl ThumbCache {
    fn open() -> Self {
        let dir = crate::cache::default_path()
            .parent()
            .map(|parent| parent.join("thumbnails"));
        if let Some(dir) = dir.clone() {
            let _ = std::fs::create_dir_all(&dir);
            // Prune off the UI thread: scanning/sorting the cache dir must never
            // delay the browser opening.
            std::thread::spawn(move || prune_dir(&dir, THUMB_DISK_CAP));
        }
        Self { dir }
    }

    fn path(&self, hash: u64) -> Option<PathBuf> {
        Some(self.dir.as_ref()?.join(format!("{hash:016x}.qoi")))
    }

    /// Load a cached thumbnail image by content hash, if present.
    fn load(&self, hash: u64) -> Option<RgbaImage> {
        let bytes = std::fs::read(self.path(hash)?).ok()?;
        image::load_from_memory_with_format(&bytes, image::ImageFormat::Qoi)
            .ok()
            .map(|image| image.to_rgba8())
    }

    /// Persist a decoded thumbnail image (best-effort; failures are ignored).
    fn store(&self, hash: u64, image: &RgbaImage) {
        let Some(path) = self.path(hash) else {
            return;
        };
        let mut bytes = Vec::new();
        if image::codecs::qoi::QoiEncoder::new(&mut bytes)
            .write_image(image, image.width(), image.height(), image::ExtendedColorType::Rgba8)
            .is_ok()
        {
            // Write to a sibling temp file then rename, so a concurrent reader (or a
            // crash mid-write) never sees a truncated QOI. The temp name is keyed by
            // the content hash, so parallel stores of different thumbnails don't clash.
            let temp = path.with_extension("qoi.tmp");
            if std::fs::write(&temp, &bytes).is_ok() && std::fs::rename(&temp, &path).is_err() {
                let _ = std::fs::remove_file(&temp);
            }
        }
    }
}

/// Stable 64-bit FNV-1a over bytes — content hash for cache keys.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Delete the oldest files in `dir` until its total size is within `cap`.
fn prune_dir(dir: &Path, cap: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            meta.is_file()
                .then(|| Some((meta.modified().ok()?, meta.len(), entry.path())))
                .flatten()
        })
        .collect();
    let total: u64 = files.iter().map(|(_, size, _)| size).sum();
    if total <= cap {
        return;
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    let mut over = total - cap;
    for (_, size, path) in files {
        if over == 0 {
            break;
        }
        let _ = std::fs::remove_file(path);
        over = over.saturating_sub(size);
    }
}

pub struct DdsBrowser {
    caps: Caps,
    /// Local snapshot of the discovered textures, grown from `catalog` on tick.
    /// Kept in discovery order so indices match the catalog (the decode cache key).
    items: Vec<DdsItem>,
    /// Background discovery, ingested progressively so the UI never blocks on it.
    catalog: Arc<DdsCatalog>,
    /// Where the textures came from — shown in the header (e.g. the install path).
    source: String,
    /// Lowercased label per item (parallel to `items`), for fuzzy filtering.
    haystacks: Vec<String>,
    /// All item indices sorted by label — the unfiltered display order.
    order: Vec<usize>,
    /// Item indices currently shown (the active filter applied to `order`).
    visible: Vec<usize>,
    selected: usize,
    /// First grid row drawn (vertical scroll), in grid rows.
    offset: usize,
    /// Grid columns from the last render (for 2-D navigation between frames).
    cols: usize,
    /// Thumbnail image size (cells) from the last render; thumbnails are decoded to
    /// fill it, so a terminal resize re-decodes at the new size.
    cell: Size,
    filter: String,
    filtering: bool,
    /// Whether the single-texture focus view is open (else the grid).
    focused: bool,
    /// Sprite playback frames per second (user-adjustable within [`FPS_MIN`,
    /// `FPS_MAX`]); the displayed frame is derived from wall-clock time so it's
    /// rate-correct regardless of redraw jitter.
    fps: u32,
    /// Whether sprite playback is running (else frozen on `base_frame`).
    playing: bool,
    /// Whether a sprite's frames are shown joined into one montage (all frames
    /// tiled) instead of cycling one at a time.
    joined: bool,
    /// Clock base for playback and the frame index at that base.
    anim_base: Instant,
    base_frame: usize,
    /// Selected surface (0=base, 1=alpha) for the focused texture.
    surface: usize,
    /// Selected mip level for the focused texture (clamped to its chain at render).
    mip: usize,
    // Full-preview decode (focus view).
    cache: Arc<Cache>,
    tx: SyncSender<PreviewReq>,
    /// Cancels the in-flight preview decode when the focused texture changes.
    preview_cancel: CancellationToken,
    // Thumbnail decode (grid) — lock-free worker pool, UI-owned cache.
    thumbs: Thumbs,
    /// Push decode jobs to the worker pool (item-major; no per-scroll cancel).
    thumb_jobs: CbSender<ThumbJob>,
    /// Drain decoded thumbnails from the pool (lock-free, non-blocking).
    thumb_results: CbReceiver<ThumbResult>,
    /// Cell-size epoch shared with the pool; bumped on grid resize so in-flight
    /// jobs decoded for the old cell size are dropped instead of cached.
    thumb_generation: Arc<AtomicU64>,
    /// Monotonic clock stamped onto thumbnails as they're shown, for LRU eviction.
    clock: u64,
    /// Whether the last grid render had a visible sprite — drives the animation
    /// redraw cadence while in the grid.
    grid_has_sprite: bool,
    /// Set when the focused image changes (cycle/enter/exit) so the image region is
    /// force-repainted next frame — graphics cells would otherwise leave residue.
    dirty: bool,
    /// The body (image) region from the last render, repainted when `dirty`.
    body: Rect,
    /// The focused image's draw rect from the last render; when the next frame's
    /// rect differs (a differently-sized sprite frame), we clear once to avoid
    /// residue — rather than clearing every frame.
    last_image: Option<Rect>,
    /// Send focus/montage encode work to the off-thread encoder.
    encode_tx: CbSender<EncodeReq>,
    /// Drain encoded protocols from the encoder (lock-free, non-blocking).
    encode_rx: CbReceiver<EncodeResult>,
    /// UI-owned cache of encoded focus protocols, keyed by [`FocusKey`]. Filled by
    /// the encoder; cleared when the focused texture or image size changes.
    focus_protocols: HashMap<FocusKey, Arc<Protocol>>,
    /// Focus keys with an encode in flight, so we don't re-request them each frame.
    focus_requested: HashSet<FocusKey>,
    /// Bumped on focus/size change; encode results from an older generation are
    /// dropped on arrival.
    encode_generation: u64,
    /// Recently-focused item indices (most recent at the back). Bounds the preview
    /// cache: previews for items evicted from this list are dropped.
    preview_lru: VecDeque<usize>,
    /// The focus image-area size from the last render; a change invalidates the
    /// encoded protocols (they were sized for the old area).
    focus_size: Option<(u16, u16)>,
    status: Option<String>,
    result: Option<String>,
}

impl DdsBrowser {
    pub fn new(
        catalog: Arc<DdsCatalog>,
        store: Arc<TextureStore>,
        source: String,
        runner: JobRunner,
        picker: Picker,
        caps: Caps,
    ) -> Self {
        // Decode on private pools so interactive work never queues behind (or is
        // starved by) background pak discovery, which runs on the passed-in `runner`.
        let workers = std::thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1).max(2))
            .unwrap_or(4);
        let decode_runner =
            JobRunner::with_workers(workers).unwrap_or_else(|_| runner.clone());
        let cache: Arc<Cache> = Arc::new(Mutex::new(HashMap::new()));
        let tx = spawn_decoder(catalog.clone(), store.clone(), decode_runner, cache.clone());
        let thumb_cache = Arc::new(ThumbCache::open());
        let thumb_generation = Arc::new(AtomicU64::new(0));
        let (thumb_jobs, thumb_results) = spawn_thumb_pool(
            workers,
            catalog.clone(),
            store,
            thumb_cache,
            picker.clone(),
            thumb_generation.clone(),
        );
        let (encode_tx, encode_rx) = spawn_encoder(picker);
        let mut browser = Self {
            caps,
            items: Vec::new(),
            catalog,
            source,
            haystacks: Vec::new(),
            order: Vec::new(),
            visible: Vec::new(),
            selected: 0,
            offset: 0,
            cols: 1,
            cell: Size::new(0, 0),
            filter: String::new(),
            filtering: false,
            focused: false,
            fps: FPS_DEFAULT,
            playing: true,
            joined: false,
            anim_base: Instant::now(),
            base_frame: 0,
            surface: 0,
            mip: 0,
            cache,
            tx,
            preview_cancel: CancellationToken::new(),
            thumbs: HashMap::new(),
            thumb_jobs,
            thumb_results,
            thumb_generation,
            clock: 0,
            grid_has_sprite: false,
            dirty: false,
            body: Rect::default(),
            last_image: None,
            encode_tx,
            encode_rx,
            focus_protocols: HashMap::new(),
            focus_requested: HashSet::new(),
            encode_generation: 0,
            preview_lru: VecDeque::new(),
            focus_size: None,
            status: None,
            result: None,
        };
        browser.ingest();
        browser
    }

    /// Pull any textures discovered since the last ingest into the snapshot, sort
    /// the display order by label, and reapply the filter. Returns whether
    /// anything new arrived.
    fn ingest(&mut self) -> bool {
        let fresh = self.catalog.tail_from(self.items.len());
        if fresh.is_empty() {
            return false;
        }
        for item in &fresh {
            self.haystacks.push(item.label.to_ascii_lowercase());
        }
        self.items.extend(fresh);
        self.order = (0..self.items.len()).collect();
        self.order.sort_by(|&a, &b| self.items[a].label.cmp(&self.items[b].label));
        self.rebuild();
        // New items can shift grid positions; clear residue once (cheap — only
        // while discovery is actively streaming, not on a settled grid).
        if !self.focused {
            self.dirty = true;
        }
        true
    }

    /// Recompute the visible rows: every texture in label order, or — when a
    /// filter is set — the fuzzy matches ranked best-first. Keeps the selection on
    /// the same texture where possible.
    fn rebuild(&mut self) {
        let anchor = self.current_item();
        self.visible = if self.filter.is_empty() {
            self.order.clone()
        } else {
            fuzzy::rank(&self.filter, &self.haystacks)
                .into_iter()
                .map(|(index, _)| index)
                .collect()
        };
        // Keep the selection on the same texture if it survived the filter;
        // otherwise jump to the best match (index 0), never the stale index —
        // which `.min(len-1)` would clamp to the *last* result.
        self.selected = anchor
            .and_then(|item| self.visible.iter().position(|&candidate| candidate == item))
            .unwrap_or(0)
            .min(self.visible.len().saturating_sub(1));
    }

    /// The focused texture (an index into `items`), if any.
    fn current_item(&self) -> Option<usize> {
        self.visible.get(self.selected).copied()
    }

    /// Frame count of the focused texture (1 for a non-sprite).
    fn focused_frames(&self) -> usize {
        self.current_item().map_or(1, |item| self.items[item].frames.len())
    }

    /// The frame to show now — derived from wall-clock time while playing, so it's
    /// rate-correct regardless of redraw cadence; frozen otherwise.
    fn current_frame(&self, count: usize) -> usize {
        if count <= 1 {
            return 0;
        }
        if self.playing {
            let advanced = (self.anim_base.elapsed().as_secs_f64() * f64::from(self.fps)) as usize;
            (self.base_frame + advanced) % count
        } else {
            self.base_frame % count
        }
    }

    /// True while sprites are actively cycling (drives redraw cadence): the focused
    /// sprite, or any sprite visible in the grid. The joined montage is static.
    fn animating(&self) -> bool {
        if !self.playing {
            return false;
        }
        if self.focused {
            return !self.joined && self.focused_frames() > 1;
        }
        self.grid_has_sprite
    }

    /// Toggle a sprite between the joined montage (all frames tiled) and cycling.
    fn toggle_joined(&mut self) {
        if self.focused_frames() > 1 {
            self.joined = !self.joined;
            self.status = None;
            self.dirty = true;
        }
    }

    /// Drain encoded focus protocols from the encoder into the UI-owned cache
    /// (non-blocking). Results from a superseded generation are discarded.
    fn drain_encodes(&mut self) {
        while let Ok(result) = self.encode_rx.try_recv() {
            self.focus_requested.remove(&result.key);
            if result.generation == self.encode_generation
                && let Some(protocol) = result.protocol
            {
                self.focus_protocols.insert(result.key, protocol);
            }
        }
    }

    /// Queue an off-thread encode for `key` unless it's already cached or in flight.
    fn request_focus_encode(&mut self, key: FocusKey, job: EncodeJob) {
        if self.focus_protocols.contains_key(&key) || !self.focus_requested.insert(key) {
            return;
        }
        let _ = self.encode_tx.send(EncodeReq {
            key,
            generation: self.encode_generation,
            job,
        });
    }

    /// Invalidate cached focus protocols (on a texture or image-size change) so any
    /// in-flight encodes for the old generation are dropped when they arrive.
    fn reset_focus_encodes(&mut self) {
        self.encode_generation = self.encode_generation.wrapping_add(1);
        self.focus_protocols.clear();
        self.focus_requested.clear();
    }

    /// True while the focused frame's preview or its encoded protocol is still
    /// pending — keeps the view redrawing until the image is ready to blit.
    fn focus_pending(&self) -> bool {
        if !self.focus_requested.is_empty() {
            return true;
        }
        match self.current_item() {
            Some(item) => {
                let frame = self.current_frame(self.items[item].frames.len());
                let cache = self.cache.lock().unwrap();
                !matches!(
                    cache.get(&(item, frame)),
                    Some(Slot::Ready(_)) | Some(Slot::Failed(_))
                )
            }
            None => false,
        }
    }

    /// Rebase playback onto the frame showing now (so toggling play/pause or
    /// changing fps is seamless).
    fn rebase_playback(&mut self) {
        self.base_frame = self.current_frame(self.focused_frames());
        self.anim_base = Instant::now();
    }

    fn toggle_play(&mut self) {
        self.rebase_playback();
        self.playing = !self.playing;
        self.dirty = true;
    }

    fn adjust_fps(&mut self, delta: i32) {
        self.rebase_playback();
        self.fps = (self.fps as i32 + delta).clamp(FPS_MIN as i32, FPS_MAX as i32) as u32;
        self.status = Some(format!("{} fps", self.fps));
        self.dirty = true;
    }

    /// Step frames manually (pauses playback).
    fn step_frame(&mut self, delta: isize) {
        let count = self.focused_frames();
        if count <= 1 {
            return;
        }
        let current = self.current_frame(count) as isize;
        self.playing = false;
        self.base_frame = (current + delta).rem_euclid(count as isize) as usize;
        self.dirty = true;
    }

    /// Move the grid selection by `delta` (±1 to step, ±cols to change row).
    fn move_by(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let last = (self.visible.len() - 1) as isize;
        self.selected = (self.selected as isize).saturating_add(delta).clamp(0, last) as usize;
        self.status = None;
        self.dirty = true; // content may scroll; clear residue once next frame
    }

    fn select(&mut self, index: usize) {
        self.selected = index.min(self.visible.len().saturating_sub(1));
        self.status = None;
        self.dirty = true;
    }

    /// Open the focus view on the current texture and request its full decode.
    fn focus(&mut self) {
        if self.current_item().is_some() {
            self.focused = true;
            self.surface = 0;
            self.mip = 0;
            self.base_frame = 0;
            self.anim_base = Instant::now();
            self.playing = true;
            self.joined = false;
            self.last_image = None;
            self.reset_focus_encodes();
            self.status = None;
            self.dirty = true;
            self.request_preview();
        }
    }

    /// Request a full decode of every frame of the focused texture (so sprite
    /// playback is smooth), cancelling any previous (now stale) decode.
    fn request_preview(&mut self) {
        if let Some(item) = self.current_item() {
            self.pin_preview_item(item);
            self.preview_cancel.cancel();
            let cancel = CancellationToken::new();
            self.preview_cancel = cancel.clone();
            let frames = (0..self.items[item].frames.len()).map(|frame| (item, frame)).collect();
            let _ = self.tx.try_send(PreviewReq { items: frames, cancel });
        }
    }

    /// Mark `item` most-recently focused and drop the decoded previews of textures
    /// that fall off the end — bounding the preview cache (the current focus, at the
    /// back, is never evicted). Sprite frames make full previews large, so retaining
    /// every focused texture would leak memory over a long session.
    fn pin_preview_item(&mut self, item: usize) {
        self.preview_lru.retain(|&existing| existing != item);
        self.preview_lru.push_back(item);
        while self.preview_lru.len() > MAX_PREVIEW_ITEMS {
            if let Some(evicted) = self.preview_lru.pop_front() {
                let mut cache = self.cache.lock().unwrap();
                cache.retain(|&(owner, _), _| owner != evicted);
            }
        }
    }

    /// Queue thumbnail decodes for the visible `items` (item-major: all of one
    /// sprite's frames together, so each cell loads as an independent unit). Skips
    /// frames already cached or in flight, refreshes their LRU stamp, then evicts
    /// down to the cache cap (keeping the visible window).
    fn request_thumbs(&mut self, items: &[usize]) {
        let generation = self.thumb_generation.load(Ordering::Relaxed);
        let size = self.cell;
        self.clock = self.clock.wrapping_add(1);
        let now = self.clock;
        let mut keep: HashSet<(usize, usize)> = HashSet::new();
        for &item in items {
            let frames = self.items[item].frames.len();
            let count = if frames > 1 { frames } else { 1 };
            for frame in 0..count {
                let key = (item, frame);
                keep.insert(key);
                match self.thumbs.get_mut(&key) {
                    Some((used, _)) => *used = now, // already cached/in-flight; keep warm
                    None => {
                        self.thumbs.insert(key, (now, Thumb::Pending));
                        let _ = self.thumb_jobs.send(ThumbJob { key, size, generation });
                    }
                }
            }
        }
        self.evict_thumbs(&keep);
    }

    /// Drain decoded thumbnails from the worker pool into the UI-owned cache
    /// (non-blocking). Results from a superseded cell-size generation are dropped.
    fn drain_thumbs(&mut self) {
        let current = self.thumb_generation.load(Ordering::Relaxed);
        while let Ok(result) = self.thumb_results.try_recv() {
            if result.generation == current
                && let Some(entry) = self.thumbs.get_mut(&result.key)
            {
                entry.1 = result.thumb;
            }
        }
    }

    /// Bound the thumbnail cache: only when it exceeds the cap, drop the
    /// least-recently-shown ready/failed thumbnails that aren't currently visible.
    /// In-flight (pending) and visible entries are never evicted, so moving around
    /// doesn't force a re-decode of what was just viewed.
    fn evict_thumbs(&mut self, keep: &HashSet<(usize, usize)>) {
        if self.thumbs.len() <= THUMB_CACHE_CAP {
            return;
        }
        let mut evictable: Vec<(u64, (usize, usize))> = self
            .thumbs
            .iter()
            .filter(|(key, (_, thumb))| {
                !keep.contains(*key) && matches!(thumb, Thumb::Ready(_) | Thumb::Failed)
            })
            .map(|(key, (used, _))| (*used, *key))
            .collect();
        evictable.sort_unstable();
        let excess = self.thumbs.len() - THUMB_CACHE_CAP;
        for (_, key) in evictable.into_iter().take(excess) {
            self.thumbs.remove(&key);
        }
    }

    /// Reset the focus view to the newly-selected texture and request its decode.
    fn refocus(&mut self) {
        self.surface = 0;
        self.mip = 0;
        self.base_frame = 0;
        self.anim_base = Instant::now();
        self.playing = true;
        self.joined = false;
        self.last_image = None;
        self.reset_focus_encodes();
        self.status = None;
        self.dirty = true;
        self.request_preview();
    }

    fn on_grid_key(&mut self, key: KeyEvent) -> Flow {
        let cols = self.cols.max(1) as isize;
        match key.code {
            KeyCode::Char('q') => return Flow::Quit,
            KeyCode::Esc => {
                if self.filter.is_empty() {
                    return Flow::Quit;
                }
                self.filter.clear();
                self.rebuild();
            }
            KeyCode::Char('h') | KeyCode::Left => self.move_by(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-cols),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(cols),
            KeyCode::PageDown => self.move_by(cols * 4),
            KeyCode::PageUp => self.move_by(-cols * 4),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_by(cols * 4);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_by(-cols * 4);
            }
            KeyCode::Char('g') | KeyCode::Home => self.select(0),
            KeyCode::Char('G') | KeyCode::End => self.select(self.visible.len().saturating_sub(1)),
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Char('y') => self.copy_path(),
            KeyCode::Enter => self.focus(),
            _ => {}
        }
        Flow::Continue
    }

    fn on_focus_key(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Char('q') => return Flow::Quit,
            KeyCode::Esc => {
                self.focused = false;
                self.status = None;
                self.dirty = true;
            }
            KeyCode::Char('[') | KeyCode::Left => self.cycle_mip(-1),
            KeyCode::Char(']') | KeyCode::Right => self.cycle_mip(1),
            KeyCode::Char('a') | KeyCode::Tab => self.cycle_surface(1),
            KeyCode::Char(' ') => self.toggle_play(),
            KeyCode::Char('m') => self.toggle_joined(),
            KeyCode::Char('+') | KeyCode::Char('=') => self.adjust_fps(1),
            KeyCode::Char('-') | KeyCode::Char('_') => self.adjust_fps(-1),
            KeyCode::Char(',') => self.step_frame(-1),
            KeyCode::Char('.') => self.step_frame(1),
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_by(1);
                self.refocus();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_by(-1);
                self.refocus();
            }
            KeyCode::Char('y') => self.copy_path(),
            KeyCode::Enter => {
                if let Some(item) = self.current_item() {
                    self.result = Some(self.items[item].frame(0).header.clone());
                    return Flow::Quit;
                }
            }
            _ => {}
        }
        Flow::Continue
    }

    /// The decoded preview for the frame showing now, if ready.
    fn ready_preview(&self) -> Option<Arc<Preview>> {
        let item = self.current_item()?;
        let frame = self.current_frame(self.items[item].frames.len());
        let cache = self.cache.lock().unwrap();
        match cache.get(&(item, frame)) {
            Some(Slot::Ready(preview)) => Some(preview.clone()),
            _ => None,
        }
    }

    /// Step the selected mip level, clamped to the current surface's chain.
    fn cycle_mip(&mut self, delta: isize) {
        let Some(preview) = self.ready_preview() else {
            return;
        };
        let surface = self.surface.min(preview.surfaces.len().saturating_sub(1));
        let count = preview.surfaces.get(surface).map_or(0, |s| s.mips.len());
        if count <= 1 {
            return;
        }
        self.mip = (self.mip as isize + delta).rem_euclid(count as isize) as usize;
        self.status = None;
        self.dirty = true;
    }

    /// Switch between the base and attached-alpha surfaces, resetting to mip 0.
    fn cycle_surface(&mut self, delta: isize) {
        let Some(preview) = self.ready_preview() else {
            return;
        };
        let count = preview.surfaces.len();
        if count <= 1 {
            return;
        }
        self.surface = (self.surface as isize + delta).rem_euclid(count as isize) as usize;
        self.mip = 0;
        self.status = None;
        self.dirty = true;
    }

    /// Toggle a directory; on a file, do nothing (handled as "open" elsewhere).
    fn copy_path(&mut self) {
        if let Some(item) = self.current_item() {
            let path = self.items[item].frame(0).header.clone();
            self.status = Some(match set_clipboard(&path) {
                Ok(()) => format!("copied {path}"),
                Err(error) => format!("copy failed: {error}"),
            });
        }
    }
}

impl View for DdsBrowser {
    fn take_result(&mut self) -> Option<String> {
        self.result.take()
    }

    fn ticking(&self) -> bool {
        // Keep redrawing while discovery streams textures in, or while anything
        // on screen is still decoding (the focused preview, or grid thumbnails).
        if self.catalog.is_scanning() {
            return true;
        }
        // A playing sprite always redraws at its frame rate (grid or focus).
        if self.animating() {
            return true;
        }
        if self.focused {
            // Otherwise tick until the frame showing now is decoded AND its
            // protocol has been encoded off-thread.
            return self.focus_pending();
        }
        // Keep ticking while any thumbnail is still decoding (drains + redraws).
        self.thumbs.values().any(|(_, thumb)| matches!(thumb, Thumb::Pending))
    }

    fn tick(&mut self) {
        self.ingest();
        self.drain_thumbs();
        self.drain_encodes();
    }

    fn poll_interval(&self) -> Duration {
        // Redraw at the sprite's frame rate while it plays; fall back to the
        // slower background-progress cadence otherwise.
        if self.animating() {
            Duration::from_millis((1000 / self.fps.max(1)).max(16) as u64)
        } else {
            Duration::from_millis(120)
        }
    }

    fn needs_clear(&mut self) -> Option<Rect> {
        // Only clear on a discrete change (enter/exit, cycle mip/surface, resize,
        // or a frame whose size differs from the last). Steady same-size animation
        // relies on the graphics protocol's cell diff to swap frames in place — an
        // unconditional per-frame clear is what caused the flicker.
        std::mem::take(&mut self.dirty).then_some(self.body)
    }

    fn on_key(&mut self, key: KeyEvent) -> Flow {
        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.filtering = false;
                    self.rebuild();
                    self.dirty = true;
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.rebuild();
                    self.dirty = true;
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.rebuild();
                    self.dirty = true;
                }
                _ => {}
            }
            return Flow::Continue;
        }

        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Flow::Quit;
        }
        if self.focused {
            return self.on_focus_key(key);
        }
        self.on_grid_key(key)
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);
        let (top, divider, body, bottom) = (rows[0], rows[1], rows[2], rows[3]);

        self.render_top(frame, top);
        frame.buffer_mut().set_line(
            divider.x,
            divider.y,
            &Line::from(Span::styled(
                std::iter::repeat_n('─', divider.width as usize).collect::<String>(),
                theme::dim(),
            )),
            divider.width,
        );

        self.body = body;
        // Don't wipe the body every frame — that forces every thumbnail/image to
        // re-emit each animation tick (the multi-sprite chug). Residue from a
        // content shift (scroll, filter, focus enter/exit) is cleared once via the
        // `dirty` flag (see `needs_clear`); steady animation repaints only the cells
        // whose image actually changed.
        if self.focused {
            self.render_focus(frame, body);
        } else {
            self.render_grid(frame, body);
        }
        self.render_bottom(frame, bottom);
    }
}

impl DdsBrowser {
    fn render_top(&self, frame: &mut Frame, area: Rect) {
        let glyphs = theme::glyphs(self.caps);
        let mut spans = vec![
            Span::styled("dds", theme::accent()),
            Span::raw("  "),
            Span::styled(self.source.clone(), theme::dim()),
            Span::raw("  "),
            Span::styled("textures ", theme::dim()),
            Span::styled(self.items.len().to_string(), theme::bold()),
        ];
        let (scanned, total) = self.catalog.progress();
        if scanned < total {
            spans.push(Span::styled(format!("  scanning {scanned}/{total}"), theme::accent()));
        }
        if !self.filter.is_empty() {
            spans.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
            spans.push(Span::styled("matched ", theme::dim()));
            spans.push(Span::styled(self.visible.len().to_string(), theme::bold()));
        }
        let buf = frame.buffer_mut();
        buf.set_line(area.x, area.y, &Line::from(spans), area.width);

        let right = if self.visible.is_empty() {
            "0/0".to_string()
        } else {
            format!("{}/{}", self.selected + 1, self.visible.len())
        };
        let width = theme::display_width(&right) as u16;
        if width < area.width {
            buf.set_line(
                area.right() - width,
                area.y,
                &Line::from(Span::styled(right, theme::dim())),
                width,
            );
        }
    }

    fn render_grid(&mut self, frame: &mut Frame, area: Rect) {
        if self.visible.is_empty() {
            let message = if self.catalog.is_scanning() {
                "scanning…"
            } else {
                "no textures"
            };
            self.center_text(frame, area, message, theme::dim());
            return;
        }

        // Choose columns to land cells near the target width, then size the
        // thumbnail to fill the cell (minus a gutter) with a label row beneath.
        // Cells are ~twice as tall as wide in pixels, so a square-ish thumbnail
        // uses about half as many rows as columns.
        let cols = (area.width / TARGET_CELL_W).clamp(MIN_COLS, MAX_COLS).min(area.width.max(1));
        let cell_w = area.width / cols;
        let thumb_w = cell_w.saturating_sub(2).max(1);
        let thumb_h = (thumb_w / 2).clamp(5, 24);
        let cell_h = thumb_h + 2;
        if cell_w == 0 || cell_h > area.height {
            return;
        }
        let cols = cols as usize;
        self.cols = cols;

        // Re-decode at the new size if the cell geometry changed (terminal resize):
        // bump the generation so in-flight jobs for the old size are dropped, and
        // clear the cache so cells re-request at the new size.
        let cell = Size::new(thumb_w, thumb_h);
        if cell != self.cell {
            self.cell = cell;
            self.thumb_generation.fetch_add(1, Ordering::Relaxed);
            self.thumbs.clear();
            self.dirty = true;
        }

        let rows_visible = (area.height / cell_h).max(1) as usize;
        // Vertical scroll: keep the selected row in view.
        let sel_row = self.selected / cols;
        if sel_row < self.offset {
            self.offset = sel_row;
        } else if sel_row >= self.offset + rows_visible {
            self.offset = sel_row + 1 - rows_visible;
        }

        let start = self.offset * cols;
        let end = (start + cols * rows_visible).min(self.visible.len());
        let items: Vec<usize> = self.visible[start..end].to_vec();

        self.grid_has_sprite = items.iter().any(|&item| self.items[item].frames.len() > 1);
        // Queue decodes for every visible cell (item-major, deduped) and refresh
        // their LRU stamps. No per-scroll cancel: queued jobs finish and cache, so
        // scrolling back to a row is instant.
        self.request_thumbs(&items);

        let glyphs = theme::glyphs(self.caps);
        // Snapshot each cell's current-frame protocol (cheap `Arc` clone) from the
        // UI-owned cache — no lock. Falls back to frame 0 while the live sprite frame
        // still decodes, so a cell never blinks to a placeholder mid-cycle. Each cell
        // is independent: it shows its image the moment ITS frames are ready,
        // regardless of other (still-decoding) cells.
        let cells: Vec<CellVisual> = items
            .iter()
            .map(|&item| {
                let frames = self.items[item].frames.len();
                let live = if frames > 1 { self.current_frame(frames) } else { 0 };
                for candidate in [live, 0] {
                    match self.thumbs.get(&(item, candidate)).map(|(_, thumb)| thumb) {
                        Some(Thumb::Ready(protocol)) => return CellVisual::Image(protocol.clone()),
                        Some(Thumb::Failed) if candidate == 0 => return CellVisual::Failed,
                        _ => {}
                    }
                }
                CellVisual::Pending
            })
            .collect();

        // Render the snapshot.
        for (slot, (&item, visual)) in items.iter().zip(&cells).enumerate() {
            let grid_index = start + slot;
            let col = (slot % cols) as u16;
            let row = (slot / cols) as u16;
            let cell_x = area.x + col * cell_w;
            let cell_y = area.y + row * cell_h;
            let img_area = Rect { x: cell_x + 1, y: cell_y, width: thumb_w, height: thumb_h };

            match visual {
                CellVisual::Image(protocol) => {
                    let size = protocol.size();
                    let w = size.width.min(thumb_w);
                    let h = size.height.min(thumb_h);
                    let rect = Rect {
                        x: img_area.x + (thumb_w - w) / 2,
                        y: img_area.y + (thumb_h - h) / 2,
                        width: w,
                        height: h,
                    };
                    frame.render_widget(Image::new(protocol.as_ref()), rect);
                }
                CellVisual::Failed => self.center_text(frame, img_area, "✕", theme::bad()),
                CellVisual::Pending => self.center_text(frame, img_area, "·", theme::dim()),
            }

            // Filename label under the thumbnail; highlight the selected cell.
            let entry = &self.items[item];
            let name = entry.label.rsplit('/').next().unwrap_or(&entry.label);
            // Mark sprites (animated sequences) with a play glyph + frame count.
            let name = if entry.is_sprite() {
                format!("▶{} {name}", entry.frames.len())
            } else {
                name.to_string()
            };
            let label = theme::fit_end(&name, thumb_w as usize, glyphs.ellipsis);
            let label_y = cell_y + thumb_h;
            let selected = grid_index == self.selected;
            let style = if selected {
                theme::accent().add_modifier(Modifier::BOLD)
            } else {
                theme::dim()
            };
            frame
                .buffer_mut()
                .set_line(img_area.x, label_y, &Line::from(Span::styled(label, style)), thumb_w);
            if selected && self.caps.color {
                let buf = frame.buffer_mut();
                for x in img_area.x..img_area.x + thumb_w {
                    buf[(x, label_y)].set_bg(HILITE);
                }
            }
        }
    }

    fn render_focus(&mut self, frame: &mut Frame, area: Rect) {
        if area.width < 6 || area.height < 3 {
            return;
        }
        let Some(item) = self.current_item() else {
            self.center_text(frame, area, "select a texture", theme::dim());
            return;
        };

        let frames = self.items[item].frames.len();
        let frame_index = self.current_frame(frames);
        let label = self.items[item].label.clone();
        let glyphs = theme::glyphs(self.caps);
        let title = if frames > 1 {
            if self.joined {
                format!("⊞ {frames} frames joined   m cycle   {label}")
            } else {
                let state = if self.playing { glyphs.play } else { glyphs.pause };
                format!("{state} {}/{frames}  {} fps   m join   {label}", frame_index + 1, self.fps)
            }
        } else {
            label
        };
        frame.buffer_mut().set_line(
            area.x,
            area.y,
            &Line::from(Span::styled(
                theme::fit_middle(&title, area.width as usize, glyphs.ellipsis),
                theme::accent(),
            )),
            area.width,
        );

        if self.joined && frames > 1 {
            self.render_montage(frame, area, item, frames);
            return;
        }

        let slot = {
            let cache = self.cache.lock().unwrap();
            match cache.get(&(item, frame_index)) {
                Some(Slot::Ready(preview)) => Some(Ok(preview.clone())),
                Some(Slot::Failed(error)) => Some(Err(error.clone())),
                _ => None,
            }
        };

        let spinner_area = Rect {
            x: area.x,
            y: area.y + 2,
            width: area.width,
            height: area.height.saturating_sub(2),
        };

        match slot {
            Some(Ok(preview)) if !preview.surfaces.is_empty() => {
                let surface_index = self.surface.min(preview.surfaces.len() - 1);
                let surface = &preview.surfaces[surface_index];
                let count = surface.mips.len();
                let mip = self.mip.min(count.saturating_sub(1));
                let level = &surface.mips[mip];

                let mut row = area.y + 1;
                // Format / kind chips.
                let mut chips = Vec::new();
                for (index, (key, value)) in preview.meta.iter().enumerate() {
                    if index > 0 {
                        chips.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
                    }
                    chips.push(Span::styled(format!("{key} "), theme::dim()));
                    chips.push(Span::raw(value.clone()));
                }
                frame
                    .buffer_mut()
                    .set_line(area.x, row, &Line::from(chips), area.width);
                row += 1;

                // Surface selector — only when there's an attached alpha to switch to.
                if preview.surfaces.len() > 1 {
                    frame.buffer_mut().set_line(
                        area.x,
                        row,
                        &self.surface_line(&preview.surfaces, surface_index),
                        area.width,
                    );
                    row += 1;
                }

                // Mip selector + current level dimensions.
                frame.buffer_mut().set_line(
                    area.x,
                    row,
                    &self.mip_line(mip, count, level.width, level.height),
                    area.width,
                );
                row += 1;

                let used = row - area.y;
                let image_area = Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: area.height.saturating_sub(used),
                };
                if image_area.height == 0 {
                    return;
                }
                // The encode (the costly step) runs off-thread: blit the cached
                // protocol if ready, else queue it and show a spinner. A size change
                // invalidates the cache (protocols were sized for the old area).
                if self.focus_size != Some((image_area.width, image_area.height)) {
                    self.focus_size = Some((image_area.width, image_area.height));
                    self.reset_focus_encodes();
                }
                let key: FocusKey =
                    (item, frame_index, surface_index, mip, image_area.width, image_area.height);
                if let Some(protocol) = self.focus_protocols.get(&key) {
                    let size = protocol.size();
                    let w = size.width.min(image_area.width);
                    let h = size.height.min(image_area.height);
                    let rect = Rect {
                        x: image_area.x + (image_area.width - w) / 2,
                        y: image_area.y + (image_area.height - h) / 2,
                        width: w,
                        height: h,
                    };
                    // A frame whose size differs from the last needs one clear to
                    // avoid residue; same-size frames swap in place (no flicker).
                    if self.last_image.is_some_and(|previous| previous != rect) {
                        self.dirty = true;
                    }
                    self.last_image = Some(rect);
                    frame.render_widget(Image::new(protocol.as_ref()), rect);
                } else {
                    self.request_focus_encode(
                        key,
                        EncodeJob::Frame {
                            preview: preview.clone(),
                            surface: surface_index,
                            mip,
                        },
                    );
                    self.center_text(frame, spinner_area, "decoding…", theme::dim());
                }
            }
            Some(Ok(_)) | None => {
                // The decode was already requested on focus; just wait for it.
                self.center_text(frame, spinner_area, "decoding…", theme::dim());
            }
            Some(Err(error)) => self.center_text(frame, spinner_area, &error, theme::bad()),
        }
    }

    /// Render every frame of a sprite tiled into one sheet (the "joined" view).
    /// Assembly + encode happen off-thread; until the sheet is ready a spinner shows.
    fn render_montage(&mut self, frame: &mut Frame, area: Rect, item: usize, frames: usize) {
        let image_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };
        if image_area.height == 0 {
            return;
        }
        if self.focus_size != Some((image_area.width, image_area.height)) {
            self.focus_size = Some((image_area.width, image_area.height));
            self.reset_focus_encodes();
        }
        // Sentinel frame `usize::MAX` distinguishes the montage from per-frame keys.
        let key: FocusKey = (item, usize::MAX, 0, 0, image_area.width, image_area.height);
        if let Some(protocol) = self.focus_protocols.get(&key) {
            let size = protocol.size();
            let w = size.width.min(image_area.width);
            let h = size.height.min(image_area.height);
            let rect = Rect {
                x: image_area.x + (image_area.width - w) / 2,
                y: image_area.y + (image_area.height - h) / 2,
                width: w,
                height: h,
            };
            self.last_image = Some(rect);
            frame.render_widget(Image::new(protocol.as_ref()), rect);
            return;
        }

        // Snapshot every frame's preview (cheap Arc clones); bail until all ready.
        let previews: Vec<Arc<Preview>> = {
            let cache = self.cache.lock().unwrap();
            let mut previews = Vec::with_capacity(frames);
            for index in 0..frames {
                match cache.get(&(item, index)) {
                    Some(Slot::Ready(preview)) => previews.push(preview.clone()),
                    Some(Slot::Failed(error)) => {
                        let error = error.clone();
                        drop(cache);
                        self.center_text(frame, image_area, &error, theme::bad());
                        return;
                    }
                    _ => {
                        drop(cache);
                        self.center_text(frame, image_area, "decoding…", theme::dim());
                        return;
                    }
                }
            }
            previews
        };
        self.request_focus_encode(key, EncodeJob::Montage { frames: previews });
        self.center_text(frame, image_area, "building…", theme::dim());
    }

    /// The surface selector: `surface  base  alpha` with the active one highlighted.
    fn surface_line(&self, surfaces: &[Surface], active: usize) -> Line<'static> {
        let mut spans = vec![Span::styled("surface  ", theme::dim())];
        for (index, surface) in surfaces.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            let style = if index == active {
                theme::accent()
            } else {
                theme::dim()
            };
            spans.push(Span::styled(surface.name.to_string(), style));
        }
        spans.push(Span::styled("   a toggle", theme::dim()));
        Line::from(spans)
    }

    /// The mip-level selector: position, a pip per level, and the level's size.
    fn mip_line(&self, mip: usize, count: usize, width: u32, height: u32) -> Line<'static> {
        let cross = if self.caps.unicode { "×" } else { "x" };
        if count <= 1 {
            return Line::from(vec![
                Span::styled("mip ", theme::dim()),
                Span::styled("1/1", theme::dim()),
                Span::raw("   "),
                Span::styled(format!("{width}{cross}{height}"), theme::bold()),
            ]);
        }
        let (filled, empty) = if self.caps.unicode { ("●", "·") } else { ("#", "-") };
        let mut spans = vec![
            Span::styled("mip ", theme::dim()),
            Span::styled(format!("{}/{count}", mip + 1), theme::bold()),
            Span::raw("  "),
        ];
        for level in 0..count {
            let (glyph, style) = if level == mip {
                (filled, theme::accent())
            } else {
                (empty, theme::dim())
            };
            spans.push(Span::styled(glyph.to_string(), style));
            spans.push(Span::raw(" "));
        }
        spans.push(Span::raw(" "));
        spans.push(Span::styled(format!("{width}{cross}{height}"), theme::bold()));
        spans.push(Span::styled("   [ ] cycle", theme::dim()));
        Line::from(spans)
    }

    /// Draw a short message centered in `area`.
    fn center_text(&self, frame: &mut Frame, area: Rect, text: &str, style: Style) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let width = theme::display_width(text).min(area.width as usize);
        let x = area.x + (area.width - width as u16) / 2;
        let y = area.y + area.height / 2;
        frame.buffer_mut().set_line(
            x,
            y,
            &Line::from(Span::styled(text.to_string(), style)),
            width as u16,
        );
    }

    fn render_bottom(&self, frame: &mut Frame, area: Rect) {
        let line = if self.filtering {
            Line::from(vec![
                Span::styled("/", theme::accent()),
                Span::raw(self.filter.clone()),
                Span::styled("▏", theme::dim()),
            ])
        } else if let Some(status) = &self.status {
            Line::from(Span::styled(status.clone(), theme::dim()))
        } else {
            // Sprite playback controls appear only when the focused texture is an
            // animated sequence.
            let sprite = self.focused && self.focused_frames() > 1;
            let hint = match (self.focused, sprite, self.caps.unicode) {
                (false, _, true) => "↑↓←→ move   ⏎ open   / filter   y copy   q quit",
                (false, _, false) => "arrows move   enter open   / filter   y copy   q quit",
                (true, true, true) => {
                    "[ ] mip   a alpha   space play   ± fps   , . frame   m join   esc back"
                }
                (true, true, false) => {
                    "[ ] mip   a alpha   space play   +/- fps   , . frame   m join   esc back"
                }
                (true, false, true) => "[ ] mip   a alpha   ↑↓ texture   ⏎ select   esc back",
                (true, false, false) => {
                    "[ ] mip   a alpha   up/dn texture   enter select   esc back"
                }
            };
            Line::from(Span::styled(hint.to_string(), theme::dim()))
        };
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &line, area.width);
    }
}

fn spawn_decoder(
    catalog: Arc<DdsCatalog>,
    store: Arc<TextureStore>,
    runner: JobRunner,
    cache: Arc<Cache>,
) -> SyncSender<PreviewReq> {
    let (tx, rx) = sync_channel::<PreviewReq>(8);
    std::thread::spawn(move || decode_loop(&rx, &catalog, &store, &runner, &cache));
    tx
}

/// Spawn the focus-view protocol encoder. It owns a cloned [`Picker`] and turns
/// decoded previews into terminal graphics protocols off the UI thread, returning
/// each over a lock-free channel the UI drains without blocking.
fn spawn_encoder(picker: Picker) -> (CbSender<EncodeReq>, CbReceiver<EncodeResult>) {
    let (req_tx, req_rx) = unbounded::<EncodeReq>();
    let (res_tx, res_rx) = unbounded::<EncodeResult>();
    std::thread::spawn(move || {
        while let Ok(req) = req_rx.recv() {
            let (.., width, height) = req.key;
            let protocol = match req.job {
                EncodeJob::Frame {
                    preview,
                    surface,
                    mip,
                } => preview
                    .surfaces
                    .get(surface)
                    .and_then(|surface| surface.mips.get(mip))
                    .and_then(|level| encode_image(&picker, level.image.clone(), width, height)),
                EncodeJob::Montage { frames } => {
                    montage_sheet(&frames).and_then(|sheet| encode_image(&picker, sheet, width, height))
                }
            };
            if res_tx
                .send(EncodeResult {
                    key: req.key,
                    generation: req.generation,
                    protocol,
                })
                .is_err()
            {
                break; // UI gone
            }
        }
    });
    (req_tx, res_rx)
}

/// Encode an RGBA image to a terminal graphics protocol fit to `width`×`height` cells.
fn encode_image(picker: &Picker, image: RgbaImage, width: u16, height: u16) -> Option<Arc<Protocol>> {
    picker
        .new_protocol(
            DynamicImage::ImageRgba8(image),
            Size::new(width, height),
            Resize::Fit(None),
        )
        .ok()
        .map(Arc::new)
}

/// Tile every frame's base-surface top mip into one montage sheet (√-grid, each
/// frame centered in its cell). Returns `None` if any frame lacks a decoded mip.
fn montage_sheet(frames: &[Arc<Preview>]) -> Option<RgbaImage> {
    let images: Vec<&RgbaImage> = frames
        .iter()
        .map(|preview| preview.surfaces.first().and_then(|s| s.mips.first()).map(|m| &m.image))
        .collect::<Option<Vec<_>>>()?;
    if images.is_empty() {
        return None;
    }
    let count = images.len();
    let cols = (count as f64).sqrt().ceil() as usize;
    let rows = count.div_ceil(cols);
    let cell_w = images.iter().map(|image| image.width()).max().unwrap_or(1);
    let cell_h = images.iter().map(|image| image.height()).max().unwrap_or(1);
    let mut sheet = RgbaImage::new(cell_w * cols as u32, cell_h * rows as u32);
    for (index, image) in images.iter().enumerate() {
        let x = (index % cols) as u32 * cell_w + (cell_w - image.width()) / 2;
        let y = (index / cols) as u32 * cell_h + (cell_h - image.height()) / 2;
        image::imageops::overlay(&mut sheet, *image, i64::from(x), i64::from(y));
    }
    Some(sheet)
}

fn decode_loop(
    rx: &Receiver<PreviewReq>,
    catalog: &DdsCatalog,
    store: &TextureStore,
    runner: &JobRunner,
    cache: &Cache,
) {
    while let Ok(mut req) = rx.recv() {
        // Coalesce to the most recent request so fast cycling doesn't backlog.
        while let Ok(next) = rx.try_recv() {
            req = next;
        }
        if req.cancel.is_cancelled() {
            continue;
        }
        // Claim the not-yet-decoded frames and snapshot them out of the (growing)
        // catalog, so the decode runs without holding any lock. Each key is
        // (texture index, frame) — sprites decode every frame for smooth playback.
        let todo: Vec<((usize, usize), String, DdsFrame)> = {
            let mut guard = cache.lock().unwrap();
            let mut todo = Vec::new();
            for &(index, frame) in &req.items {
                let key = (index, frame);
                if !guard.contains_key(&key)
                    && let Some(item) = catalog.item(index)
                    && let Some(data) = item.frames.get(frame)
                {
                    guard.insert(key, Slot::Pending);
                    todo.push((key, item.label.clone(), data.clone()));
                }
            }
            todo
        };
        if todo.is_empty() {
            continue;
        }
        // Publish each frame the instant it decodes (no result Vec), so the focus
        // view shows the first frame immediately instead of waiting for the whole
        // sprite. The cancel token is also threaded into the decode so a cancelled
        // job abandons the remaining mip levels between levels.
        runner.for_each_until_cancelled(&todo, &req.cancel, |(key, label, data)| {
            let slot = match decode_item(store, label, data, &req.cancel) {
                Ok(preview) => Slot::Ready(Arc::new(preview)),
                // Abandoned mid-decode: leave Pending for the cleanup below to drop.
                Err(_) if req.cancel.is_cancelled() => return,
                Err(error) => Slot::Failed(error),
            };
            let mut guard = cache.lock().unwrap();
            // Only publish if still the in-flight entry (not superseded/cancelled).
            if matches!(guard.get(key), Some(Slot::Pending)) {
                guard.insert(*key, slot);
            }
        });
        // Drop any frames left Pending by a cancelled batch so they re-decode later.
        let mut guard = cache.lock().unwrap();
        for (key, _, _) in &todo {
            if matches!(guard.get(key), Some(Slot::Pending)) {
                guard.remove(key);
            }
        }
    }
}

/// Spawn the grid thumbnail worker pool: `workers` threads that each pull one
/// [`ThumbJob`] at a time from a shared lock-free queue, decode + encode it, and
/// hand the result back over a channel the UI drains. Jobs are processed in
/// roughly FIFO order, so item-major enqueue makes each sprite finish (and start
/// playing smoothly) independently. A job whose cell-size `generation` is stale is
/// skipped without decoding.
fn spawn_thumb_pool(
    workers: usize,
    catalog: Arc<DdsCatalog>,
    store: Arc<TextureStore>,
    cache: Arc<ThumbCache>,
    picker: Picker,
    generation: Arc<AtomicU64>,
) -> (CbSender<ThumbJob>, CbReceiver<ThumbResult>) {
    let (job_tx, job_rx) = unbounded::<ThumbJob>();
    let (res_tx, res_rx) = unbounded::<ThumbResult>();
    for _ in 0..workers.max(1) {
        let job_rx = job_rx.clone();
        let res_tx = res_tx.clone();
        let catalog = catalog.clone();
        let store = store.clone();
        let cache = cache.clone();
        let picker = picker.clone();
        let generation = generation.clone();
        std::thread::spawn(move || {
            while let Ok(job) = job_rx.recv() {
                // Skip work the grid has already resized past.
                if job.generation != generation.load(Ordering::Relaxed) {
                    continue;
                }
                let (item, frame) = job.key;
                let thumb = match catalog.item(item).and_then(|dds| dds.frames.get(frame).cloned()) {
                    Some(data) => match decode_thumbnail(&store, &cache, &data, &picker, job.size) {
                        Ok(protocol) => Thumb::Ready(Arc::new(protocol)),
                        Err(_) => Thumb::Failed,
                    },
                    None => Thumb::Failed,
                };
                if res_tx
                    .send(ThumbResult {
                        key: job.key,
                        generation: job.generation,
                        thumb,
                    })
                    .is_err()
                {
                    break; // UI gone
                }
            }
        });
    }
    (job_tx, res_rx)
}

/// Decode a texture's top mip to a small thumbnail and encode it to a terminal
/// graphics protocol sized to a grid cell — all off the UI thread.
fn decode_thumbnail(
    store: &TextureStore,
    cache: &ThumbCache,
    frame: &DdsFrame,
    picker: &Picker,
    size: Size,
) -> Result<Protocol, String> {
    // The header read is cheap and identifies the texture's content; key the disk
    // cache on its hash so an unchanged DDS reuses the cached thumbnail (even after
    // a game patch). A hit skips the sidecar reads + DDS decode.
    let header = store.read(&frame.header)?;
    let key = fnv1a(&header);
    let image = match cache.load(key) {
        Some(image) => image,
        None => {
            let decoded = nw_dds::decode_mip_max(&header, THUMB_CACHE_PX, |part| {
                frame.sidecars.iter().find(|(p, _)| *p == part).and_then(|(_, key)| store.read(key).ok())
            })
            .map_err(|error| error.to_string())?;
            let decoded = RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba)
                .ok_or_else(|| "decoded texture had an unexpected size".to_string())?;
            let image = downscale(decoded, THUMB_CACHE_PX);
            cache.store(key, &image);
            image
        }
    };
    picker
        .new_protocol(DynamicImage::ImageRgba8(image), size, Resize::Fit(None))
        .map_err(|error| error.to_string())
}

/// Decode a texture's surfaces (base, and the attached alpha if present) and
/// gather its metadata. Runs on a worker. The base must decode; a failing alpha
/// surface is simply omitted rather than failing the whole texture.
fn decode_item(
    store: &TextureStore,
    label: &str,
    frame: &DdsFrame,
    cancel: &CancellationToken,
) -> Result<Preview, String> {
    let header_bytes = store.read(&frame.header)?;
    let meta = read_meta(label, &header_bytes);

    let mut surfaces = vec![Surface {
        name: "base",
        // Reuse the header bytes already read for `meta` instead of re-reading.
        mips: decode_chain(store, &header_bytes, &frame.sidecars, cancel)?,
    }];
    if let Some(alpha) = &frame.alpha
        && let Ok(alpha_header) = store.read(&alpha.header)
        && let Ok(mips) = decode_chain(store, &alpha_header, &alpha.sidecars, cancel)
        && !mips.is_empty()
    {
        surfaces.push(Surface {
            name: "alpha",
            mips,
        });
    }
    Ok(Preview { meta, surfaces })
}

/// Decode one surface's full mip chain to display-sized RGBA, assembling sidecars
/// from `header_bytes` (already read by the caller). Falls back to the top mip
/// alone if the full chain can't be decoded.
fn decode_chain(
    store: &TextureStore,
    header_bytes: &[u8],
    sidecars: &[(nw_dds::SplitPart, String)],
    cancel: &CancellationToken,
) -> Result<Vec<Mip>, String> {
    let mut sidecar_bytes = Vec::with_capacity(sidecars.len());
    for (part, key) in sidecars {
        sidecar_bytes.push((*part, store.read(key)?));
    }
    let parts = sidecar_bytes
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes.as_slice()))
        .collect::<Vec<_>>();

    // Abandon the chain between mip levels if the request was cancelled.
    let decoded = match nw_dds::decode_all_mips_until(header_bytes, &parts, &|| !cancel.is_cancelled())
    {
        Ok(mips) if !mips.is_empty() => mips,
        _ if cancel.is_cancelled() => return Err("cancelled".to_string()),
        _ => vec![nw_dds::decode_top_mip(header_bytes, &parts).map_err(|e| e.to_string())?],
    };

    let mut mips = Vec::with_capacity(decoded.len());
    for level in decoded {
        let image = RgbaImage::from_raw(level.width, level.height, level.rgba)
            .ok_or_else(|| "decoded texture had an unexpected size".to_string())?;
        mips.push(Mip {
            width: level.width,
            height: level.height,
            image: downscale(image, PREVIEW_MAX),
        });
    }
    Ok(mips)
}

/// Pull display-worthy metadata chips from a DDS header.
fn read_meta(label: &str, header_bytes: &[u8]) -> Vec<(String, String)> {
    let mut meta = Vec::new();
    if let Ok(asset) = nw_dds::Asset::parse(label, header_bytes)
        && let nw_dds::AssetKind::Header(dds) = asset.kind()
    {
        meta.push(("format".to_string(), dds.format_name()));
        if dds.is_cry_extended() {
            meta.push(("kind".to_string(), "Cry".to_string()));
        }
    }
    meta
}

/// Downscale so the largest side is at most `max`, preserving aspect.
fn downscale(image: RgbaImage, max: u32) -> RgbaImage {
    let (width, height) = (image.width(), image.height());
    if width <= max && height <= max {
        return image;
    }
    let scale = f64::from(max) / f64::from(width.max(height));
    let target_w = ((f64::from(width) * scale) as u32).max(1);
    let target_h = ((f64::from(height) * scale) as u32).max(1);
    image::imageops::resize(
        &image,
        target_w,
        target_h,
        image::imageops::FilterType::Triangle,
    )
}

/// Put `text` on the system clipboard.
fn set_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text.to_string()))
        .map_err(|error| error.to_string())
}
