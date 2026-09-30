//! A streaming picker over a [`SheetSource`]'s datasheets. The list is pulled
//! from the source on every tick, so it fills in live while the background
//! discovery sweep parses pak archives — the UI is usable from the first frame.
//!
//! The `/` filter matches sheet names instantly on the keystroke path, then
//! merges content hits — columns and cell values at full recall — from a
//! debounced background search over the entire indexed universe. Content
//! matches stream in as they arrive (and re-fire as indexing progresses) and
//! are marked with a dim `~`; the keystroke path never parses a sheet, so
//! filtering never blocks.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use humansize::{DECIMAL, format_size};

use super::app::{Flow, View};
use super::datasheet::SheetSource;
use crate::fuzzy;
use crate::ui::theme::{self, Caps};

const HILITE: ratatui::style::Color = ratatui::style::Color::Indexed(238);

/// Idle time after the last keystroke before the background value search fires.
/// The name/column tier is instant; this only delays the full-recall pass so
/// fast typing issues one search per pause instead of one per keystroke.
const CONTENT_DEBOUNCE: Duration = Duration::from_millis(200);

pub struct SheetPicker {
    source: Arc<dyn SheetSource>,
    caps: Caps,
    sheets: Vec<(String, u64)>,
    view: Vec<usize>,
    selected: usize,
    offset: usize,
    filter: String,
    filtering: bool,
    picked: Option<u32>,
    /// Sheets in `view` that matched contents but not the sheet name.
    content_hits: HashSet<usize>,
    /// Background content-search hits (sheet ids, best-score-first) for
    /// `bg_for_query`. Merged below the name tier by [`Self::recompute`].
    bg_sheets: Vec<usize>,
    /// Query `bg_sheets` was computed for; stale results never merge.
    bg_for_query: String,
    /// Generation of the outstanding background request (`0` = none).
    bg_gen: u64,
    /// Query last sent to the background search.
    searched_query: String,
    /// Last keystroke into the filter; drives the search debounce.
    last_edit: Instant,
    /// Last indexer count observed. Advances re-fire a live filter's search so
    /// content matches stream in as indexing progresses.
    indexed: usize,
    /// The index grew since the last search request.
    index_dirty: bool,
    /// Set once we've refreshed after discovery reported done, so the final batch
    /// of sheets is never missed before the picker stops ticking.
    synced_done: bool,
}

impl SheetPicker {
    pub fn new(source: Arc<dyn SheetSource>, caps: Caps) -> Self {
        let sheets = source.sheets();
        let indexed = source.progress().indexed;
        let mut picker = Self {
            source,
            caps,
            sheets,
            view: Vec::new(),
            selected: 0,
            offset: 0,
            filter: String::new(),
            filtering: false,
            picked: None,
            content_hits: HashSet::new(),
            bg_sheets: Vec::new(),
            bg_for_query: String::new(),
            bg_gen: 0,
            searched_query: String::new(),
            last_edit: Instant::now(),
            indexed,
            index_dirty: false,
            synced_done: false,
        };
        picker.recompute();
        picker
    }

    /// The chosen sheet id, if the user pressed Enter.
    pub fn picked(&self) -> Option<u32> {
        self.picked
    }

    fn recompute(&mut self) {
        let anchor = self.view.get(self.selected).copied();
        self.content_hits.clear();
        if self.filter.is_empty() {
            self.view = (0..self.sheets.len()).collect();
        } else {
            // Name matches first, best fuzzy score first.
            let haystacks = self
                .sheets
                .iter()
                .map(|(label, _)| label.clone())
                .collect::<Vec<_>>();
            let mut view = Vec::new();
            let mut listed = HashSet::new();
            for (index, _) in fuzzy::rank(&self.filter, &haystacks) {
                listed.insert(index);
                view.push(index);
            }
            // Background content hits, but only for the exact query they were
            // computed for — stale generations never merge.
            if self.bg_for_query == self.filter {
                let bg = self.bg_sheets.clone();
                for index in bg {
                    if listed.insert(index) {
                        self.content_hits.insert(index);
                        view.push(index);
                    }
                }
            }
            self.view = view;
        }
        self.selected = anchor
            .and_then(|row| self.view.iter().position(|&candidate| candidate == row))
            .unwrap_or(0)
            .min(self.view.len().saturating_sub(1));
    }

