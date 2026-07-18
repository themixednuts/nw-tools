use crate::emit::GameDataCodegenFile;
use crate::project::{RUST_EDITION, RUST_VERSION};

use super::{RustSourceEmitError, format_rust_source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneProject {
    files: Vec<RustStandaloneProjectFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneProjectOptions {
    package_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneProjectFile {
    path: String,
    source: String,
}

impl Default for RustStandaloneProjectOptions {
    fn default() -> Self {
        Self {
            package_name: "newworld-gamedata".to_owned(),
        }
    }
}

impl super::RustSourceEmitter {
    pub fn emit_standalone_project(&self) -> Result<RustStandaloneProject, RustSourceEmitError> {
        self.emit_standalone_project_with_options(&RustStandaloneProjectOptions::default())
    }

    pub fn emit_standalone_project_with_options(
        &self,
        options: &RustStandaloneProjectOptions,
    ) -> Result<RustStandaloneProject, RustSourceEmitError> {
        let mut files = vec![
            RustStandaloneProjectFile::new(
                "Cargo.toml",
                cargo_manifest_source(options.package_name()),
            ),
            RustStandaloneProjectFile::new(
                ".cargo/config.toml",
                rust_standalone_cargo_config_source()?,
            ),
            RustStandaloneProjectFile::new("build.rs", rust_standalone_build_rs_source()?),
            RustStandaloneProjectFile::new("src/lib.rs", standalone_lib_rs_source()?),
        ];
        files.push(RustStandaloneProjectFile::new(
            "src/assets.rs",
            asset_facade_source()?,
        ));
        Ok(RustStandaloneProject { files })
    }
}

impl RustStandaloneProject {
    #[must_use]
    pub fn files(&self) -> &[RustStandaloneProjectFile] {
        &self.files
    }

    #[must_use]
    pub fn into_files(self) -> Vec<RustStandaloneProjectFile> {
        self.files
    }

    #[must_use]
    pub fn into_codegen_files(self) -> Vec<GameDataCodegenFile> {
        self.files
            .into_iter()
            .map(RustStandaloneProjectFile::into_codegen_file)
            .collect()
    }
}

impl RustStandaloneProjectOptions {
    #[must_use]
    pub fn new(package_name: impl Into<String>) -> Self {
        Self {
            package_name: package_name.into(),
        }
    }

    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }
}

impl RustStandaloneProjectFile {
    #[must_use]
    pub fn new(path: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn into_codegen_file(self) -> GameDataCodegenFile {
        GameDataCodegenFile::new(self.path, self.source)
    }
}

fn standalone_lib_rs_source() -> Result<String, RustSourceEmitError> {
    format_rust_source(
        "mod datasheet_catalog;\n\nmod assets;\n\
         pub use assets::{AssetCatalog, AssetId, AssetLoader, AssetReference, AssetType};\n\n\
         pub mod managers;\n\
         pub use managers::Managers;\n\n\
         pub use nw_datasheet::game_system::Crc32;\n\
         pub use glam::Vec3;\n\
         pub use uuid::Uuid;\n",
    )
}

fn cargo_manifest_source(package_name: &str) -> String {
    RUST_STANDALONE_CARGO_TOML
        .replace("{{PACKAGE_NAME}}", package_name)
        .replace("{{RUST_EDITION}}", RUST_EDITION)
        .replace("{{RUST_VERSION}}", RUST_VERSION)
        .replace(
            "{{RUST_STANDALONE_DEPENDENCIES}}",
            &rust_standalone_dependencies(),
        )
}

fn rust_standalone_dependencies() -> String {
    let dependencies = vec![
        "anyhow = \"1\"",
        "flate2 = \"1.1.9\"",
        "glam = \"0.32\"",
        "nw-asset = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-asset\", features = [\"oodle\"] }",
        "nw-datasheet = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-datasheet\" }",
        "nw-objectstream = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-objectstream\" }",
        "nw-pak = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-pak\", features = [\"oodle\"] }",
        "once_cell = \"1\"",
        "serde = { version = \"1\", features = [\"derive\"] }",
        "serde_json = \"1\"",
        "uuid = \"1.23.3\"",
    ];
    dependencies.join("\n")
}

fn rust_standalone_cargo_config_source() -> Result<String, RustSourceEmitError> {
    super::format_rust_source(
        r#"
"#,
    )?;
    Ok(r#"[env]
OODLE_LIB_DIR = { value = "bin", relative = true }
"#
    .to_owned())
}

fn rust_standalone_build_rs_source() -> Result<String, RustSourceEmitError> {
    super::format_rust_source(
        r#"
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let Some(manifest_dir) = env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from) else {
        println!("cargo:warning=CARGO_MANIFEST_DIR was not set; skipping Oodle DLL copy");
        return;
    };
    let bin_dir = manifest_dir.join("bin");
    println!("cargo:rerun-if-changed={}", bin_dir.display());
    println!("cargo:rustc-link-search={}", bin_dir.display());
    copy_runtime_dlls(&bin_dir);
}

fn copy_runtime_dlls(bin_dir: &Path) {
    let Some(profile_dir) = profile_dir() else {
        return;
    };
    for name in ["oo2core_win64.dll"] {
        let source = bin_dir.join(name);
        if !source.is_file() {
            continue;
        }
        copy_runtime_dll(&source, &profile_dir.join(name));
        let deps_dir = profile_dir.join("deps");
        if deps_dir.is_dir() {
            copy_runtime_dll(&source, &deps_dir.join(name));
        }
    }
}

fn copy_runtime_dll(source: &Path, target: &Path) {
    if let Err(error) = fs::copy(source, target) {
        println!(
            "cargo:warning=failed to copy {} to {}: {error}",
            source.display(),
            target.display()
        );
    }
}

fn profile_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR")?);
    out_dir.ancestors().nth(3).map(Path::to_path_buf)
}
"#,
    )
}

