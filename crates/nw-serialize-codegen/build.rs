use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set for build scripts"),
    );
    let rust_serialize_context_inputs = rust_serialize_context_product_inputs(&manifest_dir);
    emit_rerun_if_changed(&manifest_dir, &rust_serialize_context_inputs);
    println!(
        "cargo:rustc-env=NW_SERIALIZE_CODEGEN_RUST_SERIALIZE_CONTEXT_EMITTER_COMPILER_FINGERPRINT={}",
        fingerprint_inputs(&manifest_dir, rust_serialize_context_inputs)
    );

    if env::var_os("CARGO_FEATURE_FULL").is_some() {
        let mut inputs = vec![
            manifest_dir.join("Cargo.toml"),
            manifest_dir.join("build.rs"),
        ];
        collect_files(&manifest_dir.join("src"), &mut inputs);
        collect_files(&manifest_dir.join("resources"), &mut inputs);
        emit_rerun_if_changed(&manifest_dir, &inputs);
        println!(
            "cargo:rustc-env=NW_SERIALIZE_CODEGEN_SOURCE_FINGERPRINT={}",
            fingerprint_inputs(&manifest_dir, inputs)
        );
    }
}

fn emit_rerun_if_changed(manifest_dir: &Path, inputs: &[PathBuf]) {
    for path in inputs {
        println!(
            "cargo:rerun-if-changed={}",
            normalized_relative_path(manifest_dir, path)
        );
    }
}

/// Sources that can change New World's SerializeContext compiler or integrated
/// Rust emitter result. Keep this list deliberately narrower than the crate's
/// complete source fingerprint: the network, Go, TypeScript, CLI, and
/// component-scaffold products do not participate in that build-script API.
///
/// The list includes the compiler pipeline and every Rust-emission dependency
/// reachable from the APIs used by `gems/newworld/runtime/build.rs`. It excludes
/// test-only files because they cannot affect the shipped generator product.
const RUST_SERIALIZE_CONTEXT_PRODUCT_SOURCE_FILES: &[&str] = &[
    "Cargo.toml",
    "build.rs",
    "src/lib.rs",
    "src/catalog.rs",
    "src/class_registration.rs",
    "src/compiler.rs",
    "src/compiler/diagnostics.rs",
    "src/compiler/error.rs",
    "src/compiler/input.rs",
    "src/completion.rs",
    "src/context.rs",
    "src/document.rs",
    "src/field_evidence.rs",
    "src/field_projection.rs",
    "src/graph.rs",
    "src/ir.rs",
    "src/layout.rs",
    "src/lint.rs",
    "src/model.rs",
    "src/module_descriptors.rs",
    "src/naming.rs",
    "src/native.rs",
    "src/reference.rs",
    "src/role.rs",
    "src/selection.rs",
    "src/status.rs",
    "src/symbol_surface.rs",
    "src/types.rs",
    "src/uuid_format.rs",
    "src/value.rs",
];

/// Entire shared implementation surfaces used by the compiler and Rust emitter.
/// A new helper module below one of these roots must invalidate the product;
/// keeping them as roots avoids an unsound hand-maintained per-file closure.
const RUST_SERIALIZE_CONTEXT_PRODUCT_SOURCE_DIRECTORIES: &[&str] =
    &["src/layout", "src/model", "src/rust", "src/schema"];

fn rust_serialize_context_product_inputs(manifest_dir: &Path) -> Vec<PathBuf> {
    let mut inputs = RUST_SERIALIZE_CONTEXT_PRODUCT_SOURCE_FILES
        .iter()
        .map(|relative| manifest_dir.join(relative))
        .collect::<Vec<_>>();
    for relative in RUST_SERIALIZE_CONTEXT_PRODUCT_SOURCE_DIRECTORIES {
        collect_product_files(&manifest_dir.join(relative), &mut inputs);
    }
    inputs
}

fn collect_product_files(root: &Path, files: &mut Vec<PathBuf>) {
    let mut product_files = Vec::new();
    collect_files(root, &mut product_files);
    files.extend(
        product_files
            .into_iter()
            .filter(|path| is_product_source_file(path)),
    );
}

fn is_product_source_file(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) != Some("tests.rs")
        && !path
            .components()
            .any(|component| component.as_os_str() == "tests")
}

fn fingerprint_inputs(manifest_dir: &Path, mut inputs: Vec<PathBuf>) -> blake3::Hash {
    inputs.sort_by_key(|path| normalized_relative_path(manifest_dir, path));

    let mut hasher = blake3::Hasher::new();
    for path in inputs {
        let relative = normalized_relative_path(manifest_dir, &path);
        let bytes = fs::read(&path)
            .unwrap_or_else(|error| panic!("read generator source {}: {error}", path.display()));
        hasher.update(&(relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    hasher.finalize()
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }
    let mut entries = fs::read_dir(root)
        .unwrap_or_else(|error| {
            panic!(
                "read generator source directory {}: {error}",
                root.display()
            )
        })
        .map(|entry| entry.expect("read generator source directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_files(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
    }
}

fn normalized_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("generator source input is under the crate root")
        .to_string_lossy()
        .replace('\\', "/")
}
