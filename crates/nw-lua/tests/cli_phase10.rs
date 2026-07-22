use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(path)
}

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nw-lua"))
}

#[test]
fn cli_disassembles_shopcommon() {
    let output = command()
        .arg("--dis")
        .arg(fixture("shopcommon.luac"))
        .output()
        .expect("run nw-lua --dis");

    let stdout = assert_success(output, "--dis");
    assert!(!stdout.trim().is_empty());
    assert!(stdout.contains("GETGLOBAL"), "{stdout}");
    assert!(stdout.contains("RETURN"), "{stdout}");
}

#[test]
fn cli_decompiles_shopcommon_by_default_and_reparses() {
    let output = command()
        .arg(fixture("shopcommon.luac"))
        .output()
        .expect("run nw-lua default decompile");

    let stdout = assert_success(output, "default decompile");
    assert!(!stdout.trim().is_empty());
    full_moon::parse(&stdout).unwrap_or_else(|errors| {
        panic!("CLI decompiled output did not parse:\n{stdout}\n{errors:#?}")
    });
    assert!(stdout.contains("local Shopcommon = {"), "{stdout}");
    assert!(stdout.contains("ShopCurrencyType = {"), "{stdout}");
    assert!(
        !stdout.contains("Shopcommon.ShopCurrencyType ="),
        "{stdout}"
    );
    assert!(stdout.contains("function Shopcommon.OpenShop("), "{stdout}");
    assert!(stdout.contains("return Shopcommon"), "{stdout}");
    assert!(!stdout.contains("local v4_2 = {}"), "{stdout}");
    assert!(!stdout.contains("return v4_2"), "{stdout}");
}

#[test]
fn cli_ssa_dump_is_non_empty_and_deterministic() {
    let first = command()
        .arg("--ssa-dump")
        .arg(fixture("shopcommon.luac"))
        .output()
        .expect("run first nw-lua --ssa-dump");
    let second = command()
        .arg("--ssa-dump")
        .arg(fixture("shopcommon.luac"))
        .output()
        .expect("run second nw-lua --ssa-dump");

    let first = assert_success(first, "first --ssa-dump");
    let second = assert_success(second, "second --ssa-dump");
    assert!(!first.trim().is_empty());
    assert_eq!(first, second);
}

#[test]
fn cli_rejects_future_lua_version_override_cleanly() {
    let output = command()
        .args(["--lua-version", "54"])
        .arg(fixture("shopcommon.luac"))
        .output()
        .expect("run nw-lua --lua-version 54");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported Lua version 5.4"), "{stderr}");
    assert!(!stderr.to_ascii_lowercase().contains("panic"), "{stderr}");
}

#[test]
fn cli_accepts_custom_opcode_table() {
    let output = command()
        .arg("--dis")
        .arg("--opcode-table")
        .arg(fixture("idle_heroes.txt"))
        .arg(fixture("shopcommon.luac"))
        .output()
        .expect("run nw-lua with custom opcode table");

    let stdout = assert_success(output, "custom opcode table");
    assert!(!stdout.trim().is_empty());
    assert!(stdout.contains("-- Lua 5.1 Disassembly --"), "{stdout}");
}

#[test]
fn cli_reads_luac_from_stdin() {
    let bytes = fs::read(fixture("shopcommon.luac")).expect("read shopcommon fixture");
    let mut child = command()
        .arg("--dis")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nw-lua --dis -");

    child
        .stdin
        .as_mut()
        .expect("child stdin is piped")
        .write_all(&bytes)
        .expect("write luac bytes to stdin");

    let output = child.wait_with_output().expect("wait for nw-lua");
    let stdout = assert_success(output, "stdin --dis");
    assert!(stdout.contains("GETGLOBAL"), "{stdout}");
}

#[test]
fn cli_parallel_batch_decompiles_to_deterministic_output_names() {
    let output_dir = tempfile::tempdir().expect("create batch output directory");
    let output = command()
        .args(["--jobs", "2", "--output"])
        .arg(output_dir.path())
        .arg(fixture("shopcommon.luac"))
        .arg(fixture("linear/local_add.luac"))
        .output()
        .expect("run parallel nw-lua batch");

    let stdout = assert_success(output, "parallel batch");
    assert!(stdout.is_empty(), "batch output belongs in files: {stdout}");
    for name in ["shopcommon.lua", "local_add.lua"] {
        let source = fs::read_to_string(output_dir.path().join(name))
            .unwrap_or_else(|error| panic!("read {name}: {error}"));
        full_moon::parse(&source)
            .unwrap_or_else(|errors| panic!("{name} did not parse:\n{source}\n{errors:#?}"));
    }
}

#[test]
fn cli_batch_requires_an_output_directory() {
    let output = command()
        .arg(fixture("shopcommon.luac"))
        .arg(fixture("linear/local_add.luac"))
        .output()
        .expect("run invalid nw-lua batch");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("require an output directory"), "{stderr}");
}

fn assert_success(output: Output, context: &str) -> String {
    assert!(
        output.status.success(),
        "{context} failed with status {}\nstderr:\n{}\nstdout:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    String::from_utf8(output.stdout).expect("CLI stdout is UTF-8")
}
