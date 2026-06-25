use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use nw_localization::{
    LanguageCode, LocalizationCatalog, LocalizationKey, LocalizationLoader, LocalizationTag,
    localization_keys,
};

use crate::jobs::{JobArgs, RunCtx};
use crate::support::{collect_matching, ensure_parent, guard_existing};
use crate::ui::{Cell, Report, Table};

use super::common::{csv_cell, finish_scan, lowered, path_label, text_matches, trim_cell};
use super::datasheet_browser::browse_datasheets;

#[derive(Debug, Args)]
pub struct Datasheet {
    /// Datasheet file or directory. Omit to browse datasheets under the current directory.
    path: Option<PathBuf>,

    #[arg(long, default_value_t = 25)]
    show: usize,

    #[arg(long)]
    columns: bool,

    #[arg(long)]
    rows: Option<usize>,

    #[arg(long)]
    find: Vec<String>,

    /// Exact substring match for --find instead of the default fuzzy ranking.
    #[arg(long)]
    exact: bool,

    #[arg(long)]
    show_empty: bool,

    /// Locale used to resolve localization labels in string cells.
    #[arg(long)]
    locale: Option<LanguageCode>,

    /// Asset root used to load localization. Defaults to the detected game assets path.
    #[arg(long = "loc-root", value_name = "ROOT", requires = "locale")]
    loc_root: Option<PathBuf>,

    /// Localization manifest tag to load; repeat for multiple tags.
    #[arg(long = "loc-tag", requires = "locale")]
    loc_tags: Vec<LocalizationTag>,

    /// String rendering mode when localization is loaded.
    #[arg(long, value_enum, default_value_t = LocalizeArg::Text)]
    localize: LocalizeArg,

    /// Export rows to CSV under this file or directory.
    #[arg(long, value_name = "PATH")]
    csv: Option<PathBuf>,