    /// A value search still needs requesting once the user pauses typing.
    fn debounce_pending(&self) -> bool {
        !self.filter.is_empty() && self.searched_query != self.filter
    }

    /// A value search was requested for the live filter but hasn't landed.
    fn bg_outstanding(&self) -> bool {
        !self.filter.is_empty()
            && self.bg_gen != 0
            && self.searched_query == self.filter
            && self.bg_for_query != self.filter
    }

    /// Fire the debounced full-recall search — once the user pauses after a
    /// query change, or when newly indexed sheets arrived since the last
    /// request — then merge the latest completion. Stale generations are
    /// ignored. At most one request is ever outstanding.
    fn poll_background_search(&mut self) {
        if !self.filter.is_empty()
            && (self.searched_query != self.filter || self.index_dirty)
            && self.last_edit.elapsed() >= CONTENT_DEBOUNCE
            && !self.bg_outstanding()
        {
            self.searched_query = self.filter.clone();
            self.index_dirty = false;
            self.bg_gen = self.source.search_contents(self.filter.clone());
        }
        if let Some((generation, hits)) = self.source.take_content_results()
            && generation != 0
            && generation == self.bg_gen
            && self.searched_query == self.filter
        {
            self.bg_for_query = self.filter.clone();
            self.bg_sheets = hits
                .into_iter()
                .map(|(sheet, _)| sheet as usize)
                .filter(|&sheet| sheet < self.sheets.len())
                .collect();
            self.recompute();
        }
    }

    fn move_by(&mut self, delta: isize) {
        if self.view.is_empty() {
            return;
        }
        let last = (self.view.len() - 1) as isize;
        self.selected = (self.selected as isize)
            .saturating_add(delta)
            .clamp(0, last) as usize;
    }
}

impl View for SheetPicker {
    fn ticking(&self) -> bool {
        // Discovery and its settle tick; a debounced search pending; a
        // requested search not yet landed; and a live filter while the index
        // still grows (newly indexed sheets re-fire the search).
        !self.source.discovery().done
            || !self.synced_done
            || self.debounce_pending()
            || self.bg_outstanding()
            || (!self.filter.is_empty() && (!self.source.progress().done || self.index_dirty))
    }

    fn tick(&mut self) {
        let sheets = self.source.sheets();
        if sheets.len() != self.sheets.len() {
            self.sheets = sheets;
            self.recompute();
        }
        if self.source.discovery().done {
            self.synced_done = true;
        }
        let indexed = self.source.progress().indexed;
        if indexed != self.indexed {
            self.indexed = indexed;
            self.index_dirty = true;
        }
        self.poll_background_search();
    }

    fn on_key(&mut self, key: KeyEvent) -> Flow {
        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.filtering = false;
                    self.bg_sheets.clear();
                    self.bg_for_query.clear();
                    self.bg_gen = 0;
                    self.searched_query.clear();
                    self.recompute();
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.last_edit = Instant::now();
                    self.recompute();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.last_edit = Instant::now();
                    self.recompute();
                }
                _ => {}
            }
            return Flow::Continue;
        }

        let page = 10;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Flow::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Flow::Quit;
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::PageDown => self.move_by(page),
            KeyCode::PageUp => self.move_by(-page),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_by(page)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_by(-page)
            }
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.selected = self.view.len().saturating_sub(1),
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Enter => {
                if let Some(&index) = self.view.get(self.selected) {
                    self.picked = Some(index as u32);
                    return Flow::Quit;
                }
            }
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
            ])
            .split(area);
        let (top, divider, body, bottom) = (chunks[0], chunks[1], chunks[2], chunks[3]);

        self.render_top(frame, top);
        let rule = std::iter::repeat_n('─', divider.width as usize).collect::<String>();
        frame.buffer_mut().set_line(
            divider.x,
            divider.y,
            &Line::from(Span::styled(rule, theme::dim())),
            divider.width,
        );
        self.render_body(frame, body);
        self.render_bottom(frame, bottom);
    }
}

