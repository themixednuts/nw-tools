mod asset;
mod formats;
mod jobs;
mod output;
mod pak;
mod support;

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "nw-tools", version, about = "New World asset inspection tools")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
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
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    match cli.command {
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
