//! Interactive DDS texture browser — an engine-style asset grid. A scrollable
//! grid of image thumbnails (via the kitty/sixel/iterm graphics protocols with a
//! unicode half-block fallback), filterable by path, with Enter to focus one
//! texture for a full mip/surface view. Thumbnails decode AND encode on a
//! background thread (so the UI never blocks on image work) as textures stream in
//! from discovery, and stale decode batches are cancelled when the view moves.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use image::{DynamicImage, RgbaImage};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect, Size};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
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
/// scrolling back doesn't re-decode what was just viewed.
const THUMB_CACHE_CAP: usize = 1024;

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

/// One logical texture: a DDS header plus its ordered split-mip sidecars. Keys
/// are filesystem paths or pak virtual paths, resolved through a [`TextureStore`].
#[derive(Clone)]
pub struct DdsItem {
    /// Display label (relative path, forward slashes).
    pub label: String,
    pub header: String,
    pub sidecars: Vec<(nw_dds::SplitPart, String)>,
    /// The attached-alpha surface (`.dds.a` header + `.dds.Na` mips), if present —
    /// a second image of the same texture (gloss/opacity), viewable on its own.
    pub alpha: Option<AlphaSurface>,
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

type Cache = Mutex<HashMap<usize, Slot>>;

/// A grid thumbnail's decode state: in-flight, an encoded protocol ready to blit,
/// or failed.
enum Thumb {
    Pending,
    Ready(Protocol),
    Failed,
}

/// The thumbnail cache: item index → (last-used clock, state). The clock drives
/// LRU eviction so decoded thumbnails survive scrolling away (and switching to the
/// focus view) and are only dropped, oldest-off-screen first, past a memory cap.
type Thumbs = Mutex<HashMap<usize, (u64, Thumb)>>;

/// Identifies the focus-view protocol: (item, surface, mip, area width, area height).
type FocusKey = (usize, usize, usize, u16, u16);

/// A request to decode thumbnails for a window of item indices at `size` (cells).
/// `cancel` is tripped by the UI when the window moves, so a stale batch stops.
struct ThumbReq {
    items: Vec<usize>,
    size: Size,
    cancel: CancellationToken,
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
    /// Selected surface (0=base, 1=alpha) for the focused texture.
    surface: usize,
    /// Selected mip level for the focused texture (clamped to its chain at render).
    mip: usize,
    // Full-preview decode (focus view).
    cache: Arc<Cache>,
    tx: SyncSender<Vec<usize>>,
    // Thumbnail decode (grid).
    thumbs: Arc<Thumbs>,
    thumb_tx: SyncSender<ThumbReq>,
    /// Cancels the in-flight thumbnail batch when the visible window changes.
    thumb_cancel: CancellationToken,
    /// The item-index window last requested, so we only re-decode (and cancel) on change.
    requested: Vec<usize>,
    /// Monotonic clock stamped onto thumbnails as they're shown, for LRU eviction.
    clock: u64,
    picker: Picker,
    /// The encoded protocol for the focused (texture, surface, mip) at a given
    /// image-area size — rebuilt only when that key changes.
    protocol: Option<(FocusKey, Protocol)>,
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
        let cache: Arc<Cache> = Arc::new(Mutex::new(HashMap::new()));
        let tx = spawn_decoder(catalog.clone(), store.clone(), runner.clone(), cache.clone());
        let thumbs: Arc<Thumbs> = Arc::new(Mutex::new(HashMap::new()));
        let thumb_tx =
            spawn_thumb_decoder(catalog.clone(), store, runner, picker.clone(), thumbs.clone());
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
            surface: 0,
            mip: 0,
            cache,
            tx,
            thumbs,
            thumb_tx,
            thumb_cancel: CancellationToken::new(),
            requested: Vec::new(),
            clock: 0,
            picker,
            protocol: None,
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
        self.selected = anchor
            .and_then(|item| self.visible.iter().position(|&candidate| candidate == item))
            .unwrap_or(self.selected)
            .min(self.visible.len().saturating_sub(1));
    }

    /// The focused texture (an index into `items`), if any.
    fn current_item(&self) -> Option<usize> {
        self.visible.get(self.selected).copied()
    }

