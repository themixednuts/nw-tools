use std::ffi::OsStr;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{ArgGroup, Args};
use humansize::{DECIMAL, format_size};
use nw_lua::{bytecode::OpcodeTable, version::LuaVersion};

use crate::jobs::{JobArgs, RunCtx};
use crate::support::{collect_matching, ensure_parent, guard_existing, write_guarded};
use crate::ui::{Cell, Report, Table};

use super::common::{finish_scan, path_label, strip_suffix_ignore_ascii_case};

#[derive(Debug, Args)]
#[command(group(ArgGroup::new("mode").args(["dis", "dec", "ssa_dump"]).multiple(false)))]
pub struct Lua {
    /// Lua bytecode file or directory.
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Disassemble bytecode.
    #[arg(long)]
    dis: bool,

    /// Decompile to Lua source (default).
    #[arg(long)]
    dec: bool,

    /// Dump SSA IR for all prototypes.
    #[arg(long)]
    ssa_dump: bool,

    /// Output file for a single input, or output directory for a batch.
    #[arg(long, value_name = "PATH")]
    out: Option<PathBuf>,

    /// Replace existing outputs.
    #[arg(long)]
    overwrite: bool,

    /// Override detected version: 51|52|53|54|55 (only 51 supported now).
    #[arg(long, value_name = "VER", value_parser = parse_lua_version)]
    lua_version: Option<LuaVersion>,

    /// Load a custom opcode-table mapping from file F.
    #[arg(long, value_name = "F")]
    opcode_table: Option<PathBuf>,

    /// Skip idiomatic AST cleanup during decompilation.
    #[arg(long)]
    no_idiomatic: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LuaMode {
    Disassemble,
    Decompile,
    SsaDump,
}

#[derive(Debug, Clone, Copy)]
struct LuaRender<'a> {
    mode: LuaMode,
    no_idiomatic: bool,
    table: Option<&'a OpcodeTable>,
}

#[derive(Debug, Clone)]
struct LuaOutput {
    source: String,
    output: String,
    bytes: String,
}

impl Lua {
    pub(super) fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let root = self
            .path
            .as_deref()
            .context("format lua needs a .luac file or directory path")?;
        let mode = self.mode()?;
        let table = load_opcode_table(self.lua_version, self.opcode_table.as_deref())?;
        let render = LuaRender {
            mode,
            no_idiomatic: self.no_idiomatic,
            table: table.as_ref(),
        };
        let paths = collect_matching(root, is_luac_path)?;

        if paths.is_empty() {
            Report::new("lua")
                .stat("files", 0usize)
                .stat(mode.done_label(), 0usize)
                .stat("errors", 0usize)
                .note("no Lua bytecode files found")
                .print();
            return Ok(());
        }

        if self.out.is_none() {
            if root.is_file() && paths.len() == 1 {
                let output = render_file(&paths[0], render)?;
                std::io::stdout()
                    .lock()
                    .write_all(output.as_bytes())
                    .context("write stdout")?;
                return Ok(());
            }
            bail!("--out is required when processing multiple Lua bytecode files");
        }

        write_batch(
            &ctx,
            root,
            &paths,
            self.out.as_deref().expect("--out checked above"),
            self.overwrite,
            render,
        )
    }

    fn mode(&self) -> Result<LuaMode> {
        let mode = if self.dis {
            LuaMode::Disassemble
        } else if self.ssa_dump {
            LuaMode::SsaDump
        } else {
            let _ = self.dec;
            LuaMode::Decompile
        };
        if self.no_idiomatic && mode != LuaMode::Decompile {
            bail!("--no-idiomatic can only be used with decompilation");
        }
        Ok(mode)
    }
}

impl LuaMode {
    const fn done_label(self) -> &'static str {
        match self {
            Self::Disassemble => "disassembled",
            Self::Decompile => "decompiled",
            Self::SsaDump => "ssa dumps",
        }
    }

    const fn output_suffix(self) -> &'static str {
        match self {
            Self::Disassemble => ".dis.txt",
            Self::Decompile => ".lua",
            Self::SsaDump => ".ssa.txt",
        }
    }
}

fn write_batch(
    ctx: &RunCtx,
    root: &Path,
    paths: &[PathBuf],
    out: &Path,
    overwrite: bool,
    render: LuaRender<'_>,
) -> Result<()> {
    let batch = ctx.map_results_compact(
        "lua",
        paths,
        |path| path_label(path),
        |path, progress| progress.step(|| decompile_one(root, path, out, overwrite, render)),
    );
    let skipped = batch.skipped();
    let cancelled = batch.was_cancelled();
    let mut written = Vec::new();
    let mut errors = Vec::new();

    for result in batch.into_completed() {
        match result {
            Ok(row) => written.push(row),
            Err(error) => errors.push(error),
        }
    }
    written.sort_by(|left, right| left.source.cmp(&right.source));

    let mut report = Report::new("lua")
        .stat("files", paths.len())
        .stat(render.mode.done_label(), written.len())
        .stat("errors", errors.len());
    let mut table = Table::new(["Source", "Output", "Bytes"]).right([2]);
    for row in written {
        table.push([
            Cell::path(row.source),
            Cell::path(row.output),
            Cell::size(row.bytes),
        ]);
    }
    report.table_or(table, "no Lua bytecode files written");
    report.print();

    finish_scan(cancelled, skipped, &errors, "lua")
}

