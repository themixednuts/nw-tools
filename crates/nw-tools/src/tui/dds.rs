//! Interactive DDS texture browser. A fuzzy-filterable file list on the left, a
//! live image preview on the right (via the kitty/sixel/iterm graphics protocols
//! with a unicode half-block fallback). Textures decode lazily on a background
//! thread that fans the work out across the shared [`JobRunner`], so the list is
//! responsive from the first frame and scrolling pre-warms a window of neighbours.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use image::{DynamicImage, RgbaImage};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui_image::StatefulImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use nw_jobs::JobRunner;

use super::app::{Flow, View};
use crate::fuzzy;
use crate::ui::theme::{self, Caps};

const HILITE: ratatui::style::Color = ratatui::style::Color::Indexed(238);

/// Largest preview decoded off-thread; the widget downscales further to the pane.
const PREVIEW_MAX: u32 = 1024;

/// Resolves a texture key (a pak virtual path) to the archive entry that holds
/// it. Built during discovery from the readers it already opened, so reads are an
/// O(1) map lookup + decompress — no per-read scan over every pak.
pub type PakIndex = HashMap<String, (Arc<nw_pak::PakMmapReader>, usize)>;

/// Where texture bytes are read from: the filesystem, or straight out of the
/// install's pak archives (by entry index — no catalog needed, the install ships
/// none).
pub enum TextureStore {
    Fs,
    Pak(PakIndex),
}

