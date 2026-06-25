//! A spreadsheet-style datasheet viewer with cross-sheet navigation.
//!
//! The viewer renders one sheet at a time but talks to a [`SheetSource`] — a
//! workspace of many datasheets whose cross-reference index is built in the
//! background. Goto-definition and find-references therefore start with
//! whatever is already indexed and keep growing while you browse; jumping to a
//! reference in another sheet loads that sheet on demand.

use std::collections::HashSet;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::app::{Flow, View};
use crate::fuzzy;
use crate::ui::theme::{self, Caps};

const ROW_HILITE: Color = Color::Indexed(238);
const NUMBER: Color = Color::Cyan;
const COL_GAP: usize = 2;
const MAX_COL: usize = 30;
const MIN_COL: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GridType {
    String,
    Number,
    Boolean,
}

impl GridType {
    fn label(self) -> &'static str {
        match self {
            GridType::String => "string",
            GridType::Number => "number",
            GridType::Boolean => "boolean",
        }
    }
}

#[derive(Clone)]
pub struct GridColumn {
    pub name: String,
    pub kind: GridType,
}

#[derive(Clone)]
pub struct GridCell {
    /// Canonical value: the `@key` form for localized strings, else the raw value.
    pub display: String,
    /// Resolved localized text, when a key resolved to something different.
    pub localized: Option<String>,
    pub kind: GridType,
    /// Value is an `@label` localization reference.
    pub is_key: bool,
    /// An `@key` that failed to resolve against the loaded localization.
    pub unresolved: bool,
}

impl GridCell {
    fn mismatch(&self, column: GridType) -> bool {
        !self.display.is_empty() && self.kind != column
    }

    fn invalid(&self, column: GridType) -> bool {
        self.unresolved || self.mismatch(column)
    }
}

/// One sheet's full grid, loaded on demand by a [`SheetSource`].
#[derive(Clone)]
pub struct SheetData {
    pub label: String,
    pub columns: Vec<GridColumn>,
    pub rows: Vec<Vec<GridCell>>,
    pub has_localization: bool,
}

/// A location of a value within the workspace.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Loc {
    pub sheet: u32,
    pub row: u32,
    pub col: u32,
}

/// Progress of the background cross-reference index.
#[derive(Clone, Copy)]
pub struct IndexProgress {
    pub indexed: usize,
    pub total: usize,
    pub done: bool,
}

/// The workspace's current localization state. `generation` bumps every time the
/// loaded catalog changes so the viewer knows to re-resolve the visible sheet.
#[derive(Clone, Default)]
pub struct LocaleState {
    /// The language currently loaded (or being loaded), e.g. `en-us`.
    pub code: Option<String>,
    /// A locale load is in flight in the background.
    pub loading: bool,
    /// Bumped whenever the resolved catalog changes.
    pub generation: u64,
    /// The last load error, if any.
    pub error: Option<String>,
}

/// A workspace of datasheets that the viewer can navigate across. Implementations
/// build their reference index in the background; queries return whatever is ready.
pub trait SheetSource: Send + Sync {
    /// Parse and build a sheet's grid (on demand).
    fn load(&self, sheet: u32) -> Option<SheetData>;
    /// Human-readable label for a sheet.
    fn label(&self, sheet: u32) -> String;
    /// Every indexed location whose cell value equals `value`.
    fn references(&self, value: &str) -> Vec<Loc>;
    /// Index build progress.
    fn progress(&self) -> IndexProgress;
    /// Language codes the user can switch to (empty when no install is available).
    fn locales(&self) -> Vec<String> {
        Vec::new()
    }
    /// Current localization state.
    fn locale(&self) -> LocaleState {
        LocaleState::default()
    }
    /// Request loading `code` (`None` clears localization). Loads in the background;
    /// progress is observed through [`SheetSource::locale`].
    fn set_locale(&self, code: Option<&str>) {
        let _ = code;
    }
    /// Snapshot of the discovered sheets as `(label, size)`. Grows during the
    /// background discovery sweep so the picker can stream.
    fn sheets(&self) -> Vec<(String, u64)> {
        Vec::new()
    }
    /// Progress of the background pak-discovery sweep (distinct from the
    /// cross-reference index in [`SheetSource::progress`]).
    fn discovery(&self) -> IndexProgress {
        IndexProgress {
            indexed: 0,
            total: 0,
            done: true,
        }
    }
}

/// How string cells are rendered once a locale is loaded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Display {
    Key,
    Text,
    Both,
}

impl Display {
    fn next(self) -> Self {
        match self {
            Display::Key => Display::Text,
            Display::Text => Display::Both,
            Display::Both => Display::Key,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Display::Key => "key",
            Display::Text => "text",
            Display::Both => "both",
        }
    }
}

struct LocList {
    title: String,
    value: String,
    defs_only: bool,
    locs: Vec<Loc>,
    sel: usize,
}

/// One choice in the language picker. `code` is `None` for the "keys only" option.
struct PickItem {
    code: Option<String>,
    label: String,
}

struct Picker {
    title: String,
    items: Vec<PickItem>,
    sel: usize,
}

enum Popup {
    Text { title: String, body: Vec<String> },
    Locations(LocList),
    Picker(Picker),
}

pub struct DatasheetView {
    source: Arc<dyn SheetSource>,
    sheet: u32,
    data: SheetData,
    col_widths: Vec<usize>,
    gutter: usize,
    caps: Caps,

