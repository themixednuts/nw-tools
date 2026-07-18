use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::emit::GameDataCodegenFile;

const OODLE_DLL_NAMES: &[&str] = &["oo2core_9_win64.dll", "oo2core_8_win64.dll"];
const OODLE_RUST_LINK_LIB: &str = "oo2core_win64.lib";

pub(crate) fn oodle_dynamic_runtime_files() -> Result<Vec<GameDataCodegenFile>> {
    let resource_root = oodle_resource_root();
    let mut files = Vec::new();
    for file_name in OODLE_DLL_NAMES {
        files.push(GameDataCodegenFile::binary(
            PathBuf::from("bin").join(file_name),
            read_oodle_resource(&resource_root, file_name, "DLL")?,
        ));
    }
    Ok(files)
}

pub(crate) fn oodle_rust_runtime_files() -> Result<Vec<GameDataCodegenFile>> {
    let resource_root = oodle_resource_root();
    let link_lib_path = resource_root.join(OODLE_RUST_LINK_LIB);
    let link_lib = fs::read(&link_lib_path).with_context(|| {
        format!(
            "read bundled Oodle Rust import library {}",
            link_lib_path.display()
        )
    })?;
    Ok(vec![
        GameDataCodegenFile::binary(PathBuf::from("bin").join(OODLE_RUST_LINK_LIB), link_lib),
        GameDataCodegenFile::binary(
            PathBuf::from("bin").join("oo2core_win64.dll"),
            read_oodle_resource(&resource_root, "oo2core_9_win64.dll", "Rust runtime DLL")?,
        ),
    ])
}

fn read_oodle_resource(root: &Path, file_name: &str, kind: &str) -> Result<Vec<u8>> {
    let path = root.join(file_name);
    fs::read(&path).with_context(|| format!("read bundled Oodle {kind} {}", path.display()))
}

fn oodle_resource_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/oodle")
        .components()
        .collect()
}
