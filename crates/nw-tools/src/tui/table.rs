//! A scrollable, filterable, sortable browser over the shared [`ui::Table`]
//! model. The same table that prints as a static report drives this view, so
//! there is a single source of truth for columns and cell styling.

use std::cmp::Ordering;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex, MutexGuard};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::app::{Flow, View};
use crate::fuzzy;
use crate::ui::theme::{self, Caps};
use crate::ui::{Cell, Table};

/// Background applied to the selected row when color is available.
const HILITE: ratatui::style::Color = ratatui::style::Color::Indexed(238);

/// A growing set of table rows, filled by a background scan while the browser is
/// already on screen. Lets row-listing browsers open instantly and stream their
/// results in instead of blocking on the full scan first.
pub struct RowFeed {
    rows: Mutex<Vec<Vec<Cell>>>,
    scanned: AtomicUsize,
    total: usize,
}

impl RowFeed {
    /// A feed awaiting `total` units of work (e.g. one per pak).
    #[must_use]
    pub fn new(total: usize) -> Arc<Self> {
        Arc::new(Self {
            rows: Mutex::new(Vec::new()),
            scanned: AtomicUsize::new(0),
            total,
        })
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Vec<Cell>>> {
        self.rows.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Append a batch of fully-built rows.
    pub fn extend(&self, rows: Vec<Vec<Cell>>) {
        self.lock().extend(rows);
    }

    /// Record one more unit of work (e.g. one pak) finished.
    pub fn mark_done(&self) {
        self.scanned.fetch_add(1, AtomicOrdering::Relaxed);
    }

    fn tail_from(&self, have: usize) -> Vec<Vec<Cell>> {
        let rows = self.lock();
        rows.get(have..).map(<[Vec<Cell>]>::to_vec).unwrap_or_default()
    }

    fn progress(&self) -> (usize, usize) {
        (self.scanned.load(AtomicOrdering::Relaxed), self.total)
    }

    fn scanning(&self) -> bool {
        self.scanned.load(AtomicOrdering::Relaxed) < self.total
    }
}

pub struct TableView {
    title: String,
    stats: Vec<(String, String)>,
    table: Table,
    caps: Caps,
    /// Column whose value is printed to stdout when the user presses Enter.
    primary_col: usize,

    view: Vec<usize>,
    selected: usize,
    offset: usize,

    filter: String,
    filtering: bool,
    sort_col: usize,
    sorted: bool,
    sort_desc: bool,
    help: bool,
    result: Option<String>,
    /// Background scan streaming rows in; `None` for an already-complete table.
    feed: Option<Arc<RowFeed>>,
    /// Rows already pulled from `feed` into `table`.
    ingested: usize,
}

impl TableView {
    pub fn new(
        title: impl Into<String>,
        stats: Vec<(String, String)>,
        table: Table,
        primary_col: usize,
        caps: Caps,
    ) -> Self {
        let ingested = table.rows().len();
        let view = (0..ingested).collect();
        Self {
            title: title.into(),
            stats,
            table,
            caps,
            primary_col,
            view,
            selected: 0,
            offset: 0,
            filter: String::new(),
            filtering: false,
            sort_col: 0,
            sorted: false,
            sort_desc: false,
            help: false,
            result: None,
            feed: None,
            ingested,
        }
    }

    /// A browser whose rows stream in from `feed` (which a background scan fills).
    /// `table` is an empty template carrying the headers and column alignment.
    pub fn streaming(
        title: impl Into<String>,
        stats: Vec<(String, String)>,
        table: Table,
        primary_col: usize,
        feed: Arc<RowFeed>,
        caps: Caps,
    ) -> Self {
        let mut view = Self::new(title, stats, table, primary_col, caps);
        view.feed = Some(feed);
        view
    }

    /// Pull rows discovered since the last ingest into the table and reapply the
    /// current filter/sort. Returns whether anything new arrived.
    fn ingest(&mut self) -> bool {
        let Some(feed) = self.feed.clone() else {
            return false;
        };
        let fresh = feed.tail_from(self.ingested);
        if fresh.is_empty() {
            return false;
        }
        self.ingested += fresh.len();
        for row in fresh {
            self.table.push(row);
        }
        self.recompute();
        true
    }

    fn cell_text(&self, row: usize, col: usize) -> &str {
        self.table
            .rows()
            .get(row)
            .and_then(|cells| cells.get(col))
            .map_or("", |cell| cell.raw())
    }

    /// Everything searchable on a row, joined for fuzzy matching.
    fn row_haystack(&self, row: usize) -> String {
        self.table
            .rows()
            .get(row)
            .into_iter()
            .flatten()
            .map(|cell| cell.raw())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn cmp_rows(&self, a: usize, b: usize) -> Ordering {
        let left = self.cell_text(a, self.sort_col);
        let right = self.cell_text(b, self.sort_col);
        let ord = match (left.parse::<f64>(), right.parse::<f64>()) {
            (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => left.cmp(right),
        };
        if self.sort_desc { ord.reverse() } else { ord }
    }

    fn recompute(&mut self) {
        let anchor = self.view.get(self.selected).copied();
        if self.filter.is_empty() {
            self.view = (0..self.table.rows().len()).collect();
        } else {
            // frizbee returns matches already ranked best-first; an explicit
            // column sort (below) overrides that ordering when active.
            let haystacks = (0..self.table.rows().len())
                .map(|row| self.row_haystack(row))
                .collect::<Vec<_>>();
            self.view = fuzzy::rank(&self.filter, &haystacks)
                .into_iter()
                .map(|(row, _)| row)
                .collect();
        }
        if self.sorted {
            let mut view = std::mem::take(&mut self.view);
            view.sort_by(|&a, &b| self.cmp_rows(a, b));
            self.view = view;
        }
        // Keep the cursor on the same underlying row where possible.
        self.selected = anchor
            .and_then(|row| self.view.iter().position(|&candidate| candidate == row))
            .unwrap_or(0);
        self.clamp();
    }

    fn clamp(&mut self) {
        if self.view.is_empty() {
            self.selected = 0;
            self.offset = 0;
            return;
        }
        self.selected = self.selected.min(self.view.len() - 1);
    }

    fn move_by(&mut self, delta: isize) {
        if self.view.is_empty() {
            return;
        }
        let last = (self.view.len() - 1) as isize;
        let next = (self.selected as isize)
            .saturating_add(delta)
            .clamp(0, last);
        self.selected = next as usize;
    }

    fn move_to_end(&mut self) {
        self.selected = self.view.len().saturating_sub(1);
    }

    fn select(&mut self) {
        if let Some(&row) = self.view.get(self.selected) {
            self.result = Some(self.cell_text(row, self.primary_col).to_string());
        }
    }

    fn cycle_sort(&mut self) {
        let columns = self.table.headers().len().max(1);
        self.sort_col = if self.sorted {
            (self.sort_col + 1) % columns
        } else {
            self.sort_col
        };
        self.sorted = true;
        self.sort_desc = false;
        self.recompute();
    }

    fn reverse_sort(&mut self) {
        if !self.sorted {
            self.sorted = true;
        } else {
            self.sort_desc = !self.sort_desc;
        }
        self.recompute();
    }
}

impl View for TableView {
    fn take_result(&mut self) -> Option<String> {
        self.result.take()
    }

    fn ticking(&self) -> bool {
        self.feed.as_ref().is_some_and(|feed| feed.scanning())
    }

    fn tick(&mut self) {
        self.ingest();
    }

    fn on_key(&mut self, key: KeyEvent) -> Flow {
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

        let page = 10;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Flow::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Flow::Quit;
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_by(page);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_by(-page);
            }
            KeyCode::PageDown => self.move_by(page),
            KeyCode::PageUp => self.move_by(-page),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.move_to_end(),
            KeyCode::Char('/') => {
                self.filtering = true;
                self.help = false;
            }
            KeyCode::Char('s') => self.cycle_sort(),
            KeyCode::Char('r') => self.reverse_sort(),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Enter => {
                self.select();
                return Flow::Quit;
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
        frame.buffer_mut().set_line(
            divider.x,
            divider.y,
            &Line::from(Span::styled(
                std::iter::repeat_n('─', divider.width as usize).collect::<String>(),
                theme::dim(),
            )),
            divider.width,
        );
        self.render_body(frame, body);
        self.render_bottom(frame, bottom);
        if self.help {
            self.render_help(frame, area);
        }
    }
}

impl TableView {
    fn render_top(&self, frame: &mut Frame, area: Rect) {
        let glyphs = theme::glyphs(self.caps);
        let mut spans = vec![Span::styled(self.title.clone(), theme::accent())];
        if !self.stats.is_empty() {
            spans.push(Span::raw("   "));
            for (index, (label, value)) in self.stats.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::styled(glyphs.sep.to_string(), theme::dim()));
                }
                spans.push(Span::styled(format!("{label} "), theme::dim()));
                spans.push(Span::styled(value.clone(), theme::bold()));
            }
        }
        let buf = frame.buffer_mut();
        buf.set_line(area.x, area.y, &Line::from(spans), area.width);

        let right = self.right_indicator();
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

    fn right_indicator(&self) -> String {
        let position = if self.view.is_empty() {
            "0/0".to_string()
        } else {
            format!("{}/{}", self.selected + 1, self.view.len())
        };
        if let Some((scanned, total)) = self.feed.as_ref().map(|feed| feed.progress())
            && scanned < total
        {
            return format!("scanning {scanned}/{total}   {position}");
        }
        if self.sorted {
            let column = self
                .table
                .headers()
                .get(self.sort_col)
                .map_or("", String::as_str);
            let arrow = if self.sort_desc { "v" } else { "^" };
            format!("sort {column}{arrow}   {position}")
        } else {
            position
        }
    }

    fn render_body(&mut self, frame: &mut Frame, area: Rect) {
        let inner_width = area.width as usize;
        let widths = self.table.column_widths(inner_width);

        let buf = frame.buffer_mut();
        buf.set_line(
            area.x,
            area.y,
            &self.table.header_line(&widths, self.caps),
            area.width,
        );
        buf.set_line(
            area.x,
            area.y + 1,
            &self.table.rule_line(&widths, self.caps),
            area.width,
        );

        let visible = area.height.saturating_sub(2) as usize;
        if visible == 0 || self.view.is_empty() {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible {
            self.offset = self.selected + 1 - visible;
        }

        let rows = self.table.rows();
        for slot in 0..visible {
            let view_index = self.offset + slot;
            let Some(&row_index) = self.view.get(view_index) else {
                break;
            };
            let y = area.y + 2 + slot as u16;
            let line = self.table.data_line(&rows[row_index], &widths, self.caps);
            frame.buffer_mut().set_line(area.x, y, &line, area.width);
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
                format!("no rows match \"{}\"  ·  / edit   esc clear", self.filter),
                theme::dim(),
            ))
        } else {
            Line::from(Span::styled(self.hint(), theme::dim()))
        };
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &line, area.width);
    }

