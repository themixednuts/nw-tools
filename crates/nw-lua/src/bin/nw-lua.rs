use std::{
    ffi::OsStr,
    fmt, fs,
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    process,
};

use clap::{ArgAction, ArgGroup, Parser};
use nw_lua::{LuaError, bytecode::OpcodeTable, version::LuaVersion};

#[derive(Debug, Parser)]
#[command(
    name = "nw-lua",
    version,
    about = "Disassemble or decompile Lua binary chunks",
    override_usage = "nw-lua [options] <file.luac>",
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
        help = "override detected version: 51|52|53|54|55 (only 51 supported now)"
    )]
    lua_version: Option<LuaVersion>,
    #[arg(
        long,
        value_name = "F",
        help = "load a custom opcode-table mapping from file F"
    )]
    opcode_table: Option<PathBuf>,
    #[arg(long, action = ArgAction::SetTrue, help = "emit debug trace to stderr")]
    debug: bool,
    #[arg(
        short,
        long,
        value_name = "F",
        help = "write to file F instead of stdout"
    )]
    output: Option<PathBuf>,
    #[arg(value_name = "file.luac", allow_hyphen_values = true)]
    input: PathBuf,
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

    if let Some(version) = cli.lua_version {
        ensure_supported_version(version)?;
    }

    let bytes = read_input(&cli.input)?;
    let table = load_opcode_table(&cli)?;

    if cli.debug {
        eprintln!("nw-lua: mode={mode:?}");
        eprintln!("nw-lua: input={} bytes", bytes.len());
        match &table {
            Some(table) => eprintln!(
                "nw-lua: opcode-table version={} opcodes={}",
                version_label(table.version),
                table.map.len()
            ),
            None => eprintln!("nw-lua: opcode-table=detected builtin"),
        }
    }

    let module_stem = input_module_stem(&cli.input);
    let output = render(
        &bytes,
        mode,
        cli.annotate,
        cli.no_idiomatic,
        table.as_ref(),
        module_stem.as_deref(),
    )?;
    write_output(cli.output.as_deref(), &output)?;
    Ok(())
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
        return cli
            .lua_version
            .map(OpcodeTable::builtin)
            .transpose()
            .map_err(CliError::Lua);
    };

    let text = fs::read_to_string(path).map_err(|source| CliError::Io {
        context: format!("read opcode table {}", path.display()),
        source,
    })?;
    let table = OpcodeTable::from_custom_text(&text)?;
    ensure_supported_version(table.version)?;

    if let Some(version) = cli.lua_version
        && version != table.version
    {
        return Err(CliError::Message(format!(
            "opcode table version {} does not match --lua-version {}",
            version_label(table.version),
            version_label(version)
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

fn parse_lua_version(value: &str) -> Result<LuaVersion, String> {
    match value {
        "51" => Ok(LuaVersion::V51),
        "52" => Ok(LuaVersion::V52),
        "53" => Ok(LuaVersion::V53),
        "54" => Ok(LuaVersion::V54),
        "55" => Ok(LuaVersion::V55),
        _ => Err("expected one of 51, 52, 53, 54, 55".to_string()),
    }
}

fn ensure_supported_version(version: LuaVersion) -> Result<(), CliError> {
    if version == LuaVersion::V51 {
        Ok(())
    } else {
        Err(CliError::Message(format!(
            "unsupported Lua version {}; only Lua 5.1 is supported in this phase",
            version_label(version)
        )))
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

fn version_label(version: LuaVersion) -> &'static str {
    match version {
        LuaVersion::V51 => "5.1",
        LuaVersion::V52 => "5.2",
        LuaVersion::V53 => "5.3",
        LuaVersion::V54 => "5.4",
        LuaVersion::V55 => "5.5",
    }
}

fn version_label_byte(byte: u8) -> String {
    LuaVersion::from_byte(byte).map_or_else(
        || format!("0x{byte:02x}"),
        |version| version_label(version).to_string(),
    )
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