    /// Move the grid selection by `delta` (±1 to step, ±cols to change row).
    fn move_by(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let last = (self.visible.len() - 1) as isize;
        self.selected = (self.selected as isize).saturating_add(delta).clamp(0, last) as usize;
        self.status = None;
    }

    fn select(&mut self, index: usize) {
        self.selected = index.min(self.visible.len().saturating_sub(1));
        self.status = None;
    }

    /// Open the focus view on the current texture and request its full decode.
    fn focus(&mut self) {
        if self.current_item().is_some() {
            self.focused = true;
            self.surface = 0;
            self.mip = 0;
            self.status = None;
            self.request_preview();
        }
    }

    /// Request a full-resolution preview decode of the focused texture.
    fn request_preview(&self) {
        if let Some(item) = self.current_item() {
            let _ = self.tx.try_send(vec![item]);
        }
    }

    /// Request thumbnail decodes for the visible grid `window`, cancelling the
    /// previous batch. No-op when the window is unchanged, so a still grid keeps
    /// decoding instead of being cancelled every frame; thumbnails outside the
    /// window are evicted to bound memory.
    fn request_thumbs(&mut self, window: Vec<usize>) {
        if window == self.requested {
            return;
        }
        self.thumb_cancel.cancel();
        let cancel = CancellationToken::new();
        self.thumb_cancel = cancel.clone();
        self.evict_thumbs(&window);
        let _ = self.thumb_tx.try_send(ThumbReq { items: window.clone(), size: self.cell, cancel });
        self.requested = window;
    }

    /// Bound the thumbnail cache: only when it exceeds the cap, drop the
    /// least-recently-shown thumbnails that aren't currently visible. Off-screen
    /// and other-screen thumbnails are kept until that pressure point, so moving
    /// around never forces a re-decode of what was just viewed.
    fn evict_thumbs(&self, window: &[usize]) {
        let mut thumbs = self.thumbs.lock().unwrap_or_else(|error| error.into_inner());
        if thumbs.len() <= THUMB_CACHE_CAP {
            return;
        }
        let keep: HashSet<usize> = window.iter().copied().collect();
        let mut evictable: Vec<(u64, usize)> = thumbs
            .iter()
            .filter(|(item, _)| !keep.contains(item))
            .map(|(item, (used, _))| (*used, *item))
            .collect();
        evictable.sort_unstable();
        let excess = thumbs.len() - THUMB_CACHE_CAP;
        for (_, item) in evictable.into_iter().take(excess) {
            thumbs.remove(&item);
        }
    }

