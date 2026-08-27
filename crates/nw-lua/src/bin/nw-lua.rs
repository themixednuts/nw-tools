use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fmt, fs,
    io::{self, IsTerminal as _, Read as _, Write as _},
    path::{Path, PathBuf},
    process::{self, Command as ProcessCommand},
};

use clap::{ArgAction, ArgGroup, Parser, ValueEnum};
use nw_jobs::JobRunner;
use nw_lua::{
    LuaError,
    bytecode::OpcodeTable,
    version::{LuaTarget, LuaVersion},
};

#[derive(Debug, Parser)]
#[command(
    name = "nw-lua",
    version,
    about = "Disassemble or decompile Lua binary chunks",
    override_usage = "nw-lua [options] <file.luac>...",
    group(ArgGroup::new("output").args(["out", "out_dir"]).multiple(false)),
    after_help = "Environment:\n  RUST_LOG  Layer tracing directives over -v/--verbose or -q/--quiet.\n  NO_COLOR  Disable automatic color output."
)]
struct Cli {
    /// Output encoding. JSON uses the stable `nw-lua.output.v1` envelope.
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

    /// Lua rendering operation.
    #[arg(long, value_enum, default_value_t = Mode::Decompile)]
    mode: Mode,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "prepend disassembly as Lua comments during decompilation"
    )]
    annotate: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "skip idiomatic AST cleanup during decompilation"
    )]
    no_idiomatic: bool,
    #[arg(
        long,
        value_name = "VERSION",
        value_enum,
        help = "override the detected Lua bytecode version"
    )]
    lua_version: Option<LuaVersionArg>,
    #[arg(
        long,
        value_name = "FILE",
        help = "load a custom opcode-table mapping from FILE"
    )]
    opcode_table: Option<PathBuf>,
    #[arg(long, action = ArgAction::SetTrue, help = "emit debug trace to stderr")]
    debug: bool,
    #[arg(
        short = 'j',
        long,
        value_name = "N",
        help = "worker count; omit for automatic selection, or use 0 for the caller thread"
    )]
    jobs: Option<usize>,
    #[arg(
        short,
        long = "out",
        alias = "output",
        value_name = "FILE",
        help = "write one input's rendered result to FILE"
    )]
    out: Option<PathBuf>,
    #[arg(
        long = "out-dir",
        value_name = "DIR",
        help = "write multiple inputs' rendered results beneath DIR"
    )]
    out_dir: Option<PathBuf>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        requires = "output",
        help = "replace existing output files"
    )]
    force: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        requires = "output",
        help = "read and render inputs without writing output files"
    )]
    dry_run: bool,
    #[arg(value_name = "FILE", allow_hyphen_values = true, num_args = 1..)]
    inputs: Vec<PathBuf>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LuaVersionArg {
    /// Lua 5.1 bytecode.
    #[value(name = "51")]
    Lua51,
}

