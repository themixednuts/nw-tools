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
use ratatui_image::picker::Picker;
use ratatui_image::protocol::{Protocol, StatefulProtocol};
use ratatui_image::{Image, Resize, StatefulImage};

use nw_jobs::{CancellationToken, JobRunner};

use super::app::{Flow, View};
use crate::fuzzy;
use crate::ui::theme::{self, Caps};

const HILITE: ratatui::style::Color = ratatui::style::Color::Indexed(238);

/// Largest preview decoded off-thread; the widget downscales further to the pane.
const PREVIEW_MAX: u32 = 1024;

/// Grid thumbnail geometry, in terminal cells: the image area per cell, plus a
/// label row and gaps. A texture cell is `CELL_W × CELL_H`.
const THUMB_W: u16 = 24;
const THUMB_H: u16 = 11;
const CELL_W: u16 = THUMB_W + 2;
const CELL_H: u16 = THUMB_H + 2;
/// Thumbnails decode to at most this many pixels on the long edge before encoding.
const THUMB_PX: u32 = 320;

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

type Thumbs = Mutex<HashMap<usize, Thumb>>;

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
    picker: Picker,
    /// The graphics protocol for the displayed (texture, surface, mip) — UI thread.
    protocol: Option<(usize, usize, usize, StatefulProtocol)>,
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
        let keep: HashSet<usize> = window.iter().copied().collect();
        self.thumbs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|item, _| keep.contains(item));
        let _ = self.thumb_tx.try_send(ThumbReq {
            items: window.clone(),
            size: Size::new(THUMB_W, THUMB_H),
            cancel,
        });
        self.requested = window;
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
        self.requested
            .iter()
            .any(|item| !matches!(thumbs.get(item), Some(Thumb::Ready(_)) | Some(Thumb::Failed)))
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
        if area.width < CELL_W || area.height < CELL_H {
            return;
        }
        let cols = (area.width / CELL_W).max(1) as usize;
        self.cols = cols;
        let rows_visible = (area.height / CELL_H).max(1) as usize;

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

        let glyphs = theme::glyphs(self.caps);
        let thumbs = self.thumbs.lock().unwrap_or_else(|error| error.into_inner());
        for (slot, &item) in window.iter().enumerate() {
            let grid_index = start + slot;
            let col = (slot % cols) as u16;
            let row = (slot / cols) as u16;
            let cell_x = area.x + col * CELL_W;
            let cell_y = area.y + row * CELL_H;
            let img_area = Rect { x: cell_x + 1, y: cell_y, width: THUMB_W, height: THUMB_H };

            match thumbs.get(&item) {
                Some(Thumb::Ready(protocol)) => {
                    let size = protocol.size();
                    let w = size.width.min(THUMB_W);
                    let h = size.height.min(THUMB_H);
                    let rect = Rect {
                        x: img_area.x + (THUMB_W - w) / 2,
                        y: img_area.y + (THUMB_H - h) / 2,
                        width: w,
                        height: h,
                    };
                    frame.render_widget(Image::new(protocol), rect);
                }
                Some(Thumb::Failed) => self.center_text(frame, img_area, "✕", theme::bad()),
                _ => self.center_text(frame, img_area, "·", theme::dim()),
            }

            // Filename label under the thumbnail; highlight the selected cell.
            let name = self.items[item].label.rsplit('/').next().unwrap_or(&self.items[item].label);
            let label = theme::fit_end(name, THUMB_W as usize, glyphs.ellipsis);
            let label_y = cell_y + THUMB_H;
            let selected = grid_index == self.selected;
            let style = if selected {
                theme::accent().add_modifier(Modifier::BOLD)
            } else {
                theme::dim()
            };
            frame
                .buffer_mut()
                .set_line(img_area.x, label_y, &Line::from(Span::styled(label, style)), THUMB_W);
            if selected && self.caps.color {
                let buf = frame.buffer_mut();
                for x in img_area.x..img_area.x + THUMB_W {
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
                let key = (item, surface_index, mip);
                if self.protocol.as_ref().map(|(it, su, mp, _)| (*it, *su, *mp)) != Some(key) {
                    let dynamic = DynamicImage::ImageRgba8(level.image.clone());
                    self.protocol = Some((item, surface_index, mip, self.picker.new_resize_protocol(dynamic)));
                }
                let font = self.picker.font_size();
                let rect = centered_image_rect(image_area, level.width, level.height, font);
                if let Some((_, _, _, protocol)) = self.protocol.as_mut() {
                    frame.render_stateful_widget(StatefulImage::default(), rect, protocol);
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

/// Center an image inside `area`, preserving aspect. Converts the image's pixel
/// dimensions to terminal cells using the detected font size, fits to the area,
/// then centers the resulting rectangle.
fn centered_image_rect(
    area: Rect,
    width: u32,
    height: u32,
    font: ratatui_image::FontSize,
) -> Rect {
    let font_w = f32::from(font.width.max(1));
    let font_h = f32::from(font.height.max(1));
    let cells_w = width as f32 / font_w;
    let cells_h = height as f32 / font_h;
    if cells_w <= 0.0 || cells_h <= 0.0 {
        return area;
    }
    let scale = (f32::from(area.width) / cells_w).min(f32::from(area.height) / cells_h);
    let w = ((cells_w * scale).round() as u16).clamp(1, area.width);
    let h = ((cells_h * scale).round() as u16).clamp(1, area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
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
                    guard.insert(item, Thumb::Pending);
                    todo.push((item, dds));
                }
            }
            todo
        };
        if todo.is_empty() {
            continue;
        }
        let size = req.size;
        let batch = runner.map_until_cancelled(&todo, &req.cancel, |(item, dds)| {
            (*item, decode_thumbnail(store, dds, picker, size))
        });
        let mut guard = thumbs.lock().unwrap_or_else(|error| error.into_inner());
        let mut done = HashSet::new();
        for (item, result) in batch.into_completed() {
            done.insert(item);
            let slot = match result {
                Ok(protocol) => Thumb::Ready(protocol),
                Err(_) => Thumb::Failed,
            };
            guard.insert(item, slot);
        }
        // Items left Pending by a cancelled batch: drop them so they re-decode
        // when next requested.
        for (item, _) in &todo {
            if !done.contains(item) {
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
    let header = store.read(&item.header)?;
    let mut sidecar_bytes = Vec::with_capacity(item.sidecars.len());
    for (part, key) in &item.sidecars {
        sidecar_bytes.push((*part, store.read(key)?));
    }
    let parts = sidecar_bytes
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes.as_slice()))
        .collect::<Vec<_>>();
    let decoded = nw_dds::decode_top_mip(&header, &parts).map_err(|error| error.to_string())?;
    let image = RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba)
        .ok_or_else(|| "decoded texture had an unexpected size".to_string())?;
    let dynamic = DynamicImage::ImageRgba8(downscale(image, THUMB_PX));
    picker
        .new_protocol(dynamic, size, Resize::Fit(None))
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