    /// Reset the focus view to the newly-selected texture and request its decode.
    fn refocus(&mut self) {
        self.surface = 0;
        self.mip = 0;
        self.status = None;
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
            }
            KeyCode::Char('[') | KeyCode::Char(',') | KeyCode::Left => self.cycle_mip(-1),
            KeyCode::Char(']') | KeyCode::Char('.') | KeyCode::Right => self.cycle_mip(1),
            KeyCode::Char('a') | KeyCode::Tab => self.cycle_surface(1),
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
                    self.result = Some(self.items[item].header.clone());
                    return Flow::Quit;
                }
            }
            _ => {}
        }
        Flow::Continue
    }

    /// The decoded preview for the focused texture, if ready.
    fn ready_preview(&self) -> Option<Arc<Preview>> {
        let item = self.current_item()?;
        let cache = self.cache.lock().unwrap();
        match cache.get(&item) {
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
    }

    /// Toggle a directory; on a file, do nothing (handled as "open" elsewhere).
    fn copy_path(&mut self) {
        if let Some(item) = self.current_item() {
            let path = self.items[item].header.clone();
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
        if self.focused {
            return match self.current_item() {
                Some(item) => {
                    let cache = self.cache.lock().unwrap();
                    !matches!(cache.get(&item), Some(Slot::Ready(_)) | Some(Slot::Failed(_)))
                }
                None => false,
            };
        }
        let thumbs = self.thumbs.lock().unwrap_or_else(|error| error.into_inner());
        self.requested.iter().any(|item| {
            !matches!(
                thumbs.get(item).map(|(_, state)| state),
                Some(Thumb::Ready(_)) | Some(Thumb::Failed)
            )
        })
    }

    fn tick(&mut self) {
        self.ingest();
    }

    fn on_key(&mut self, key: KeyEvent) -> Flow {
        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.filtering = false;
                    self.rebuild();
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.rebuild();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.rebuild();
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

        // Wipe the body so graphics-protocol image cells from the previous frame
        // (a scrolled-off thumbnail, or the grid when switching to focus) don't
        // leave residue behind the new content.
        frame.render_widget(Clear, body);
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

        // Re-decode at the new size if the cell geometry changed (terminal resize).
        let cell = Size::new(thumb_w, thumb_h);
        if cell != self.cell {
            self.cell = cell;
            self.thumbs.lock().unwrap_or_else(|error| error.into_inner()).clear();
            self.requested.clear();
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
        let window: Vec<usize> = self.visible[start..end].to_vec();
        self.request_thumbs(window.clone());

        self.clock = self.clock.wrapping_add(1);
        let now = self.clock;
        let glyphs = theme::glyphs(self.caps);
        let mut thumbs = self.thumbs.lock().unwrap_or_else(|error| error.into_inner());
        for (slot, &item) in window.iter().enumerate() {
            let grid_index = start + slot;
            let col = (slot % cols) as u16;
            let row = (slot / cols) as u16;
            let cell_x = area.x + col * cell_w;
            let cell_y = area.y + row * cell_h;
            let img_area = Rect { x: cell_x + 1, y: cell_y, width: thumb_w, height: thumb_h };

            match thumbs.get_mut(&item) {
                Some(entry) => {
                    entry.0 = now; // mark recently shown for LRU
                    match &entry.1 {
                        Thumb::Ready(protocol) => {
                            let size = protocol.size();
                            let w = size.width.min(thumb_w);
                            let h = size.height.min(thumb_h);
                            let rect = Rect {
                                x: img_area.x + (thumb_w - w) / 2,
                                y: img_area.y + (thumb_h - h) / 2,
                                width: w,
                                height: h,
                            };
                            frame.render_widget(Image::new(protocol), rect);
                        }
                        Thumb::Failed => self.center_text(frame, img_area, "✕", theme::bad()),
                        Thumb::Pending => self.center_text(frame, img_area, "·", theme::dim()),
                    }
                }
                None => self.center_text(frame, img_area, "·", theme::dim()),
            }

            // Filename label under the thumbnail; highlight the selected cell.
            let name = self.items[item].label.rsplit('/').next().unwrap_or(&self.items[item].label);
            let label = theme::fit_end(name, thumb_w as usize, glyphs.ellipsis);
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

        let label = self.items[item].label.clone();
        let glyphs = theme::glyphs(self.caps);
        frame.buffer_mut().set_line(
            area.x,
            area.y,
            &Line::from(Span::styled(
                theme::fit_middle(&label, area.width as usize, glyphs.ellipsis),
                theme::accent(),
            )),
            area.width,
        );

        let slot = {
            let cache = self.cache.lock().unwrap();
            match cache.get(&item) {
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
                // Encode the mip to fill the image area, then blit it centered, so
                // it's centered regardless of mip level (no top-left jump).
                let key: FocusKey = (item, surface_index, mip, image_area.width, image_area.height);
                if self.protocol.as_ref().map(|(existing, _)| *existing) != Some(key) {
                    let dynamic = DynamicImage::ImageRgba8(level.image.clone());
                    let size = Size::new(image_area.width, image_area.height);
                    self.protocol = self
                        .picker
                        .new_protocol(dynamic, size, Resize::Fit(None))
                        .ok()
                        .map(|protocol| (key, protocol));
                }
                if let Some((_, protocol)) = &self.protocol {
                    let size = protocol.size();
                    let w = size.width.min(image_area.width);
                    let h = size.height.min(image_area.height);
                    let rect = Rect {
                        x: image_area.x + (image_area.width - w) / 2,
                        y: image_area.y + (image_area.height - h) / 2,
                        width: w,
                        height: h,
                    };
                    frame.render_widget(Image::new(protocol), rect);
                }
            }
            Some(Ok(_)) | None => {
                self.request_preview();
                self.center_text(frame, spinner_area, "decoding…", theme::dim());
            }
            Some(Err(error)) => self.center_text(frame, spinner_area, &error, theme::bad()),
        }
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
            let hint = match (self.focused, self.caps.unicode) {
                (false, true) => "↑↓←→ move   ⏎ open   / filter   y copy   q quit",
                (false, false) => "arrows move   enter open   / filter   y copy   q quit",
                (true, true) => "[ ] mip   a alpha   ↑↓ texture   ⏎ select   esc back   q quit",
                (true, false) => "[ ] mip   a alpha   up/dn texture   enter select   esc back   q quit",
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
) -> SyncSender<Vec<usize>> {
    let (tx, rx) = sync_channel::<Vec<usize>>(8);
    std::thread::spawn(move || decode_loop(&rx, &catalog, &store, &runner, &cache));
    tx
}

fn decode_loop(
    rx: &Receiver<Vec<usize>>,
    catalog: &DdsCatalog,
    store: &TextureStore,
    runner: &JobRunner,
    cache: &Cache,
) {
    while let Ok(mut want) = rx.recv() {
        // Coalesce to the most recent request so fast scrolling doesn't backlog.
        while let Ok(next) = rx.try_recv() {
            want = next;
        }
        // Claim the not-yet-decoded items and snapshot them out of the (growing)
        // catalog, so the decode runs without holding any lock.
        let todo: Vec<(usize, DdsItem)> = {
            let mut guard = cache.lock().unwrap();
            let mut todo = Vec::new();
            for index in want {
                if !guard.contains_key(&index)
                    && let Some(item) = catalog.item(index)
                {
                    guard.insert(index, Slot::Pending);
                    todo.push((index, item));
                }
            }
            todo
        };
        if todo.is_empty() {
            continue;
        }
        // Fan the decode out across the shared job pool.
        let decoded = runner.map(&todo, |(index, item)| (*index, decode_item(store, item)));
        let mut guard = cache.lock().unwrap();
        for (index, result) in decoded {
            let slot = match result {
                Ok(preview) => Slot::Ready(Arc::new(preview)),
                Err(error) => Slot::Failed(error),
            };
            guard.insert(index, slot);
        }
    }
}

fn spawn_thumb_decoder(
    catalog: Arc<DdsCatalog>,
    store: Arc<TextureStore>,
    runner: JobRunner,
    picker: Picker,
    thumbs: Arc<Thumbs>,
) -> SyncSender<ThumbReq> {
    let (tx, rx) = sync_channel::<ThumbReq>(8);
    std::thread::spawn(move || thumb_loop(&rx, &catalog, &store, &runner, &picker, &thumbs));
    tx
}

/// Decode + encode grid thumbnails on a worker thread. Each request is a window
/// of items at a cell size; the batch is cancellable (the UI trips the token when
/// the window moves), and items are decoded straight out of the (still-growing)
/// catalog as soon as they are discovered — never waiting for discovery to finish.
fn thumb_loop(
    rx: &Receiver<ThumbReq>,
    catalog: &DdsCatalog,
    store: &TextureStore,
    runner: &JobRunner,
    picker: &Picker,
    thumbs: &Thumbs,
) {
    while let Ok(mut req) = rx.recv() {
        // Coalesce to the most recent request so fast scrolling doesn't backlog.
        while let Ok(next) = rx.try_recv() {
            req = next;
        }
        if req.cancel.is_cancelled() {
            continue;
        }
        // Claim the not-yet-decoded items, snapshotting their DdsItem out of the
        // (growing) catalog so the decode holds no lock.
        let todo: Vec<(usize, DdsItem)> = {
            let mut guard = thumbs.lock().unwrap_or_else(|error| error.into_inner());
            let mut todo = Vec::new();
            for &item in &req.items {
                if !guard.contains_key(&item)
                    && let Some(dds) = catalog.item(item)
                {
                    guard.insert(item, (0, Thumb::Pending));
                    todo.push((item, dds));
                }
            }
            todo
        };
        if todo.is_empty() {
            continue;
        }
        // `todo` is in display order; rayon starts it in order and each thumbnail
        // is published the instant it decodes, so the grid fills progressively
        // (roughly top-to-bottom) instead of all at once after the slowest one.
        let size = req.size;
        runner.map_until_cancelled(&todo, &req.cancel, |(item, dds)| {
            let state = match decode_thumbnail(store, dds, picker, size) {
                Ok(protocol) => Thumb::Ready(protocol),
                Err(_) => Thumb::Failed,
            };
            let mut guard = thumbs.lock().unwrap_or_else(|error| error.into_inner());
            // Only publish if still the in-flight entry (not evicted/superseded).
            if matches!(guard.get(item), Some((_, Thumb::Pending))) {
                guard.insert(*item, (0, state));
            }
        });
        // Items left Pending by a cancelled batch: drop them so they re-decode
        // when next requested.
        let mut guard = thumbs.lock().unwrap_or_else(|error| error.into_inner());
        for (item, _) in &todo {
            if matches!(guard.get(item), Some((_, Thumb::Pending))) {
                guard.remove(item);
            }
        }
    }
}

/// Decode a texture's top mip to a small thumbnail and encode it to a terminal
/// graphics protocol sized to a grid cell — all off the UI thread.
fn decode_thumbnail(
    store: &TextureStore,
    item: &DdsItem,
    picker: &Picker,
    size: Size,
) -> Result<Protocol, String> {
    // Decode the mip that matches the cell's pixel size so the thumbnail fills the
    // cell — reading only the one sidecar that mip needs (not the huge top mip).
    let header = store.read(&item.header)?;
    let font = picker.font_size();
    let target_px = u32::from(font.width) * u32::from(size.width);
    let target_py = u32::from(font.height) * u32::from(size.height);
    let max_dim = target_px.max(target_py).max(64);
    let decoded = nw_dds::decode_mip_max(&header, max_dim, |part| {
        item.sidecars.iter().find(|(p, _)| *p == part).and_then(|(_, key)| store.read(key).ok())
    })
    .map_err(|error| error.to_string())?;
    let image = RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba)
        .ok_or_else(|| "decoded texture had an unexpected size".to_string())?;
    picker
        .new_protocol(DynamicImage::ImageRgba8(image), size, Resize::Fit(None))
        .map_err(|error| error.to_string())
}

/// Decode a texture's surfaces (base, and the attached alpha if present) and
/// gather its metadata. Runs on a worker. The base must decode; a failing alpha
/// surface is simply omitted rather than failing the whole texture.
fn decode_item(store: &TextureStore, item: &DdsItem) -> Result<Preview, String> {
    let header_bytes = store.read(&item.header)?;
    let meta = read_meta(&item.label, &header_bytes);

    let mut surfaces = vec![Surface {
        name: "base",
        mips: decode_chain(store, &item.header, &item.sidecars)?,
    }];
    if let Some(alpha) = &item.alpha
        && let Ok(mips) = decode_chain(store, &alpha.header, &alpha.sidecars)
        && !mips.is_empty()
    {
        surfaces.push(Surface {
            name: "alpha",
            mips,
        });
    }
    Ok(Preview { meta, surfaces })
}

/// Decode one surface's full mip chain to display-sized RGBA, assembling sidecars.
/// Falls back to the top mip alone if the full chain can't be decoded.
fn decode_chain(
    store: &TextureStore,
    header: &str,
    sidecars: &[(nw_dds::SplitPart, String)],
) -> Result<Vec<Mip>, String> {
    let header_bytes = store.read(header)?;
    let mut sidecar_bytes = Vec::with_capacity(sidecars.len());
    for (part, key) in sidecars {
        sidecar_bytes.push((*part, store.read(key)?));
    }
    let parts = sidecar_bytes
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes.as_slice()))
        .collect::<Vec<_>>();

    let decoded = match nw_dds::decode_all_mips(&header_bytes, &parts) {
        Ok(mips) if !mips.is_empty() => mips,
        _ => vec![nw_dds::decode_top_mip(&header_bytes, &parts).map_err(|e| e.to_string())?],
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
