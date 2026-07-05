mod fidelity;

use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use clap::{ArgAction, Parser};

use fidelity::{Config, run};

const DEFAULT_LUAC: &str = r"E:\Projects\lua-5.1.5\src\luac.exe";
const DEFAULT_GOOD_LUA: &str = r"E:\Projects\az-rs\resources\fixtures\lua\good-lua";
const DEFAULT_DEMOJSON: &str = r"E:\Projects\DEMOJSON";

#[derive(Debug, Parser)]
#[command(
    name = "nw-lua-fidelity",
    about = "Compare original Lua source against nw-lua decompile output"
)]
struct Cli {
    #[arg(long, value_name = "EXE", default_value = DEFAULT_LUAC)]
    luac: PathBuf,

    #[arg(long = "root", value_name = "DIR", action = ArgAction::Append)]
    roots: Vec<PathBuf>,

    #[arg(long, default_value_t = 300)]
    limit: usize,

    #[arg(long, default_value_t = 12)]
    examples: usize,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let config = Config {
        luac: cli.luac,
        roots: if cli.roots.is_empty() {
            vec![
                PathBuf::from(DEFAULT_GOOD_LUA),
                PathBuf::from(DEFAULT_DEMOJSON),
            ]
        } else {
            cli.roots
        },
        limit: cli.limit,
        examples: cli.examples,
    };

    let examples = config.examples;
    let worker = thread::Builder::new()
        .name("nw-lua-fidelity".to_owned())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || run(&config));

    let result = match worker {
        Ok(worker) => match worker.join() {
            Ok(result) => result,
            Err(_) => Err("fidelity worker panicked".to_owned()),
        },
        Err(error) => Err(format!("failed to start fidelity worker: {error}")),
    };

    match result {
        Ok(summary) => {
            summary.print(examples);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("nw-lua-fidelity: {error}");
            ExitCode::FAILURE
        }
    }
}
