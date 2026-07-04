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