fn asset_facade_source() -> Result<String, RustSourceEmitError> {
    super::format_rust_source(
        r#"
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use nw_asset::{ASSET_CATALOG_PATH, Rasc};
use nw_pak::PakMmapReader as PakReader;

pub use nw_asset::{AssetCatalog, AssetId, AssetReference, AssetType};
use nw_asset::normalize_virtual_path;

#[derive(Clone)]
pub struct AssetLoader {
    inner: Arc<AssetLoaderInner>,
}

struct AssetLoaderInner {
    catalog: AssetCatalog,
    entries_by_path: HashMap<String, PakEntryRef>,
}

#[derive(Clone)]
struct PakEntryRef {
    reader: Arc<PakReader>,
    index: usize,
}

impl AssetLoader {
    /// Opens a New World asset directory and indexes every cataloged asset.
    pub fn open(asset_root: impl AsRef<Path>) -> Result<Self> {
        let asset_root = canonical_path(asset_root.as_ref())
            .with_context(|| format!("resolve asset root {}", asset_root.as_ref().display()))?;
        let pak_paths = collect_pak_paths(&asset_root)?;
        if pak_paths.is_empty() {
            bail!("no .pak files found under {}", asset_root.display());
        }

        let mut readers: Vec<(String, Arc<PakReader>)> = Vec::new();
        let mut entries_by_path: HashMap<String, PakEntryRef> = HashMap::new();
        let mut claimed_paths: HashSet<String> = HashSet::new();

        for pak_path in pak_paths {
            let reader = Arc::new(
                PakReader::open(&pak_path)
                    .with_context(|| format!("open pak {}", pak_path.display()))?,
            );
            let mount_root = pak_mount_root(&asset_root, &pak_path)?;
            for entry in reader.entries() {
                let path = normalize_data_path(&mounted_entry_path(&mount_root, entry.name()));
                if claimed_paths.insert(path.clone()) {
                    entries_by_path.insert(
                        path,
                        PakEntryRef {
                            reader: reader.clone(),
                            index: entry.index(),
                        },
                    );
                }
            }
            readers.push((mount_root, reader));
        }

        let catalog = load_catalog_from_paks(&readers)?;
        Ok(Self {
            inner: Arc::new(AssetLoaderInner {
                catalog,
                entries_by_path,
            }),
        })
    }

    pub fn catalog(&self) -> &AssetCatalog {
        &self.inner.catalog
    }

    pub fn read(&self, path: impl AsRef<str>) -> Result<Vec<u8>> {
        let path = path.as_ref();
        let normalized = normalize_data_path(path);
        let located = self
            .inner
            .entries_by_path
            .get(&normalized)
            .or_else(|| {
                let suffix = format!("/{normalized}");
                self.inner
                    .entries_by_path
                    .iter()
                    .find_map(|(candidate, located)| candidate.ends_with(&suffix).then_some(located))
            })
            .with_context(|| format!("asset {path} was not present in selected paks"))?;
        located
            .reader
            .read_by_index(located.index)
            .with_context(|| format!("read pak asset {path}"))
    }

}

fn load_catalog_from_paks(readers: &[(String, Arc<PakReader>)]) -> Result<AssetCatalog> {
    for (_mount_root, reader) in readers {
        if let Some(entry) = reader.entry(ASSET_CATALOG_PATH) {
            let bytes = reader
                .read_by_index(entry.index())
                .context("read asset catalog from pak")?;
            let rasc = Rasc::parse(&bytes).context("parse asset catalog")?;
            return Ok(AssetCatalog::new(rasc, None));
        }
    }
    bail!("asset catalog {ASSET_CATALOG_PATH} was not found in selected paks")
}

fn collect_pak_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_pak_paths_inner(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_pak_paths_inner(path: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let metadata =
        std::fs::symlink_metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if metadata.is_file() {
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pak"))
        {
            out.push(
                canonical_path(path).with_context(|| format!("resolve pak {}", path.display()))?,
            );
        }
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry.with_context(|| format!("read entry in {}", path.display()))?;
        collect_pak_paths_inner(&entry.path(), out)?;
    }
    Ok(())
}

fn pak_mount_root(asset_root: &Path, pak_path: &Path) -> Result<String> {
    let parent = pak_path.parent().unwrap_or(asset_root);
    let relative = parent.strip_prefix(asset_root).with_context(|| {
        format!(
            "pak {} is not under asset root {}",
            pak_path.display(),
            asset_root.display()
        )
    })?;
    Ok(normalize_data_path(&relative.to_string_lossy()))
}

fn canonical_path(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).with_context(|| format!("canonicalize {}", path.display()))
}

fn mounted_entry_path(mount_root: &str, entry_name: &str) -> String {
    if mount_root.is_empty() || mount_root == "." {
        entry_name.to_owned()
    } else {
        format!("{mount_root}/{entry_name}")
    }
}

fn normalize_data_path(path: &str) -> String {
    normalize_virtual_path(path)
}
"#,
    )
}

