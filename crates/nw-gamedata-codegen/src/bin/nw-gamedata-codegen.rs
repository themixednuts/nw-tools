use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use nw_gamedata_codegen::{
    GameDataCodegenOutput, GameDataCompileMode, GameDataCompiler, GameDataCompilerOptions,
    GameDataEmitter, GameDataTargetLanguage, GameDataTargetPlan, GoSourceEmitter,
    RustSourceEmitter, TypeScriptSourceEmitter, load_catalog_from_asset_root,
};

#[derive(Debug, Parser)]
#[command(
    name = "nw-gamedata-codegen",
    about = "Emit self-contained New World GameData projects from shipping assets",
    version
)]
struct Cli {
    /// New World asset root containing shipping `.pak` files or loose datasheets.
    #[arg(long)]
    assets: PathBuf,

    /// Table typing mode: strict semantic affinity or shipping datasheet cell kinds.
    #[arg(long, value_enum, default_value_t = CliCompileMode::SourceFormat)]
    mode: CliCompileMode,

    /// Output root. Per-language projects are emitted under rust/, typescript/, and go/.
    #[arg(long, alias = "standalone-output")]
    output: PathBuf,

    /// Language to emit. Defaults to all supported standalone languages.
    #[arg(long, value_enum, alias = "standalone-language")]
    language: Vec<CliTargetLanguage>,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum CliCompileMode {
    Strict,
    #[default]
    SourceFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliTargetLanguage {
    Rust,
    #[value(name = "typescript", alias = "type-script")]
    TypeScript,
    Go,
}

impl From<CliCompileMode> for GameDataCompileMode {
    fn from(mode: CliCompileMode) -> Self {
        match mode {
            CliCompileMode::Strict => Self::Strict,
            CliCompileMode::SourceFormat => Self::SourceFormat,
        }
    }
}

impl From<CliTargetLanguage> for GameDataTargetLanguage {
    fn from(language: CliTargetLanguage) -> Self {
        match language {
            CliTargetLanguage::Rust => Self::Rust,
            CliTargetLanguage::TypeScript => Self::TypeScript,
            CliTargetLanguage::Go => Self::Go,
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let mode = GameDataCompileMode::from(cli.mode);
    let languages = target_languages(&cli);
    let targets = languages
        .iter()
        .copied()
        .map(GameDataTargetPlan::standalone);
    let options = GameDataCompilerOptions::with_targets(mode, targets)?;
    let compiler = GameDataCompiler::with_options(options);
    let catalog = load_catalog_from_asset_root(&cli.assets)?;
    let unit = compiler.compile_unit(&catalog);

    for language in languages {
        let target = GameDataTargetPlan::standalone(language);
        let output = emit_target(&unit, target)?;
        let root = cli.output.join(language_output_dir(language));
        let files = write_codegen_output(&output, &root)?;
        println!(
            "emitted {language:?} standalone project: {files} files under {}",
            root.display()
        );
    }

    Ok(())
}

fn target_languages(cli: &Cli) -> Vec<GameDataTargetLanguage> {
    if cli.language.is_empty() {
        return vec![
            GameDataTargetLanguage::Rust,
            GameDataTargetLanguage::TypeScript,
            GameDataTargetLanguage::Go,
        ];
    }
    cli.language
        .iter()
        .copied()
        .map(GameDataTargetLanguage::from)
        .collect()
}

fn emit_target(
    unit: &nw_gamedata_codegen::GameDataCompileUnit,
    target: GameDataTargetPlan,
) -> Result<GameDataCodegenOutput> {
    match target.language() {
        GameDataTargetLanguage::Rust => RustSourceEmitter::new(target)?.emit(unit),
        GameDataTargetLanguage::TypeScript => TypeScriptSourceEmitter::new(target)?.emit(unit),
        GameDataTargetLanguage::Go => GoSourceEmitter::new(target)?.emit(unit),
    }
}

fn language_output_dir(language: GameDataTargetLanguage) -> &'static str {
    match language {
        GameDataTargetLanguage::Rust => "rust",
        GameDataTargetLanguage::TypeScript => "typescript",
        GameDataTargetLanguage::Go => "go",
    }
}

fn write_codegen_output(output: &GameDataCodegenOutput, output_root: &Path) -> Result<usize> {
    let mut files = 0usize;
    for file in output.files() {
        remove_flat_module_directory_collision(output_root, file.path())?;
        let path = output_root.join(file.path());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(&path, file.contents_bytes())
            .with_context(|| format!("write {}", path.display()))?;
        files += 1;
    }
    Ok(files)
}

fn remove_flat_module_directory_collision(output_root: &Path, relative_path: &Path) -> Result<()> {
    if relative_path.parent() != Some(Path::new("src"))
        || relative_path
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("rs")
    {
        return Ok(());
    }

    let Some(module_name) = relative_path.file_stem().and_then(|stem| stem.to_str()) else {
        return Ok(());
    };
    if module_name == "lib" {
        return Ok(());
    }

    let src_root = output_root.join("src");
    let module_dir = src_root.join(module_name);
    if !module_dir.is_dir() {
        return Ok(());
    }

    ensure_existing_child_path(&src_root, &module_dir)?;
    fs::remove_dir_all(&module_dir).with_context(|| {
        format!(
            "remove stale generated module directory {} before writing {}",
            module_dir.display(),
            output_root.join(relative_path).display()
        )
    })?;
    Ok(())
}

fn ensure_existing_child_path(parent: &Path, child: &Path) -> Result<()> {
    let parent = parent
        .canonicalize()
        .with_context(|| format!("canonicalize {}", parent.display()))?;
    let child = child
        .canonicalize()
        .with_context(|| format!("canonicalize {}", child.display()))?;
    if !child.starts_with(&parent) {
        bail!(
            "refusing to remove `{}` because it is outside `{}`",
            child.display(),
            parent.display()
        );
    }
    Ok(())
}
