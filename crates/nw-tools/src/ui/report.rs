//! The shared output model. A [`Report`] is a title, a stat strip, and a
//! sequence of blocks (tables, notes, sections, key/value lines). The same
//! [`Table`] model feeds both the static printer ([`super::print`]) and the
//! interactive browsers ([`crate::tui`]).

use ratatui::text::{Line, Span};

use super::theme::{self, Caps};

/// Column alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Right,
}

/// How a cell's text is styled when color is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    Dim,
    Path,
    Size,
    Method,
    Good,
}

/// A single table cell: plain text plus a semantic style hint.
#[derive(Debug, Clone)]
pub struct Cell {
    text: String,
    kind: Kind,
}

impl Cell {
    fn new(text: impl Into<String>, kind: Kind) -> Self {
        Self {
            text: theme::clean(&text.into()),
            kind,
        }
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self::new(value, Kind::Plain)
    }

    pub fn dim(value: impl Into<String>) -> Self {
        Self::new(value, Kind::Dim)
    }

    /// A filesystem or archive path; the directory part is dimmed and the file
    /// name is emphasized.
    pub fn path(value: impl Into<String>) -> Self {
        Self::new(value, Kind::Path)
    }

    /// A pre-formatted human size (e.g. `14.2 GB`).
    pub fn size(value: impl Into<String>) -> Self {
        Self::new(value, Kind::Size)
    }

    /// A CryPak compression method label.
    pub fn method(value: impl Into<String>) -> Self {
        Self::new(value, Kind::Method)
    }

    pub fn good(value: impl Into<String>) -> Self {
        Self::new(value, Kind::Good)
    }

    /// `yes` in green, otherwise a dim dash.
    pub fn yes_no(value: bool) -> Self {
        if value {
            Self::good("yes")
        } else {
            Self::dim("-")
        }
    }

    pub fn raw(&self) -> &str {
        &self.text
    }

    fn measure(&self) -> usize {
        theme::display_width(&self.text)
    }

    /// Render to styled spans clipped/padded to `width`.
    fn render(&self, width: usize, align: Align, caps: Caps) -> Vec<Span<'static>> {
        let glyphs = theme::glyphs(caps);
        let shown = match self.kind {
            Kind::Path => theme::fit_middle(&self.text, width, glyphs.ellipsis),
            _ => theme::fit_end(&self.text, width, glyphs.ellipsis),
        };
        let pad = width.saturating_sub(theme::display_width(&shown));

        let mut spans = Vec::new();
        if align == Align::Right && pad > 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
        self.push_styled(&mut spans, shown, caps);
        if align == Align::Left && pad > 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
        spans
    }

    fn push_styled(&self, spans: &mut Vec<Span<'static>>, shown: String, caps: Caps) {
        if !caps.color {
            spans.push(Span::raw(shown));
            return;
        }
        match self.kind {
            Kind::Plain => spans.push(Span::raw(shown)),
            Kind::Dim => spans.push(Span::styled(shown, theme::dim())),
            Kind::Good => spans.push(Span::styled(shown, theme::good())),
            Kind::Method => spans.push(Span::styled(shown.clone(), theme::method_style(&shown))),
            Kind::Path => push_path(spans, shown),
            Kind::Size => push_size(spans, shown),
        }
    }
}

fn push_path(spans: &mut Vec<Span<'static>>, shown: String) {
    match shown.rfind('/') {
        Some(slash) => {
            let (dir, file) = shown.split_at(slash + 1);
            spans.push(Span::styled(dir.to_string(), theme::dim()));
            if !file.is_empty() {
                spans.push(Span::raw(file.to_string()));
            }
        }
        None => spans.push(Span::raw(shown)),
    }
}

fn push_size(spans: &mut Vec<Span<'static>>, shown: String) {
    match shown.rsplit_once(' ') {
        Some((number, unit)) => {
            spans.push(Span::raw(format!("{number} ")));
            spans.push(Span::styled(unit.to_string(), theme::dim()));
        }
        None => spans.push(Span::raw(shown)),
    }
}

/// A borderless table: an accent header row over a dim rule, then data rows.
#[derive(Debug, Clone)]
pub struct Table {
    headers: Vec<String>,
    aligns: Vec<Align>,
    rows: Vec<Vec<Cell>>,
}

impl Table {
    pub fn new<H: Into<String>>(headers: impl IntoIterator<Item = H>) -> Self {
        let headers = headers.into_iter().map(Into::into).collect::<Vec<_>>();
        let aligns = vec![Align::Left; headers.len()];
        Self {
            headers,
            aligns,
            rows: Vec::new(),
        }
    }

    /// Right-align the given column indices (typically numeric columns).
    #[must_use]
    pub fn right(mut self, columns: impl IntoIterator<Item = usize>) -> Self {
        for column in columns {
            if let Some(align) = self.aligns.get_mut(column) {
                *align = Align::Right;
            }
        }
        self
    }

    pub fn push(&mut self, cells: impl IntoIterator<Item = Cell>) {
        self.rows.push(cells.into_iter().collect());
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(crate) fn headers(&self) -> &[String] {
        &self.headers
    }

    pub(crate) fn rows(&self) -> &[Vec<Cell>] {
        &self.rows
    }

    /// Column widths fitted to `limit`, accounting for the gutter and gaps.
    pub(crate) fn column_widths(&self, limit: usize) -> Vec<usize> {
        let columns = self
            .headers
            .len()
            .max(self.rows.iter().map(Vec::len).max().unwrap_or(0));
        let mut widths = vec![0usize; columns];
        for (index, header) in self.headers.iter().enumerate() {
            widths[index] = widths[index].max(theme::display_width(header));
        }
        for row in &self.rows {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(cell.measure());
            }
        }

        let gaps = GUTTER + columns.saturating_sub(1) * GAP;
        let available = limit.saturating_sub(gaps).max(columns);
        theme::fit_widths(widths, available)
    }

