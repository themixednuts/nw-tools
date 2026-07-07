use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::emit::GameDataCodegenFile;

const OODLE_DLL_NAMES: &[&str] = &["oo2core_9_win64.dll", "oo2core_8_win64.dll"];
const OODLE_RUST_LINK_LIB: &str = "oo2core_win64.lib";

pub(crate) fn oodle_runtime_files() -> Result<Vec<GameDataCodegenFile>> {
    let resource_root = oodle_resource_root();
    let mut files = Vec::new();
    for file_name in OODLE_DLL_NAMES {
        let path = resource_root.join(file_name);
        let bytes = fs::read(&path)
            .with_context(|| format!("read bundled Oodle DLL {}", path.display()))?;
        files.push(GameDataCodegenFile::binary(
            PathBuf::from("bin").join(file_name),
            bytes,
        ));
    }
    let link_lib_path = resource_root.join(OODLE_RUST_LINK_LIB);
    let link_lib = fs::read(&link_lib_path).with_context(|| {
        format!(
            "read bundled Oodle Rust import library {}",
            link_lib_path.display()
        )
    })?;
    files.push(GameDataCodegenFile::binary(
        PathBuf::from("bin").join(OODLE_RUST_LINK_LIB),
        link_lib,
    ));
    let rust_dll_path = resource_root.join("oo2core_9_win64.dll");
    let rust_dll = fs::read(&rust_dll_path).with_context(|| {
        format!(
            "read bundled Oodle Rust runtime DLL {}",
            rust_dll_path.display()
        )
    })?;
    files.push(GameDataCodegenFile::binary(
        PathBuf::from("bin").join("oo2core_win64.dll"),
        rust_dll,
    ));
    Ok(files)
}

fn oodle_resource_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/oodle")
        .components()
        .collect()
}
