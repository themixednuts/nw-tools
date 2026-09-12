use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT_CASE_ID: AtomicUsize = AtomicUsize::new(0);

struct LuaTools {
    lua: PathBuf,
    luac: PathBuf,
}

/// Environment variable pointing at a Lua 5.1 `lua` binary. The reference
/// interpreter lives outside this repo, so tests that shell out to it read
/// the path from here instead of a machine-specific literal.
pub const LUA_EXE_ENV: &str = "NW_LUA_EXE";
/// Environment variable pointing at a Lua 5.1 `luac` binary (see [`LUA_EXE_ENV`]).
pub const LUAC_EXE_ENV: &str = "NW_LUAC_EXE";
/// Environment variable pointing at an external Lua corpus checkout used
/// only as decompile input by the corpus tests.
pub const GOOD_LUA_ROOT_ENV: &str = "NW_LUA_GOOD_LUA_ROOT";
/// Environment variable pointing at an external Lua corpus checkout used
/// only as decompile input by the corpus tests.
pub const DEMOJSON_ROOT_ENV: &str = "NW_LUA_DEMOJSON_ROOT";

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

/// Reference `lua` binary, or `None` when unset or missing so tests skip.
#[allow(dead_code)]
pub fn lua_path() -> Option<PathBuf> {
    env_path(LUA_EXE_ENV).filter(|path| path.exists())
}

/// Reference `luac` binary, or `None` when unset or missing so tests skip.
#[allow(dead_code)]
pub fn luac_path() -> Option<PathBuf> {
    env_path(LUAC_EXE_ENV).filter(|path| path.exists())
}

/// External good-lua corpus root, or `None` when unset or missing.
#[allow(dead_code)]
pub fn good_lua_root() -> Option<PathBuf> {
    env_path(GOOD_LUA_ROOT_ENV).filter(|path| path.exists())
}

/// External DEMOJSON corpus root, or `None` when unset or missing.
#[allow(dead_code)]
pub fn demojson_root() -> Option<PathBuf> {
    env_path(DEMOJSON_ROOT_ENV).filter(|path| path.exists())
}

/// A file under the good-lua corpus root, or `None` when unavailable.
#[allow(dead_code)]
pub fn good_lua_fixture(relative: &str) -> Option<PathBuf> {
    good_lua_root()
        .map(|root| root.join(relative))
        .filter(|path| path.exists())
}

/// A file under the DEMOJSON corpus root, or `None` when unavailable.
#[allow(dead_code)]
pub fn demojson_fixture(relative: &str) -> Option<PathBuf> {
    demojson_root()
        .map(|root| root.join(relative))
        .filter(|path| path.exists())
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

impl Drop for CasePaths {
    fn drop(&mut self) {
        self.cleanup();
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

#[allow(dead_code)]
pub fn compile_source_bytes(name: &str, source: &str, strip_debug: bool) -> Option<Vec<u8>> {
    let tools = lua_tools()?;
    let paths = CasePaths::new(name);

    fs::write(&paths.source, source).expect("write Lua source");
    compile_lua(&tools.luac, &paths.source, &paths.bytecode, strip_debug);
    let bytecode = fs::read(&paths.bytecode).expect("read compiled bytecode");

    paths.cleanup();
    Some(bytecode)
}

#[allow(dead_code)]
pub fn compile_file_bytes(name: &str, source: &Path, strip_debug: bool) -> Option<Vec<u8>> {
    let tools = lua_tools()?;
    let paths = CasePaths::new(name);

    compile_lua(&tools.luac, source, &paths.bytecode, strip_debug);
    let bytecode = fs::read(&paths.bytecode).expect("read compiled bytecode");

    paths.cleanup();
    Some(bytecode)
}

#[allow(dead_code)]
pub fn run_bytecode_equivalence(name: &str, bytecode: &[u8], args: &[&str]) -> Option<String> {
    let tools = lua_tools()?;
    let paths = CasePaths::new(name);

    fs::write(&paths.bytecode, bytecode).expect("write original bytecode");
    let original_stdout = run_lua(&tools.lua, &paths.bytecode, args, "original Lua bytecode");
    let decompiled = nw_lua::decompile(bytecode)
        .unwrap_or_else(|error| panic!("{name} failed to decompile bytecode: {error}"));
    full_moon::parse(&decompiled).expect("decompiled source reparses with full_moon");

    fs::write(&paths.decompiled, &decompiled).expect("write decompiled Lua source");
    let decompiled_stdout = run_lua(&tools.lua, &paths.decompiled, args, "decompiled Lua source");
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

fn run_equivalence_inner(
    name: &str,
    source: &str,
    args: &[&str],
    strip_debug: bool,
) -> Option<String> {
    let tools = lua_tools()?;
    let paths = CasePaths::new(name);

    fs::write(&paths.source, source).expect("write original Lua source");
    compile_lua(&tools.luac, &paths.source, &paths.bytecode, strip_debug);

    let original_stdout = run_lua(&tools.lua, &paths.source, args, "original Lua source");
    let bytecode = fs::read(&paths.bytecode).expect("read compiled bytecode");
    let decompiled = nw_lua::decompile(&bytecode)
        .unwrap_or_else(|error| panic!("{name} failed to decompile bytecode: {error}"));
    full_moon::parse(&decompiled).expect("decompiled source reparses with full_moon");

    fs::write(&paths.decompiled, &decompiled).expect("write decompiled Lua source");
    let decompiled_stdout = run_lua(&tools.lua, &paths.decompiled, args, "decompiled Lua source");
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
    let (Some(lua), Some(luac)) = (lua_path(), luac_path()) else {
        eprintln!(
            "skipping Lua 5.1 runtime equivalence tests; set {LUA_EXE_ENV} and {LUAC_EXE_ENV} to the reference binaries"
        );
        return None;
    };

    Some(LuaTools { lua, luac })
}

fn run_lua(lua: &Path, source: &Path, args: &[&str], context: &str) -> Vec<u8> {
    let mut child = Command::new(lua)
        .arg(source)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("failed to run {context}: {err}"));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child
                    .wait_with_output()
                    .unwrap_or_else(|err| panic!("failed to collect {context}: {err}"));
                return assert_success(output, context);
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{context} timed out after 10s: {}", source.display());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(err) => panic!("failed to wait for {context}: {err}"),
        }
    }
}

fn compile_lua(luac: &Path, source: &Path, bytecode: &Path, strip_debug: bool) {
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