    view: Vec<usize>,
    row: usize, // index into `view`
    col: usize, // absolute column
    row_off: usize,
    col_off: usize,

    filter: String,
    filtering: bool,
    display: Display,
    validate_only: bool,
    /// Last locale generation we re-resolved the sheet for.
    loc_gen: u64,

    status: Option<String>,
    popup: Option<Popup>,
    help: bool,
}

impl DatasheetView {
    pub fn new(source: Arc<dyn SheetSource>, sheet: u32, locale_mode: u8, caps: Caps) -> Self {
        let data = source.load(sheet).unwrap_or_else(|| SheetData {
            label: "(unavailable)".to_string(),
            columns: Vec::new(),
            rows: Vec::new(),
            has_localization: false,
        });
        let (col_widths, gutter) = compute_widths(&data);
        let display = match locale_mode {
            1 => Display::Text,
            2 => Display::Both,
            _ => Display::Key,
        };
        let loc_gen = source.locale().generation;
        let view = (0..data.rows.len()).collect();
        Self {
            source,
            sheet,
            data,
            col_widths,
            gutter,
            caps,
            view,
            row: 0,
            col: 0,
            row_off: 0,
            col_off: 0,
            filter: String::new(),
            filtering: false,
            display,
            validate_only: false,
            loc_gen,
            status: None,
            popup: None,
            help: false,
        }
    }

    fn shown(&self, cell: &GridCell) -> String {
        if cell.kind != GridType::String {
            return cell.display.clone();
        }
        match self.display {
            Display::Key => cell.display.clone(),
            Display::Text => cell
                .localized
                .clone()
                .unwrap_or_else(|| cell.display.clone()),
            Display::Both => match &cell.localized {
                Some(text) if *text != cell.display => format!("{} | {text}", cell.display),
                _ => cell.display.clone(),
            },
        }
    }

    fn cell(&self, view_row: usize, col: usize) -> Option<&GridCell> {
        let row = *self.view.get(view_row)?;
        self.data.rows.get(row).and_then(|cells| cells.get(col))
    }

    fn selected_cell(&self) -> Option<&GridCell> {
        self.cell(self.row, self.col)
    }

    fn row_invalid(&self, row: usize) -> bool {
        self.data.rows[row].iter().enumerate().any(|(col, cell)| {
            self.data
                .columns
                .get(col)
                .is_some_and(|c| cell.invalid(c.kind))
        })
    }

    /// Every searchable value on a row (raw + resolved text), joined for fuzzy
    /// matching.
    fn row_haystack(&self, row: usize) -> String {
        let mut parts = Vec::new();
        for cell in &self.data.rows[row] {
            parts.push(cell.display.as_str());
            if let Some(text) = &cell.localized {
                parts.push(text.as_str());
            }
        }
        parts.join(" ")
    }

    fn recompute(&mut self) {
        let anchor = self.view.get(self.row).copied();
        // Fuzzy-match values, but keep the natural row order so the grid still
        // reads like a spreadsheet (the gutter row numbers stay ascending).
        let matched = (!self.filter.is_empty()).then(|| {
            let haystacks = (0..self.data.rows.len())
                .map(|row| self.row_haystack(row))
                .collect::<Vec<_>>();
            fuzzy::rank(&self.filter, &haystacks)
                .into_iter()
                .map(|(row, _)| row)
                .collect::<HashSet<_>>()
        });
        self.view = (0..self.data.rows.len())
            .filter(|&row| {
                if self.validate_only && !self.row_invalid(row) {
                    return false;
                }
                matched.as_ref().is_none_or(|set| set.contains(&row))
            })
            .collect();
        self.row = anchor
            .and_then(|row| self.view.iter().position(|&candidate| candidate == row))
            .unwrap_or(0)
            .min(self.view.len().saturating_sub(1));
    }

    fn move_row(&mut self, delta: isize) {
        if self.view.is_empty() {
            return;
        }
        let last = (self.view.len() - 1) as isize;
        self.row = (self.row as isize).saturating_add(delta).clamp(0, last) as usize;
    }

    fn move_col(&mut self, delta: isize) {
        let last = self.data.columns.len().saturating_sub(1) as isize;
        self.col = (self.col as isize).saturating_add(delta).clamp(0, last) as usize;
    }

    fn toggle_display(&mut self) {
        if !self.data.has_localization {
            self.status = Some("no language loaded — press L to pick one".to_string());
            return;
        }
        self.display = self.display.next();
        self.status = Some(format!("showing {}", self.display.label()));
    }

    /// Open the language picker, listing every locale the workspace can load.
    fn pick_locale(&mut self) {
        let mut items = vec![PickItem {
            code: None,
            label: "keys only (no language)".to_string(),
        }];
        for code in self.source.locales() {
            items.push(PickItem {
                label: code.clone(),
                code: Some(code),
            });
        }
        if items.len() == 1 {
            self.status = Some(if self.source.discovery().done {
                "no localization available — run from the game install".to_string()
            } else {
                "languages still loading — try again in a moment".to_string()
            });
            return;
        }
        let current = self.source.locale().code;
        let sel = items
            .iter()
            .position(|item| item.code == current)
            .unwrap_or(0);
        self.popup = Some(Popup::Picker(Picker {
            title: "select language".to_string(),
            items,
            sel,
        }));
    }