impl TextureStore {
    fn read(&self, key: &str) -> Result<Vec<u8>, String> {
        match self {
            TextureStore::Fs => std::fs::read(key).map_err(|error| error.to_string()),
            TextureStore::Pak(index) => {
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

/// One node in the texture tree: a directory, or a leaf texture.
struct Node {
    /// Path segment (directory name, or file name for a leaf).
    label: String,
    depth: usize,
    parent: Option<usize>,
    children: Vec<usize>,
    expanded: bool,
    /// For leaves: index into `items`. `None` for directories.
    item: Option<usize>,
}

pub struct DdsBrowser {
    caps: Caps,
    items: Arc<Vec<DdsItem>>,
    /// Where the textures came from — shown in the header (e.g. the install path).
    source: String,
    // Tree arena.
    nodes: Vec<Node>,
    roots: Vec<usize>,
    /// Leaf node ids and their lowercased full paths, for fuzzy filtering.
    file_nodes: Vec<usize>,
    haystacks: Vec<String>,
    file_count: usize,
    // View state (indices into `visible`).
    visible: Vec<usize>,
    selected: usize,
    offset: usize,
    filter: String,
    filtering: bool,
    /// Selected surface (0=base, 1=alpha) for the focused texture.
    surface: usize,
    /// Selected mip level for the focused texture (clamped to its chain at render).
    mip: usize,
    // Decode plumbing.
    cache: Arc<Cache>,
    /// Sends a priority-ordered window of item indices to the decode worker.
    tx: SyncSender<Vec<usize>>,
    picker: Picker,
    /// The graphics protocol for the displayed (texture, surface, mip) — UI thread.
    protocol: Option<(usize, usize, usize, StatefulProtocol)>,
    status: Option<String>,
    result: Option<String>,
}

impl DdsBrowser {
    pub fn new(
        items: Vec<DdsItem>,
        store: Arc<TextureStore>,
        source: String,
        runner: JobRunner,
        picker: Picker,
        caps: Caps,
    ) -> Self {
        let items = Arc::new(items);
        let (nodes, roots) = build_tree(&items);
        let mut file_nodes = Vec::new();
        let mut haystacks = Vec::new();
        for (id, node) in nodes.iter().enumerate() {
            if let Some(item) = node.item {
                file_nodes.push(id);
                haystacks.push(items[item].label.to_ascii_lowercase());
            }
        }
        let file_count = file_nodes.len();
        let cache: Arc<Cache> = Arc::new(Mutex::new(HashMap::new()));
        let tx = spawn_decoder(items.clone(), store, runner, cache.clone());
        let mut browser = Self {
            caps,
            items,
            source,
            nodes,
            roots,
            file_nodes,
            haystacks,
            file_count,
            visible: Vec::new(),
            selected: 0,
            offset: 0,
            filter: String::new(),
            filtering: false,
            surface: 0,
            mip: 0,
            cache,
            tx,
            picker,
            protocol: None,
            status: None,
            result: None,
        };
        browser.rebuild();
        browser.request();
        browser
    }

    /// Recompute the visible rows from the tree and the active filter, keeping the
    /// selection anchored to the same node where possible.
    fn rebuild(&mut self) {
        let anchor = self.visible.get(self.selected).copied();
        self.visible.clear();
        if self.filter.is_empty() {
            let roots = self.roots.clone();
            for root in roots {
                collect_open(&self.nodes, root, &mut self.visible);
            }
        } else {
            self.collect_filtered();
        }
        self.selected = anchor
            .and_then(|node| self.visible.iter().position(|&candidate| candidate == node))
            .unwrap_or(self.selected)
            .min(self.visible.len().saturating_sub(1));
    }

    /// Prune the tree to branches containing a fuzzy match, auto-expanded.
    fn collect_filtered(&mut self) {
        let ranked = fuzzy::rank(&self.filter, &self.haystacks);
        if ranked.is_empty() {
            return;
        }
        let matched: HashSet<usize> = ranked
            .into_iter()
            .map(|(index, _)| self.file_nodes[index])
            .collect();
        // Mark every ancestor directory of a match.
        let mut contains: HashSet<usize> = HashSet::new();
        for &leaf in &matched {
            let mut parent = self.nodes[leaf].parent;
            while let Some(node) = parent {
                if !contains.insert(node) {
                    break;
                }
                parent = self.nodes[node].parent;
            }
        }
        let roots = self.roots.clone();
        for root in roots {
            collect_match(&self.nodes, root, &matched, &contains, &mut self.visible);
        }
    }

    fn current(&self) -> Option<usize> {
        self.visible.get(self.selected).copied()
    }

    fn current_item(&self) -> Option<usize> {
        self.current().and_then(|node| self.nodes[node].item)
    }

    /// Queue the focused texture (first, so it lands soonest) plus the file leaves
    /// just above/below it in view order, so scrolling pre-warms neighbours.
    fn request(&self) {
        if self.visible.is_empty() {
            return;
        }
        let mut want = Vec::new();
        if let Some(item) = self.current_item() {
            want.push(item);
        }
        let start = self.selected.saturating_sub(3);
        let end = (self.selected + 12).min(self.visible.len());
        for row in start..end {
            if row == self.selected {
                continue;
            }
            if let Some(&node) = self.visible.get(row)
                && let Some(item) = self.nodes[node].item
            {
                want.push(item);
            }
        }
        if !want.is_empty() {
            let _ = self.tx.try_send(want);
        }
    }

    fn move_by(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let last = (self.visible.len() - 1) as isize;
        self.selected = (self.selected as isize).saturating_add(delta).clamp(0, last) as usize;
        self.status = None;
        self.surface = 0;
        self.mip = 0;
        self.request();
    }

    fn select(&mut self, row: usize) {
        self.selected = row.min(self.visible.len().saturating_sub(1));
        self.status = None;
        self.surface = 0;
        self.mip = 0;
        self.request();
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
    fn toggle(&mut self) {
        if let Some(node) = self.current()
            && self.nodes[node].item.is_none()
        {
            self.nodes[node].expanded = !self.nodes[node].expanded;
            self.rebuild();
        }
    }

    /// Right/`l`: expand a directory (or step into it if already open).
    fn expand(&mut self) {
        if let Some(node) = self.current() {
            if self.nodes[node].item.is_some() {
                return;
            }
            if self.nodes[node].expanded {
                self.move_by(1);
            } else {
                self.nodes[node].expanded = true;
                self.rebuild();
            }
        }
    }

    /// Left/`h`: collapse an open directory, else jump to the parent directory.
    fn collapse(&mut self) {
        if let Some(node) = self.current() {
            if self.nodes[node].item.is_none() && self.nodes[node].expanded {
                self.nodes[node].expanded = false;
                self.rebuild();
            } else if let Some(parent) = self.nodes[node].parent
                && let Some(row) = self.visible.iter().position(|&candidate| candidate == parent)
            {
                self.select(row);
            }
        }
    }

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

/// Build the directory tree arena from the items' (sorted) full paths.
fn build_tree(items: &[DdsItem]) -> (Vec<Node>, Vec<usize>) {
    let mut nodes: Vec<Node> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    // (parent, segment) -> node id; usize::MAX is the virtual root.
    let mut lookup: HashMap<(usize, String), usize> = HashMap::new();

    for (item_index, item) in items.iter().enumerate() {
        let mut parent: Option<usize> = None;
        let segments: Vec<&str> = item.label.split('/').filter(|s| !s.is_empty()).collect();
        for (depth, segment) in segments.iter().enumerate() {
            let is_leaf = depth + 1 == segments.len();
            let key = (parent.unwrap_or(usize::MAX), (*segment).to_string());
            let node = if let Some(&existing) = lookup.get(&key) {
                existing
            } else {
                let id = nodes.len();
                nodes.push(Node {
                    label: (*segment).to_string(),
                    depth,
                    parent,
                    children: Vec::new(),
                    expanded: false,
                    item: is_leaf.then_some(item_index),
                });
                lookup.insert(key, id);
                match parent {
                    Some(p) => nodes[p].children.push(id),
                    None => roots.push(id),
                }
                id
            };
            parent = Some(node);
        }
    }

    // Directories before files, alphabetical within each.
    let order = |nodes: &[Node], a: usize, b: usize| {
        nodes[a]
            .item
            .is_some()
            .cmp(&nodes[b].item.is_some())
            .then_with(|| nodes[a].label.cmp(&nodes[b].label))
    };
    for id in 0..nodes.len() {
        let mut children = std::mem::take(&mut nodes[id].children);
        children.sort_by(|&a, &b| order(&nodes, a, b));
        nodes[id].children = children;
    }
    roots.sort_by(|&a, &b| order(&nodes, a, b));
    (nodes, roots)
}

/// DFS append `node` and its open descendants in display order.
fn collect_open(nodes: &[Node], node: usize, out: &mut Vec<usize>) {
    out.push(node);
    if nodes[node].expanded {
        for &child in &nodes[node].children {
            collect_open(nodes, child, out);
        }
    }
}

/// DFS append only matched leaves and the directories that contain them.
fn collect_match(
    nodes: &[Node],
    node: usize,
    matched: &HashSet<usize>,
    contains: &HashSet<usize>,
    out: &mut Vec<usize>,
) {
    if nodes[node].item.is_some() {
        if matched.contains(&node) {
            out.push(node);
        }
        return;
    }
    if contains.contains(&node) {
        out.push(node);
        for &child in &nodes[node].children {
            collect_match(nodes, child, matched, contains, out);
        }
    }
}

impl View for DdsBrowser {
    fn take_result(&mut self) -> Option<String> {
        self.result.take()
    }

    fn ticking(&self) -> bool {
        // Keep redrawing while the focused texture is still decoding.
        match self.current_item() {
            Some(item) => {
                let cache = self.cache.lock().unwrap();
                !matches!(
                    cache.get(&item),
                    Some(Slot::Ready(_)) | Some(Slot::Failed(_))
                )
            }
            None => false,
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Flow {
        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.filtering = false;
                    self.rebuild();
                    self.request();
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.rebuild();
                    self.request();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.rebuild();
                    self.request();
                }
                _ => {}
            }
            return Flow::Continue;
        }

        let page = 10;
        match key.code {
            KeyCode::Char('q') => return Flow::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Flow::Quit;
            }
            KeyCode::Esc => {
                if self.filter.is_empty() {
                    return Flow::Quit;
                }
                self.filter.clear();
                self.rebuild();
                self.request();
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::PageDown => self.move_by(page),
            KeyCode::PageUp => self.move_by(-page),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => self.move_by(page),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_by(-page)
            }
            KeyCode::Char('g') | KeyCode::Home => self.select(0),
            KeyCode::Char('G') | KeyCode::End => {
                self.select(self.visible.len().saturating_sub(1))
            }
            KeyCode::Char('h') | KeyCode::Left => self.collapse(),
            KeyCode::Char('l') | KeyCode::Right => self.expand(),
            KeyCode::Char('[') | KeyCode::Char(',') => self.cycle_mip(-1),
            KeyCode::Char(']') | KeyCode::Char('.') => self.cycle_mip(1),
            KeyCode::Char('a') | KeyCode::Tab => self.cycle_surface(1),
            KeyCode::Char(' ') => self.toggle(),
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Char('y') => self.copy_path(),
            KeyCode::Enter => {
                if let Some(item) = self.current_item() {
                    self.result = Some(self.items[item].header.clone());
                    return Flow::Quit;
                }
                self.toggle();
            }
            _ => {}
        }
        Flow::Continue
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

        // A narrow tree on the left, a large viewport on the right — the point.
        let tree_width = ((u32::from(body.width) * 32 / 100) as u16).clamp(22, 52);
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(tree_width), Constraint::Min(1)])
            .split(body);
        self.render_tree(frame, columns[0]);
        self.render_viewport(frame, columns[1]);
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
            Span::styled(self.file_count.to_string(), theme::bold()),
        ];
        if !self.filter.is_empty() {
            spans.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
            spans.push(Span::styled("matched ", theme::dim()));
            spans.push(Span::styled(
                self.visible
                    .iter()
                    .filter(|&&node| self.nodes[node].item.is_some())
                    .count()
                    .to_string(),
                theme::bold(),
            ));
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

    fn render_tree(&mut self, frame: &mut Frame, area: Rect) {
        let height = area.height as usize;
        if height == 0 || self.visible.is_empty() {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected + 1 - height;
        }

        let glyphs = theme::glyphs(self.caps);
        let width = area.width as usize;
        for slot in 0..height {
            let row = self.offset + slot;
            let Some(&node_id) = self.visible.get(row) else {
                break;
            };
            let node = &self.nodes[node_id];
            let y = area.y + slot as u16;
            let indent = "  ".repeat(node.depth);
            let marker = if node.item.is_some() {
                "  "
            } else if node.expanded {
                if self.caps.unicode { "▾ " } else { "v " }
            } else if self.caps.unicode {
                "▸ "
            } else {
                "> "
            };
            let name_width = width.saturating_sub(indent.len() + marker.len());
            let name = theme::fit_end(&node.label, name_width, glyphs.ellipsis);
            let name_style = if node.item.is_some() {
                Style::default()
            } else {
                theme::accent()
            };
            let line = Line::from(vec![
                Span::raw(indent),
                Span::styled(marker.to_string(), theme::dim()),
                Span::styled(name, name_style),
            ]);
            frame.buffer_mut().set_line(area.x, y, &line, area.width);
            if row == self.selected {
                let style = if self.caps.color {
                    Style::default().bg(HILITE).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().add_modifier(Modifier::REVERSED)
                };
                let buf = frame.buffer_mut();
                for x in area.x..area.right() {
                    buf[(x, y)].set_style(style);
                }
            }
        }
    }

    fn render_viewport(&mut self, frame: &mut Frame, area: Rect) {
        if area.width < 6 || area.height < 3 {
            return;
        }
        // Vertical separator + gutter so the viewport doesn't touch the tree.
        for y in area.y..area.bottom() {
            frame.buffer_mut().set_line(
                area.x,
                y,
                &Line::from(Span::styled("│", theme::dim())),
                1,
            );
        }
        let area = Rect {
            x: area.x + 2,
            width: area.width - 2,
            ..area
        };

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
                self.request();
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
            let hint = if self.caps.unicode {
                "↑↓ move   ←→ fold   [ ] mip   a alpha   / search   y copy   q quit"
            } else {
                "up/dn move   l/r fold   [ ] mip   a alpha   / search   y copy   q quit"
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
    items: Arc<Vec<DdsItem>>,
    store: Arc<TextureStore>,
    runner: JobRunner,
    cache: Arc<Cache>,
) -> SyncSender<Vec<usize>> {
    let (tx, rx) = sync_channel::<Vec<usize>>(8);
    std::thread::spawn(move || decode_loop(&rx, &items, &store, &runner, &cache));
    tx
}

fn decode_loop(
    rx: &Receiver<Vec<usize>>,
    items: &[DdsItem],
    store: &TextureStore,
    runner: &JobRunner,
    cache: &Cache,
) {
    while let Ok(mut want) = rx.recv() {
        // Coalesce to the most recent request so fast scrolling doesn't backlog.
        while let Ok(next) = rx.try_recv() {
            want = next;
        }
        let todo: Vec<usize> = {
            let mut guard = cache.lock().unwrap();
            let mut todo = Vec::new();
            for index in want {
                if index < items.len() && !guard.contains_key(&index) {
                    guard.insert(index, Slot::Pending);
                    todo.push(index);
                }
            }
            todo
        };
        if todo.is_empty() {
            continue;
        }
        // Fan the decode out across the shared job pool.
        let decoded = runner.map(&todo, |&index| (index, decode_item(store, &items[index])));
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
