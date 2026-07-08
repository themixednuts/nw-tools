use crate::emit::GameDataCodegenFile;
use crate::project::{RUST_EDITION, RUST_VERSION};
use crate::target::{GameDataDataFormat, GameDataProduct, GameDataTargetPlan};

use super::{RustSourceEmitError, format_rust_source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneProject {
    files: Vec<RustStandaloneProjectFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneProjectOptions {
    package_name: String,
    include_product_placeholders: bool,
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
            include_product_placeholders: true,
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
        let data_format = self.target.data_format();
        let mut files = vec![
            RustStandaloneProjectFile::new(
                "Cargo.toml",
                cargo_manifest_source(options.package_name(), data_format),
            ),
            RustStandaloneProjectFile::new(
                ".cargo/config.toml",
                rust_standalone_cargo_config_source()?,
            ),
            RustStandaloneProjectFile::new("build.rs", rust_standalone_build_rs_source()?),
            RustStandaloneProjectFile::new(
                "src/lib.rs",
                standalone_lib_rs_source(&self.target, data_format)?,
            ),
        ];
        if options.include_product_placeholders {
            if self.target.supports_product(GameDataProduct::TableManifest) {
                files.push(RustStandaloneProjectFile::new("src/table_manifest.rs", ""));
            }
        }
        if self
            .target
            .supports_product(GameDataProduct::GameAssetAccess)
        {
            files.push(RustStandaloneProjectFile::new(
                "src/assets.rs",
                asset_facade_source()?,
            ));
        }
        if options.include_product_placeholders
            && self.target.supports_product(GameDataProduct::Systems)
        {
            files.push(RustStandaloneProjectFile::new(
                "src/system.rs",
                standalone_system_source()?,
            ));
        }
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
            include_product_placeholders: true,
        }
    }

    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    #[must_use]
    pub const fn with_product_placeholders(mut self, include: bool) -> Self {
        self.include_product_placeholders = include;
        self
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

fn standalone_lib_rs_source(
    target: &GameDataTargetPlan,
    data_format: GameDataDataFormat,
) -> Result<String, RustSourceEmitError> {
    let mut source =
        String::from("#![allow(clippy::struct_excessive_bools, dead_code, unused_mut)]\n\n");
    if target.supports_product(GameDataProduct::TableManifest) {
        source.push_str("mod table_manifest;\n");
    }
    if target.supports_product(GameDataProduct::GameAssetAccess) {
        source.push_str("\npub mod assets;\n");
        source.push_str("pub use assets::AssetLoader;\n");
    }
    if target.supports_product(GameDataProduct::Systems)
        && matches!(data_format, GameDataDataFormat::Datasheet)
    {
        source.push_str("\npub mod system;\n");
    }
    if target.supports_product(GameDataProduct::SemanticManagers) {
        source.push_str("\npub mod managers;\n");
        source.push_str("pub use managers::Managers;\n");
    }
    format_rust_source(&source)
}

fn cargo_manifest_source(package_name: &str, data_format: GameDataDataFormat) -> String {
    RUST_STANDALONE_CARGO_TOML
        .replace("{{PACKAGE_NAME}}", package_name)
        .replace("{{RUST_EDITION}}", RUST_EDITION)
        .replace("{{RUST_VERSION}}", RUST_VERSION)
        .replace(
            "{{RUST_STANDALONE_DEPENDENCIES}}",
            &rust_standalone_dependencies(data_format),
        )
}

fn rust_standalone_dependencies(data_format: GameDataDataFormat) -> String {
    let mut dependencies = vec![
        "anyhow = \"1\"",
        "flate2 = \"1.1.9\"",
        "nw-asset = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-asset\", features = [\"oodle\"] }",
        "nw-datasheet = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-datasheet\" }",
        "nw-filesystem = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-filesystem\" }",
        "nw-jobs = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-jobs\" }",
        "nw-localization = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-localization\" }",
        "nw-objectstream = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-objectstream\" }",
        "nw-pak = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-pak\", features = [\"oodle\"] }",
        "serde = { version = \"1\", features = [\"derive\"] }",
        "serde_json = \"1\"",
        "thiserror = \"2\"",
    ];
    if matches!(data_format, GameDataDataFormat::Ron) {
        dependencies.insert(5, "ron = \"0.12.1\"");
    }
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
    for name in ["oo2core_win64.dll", "oo2core_9_win64.dll", "oo2core_8_win64.dll"] {
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

pub mod asset {
    pub use nw_asset::AssetId;
}

pub use nw_asset::{
    AssetCatalog, AssetId, AssetInfo, AssetStore, AssetStoreError, AssetType,
    normalize_virtual_path,
};
pub use nw_datasheet::{Cell, CellValue, Column, ColumnType, Datasheet, Row};
pub use nw_filesystem::{
    SafeJoinError, archive_extension_key, display_relative, normalize_archive_path, safe_join,
};
pub use nw_jobs::{
    CancellationToken, JobBatch, JobRunner, JobRunnerBuildError, JobRunnerPolicy,
};
pub use nw_localization::{LanguageCode, LocalizationError, LocalizedTextResolver};
pub use nw_objectstream::{ObjectStream, ObjectStreamEncoding, ObjectStreamError};
pub use nw_pak::{EntryInfo, PakArchive, PakError, PakFile, PakFileMmap, PakMmapReader};

#[derive(Debug, Clone)]
pub(crate) struct DatasheetAsset {
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct BinaryAsset {
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct PakDatasheetSource {
    pub(crate) catalog: AssetCatalog,
    pub(crate) datasheets: Vec<DatasheetAsset>,
    pub(crate) assets: Vec<BinaryAsset>,
}

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
    pub fn from_dir(asset_root: impl AsRef<Path>) -> Result<Self> {
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

    pub(crate) fn datasheet_source(&self) -> Result<PakDatasheetSource> {
        let mut datasheets = Vec::new();
        let mut assets = Vec::new();
        for entry in self.inner.catalog.entries() {
            let path = normalize_data_path(entry.path());
            if !is_datasheet_path(&path) && !is_manager_asset_path(&path) {
                continue;
            }
            let bytes = self.read(&path)?;
            if is_datasheet_path(&path) {
                datasheets.push(DatasheetAsset { path, bytes });
            } else {
                assets.push(BinaryAsset { path, bytes });
            }
        }

        Ok(PakDatasheetSource {
            catalog: self.inner.catalog.clone(),
            datasheets,
            assets,
        })
    }
}

pub(crate) fn load_pak_datasheet_source(asset_root: impl AsRef<Path>) -> Result<PakDatasheetSource> {
    AssetLoader::from_dir(asset_root)?.datasheet_source()
}

pub fn is_manager_asset_path(path: &str) -> bool {
    let normalized = normalize_data_path(path);
    if normalized == "libs/camera/gamecamera.xml" {
        return true;
    }
    matches!(
        path_extension(&normalized).as_deref(),
        Some(
            "aoffdb" | "equipdb" | "gds" | "uidb" | "pbadb" | "sprd" | "gdb" | "gactdb"
            | "rankdb" | "craftstationdb",
        )
    )
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

fn is_datasheet_path(path: &str) -> bool {
    path_extension(path).is_some_and(|extension| extension == "datasheet")
}

fn path_extension(path: &str) -> Option<String> {
    path.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
}

fn normalize_data_path(path: &str) -> String {
    normalize_virtual_path(path)
}
"#,
    )
}

pub(super) fn standalone_system_source() -> Result<String, RustSourceEmitError> {
    super::format_rust_source(
        r#"
//! Runtime system integration for the standalone datasheet GameData package.
//!
//! The standalone package loads shipping datasheets and manager resources from
//! game assets. System-specific code is emitted here only after the
//! system behavior is mapped.
"#,
    )
}

const RUST_STANDALONE_CARGO_TOML: &str = include_str!("../../../resources/rust/Cargo.toml.in");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_project_uses_current_rust_toolchain() {
        let emitter = crate::rust::source::RustSourceEmitter::standalone();
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
        assert!(cargo.contains("thiserror = \"2\""));
        assert!(cargo.contains(
            "nw-asset = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-asset\", features = [\"oodle\"] }"
        ));
        assert!(cargo.contains(
            "nw-datasheet = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-datasheet\" }"
        ));
        assert!(cargo.contains(
            "nw-filesystem = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-filesystem\" }"
        ));
        assert!(cargo.contains(
            "nw-jobs = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-jobs\" }"
        ));
        assert!(cargo.contains(
            "nw-localization = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-localization\" }"
        ));
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
                .any(|file| file.path() == "src/table_manifest.rs")
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
                .any(|file| file.path() == "src/system.rs")
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
        let emitter = crate::rust::source::RustSourceEmitter::standalone();
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
        assert!(cargo.contains(
            "nw-filesystem = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-filesystem\" }"
        ));
        assert!(cargo.contains(
            "nw-jobs = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-jobs\" }"
        ));
        assert!(cargo.contains(
            "nw-localization = { git = \"https://github.com/themixednuts/nw-tools\", package = \"nw-localization\" }"
        ));
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
        assert!(assets.contains("pub use nw_datasheet::"));
        assert!(assets.contains("pub use nw_jobs::"));
        assert!(assets.contains("pub use nw_localization::"));
        assert!(assets.contains("pub use nw_objectstream::"));
        assert!(assets.contains("pub use nw_pak::"));
        assert!(assets.contains("pub(crate) struct PakDatasheetSource"));
        assert!(assets.contains("pub(crate) fn datasheet_source"));
        assert!(assets.contains("pub(crate) fn load_pak_datasheet_source"));
        assert!(assets.contains("std::fs::canonicalize(path)"));
        assert!(assets.contains("std::fs::symlink_metadata(path)"));
        assert!(assets.contains("pak {} is not under asset root {}"));
        assert!(!assets.contains("pub struct PakDatasheetSource"));
        assert!(!assets.contains("pub fn datasheet_source"));
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