    /// Re-resolve the visible sheet after a locale load completes, preserving the
    /// cursor and the active filter.
    fn reload_sheet(&mut self) {
        let Some(data) = self.source.load(self.sheet) else {
            return;
        };
        let anchor = self.view.get(self.row).copied();
        let (widths, gutter) = compute_widths(&data);
        self.data = data;
        self.col_widths = widths;
        self.gutter = gutter;
        // Reveal localization the first time it becomes available, but never
        // clobber the user's chosen display mode afterwards — `shown()` already
        // renders just the key when there's no localized text, so disabling a
        // language needs no mode change (and re-enabling won't double up).
        if self.data.has_localization && self.display == Display::Key {
            self.display = Display::Both;
        }
        self.recompute();
        if let Some(position) =
            anchor.and_then(|row| self.view.iter().position(|&candidate| candidate == row))
        {
            self.row = position;
        }
    }

    fn toggle_validation(&mut self) {
        self.validate_only = !self.validate_only;
        self.recompute();
        self.status = Some(if self.validate_only {
            "showing rows with issues".to_string()
        } else {
            "showing all rows".to_string()
        });
    }

    /// The shown value at a view row / absolute column (current locale + mode).
    fn shown_at(&self, view_row: usize, col: usize) -> String {
        self.cell(view_row, col)
            .map(|cell| self.shown(cell))
            .unwrap_or_default()
    }

    /// One row's shown values across all columns, tab-separated.
    fn row_text(&self, view_row: usize) -> Option<String> {
        if view_row >= self.view.len() {
            return None;
        }
        Some(
            (0..self.data.columns.len())
                .map(|col| self.shown_at(view_row, col))
                .collect::<Vec<_>>()
                .join("\t"),
        )
    }

    /// The whole (filtered) sheet as tab-separated values, header first.
    fn sheet_text(&self) -> String {
        let mut lines = Vec::with_capacity(self.view.len() + 1);
        lines.push(
            self.data
                .columns
                .iter()
                .map(|column| column.name.clone())
                .collect::<Vec<_>>()
                .join("\t"),
        );
        for view_row in 0..self.view.len() {
            if let Some(text) = self.row_text(view_row) {
                lines.push(text);
            }
        }
        lines.join("\n")
    }

    fn copy(&mut self, what: Copy) {
        let text = match what {
            Copy::Cell => self.selected_cell().map(|cell| self.shown(cell)),
            Copy::Row => self.row_text(self.row),
            Copy::Sheet => Some(self.sheet_text()),
        };
        let Some(text) = text else {
            self.status = Some("nothing to copy".to_string());
            return;
        };
        self.status = Some(match set_clipboard(&text) {
            Ok(()) => format!("copied {} to clipboard", what.label()),
            Err(error) => format!("clipboard error: {error}"),
        });
    }

    /// Gather references to `value` — current-sheet matches (immediate) merged
    /// with the growing cross-sheet index.
    fn gather(&self, value: &str, defs_only: bool) -> Vec<Loc> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for (row, cells) in self.data.rows.iter().enumerate() {
            for (col, cell) in cells.iter().enumerate() {
                if cell.display == value && (!defs_only || col == 0) {
                    let loc = Loc {
                        sheet: self.sheet,
                        row: row as u32,
                        col: col as u32,
                    };
                    if seen.insert(loc) {
                        out.push(loc);
                    }
                }
            }
        }
        for loc in self.source.references(value) {
            if defs_only && loc.col != 0 {
                continue;
            }
            if seen.insert(loc) {
                out.push(loc);
            }
        }
        out
    }

    fn find_references(&mut self) {
        let Some(value) = self.selected_cell().map(|cell| cell.display.clone()) else {
            return;
        };
        if value.is_empty() {
            self.status = Some("empty cell has no references".to_string());
            return;
        }
        let locs = self.gather(&value, false);
        self.popup = Some(Popup::Locations(LocList {
            title: "references".to_string(),
            value,
            defs_only: false,
            locs,
            sel: 0,
        }));
    }

    fn goto_definition(&mut self) {
        let Some(cell) = self.selected_cell() else {
            return;
        };
        let value = cell.display.clone();
        let is_key = cell.is_key;
        let localized = cell.localized.clone();
        if value.is_empty() {
            self.status = Some("empty cell".to_string());
            return;
        }
        let here = Loc {
            sheet: self.sheet,
            row: *self.view.get(self.row).unwrap_or(&0) as u32,
            col: self.col as u32,
        };
        let defs = self
            .gather(&value, true)
            .into_iter()
            .filter(|loc| *loc != here)
            .collect::<Vec<_>>();
        match defs.len() {
            0 => {
                if is_key {
                    let text = localized.unwrap_or_else(|| "(unresolved)".to_string());
                    self.popup = Some(Popup::Text {
                        title: value,
                        body: vec![text],
                    });
                } else {
                    self.status = Some("no definition for this value".to_string());
                }
            }
            1 => {
                let loc = defs[0];
                self.jump(loc);
                self.status = Some(format!("→ {}", self.source.label(loc.sheet)));
            }
            _ => {
                self.popup = Some(Popup::Locations(LocList {
                    title: "definitions".to_string(),
                    value,
                    defs_only: true,
                    locs: defs,
                    sel: 0,
                }));
            }
        }
    }

    fn jump(&mut self, loc: Loc) {
        if loc.sheet == self.sheet {
            self.jump_in_sheet(loc.row as usize, loc.col as usize);
        } else if let Some(data) = self.source.load(loc.sheet) {
            self.sheet = loc.sheet;
            let (widths, gutter) = compute_widths(&data);
            self.data = data;
            self.col_widths = widths;
            self.gutter = gutter;
            self.filter.clear();
            self.validate_only = false;
            self.view = (0..self.data.rows.len()).collect();
            self.row_off = 0;
            self.col_off = 0;
            self.jump_in_sheet(loc.row as usize, loc.col as usize);
        }
    }

    fn jump_in_sheet(&mut self, row: usize, col: usize) {
        if !self.view.contains(&row) {
            self.validate_only = false;
            self.filter.clear();
            self.recompute();
        }
        if let Some(position) = self.view.iter().position(|&candidate| candidate == row) {
            self.row = position;
        }
        self.col = col.min(self.data.columns.len().saturating_sub(1));
    }
}