    fn hint(&self) -> String {
        if self.caps.unicode {
            "↑↓ move   / filter   s sort   r reverse   ↵ select   ? help   q quit".to_string()
        } else {
            "up/dn move   / filter   s sort   r reverse   enter select   ? help   q quit"
                .to_string()
        }
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let keys = [
            ("↑ ↓ / j k", "move selection"),
            ("PgUp PgDn / ^u ^d", "page"),
            ("g G / Home End", "jump to top / bottom"),
            ("/", "fuzzy filter (incremental); esc clears"),
            ("s", "cycle sort column"),
            ("r", "reverse sort"),
            ("↵ enter", "select row (prints value, exits)"),
            ("q esc", "quit"),
        ];
        let width = 52u16.min(area.width);
        let height = (keys.len() as u16 + 2).min(area.height);
        let popup = centered(area, width, height);

        let mut lines = Vec::with_capacity(keys.len());
        for (key, desc) in keys {
            lines.push(Line::from(vec![
                Span::styled(format!(" {key:<19}"), theme::accent()),
                Span::styled(desc.to_string(), theme::dim()),
            ]));
        }

        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" keys ")
                    .border_style(theme::dim()),
            ),
            popup,
        );
    }
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
    use crate::ui::Cell;

    fn sample() -> TableView {
        let mut table = Table::new(["Name", "Count"]).right([1]);
        table.push([Cell::path("b/two.slice"), Cell::text("20")]);
        table.push([Cell::path("a/one.slice"), Cell::text("100")]);
        table.push([Cell::path("c/three.dds"), Cell::text("3")]);
        TableView::new("t", Vec::new(), table, 0, Caps::PLAIN)
    }

    fn press(view: &mut TableView, code: KeyCode) -> Flow {
        view.on_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn typed(view: &mut TableView, text: &str) {
        for c in text.chars() {
            press(view, KeyCode::Char(c));
        }
    }

    #[test]
    fn navigation_clamps_within_bounds() {
        let mut view = sample();
        assert_eq!(view.selected, 0);
        press(&mut view, KeyCode::Char('j'));
        assert_eq!(view.selected, 1);
        press(&mut view, KeyCode::Char('G'));
        assert_eq!(view.selected, 2);
        press(&mut view, KeyCode::Char('j'));
        assert_eq!(view.selected, 2, "cannot move past the last row");
        press(&mut view, KeyCode::Char('g'));
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn incremental_filter_then_clear() {
        let mut view = sample();
        press(&mut view, KeyCode::Char('/'));
        typed(&mut view, "slice");
        assert_eq!(view.view.len(), 2);
        press(&mut view, KeyCode::Backspace);
        // "slic" still matches the two .slice rows
        assert_eq!(view.view.len(), 2);
        press(&mut view, KeyCode::Esc);
        assert_eq!(view.view.len(), 3, "esc clears the filter");
        assert!(!view.filtering);
    }

    #[test]
    fn sort_and_reverse_by_column() {
        let mut view = sample();
        press(&mut view, KeyCode::Char('s')); // sort by Name asc
        let first = view.cell_text(view.view[0], 0).to_string();
        assert_eq!(first, "a/one.slice");
        press(&mut view, KeyCode::Char('r')); // reverse
        let first = view.cell_text(view.view[0], 0).to_string();
        assert_eq!(first, "c/three.dds");
    }

    #[test]
    fn enter_records_primary_column_selection() {
        let mut view = sample();
        press(&mut view, KeyCode::Char('j'));
        assert_eq!(press(&mut view, KeyCode::Enter), Flow::Quit);
        assert_eq!(view.take_result().as_deref(), Some("a/one.slice"));
    }

    #[test]
    fn numeric_column_sorts_numerically() {
        let mut view = sample();
        // Name col 0 first, advance to col 1 (Count) and sort.
        press(&mut view, KeyCode::Char('s'));
        press(&mut view, KeyCode::Char('s'));
        let order: Vec<&str> = view.view.iter().map(|&r| view.cell_text(r, 1)).collect();
        assert_eq!(order, ["3", "20", "100"], "3 < 20 < 100 numerically");
    }
}
