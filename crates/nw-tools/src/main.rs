mod asset;
mod audio_export;
mod azoth;
mod cache;
mod dds;
mod extract;
mod format;
mod fuzzy;
mod jobs;
mod model;
mod model_asset;
mod pak;
mod progress;
mod rnr_asset;
mod source;
mod support;
mod tui;

use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum};
use nw_tools::native_port;
use nw_tools::ui;

use ui::{OutputFormat, Report, print, theme};

#[derive(Debug, Parser)]
#[command(
    name = "nw-tools",
    version,
    about = "New World asset inspection tools",
    after_help = "Environment:\n  RUST_LOG       Layer tracing directives over -v/--verbose or -q/--quiet.\n  NO_COLOR       Disable automatic color output.\n  NW_INSTALL_DIR Preferred New World install root."
)]
struct Cli {
    /// When to colorize output.
    #[arg(long, value_enum, default_value_t = ColorArg::Auto, global = true)]
    color: ColorArg,

    /// Output encoding for read and query commands.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,

    /// Plain, non-interactive output: no color, no full-screen browsers.
    #[arg(long, global = true)]
    plain: bool,

    /// Increase default log verbosity (`-v` info, `-vv` debug, `-vvv` trace).
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,

    /// Restrict default diagnostics to errors. RUST_LOG can add directives.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ColorArg {
    /// Colorize only when the output stream supports it.
    Auto,
    /// Always emit ANSI color codes.
    Always,
    /// Never emit ANSI color codes.
    Never,
}

impl From<ColorArg> for theme::ColorChoice {
    fn from(value: ColorArg) -> Self {
        match value {
            ColorArg::Auto => Self::Auto,
            ColorArg::Always => Self::Always,
            ColorArg::Never => Self::Never,
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Blender bridge for the AZoth extension")]
    Azoth {
        #[command(subcommand)]
        command: azoth::Cmd,
    },
    #[command(about = "Print the detected New World install paths")]
    Locate,
    #[command(about = "Normalize an archive path")]
    Paths {
        /// Archive path to normalize.
        path: String,
    },
    #[command(about = "Cross-pak asset summary, search, and extraction")]
    Asset {
        #[command(subcommand)]
        command: asset::Cmd,
    },
    #[command(about = "Pak archive list, shape, extract, and repack commands")]
    Pak {
        #[command(subcommand)]
        command: pak::Cmd,
    },
    #[command(about = "Inspect a specific supported file format")]
    Format {
        #[command(subcommand)]
        command: format::Cmd,
    },
    #[command(about = "Convert extracted legacy assets into native source assets")]
    Port {
        #[command(subcommand)]
        command: native_port::Cmd,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    print::init(cli.format);
    theme::init(
        cli.color.into(),
        cli.plain || cli.format == OutputFormat::Json,
    );
    let level = match (cli.quiet, cli.verbose) {
        (true, _) => tracing_subscriber::filter::LevelFilter::ERROR,
        (false, 0) => tracing_subscriber::filter::LevelFilter::WARN,
        (false, 1) => tracing_subscriber::filter::LevelFilter::INFO,
        (false, 2) => tracing_subscriber::filter::LevelFilter::DEBUG,
        (false, _) => tracing_subscriber::filter::LevelFilter::TRACE,
    };
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    match cli.command {
        Some(Command::Azoth { command }) => command.run()?,
        Some(Command::Locate) => {
            let install = nw_locator::Install::locate()?;
            let mut report = Report::new("install");
            report
                .kv("source", install.source().to_string())
                .kv("root", install.root().display().to_string())
                .kv("assets", install.assets().display().to_string());
            report.print();
        }
        Some(Command::Paths { path }) => {
            let mut report = Report::new("path");
            report.kv("normalized", nw_filesystem::normalize_archive_path(&path));
            report.print();
        }
        Some(Command::Asset { command }) => command.run()?,
        Some(Command::Pak { command }) => command.run()?,
        Some(Command::Format { command }) => command.run()?,
        Some(Command::Port { command }) => command.run()?,
        None => {
            Cli::command().print_help()?;
            println!();
        }
    }
    Ok(())
}