impl View for DatasheetView {
    fn ticking(&self) -> bool {
        // Keep redrawing while the index builds, a locale loads in the
        // background, or a results popup can still grow.
        let locale = self.source.locale();
        let index_growing =
            !self.source.progress().done && matches!(self.popup, None | Some(Popup::Locations(_)));
        index_growing || locale.loading || locale.generation != self.loc_gen
    }

    fn tick(&mut self) {
        // A locale load finished (or was cleared): re-resolve the visible sheet.
        let locale = self.source.locale();
        if locale.generation != self.loc_gen {
            self.loc_gen = locale.generation;
            self.reload_sheet();
            self.status = Some(match (&locale.code, &locale.error) {
                (_, Some(error)) => format!("locale error: {error}"),
                (Some(code), None) => format!("language: {code}"),
                (None, None) => "language cleared".to_string(),
            });
        }
        // Refresh an open results list so cross-sheet matches stream in.
        if let Some(Popup::Locations(list)) = &self.popup {
            let anchor = list.locs.get(list.sel).copied();
            let value = list.value.clone();
            let defs_only = list.defs_only;
            let title = list.title.clone();
            let locs = self.gather(&value, defs_only);
            let sel = anchor
                .and_then(|loc| locs.iter().position(|candidate| *candidate == loc))
                .unwrap_or(0);
            self.popup = Some(Popup::Locations(LocList {
                title,
                value,
                defs_only,
                locs,
                sel,
            }));
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Flow {
        if let Some(popup) = &mut self.popup {
            match popup {
                Popup::Text { .. } => {
                    self.popup = None;
                }
                Popup::Locations(list) => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => self.popup = None,
                    KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('n') => {
                        if !list.locs.is_empty() {
                            list.sel = (list.sel + 1) % list.locs.len();
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up | KeyCode::Char('N') => {
                        if !list.locs.is_empty() {
                            list.sel = (list.sel + list.locs.len() - 1) % list.locs.len();
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(&loc) = list.locs.get(list.sel) {
                            self.popup = None;
                            self.jump(loc);
                        }
                    }
                    _ => {}
                },
                Popup::Picker(picker) => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => self.popup = None,
                    KeyCode::Char('j') | KeyCode::Down => {
                        picker.sel = (picker.sel + 1) % picker.items.len();
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        picker.sel = (picker.sel + picker.items.len() - 1) % picker.items.len();
                    }
                    KeyCode::Enter => {
                        if let Some(item) = picker.items.get(picker.sel) {
                            let code = item.code.clone();
                            self.source.set_locale(code.as_deref());
                            self.status = Some(match &code {
                                Some(code) => format!("loading language {code}…"),
                                None => "language cleared".to_string(),
                            });
                            self.popup = None;
                        }
                    }
                    _ => {}
                },
            }
            return Flow::Continue;
        }
        if self.help {
            self.help = false;
            return Flow::Continue;
        }
        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.filtering = false;
                    self.recompute();
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.recompute();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.recompute();
                }
                _ => {}
            }
            return Flow::Continue;
        }

        self.status = None;
        let page = 10;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Flow::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Flow::Quit;
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_row(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_row(-1),
            KeyCode::PageDown => self.move_row(page),
            KeyCode::PageUp => self.move_row(-page),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_row(page)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_row(-page)
            }
            KeyCode::Char('h') | KeyCode::Left => self.move_col(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_col(1),
            KeyCode::Char('g') | KeyCode::Home => self.row = 0,
            KeyCode::Char('G') | KeyCode::End => self.row = self.view.len().saturating_sub(1),
            KeyCode::Char('L') => self.pick_locale(),
            KeyCode::Char('t') => self.toggle_display(),
            KeyCode::Char('v') => self.toggle_validation(),
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.copy(Copy::Sheet)
            }
            KeyCode::Char('y') => self.copy(Copy::Cell),
            KeyCode::Char('Y') => self.copy(Copy::Row),
            KeyCode::Char('*') => self.find_references(),
            KeyCode::Enter => self.goto_definition(),
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Char('?') => self.help = true,
            _ => {}
        }
        Flow::Continue
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area);
        let (top, head, body, info, bottom) =
            (chunks[0], chunks[1], chunks[2], chunks[3], chunks[4]);

        let avail = body.width as usize;
        if self.col < self.col_off {
            self.col_off = self.col;
        }
        let mut cols = self.visible_cols(avail);
        if !cols.contains(&self.col) {
            self.col_off = self.col;
            cols = self.visible_cols(avail);
        }

        self.render_top(frame, top);
        self.render_header(frame, head, &cols);
        self.render_body(frame, body, &cols);
        self.render_info(frame, info);
        self.render_bottom(frame, bottom);
        if let Some(popup) = &self.popup {
            self.render_popup(frame, area, popup);
        }
        if self.help {
            self.render_help(frame, area);
        }
    }
}

