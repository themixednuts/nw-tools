use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;
use nw_datasheet::game_system::{GameSystemCell, GameSystemTable, OwnedCellValue};
use nw_gamedata_codegen::load_catalog_from_asset_root;

#[derive(Debug, Parser)]
#[command(
    name = "nw-gamedata-inspect",
    about = "Inspect compiler input tables from an extracted New World asset root",
    version
)]
struct Cli {
    /// New World asset root containing the shipping `.pak` files.
    #[arg(long)]
    assets: PathBuf,

    /// Table sheet name or row type name to inspect.
    #[arg(long)]
    table: String,

    /// Restrict output to one or more column names.
    #[arg(long = "column")]
    columns: Vec<String>,

    /// Maximum matching rows to print (`0` means summary only).
    #[arg(long, default_value_t = 20)]
    limit: usize,

    /// Include empty string cells in row output.
    #[arg(long)]
    show_empty: bool,

    /// Case-insensitive row filter applied to selected columns.
    #[arg(long)]
    find: Option<String>,

    /// Compare selected source column values against this target table.
    #[arg(long)]
    target_table: Option<String>,

    /// Compare selected source column values against this target column.
    #[arg(long)]
    target_column: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let catalog = load_catalog_from_asset_root(&cli.assets)?;
    let tables = catalog
        .tables()
        .iter()
        .filter(|table| {
            table.name().eq_ignore_ascii_case(&cli.table)
                || table.type_name().eq_ignore_ascii_case(&cli.table)
        })
        .collect::<Vec<_>>();

    if tables.is_empty() {
        bail!("no table named `{}`", cli.table);
    }

    if cli.target_table.is_some() || cli.target_column.is_some() {
        let target_table_name = cli
            .target_table
            .as_deref()
            .context("--target-table requires --target-column")?;
        let target_column = cli
            .target_column
            .as_deref()
            .context("--target-column requires --target-table")?;
        let target_table = catalog
            .tables()
            .iter()
            .find(|table| {
                table.name().eq_ignore_ascii_case(target_table_name)
                    || table.type_name().eq_ignore_ascii_case(target_table_name)
            })
            .with_context(|| format!("no target table named `{target_table_name}`"))?;
        for table in tables {
            print_reference_evidence(table, &cli, target_table, target_column)?;
        }
        return Ok(());
    }

    for table in tables {
        print_table(table, &cli)?;
    }
    Ok(())
}

fn print_reference_evidence(
    source_table: &GameSystemTable,
    cli: &Cli,
    target_table: &GameSystemTable,
    target_column: &str,
) -> Result<()> {
    if cli.columns.len() != 1 {
        bail!("reference evidence requires exactly one --column");
    }
    let source_column = &cli.columns[0];
    let source_index = source_table.column_index(source_column).with_context(|| {
        format!(
            "source table `{}` has no column `{source_column}`",
            source_table.name()
        )
    })?;
    let target_index = target_table.column_index(target_column).with_context(|| {
        format!(
            "target table `{}` has no column `{target_column}`",
            target_table.name()
        )
    })?;

    let source_values = string_column_values(source_table, source_index);
    let target_values = string_column_values(target_table, target_index);
    let matched = source_values
        .iter()
        .filter(|value| target_values.contains(*value))
        .count();
    let checked = source_values.len();
    let missing = checked.saturating_sub(matched);
    let confidence = if checked == 0 {
        0.0
    } else {
        matched as f64 / checked as f64
    };

    println!(
        "{}.{} -> {}.{} checked={} matched={} missing={} confidence={confidence:.6}",
        source_table.name(),
        source_column,
        target_table.name(),
        target_column,
        checked,
        matched,
        missing
    );

    if missing > 0 {
        println!("  missing:");
        for value in source_values
            .iter()
            .filter(|value| !target_values.contains(*value))
            .take(cli.limit)
        {
            println!("    {value}");
        }
        if cli.limit != 0 && missing > cli.limit {
            println!("    ... limit reached");
        }
    }
    Ok(())
}

