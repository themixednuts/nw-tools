use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_CASE_ID: AtomicUsize = AtomicUsize::new(0);

struct LuaTools {
    lua: &'static str,
    luac: &'static str,
}

struct CasePaths {
    source: PathBuf,
    bytecode: PathBuf,
    decompiled: PathBuf,
}

impl CasePaths {
    fn new(name: &str) -> Self {
        let id = NEXT_CASE_ID.fetch_add(1, Ordering::Relaxed);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_millis();
        let stem = format!(
            "nw_lua_{name}_{}_{}",
            std::process::id(),
            millis + id as u128
        );
        let dir = std::env::temp_dir();
        Self {
            source: dir.join(format!("{stem}.lua")),
            bytecode: dir.join(format!("{stem}.luac")),
            decompiled: dir.join(format!("{stem}_decompiled.lua")),
        }
    }

    fn cleanup(&self) {
        let _ = fs::remove_file(&self.source);
        let _ = fs::remove_file(&self.bytecode);
        let _ = fs::remove_file(&self.decompiled);
    }
}

#[allow(dead_code)]
pub fn run_equivalence(name: &str, source: &str) -> Option<String> {
    run_equivalence_with_args(name, source, &[])
}

#[allow(dead_code)]
pub fn run_stripped_equivalence(name: &str, source: &str) -> Option<String> {
    run_equivalence_inner(name, source, &[], true)
}

#[allow(dead_code)]
pub fn run_equivalence_with_args(name: &str, source: &str, args: &[&str]) -> Option<String> {
    run_equivalence_inner(name, source, args, false)
}

fn run_equivalence_inner(
    name: &str,
    source: &str,
    args: &[&str],
    strip_debug: bool,
) -> Option<String> {
    let tools = lua_tools()?;
    let paths = CasePaths::new(name);

    fs::write(&paths.source, source).expect("write original Lua source");
    compile_lua(tools.luac, &paths.source, &paths.bytecode, strip_debug);

    let original_stdout = run_lua(tools.lua, &paths.source, args, "original Lua source");
    let bytecode = fs::read(&paths.bytecode).expect("read compiled bytecode");
    let decompiled = nw_lua::decompile(&bytecode).expect("decompile bytecode");
    full_moon::parse(&decompiled).expect("decompiled source reparses with full_moon");

    fs::write(&paths.decompiled, &decompiled).expect("write decompiled Lua source");
    let decompiled_stdout = run_lua(tools.lua, &paths.decompiled, args, "decompiled Lua source");
    assert_eq!(
        original_stdout,
        decompiled_stdout,
        "{name} stdout differed\noriginal:\n{}\ndecompiled source:\n{}\ndecompiled stdout:\n{}",
        String::from_utf8_lossy(&original_stdout),
        decompiled,
        String::from_utf8_lossy(&decompiled_stdout)
    );

    paths.cleanup();
    Some(decompiled)
}

fn lua_tools() -> Option<LuaTools> {
    let tools = LuaTools {
        lua: r"E:\Projects\lua-5.1.5\src\lua.exe",
        luac: r"E:\Projects\lua-5.1.5\src\luac.exe",
    };

    if !Path::new(tools.lua).exists() || !Path::new(tools.luac).exists() {
        eprintln!(
            "skipping Lua 5.1 runtime equivalence tests; missing lua.exe or luac.exe at expected paths"
        );
        return None;
    }

    Some(tools)
}

fn run_lua(lua: &str, source: &Path, args: &[&str], context: &str) -> Vec<u8> {
    let output = Command::new(lua)
        .arg(source)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run {context}: {err}"));
    assert_success(output, context)
}

fn compile_lua(luac: &str, source: &Path, bytecode: &Path, strip_debug: bool) {
    let mut command = Command::new(luac);
    if strip_debug {
        command.arg("-s");
    }
    let output = command
        .arg("-o")
        .arg(bytecode)
        .arg(source)
        .output()
        .unwrap_or_else(|err| panic!("failed to run luac: {err}"));
    let _ = assert_success(output, "luac compile");
}

fn assert_success(output: Output, context: &str) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{context} failed with status {}\nstderr:\n{}\nstdout:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    output.stdout
}