impl DatasheetView {
    fn visible_cols(&self, avail: usize) -> Vec<usize> {
        let mut cols = Vec::new();
        let mut used = self.gutter + COL_GAP;
        for index in self.col_off..self.data.columns.len() {
            let width = self.col_widths[index] + COL_GAP;
            if !cols.is_empty() && used + width > avail {
                break;
            }
            used += width;
            cols.push(index);
        }
        if cols.is_empty() && self.col_off < self.data.columns.len() {
            cols.push(self.col_off);
        }
        cols
    }

    fn render_top(&self, frame: &mut Frame, area: Rect) {
        let glyphs = theme::glyphs(self.caps);
        let locale = self.source.locale();
        let (lang_label, lang_style) = if locale.loading {
            (
                locale
                    .code
                    .clone()
                    .map_or_else(|| "loading…".to_string(), |code| format!("{code}…")),
                theme::warn(),
            )
        } else if let Some(error) = &locale.error {
            (format!("error: {error}"), theme::bad())
        } else {
            match &locale.code {
                Some(code) => (code.clone(), theme::bold()),
                None => ("none".to_string(), theme::dim()),
            }
        };
        let mut spans = vec![
            Span::styled(self.data.label.clone(), theme::accent()),
            Span::raw("   "),
            Span::styled("rows ", theme::dim()),
            Span::styled(self.view.len().to_string(), theme::bold()),
            Span::styled(glyphs.sep.to_string(), theme::dim()),
            Span::styled("cols ", theme::dim()),
            Span::styled(self.data.columns.len().to_string(), theme::bold()),
            Span::styled(glyphs.sep.to_string(), theme::dim()),
            Span::styled("lang ", theme::dim()),
            Span::styled(lang_label, lang_style),
        ];
        if self.data.has_localization {
            spans.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
            spans.push(Span::styled("show ", theme::dim()));
            spans.push(Span::styled(
                self.display.label().to_string(),
                theme::bold(),
            ));
        }
        if self.validate_only {
            spans.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
            spans.push(Span::styled("issues only".to_string(), theme::warn()));
        }
        let progress = self.source.progress();
        if !progress.done {
            spans.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
            spans.push(Span::styled(
                format!("indexing {}/{}", progress.indexed, progress.total),
                theme::warn(),
            ));
        }
        let buf = frame.buffer_mut();
        buf.set_line(area.x, area.y, &Line::from(spans), area.width);

        let right = format!(
            "r{}/{} c{}/{}",
            self.view.len().min(self.row + 1),
            self.view.len(),
            self.col + 1,
            self.data.columns.len()
        );
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

    fn render_header(&self, frame: &mut Frame, area: Rect, cols: &[usize]) {
        let mut spans = vec![Span::raw(" ".repeat(self.gutter + COL_GAP))];
        for &col in cols {
            let width = self.col_widths[col];
            let name = theme::fit_end(&self.data.columns[col].name, width, "…");
            let style = if col == self.col {
                theme::accent()
            } else {
                theme::header()
            };
            spans.push(Span::styled(format!("{name:<width$}"), style));
            spans.push(Span::raw(" ".repeat(COL_GAP)));
        }
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &Line::from(spans), area.width);
    }

    fn render_body(&mut self, frame: &mut Frame, area: Rect, cols: &[usize]) {
        let visible = area.height as usize;
        if visible == 0 || self.view.is_empty() {
            return;
        }
        if self.row < self.row_off {
            self.row_off = self.row;
        } else if self.row >= self.row_off + visible {
            self.row_off = self.row + 1 - visible;
        }

        for slot in 0..visible {
            let view_row = self.row_off + slot;
            let Some(&row_index) = self.view.get(view_row) else {
                break;
            };
            let y = area.y + slot as u16;
            let selected_row = view_row == self.row;
            let line = self.row_line(row_index, cols, selected_row);
            frame.buffer_mut().set_line(area.x, y, &line, area.width);
        }
    }

