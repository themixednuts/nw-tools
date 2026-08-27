use std::io::{self, IsTerminal as _};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, bail};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use nw_datasheet::game_system::{GameSystemCell, GameSystemTable, OwnedCellValue};
use nw_gamedata_codegen::load_catalog_from_asset_root;

#[derive(Debug, Parser)]
#[command(
    name = "nw-gamedata-inspect",
    about = "Inspect or compare New World GameData tables.",
    version,
    arg_required_else_help = true,
    after_help = "Environment:\n  RUST_LOG      Layer tracing directives over -v/--verbose or -q/--quiet.\n  NO_COLOR      Disable automatic color output.\n  NW_ASSETS_DIR Default New World asset root for --assets."
)]
struct CommandLine {
    /// Output encoding. JSON uses the stable `nw-gamedata-inspect.output.v1` envelope.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,

    /// Increase default log verbosity (`-v` info, `-vv` debug, `-vvv` trace).
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,

    /// Restrict default diagnostics to errors. RUST_LOG can add directives.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,

    /// When to colorize diagnostic output.
    #[arg(long, value_enum, default_value_t = ColorArg::Auto, global = true)]
    color: ColorArg,

    #[command(subcommand)]
    command: InspectCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
enum OutputFormat {
    /// Human-readable command output.
    #[default]
    Text,
    /// Machine-readable output with rendered lines represented as strings.
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
enum ColorArg {
    /// Colorize diagnostics only on a terminal when NO_COLOR is unset.
    #[default]
    Auto,
    /// Always colorize diagnostic output.
    Always,
    /// Never colorize diagnostic output.
    Never,
}

impl ColorArg {
    fn stderr_ansi(self) -> bool {
        match self {
            Self::Auto => std::env::var_os("NO_COLOR").is_none() && io::stderr().is_terminal(),
            Self::Always => true,
            Self::Never => false,
        }
    }
}

#[derive(Debug, Subcommand)]
enum InspectCommand {
    /// Show a table's schema, sources, and matching rows.
    Show(ShowArgs),

    /// Compare normalized values between one source and target column.
    Compare(CompareArgs),
}

#[derive(Debug, Args)]
struct ShowArgs {
    /// New World asset root containing the shipping `.pak` files.
    #[arg(long, value_name = "DIR", env = "NW_ASSETS_DIR")]
    assets: PathBuf,

    /// Table sheet name or row type name.
    #[arg(value_name = "TABLE")]
    table: String,

    /// Restrict output to one or more column names.
    #[arg(long = "column", value_name = "COLUMN")]
    columns: Vec<String>,

    /// Maximum matching rows to print; `0` means unlimited.
    #[arg(long, value_name = "N", default_value_t = 20)]
    limit: usize,

    /// Include empty string cells in row output.
    #[arg(long)]
    show_empty: bool,

    /// Case-insensitive text filter applied to selected columns.
    #[arg(long, value_name = "TEXT")]
    filter: Option<String>,

    /// Print only table metadata and selected column definitions.
    #[arg(long)]
    summary_only: bool,
}

#[derive(Debug, Args)]
struct CompareArgs {
    /// New World asset root containing the shipping `.pak` files.
    #[arg(long, value_name = "DIR", env = "NW_ASSETS_DIR")]
    assets: PathBuf,

    /// Source table sheet name or row type name.
    #[arg(value_name = "SOURCE_TABLE")]
    source_table: String,

    /// Source column whose normalized values are checked.
    #[arg(value_name = "SOURCE_COLUMN")]
    source_column: String,

    /// Target table sheet name or row type name.
    #[arg(value_name = "TARGET_TABLE")]
    target_table: String,

    /// Target column containing the allowed normalized values.
    #[arg(value_name = "TARGET_COLUMN")]
    target_column: String,