    /// Render the whole table to styled lines.
    pub(crate) fn render(&self, limit: usize, caps: Caps) -> Vec<Line<'static>> {
        let widths = self.column_widths(limit);
        let mut lines = Vec::with_capacity(self.rows.len() + 2);
        lines.push(self.header_line(&widths, caps));
        lines.push(self.rule_line(&widths, caps));
        for row in &self.rows {
            lines.push(self.data_line(row, &widths, caps));
        }
        lines
    }

    pub(crate) fn header_line(&self, widths: &[usize], caps: Caps) -> Line<'static> {
        let mut spans = vec![gutter()];
        for (index, width) in widths.iter().enumerate() {
            if index > 0 {
                spans.push(gap());
            }
            let header = self.headers.get(index).map_or("", String::as_str);
            let cell = Cell::new(header, Kind::Plain);
            let mut rendered = cell.render(*width, self.aligns[index], Caps::PLAIN);
            if caps.color {
                for span in &mut rendered {
                    span.style = theme::header();
                }
            }
            spans.extend(rendered);
        }
        Line::from(spans)
    }

    pub(crate) fn rule_line(&self, widths: &[usize], caps: Caps) -> Line<'static> {
        let glyphs = theme::glyphs(caps);
        let mut spans = vec![gutter()];
        for (index, width) in widths.iter().enumerate() {
            if index > 0 {
                spans.push(gap());
            }
            let rule = std::iter::repeat_n(glyphs.rule, *width).collect::<String>();
            if caps.color {
                spans.push(Span::styled(rule, theme::dim()));
            } else {
                spans.push(Span::raw(rule));
            }
        }
        Line::from(spans)
    }

    pub(crate) fn data_line(&self, row: &[Cell], widths: &[usize], caps: Caps) -> Line<'static> {
        let mut spans = vec![gutter()];
        for (index, width) in widths.iter().enumerate() {
            if index > 0 {
                spans.push(gap());
            }
            match row.get(index) {
                Some(cell) => spans.extend(cell.render(*width, self.aligns[index], caps)),
                None => spans.push(Span::raw(" ".repeat(*width))),
            }
        }
        Line::from(spans)
    }
}

const GUTTER: usize = 2;
const GAP: usize = 2;

fn gutter() -> Span<'static> {
    Span::raw(" ".repeat(GUTTER))
}

fn gap() -> Span<'static> {
    Span::raw(" ".repeat(GAP))
}

/// A block within a report body.
#[derive(Debug, Clone)]
pub(crate) enum Block {
    Table(Table),
    Line(Line<'static>),
    Blank,
}

/// A complete command report: header band plus body blocks.
#[derive(Debug, Clone, Default)]
pub struct Report {
    title: Option<String>,
    stats: Vec<(String, String)>,
    blocks: Vec<Block>,
}

impl Report {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            stats: Vec::new(),
            blocks: Vec::new(),
        }
    }

    /// A titled report seeded with a pre-built stat strip.
    pub fn with_stats(title: impl Into<String>, stats: Vec<(String, String)>) -> Self {
        Self {
            title: Some(title.into()),
            stats,
            blocks: Vec::new(),
        }
    }

    /// Add a `label value` chip to the header stat strip.
    #[must_use]
    pub fn stat(mut self, label: impl Into<String>, value: impl ToString) -> Self {
        self.stats.push((label.into(), value.to_string()));
        self
    }

    /// Add a table, preceded by a blank line for breathing room.
    pub fn table(&mut self, table: Table) -> &mut Self {
        if !self.blocks.is_empty() {
            self.blocks.push(Block::Blank);
        }
        self.blocks.push(Block::Table(table));
        self
    }

    /// Push a table, or a dim note when it has no rows.
    pub fn table_or(&mut self, table: Table, empty: impl Into<String>) -> &mut Self {
        if table.is_empty() {
            self.note(empty)
        } else {
            self.table(table)
        }
    }

    /// A dim, informational line (e.g. `no entries`, `… 12 more`).
    pub fn note(&mut self, text: impl Into<String>) -> &mut Self {
        let line = Line::from(Span::styled(text.into(), theme::dim()));
        self.blocks.push(Block::Line(line));
        self
    }

    /// A dim `… N more <noun>` overflow footer using the active ellipsis glyph.
    pub fn more(&mut self, count: impl ToString, noun: &str) -> &mut Self {
        let glyphs = theme::glyphs(theme::caps());
        self.note(format!(
            "{} {} more {noun}",
            glyphs.ellipsis,
            count.to_string()
        ))
    }

    /// An accent sub-heading.
    pub fn section(&mut self, text: impl Into<String>) -> &mut Self {
        self.blocks.push(Block::Blank);
        let line = Line::from(Span::styled(text.into(), theme::accent()));
        self.blocks.push(Block::Line(line));
        self
    }

    /// A `label: value` line under a 2-space gutter, value in dim.
    pub fn kv(&mut self, label: impl Into<String>, value: impl Into<String>) -> &mut Self {
        let line = Line::from(vec![
            Span::raw("  "),
            Span::raw(format!("{}: ", label.into())),
            Span::styled(value.into(), theme::dim()),
        ]);
        self.blocks.push(Block::Line(line));
        self
    }

    pub(crate) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(crate) fn stats(&self) -> &[(String, String)] {
        &self.stats
    }

    pub(crate) fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Render the report to stdout (styled or plain per [`theme::caps`]).
    pub fn print(&self) {
        super::print::print_report(self);
    }
}
