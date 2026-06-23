use std::fmt;
use std::io::{self, IsTerminal};

#[derive(Debug, Clone)]
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            headers: headers.into_iter().map(Into::into).collect(),
            rows: Vec::new(),
        }
    }

    pub fn push(&mut self, row: impl IntoIterator<Item = impl Into<String>>) {
        self.rows.push(row.into_iter().map(Into::into).collect());
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let columns = self
            .headers
            .len()
            .max(self.rows.iter().map(Vec::len).max().unwrap_or(0));
        let widths = self.widths(columns);

        write_row(f, &self.headers, &widths)?;
        write_separator(f, &widths)?;
        for row in &self.rows {
            write_row(f, row, &widths)?;
        }
        Ok(())
    }
}

impl Table {
    fn widths(&self, columns: usize) -> Vec<usize> {
        let mut widths = vec![0usize; columns];
        for (index, header) in self.headers.iter().enumerate() {
            widths[index] = widths[index].max(display_width(header));
        }
        for row in &self.rows {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(display_width(cell));
            }
        }

        let Some(limit) = terminal_width() else {
            return widths;
        };

        fit_widths(widths, limit)
    }
}

fn fit_widths(mut widths: Vec<usize>, limit: usize) -> Vec<usize> {
    if widths.is_empty() {
        return widths;
    }

    let separators = widths.len().saturating_sub(1) * 2;
    let available = limit.saturating_sub(separators).max(widths.len());
    let mut total = widths.iter().sum::<usize>();
    if total <= available {
        return widths;
    }

    let min_width = if available >= widths.len() * 4 {
        4
    } else {
        (available / widths.len()).max(1)
    };
    let mins = widths
        .iter()
        .map(|width| (*width).min(min_width))
        .collect::<Vec<_>>();

    while total > available {
        let Some((index, width)) = widths
            .iter()
            .enumerate()
            .filter(|(index, width)| **width > mins[*index])
            .max_by_key(|(_, width)| **width)
        else {
            break;
        };
        let shrink = (total - available).min(width - mins[index]).max(1);
        widths[index] -= shrink;
        total -= shrink;
    }

    widths
}

fn write_row(f: &mut fmt::Formatter<'_>, row: &[String], widths: &[usize]) -> fmt::Result {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            f.write_str("  ")?;
        }
        let cell = row.get(index).map_or("", String::as_str);
        let cell = ellipsize(cell, *width);
        write!(f, "{cell:<width$}")?;
    }
    f.write_str("\n")
}

fn write_separator(f: &mut fmt::Formatter<'_>, widths: &[usize]) -> fmt::Result {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            f.write_str("  ")?;
        }
        for _ in 0..*width {
            f.write_str("-")?;
        }
    }
    f.write_str("\n")
}

fn terminal_width() -> Option<usize> {
    if !io::stdout().is_terminal() {
        return None;
    }

    crossterm::terminal::size()
        .ok()
        .map(|(width, _)| usize::from(width))
        .filter(|width| *width > 0)
}

fn display_width(value: &str) -> usize {
    clean(value).chars().count()
}

fn ellipsize(value: &str, width: usize) -> String {
    let value = clean(value);
    let count = value.chars().count();
    if count <= width {
        return value;
    }

    match width {
        0 => String::new(),
        1 => ".".to_string(),
        2 => "..".to_string(),
        3 => "...".to_string(),
        _ => {
            let keep = width - 3;
            format!("{}...", value.chars().take(keep).collect::<String>())
        }
    }
}

fn clean(value: &str) -> String {
    value.replace(['\r', '\n', '\t'], " ")
}
