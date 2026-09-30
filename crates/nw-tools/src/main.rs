mod asset;
mod audio_export;
mod azoth;
mod dds;
mod format;
mod fuzzy;
mod grep_cli;
mod jobs;
mod model;
mod model_asset;
mod mount;
mod pak;
mod progress;
mod rnr_asset;
mod source;
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

    /// Worker count. Omit for Rayon default; use 0 to run on the caller thread.
    #[arg(long, global = true)]
    jobs: Option<usize>,

    /// Replace bundled serialize.json for this process only.
    #[arg(long, global = true, value_name = "FILE")]
    serialize: Option<std::path::PathBuf>,

    /// Replace bundled behavior-context (JSON or 7z) for this process only.
    #[arg(long = "behavior-context", global = true, value_name = "FILE")]
    behavior_context: Option<std::path::PathBuf>,

    /// Replace bundled module descriptors with every `*.json` in this directory.
    #[arg(long, global = true, value_name = "DIR")]
    modules: Option<std::path::PathBuf>,

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
    #[command(about = "Search decoded content and filenames, or resolve a CRC32")]
    Grep(grep_cli::Cmd),
    #[command(about = "Build the local content index")]
    Index {
        /// Skip the confirmation prompt.
        #[arg(short, long)]
        yes: bool,
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
        .with_writer(std::io::stderr)
        .init();

    if cli.serialize.is_some() || cli.behavior_context.is_some() || cli.modules.is_some() {
        nw_tools::resources::install_session(nw_tools::resources::ResourceView::from_overrides(
            cli.serialize.as_deref(),
            cli.behavior_context.as_deref(),
            cli.modules.as_deref(),
        )?);
    }

    match cli.command {
        Some(Command::Index { yes }) => run_index(yes, cli.plain, cli.jobs)?,
        Some(Command::Grep(command)) => command.run()?,
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

fn run_index(yes: bool, plain: bool, jobs: Option<usize>) -> anyhow::Result<()> {
    use std::io::{self, IsTerminal, Write};

    use nw_tools::index::{IndexStatus, build_install, first_build_warning};

    let root = nw_locator::Install::locate()?.assets().to_path_buf();
    if let Some(index) = nw_tools::index::open_existing()
        && index.is_complete(&root)
        && index.has_embedded()
    {
        let mut report = Report::new("index");
        report.kv("status", "current");
        report.print();
        return Ok(());
    }

    if !yes {
        if plain || !io::stdin().is_terminal() {
            anyhow::bail!("pass --yes to build the index without a prompt");
        }
        eprintln!("{}", first_build_warning());
        eprint!("Start? [y/N] ");
        io::stderr().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        if !nw_tools::index::confirm_accepted(&line) {
            let mut report = Report::new("index");
            report.kv("status", "cancelled");
            report.print();
            return Ok(());
        }
    }

    let runner = nw_jobs::JobRunner::from_jobs(jobs)?;
    let status = build_install(&runner)?;
    let mut report = Report::new("index");
    match status {
        IndexStatus::Complete { paks } => {
            report.kv("status", "complete").kv("paks", paks.to_string());
        }
        IndexStatus::Partial { indexed_paks } => {
            report
                .kv("status", "partial")
                .kv("paks", indexed_paks.to_string());
        }
    }
    report.print();
    Ok(())
}
