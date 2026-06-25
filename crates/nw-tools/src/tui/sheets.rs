//! A streaming picker over a [`SheetSource`]'s datasheets. The list is pulled
//! from the source on every tick, so it fills in live while the background
//! discovery sweep parses pak archives — the UI is usable from the first frame.

use std::sync::Arc;

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
    /// Set once we've refreshed after discovery reported done, so the final batch
    /// of sheets is never missed before the picker stops ticking.
    synced_done: bool,
}

impl SheetPicker {
    pub fn new(source: Arc<dyn SheetSource>, caps: Caps) -> Self {
        let sheets = source.sheets();
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
        if self.filter.is_empty() {
            self.view = (0..self.sheets.len()).collect();
        } else {
            let haystacks = self
                .sheets
                .iter()
                .map(|(label, _)| label.clone())
                .collect::<Vec<_>>();
            self.view = fuzzy::rank(&self.filter, &haystacks)
                .into_iter()
                .map(|(index, _)| index)
                .collect();
        }
        self.selected = anchor
            .and_then(|row| self.view.iter().position(|&candidate| candidate == row))
            .unwrap_or(0)
            .min(self.view.len().saturating_sub(1));
    }

    fn move_by(&mut self, delta: isize) {
        if self.view.is_empty() {
            return;
        }
        let last = (self.view.len() - 1) as isize;
        self.selected = (self.selected as isize).saturating_add(delta).clamp(0, last) as usize;
    }
}

impl View for SheetPicker {
    fn ticking(&self) -> bool {
        // Keep refreshing while paks are still being discovered, plus one tick
        // after discovery finishes so the final batch is captured.
        !self.source.discovery().done || !self.synced_done
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
    }

    fn on_key(&mut self, key: KeyEvent) -> Flow {
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
            KeyCode::PageDown => self.move_by(page),
            KeyCode::PageUp => self.move_by(-page),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => self.move_by(page),
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
        let name_width = (area.width as usize).saturating_sub(size_width + 2);
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
                "↑↓ move   / fuzzy filter   ↵ open   q quit"
            } else {
                "up/dn move   / fuzzy filter   enter open   q quit"
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
