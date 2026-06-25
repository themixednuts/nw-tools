//! Serialize a [`Report`] of ratatui [`Line`]s to stdout — ANSI when color is
//! enabled, plain text otherwise.

use std::io::{self, Write};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::report::{Block, Report};
use super::theme::{self, Caps};

/// Render `report` to stdout using the resolved process capabilities.
pub fn print_report(report: &Report) {
    let caps = theme::caps();
    let width = theme::report_width(caps);
    let mut out = String::new();

    let mut needs_band_gap = false;
    if let Some(line) = header_band(report, width, caps) {
        write_line(&mut out, &line, caps);
        needs_band_gap = true;
    }
    if needs_band_gap && !report.blocks().is_empty() {
        out.push('\n');
    }

    for block in report.blocks() {
        match block {
            Block::Table(table) => {
                for line in table.render(width, caps) {
                    write_line(&mut out, &line, caps);
                }
            }
            Block::Line(line) => write_line(&mut out, line, caps),
            Block::Blank => out.push('\n'),
        }
    }

    let stdout = io::stdout();
    let mut lock = stdout.lock();
    let _ = lock.write_all(out.as_bytes());
}

/// The header band: bold accent title followed by a dim stat strip.
fn header_band(report: &Report, width: usize, caps: Caps) -> Option<Line<'static>> {
    let title = report.title()?;
    let glyphs = theme::glyphs(caps);
    let mut spans = vec![Span::styled(
        title.to_string(),
        styled(theme::accent(), caps),
    )];

    if !report.stats().is_empty() {
        spans.push(Span::raw("   "));
        for (index, (label, value)) in report.stats().iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(
                    glyphs.sep.to_string(),
                    styled(theme::dim(), caps),
                ));
            }
            spans.push(Span::styled(
                format!("{label} "),
                styled(theme::dim(), caps),
            ));
            spans.push(Span::styled(value.clone(), styled(theme::bold(), caps)));
        }
    }

    Some(clip_line(spans, width))
}

/// Drop styling when color is disabled so headers/notes stay plain.
fn styled(style: Style, caps: Caps) -> Style {
    if caps.color { style } else { Style::default() }
}

/// Keep a header line within the terminal width without breaking mid-escape.
fn clip_line(spans: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let total: usize = spans
        .iter()
        .map(|span| theme::display_width(&span.content))
        .sum();
    if total <= width {
        return Line::from(spans);
    }

    let mut budget = width;
    let mut clipped = Vec::new();
    for span in spans {
        if budget == 0 {
            break;
        }
        let content_width = theme::display_width(&span.content);
        if content_width <= budget {
            budget -= content_width;
            clipped.push(span);
        } else {
            let text = span.content.chars().take(budget).collect::<String>();
            budget = 0;
            clipped.push(Span::styled(text, span.style));
        }
    }
    Line::from(clipped)
}

fn write_line(out: &mut String, line: &Line<'_>, caps: Caps) {
    for span in &line.spans {
        if caps.color {
            let style = span.style.patch(line.style);
            write_span(out, &span.content, style);
        } else {
            out.push_str(&span.content);
        }
    }
    trim_trailing(out);
    out.push('\n');
}

fn write_span(out: &mut String, content: &str, style: Style) {
    let prefix = ansi_prefix(style);
    if prefix.is_empty() {
        out.push_str(content);
    } else {
        out.push_str(&prefix);
        out.push_str(content);
        out.push_str(RESET);
    }
}

const RESET: &str = "\x1b[0m";

fn ansi_prefix(style: Style) -> String {
    let mut codes: Vec<String> = Vec::new();
    let mods = style.add_modifier;
    if mods.contains(Modifier::BOLD) {
        codes.push("1".into());
    }
    if mods.contains(Modifier::DIM) {
        codes.push("2".into());
    }
    if mods.contains(Modifier::ITALIC) {
        codes.push("3".into());
    }
    if mods.contains(Modifier::UNDERLINED) {
        codes.push("4".into());
    }
    if let Some(color) = style.fg {
        codes.push(color_code(color, true));
    }
    if let Some(color) = style.bg {
        codes.push(color_code(color, false));
    }
    if codes.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", codes.join(";"))
    }
}

fn color_code(color: Color, foreground: bool) -> String {
    let base = if foreground { 30 } else { 40 };
    let bright = if foreground { 90 } else { 100 };
    match color {
        Color::Black => (base).to_string(),
        Color::Red => (base + 1).to_string(),
        Color::Green => (base + 2).to_string(),
        Color::Yellow => (base + 3).to_string(),
        Color::Blue => (base + 4).to_string(),
        Color::Magenta => (base + 5).to_string(),
        Color::Cyan => (base + 6).to_string(),
        Color::Gray => (base + 7).to_string(),
        Color::DarkGray => (bright).to_string(),
        Color::LightRed => (bright + 1).to_string(),
        Color::LightGreen => (bright + 2).to_string(),
        Color::LightYellow => (bright + 3).to_string(),
        Color::LightBlue => (bright + 4).to_string(),
        Color::LightMagenta => (bright + 5).to_string(),
        Color::LightCyan => (bright + 6).to_string(),
        Color::White => (bright + 7).to_string(),
        Color::Indexed(index) => {
            let kind = if foreground { 38 } else { 48 };
            format!("{kind};5;{index}")
        }
        Color::Rgb(r, g, b) => {
            let kind = if foreground { 38 } else { 48 };
            format!("{kind};2;{r};{g};{b}")
        }
        Color::Reset => (if foreground { 39 } else { 49 }).to_string(),
    }
}

fn trim_trailing(out: &mut String) {
    let trimmed = out.trim_end_matches(' ').len();
    out.truncate(trimmed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_prefix_combines_modifiers_and_color() {
        let style = theme::accent();
        assert_eq!(ansi_prefix(style), "\x1b[1;36m");
    }

    #[test]
    fn plain_span_has_no_escape() {
        assert!(ansi_prefix(Style::default()).is_empty());
    }
}