fn decompile_one(
    root: &Path,
    path: &Path,
    out: &Path,
    overwrite: bool,
    render: LuaRender<'_>,
) -> Result<LuaOutput> {
    let output = lua_output_path(root, path, out, render.mode)?;
    let text = render_file(path, render)?;
    write_lua_text(&output, &text, overwrite)?;

    Ok(LuaOutput {
        source: path_label(path),
        output: output.display().to_string(),
        bytes: format_size(text.len(), DECIMAL),
    })
}

fn write_lua_text(path: &Path, text: &str, overwrite: bool) -> Result<()> {
    guard_existing(path, overwrite.into())?;
    ensure_parent(path)?;
    write_guarded(path, text.as_bytes(), crate::support::Overwrite::Replace)
}

fn render_file(path: &Path, render: LuaRender<'_>) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    render_lua(&bytes, path, render).with_context(|| format!("render {}", path.display()))
}

fn render_lua(bytes: &[u8], path: &Path, render: LuaRender<'_>) -> Result<String> {
    let module_stem = module_stem(path);
    let options = if render.no_idiomatic {
        nw_lua::DecompOptions::core()
    } else {
        nw_lua::DecompOptions::default()
    };
    Ok(match render.mode {
        LuaMode::Disassemble => match render.table {
            Some(table) => nw_lua::disassemble_with(bytes, table)?,
            None => nw_lua::disassemble(bytes)?,
        },
        LuaMode::Decompile => match render.table {
            Some(table) => nw_lua::decompile_with_table_options_and_module_stem(
                bytes,
                table,
                options,
                module_stem.as_deref(),
            )?,
            None => nw_lua::decompile_with_options_and_module_stem(
                bytes,
                options,
                module_stem.as_deref(),
            )?,
        },
        LuaMode::SsaDump => match render.table {
            Some(table) => nw_lua::ssa_dump_with(bytes, table)?,
            None => nw_lua::ssa_dump(bytes)?,
        },
    })
}

fn load_opcode_table(
    lua_version: Option<LuaVersion>,
    opcode_table: Option<&Path>,
) -> Result<Option<OpcodeTable>> {
    if let Some(version) = lua_version {
        ensure_supported_version(version)?;
    }

    let Some(path) = opcode_table else {
        return lua_version
            .map(OpcodeTable::builtin)
            .transpose()
            .context("load Lua opcode table");
    };

    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let table = OpcodeTable::from_custom_text(&text).context("parse opcode table")?;
    ensure_supported_version(table.version)?;

    if let Some(version) = lua_version
        && version != table.version
    {
        bail!(
            "opcode table version {} does not match --lua-version {}",
            version_label(table.version),
            version_label(version)
        );
    }

    Ok(Some(table))
}

fn lua_output_path(root: &Path, source: &Path, out: &Path, mode: LuaMode) -> Result<PathBuf> {
    if root.is_file() {
        return Ok(out.to_path_buf());
    }

    let relative = source.strip_prefix(root).unwrap_or(source);
    let mut output = out.join(relative);
    let file_name = output
        .file_name()
        .and_then(OsStr::to_str)
        .with_context(|| format!("Lua output path has no file name: {}", output.display()))?;
    let stem = strip_suffix_ignore_ascii_case(file_name, ".luac").unwrap_or(file_name);
    output.set_file_name(format!("{stem}{}", mode.output_suffix()));
    Ok(output)
}

fn is_luac_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("luac"))
}

fn module_stem(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(OsStr::to_str)
        .filter(|stem| !stem.is_empty())
        .map(str::to_string)
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

fn ensure_supported_version(version: LuaVersion) -> Result<()> {
    if version == LuaVersion::V51 {
        Ok(())
    } else {
        bail!(
            "unsupported Lua version {}; only Lua 5.1 is supported in this phase",
            version_label(version)
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const SHOPCOMMON: &[u8] = include_bytes!("../../../nw-lua/tests/fixtures/shopcommon.luac");

    #[test]
    fn lua_decompile_smoke_produces_source() -> Result<()> {
        let output = render_lua(
            SHOPCOMMON,
            Path::new("shopcommon.luac"),
            LuaRender {
                mode: LuaMode::Decompile,
                no_idiomatic: false,
                table: None,
            },
        )?;

        assert!(output.trim().len() > 20);
        Ok(())
    }

    #[test]
    fn lua_batch_outputs_mirror_input_layout() -> Result<()> {
        let root = Path::new("in");
        let source = Path::new("in/scripts/ui/shopcommon.luac");
        let out = Path::new("out");

        assert_eq!(
            lua_output_path(root, source, out, LuaMode::Decompile)?,
            PathBuf::from("out/scripts/ui/shopcommon.lua")
        );
        assert_eq!(
            lua_output_path(root, source, out, LuaMode::Disassemble)?,
            PathBuf::from("out/scripts/ui/shopcommon.dis.txt")
        );
        assert_eq!(
            lua_output_path(root, source, out, LuaMode::SsaDump)?,
            PathBuf::from("out/scripts/ui/shopcommon.ssa.txt")
        );
        Ok(())
    }
}
