use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fmt, fs,
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    process,
};

use clap::{ArgAction, ArgGroup, Parser};
use nw_jobs::JobRunner;
use nw_lua::{
    LuaError,
    bytecode::OpcodeTable,
    version::{LuaTarget, LuaVersion},
};

const MAX_AUTOMATIC_JOBS: usize = 8;

#[derive(Debug, Parser)]
#[command(
    name = "nw-lua",
    version,
    about = "Disassemble or decompile Lua binary chunks",
    override_usage = "nw-lua [options] <file.luac>...",
    group(ArgGroup::new("mode").args(["dis", "dec", "ssa_dump"]).multiple(false))
)]
struct Cli {
    #[arg(long, action = ArgAction::SetTrue, help = "disassemble bytecode")]
    dis: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "decompile to Lua source (DEFAULT when no mode given)"
    )]
    dec: bool,
    #[arg(long, action = ArgAction::SetTrue, help = "dump SSA IR for all protos")]
    ssa_dump: bool,
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
        value_name = "VER",
        value_parser = parse_lua_version,
        help = "override the detected complete compiler target (currently 51)"
    )]
    lua_version: Option<LuaTarget>,
    #[arg(
        long,
        value_name = "F",
        help = "load a custom opcode-table mapping from file F"
    )]
    opcode_table: Option<PathBuf>,
    #[arg(long, action = ArgAction::SetTrue, help = "emit debug trace to stderr")]
    debug: bool,
    #[arg(
        short = 'j',
        long,
        value_name = "N",
        default_value_t = default_jobs(),
        value_parser = parse_jobs,
        help = "maximum parallel workers for multiple input files"
    )]
    jobs: usize,
    #[arg(
        short,
        long,
        value_name = "F",
        help = "write one result to file F, or multiple results under directory F"
    )]
    output: Option<PathBuf>,
    #[arg(value_name = "file.luac", allow_hyphen_values = true, num_args = 1..)]
    inputs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Disassemble,
    Decompile,
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
    let mode = cli.mode();

    validate_cli(&cli, mode)?;
    let table = load_opcode_table(&cli)?;
    if cli.inputs.len() == 1 {
        run_single(&cli, mode, table.as_ref())
    } else {
        run_batch(&cli, mode, table.as_ref())
    }
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
        if cli.output.as_deref().is_none_or(is_dash) {
            return Err(CliError::Message(
                "multiple input files require an output directory with --output".to_string(),
            ));
        }
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
    write_output(cli.output.as_deref(), &output)?;
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
    let output_dir = cli.output.as_deref().expect("validated output directory");
    fs::create_dir_all(output_dir).map_err(|source| CliError::Io {
        context: format!("create output directory {}", output_dir.display()),
        source,
    })?;
    let outputs = batch_output_paths(&cli.inputs, output_dir, mode)?;
    let jobs = cli.jobs.min(cli.inputs.len());
    let runner = JobRunner::with_workers(jobs)
        .map_err(|error| CliError::Message(format!("create {jobs}-worker pool: {error}")))?;

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
    write_output(Some(output), &rendered)?;
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

impl Cli {
    fn mode(&self) -> Mode {
        if self.dis {
            Mode::Disassemble
        } else if self.ssa_dump {
            Mode::SsaDump
        } else {
            Mode::Decompile
        }
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
        return Ok(cli.lua_version.map(OpcodeTable::builtin));
    };

    let text = fs::read_to_string(path).map_err(|source| CliError::Io {
        context: format!("read opcode table {}", path.display()),
        source,
    })?;
    let table = OpcodeTable::from_custom_text(&text)?;

    if let Some(version) = cli.lua_version
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

fn write_output(path: Option<&Path>, output: &str) -> Result<(), CliError> {
    match path {
        Some(path) if !is_dash(path) => fs::write(path, output).map_err(|source| CliError::Io {
            context: format!("write {}", path.display()),
            source,
        }),
        _ => io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .map_err(|source| CliError::Io {
                context: "write stdout".to_string(),
                source,
            }),
    }
}

fn parse_lua_version(value: &str) -> Result<LuaTarget, String> {
    let version = match value {
        "51" => LuaVersion::V51,
        "52" => LuaVersion::V52,
        "53" => LuaVersion::V53,
        "54" => LuaVersion::V54,
        "55" => LuaVersion::V55,
        _ => return Err("expected one of 51, 52, 53, 54, 55".to_string()),
    };
    LuaTarget::for_version(version)
        .map_err(|_| format!("unsupported Lua version {version}; only Lua 5.1 is supported"))
}

fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(MAX_AUTOMATIC_JOBS)
}

fn parse_jobs(value: &str) -> Result<usize, String> {
    match value.parse() {
        Ok(jobs) if jobs > 0 => Ok(jobs),
        Ok(_) => Err("worker count must be greater than zero".to_string()),
        Err(error) => Err(format!("invalid worker count: {error}")),
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