    /// Replace existing CSV outputs.
    #[arg(long, requires = "csv")]
    overwrite: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum LocalizeArg {
    Key,
    Text,
    Both,
}

#[derive(Debug, Clone)]
struct SheetOptions<'a> {
    columns: bool,
    rows: Option<usize>,
    find: Vec<String>,
    fuzzy: bool,
    show_empty: bool,
    localization: Option<&'a LocalizationCatalog>,
    localize: LocalizeArg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SheetSummary {
    version: u32,
    rows: usize,
    columns: usize,
    cells: usize,
    name: String,
    type_name: String,
    string_columns: usize,
    number_columns: usize,
    boolean_columns: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SheetTotals {
    files: usize,
    rows: usize,
    columns: usize,
    cells: usize,
}

#[derive(Debug, Clone)]
struct SheetScan {
    source: String,
    summary: SheetSummary,
    columns: Vec<ColumnRow>,
    rows: Vec<RowSample>,
    hits: Vec<SheetHit>,
}

#[derive(Debug, Clone)]
struct ColumnRow {
    source: String,
    index: String,
    kind: String,
    crc: String,
    name: String,
}

#[derive(Debug, Clone)]
struct RowSample {
    source: String,
    row: String,
    values: String,
}

#[derive(Debug, Clone)]
struct SheetHit {
    source: String,
    row: String,
    column: String,
    value: String,
    /// Fuzzy match score (0 in exact mode); higher ranks first.
    score: u16,
}

#[derive(Debug, Clone)]
struct SheetExport {
    source: String,
    output: String,
    rows: String,
}

impl From<nw_datasheet::DatasheetSummary<'_>> for SheetSummary {
    fn from(summary: nw_datasheet::DatasheetSummary<'_>) -> Self {
        Self {
            version: summary.version,
            rows: summary.rows,
            columns: summary.columns,
            cells: summary.cells,
            name: summary.name.to_owned(),
            type_name: summary.type_name.to_owned(),
            string_columns: summary.string_columns,
            number_columns: summary.number_columns,
            boolean_columns: summary.boolean_columns,
        }
    }
}

impl SheetTotals {
    fn add(&mut self, summary: &SheetSummary) {
        self.files += 1;
        self.rows += summary.rows;
        self.columns += summary.columns;
        self.cells += summary.cells;
    }
}

impl Datasheet {
    pub(super) fn run(self) -> Result<()> {
        // The interactive grid is the front door: with no path it streams from
        // the located install's paks, and it loads localization dynamically from
        // inside the TUI — so it short-circuits before any filesystem scan or
        // eager catalog load.
        let will_grid = crate::tui::interactive()
            && self.csv.is_none()
            && !self.columns
            && self.rows.is_none()
            && self.find.is_empty();
        if will_grid {
            let mode = match self.localize {
                LocalizeArg::Key => 0,
                LocalizeArg::Text => 1,
                LocalizeArg::Both => 2,
            };
            let initial = self.locale.as_ref().map(|code| code.as_str().to_string());
            return browse_datasheets(
                self.path.clone(),
                self.loc_root.clone(),
                initial,
                mode,
                self.jobs.jobs,
            );
        }

        let ctx = self.jobs.ctx()?;
        let root = self.path.clone().unwrap_or_else(|| PathBuf::from("."));
        let paths = collect_matching(&root, nw_datasheet::is_datasheet_path)?;
        let find = lowered(self.find);
        let needs_localization = self.locale.is_some()
            && self.localize != LocalizeArg::Key
            && (self.csv.is_some() || self.rows.is_some() || !find.is_empty());
        let needed_keys = if needs_localization {
            collect_datasheet_localization_keys(
                &paths,
                (self.csv.is_none() && find.is_empty())
                    .then_some(self.rows)
                    .flatten(),
            )?
        } else {
            BTreeSet::new()
        };
        let localization = self
            .locale
            .clone()
            .filter(|_| needs_localization)
            .map(|locale| {
                let root = match self.loc_root.as_ref() {
                    Some(root) => root.clone(),
                    None => nw_locator::Install::locate()?.assets(),
                };
                let assets = nw_asset::AssetStore::open(root)?;
                // Run the parallel load on the configured --jobs pool.
                let catalog = ctx.runner.install(|| {
                    LocalizationLoader::new(&assets, locale)
                        .tags(self.loc_tags.clone())
                        .keys(needed_keys.iter().cloned())
                        .load()
                })?;
                Ok::<LocalizationCatalog, anyhow::Error>(catalog)
            })
            .transpose()?;
        let localization = localization.map(Arc::new);
        if let Some(catalog) = localization.as_ref() {
            let loc = catalog.report();
            Report::new("localization")
                .stat("language", catalog.language())
                .stat("needed", needed_keys.len())
                .stat("files", loc.source_files())
                .stat("entries", loc.entries())
                .stat("duplicates", loc.duplicates().len())
                .print();
        }
        let options = SheetOptions {
            columns: self.columns,
            rows: self.rows,
            find,
            fuzzy: !self.exact,
            show_empty: self.show_empty,
            localization: localization.as_deref(),
            localize: self.localize,
        };
        if let Some(out) = self.csv.as_ref() {
            return export_datasheets(&ctx, &root, &paths, out, &options, self.overwrite);
        }
        let batch = ctx.map_results_compact(
            "datasheet",
            &paths,
            |path| path_label(path),
            |path, progress| progress.step(|| scan_sheet(path, &options)),
        );
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut scans = Vec::new();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(scan) => scans.push(scan),
                Err(error) => errors.push(error),
            }
        }
        scans.sort_by(|left, right| left.source.cmp(&right.source));
        let mut report = sheet_summary_report(&scans, self.show);

        if self.columns {
            let mut table = Table::new(["Source", "Index", "Type", "CRC", "Name"]).right([1]);
            for row in scans.iter().flat_map(|scan| scan.columns.clone()) {
                table.push([
                    Cell::path(row.source),
                    Cell::text(row.index),
                    Cell::text(row.kind),
                    Cell::text(row.crc),
                    Cell::text(row.name),
                ]);
            }
            if !table.is_empty() {
                report.table(table);
            }
        }

        if self.rows.is_some() {
            let mut table = Table::new(["Source", "Row", "Values"]).right([1]);
            for row in scans.iter().flat_map(|scan| scan.rows.clone()) {
                table.push([
                    Cell::path(row.source),
                    Cell::text(row.row),
                    Cell::text(row.values),
                ]);
            }
            if !table.is_empty() {
                report.table(table);
            }
        }

        if !options.find.is_empty() {
            let mut hits = scans
                .into_iter()
                .flat_map(|scan| scan.hits)
                .collect::<Vec<_>>();
            // Stable rank by fuzzy score; a no-op in exact mode (all scores 0).
            hits.sort_by_key(|hit| std::cmp::Reverse(hit.score));
            let mut table = Table::new(["Source", "Row", "Column", "Value"]).right([1]);
            for hit in hits {
                table.push([
                    Cell::path(hit.source),
                    Cell::text(hit.row),
                    Cell::text(hit.column),
                    Cell::text(hit.value),
                ]);
            }
            report.table_or(table, "no datasheet matches");
        }
        report.print();

