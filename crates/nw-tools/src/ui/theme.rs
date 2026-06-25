//! Terminal capability detection, the semantic palette, glyphs, and the shared
//! width/ellipsize helpers used by every renderer (static reports, the
//! interactive TUI, and the live progress display).

use std::io::{self, IsTerminal};
use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};

/// How color was requested on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

/// Resolved terminal capabilities for the current process.
#[derive(Debug, Clone, Copy)]
pub struct Caps {
    /// Emit ANSI styling.
    pub color: bool,
    /// Use Unicode glyphs (rules, ellipsis, status marks) instead of ASCII.
    pub unicode: bool,
    /// Launch full-screen interactive browsers for the commands that support them.
    pub interactive: bool,
    /// stdout is attached to a terminal.
    pub tty: bool,
    /// The terminal supports the kitty graphics protocol (inline images).
    pub graphics: bool,
}

impl Caps {
    /// Plain, non-interactive capabilities — used as the fallback before
    /// [`init`] runs and for deterministic tests.
    pub const PLAIN: Self = Self {
        color: false,
        unicode: false,
        interactive: false,
        tty: false,
        graphics: false,
    };
}

static CAPS: OnceLock<Caps> = OnceLock::new();

/// Resolve and store the process capabilities from the global flags and the
/// environment. Call once from `main` before any rendering.
pub fn init(choice: ColorChoice, plain: bool) {
    let tty = io::stdout().is_terminal();
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
    let color = match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => tty && !no_color && !plain,
    };
    // Glyphs travel with color: styled output gets Unicode, plain output stays ASCII-safe
    // so it survives pipes, files, and minimal terminals.
    let unicode = color;
    let interactive = tty && !plain && choice != ColorChoice::Never;
    let graphics = tty && !plain && detect_graphics();
    let _ = CAPS.set(Caps {
        color,
        unicode,
        interactive,
        tty,
        graphics,
    });
}

/// Detect kitty-graphics-protocol support from the environment. Covers kitty,
/// Ghostty, WezTerm, and Konsole — the common terminals that implement it.
fn detect_graphics() -> bool {
    let term = std::env::var("TERM").unwrap_or_default();
    if term.contains("kitty") || term.contains("ghostty") {
        return true;
    }
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    if matches!(term_program.as_str(), "WezTerm" | "ghostty") {
        return true;
    }
    std::env::var_os("KITTY_WINDOW_ID").is_some() || std::env::var_os("KONSOLE_VERSION").is_some()
}

/// The resolved process capabilities (or [`Caps::PLAIN`] before `init`).
pub fn caps() -> Caps {
    CAPS.get().copied().unwrap_or(Caps::PLAIN)
}

/// The render width for static reports: the live terminal width on a tty,
/// otherwise a stable default so piped/redirected output is reproducible.
pub fn report_width(caps: Caps) -> usize {
    const DEFAULT: usize = 120;
    if !caps.tty {
        return DEFAULT;
    }
    crossterm::terminal::size()
        .ok()
        .map(|(width, _)| usize::from(width))
        .filter(|width| *width > 0)
        .unwrap_or(DEFAULT)
}

/// The semantic palette. Kept small and meaningful rather than decorative.
pub mod palette {
    use super::Color;

    pub const ACCENT: Color = Color::Cyan;
    pub const GOOD: Color = Color::Green;
    pub const BAD: Color = Color::Red;
    pub const WARN: Color = Color::Yellow;
    pub const INFO: Color = Color::Blue;
    pub const SPECIAL: Color = Color::Magenta;
}

pub fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn accent() -> Style {
    Style::default()
        .fg(palette::ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub fn header() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn good() -> Style {
    Style::default().fg(palette::GOOD)
}

pub fn bad() -> Style {
    Style::default().fg(palette::BAD)
}

pub fn warn() -> Style {
    Style::default().fg(palette::WARN)
}

/// Color for a CryPak compression method label.
pub fn method_style(method: &str) -> Style {
    match method.to_ascii_lowercase().as_str() {
        "stored" | "store" => dim(),
        "deflated" | "deflate" => Style::default().fg(palette::INFO),
        "oodle" => Style::default().fg(palette::SPECIAL),
        _ => Style::default(),
    }
}

/// Glyphs, with ASCII fallbacks for plain output.
pub struct Glyphs {
    pub rule: char,
    pub ellipsis: &'static str,
    pub sep: &'static str,
}

pub fn glyphs(caps: Caps) -> Glyphs {
    if caps.unicode {
        Glyphs {
            rule: '─',
            ellipsis: "…",
            sep: " · ",
        }
    } else {
        Glyphs {
            rule: '-',
            ellipsis: "...",
            sep: "  ",
        }
    }
}

/// Replace control whitespace so a value renders on a single line.
pub fn clean(value: &str) -> String {
    value.replace(['\r', '\n', '\t'], " ")
}

/// Display width of a value once cleaned (character count).
pub fn display_width(value: &str) -> usize {
    clean(value).chars().count()
}

/// Truncate `value` to `width`, appending an ellipsis at the end.
pub fn fit_end(value: &str, width: usize, ellipsis: &str) -> String {
    let value = clean(value);
    if value.chars().count() <= width {
        return value;
    }
    truncate_with(&value, width, ellipsis, |kept| {
        kept.chars().collect::<String>()
    })
}

/// Truncate `value` to `width`, collapsing the middle with an ellipsis so both
/// ends stay visible — ideal for paths.
pub fn fit_middle(value: &str, width: usize, ellipsis: &str) -> String {
    let value = clean(value);
    let count = value.chars().count();
    if count <= width {
        return value;
    }
    let dots = ellipsis.chars().count();
    if width <= dots {
        return value.chars().take(width).collect();
    }
    let content = width - dots;
    let head = content / 2;
    let tail = content - head;
    let start = value.chars().take(head).collect::<String>();
    let end = value.chars().skip(count - tail).collect::<String>();
    format!("{start}{ellipsis}{end}")
}

fn truncate_with(
    value: &str,
    width: usize,
    ellipsis: &str,
    keep: impl Fn(&str) -> String,
) -> String {
    let dots = ellipsis.chars().count();
    if width <= dots {
        return value.chars().take(width).collect();
    }
    let kept = value.chars().take(width - dots).collect::<String>();
    format!("{}{ellipsis}", keep(&kept))
}

/// Shrink column widths so the table fits within `limit`, never below a small
/// floor. Lifted from the original `output.rs`/`progress.rs` logic.
pub fn fit_widths(mut widths: Vec<usize>, limit: usize) -> Vec<usize> {
    if widths.is_empty() {
        return widths;
    }

    let mut total = widths.iter().sum::<usize>();
    if total <= limit {
        return widths;
    }

    let min_width = if limit >= widths.len() * 4 {
        4
    } else {
        (limit / widths.len()).max(1)
    };
    let mins = widths
        .iter()
        .map(|width| (*width).min(min_width))
        .collect::<Vec<_>>();

    while total > limit {
        let Some((index, width)) = widths
            .iter()
            .enumerate()
            .filter(|(index, width)| **width > mins[*index])
            .max_by_key(|(_, width)| **width)
        else {
            break;
        };
        let shrink = (total - limit).min(width - mins[index]).max(1);
        widths[index] -= shrink;
        total -= shrink;
    }

    widths
}