    fn row_line(&self, row_index: usize, cols: &[usize], selected_row: bool) -> Line<'static> {
        let row = &self.data.rows[row_index];
        let gutter_style = if selected_row {
            theme::accent()
        } else {
            theme::dim()
        };
        let mut spans = vec![
            Span::styled(
                format!("{row_index:>width$}", width = self.gutter),
                gutter_style,
            ),
            Span::raw(" ".repeat(COL_GAP)),
        ];
        for &col in cols {
            let width = self.col_widths[col];
            let column_kind = self.data.columns[col].kind;
            let blank = GridCell {
                display: String::new(),
                localized: None,
                kind: column_kind,
                is_key: false,
                unresolved: false,
            };
            let cell = row.get(col).unwrap_or(&blank);
            let text = theme::fit_end(&self.shown(cell), width, "…");
            let mut style = self.cell_style(cell, column_kind);
            if selected_row && col == self.col && self.caps.color {
                // Reverse-video cursor: unmistakable, and preserves the cell's
                // semantic color as the highlight background.
                style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
            }
            spans.push(Span::styled(format!("{text:<width$}"), style));
            spans.push(Span::raw(" ".repeat(COL_GAP)));
        }
        Line::from(spans)
    }

    fn cell_style(&self, cell: &GridCell, column: GridType) -> Style {
        if !self.caps.color {
            return Style::default();
        }
        if cell.unresolved {
            return theme::bad();
        }
        if cell.mismatch(column) {
            return theme::warn();
        }
        if cell.display.is_empty() {
            return theme::dim();
        }
        match cell.kind {
            GridType::Number => Style::default().fg(NUMBER),
            GridType::Boolean => {
                if cell.display == "true" {
                    theme::good()
                } else {
                    theme::dim()
                }
            }
            GridType::String if cell.is_key => Style::default().fg(theme::palette::INFO),
            GridType::String => Style::default(),
        }
    }

    fn render_info(&self, frame: &mut Frame, area: Rect) {
        let line = if let Some(status) = &self.status {
            Line::from(Span::styled(status.clone(), theme::accent()))
        } else if let Some(cell) = self.selected_cell() {
            let column = &self.data.columns[self.col];
            let mut spans = vec![
                Span::styled(format!("{} ", column.name), theme::bold()),
                Span::styled(format!("({}) ", column.kind.label()), theme::dim()),
                Span::raw("= "),
                Span::styled(self.shown(cell), self.cell_style(cell, column.kind)),
            ];
            if cell.unresolved {
                spans.push(Span::styled("   unresolved @key".to_string(), theme::bad()));
            } else if cell.mismatch(column.kind) {
                spans.push(Span::styled(
                    format!(
                        "   value is {} in a {} column",
                        cell.kind.label(),
                        column.kind.label()
                    ),
                    theme::warn(),
                ));
            }
            Line::from(spans)
        } else {
            Line::from(Span::raw(""))
        };
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &line, area.width);
    }

    fn render_bottom(&self, frame: &mut Frame, area: Rect) {
        let line = if self.filtering {
            Line::from(vec![
                Span::styled("/", theme::accent()),
                Span::raw(self.filter.clone()),
                Span::styled("▏", theme::dim()),
            ])
        } else {
            let hint = if self.caps.unicode {
                "↑↓←→ move   ↵ goto def   * refs   L lang   t show   y copy   / filter   ? help   q quit"
            } else {
                "move arrows   enter goto-def   * refs   L lang   t show   y copy   / filter   ? help   q"
            };
            Line::from(Span::styled(hint.to_string(), theme::dim()))
        };
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &line, area.width);
    }

    fn render_popup(&self, frame: &mut Frame, area: Rect, popup: &Popup) {
        match popup {
            Popup::Text { title, body } => {
                let lines = body
                    .iter()
                    .map(|line| Line::from(Span::raw(line.clone())))
                    .collect::<Vec<_>>();
                let width = 70u16.min(area.width.saturating_sub(2)).max(10);
                let height = (lines.len() as u16 + 2).min(area.height);
                let rect = centered(area, width, height);
                frame.render_widget(Clear, rect);
                frame.render_widget(
                    Paragraph::new(lines).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!(" {title} "))
                            .border_style(theme::accent()),
                    ),
                    rect,
                );
            }
            Popup::Locations(list) => self.render_locations(frame, area, list),
            Popup::Picker(picker) => self.render_picker(frame, area, picker),
        }
    }

    fn render_picker(&self, frame: &mut Frame, area: Rect, picker: &Picker) {
        let width = 40u16.min(area.width.saturating_sub(2)).max(16);
        let visible = area.height.saturating_sub(4).clamp(1, 18);
        let height = (picker.items.len() as u16 + 2).clamp(3, visible + 2);
        let rect = centered(area, width, height);
        frame.render_widget(Clear, rect);

        let inner = height.saturating_sub(2) as usize;
        let start = picker
            .sel
            .saturating_sub(inner.saturating_sub(1))
            .min(picker.items.len().saturating_sub(inner));
        let mut lines = Vec::new();
        for (offset, item) in picker.items.iter().skip(start).take(inner).enumerate() {
            let index = start + offset;
            let marker = if index == picker.sel { "›" } else { " " };
            let mut spans = vec![
                Span::styled(format!(" {marker} "), theme::accent()),
                Span::raw(item.label.clone()),
            ];
            if index == picker.sel {
                for span in &mut spans {
                    span.style = span.style.bg(ROW_HILITE).add_modifier(Modifier::BOLD);
                }
            }
            lines.push(Line::from(spans));
        }
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", picker.title))
                    .border_style(theme::accent()),
            ),
            rect,
        );
    }

    fn render_locations(&self, frame: &mut Frame, area: Rect, list: &LocList) {
        let progress = self.source.progress();
        let suffix = if progress.done {
            String::new()
        } else {
            format!(" (indexing {}/{}…)", progress.indexed, progress.total)
        };
        let title = format!(
            " {} to \"{}\" — {}{} ",
            list.title,
            theme::fit_end(&list.value, 30, "…"),
            list.locs.len(),
            suffix
        );

        let width = 76u16.min(area.width.saturating_sub(2)).max(20);
        let visible = area.height.saturating_sub(4).clamp(1, 16);
        let height = (list.locs.len() as u16 + 2).clamp(3, visible + 2);
        let rect = centered(area, width, height);
        frame.render_widget(Clear, rect);

        let inner = height.saturating_sub(2) as usize;
        let start = list
            .sel
            .saturating_sub(inner.saturating_sub(1))
            .min(list.locs.len().saturating_sub(inner));
        let mut lines = Vec::new();
        if list.locs.is_empty() {
            lines.push(Line::from(Span::styled(
                "  no matches yet".to_string(),
                theme::dim(),
            )));
        }
        for (offset, loc) in list.locs.iter().skip(start).take(inner).enumerate() {
            let index = start + offset;
            let label = self.source.label(loc.sheet);
            let same = loc.sheet == self.sheet;
            let marker = if same { "·" } else { "→" };
            let mut spans = vec![
                Span::styled(format!(" {marker} "), theme::dim()),
                Span::styled(
                    theme::fit_end(&label, 44, "…"),
                    if same { theme::dim() } else { theme::accent() },
                ),
                Span::styled(format!("  r{} c{}", loc.row, loc.col), theme::dim()),
            ];
            if index == list.sel {
                for span in &mut spans {
                    span.style = span.style.bg(ROW_HILITE).add_modifier(Modifier::BOLD);
                }
            }
            lines.push(Line::from(spans));
        }

        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(theme::accent()),
            ),
            rect,
        );
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let keys = [
            ("↑ ↓ ← → / hjkl", "move cell"),
            ("↵ enter", "goto definition (cross-sheet)"),
            ("*", "find references (cross-sheet, grows live)"),
            ("in popup: ↵", "jump (loads the other sheet)"),
            ("L", "pick language (loads in background)"),
            ("t", "cycle display: key / text / both"),
            ("y / Y / ^y", "copy cell / row / whole sheet"),
            ("v", "show only rows with validation issues"),
            ("/", "fuzzy filter rows by value"),
            ("g G", "first / last row"),
            ("q esc", "quit"),
        ];
        let width = 60u16.min(area.width);
        let height = (keys.len() as u16 + 2).min(area.height);
        let rect = centered(area, width, height);
        let lines = keys
            .iter()
            .map(|(key, desc)| {
                Line::from(vec![
                    Span::styled(format!(" {key:<16}"), theme::accent()),
                    Span::styled((*desc).to_string(), theme::dim()),
                ])
            })
            .collect::<Vec<_>>();
        frame.render_widget(Clear, rect);
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" keys ")
                    .border_style(theme::dim()),
            ),
            rect,
        );
    }
}