        finish_scan(cancelled, skipped, &errors, "datasheet")
    }
}

fn export_datasheets(
    ctx: &RunCtx,
    root: &Path,
    paths: &[PathBuf],
    out: &Path,
    options: &SheetOptions<'_>,
    overwrite: bool,
) -> Result<()> {
    if paths.len() > 1 && out.extension().is_some() {
        bail!("CSV output must be a directory when exporting more than one datasheet");
    }

    let batch = ctx.map_results_compact(
        "datasheet export",
        paths,
        |path| path_label(path),
        |path, progress| progress.step(|| export_datasheet(root, path, out, options, overwrite)),
    );
    let skipped = batch.skipped();
    let cancelled = batch.was_cancelled();
    let mut exported = Vec::new();
    let mut errors = Vec::new();

    for result in batch.into_completed() {
        match result {
            Ok(row) => exported.push(row),
            Err(error) => errors.push(error),
        }
    }
    exported.sort_by(|left, right| left.source.cmp(&right.source));

    let mut report = Report::new("datasheet export")
        .stat("datasheets", paths.len())
        .stat("exported", exported.len());
    let mut table = Table::new(["Source", "Output", "Rows"]).right([2]);
    for row in exported {
        table.push([
            Cell::path(row.source),
            Cell::path(row.output),
            Cell::text(row.rows),
        ]);
    }
    report.table_or(table, "no datasheets exported");
    report.print();

    finish_scan(cancelled, skipped, &errors, "datasheet export")
}

fn export_datasheet(
    root: &Path,
    path: &Path,
    out: &Path,
    options: &SheetOptions<'_>,
    overwrite: bool,
) -> Result<SheetExport> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut sheet = nw_datasheet::Datasheet::parse(&bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    if let Some(localization) = options.localization {
        sheet.set_localization(Some(localization));
    }

    let output = datasheet_csv_output_path(root, path, out);
    guard_existing(&output, overwrite.into())?;
    write_datasheet_csv(&output, &sheet, options)
        .with_context(|| format!("write {}", output.display()))?;

    Ok(SheetExport {
        source: path_label(path),
        output: output.display().to_string(),
        rows: sheet.len().to_string(),
    })
}

fn write_datasheet_csv(
    path: &Path,
    sheet: &nw_datasheet::Datasheet<'_>,
    options: &SheetOptions<'_>,
) -> Result<()> {
    ensure_parent(path)?;

    let mut csv = String::new();
    for (index, column) in sheet.columns().iter().enumerate() {
        if index > 0 {
            csv.push(',');
        }
        csv.push_str(&csv_cell(column.name()));
    }
    csv.push('\n');

    for row in sheet.rows() {
        for (index, cell) in row.cells().iter().enumerate() {
            if index > 0 {
                csv.push(',');
            }
            csv.push_str(&csv_cell(&cell_text(sheet, cell, options)));
        }
        csv.push('\n');
    }

    std::fs::write(path, csv)?;
    Ok(())
}

fn datasheet_csv_output_path(root: &Path, source: &Path, out: &Path) -> PathBuf {
    if root.is_file() && out.extension().is_some() {
        return out.to_path_buf();
    }

    let relative = if root.is_file() {
        source
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| source.to_path_buf())
    } else {
        source.strip_prefix(root).unwrap_or(source).to_path_buf()
    };
    let mut output = out.join(relative);
    output.set_extension("csv");
    output
}

fn collect_datasheet_localization_keys(
    paths: &[PathBuf],
    row_limit: Option<usize>,
) -> Result<BTreeSet<LocalizationKey>> {
    let mut keys = BTreeSet::new();
    for path in paths {
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let sheet = nw_datasheet::Datasheet::parse(&bytes)
            .with_context(|| format!("parse {}", path.display()))?;
        for row in sheet.rows().take(row_limit.unwrap_or(usize::MAX)) {
            for cell in row.cells() {
                if let Some(value) = cell.as_str() {
                    keys.extend(localization_keys(value));
                }
            }
        }
    }
    Ok(keys)
}

