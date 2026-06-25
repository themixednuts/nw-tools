//! A collapsible tree viewer for an ObjectStream DOM. Callers hand in a flat
//! DFS list of [`TreeNode`]s (depth-tagged); the view derives parent links,
//! handles expand/collapse, scrolling, and incremental search-jump.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::app::{Flow, View};
use crate::ui::theme::{self, Caps};

const HILITE: Color = Color::Indexed(238);

/// One node in a depth-first flattening of the DOM.
pub struct TreeNode {
    pub depth: usize,
    pub children: usize,
    pub type_name: String,
    pub field: String,
    pub meta: String,
    /// The element's type UUID could not be resolved to a name.
    pub unresolved_type: bool,
    /// The element's field name could not be resolved (CRC only).
    pub unresolved_field: bool,
}

struct Node {
    depth: usize,
    parent: Option<usize>,
    children: usize,
    collapsed: bool,
    type_name: String,
    field: String,
    meta: String,
    unresolved_type: bool,
    unresolved_field: bool,
    search: String,
}

pub struct TreeView {
    title: String,
    nodes: Vec<Node>,
    caps: Caps,
    visible: Vec<usize>,
    selected: usize,
    offset: usize,
    search: String,
    searching: bool,
    help: bool,
}

impl TreeView {
    pub fn new(title: impl Into<String>, input: Vec<TreeNode>, caps: Caps) -> Self {
        // Derive each node's parent from the depth sequence (DFS order).
        let mut stack: Vec<usize> = Vec::new();
        let mut nodes = Vec::with_capacity(input.len());
        for (index, node) in input.into_iter().enumerate() {
            stack.truncate(node.depth);
            let parent = stack.last().copied();
            stack.push(index);
            let search =
                format!("{} {} {}", node.type_name, node.field, node.meta).to_ascii_lowercase();
            nodes.push(Node {
                depth: node.depth,
                parent,
                children: node.children,
                collapsed: false,
                type_name: node.type_name,
                field: node.field,
                meta: node.meta,
                unresolved_type: node.unresolved_type,
                unresolved_field: node.unresolved_field,
                search,
            });
        }
        let mut view = Self {
            title: title.into(),
            nodes,
            caps,
            visible: Vec::new(),
            selected: 0,
            offset: 0,
            search: String::new(),
            searching: false,
            help: false,
        };
        view.rebuild();
        view
    }

    fn rebuild(&mut self) {
        let anchor = self.visible.get(self.selected).copied();
        self.visible.clear();
        let mut skip_below: Option<usize> = None;
        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(depth) = skip_below {
                if node.depth > depth {
                    continue;
                }
                skip_below = None;
            }
            self.visible.push(index);
            if node.collapsed && node.children > 0 {
                skip_below = Some(node.depth);
            }
        }
        self.selected = anchor
            .and_then(|node| self.visible.iter().position(|&candidate| candidate == node))
            .unwrap_or(self.selected)
            .min(self.visible.len().saturating_sub(1));
    }

    fn current(&self) -> Option<usize> {
        self.visible.get(self.selected).copied()
    }

    fn move_by(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let last = (self.visible.len() - 1) as isize;
        self.selected = (self.selected as isize)
            .saturating_add(delta)
            .clamp(0, last) as usize;
    }

    fn set_collapsed(&mut self, collapsed: bool) {
        if let Some(node) = self.current()
            && self.nodes[node].children > 0
        {
            self.nodes[node].collapsed = collapsed;
            self.rebuild();
        }
    }

    fn toggle(&mut self) {
        if let Some(node) = self.current()
            && self.nodes[node].children > 0
        {
            self.nodes[node].collapsed = !self.nodes[node].collapsed;
            self.rebuild();
        }
    }

    /// Seed the search with the selected node's type and jump to the next one,
    /// so `n` continues cycling elements of the same type.
    fn search_same_type(&mut self) {
        if let Some(node) = self.current() {
            self.search = self.nodes[node].type_name.clone();
            self.jump_to_match();
        }
    }

    fn jump_to_match(&mut self) {
        let needle = self.search.to_ascii_lowercase();
        if needle.is_empty() {
            return;
        }
        let start = self.current().map_or(0, |node| node + 1);
        let order = (start..self.nodes.len()).chain(0..start);
        let Some(target) = order
            .into_iter()
            .find(|&node| self.nodes[node].search.contains(&needle))
        else {
            return;
        };
        // Expand every ancestor so the match is visible.
        let mut ancestor = self.nodes[target].parent;
        while let Some(index) = ancestor {
            self.nodes[index].collapsed = false;
            ancestor = self.nodes[index].parent;
        }
        self.rebuild();
        if let Some(position) = self.visible.iter().position(|&node| node == target) {
            self.selected = position;
        }
    }
}