/// What a copy action targets.
#[derive(Clone, Copy)]
enum Copy {
    Cell,
    Row,
    Sheet,
}

impl Copy {
    fn label(self) -> &'static str {
        match self {
            Copy::Cell => "cell",
            Copy::Row => "row",
            Copy::Sheet => "sheet",
        }
    }
}

/// Put `text` on the system clipboard.
fn set_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text.to_string()))
        .map_err(|error| error.to_string())
}

fn compute_widths(data: &SheetData) -> (Vec<usize>, usize) {
    let mut widths = data
        .columns
        .iter()
        .map(|column| theme::display_width(&column.name).clamp(MIN_COL, MAX_COL))
        .collect::<Vec<_>>();
    for row in &data.rows {
        for (index, cell) in row.iter().enumerate() {
            if let Some(width) = widths.get_mut(index) {
                let display = theme::display_width(&cell.display);
                let localized = cell.localized.as_deref().map_or(0, theme::display_width);
                *width = (*width).max(display).max(localized).min(MAX_COL);
            }
        }
    }
    let gutter = data.rows.len().to_string().len().max(1);
    (widths, gutter)
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn str_cell(display: &str) -> GridCell {
        GridCell {
            display: display.to_string(),
            localized: None,
            kind: GridType::String,
            is_key: false,
            unresolved: false,
        }
    }

    struct MockSource {
        sheets: Vec<SheetData>,
        refs: Mutex<HashMap<String, Vec<Loc>>>,
        done: bool,
    }

    impl SheetSource for MockSource {
        fn load(&self, sheet: u32) -> Option<SheetData> {
            self.sheets.get(sheet as usize).cloned()
        }
        fn label(&self, sheet: u32) -> String {
            self.sheets
                .get(sheet as usize)
                .map_or_else(|| "?".into(), |data| data.label.clone())
        }
        fn references(&self, value: &str) -> Vec<Loc> {
            self.refs
                .lock()
                .unwrap()
                .get(value)
                .cloned()
                .unwrap_or_default()
        }
        fn progress(&self) -> IndexProgress {
            IndexProgress {
                indexed: self.sheets.len(),
                total: self.sheets.len(),
                done: self.done,
            }
        }
    }

    fn sheet(label: &str, rows: Vec<Vec<GridCell>>) -> SheetData {
        SheetData {
            label: label.to_string(),
            columns: vec![
                GridColumn {
                    name: "Key".into(),
                    kind: GridType::String,
                },
                GridColumn {
                    name: "Ref".into(),
                    kind: GridType::String,
                },
            ],
            rows,
            has_localization: false,
        }
    }

    fn view_with(refs: HashMap<String, Vec<Loc>>, sheets: Vec<SheetData>) -> DatasheetView {
        let source = Arc::new(MockSource {
            sheets,
            refs: Mutex::new(refs),
            done: true,
        });
        DatasheetView::new(source, 0, 0, Caps::PLAIN)
    }

    #[test]
    fn in_sheet_goto_definition_jumps_to_key_row() {
        let s = sheet(
            "a",
            vec![
                vec![str_cell("alpha"), str_cell("gamma")],
                vec![str_cell("beta"), str_cell("alpha")],
            ],
        );
        let mut view = view_with(HashMap::new(), vec![s]);
        view.row = 1; // "beta"
        view.col = 1; // value "alpha"
        view.goto_definition();
        assert_eq!(view.view[view.row], 0, "jumped to the alpha-keyed row");
        assert_eq!(view.col, 0);
    }

    #[test]
    fn cross_sheet_references_merge_and_jump_switches_sheet() {
        let a = sheet("a", vec![vec![str_cell("alpha"), str_cell("x")]]);
        let b = sheet("b", vec![vec![str_cell("z"), str_cell("alpha")]]);
        let mut refs = HashMap::new();
        // index reports alpha in sheet b at (row 0, col 1) and sheet a (row0,col0)
        refs.insert(
            "alpha".to_string(),
            vec![
                Loc {
                    sheet: 0,
                    row: 0,
                    col: 0,
                },
                Loc {
                    sheet: 1,
                    row: 0,
                    col: 1,
                },
            ],
        );
        let mut view = view_with(refs, vec![a, b]);
        view.row = 0;
        view.col = 0; // "alpha" in sheet a
        view.find_references();
        let Some(Popup::Locations(list)) = &view.popup else {
            panic!("expected a references popup");
        };
        assert_eq!(list.locs.len(), 2, "current sheet + cross-sheet, deduped");

        // Jump to the cross-sheet reference.
        let cross = Loc {
            sheet: 1,
            row: 0,
            col: 1,
        };
        view.jump(cross);
        assert_eq!(view.sheet, 1, "switched to sheet b");
        assert_eq!(view.data.label, "b");
        assert_eq!(view.col, 1);
    }

    struct LocaleMock {
        generation: Mutex<u64>,
        loaded: Mutex<bool>,
    }

    impl SheetSource for LocaleMock {
        fn load(&self, sheet: u32) -> Option<SheetData> {
            if sheet != 0 {
                return None;
            }
            let loaded = *self.loaded.lock().unwrap();
            let cell = GridCell {
                display: "@Name".into(),
                localized: loaded.then(|| "Iron Sword".to_string()),
                kind: GridType::String,
                is_key: true,
                unresolved: false,
            };
            Some(SheetData {
                label: "items".into(),
                columns: vec![GridColumn {
                    name: "Name".into(),
                    kind: GridType::String,
                }],
                rows: vec![vec![cell]],
                has_localization: loaded,
            })
        }
        fn label(&self, _sheet: u32) -> String {
            "items".into()
        }
        fn references(&self, _value: &str) -> Vec<Loc> {
            Vec::new()
        }
        fn progress(&self) -> IndexProgress {
            IndexProgress {
                indexed: 1,
                total: 1,
                done: true,
            }
        }
        fn locales(&self) -> Vec<String> {
            vec!["en-us".into()]
        }
        fn locale(&self) -> LocaleState {
            LocaleState {
                code: self.loaded.lock().unwrap().then(|| "en-us".to_string()),
                loading: false,
                generation: *self.generation.lock().unwrap(),
                error: None,
            }
        }
        fn set_locale(&self, code: Option<&str>) {
            *self.loaded.lock().unwrap() = code.is_some();
            *self.generation.lock().unwrap() += 1;
        }
    }

    #[test]
    fn picking_a_language_reloads_localized_text() {
        let source = Arc::new(LocaleMock {
            generation: Mutex::new(0),
            loaded: Mutex::new(false),
        });
        let mut view = DatasheetView::new(source.clone(), 0, 0, Caps::PLAIN);
        assert!(!view.data.has_localization, "starts with keys only");

        // The workspace loads the language (synchronous in the mock); the view
        // notices the new generation on the next tick and re-resolves the sheet.
        source.set_locale(Some("en-us"));
        view.tick();

        assert!(view.data.has_localization);
        assert_eq!(view.display, Display::Both, "display switches off key-only");
        let cell = view.selected_cell().unwrap();
        assert!(
            view.shown(cell).contains("Iron Sword"),
            "resolved text shows after the language loads"
        );
    }

    #[test]
    fn copy_builds_tab_separated_values() {
        let s = sheet("a", vec![vec![str_cell("alpha"), str_cell("beta")]]);
        let view = view_with(HashMap::new(), vec![s]);
        assert_eq!(view.row_text(0).as_deref(), Some("alpha\tbeta"));
        assert_eq!(view.sheet_text(), "Key\tRef\nalpha\tbeta");
        assert_eq!(view.row_text(5), None, "out-of-range row");
    }

    #[test]
    fn type_mismatch_is_invalid() {
        let data = SheetData {
            label: "a".into(),
            columns: vec![GridColumn {
                name: "Num".into(),
                kind: GridType::Number,
            }],
            rows: vec![vec![str_cell("not-a-number")]],
            has_localization: false,
        };
        let view = view_with(HashMap::new(), vec![data]);
        assert!(view.row_invalid(0), "string in a number column is invalid");
    }

    #[test]
    fn unresolved_localization_key_is_invalid() {
        let data = SheetData {
            label: "a".into(),
            columns: vec![GridColumn {
                name: "Name".into(),
                kind: GridType::String,
            }],
            rows: vec![vec![GridCell {
                display: "@Missing".into(),
                localized: None,
                kind: GridType::String,
                is_key: true,
                unresolved: true,
            }]],
            has_localization: false,
        };
        let view = view_with(HashMap::new(), vec![data]);
        assert!(view.row_invalid(0));
    }
}