impl SheetPicker {
    fn render_top(&self, frame: &mut Frame, area: Rect) {
        let glyphs = theme::glyphs(self.caps);
        let mut spans = vec![
            Span::styled("datasheets", theme::accent()),
            Span::raw("   "),
            Span::styled("found ", theme::dim()),
            Span::styled(self.sheets.len().to_string(), theme::bold()),
        ];
        let discovery = self.source.discovery();
        if !discovery.done {
            spans.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
            spans.push(Span::styled(
                format!("scanning {}/{}", discovery.indexed, discovery.total),
                theme::warn(),
            ));
        }
        // Content matches keep growing until the index finishes.
        let progress = self.source.progress();
        if !progress.done {
            spans.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
            spans.push(Span::styled(
                format!("indexing {}/{}", progress.indexed, progress.total),
                theme::warn(),
            ));
        }
        if self.bg_outstanding() {
            spans.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
            spans.push(Span::styled("searching…".to_string(), theme::warn()));
        }
        let buf = frame.buffer_mut();
        buf.set_line(area.x, area.y, &Line::from(spans), area.width);

        let right = if self.view.is_empty() {
            "0/0".to_string()
        } else {
            format!("{}/{}", self.selected + 1, self.view.len())
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

    fn render_body(&mut self, frame: &mut Frame, area: Rect) {
        let visible = area.height as usize;
        if visible == 0 || self.view.is_empty() {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible {
            self.offset = self.selected + 1 - visible;
        }

        let size_width = 10usize;
        let name_width = (area.width as usize).saturating_sub(size_width + 4);
        let ellipsis = theme::glyphs(self.caps).ellipsis;
        for slot in 0..visible {
            let view_index = self.offset + slot;
            let Some(&sheet) = self.view.get(view_index) else {
                break;
            };
            let (label, size) = &self.sheets[sheet];
            let y = area.y + slot as u16;
            let shown = theme::fit_middle(label, name_width, ellipsis);
            let human = format_size(*size, DECIMAL);
            let mut spans = Vec::new();
            // Content-only hits get a dim `~` so it's clear why they matched.
            if self.content_hits.contains(&sheet) {
                spans.push(Span::styled("~ ".to_string(), theme::dim()));
            } else {
                spans.push(Span::raw("  "));
            }
            push_path(&mut spans, &shown, name_width, self.caps);
            spans.push(Span::raw("  "));
            spans.push(Span::styled(format!("{human:>size_width$}"), theme::dim()));
            frame
                .buffer_mut()
                .set_line(area.x, y, &Line::from(spans), area.width);
            if view_index == self.selected {
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

    fn render_bottom(&self, frame: &mut Frame, area: Rect) {
        let line = if self.filtering {
            Line::from(vec![
                Span::styled("/", theme::accent()),
                Span::raw(self.filter.clone()),
                Span::styled("▏", theme::dim()),
            ])
        } else if self.view.is_empty() && !self.filter.is_empty() {
            Line::from(Span::styled(
                format!("no datasheets match \"{}\"  ·  esc clears", self.filter),
                theme::dim(),
            ))
        } else {
            let hint = if self.caps.unicode {
                "↑↓ move   / filter names + contents   ↵ open   q quit"
            } else {
                "up/dn move   / filter names + contents   enter open   q quit"
            };
            Line::from(Span::styled(hint.to_string(), theme::dim()))
        };
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &line, area.width);
    }
}

/// Render a path with the directory dimmed and the file name emphasized.
fn push_path(spans: &mut Vec<Span<'static>>, shown: &str, width: usize, caps: Caps) {
    let padded = format!("{shown:<width$}");
    if !caps.color {
        spans.push(Span::raw(padded));
        return;
    }
    match shown.rfind('/') {
        Some(slash) => {
            let (dir, file) = padded.split_at(slash + 1);
            spans.push(Span::styled(dir.to_string(), theme::dim()));
            spans.push(Span::raw(file.to_string()));
        }
        None => spans.push(Span::raw(padded)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::datasheet::{
        ContentSearchResult, IndexProgress, Loc, SheetData, SheetSource,
    };
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;

    struct PickerMock {
        sheets: Vec<(String, u64)>,
        indexed: Mutex<usize>,
        done: Mutex<bool>,
        /// Searchable universe: `(sheet, term)` pairs covering column names and
        /// cell values (a miniature of the workspace index + column map).
        universe: Mutex<Vec<(u32, String)>>,
        seq: Mutex<u64>,
        pending: Mutex<Option<ContentSearchResult>>,
    }

    impl PickerMock {
        fn new(labels: &[&str], universe: Vec<(u32, &str)>) -> Self {
            Self {
                sheets: labels
                    .iter()
                    .map(|label| ((*label).to_string(), 1))
                    .collect(),
                indexed: Mutex::new(labels.len()),
                done: Mutex::new(true),
                universe: Mutex::new(
                    universe
                        .into_iter()
                        .map(|(sheet, term)| (sheet, term.to_string()))
                        .collect(),
                ),
                seq: Mutex::new(0),
                pending: Mutex::new(None),
            }
        }

        /// Pretend the user paused long ago so the debounce is already due.
        fn pause_elapsed(view: &mut SheetPicker) {
            view.last_edit = Instant::now() - Duration::from_secs(60);
        }
    }

    impl SheetSource for PickerMock {
        fn load(&self, _sheet: u32) -> Option<SheetData> {
            None
        }
        fn label(&self, sheet: u32) -> String {
            self.sheets
                .get(sheet as usize)
                .map(|(label, _)| label.clone())
                .unwrap_or_default()
        }
        fn references(&self, _value: &str) -> Vec<Loc> {
            Vec::new()
        }
        fn progress(&self) -> IndexProgress {
            IndexProgress {
                indexed: *self.indexed.lock().unwrap(),
                total: self.sheets.len(),
                done: *self.done.lock().unwrap(),
            }
        }
        fn sheets(&self) -> Vec<(String, u64)> {
            self.sheets.clone()
        }
        fn search_contents(&self, query: String) -> u64 {
            // Immediate inline miniature of the worker: full recall over the
            // universe, best-score-first.
            let universe = self.universe.lock().unwrap();
            let texts = universe
                .iter()
                .map(|(_, term)| term.clone())
                .collect::<Vec<_>>();
            let hits = crate::fuzzy::rank(&query, &texts)
                .into_iter()
                .map(|(index, score)| (universe[index].0, score))
                .collect();
            drop(universe);
            let mut seq = self.seq.lock().unwrap();
            *seq += 1;
            *self.pending.lock().unwrap() = Some((*seq, hits));
            *seq
        }
        fn take_content_results(&self) -> Option<ContentSearchResult> {
            self.pending.lock().unwrap().take()
        }
    }

    fn picker(mock: PickerMock) -> SheetPicker {
        SheetPicker::new(Arc::new(mock), Caps::PLAIN)
    }

    #[test]
    fn name_match_ranks_above_content_match() {
        let mock = Arc::new(PickerMock::new(
            &["weapons", "zzz-armor"],
            vec![(1, "Rarity"), (1, "weapons rack")],
        ));
        let mut view = SheetPicker::new(mock.clone(), Caps::PLAIN);
        view.filter = "weapons".to_string();
        view.recompute();
        assert_eq!(view.view, vec![0], "instant tier is names only");
        assert!(view.content_hits.is_empty());

        PickerMock::pause_elapsed(&mut view);
        view.tick();
        assert_eq!(view.view, vec![0, 1], "name hit first, content hit below");
        assert!(!view.content_hits.contains(&0));
        assert!(view.content_hits.contains(&1), "content hit is marked");
    }

    #[test]
    fn value_match_streams_in_after_a_pause() {
        let mock = Arc::new(PickerMock::new(
            &["weapons", "zzz-armor"],
            vec![(1, "plasma longsword of the seventh dawn")],
        ));
        let mut view = SheetPicker::new(mock.clone(), Caps::PLAIN);
        view.filter = "seventh dawn".to_string();
        view.recompute();
        assert!(
            view.view.is_empty(),
            "no name match — value tier must find it"
        );

        PickerMock::pause_elapsed(&mut view);
        view.tick();
        assert_eq!(view.view, vec![1], "background value hit streamed in");
        assert!(view.content_hits.contains(&1));
        assert!(
            !view.bg_outstanding(),
            "applied results clear the searching state"
        );
    }

    #[test]
    fn background_search_waits_for_a_pause_in_typing() {
        let mock = Arc::new(PickerMock::new(&["weapons"], vec![(0, "plasma longsword")]));
        let mut view = SheetPicker::new(mock.clone(), Caps::PLAIN);
        view.filter = "plasma".to_string();
        view.last_edit = Instant::now();
        view.tick();
        assert_eq!(view.bg_gen, 0, "no request while still typing");
        assert!(!view.bg_outstanding());

        PickerMock::pause_elapsed(&mut view);
        view.tick();
        assert_ne!(view.bg_gen, 0, "request fires after the pause");
        assert_eq!(view.view, vec![0], "mock answers immediately");
    }

    #[test]
    fn stale_background_results_are_ignored() {
        let mock = Arc::new(PickerMock::new(
            &["weapons", "zzz-armor"],
            vec![(0, "plasma longsword"), (1, "plasma shield")],
        ));
        let mut view = SheetPicker::new(mock.clone(), Caps::PLAIN);
        view.filter = "plasma longsword".to_string();
        PickerMock::pause_elapsed(&mut view);
        view.tick();
        assert_eq!(view.view, vec![0]);

        // The user kept typing; a late arrival for the old query must lose.
        view.filter = "plasma shield".to_string();
        PickerMock::pause_elapsed(&mut view);
        view.tick();
        *mock.pending.lock().unwrap() = Some((1, vec![(0, 200)]));
        view.tick();
        assert_eq!(
            view.view,
            vec![1],
            "stale generation-1 hit for sheet 0 is ignored"
        );
    }

    #[test]
    fn index_advance_refires_a_live_filter() {
        let mock = Arc::new(PickerMock::new(
            &["weapons", "zzz-armor"],
            vec![(0, "plasma longsword")],
        ));
        let mut view = SheetPicker::new(mock.clone(), Caps::PLAIN);
        view.filter = "plasma".to_string();
        PickerMock::pause_elapsed(&mut view);
        view.tick();
        assert_eq!(view.view, vec![0]);
        let first_gen = view.bg_gen;

        // A newly indexed sheet joins the universe; the live filter re-fires.
        mock.universe
            .lock()
            .unwrap()
            .push((1, "plasma shield".to_string()));
        *mock.indexed.lock().unwrap() += 1;
        view.tick();
        assert_eq!(view.bg_gen, first_gen + 1, "index growth re-requests");
        let mut listed = view.view.clone();
        listed.sort_unstable();
        assert_eq!(listed, vec![0, 1], "both sheets stream in");
    }

    #[test]
    fn empty_filter_lists_everything_unmarked() {
        let mock = PickerMock::new(&["b-sheet", "a-sheet"], vec![(0, "Damage")]);
        let mut view = picker(mock);
        view.filter = "damage".to_string();
        view.recompute();
        assert!(
            view.view.is_empty(),
            "instant tier is names only — content arrives via tick"
        );
        PickerMock::pause_elapsed(&mut view);
        view.tick();
        assert_eq!(view.view, vec![0]);
        view.filter.clear();
        view.recompute();
        assert_eq!(view.view, vec![0, 1], "discovery order, not rank order");
        assert!(view.content_hits.is_empty(), "markers cleared");
    }

    #[test]
    fn picker_settles_once_index_reports_done() {
        let mock = Arc::new(PickerMock::new(&["weapons"], vec![]));
        *mock.done.lock().unwrap() = false;
        let mut view = SheetPicker::new(mock.clone(), Caps::PLAIN);
        assert!(view.ticking());
        *mock.done.lock().unwrap() = true;
        view.tick();
        assert!(
            !view.ticking(),
            "one final pull after done, then the picker rests"
        );
    }
}