    /// Maximum missing values to print; `0` means unlimited.
    #[arg(long, value_name = "N", default_value_t = 20)]
    limit: usize,
}

#[derive(Debug)]
struct Cli {
    assets: PathBuf,
    table: String,
    columns: Vec<String>,
    limit: usize,
    show_empty: bool,
    filter: Option<String>,
    summary_only: bool,
    target_table: Option<String>,
    target_column: Option<String>,
}

impl From<CommandLine> for Cli {
    fn from(args: CommandLine) -> Self {
        match args.command {
            InspectCommand::Show(command) => Self {
                assets: command.assets,
                table: command.table,
                columns: command.columns,
                limit: command.limit,
                show_empty: command.show_empty,
                filter: command.filter,
                summary_only: command.summary_only,
                target_table: None,
                target_column: None,
            },
            InspectCommand::Compare(command) => Self {
                assets: command.assets,
                table: command.source_table,
                columns: vec![command.source_column],
                limit: command.limit,
                show_empty: false,
                filter: None,
                summary_only: false,
                target_table: Some(command.target_table),
                target_column: Some(command.target_column),
            },
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let command_line = CommandLine::parse();
    init_logging(command_line.verbose, command_line.quiet, command_line.color);
    if command_line.format == OutputFormat::Json
        && std::env::var_os("NW_GAMEDATA_INSPECT_JSON_CHILD").is_none()
    {
        return run_json_child();
    }
    let cli = Cli::from(command_line);
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

    if let (Some(target_table_name), Some(target_column)) =
        (cli.target_table.as_deref(), cli.target_column.as_deref())
    {
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

fn init_logging(verbosity: u8, quiet: bool, color: ColorArg) {
    let level = match (quiet, verbosity) {
        (true, _) => tracing_subscriber::filter::LevelFilter::ERROR,
        (false, 0) => tracing_subscriber::filter::LevelFilter::WARN,
        (false, 1) => tracing_subscriber::filter::LevelFilter::INFO,
        (false, 2) => tracing_subscriber::filter::LevelFilter::DEBUG,
        (false, _) => tracing_subscriber::filter::LevelFilter::TRACE,
    };
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .with_target(false)
        .with_ansi(color.stderr_ansi())
        .compact()
        .try_init();
}

fn run_json_child() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    let Some(executable) = args.first() else {
        return Ok(());
    };
    let output = ProcessCommand::new(executable)
        .args(&args[1..])
        .env("NW_GAMEDATA_INSPECT_JSON_CHILD", "1")
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
    let document = serde_json::json!({
        "schema": "nw-gamedata-inspect.output.v1",
        "command": args[1..]
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        "success": output.status.success(),
        "exit_code": output.status.code(),
        "lines": stdout.lines().collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&document)?);
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
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
    let source_column_index = source_table.column_index(source_column).with_context(|| {
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

    let source_values = string_column_values(source_table, source_column_index);
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
            .take(if cli.limit == 0 {
                usize::MAX
            } else {
                cli.limit
            })
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

    if cli.summary_only {
        return Ok(());
    }

    let mut printed = 0usize;
    let mut matched = 0usize;
    let needle = cli.filter.as_deref().map(str::to_ascii_lowercase);
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
        if cli.limit != 0 && printed >= cli.limit {
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

    if cli.limit != 0 && matched > printed {
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

#[cfg(test)]
mod tests {
    use super::{Cli, CommandLine};
    use clap::Parser;

    #[test]
    fn show_uses_a_positional_table_and_unlimited_zero() {
        let parsed = CommandLine::try_parse_from([
            "nw-gamedata-inspect",
            "show",
            "--assets",
            "assets",
            "VitalsData",
            "--limit",
            "0",
        ])
        .expect("valid show command");
        let cli = Cli::from(parsed);

        assert_eq!(cli.table, "VitalsData");
        assert_eq!(cli.limit, 0);
        assert!(cli.target_table.is_none());
    }

    #[test]
    fn compare_has_complete_positional_subjects() {
        let parsed = CommandLine::try_parse_from([
            "nw-gamedata-inspect",
            "compare",
            "--assets",
            "assets",
            "Items",
            "ItemId",
            "Definitions",
            "Id",
        ])
        .expect("valid compare command");
        let cli = Cli::from(parsed);

        assert_eq!(cli.table, "Items");
        assert_eq!(cli.columns, ["ItemId"]);
        assert_eq!(cli.target_table.as_deref(), Some("Definitions"));
        assert_eq!(cli.target_column.as_deref(), Some("Id"));
    }

    #[test]
    fn partial_compare_is_rejected_by_clap() {
        let error = CommandLine::try_parse_from([
            "nw-gamedata-inspect",
            "compare",
            "--assets",
            "assets",
            "Items",
            "ItemId",
        ])
        .expect_err("target subjects are required");

        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }
}
