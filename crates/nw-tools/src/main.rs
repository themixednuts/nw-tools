mod asset;
mod cache;
mod extract;
mod formats;
mod fuzzy;
mod jobs;
mod model;
mod pak;
mod progress;
mod support;
mod tui;
mod ui;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

use ui::{Report, theme};

#[derive(Debug, Parser)]
#[command(name = "nw-tools", version, about = "New World asset inspection tools")]
struct Cli {
    /// When to colorize output.
    #[arg(long, value_enum, default_value_t = ColorArg::Auto, global = true)]
    color: ColorArg,

    /// Plain, non-interactive output: no color, no full-screen browsers.
    #[arg(long, global = true)]
    plain: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ColorArg {
    Auto,
    Always,
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
    #[command(about = "Print the detected New World install paths")]
    Locate,
    #[command(about = "Normalize an archive path")]
    Paths { path: String },
    #[command(about = "Cross-pak asset inventory, search, and extraction")]
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
        command: formats::Cmd,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    theme::init(cli.color.into(), cli.plain);
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    match cli.command {
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
            println!("{}", nw_filesystem::normalize_archive_path(&path));
        }
        Some(Command::Asset { command }) => command.run()?,
        Some(Command::Pak { command }) => command.run()?,
        Some(Command::Format { command }) => command.run()?,
        None => {
            Cli::command().print_help()?;
            println!();
        }
    }
    Ok(())
}