fn scan_sheet(path: &Path, options: &SheetOptions) -> Result<SheetScan> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut sheet = nw_datasheet::Datasheet::parse(&bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    if let Some(localization) = options.localization {
        sheet.set_localization(Some(localization));
    }
    let summary = SheetSummary::from(sheet.summary());
    let source = path.display().to_string();
    let columns = if options.columns {
        sheet
            .columns()
            .iter()
            .enumerate()
            .map(|(index, column)| ColumnRow {
                source: source.clone(),
                index: index.to_string(),
                kind: column.column_type().to_string(),
                crc: format!("0x{:08x}", column.crc()),
                name: column.name().to_string(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let rows = options
        .rows
        .map(|limit| {
            sheet
                .rows()
                .enumerate()
                .take(limit)
                .map(|(index, row)| RowSample {
                    source: source.clone(),
                    row: index.to_string(),
                    values: row_values(&sheet, &row, options),
                })
                .collect()
        })
        .unwrap_or_default();
    let hits = if options.find.is_empty() {
        Vec::new()
    } else {
        find_sheet_cells(&source, &sheet, options)
    };

    Ok(SheetScan {
        source,
        summary,
        columns,
        rows,
        hits,
    })
}

fn sheet_summary_report(scans: &[SheetScan], limit: usize) -> Report {
    let mut totals = SheetTotals::default();
    for scan in scans {
        totals.add(&scan.summary);
    }
    let mut report = Report::new("datasheet")
        .stat("files", totals.files)
        .stat("rows", totals.rows)
        .stat("columns", totals.columns)
        .stat("cells", totals.cells);

    let mut table = Table::new([
        "Source", "Version", "Rows", "Columns", "Cells", "Strings", "Numbers", "Booleans", "Name",
        "Type",
    ])
    .right([2, 3, 4, 5, 6, 7]);
    for scan in scans.iter().take(limit) {
        let summary = &scan.summary;
        table.push([
            Cell::path(scan.source.clone()),
            Cell::text(format!("0x{:x}", summary.version)),
            Cell::text(summary.rows.to_string()),
            Cell::text(summary.columns.to_string()),
            Cell::text(summary.cells.to_string()),
            Cell::text(summary.string_columns.to_string()),
            Cell::text(summary.number_columns.to_string()),
            Cell::text(summary.boolean_columns.to_string()),
            Cell::text(summary.name.clone()),
            Cell::text(summary.type_name.clone()),
        ]);
    }
    if !table.is_empty() {
        report.table(table);
    }
    if scans.len() > limit {
        report.more(scans.len() - limit, "files");
    }
    report
}

fn row_values(
    sheet: &nw_datasheet::Datasheet<'_>,
    row: &nw_datasheet::Row<'_, '_>,
    options: &SheetOptions<'_>,
) -> String {
    let mut values = Vec::new();
    for (column, cell) in row.columns().iter().zip(row.cells()) {
        let value = cell_text(sheet, cell, options);
        if !options.show_empty && value.is_empty() {
            continue;
        }
        values.push(format!("{}={}", column.name(), trim_cell(value)));
    }
    values.join(", ")
}

fn find_sheet_cells(
    source: &str,
    sheet: &nw_datasheet::Datasheet<'_>,
    options: &SheetOptions<'_>,
) -> Vec<SheetHit> {
    let mut search = options
        .fuzzy
        .then(|| crate::fuzzy::MultiSearch::new(&options.find));
    let mut hits = Vec::new();
    for (row_index, row) in sheet.rows().enumerate() {
        for (column, cell) in row.columns().iter().zip(row.cells()) {
            let value = cell_text(sheet, cell, options);
            let score = match &mut search {
                Some(search) => match search.score_any([value.as_ref(), column.name()]) {
                    Some(score) => score,
                    None => continue,
                },
                None => {
                    if !text_matches(&value, &options.find)
                        && !text_matches(column.name(), &options.find)
                    {
                        continue;
                    }
                    0
                }
            };
            hits.push(SheetHit {
                source: source.to_string(),
                row: row_index.to_string(),
                column: column.name().to_string(),
                value: trim_cell(value),
                score,
            });
        }
    }
    hits
}

fn cell_text<'a>(
    sheet: &nw_datasheet::Datasheet<'_>,
    cell: &'a nw_datasheet::Cell<'a>,
    options: &SheetOptions<'_>,
) -> Cow<'a, str> {
    let Some(value) = cell.as_str() else {
        return Cow::Owned(cell.to_string());
    };

    match options.localize {
        LocalizeArg::Key => Cow::Borrowed(value),
        LocalizeArg::Text => sheet.localized(value),
        LocalizeArg::Both => {
            let localized = sheet.localized(value);
            if localized == value {
                Cow::Borrowed(value)
            } else {
                Cow::Owned(format!("{value} | {localized}"))
            }
        }
    }
}
