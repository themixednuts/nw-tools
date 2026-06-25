//! The shared presentation layer: capability detection ([`theme`]), the output
//! model ([`report`]), and the static stdout printer ([`print`]). All three
//! renderers — static reports, the interactive browsers, and the live progress
//! display — build on the same ratatui styling primitives.

pub mod image;
pub mod print;
pub mod report;
pub mod theme;

pub use report::{Cell, Report, Table};

#[cfg(test)]
mod tests {
    use super::report::{Cell, Table};
    use super::theme::Caps;

    fn render_plain(table: &Table, width: usize) -> Vec<String> {
        table
            .render(width, Caps::PLAIN)
            .into_iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn plain_table_aligns_and_rules() {
        let mut table = Table::new(["Key", "Entries", "Sample"]).right([1]);
        table.push([
            Cell::text("dds"),
            Cell::text("92113"),
            Cell::path("a/b.dds"),
        ]);

        let lines = render_plain(&table, 80);
        assert_eq!(lines[0], "  Key  Entries  Sample");
        assert_eq!(lines[1], "  ---  -------  -------");
        // Entries is right-aligned within its 7-wide column.
        assert_eq!(lines[2], "  dds    92113  a/b.dds");
    }

    #[test]
    fn narrow_width_truncates_with_ascii_ellipsis() {
        let mut table = Table::new(["Path"]);
        table.push([Cell::path("very/long/path/to/some/file.slice")]);

        let lines = render_plain(&table, 16);
        assert!(lines[2].contains("..."));
        assert!(lines.iter().all(|line| line.chars().count() <= 16));
    }
}