const RUST_STANDALONE_CARGO_TOML: &str = include_str!("../../../resources/rust/Cargo.toml.in");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_project_uses_current_rust_toolchain() {
        let emitter = crate::rust::source::RustSourceEmitter;
        let project = emitter
            .emit_standalone_project_with_options(&RustStandaloneProjectOptions::new(
                "newworld-gamedata-check",
            ))
            .expect("rust project");
        let cargo = project
            .files()
            .iter()
            .find(|file| file.path() == "Cargo.toml")
            .expect("Cargo.toml")
            .source();

        assert!(cargo.contains("name = \"newworld-gamedata-check\""));
        assert!(cargo.contains("edition = \"2024\""));
        assert!(cargo.contains("rust-version = \"1.96\""));
        assert!(!cargo.contains("[features]"));
        assert!(cargo.contains("anyhow = \"1\""));
        assert!(!cargo.contains("thiserror"));
        assert!(cargo.contains("uuid = \"1.23.3\""));
        assert!(cargo.contains(
            "nw-asset = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-asset\", features = [\"oodle\"] }"
        ));
        assert!(cargo.contains(
            "nw-datasheet = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-datasheet\" }"
        ));
        assert!(!cargo.contains("nw-filesystem"));
        assert!(!cargo.contains("nw-jobs"));
        assert!(!cargo.contains("nw-localization"));
        assert!(cargo.contains(
            "nw-objectstream = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-objectstream\" }"
        ));
        assert!(cargo.contains(
            "nw-pak = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-pak\", features = [\"oodle\"] }"
        ));
        assert!(!cargo.contains("az-core"));
        assert!(!cargo.contains("bevy ="));
        assert!(!cargo.contains("gamedata = { path"));
        assert!(!cargo.contains("newworld-plugin"));
        assert!(!cargo.contains("nw-items"));
        assert!(!cargo.contains("quick-xml"));
        assert!(!cargo.contains("ron ="));
        let cargo_config = project
            .files()
            .iter()
            .find(|file| file.path() == ".cargo/config.toml")
            .expect("cargo config")
            .source();
        assert!(cargo_config.contains("OODLE_LIB_DIR = { value = \"bin\", relative = true }"));
        let build_rs = project
            .files()
            .iter()
            .find(|file| file.path() == "build.rs")
            .expect("build.rs")
            .source();
        assert!(build_rs.contains("manifest_dir.join(\"bin\")"));
        assert!(build_rs.contains("env::var_os(\"CARGO_MANIFEST_DIR\").map(PathBuf::from)"));
        assert!(build_rs.contains("cargo:warning=failed to copy"));
        assert!(!build_rs.contains("let _ = fs::copy"));
        assert!(
            project
                .files()
                .iter()
                .all(|file| file.path() != "src/datasheet_catalog.rs")
        );
        assert!(
            project
                .files()
                .iter()
                .any(|file| file.path() == "src/assets.rs")
        );
        assert!(
            project
                .files()
                .iter()
                .all(|file| file.path() != "src/system.rs")
        );
        assert!(
            project
                .files()
                .iter()
                .all(|file| file.path() != "src/plugin.rs")
        );
        assert!(
            project
                .files()
                .iter()
                .all(|file| !file.path().starts_with("src/products/"))
        );
    }

    #[test]
    fn standalone_project_uses_typed_datasheet_mode() {
        let emitter = crate::rust::source::RustSourceEmitter;
        let project = emitter
            .emit_standalone_project_with_options(&RustStandaloneProjectOptions::new(
                "newworld-gamedata-check",
            ))
            .expect("rust project");
        let cargo = project
            .files()
            .iter()
            .find(|file| file.path() == "Cargo.toml")
            .expect("Cargo.toml")
            .source();

        assert!(!cargo.contains("[features]"));
        assert!(cargo.contains(
            "nw-datasheet = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-datasheet\" }"
        ));
        assert!(!cargo.contains("nw-filesystem"));
        assert!(!cargo.contains("nw-jobs"));
        assert!(!cargo.contains("nw-localization"));
        assert!(cargo.contains(
            "nw-objectstream = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-objectstream\" }"
        ));
        assert!(cargo.contains(
            "nw-pak = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-pak\", features = [\"oodle\"] }"
        ));
        assert!(!cargo.contains("quick-xml"));
        assert!(!cargo.contains("ron ="));
        let assets = project
            .files()
            .iter()
            .find(|file| file.path() == "src/assets.rs")
            .expect("assets facade")
            .source();
        assert!(assets.contains("pub use nw_asset::"));
        assert!(!assets.contains("pub use nw_datasheet::"));
        assert!(!assets.contains("pub use nw_jobs::"));
        assert!(!assets.contains("pub use nw_localization::"));
        assert!(!assets.contains("pub use nw_objectstream::"));
        assert!(!assets.contains("pub use nw_pak::"));
        assert!(assets.contains("pub fn open(asset_root: impl AsRef<Path>)"));
        assert!(!assets.contains("pub fn from_dir"));
        assert!(!assets.contains("PakDatasheetSource"));
        assert!(!assets.contains("datasheet_source"));
        assert!(!assets.contains("load_pak_datasheet_source"));
        assert!(assets.contains("std::fs::canonicalize(path)"));
        assert!(assets.contains("std::fs::symlink_metadata(path)"));
        assert!(assets.contains("pak {} is not under asset root {}"));
        assert!(
            project
                .files()
                .iter()
                .all(|file| !file.path().starts_with("src/assets/"))
        );
        let lib = project
            .files()
            .iter()
            .find(|file| file.path() == "src/lib.rs")
            .expect("lib")
            .source();
        assert!(!lib.contains("pub mod products;"));
    }
}
