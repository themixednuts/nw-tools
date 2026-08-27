use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use nw_lua::{DecompOptions, decompile_with_options_and_module_stem};

use super::ast_sig;
use super::compare;
use super::lex;
use super::report::Summary;

#[derive(Debug, Clone)]
pub struct Config {
    pub luac: PathBuf,
    pub roots: Vec<PathBuf>,
    pub limit: usize,
    pub examples: usize,
}

pub fn run(config: &Config) -> Result<Summary, String> {
    if !config.luac.exists() {
        return Err(format!("luac not found: {}", config.luac.display()));
    }

    let mut files = collect_lua_files(&config.roots)?;
    files.sort();
    let total_files_seen = files.len();
    if config.limit != 0 {
        files.truncate(config.limit);
    }

    let mut summary = Summary {
        roots: config.roots.clone(),
        limit: config.limit,
        total_files_seen,
        ..Summary::default()
    };

    for file in files {
        match process_file(config, &file) {
            Ok(diff) => summary.add_diff(diff),
            Err(ProcessError::SourceCompile) => summary.source_compile_errors += 1,
            Err(ProcessError::Decompile) => summary.decompile_errors += 1,
            Err(ProcessError::Parse) => summary.parse_errors += 1,
            Err(ProcessError::Io(error)) => return Err(error),
        }
    }

    Ok(summary)
}

fn process_file(config: &Config, path: &Path) -> Result<compare::FileDiff, ProcessError> {
    let source = fs::read_to_string(path)
        .map_err(|error| ProcessError::Io(format!("failed to read {}: {error}", path.display())))?;

    let bytecode = compile_source(&config.luac, path)?;
    let stem = path.file_stem().and_then(|stem| stem.to_str());
    let decompiled =
        decompile_with_options_and_module_stem(&bytecode, DecompOptions::default(), stem)
            .map_err(|_| ProcessError::Decompile)?;

    let original_ast = full_moon::parse(&source).map_err(|_| ProcessError::Parse)?;
    let decompiled_ast = full_moon::parse(&decompiled).map_err(|_| ProcessError::Parse)?;

    let original = ast_sig::signature(&original_ast);
    let decompiled_sig = ast_sig::signature(&decompiled_ast);
    let lex = lex::scan_pair(&source, &decompiled);

    Ok(compare::compare_file(
        path.to_path_buf(),
        &original,
        &decompiled_sig,
        lex,
    ))
}

fn compile_source(luac: &Path, source: &Path) -> Result<Vec<u8>, ProcessError> {
    let temp = TempFile::new(source)?;
    let output = Command::new(luac)
        .arg("-o")
        .arg(&temp.path)
        .arg(source)
        .output()
        .map_err(|error| {
            ProcessError::Io(format!(
                "failed to run {} for {}: {error}",
                luac.display(),
                source.display()
            ))
        })?;

    if !output.status.success() {
        return Err(ProcessError::SourceCompile);
    }

    fs::read(&temp.path).map_err(|error| {
        ProcessError::Io(format!(
            "failed to read temp bytecode {}: {error}",
            temp.path.display()
        ))
    })
}

fn collect_lua_files(roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for root in roots {
        collect_lua_files_inner(root, &mut files)?;
    }
    Ok(files)
}

fn collect_lua_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("corpus root does not exist: {}", path.display()));
    }
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "lua") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }

    for entry in fs::read_dir(path)
        .map_err(|error| format!("failed to read dir {}: {error}", path.display()))?
    {
        let entry = entry
            .map_err(|error| format!("failed to read dir entry in {}: {error}", path.display()))?;
        collect_lua_files_inner(&entry.path(), files)?;
    }
    Ok(())
}

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(source: &Path) -> Result<Self, ProcessError> {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| ProcessError::Io(format!("system clock before epoch: {error}")))?
            .as_nanos();
        let stem = source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("source");
        path.push(format!("nw-lua-fidelity-{stem}-{nanos}.luac"));
        Ok(Self { path })
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

enum ProcessError {
    SourceCompile,
    Decompile,
    Parse,
    Io(String),
}