fn string_column_values(table: &GameSystemTable, column_index: usize) -> Vec<String> {
    let mut values = table
        .row_refs()
        .filter_map(|row| row.cells().get(column_index)?.value().as_str())
        .map(normalized_key)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn normalized_key(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim()
        .trim_start_matches(['!', '+'])
        .trim()
        .to_ascii_lowercase()
}

fn print_table(table: &GameSystemTable, cli: &Cli) -> Result<()> {
    let selected_columns = selected_column_indices(table, &cli.columns)?;
    println!(
        "table={} row_type={} rows={} columns={} name_crc=0x{:08x} type_crc=0x{:08x}",
        table.name(),
        table.type_name(),
        table.len(),
        table.columns().len(),
        table.name_crc(),
        table.type_crc()
    );
    for source in table.sources() {
        println!("  source={}", source.path().display());
    }
    println!("  columns:");
    for column_index in &selected_columns {
        let column = &table.columns()[*column_index];
        println!(
            "    [{column_index}] {} declared={} crc=0x{:08x}",
            column.name(),
            column.column_type(),
            column.crc()
        );
    }

    if cli.limit == 0 {
        return Ok(());
    }

    let mut printed = 0usize;
    let mut matched = 0usize;
    let needle = cli.find.as_deref().map(str::to_ascii_lowercase);
    for row in table.row_refs() {
        if let Some(needle) = needle.as_deref()
            && !row_matches_selected_columns(&selected_columns, row.cells(), needle)
        {
            continue;
        }
        if !cli.show_empty && row_is_empty_for_selection(&selected_columns, row.cells()) {
            continue;
        }

        matched += 1;
        if printed >= cli.limit {
            continue;
        }
        printed += 1;
        println!("  row={} key_crc=0x{:08x}", row.index(), row.key_crc());
        for column_index in &selected_columns {
            let column = &table.columns()[*column_index];
            let Some(cell) = row.cells().get(*column_index) else {
                continue;
            };
            if !cli.show_empty && cell_value_is_empty(cell) {
                continue;
            }
            println!(
                "    [{column_index}] {} declared={} value_type={} column_crc=0x{:08x} cell_crc=0x{:08x} value={}",
                column.name(),
                column.column_type(),
                cell.value().type_name(),
                column.crc(),
                cell.crc(),
                debug_cell_value(cell.value())
            );
        }
    }

    if printed == cli.limit {
        println!("  ... limit reached");
    }
    println!("  matched_rows={matched}");
    Ok(())
}

fn selected_column_indices(table: &GameSystemTable, columns: &[String]) -> Result<Vec<usize>> {
    if columns.is_empty() {
        return Ok((0..table.columns().len()).collect());
    }

    columns
        .iter()
        .map(|column| {
            table
                .column_index(column)
                .with_context(|| format!("table `{}` has no column `{column}`", table.name()))
        })
        .collect()
}

fn row_is_empty_for_selection(selected_columns: &[usize], cells: &[GameSystemCell]) -> bool {
    selected_columns
        .iter()
        .all(|column_index| cells.get(*column_index).is_none_or(cell_value_is_empty))
}

fn row_matches_selected_columns(
    selected_columns: &[usize],
    cells: &[GameSystemCell],
    needle: &str,
) -> bool {
    selected_columns.iter().any(|column_index| {
        cells
            .get(*column_index)
            .is_some_and(|cell| cell_value_matches(cell.value(), needle))
    })
}

fn cell_value_matches(value: &OwnedCellValue, needle: &str) -> bool {
    match value {
        OwnedCellValue::String(value) => value.to_ascii_lowercase().contains(needle),
        OwnedCellValue::Number(value) => value.to_string().contains(needle),
        OwnedCellValue::Boolean(value) => value.to_string().contains(needle),
    }
}

fn cell_value_is_empty(cell: &GameSystemCell) -> bool {
    matches!(cell.value(), OwnedCellValue::String(value) if value.is_empty())
}

fn debug_cell_value(value: &OwnedCellValue) -> String {
    match value {
        OwnedCellValue::String(value) => format!("{value:?}"),
        OwnedCellValue::Number(value) => value.to_string(),
        OwnedCellValue::Boolean(value) => value.to_string(),
    }
}
