use std::{env, path::PathBuf};

fn main() {
    if env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    for dir in candidate_dirs() {
        if dir.join("oo2core_win64.lib").is_file() {
            println!("cargo:rustc-link-search=native={}", dir.display());
            println!("cargo:rustc-link-lib=oo2core_win64");
            return;
        }
    }

    println!(
        "cargo:warning=oo2core_win64.lib not found; set OODLE_LIB_DIR or copy it to vendor/oodle"
    );
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(dir) = env::var_os("OODLE_LIB_DIR") {
        dirs.push(PathBuf::from(dir));
    }

    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"));
    dirs.push(manifest_dir.clone());

    if let Some(root) = manifest_dir.ancestors().nth(2) {
        dirs.push(root.join("vendor").join("oodle"));
    }

    dirs
}
