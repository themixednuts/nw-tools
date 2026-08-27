mod fidelity;

use std::io::{self, IsTerminal as _};
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, ExitCode};
use std::thread;

use clap::{ArgAction, Parser, ValueEnum};

use fidelity::{Config, run};

#[derive(Debug, Parser)]
#[command(
    name = "nw-lua-fidelity",
    about = "Compare original Lua source against nw-lua decompile output",
    version,
    after_help = "Environment:\n  RUST_LOG  Layer tracing directives over -v/--verbose or -q/--quiet.\n  NO_COLOR  Disable automatic color output."
)]
struct Cli {
    /// Output encoding. JSON uses the stable `nw-lua-fidelity.output.v1` envelope.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    /// Increase default log verbosity (`-v` info, `-vv` debug, `-vvv` trace).
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,

    /// Restrict default diagnostics to errors. RUST_LOG can add directives.
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,

    /// When to colorize diagnostic output.
    #[arg(long, value_enum, default_value_t = ColorArg::Auto)]
    color: ColorArg,

    /// Lua 5.1 compiler executable used to compile source fixtures.
    #[arg(long, value_name = "EXE")]
    luac: PathBuf,

    /// Lua source file or corpus directory to compare. Repeat for multiple roots.
    #[arg(
        long = "root",
        value_name = "PATH",
        action = ArgAction::Append,
        required = true
    )]
    roots: Vec<PathBuf>,

    /// Maximum number of source files to process; `0` means unlimited.
    #[arg(long, default_value_t = 300)]
    limit: usize,

    /// Maximum examples to print per divergence category; `0` means unlimited.
    #[arg(long, default_value_t = 12)]
    examples: usize,
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose, cli.quiet, cli.color);
    if cli.format == OutputFormat::Json && std::env::var_os("NW_LUA_FIDELITY_JSON_CHILD").is_none()
    {
        return run_json_child();
    }
    let config = Config {
        luac: cli.luac,
        roots: cli.roots,
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

fn run_json_child() -> ExitCode {
    let args = std::env::args_os().collect::<Vec<_>>();
    let Some(executable) = args.first() else {
        return ExitCode::SUCCESS;
    };
    let output = match ProcessCommand::new(executable)
        .args(&args[1..])
        .env("NW_LUA_FIDELITY_JSON_CHILD", "1")
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("nw-lua-fidelity: run JSON child: {error}");
            return ExitCode::FAILURE;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
    let document = serde_json::json!({
        "schema": "nw-lua-fidelity.output.v1",
        "command": args[1..]
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        "success": output.status.success(),
        "exit_code": output.status.code(),
        "lines": stdout.lines().collect::<Vec<_>>(),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&document).expect("serialize nw-lua-fidelity output envelope")
    );
    if output.status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