impl LuaVersionArg {
    fn target(self) -> LuaTarget {
        match self {
            Self::Lua51 => LuaTarget::for_version(LuaVersion::V51)
                .expect("the built-in Lua 5.1 opcode table is available"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Mode {
    /// Disassemble bytecode.
    #[value(name = "dis")]
    Disassemble,
    /// Decompile to Lua source.
    #[value(name = "dec")]
    Decompile,
    /// Dump SSA intermediate representation for every prototype.
    #[value(name = "ssa")]
    SsaDump,
}

#[derive(Debug)]
enum CliError {
    Message(String),
    Io { context: String, source: io::Error },
    Lua(LuaError),
}

fn main() {
    if let Err(error) = run() {
        eprintln!("nw-lua: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    init_logging(cli.verbose, cli.quiet, cli.color);
    if cli.format == OutputFormat::Json && std::env::var_os("NW_LUA_JSON_CHILD").is_none() {
        return run_json_child();
    }
    let mode = cli.mode;

    validate_cli(&cli, mode)?;
    let table = load_opcode_table(&cli)?;
    if cli.inputs.len() == 1 {
        run_single(&cli, mode, table.as_ref())
    } else {
        run_batch(&cli, mode, table.as_ref())
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

fn validate_cli(cli: &Cli, mode: Mode) -> Result<(), CliError> {
    if cli.annotate && mode != Mode::Decompile {
        return Err(CliError::Message(
            "--annotate can only be used with decompilation".to_string(),
        ));
    }
    if cli.no_idiomatic && mode != Mode::Decompile {
        return Err(CliError::Message(
            "--no-idiomatic can only be used with decompilation".to_string(),
        ));
    }
    if cli.inputs.len() > 1 {
        if cli.inputs.iter().any(|input| is_dash(input)) {
            return Err(CliError::Message(
                "stdin cannot be combined with multiple input files".to_string(),
            ));
        }
        if cli.out.is_some() {
            return Err(CliError::Message(
                "multiple input files require --out-dir <DIR>, not --out".to_string(),
            ));
        }
        if cli.out_dir.as_deref().is_none_or(is_dash) {
            return Err(CliError::Message(
                "multiple input files require --out-dir <DIR>".to_string(),
            ));
        }
    } else if cli.out_dir.is_some() {
        return Err(CliError::Message(
            "one input file uses --out <FILE>, not --out-dir".to_string(),
        ));
    }
    Ok(())
}

fn run_json_child() -> Result<(), CliError> {
    let args = std::env::args_os().collect::<Vec<_>>();
    let Some(executable) = args.first() else {
        return Ok(());
    };
    let output = ProcessCommand::new(executable)
        .args(&args[1..])
        .env("NW_LUA_JSON_CHILD", "1")
        .output()
        .map_err(|source| CliError::Io {
            context: "run nw-lua JSON child".to_string(),
            source,
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
    let document = serde_json::json!({
        "schema": "nw-lua.output.v1",
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
        serde_json::to_string_pretty(&document).expect("serialize nw-lua output envelope")
    );
    if !output.status.success() {
        process::exit(output.status.code().unwrap_or(1));
    }
    Ok(())
}

fn run_single(cli: &Cli, mode: Mode, table: Option<&OpcodeTable>) -> Result<(), CliError> {
    let input = &cli.inputs[0];
    let bytes = read_input(input)?;

    if cli.debug {
        eprintln!("nw-lua: mode={mode:?}");
        eprintln!("nw-lua: input={} bytes", bytes.len());
        match &table {
            Some(table) => eprintln!(
                "nw-lua: opcode-table version={} opcodes={}",
                table.version,
                table.map.len()
            ),
            None => eprintln!("nw-lua: opcode-table=detected builtin"),
        }
    }

    let module_stem = input_module_stem(input);
    let output = render(
        &bytes,
        mode,
        cli.annotate,
        cli.no_idiomatic,
        table,
        module_stem.as_deref(),
    )?;
    write_output(cli.out.as_deref(), &output, cli.force, cli.dry_run)?;
    if cli.dry_run {
        eprintln!(
            "dry-run: rendered {} -> {}",
            input.display(),
            cli.out
                .as_deref()
                .expect("--dry-run requires output")
                .display()
        );
    }
    Ok(())
}

#[derive(Debug)]
struct BatchOutcome {
    input: PathBuf,
    output: PathBuf,
    input_bytes: usize,
    output_bytes: usize,
}

fn run_batch(cli: &Cli, mode: Mode, table: Option<&OpcodeTable>) -> Result<(), CliError> {
    let output_dir = cli.out_dir.as_deref().expect("validated output directory");
    let outputs = batch_output_paths(&cli.inputs, output_dir, mode)?;
    guard_outputs(&outputs, cli.force)?;
    if !cli.dry_run {
        fs::create_dir_all(output_dir).map_err(|source| CliError::Io {
            context: format!("create output directory {}", output_dir.display()),
            source,
        })?;
    }
    let runner = JobRunner::from_jobs(cli.jobs)
        .map_err(|error| CliError::Message(format!("create worker pool: {error}")))?;

    let outcomes = runner.map_indexed(cli.inputs.len(), |index| {
        let input = &cli.inputs[index];
        let output = &outputs[index];
        render_file(input, output, mode, cli, table)
            .map_err(|error| CliError::Message(format!("{}: {error}", input.display())))
    });

    for outcome in outcomes {
        let outcome = outcome?;
        if cli.debug {
            eprintln!(
                "nw-lua: {} ({} bytes) -> {} ({} bytes)",
                outcome.input.display(),
                outcome.input_bytes,
                outcome.output.display(),
                outcome.output_bytes,
            );
        }
    }
    if cli.dry_run {
        eprintln!(
            "dry-run: rendered {} inputs beneath {}",
            cli.inputs.len(),
            output_dir.display()
        );
    }
    Ok(())
}

fn render_file(
    input: &Path,
    output: &Path,
    mode: Mode,
    cli: &Cli,
    table: Option<&OpcodeTable>,
) -> Result<BatchOutcome, CliError> {
    let bytes = read_input(input)?;
    let module_stem = input_module_stem(input);
    let rendered = render(
        &bytes,
        mode,
        cli.annotate,
        cli.no_idiomatic,
        table,
        module_stem.as_deref(),
    )?;
    write_output(Some(output), &rendered, cli.force, cli.dry_run)?;
    Ok(BatchOutcome {
        input: input.to_path_buf(),
        output: output.to_path_buf(),
        input_bytes: bytes.len(),
        output_bytes: rendered.len(),
    })
}

fn batch_output_paths(
    inputs: &[PathBuf],
    output_dir: &Path,
    mode: Mode,
) -> Result<Vec<PathBuf>, CliError> {
    let mut seen = BTreeSet::new();
    inputs
        .iter()
        .map(|input| {
            let file_name = input.file_name().ok_or_else(|| {
                CliError::Message(format!("input has no file name: {}", input.display()))
            })?;
            let mut output = output_dir.join(file_name);
            output.set_extension(mode.output_extension());
            let key = output_key(&output);
            if !seen.insert(key) {
                return Err(CliError::Message(format!(
                    "multiple inputs map to output {}",
                    output.display()
                )));
            }
            Ok(output)
        })
        .collect()
}

fn output_key(path: &Path) -> String {
    let key = path.as_os_str().to_string_lossy();
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key.into_owned()
    }
}

impl Mode {
    const fn output_extension(self) -> &'static str {
        match self {
            Self::Disassemble => "dis.txt",
            Self::Decompile => "lua",
            Self::SsaDump => "ssa.txt",
        }
    }
}

fn render(
    bytes: &[u8],
    mode: Mode,
    annotate: bool,
    no_idiomatic: bool,
    table: Option<&OpcodeTable>,
    module_stem: Option<&str>,
) -> Result<String, LuaError> {
    let options = if no_idiomatic {
        nw_lua::DecompOptions::core()
    } else {
        nw_lua::DecompOptions::default()
    };
    match mode {
        Mode::Disassemble => match table {
            Some(table) => nw_lua::disassemble_with(bytes, table),
            None => nw_lua::disassemble(bytes),
        },
        Mode::SsaDump => match table {
            Some(table) => nw_lua::ssa_dump_with(bytes, table),
            None => nw_lua::ssa_dump(bytes),
        },
        Mode::Decompile => {
            let source = match table {
                Some(table) => nw_lua::decompile_with_table_options_and_module_stem(
                    bytes,
                    table,
                    options,
                    module_stem,
                )?,
                None => {
                    nw_lua::decompile_with_options_and_module_stem(bytes, options, module_stem)?
                }
            };
            if annotate {
                let disassembly = match table {
                    Some(table) => nw_lua::disassemble_with(bytes, table)?,
                    None => nw_lua::disassemble(bytes)?,
                };
                Ok(annotate_source(&disassembly, &source))
            } else {
                Ok(source)
            }
        }
    }
}

fn annotate_source(disassembly: &str, source: &str) -> String {
    let mut out = String::from("-- disassembly annotations\n");
    for line in disassembly.lines() {
        if line.is_empty() {
            out.push_str("--\n");
        } else {
            out.push_str("-- ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push('\n');
    out.push_str(source);
    out
}

fn load_opcode_table(cli: &Cli) -> Result<Option<OpcodeTable>, CliError> {
    let Some(path) = &cli.opcode_table else {
        return Ok(cli
            .lua_version
            .map(LuaVersionArg::target)
            .map(OpcodeTable::builtin));
    };

    let text = fs::read_to_string(path).map_err(|source| CliError::Io {
        context: format!("read opcode table {}", path.display()),
        source,
    })?;
    let table = OpcodeTable::from_custom_text(&text)?;

    if let Some(version) = cli.lua_version.map(LuaVersionArg::target)
        && version != table.version
    {
        return Err(CliError::Message(format!(
            "opcode table version {} does not match --lua-version {}",
            table.version, version
        )));
    }

    Ok(Some(table))
}

fn read_input(path: &Path) -> Result<Vec<u8>, CliError> {
    if is_dash(path) {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|source| CliError::Io {
                context: "read stdin".to_string(),
                source,
            })?;
        Ok(bytes)
    } else {
        fs::read(path).map_err(|source| CliError::Io {
            context: format!("read {}", path.display()),
            source,
        })
    }
}

fn guard_outputs(paths: &[PathBuf], force: bool) -> Result<(), CliError> {
    for path in paths {
        guard_output(path, force)?;
    }
    Ok(())
}

fn guard_output(path: &Path, force: bool) -> Result<(), CliError> {
    if !force && path.exists() {
        return Err(CliError::Message(format!(
            "output already exists: {} (use --force to replace it)",
            path.display()
        )));
    }
    Ok(())
}

fn write_output(
    path: Option<&Path>,
    output: &str,
    force: bool,
    dry_run: bool,
) -> Result<(), CliError> {
    match path {
        Some(path) if !is_dash(path) => {
            guard_output(path, force)?;
            if dry_run {
                return Ok(());
            }
            fs::write(path, output).map_err(|source| CliError::Io {
                context: format!("write {}", path.display()),
                source,
            })
        }
        _ if dry_run => Ok(()),
        _ => io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .map_err(|source| CliError::Io {
                context: "write stdout".to_string(),
                source,
            }),
    }
}

fn is_dash(path: &Path) -> bool {
    path.as_os_str() == OsStr::new("-")
}

fn input_module_stem(path: &Path) -> Option<String> {
    if is_dash(path) {
        return None;
    }
    path.file_stem()
        .and_then(OsStr::to_str)
        .filter(|stem| !stem.is_empty())
        .map(str::to_string)
}

fn version_label_byte(byte: u8) -> String {
    LuaVersion::from_byte(byte)
        .map_or_else(|| format!("0x{byte:02x}"), |version| version.to_string())
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::Io { context, source } => write!(f, "{context}: {source}"),
            Self::Lua(LuaError::UnsupportedVersion(byte)) => write!(
                f,
                "unsupported Lua version {}; only Lua 5.1 is supported in this phase",
                version_label_byte(*byte)
            ),
            Self::Lua(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Lua(error) => Some(error),
            Self::Message(_) => None,
        }
    }
}

impl From<LuaError> for CliError {
    fn from(error: LuaError) -> Self {
        Self::Lua(error)
    }
}
