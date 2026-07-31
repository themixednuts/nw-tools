use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-env-changed=OODLE_LIB_DIR");
    println!("cargo:rustc-check-cfg=cfg(oodle_provider)");

    if env::var_os("CARGO_FEATURE_OODLE").is_none() {
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let library_names: &[&str] = match target_os.as_str() {
        "windows" => &["oo2core_win64.lib"],
        "linux" => &["liboo2corelinux64.a", "liboo2corelinux64.so"],
        _ => {
            println!(
                "cargo:warning=no built-in Oodle provider convention for target OS `{target_os}`"
            );
            return;
        }
    };

    let candidates = candidate_dirs();
    for dir in &candidates {
        let Some(library_name) = library_names
            .iter()
            .find(|library_name| dir.join(library_name).is_file())
        else {
            continue;
        };
        println!("cargo:rustc-link-search=native={}", dir.display());
        match (target_os.as_str(), *library_name) {
            ("windows", _) => println!("cargo:rustc-link-lib=oo2core_win64"),
            ("linux", name) if name.ends_with(".a") => {
                println!("cargo:rustc-link-lib=static=oo2corelinux64");
                println!("cargo:rustc-link-lib=dylib=stdc++");
            }
            ("linux", _) => println!("cargo:rustc-link-lib=dylib=oo2corelinux64"),
            _ => unreachable!("target OS was validated above"),
        }
        println!("cargo:rustc-cfg=oodle_provider");
        return;
    }

    println!(
        "cargo:warning=the `oodle` feature is enabled, but none of {} was found in: {}; \
         Oodle operations will return `Unavailable`. Set OODLE_LIB_DIR or place a licensed \
         target library in a workspace `vendor/oodle` directory",
        library_names.join(", "),
        display_dirs(&candidates)
    );
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(dir) = env::var_os("OODLE_LIB_DIR") {
        dirs.push(PathBuf::from(dir));
    }

    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"));
    let workspace_root = find_workspace_root(&manifest_dir).unwrap_or(&manifest_dir);
    dirs.push(workspace_root.join("vendor").join("oodle"));
    if let Some(parent) = workspace_root.parent() {
        dirs.push(parent.join("az-rs").join("vendor").join("oodle"));
        dirs.push(parent.join("new-world").join("vendor").join("oodle"));
    }

    dirs
}

fn find_workspace_root(start: &Path) -> Option<&Path> {
    start
        .ancestors()
        .find(|path| path.join("Cargo.toml").is_file() && path.join("crates").is_dir())
}

fn display_dirs(dirs: &[PathBuf]) -> String {
    dirs.iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}