impl View for TreeView {
    fn on_key(&mut self, key: KeyEvent) -> Flow {
        if self.help {
            self.help = false;
            return Flow::Continue;
        }
        if self.searching {
            match key.code {
                KeyCode::Esc => {
                    self.search.clear();
                    self.searching = false;
                }
                KeyCode::Enter => self.searching = false,
                KeyCode::Backspace => {
                    self.search.pop();
                    self.jump_to_match();
                }
                KeyCode::Char(c) => {
                    self.search.push(c);
                    self.jump_to_match();
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
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => {
                self.selected = self.visible.len().saturating_sub(1)
            }
            KeyCode::Char('h') | KeyCode::Left => self.set_collapsed(true),
            KeyCode::Char('l') | KeyCode::Right => self.set_collapsed(false),
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle(),
            KeyCode::Char('*') => self.search_same_type(),
            KeyCode::Char('n') => self.jump_to_match(),
            KeyCode::Char('/') => self.searching = true,
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

impl TreeView {
    fn render_top(&self, frame: &mut Frame, area: Rect) {
        let glyphs = theme::glyphs(self.caps);
        let spans = vec![
            Span::styled(self.title.clone(), theme::accent()),
            Span::raw("   "),
            Span::styled("elements ", theme::dim()),
            Span::styled(self.nodes.len().to_string(), theme::bold()),
            Span::styled(glyphs.sep.to_string(), theme::dim()),
            Span::styled("shown ", theme::dim()),
            Span::styled(self.visible.len().to_string(), theme::bold()),
        ];
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &Line::from(spans), area.width);
    }

    fn render_body(&mut self, frame: &mut Frame, area: Rect) {
        let visible = area.height as usize;
        if visible == 0 || self.visible.is_empty() {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible {
            self.offset = self.selected + 1 - visible;
        }

        for slot in 0..visible {
            let row = self.offset + slot;
            let Some(&node_index) = self.visible.get(row) else {
                break;
            };
            let y = area.y + slot as u16;
            let line = self.node_line(node_index);
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

    fn node_line(&self, index: usize) -> Line<'static> {
        let node = &self.nodes[index];
        let marker = if node.children == 0 {
            if self.caps.unicode { "· " } else { "  " }
        } else if node.collapsed {
            if self.caps.unicode { "▸ " } else { "> " }
        } else if self.caps.unicode {
            "▾ "
        } else {
            "v "
        };

        let type_style = if node.unresolved_type {
            theme::bad()
        } else {
            theme::accent()
        };
        let mut spans = vec![
            Span::raw("  ".repeat(node.depth)),
            Span::styled(marker.to_string(), theme::dim()),
            Span::styled(node.type_name.clone(), type_style),
        ];
        if !node.field.is_empty() {
            let field_style = if node.unresolved_field {
                theme::warn()
            } else {
                theme::good()
            };
            spans.push(Span::styled(format!(" .{}", node.field), field_style));
        }
        if node.collapsed && node.children > 0 {
            spans.push(Span::styled(format!("  ({})", node.children), theme::dim()));
        }
        if !node.meta.is_empty() {
            spans.push(Span::styled(format!("   {}", node.meta), theme::dim()));
        }
        Line::from(spans)
    }

    fn render_bottom(&self, frame: &mut Frame, area: Rect) {
        let line = if self.searching {
            Line::from(vec![
                Span::styled("/", theme::accent()),
                Span::raw(self.search.clone()),
                Span::styled("▏", theme::dim()),
            ])
        } else {
            let hint = if self.caps.unicode {
                "↑↓ move   ←→ collapse   ↵ toggle   * same-type   / search (n)   ? help   q quit"
            } else {
                "up/dn move   l/r collapse   enter toggle   * same-type   / search (n)   ? help   q"
            };
            Line::from(Span::styled(hint.to_string(), theme::dim()))
        };
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &line, area.width);
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let keys = [
            ("↑ ↓ / j k", "move"),
            ("← → / h l", "collapse / expand"),
            ("↵ space", "toggle node"),
            ("*", "jump between elements of the same type"),
            ("/  then  n", "search, jump to next match"),
            ("g G", "first / last"),
            ("q esc", "quit"),
        ];
        let width = 50u16.min(area.width);
        let height = (keys.len() as u16 + 2).min(area.height);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let popup = Rect {
            x,
            y,
            width,
            height,
        };

        let lines = keys
            .iter()
            .map(|(key, desc)| {
                Line::from(vec![
                    Span::styled(format!(" {key:<16}"), theme::accent()),
                    Span::styled((*desc).to_string(), theme::dim()),
                ])
            })
            .collect::<Vec<_>>();
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
